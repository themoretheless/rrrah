/* Auto Cut — pure logic shared by the panel and the test suite.
 *
 * Nothing in here touches Premiere, ffmpeg, or the DOM: it is all
 * string-in / number-out, so `scripts/test.js` can exercise the real code
 * paths under plain Node.
 *
 * Vocabulary
 *   source time    — seconds into the media file on disk
 *   sequence time  — seconds along the Premiere timeline
 *   clip           — { name, mediaPath, start, end, inPoint, speed,
 *                      trackType, trackIndex }
 *                    start/end are sequence time, inPoint is the source time
 *                    shown at `start`.
 */
(function (factory) {
	'use strict';
	var api = factory();
	// CEP runs the panel with Node injected into the page, so `module` and
	// `module.exports` exist in the browser context too. A normal UMD would
	// take the Node branch here and never define the window global, leaving
	// the panel with no core at all — so publish the global whenever there is
	// a document, and export for CommonJS independently (the tests).
	if (typeof window !== 'undefined' && window.document) { window.AutoCutCore = api; }
	if (typeof module === 'object' && module.exports) { module.exports = api; }
}(function () {
	'use strict';

	// Ranges closer together than this are treated as touching. One frame at
	// 60fps is ~0.0167s, so this stays under a frame at common rates.
	var EPS = 0.012;

	// ---------------------------------------------------------------- presets

	// Starting points per footage type. `noiseDb` is only a fallback — the
	// panel's "Measure" button replaces it with a level derived from the real
	// audio, which beats any preset.
	var PRESETS = {
		'talking-head': {
			label: 'Talking head',
			noiseDb: -35, minSilence: 0.5, padding: 0.05, mergeWithin: 0.25, minKeep: 0.3,
			hint: 'Close mic, one speaker. Trims breaths and dead air between sentences.'
		},
		'interview': {
			label: 'Interview / podcast',
			noiseDb: -38, minSilence: 0.9, padding: 0.12, mergeWithin: 0.4, minKeep: 0.5,
			hint: 'Keeps natural thinking pauses so answers do not sound clipped.'
		},
		'screencast': {
			label: 'Screencast / tutorial',
			noiseDb: -40, minSilence: 0.7, padding: 0.08, mergeWithin: 0.3, minKeep: 0.4,
			hint: 'Quiet room, long typing gaps. Cuts hard but leaves room to follow along.'
		},
		'aggressive': {
			label: 'Aggressive (jump cuts)',
			noiseDb: -32, minSilence: 0.25, padding: 0.02, mergeWithin: 0.15, minKeep: 0.2,
			hint: 'Fast, punchy edit. Expect visible jump cuts — best with b-roll on top.'
		}
	};

	function presetIds() { return Object.keys(PRESETS); }

	// ------------------------------------------------- ffmpeg output parsing

	// Parse `silencedetect` stderr into [{ start, end|null }] in source seconds.
	// `end` is null when a silence runs to end-of-file: ffmpeg logs no
	// silence_end for it, and the caller clamps it to the clip's out point.
	function parseSilenceDetect(stderrText) {
		var silences = [];
		var pending = null;
		var lines = String(stderrText).split('\n');
		for (var i = 0; i < lines.length; i++) {
			var startMatch = lines[i].match(/silence_start:\s*(-?[\d.]+)/);
			if (startMatch) {
				// Two starts in a row: the first silence never closed.
				if (pending !== null) { silences.push({ start: pending, end: null }); }
				pending = parseFloat(startMatch[1]);
				continue;
			}
			var endMatch = lines[i].match(/silence_end:\s*(-?[\d.]+)/);
			if (endMatch && pending !== null) {
				silences.push({ start: pending, end: parseFloat(endMatch[1]) });
				pending = null;
			}
		}
		if (pending !== null) { silences.push({ start: pending, end: null }); }
		return silences;
	}

	// Parse `volumedetect` stderr into { meanDb, maxDb }. Either may be null
	// when ffmpeg could not measure it (silent or missing audio stream).
	function parseVolumeDetect(stderrText) {
		var text = String(stderrText);
		var mean = text.match(/mean_volume:\s*(-?[\d.]+)\s*dB/);
		var max = text.match(/max_volume:\s*(-?[\d.]+)\s*dB/);
		return {
			meanDb: mean ? parseFloat(mean[1]) : null,
			maxDb: max ? parseFloat(max[1]) : null
		};
	}

	// Turn a volumedetect measurement into a silence threshold.
	//
	// mean_volume sits between the noise floor and speech level, so it is a
	// decent proxy for "quieter than this is not speech". We bias a little
	// below the mean to protect soft word endings, then clamp to a sane band
	// so a pathological measurement cannot produce a threshold that either
	// cuts everything or nothing.
	function suggestNoiseFloor(volume, opts) {
		var o = opts || {};
		var bias = o.bias === undefined ? 6 : o.bias;   // dB below mean
		var floor = o.floor === undefined ? -60 : o.floor;
		var ceil = o.ceil === undefined ? -20 : o.ceil;
		if (!volume || volume.meanDb === null || !isFinite(volume.meanDb)) { return null; }
		var suggested = volume.meanDb - bias;
		// Never land above the loudest sample — that would cut the whole clip.
		if (volume.maxDb !== null && isFinite(volume.maxDb)) {
			suggested = Math.min(suggested, volume.maxDb - 12);
		}
		return Math.round(clamp(suggested, floor, ceil));
	}

	function clamp(v, lo, hi) { return Math.max(lo, Math.min(hi, v)); }

	// ------------------------------------------------------- track targeting

	// Which clips drive silence analysis. `videoTrack`/`audioTrack` are
	// 0-based indices, or null to ignore that media type. Everything not
	// selected is still cut at the same ranges by the engine, keeping sync.
	function selectAnalysisClips(clips, target) {
		var t = target || {};
		var vIdx = t.videoTrack === undefined ? 0 : t.videoTrack;
		var aIdx = t.audioTrack === undefined ? 0 : t.audioTrack;
		return clips.filter(function (c) {
			if (c.trackType === 'video') { return vIdx !== null && c.trackIndex === vIdx; }
			if (c.trackType === 'audio') { return aIdx !== null && c.trackIndex === aIdx; }
			return false;
		});
	}

	// ---------------------------------------------------------- cut planning

	// Map one clip's source-time silences onto sequence-time cut ranges,
	// shrunk by `padding` at each end so cuts keep a little air.
	function cutsForClip(clip, silences, opts) {
		var o = opts || {};
		var padding = o.padding || 0;
		var minCut = o.minCut === undefined ? 0.1 : o.minCut;
		var srcIn = clip.inPoint;
		var srcOut = clip.inPoint + (clip.end - clip.start);
		var ranges = [];
		for (var i = 0; i < silences.length; i++) {
			var s = silences[i];
			// A silence running to EOF ends, for our purposes, where the clip does.
			var silEnd = (s.end === null || s.end === undefined) ? srcOut : s.end;
			var ovStart = Math.max(s.start + padding, srcIn);
			var ovEnd = Math.min(silEnd - padding, srcOut);
			if (ovEnd - ovStart < minCut) { continue; }
			ranges.push([
				clip.start + (ovStart - srcIn),
				clip.start + (ovEnd - srcIn)
			]);
		}
		return ranges;
	}

	// Union of overlapping/touching ranges, ascending. `within` additionally
	// merges cuts separated by a gap smaller than itself.
	function mergeRanges(ranges, within) {
		var slack = (within === undefined ? 0 : within) + EPS;
		var sorted = ranges.map(function (r) { return [r[0], r[1]]; })
			.sort(function (a, b) { return a[0] - b[0]; });
		var out = [];
		for (var i = 0; i < sorted.length; i++) {
			var r = sorted[i];
			var last = out[out.length - 1];
			if (last && r[0] <= last[1] + slack) { last[1] = Math.max(last[1], r[1]); }
			else { out.push(r); }
		}
		return out;
	}

	// Drop kept fragments shorter than `minKeep` by merging the cuts around
	// them. Without this, two silences separated by a stray 80ms click leave a
	// useless sliver of footage between two cuts.
	function absorbShortKeeps(ranges, minKeep) {
		if (!minKeep || ranges.length < 2) { return ranges; }
		var out = [ranges[0].slice()];
		for (var i = 1; i < ranges.length; i++) {
			var prev = out[out.length - 1];
			var cur = ranges[i];
			if (cur[0] - prev[1] < minKeep) { prev[1] = Math.max(prev[1], cur[1]); }
			else { out.push(cur.slice()); }
		}
		return out;
	}

	// Snap a time to the frame grid. Cuts must land on frame boundaries or
	// Premiere rounds them itself and tracks drift apart by a frame.
	function snapToFrame(seconds, fps, mode) {
		if (!fps || !isFinite(fps) || fps <= 0) { return seconds; }
		var frames = seconds * fps;
		var rounded = mode === 'floor' ? Math.floor(frames + 1e-6)
			: mode === 'ceil' ? Math.ceil(frames - 1e-6)
				: Math.round(frames);
		return rounded / fps;
	}

	// Snap a cut range inward: start up, end down. Rounding inward can only
	// ever keep a frame that should have gone, never eat a frame of speech.
	function snapRange(range, fps) {
		if (!fps) { return [range[0], range[1]]; }
		return [snapToFrame(range[0], fps, 'ceil'), snapToFrame(range[1], fps, 'floor')];
	}

	// Build the whole cut plan for a sequence.
	//
	// Returns ranges sorted DESCENDING, because the engine applies them
	// right-to-left: every range below the one being cut keeps its timing
	// while later content shifts left.
	function planCuts(clips, silenceByPath, opts) {
		var o = opts || {};
		var warnings = [];
		var all = [];
		var analyzed = 0;

		for (var i = 0; i < clips.length; i++) {
			var clip = clips[i];
			if (clip.speed !== undefined && clip.speed !== null && Math.abs(clip.speed - 1) > 1e-6) {
				warnings.push('Skipped "' + clip.name + '": ' + clip.speed +
					'x speed clips cannot be mapped 1:1 to source time.');
				continue;
			}
			var silences = silenceByPath[clip.mediaPath];
			if (!silences) {
				warnings.push('No audio analysis for "' + clip.name + '" (' +
					(clip.mediaPath || 'no media path') + ').');
				continue;
			}
			analyzed++;
			all = all.concat(cutsForClip(clip, silences, o));
		}

		var ranges = mergeRanges(all, o.mergeWithin);
		ranges = absorbShortKeeps(ranges, o.minKeep);

		if (o.fps) {
			ranges = ranges.map(function (r) { return snapRange(r, o.fps); });
			// Inward snapping can shrink a marginal range below the floor.
			var minCut = o.minCut === undefined ? 0.1 : o.minCut;
			ranges = ranges.filter(function (r) { return r[1] - r[0] >= minCut; });
		}

		// Never cut past the end of the analyzed material.
		if (o.sequenceEnd) {
			ranges = ranges.filter(function (r) { return r[0] < o.sequenceEnd - EPS; })
				.map(function (r) { return [r[0], Math.min(r[1], o.sequenceEnd)]; });
		}

		ranges.sort(function (a, b) { return b[0] - a[0]; });
		return { ranges: ranges, warnings: warnings, analyzedClips: analyzed };
	}

	// --------------------------------------------------------------- summary

	function summarizePlan(ranges, timelineLength) {
		var removed = 0;
		for (var i = 0; i < ranges.length; i++) { removed += ranges[i][1] - ranges[i][0]; }
		var pct = timelineLength > 0 ? (removed / timelineLength) * 100 : 0;
		return {
			cuts: ranges.length,
			removedSeconds: round2(removed),
			remainingSeconds: timelineLength ? round2(timelineLength - removed) : null,
			percentRemoved: round2(pct)
		};
	}

	function round2(v) { return Math.round(v * 100) / 100; }

	// 78.4 -> "1:18.4"; used in the plan preview.
	function formatDuration(seconds) {
		var s = Math.max(0, seconds);
		var m = Math.floor(s / 60);
		var rest = s - m * 60;
		if (m >= 60) {
			var h = Math.floor(m / 60);
			return h + ':' + pad2(m - h * 60) + ':' + pad2(Math.floor(rest));
		}
		return m + ':' + (rest < 10 ? '0' : '') + rest.toFixed(1);
	}

	function pad2(n) { return (n < 10 ? '0' : '') + n; }

	// -------------------------------------------------------------- validation

	// Clamp user input to ranges ffmpeg and the engine can actually honour.
	function normalizeParams(raw) {
		var p = raw || {};
		var num = function (v, dflt) {
			var n = Number(v);
			return isFinite(n) ? n : dflt;
		};
		return {
			noiseDb: clamp(num(p.noiseDb, -35), -80, -5),
			minSilence: clamp(num(p.minSilence, 0.5), 0.05, 30),
			padding: clamp(num(p.padding, 0.05), 0, 2),
			mergeWithin: clamp(num(p.mergeWithin, 0.25), 0, 5),
			minKeep: clamp(num(p.minKeep, 0.3), 0, 5),
			minCut: clamp(num(p.minCut, 0.1), 0.02, 5)
		};
	}

	return {
		EPS: EPS,
		PRESETS: PRESETS,
		presetIds: presetIds,
		parseSilenceDetect: parseSilenceDetect,
		parseVolumeDetect: parseVolumeDetect,
		suggestNoiseFloor: suggestNoiseFloor,
		selectAnalysisClips: selectAnalysisClips,
		cutsForClip: cutsForClip,
		mergeRanges: mergeRanges,
		absorbShortKeeps: absorbShortKeeps,
		snapToFrame: snapToFrame,
		snapRange: snapRange,
		planCuts: planCuts,
		summarizePlan: summarizePlan,
		formatDuration: formatDuration,
		normalizeParams: normalizeParams,
		clamp: clamp
	};
}));
