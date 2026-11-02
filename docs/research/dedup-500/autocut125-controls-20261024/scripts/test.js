#!/usr/bin/env node
'use strict';

/* Auto Cut — tests for the shared planning logic.
 *
 * These run the same extension/js/core.js the panel loads, so a regression in
 * cut planning fails here before it ever reaches a timeline.
 *   npm test
 */

const assert = require('node:assert');
const path = require('node:path');
const core = require(path.join(__dirname, '..', 'extension', 'js', 'core.js'));

let passed = 0;
let failed = 0;

function test(name, fn) {
	try {
		fn();
		passed++;
		console.log('  \x1b[32m✓\x1b[0m ' + name);
	} catch (e) {
		failed++;
		console.log('  \x1b[31m✗\x1b[0m ' + name);
		console.log('      ' + (e.message || e).toString().split('\n').join('\n      '));
	}
}

function group(name) { console.log('\n' + name); }

// Compare arrays of [a, b] within a tolerance — cut times are floats.
function assertRanges(actual, expected, tol) {
	const t = tol === undefined ? 1e-6 : tol;
	assert.strictEqual(actual.length, expected.length,
		`expected ${expected.length} range(s), got ${actual.length}: ${JSON.stringify(actual)}`);
	actual.forEach((r, i) => {
		assert.ok(Math.abs(r[0] - expected[i][0]) < t && Math.abs(r[1] - expected[i][1]) < t,
			`range ${i}: expected [${expected[i]}], got [${r}]`);
	});
}

// ---------------------------------------------------------------- ffmpeg parsing

group('parseSilenceDetect');

test('reads paired start/end lines', () => {
	const out = core.parseSilenceDetect([
		'[silencedetect @ 0x7f] silence_start: 1.5',
		'[silencedetect @ 0x7f] silence_end: 3.25 | silence_duration: 1.75',
		'[silencedetect @ 0x7f] silence_start: 10.0',
		'[silencedetect @ 0x7f] silence_end: 12.5 | silence_duration: 2.5'
	].join('\n'));
	assert.deepStrictEqual(out, [
		{ start: 1.5, end: 3.25 },
		{ start: 10.0, end: 12.5 }
	]);
});

test('a silence running to EOF has a null end', () => {
	const out = core.parseSilenceDetect('silence_start: 42.0\n[out#0] video:0kB');
	assert.deepStrictEqual(out, [{ start: 42.0, end: null }]);
});

test('two starts without an end do not swallow the first silence', () => {
	const out = core.parseSilenceDetect('silence_start: 1\nsilence_start: 5\nsilence_end: 6');
	assert.deepStrictEqual(out, [{ start: 1, end: null }, { start: 5, end: 6 }]);
});

test('ignores unrelated ffmpeg chatter', () => {
	assert.deepStrictEqual(core.parseSilenceDetect('Stream #0:0 Audio: pcm_s16le\nframe= 120'), []);
});

group('parseVolumeDetect');

test('extracts mean and peak', () => {
	const v = core.parseVolumeDetect(
		'[Parsed_volumedetect_0 @ 0x1] mean_volume: -27.4 dB\n' +
		'[Parsed_volumedetect_0 @ 0x1] max_volume: -3.1 dB'
	);
	assert.deepStrictEqual(v, { meanDb: -27.4, maxDb: -3.1 });
});

test('missing levels come back null, not NaN', () => {
	assert.deepStrictEqual(core.parseVolumeDetect('no audio here'), { meanDb: null, maxDb: null });
});

group('suggestNoiseFloor');

test('sits below the mean level', () => {
	assert.strictEqual(core.suggestNoiseFloor({ meanDb: -30, maxDb: -6 }), -36);
});

test('never lands within 12 dB of the peak (would cut everything)', () => {
	// mean -5 would suggest -11, but the peak guard pulls it to -13, then the
	// ceiling clamp lands it at -20.
	assert.strictEqual(core.suggestNoiseFloor({ meanDb: -5, maxDb: -1 }), -20);
});

test('clamps pathologically quiet measurements to the floor', () => {
	assert.strictEqual(core.suggestNoiseFloor({ meanDb: -70, maxDb: -50 }), -60);
});

test('returns null when there is nothing to measure', () => {
	assert.strictEqual(core.suggestNoiseFloor({ meanDb: null, maxDb: null }), null);
	assert.strictEqual(core.suggestNoiseFloor(null), null);
});

// ------------------------------------------------------------------ cut mapping

group('cutsForClip');

const clip = { name: 'A', mediaPath: '/m.mov', start: 10, end: 20, inPoint: 5, speed: 1 };

test('maps source silence onto sequence time', () => {
	// clip shows source 5..15 at timeline 10..20; silence at source 6..8
	// with 0.5s padding -> source 6.5..7.5 -> timeline 11.5..12.5
	assertRanges(
		core.cutsForClip(clip, [{ start: 6, end: 8 }], { padding: 0.5 }),
		[[11.5, 12.5]]
	);
});

test('padding shrinks the cut at both ends', () => {
	assertRanges(core.cutsForClip(clip, [{ start: 6, end: 8 }], { padding: 0 }), [[11, 13]]);
});

test('a silence to EOF is clamped to the clip out point', () => {
	assertRanges(
		core.cutsForClip(clip, [{ start: 12, end: null }], { padding: 0.5 }),
		[[17.5, 19.5]]
	);
});

test('silence outside the used source window is ignored', () => {
	// source 0..2 is before the clip's in point of 5
	assertRanges(core.cutsForClip(clip, [{ start: 0, end: 2 }], { padding: 0 }), []);
});

test('ranges shorter than minCut are dropped', () => {
	assertRanges(core.cutsForClip(clip, [{ start: 6, end: 6.6 }], { padding: 0.3, minCut: 0.1 }), []);
});

group('mergeRanges');

test('unions overlapping ranges', () => {
	assertRanges(core.mergeRanges([[0, 1], [0.9, 2], [5, 6]]), [[0, 2], [5, 6]]);
});

test('sorts before merging', () => {
	assertRanges(core.mergeRanges([[5, 6], [0, 1]]), [[0, 1], [5, 6]]);
});

test('mergeWithin joins cuts separated by a short gap', () => {
	assertRanges(core.mergeRanges([[0, 1], [1.2, 2]], 0.5), [[0, 2]]);
});

test('mergeWithin leaves genuinely separate cuts alone', () => {
	assertRanges(core.mergeRanges([[0, 1], [5, 6]], 0.5), [[0, 1], [5, 6]]);
});

group('absorbShortKeeps');

test('swallows a sliver of footage between two cuts', () => {
	assertRanges(core.absorbShortKeeps([[0, 1], [1.2, 2]], 0.5), [[0, 2]]);
});

test('keeps fragments that are long enough', () => {
	assertRanges(core.absorbShortKeeps([[0, 1], [1.2, 2]], 0.1), [[0, 1], [1.2, 2]]);
});

test('is a no-op below two ranges', () => {
	assertRanges(core.absorbShortKeeps([[0, 1]], 5), [[0, 1]]);
});

// --------------------------------------------------------------- frame snapping

group('snapToFrame / snapRange');

test('rounds to the nearest frame', () => {
	assert.ok(Math.abs(core.snapToFrame(1.02, 25) - 1.04) < 1e-9);
});

test('floor and ceil pick their side', () => {
	assert.ok(Math.abs(core.snapToFrame(1.02, 25, 'floor') - 1.0) < 1e-9);
	assert.ok(Math.abs(core.snapToFrame(1.001, 25, 'ceil') - 1.04) < 1e-9);
});

test('snapRange rounds inward so a cut can never eat a frame of speech', () => {
	// start moves later (ceil), end moves earlier (floor)
	assertRanges([core.snapRange([1.01, 2.03], 25)], [[1.04, 2.0]], 1e-9);
});

test('an unknown frame rate leaves times untouched', () => {
	assertRanges([core.snapRange([1.01, 2.03], 0)], [[1.01, 2.03]], 1e-9);
});

// ------------------------------------------------------------------- full plans

group('planCuts');

const twoClips = [
	{ name: 'A', mediaPath: '/a.mov', start: 0, end: 10, inPoint: 0, speed: 1, trackType: 'video', trackIndex: 0 },
	{ name: 'B', mediaPath: '/b.mov', start: 10, end: 20, inPoint: 0, speed: 1, trackType: 'video', trackIndex: 0 }
];
const silence = { '/a.mov': [{ start: 2, end: 4 }], '/b.mov': [{ start: 5, end: 7 }] };

test('returns ranges sorted descending for right-to-left application', () => {
	const plan = core.planCuts(twoClips, silence, { padding: 0 });
	assertRanges(plan.ranges, [[15, 17], [2, 4]]);
});

test('counts the clips it actually analyzed', () => {
	assert.strictEqual(core.planCuts(twoClips, silence, { padding: 0 }).analyzedClips, 2);
});

test('skips speed-ramped clips with a warning instead of mis-timing them', () => {
	const ramped = [Object.assign({}, twoClips[0], { speed: 2 })];
	const plan = core.planCuts(ramped, silence, { padding: 0 });
	assertRanges(plan.ranges, []);
	assert.match(plan.warnings[0], /2x speed/);
});

test('warns when a clip has no analysis rather than silently keeping it', () => {
	const plan = core.planCuts([twoClips[0]], {}, { padding: 0 });
	assert.match(plan.warnings[0], /No audio analysis/);
});

test('never plans a cut past the end of the sequence', () => {
	const plan = core.planCuts(twoClips, silence, { padding: 0, sequenceEnd: 16 });
	assertRanges(plan.ranges, [[15, 16], [2, 4]]);
});

test('drops ranges that frame snapping shrank below minCut', () => {
	const tiny = [{ name: 'T', mediaPath: '/t.mov', start: 0, end: 10, inPoint: 0, speed: 1 }];
	// 0.1s of silence at 25fps snaps inward to well under the 0.1s floor
	const plan = core.planCuts(tiny, { '/t.mov': [{ start: 1.01, end: 1.11 }] },
		{ padding: 0, fps: 25, minCut: 0.1 });
	assertRanges(plan.ranges, []);
});

test('frame-snapped ranges land exactly on the grid', () => {
	const one = [{ name: 'T', mediaPath: '/t.mov', start: 0, end: 10, inPoint: 0, speed: 1 }];
	const plan = core.planCuts(one, { '/t.mov': [{ start: 1.01, end: 3.03 }] },
		{ padding: 0, fps: 25 });
	plan.ranges.forEach((r) => {
		assert.ok(Math.abs(r[0] * 25 - Math.round(r[0] * 25)) < 1e-6, `start ${r[0]} off-grid`);
		assert.ok(Math.abs(r[1] * 25 - Math.round(r[1] * 25)) < 1e-6, `end ${r[1]} off-grid`);
	});
});

// -------------------------------------------------------------- track targeting

group('selectAnalysisClips');

const mixed = [
	{ name: 'v1', trackType: 'video', trackIndex: 0 },
	{ name: 'v2', trackType: 'video', trackIndex: 1 },
	{ name: 'a1', trackType: 'audio', trackIndex: 0 },
	{ name: 'a2', trackType: 'audio', trackIndex: 1 }
];

test('defaults to the first video and audio track', () => {
	const got = core.selectAnalysisClips(mixed, {}).map((c) => c.name);
	assert.deepStrictEqual(got, ['v1', 'a1']);
});

test('honours an explicit track choice', () => {
	const got = core.selectAnalysisClips(mixed, { videoTrack: 1, audioTrack: 1 }).map((c) => c.name);
	assert.deepStrictEqual(got, ['v2', 'a2']);
});

test('null excludes a media type entirely', () => {
	const got = core.selectAnalysisClips(mixed, { videoTrack: null, audioTrack: 0 }).map((c) => c.name);
	assert.deepStrictEqual(got, ['a1']);
});

// -------------------------------------------------------------------- summaries

group('summarizePlan');

test('totals cuts, seconds and percentage', () => {
	const s = core.summarizePlan([[0, 2], [10, 13]], 100);
	assert.strictEqual(s.cuts, 2);
	assert.strictEqual(s.removedSeconds, 5);
	assert.strictEqual(s.remainingSeconds, 95);
	assert.strictEqual(s.percentRemoved, 5);
});

test('an empty plan removes nothing', () => {
	assert.strictEqual(core.summarizePlan([], 100).removedSeconds, 0);
});

group('formatDuration');

test('formats minutes and seconds', () => {
	assert.strictEqual(core.formatDuration(78.4), '1:18.4');
	assert.strictEqual(core.formatDuration(5), '0:05.0');
});

test('formats past an hour', () => {
	assert.strictEqual(core.formatDuration(3725), '1:02:05');
});

// ------------------------------------------------------------------- validation

group('normalizeParams');

test('clamps values ffmpeg would reject', () => {
	const p = core.normalizeParams({ noiseDb: 999, minSilence: 0, padding: -5 });
	assert.strictEqual(p.noiseDb, -5);
	assert.strictEqual(p.minSilence, 0.05);
	assert.strictEqual(p.padding, 0);
});

test('non-numeric input falls back to defaults', () => {
	const p = core.normalizeParams({ noiseDb: 'loud', minSilence: undefined });
	assert.strictEqual(p.noiseDb, -35);
	assert.strictEqual(p.minSilence, 0.5);
});

test('every preset survives normalization unchanged', () => {
	core.presetIds().forEach((id) => {
		const preset = core.PRESETS[id];
		const p = core.normalizeParams(preset);
		assert.strictEqual(p.noiseDb, preset.noiseDb, id + ' noiseDb');
		assert.strictEqual(p.minSilence, preset.minSilence, id + ' minSilence');
		assert.strictEqual(p.padding, preset.padding, id + ' padding');
	});
});

// ------------------------------------------------- ExtendScript host: timecode

/* The QE razor takes a timecode string, so `_toTimecode` decides where cuts
 * actually land. It lives in host.jsx, which targets ExtendScript's `$`
 * global — shim that and eval the real file so these test the shipping code. */
group('host.jsx _toTimecode');

const hostSrc = require('node:fs')
	.readFileSync(path.join(__dirname, '..', 'extension', 'jsx', 'host.jsx'), 'utf8');
global.$ = {};
// eslint-disable-next-line no-eval
eval(hostSrc);
const host = global.$._autocut;

test('non-drop rates use colons and exact frames', () => {
	assert.strictEqual(host._toTimecode(0, 25), '00:00:00:00');
	assert.strictEqual(host._toTimecode(1.04, 25), '00:00:01:01');
	assert.strictEqual(host._toTimecode(60, 25), '00:01:00:00');
	assert.strictEqual(host._toTimecode(5.5, 60), '00:00:05:30');
	assert.strictEqual(host._toTimecode(86399, 24), '23:59:59:00');
});

test('rounds to the nearest frame at the half-frame boundary', () => {
	assert.strictEqual(host._toTimecode(3661.5, 25), '01:01:01:13');
	assert.strictEqual(host._toTimecode(3661.48, 25), '01:01:01:12');
});

test('drop-frame rates use a semicolon', () => {
	assert.strictEqual(host._toTimecode(1, 29.97).indexOf(';') > 0, true);
	assert.strictEqual(host._toTimecode(1, 59.94).indexOf(';') > 0, true);
	assert.strictEqual(host._toTimecode(1, 25).indexOf(';'), -1);
});

test('drop-frame renumbers, so an hour of real time reads as one hour', () => {
	// Without frame dropping this drifts ~3.6s per hour — far enough to razor
	// in the wrong place on any NTSC project.
	assert.strictEqual(host._toTimecode(3600, 29.97), '01:00:00;00');
	assert.strictEqual(host._toTimecode(3600, 59.94), '01:00:00;00');
});

test('drop-frame is exact at every 10-minute resync point', () => {
	[600, 1200, 1800, 2400, 3000, 3600].forEach((seconds) => {
		const parts = host._toTimecode(seconds, 29.97).split(/[:;]/).map(Number);
		const labelled = parts[0] * 3600 + parts[1] * 60 + parts[2] + parts[3] / 30;
		assert.ok(Math.abs(labelled - seconds) < 1e-9,
			`${seconds}s should be exact, read as ${labelled}`);
	});
});

test('drop-frame drift never exceeds two frames', () => {
	let worst = 0;
	for (let m = 0; m <= 60; m++) {
		const parts = host._toTimecode(m * 60, 29.97).split(/[:;]/).map(Number);
		const labelled = parts[0] * 3600 + parts[1] * 60 + parts[2] + parts[3] / 30;
		worst = Math.max(worst, Math.abs(labelled - m * 60));
	}
	assert.ok(worst < 2.5 / 30, `worst drift ${worst.toFixed(4)}s exceeds two frames`);
});

test('timecode is monotonic frame by frame', () => {
	let prev = null;
	for (let f = 0; f < 30; f++) {
		const tc = host._toTimecode(10 + f / 29.97, 29.97);
		if (prev !== null) { assert.ok(tc > prev, `${tc} should sort after ${prev}`); }
		prev = tc;
	}
});

test('identifies exactly the NTSC drop-frame rates', () => {
	assert.strictEqual(host._isDropFrame(29.97), true);
	assert.strictEqual(host._isDropFrame(59.94), true);
	assert.strictEqual(host._isDropFrame(25), false);
	assert.strictEqual(host._isDropFrame(30), false);
	assert.strictEqual(host._isDropFrame(24), false);
});

/* The engine compares clip edges to decide what to delete and what to shift.
 * Getting that tolerance wrong is invisible in a unit test but shows up in
 * Premiere as audio left playing over a hole in the video, so pin it down. */
group('host.jsx _eps');

test('is at least half a frame at every normal rate', () => {
	for (const fps of [23.976, 24, 25, 29.97, 30, 50, 59.94, 60]) {
		assert.ok(host._eps(fps) > 0.5 / fps,
			`${fps}fps: eps ${host._eps(fps)} must exceed half a frame ${0.5 / fps}`);
	}
});

test('never grows past a whole frame, which would eat kept footage', () => {
	for (const fps of [24, 25, 30, 50, 60]) {
		assert.ok(host._eps(fps) < 1 / fps, `${fps}fps: eps must stay under one frame`);
	}
});

test('falls back to the fixed epsilon when fps is unknown', () => {
	assert.strictEqual(host._eps(0), host.EPS);
	assert.strictEqual(host._eps(null), host.EPS);
	assert.strictEqual(host._eps(NaN), host.EPS);
});

group('host.jsx _desyncedTracks');

const ends = (list) => list.map(([name, end, locked]) =>
	({ name, end, locked: !!locked }));

test('a clean ripple reports nothing', () => {
	const before = ends([['V1', 100], ['A1', 100]]);
	const after = ends([['V1', 98], ['A1', 98]]);
	assert.deepStrictEqual(host._desyncedTracks(before, after, 10, 12, 25), []);
});

test('names the track that kept its content while the others moved', () => {
	const before = ends([['V1', 100], ['A1', 100], ['A2', 100]]);
	const after = ends([['V1', 98], ['A1', 100], ['A2', 98]]);
	assert.deepStrictEqual(host._desyncedTracks(before, after, 10, 12, 25), ['A1']);
});

test('ignores tracks with nothing past the cut', () => {
	// A2's music ends before the cut, so it cannot ripple and is not a fault.
	const before = ends([['V1', 100], ['A2', 5]]);
	const after = ends([['V1', 98], ['A2', 5]]);
	assert.deepStrictEqual(host._desyncedTracks(before, after, 10, 12, 25), []);
});

test('ignores locked tracks, which the panel warns about separately', () => {
	const before = ends([['V1', 100], ['A1', 100, true]]);
	const after = ends([['V1', 98], ['A1', 100, true]]);
	assert.deepStrictEqual(host._desyncedTracks(before, after, 10, 12, 25), []);
});

test('a sub-frame rounding difference is not a desync', () => {
	const before = ends([['V1', 100], ['A1', 100]]);
	const after = ends([['V1', 98], ['A1', 98 + 0.5 / 25]]);
	assert.deepStrictEqual(host._desyncedTracks(before, after, 10, 12, 25), []);
});

// ------------------------------------------------------------------------ report

console.log('\n' + (failed === 0
	? `\x1b[32m${passed} passed\x1b[0m`
	: `\x1b[32m${passed} passed\x1b[0m, \x1b[31m${failed} failed\x1b[0m`) + '\n');

process.exit(failed === 0 ? 0 : 1);
