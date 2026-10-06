/* Auto Cut — panel logic.
 *
 * Self-contained: this panel runs ffmpeg itself through CEP's Node
 * integration (`--enable-nodejs` in the manifest), so there is no companion
 * server, nothing to start in a terminal, and no API keys anywhere.
 *
 * Pipeline
 *   host.getSequenceMap()  -> clips + fps + track layout
 *   ffmpeg silencedetect   -> silences in source time, per media file
 *   AutoCutCore.planCuts() -> merged, frame-snapped sequence-time cut ranges
 *   [preview + approve]
 *   host.applyCuts()       -> ranges removed right-to-left, all tracks in sync
 */
(function () {
	'use strict';

	var core = window.AutoCutCore;
	var cs = new CSInterface();

	var cp = require('child_process');
	var fs = require('fs');
	var nodePath = require('path');

	// ---- state ---------------------------------------------------------------

	var state = {
		seqMap: null,      // last sequence read
		plan: null,        // { ranges, warnings, summary }
		busy: false,
		proc: null,        // running ffmpeg, for Cancel
		cancelled: false,
		ffmpeg: null       // resolved binary path
	};

	// ---- DOM -----------------------------------------------------------------

	var el = {};
	['seqName', 'seqMeta', 'ffmpegStatus', 'preset', 'presetHint', 'noise', 'minSilence',
		'padding', 'mergeWithin', 'minKeep', 'videoTrack', 'audioTrack', 'measureBtn',
		'analyzeBtn', 'applyBtn', 'cancelBtn', 'refreshBtn', 'diagBtn', 'settingsBtn',
		'settingsPanel', 'ffmpegPath', 'planSummary', 'planDetail', 'progressWrap',
		'progressBar', 'progressText', 'log'
	].forEach(function (id) { el[id] = document.getElementById(id); });

	// ---- logging -------------------------------------------------------------

	function log(msg, cls) {
		var line = document.createElement('div');
		line.className = 'line ' + (cls || 'info');
		line.textContent = new Date().toLocaleTimeString() + '  ' + msg;
		el.log.appendChild(line);
		el.log.scrollTop = el.log.scrollHeight;
	}

	function fail(msg) { log(msg, 'err'); }
	function good(msg) { log(msg, 'ok'); }
	function warn(msg) { log(msg, 'warn'); }

	function setProgress(pct, text) {
		el.progressWrap.classList.remove('hidden');
		el.progressBar.style.width = core.clamp(pct, 0, 100) + '%';
		el.progressText.textContent = text || '';
	}

	function hideProgress() {
		el.progressWrap.classList.add('hidden');
		el.progressBar.style.width = '0%';
		el.progressText.textContent = '';
	}

	function setBusy(busy) {
		state.busy = busy;
		el.analyzeBtn.disabled = busy;
		el.measureBtn.disabled = busy;
		el.refreshBtn.disabled = busy;
		el.applyBtn.disabled = busy || !state.plan || state.plan.ranges.length === 0;
		el.cancelBtn.classList.toggle('hidden', !busy);
		if (!busy) { hideProgress(); }
	}

	// ---- ExtendScript bridge --------------------------------------------------

	function evalScript(script) {
		return new Promise(function (resolve) {
			cs.evalScript(script, function (result) { resolve(result); });
		});
	}

	// Every host entry point returns a JSON string with an `ok` flag.
	function callHost(fn, arg) {
		var script = arg === undefined
			? '$._autocut.' + fn + '()'
			: '$._autocut.' + fn + '(' + JSON.stringify(JSON.stringify(arg)) + ')';
		return evalScript(script).then(function (raw) {
			if (raw === 'EvalScript error.') {
				throw new Error(fn + ': ExtendScript threw. Is the panel installed correctly?');
			}
			var parsed;
			try { parsed = JSON.parse(raw); }
			catch (e) { throw new Error(fn + ': unreadable host response — ' + String(raw).slice(0, 200)); }
			if (!parsed.ok) { throw new Error(parsed.message || (fn + ' failed')); }
			return parsed;
		});
	}

	// ---- ffmpeg ---------------------------------------------------------------

	// CEP's Node runs with a minimal PATH, so `ffmpeg` alone usually will not
	// resolve. Check the usual install locations, then a user-set override.
	var FFMPEG_CANDIDATES = [
		'/opt/homebrew/bin/ffmpeg',   // Apple Silicon Homebrew
		'/usr/local/bin/ffmpeg',      // Intel Homebrew / manual installs
		'/opt/local/bin/ffmpeg',      // MacPorts
		'/usr/bin/ffmpeg',
		'C:\\ffmpeg\\bin\\ffmpeg.exe',
		'C:\\Program Files\\ffmpeg\\bin\\ffmpeg.exe'
	];

	function resolveFfmpeg() {
		var override = (el.ffmpegPath.value || '').trim();
		var candidates = override ? [override].concat(FFMPEG_CANDIDATES) : FFMPEG_CANDIDATES;
		for (var i = 0; i < candidates.length; i++) {
			try {
				if (fs.existsSync(candidates[i])) { return candidates[i]; }
			} catch (e) { /* keep looking */ }
		}
		// Not in a known location, but it may still be on PATH (a custom
		// install, or a PATH the panel inherited). Ask it directly.
		try {
			var probe = cp.spawnSync('ffmpeg', ['-version'], { encoding: 'utf8', windowsHide: true });
			if (probe && probe.status === 0) { return 'ffmpeg'; }
		} catch (e2) { /* genuinely absent */ }
		return null;
	}

	function checkFfmpeg() {
		var found = resolveFfmpeg();
		state.ffmpeg = found;
		if (!found) {
			el.ffmpegStatus.textContent = 'ffmpeg not found';
			el.ffmpegStatus.className = 'pill bad';
			warn('ffmpeg was not found in the usual locations. Install it ' +
				'(brew install ffmpeg) or set the full path in Settings.');
			return false;
		}
		el.ffmpegStatus.textContent = 'ffmpeg ready';
		el.ffmpegStatus.className = 'pill good';
		return true;
	}

	// Run ffmpeg, streaming stderr. onTick(seconds) fires on progress lines so
	// long analyses show real movement instead of a frozen bar.
	function runFfmpeg(args, onTick) {
		return new Promise(function (resolve, reject) {
			var bin = state.ffmpeg || 'ffmpeg';
			var proc;
			try {
				proc = cp.spawn(bin, args, { windowsHide: true });
			} catch (e) {
				return reject(new Error('could not start ffmpeg (' + bin + '): ' + e.message));
			}
			state.proc = proc;
			var stderr = '';

			proc.stderr.on('data', function (chunk) {
				var text = String(chunk);
				stderr += text;
				if (onTick) {
					var m = text.match(/time=(\d+):(\d+):([\d.]+)/);
					if (m) {
						onTick(Number(m[1]) * 3600 + Number(m[2]) * 60 + Number(m[3]));
					}
				}
			});
			proc.on('error', function (e) {
				state.proc = null;
				reject(new Error('ffmpeg failed to launch (' + bin + '): ' + e.message));
			});
			proc.on('close', function (code) {
				state.proc = null;
				if (state.cancelled) { return reject(new Error('cancelled')); }
				if (code !== 0) {
					return reject(new Error('ffmpeg exited ' + code + ': ' +
						stderr.slice(-300).replace(/\s+/g, ' ')));
				}
				resolve(stderr);
			});
		});
	}

	function probeDuration(stderrText) {
		var m = String(stderrText).match(/Duration:\s*(\d+):(\d+):([\d.]+)/);
		if (!m) { return 0; }
		return Number(m[1]) * 3600 + Number(m[2]) * 60 + Number(m[3]);
	}

	// ---- sequence + params ----------------------------------------------------

	function readParams() {
		return core.normalizeParams({
			noiseDb: el.noise.value,
			minSilence: el.minSilence.value,
			padding: el.padding.value,
			mergeWithin: el.mergeWithin.value,
			minKeep: el.minKeep.value
		});
	}

	function trackTarget() {
		var v = el.videoTrack.value;
		var a = el.audioTrack.value;
		return {
			videoTrack: v === 'none' ? null : Number(v),
			audioTrack: a === 'none' ? null : Number(a)
		};
	}

	// Media files that drive analysis, given the current track targeting.
	function analysisPaths(clips) {
		var selected = core.selectAnalysisClips(clips, trackTarget());
		var seen = {};
		var paths = [];
		for (var i = 0; i < selected.length; i++) {
			var p = selected[i].mediaPath;
			if (p && !seen[p]) { seen[p] = true; paths.push(p); }
		}
		return paths;
	}

	function fillTrackPickers(tracks) {
		function fill(select, list) {
			var previous = select.value;
			select.innerHTML = '';
			var none = document.createElement('option');
			none.value = 'none';
			none.textContent = 'none';
			select.appendChild(none);
			for (var i = 0; i < list.length; i++) {
				var o = document.createElement('option');
				o.value = String(list[i].index);
				o.textContent = list[i].name +
					(list[i].locked ? ' (locked)' : '') +
					' — ' + list[i].clips + ' clip' + (list[i].clips === 1 ? '' : 's');
				select.appendChild(o);
			}
			// Keep the user's choice across refreshes when it still exists.
			var keep = false;
			for (var j = 0; j < select.options.length; j++) {
				if (select.options[j].value === previous) { keep = true; }
			}
			select.value = keep ? previous : (list.length > 0 ? String(list[0].index) : 'none');
		}
		fill(el.videoTrack, tracks.video);
		fill(el.audioTrack, tracks.audio);
	}

	function refreshSequence() {
		return callHost('getSequenceMap').then(function (map) {
			state.seqMap = map;
			el.seqName.textContent = map.seqName;
			el.seqMeta.textContent = map.clips.length + ' clips · ' +
				(map.fps ? map.fps.toFixed(2) + ' fps' : 'unknown fps') + ' · ' +
				core.formatDuration(map.timelineEnd);
			fillTrackPickers(map.tracks);
			clearPlan();
			var locked = [];
			var groups = [map.tracks.video, map.tracks.audio];
			for (var g = 0; g < groups.length; g++) {
				for (var t = 0; t < groups[g].length; t++) {
					if (groups[g][t].locked && groups[g][t].clips > 0) {
						locked.push(groups[g][t].name);
					}
				}
			}
			if (locked.length > 0) {
				warn('Locked tracks with content: ' + locked.join(', ') +
					'. Locked tracks are not cut, so they will drift out of sync — unlock them.');
			}
			if (map.unreadable > 0) {
				log(map.unreadable + ' clip(s) have no media file on disk ' +
					'(titles, mattes, offline). They are cut in sync but never analyzed.');
			}
			return map;
		}).catch(function (e) {
			el.seqName.textContent = 'no sequence';
			el.seqMeta.textContent = '';
			fail(e.message);
			throw e;
		});
	}

	// ---- measure: suggest a threshold from the real audio -----------------------

	function onMeasure() {
		if (state.busy) { return; }
		if (!checkFfmpeg()) { return; }
		state.cancelled = false;
		setBusy(true);
		log('Measuring audio levels…');

		refreshSequence().then(function (map) {
			var paths = analysisPaths(map.clips);
			if (paths.length === 0) {
				throw new Error('No media on the selected tracks — pick the track holding your raw clip.');
			}
			// One representative file is enough to set a threshold.
			var target = paths[0];
			if (paths.length > 1) {
				log('Measuring the first of ' + paths.length + ' files: ' + baseName(target));
			}
			setProgress(15, 'measuring ' + baseName(target));
			return runFfmpeg(['-hide_banner', '-i', target, '-af', 'volumedetect', '-f', 'null', '-']);
		}).then(function (stderr) {
			var volume = core.parseVolumeDetect(stderr);
			if (volume.meanDb === null) {
				throw new Error('ffmpeg reported no audio levels — does that clip have an audio stream?');
			}
			var suggested = core.suggestNoiseFloor(volume);
			log('mean ' + volume.meanDb.toFixed(1) + ' dB, peak ' +
				(volume.maxDb === null ? '?' : volume.maxDb.toFixed(1)) + ' dB');
			if (suggested === null) {
				warn('Could not derive a threshold from those levels — keeping the current value.');
			} else {
				el.noise.value = String(suggested);
				good('Noise threshold set to ' + suggested + ' dB. Analyze to see the plan.');
			}
		}).catch(function (e) {
			if (e.message !== 'cancelled') { fail(e.message); } else { log('Cancelled.'); }
		}).then(function () { setBusy(false); });
	}

	// ---- analyze: build the cut plan --------------------------------------------

	function onAnalyze() {
		if (state.busy) { return; }
		if (!checkFfmpeg()) { return; }
		state.cancelled = false;
		setBusy(true);
		clearPlan();
		var params = readParams();
		log('Analyzing at ' + params.noiseDb + ' dB, min silence ' + params.minSilence + 's…');

		var map, paths;
		refreshSequence().then(function (m) {
			map = m;
			paths = analysisPaths(map.clips);
			if (paths.length === 0) {
				throw new Error('No media on the selected tracks — pick the track holding your raw clip.');
			}
			var others = map.clips.length - core.selectAnalysisClips(map.clips, trackTarget()).length;
			if (others > 0) {
				log(others + ' clip(s) on other tracks will be cut in sync without being analyzed.');
			}
			return analyzeSequentially(paths, params);
		}).then(function (result) {
			var silenceByPath = result.silenceByPath;
			var warnings = result.warnings.slice();
			if (Object.keys(silenceByPath).length === 0) {
				throw new Error('Audio analysis failed for every selected clip.');
			}
			var plan = core.planCuts(map.clips, silenceByPath, {
				padding: params.padding,
				minCut: params.minCut,
				mergeWithin: params.mergeWithin,
				minKeep: params.minKeep,
				fps: map.fps,
				sequenceEnd: map.timelineEnd
			});
			var summary = core.summarizePlan(plan.ranges, map.timelineEnd);
			state.plan = { ranges: plan.ranges, warnings: warnings.concat(plan.warnings), summary: summary };
			renderPlan();
		}).catch(function (e) {
			if (e.message !== 'cancelled') { fail(e.message); } else { log('Cancelled.'); }
		}).then(function () { setBusy(false); });
	}

	// One file at a time: parallel ffmpeg runs just fight over the same disk
	// and make the progress bar meaningless.
	function analyzeSequentially(paths, params) {
		var silenceByPath = {};
		var warnings = [];
		var chain = Promise.resolve();

		paths.forEach(function (path, idx) {
			chain = chain.then(function () {
				if (state.cancelled) { throw new Error('cancelled'); }
				var base = (idx / paths.length) * 100;
				var slice = 100 / paths.length;
				var duration = 0;
				setProgress(base, 'analyzing ' + baseName(path));

				var filter = 'silencedetect=noise=' + params.noiseDb + 'dB:d=' + params.minSilence;
				return runFfmpeg(
					['-hide_banner', '-i', path, '-af', filter, '-f', 'null', '-'],
					function (seconds) {
						if (duration > 0) {
							setProgress(base + (seconds / duration) * slice,
								'analyzing ' + baseName(path) + ' — ' +
								Math.round((seconds / duration) * 100) + '%');
						}
					}
				).then(function (stderr) {
					duration = probeDuration(stderr);
					var silences = core.parseSilenceDetect(stderr);
					silenceByPath[path] = silences;
					log(baseName(path) + ': ' + silences.length + ' silence(s)');
				}).catch(function (e) {
					if (e.message === 'cancelled') { throw e; }
					warnings.push(baseName(path) + ': ' + e.message);
				});
			});
		});

		return chain.then(function () {
			return { silenceByPath: silenceByPath, warnings: warnings };
		});
	}

	// ---- plan preview ------------------------------------------------------------

	function clearPlan() {
		state.plan = null;
		el.planSummary.textContent = 'No plan yet — click Analyze.';
		el.planSummary.className = 'plan-summary';
		el.planDetail.innerHTML = '';
		el.applyBtn.disabled = true;
	}

	function renderPlan() {
		var plan = state.plan;
		var s = plan.summary;
		for (var i = 0; i < plan.warnings.length; i++) { warn(plan.warnings[i]); }

		if (plan.ranges.length === 0) {
			el.planSummary.textContent = 'Nothing to cut at these settings. ' +
				'Try a higher (less negative) threshold or a shorter min silence.';
			el.planSummary.className = 'plan-summary empty';
			el.applyBtn.disabled = true;
			return;
		}

		el.planSummary.className = 'plan-summary ready';
		el.planSummary.textContent = s.cuts + ' cuts · ' + core.formatDuration(s.removedSeconds) +
			' removed (' + s.percentRemoved.toFixed(0) + '%) · ' +
			core.formatDuration(s.remainingSeconds) + ' left';

		// Longest cuts first: those are the ones worth eyeballing before you
		// commit, and the ones most likely to reveal a bad threshold.
		var sorted = plan.ranges.slice().sort(function (a, b) {
			return (b[1] - b[0]) - (a[1] - a[0]);
		});
		var shown = sorted.slice(0, 8);
		var html = '<div class="detail-head">Longest cuts</div>';
		for (var j = 0; j < shown.length; j++) {
			html += '<div class="detail-row"><span>' + core.formatDuration(shown[j][0]) +
				' → ' + core.formatDuration(shown[j][1]) + '</span><span class="dur">' +
				(shown[j][1] - shown[j][0]).toFixed(2) + 's</span></div>';
		}
		if (sorted.length > shown.length) {
			html += '<div class="detail-more">+ ' + (sorted.length - shown.length) + ' more</div>';
		}
		el.planDetail.innerHTML = html;
		el.applyBtn.disabled = false;
		good('Plan ready. Review it, then Apply cuts.');
	}

	// ---- apply --------------------------------------------------------------------

	function onApply() {
		if (state.busy || !state.plan || state.plan.ranges.length === 0) { return; }
		var s = state.plan.summary;
		var confirmed = window.confirm(
			'Apply ' + s.cuts + ' cuts and remove ' + core.formatDuration(s.removedSeconds) +
			' from "' + (state.seqMap ? state.seqMap.seqName : 'this sequence') + '"?\n\n' +
			'Undo works, but duplicating the sequence first is safer.'
		);
		if (!confirmed) { return; }

		setBusy(true);
		setProgress(30, 'cutting ' + s.cuts + ' ranges…');
		log('Applying ' + s.cuts + ' cuts…');

		callHost('applyCuts', { ranges: state.plan.ranges, strategy: 'auto' })
			.then(function (res) {
				log('Strategy: ' + res.strategy);
				for (var i = 0; i < res.errors.length; i++) { fail(res.errors[i]); }
				if (res.lockedTracks && res.lockedTracks.length > 0) {
					warn('Locked and therefore NOT cut: ' + res.lockedTracks.join(', ') +
						' — these tracks are now out of sync.');
				}
				if (res.stopped) {
					fail('Stopped early with ' + res.remaining + ' cut(s) unapplied — a track ' +
						'fell out of sync. Undo (Cmd/Ctrl+Z until the timeline is back), then ' +
						'check that no track is locked and that sync lock is on everywhere.');
				}
				var msg = res.applied + ' cut(s) applied' +
					(res.failed > 0 ? ', ' + res.failed + ' failed' : '') + '. Timeline ' +
					core.formatDuration(res.endBefore) + ' → ' + core.formatDuration(res.endAfter) +
					' (' + core.formatDuration(res.removedSeconds) + ' removed).';
				if (res.failed > 0) { warn(msg); } else { good(msg); }
				clearPlan();
				return refreshSequence();
			})
			.catch(function (e) { fail(e.message); })
			.then(function () { setBusy(false); });
	}

	// ---- diagnostics ----------------------------------------------------------------

	function onDiagnostics() {
		log('Running diagnostics…');
		callHost('diagnostics').then(function (r) {
			log('— Auto Cut ' + r.autocut + ' on ' + r.app + ' ' + r.version + ' —');
			log('QE DOM: ' + (r.qeAvailable ? 'available' : 'NOT AVAILABLE') +
				', sequence: ' + (r.qeSequence ? 'yes' : 'no'));
			var caps = Object.keys(r.capabilities);
			for (var i = 0; i < caps.length; i++) {
				log('  ' + (r.capabilities[caps[i]] ? '✓' : '✗') + ' ' + caps[i],
					r.capabilities[caps[i]] ? 'ok' : 'warn');
			}
			for (var j = 0; j < r.strategies.length; j++) {
				var st = r.strategies[j];
				log('  ' + (st.available ? '✓' : '✗') + ' strategy "' + st.id + '": ' + st.label,
					st.available ? 'ok' : 'warn');
			}
			for (var k = 0; k < r.notes.length; k++) { warn(r.notes[k]); }
			log('— end of report —');
		}).catch(function (e) { fail(e.message); });
	}

	// ---- presets --------------------------------------------------------------------

	function applyPreset(id) {
		var p = core.PRESETS[id];
		if (!p) { return; }
		el.noise.value = String(p.noiseDb);
		el.minSilence.value = String(p.minSilence);
		el.padding.value = String(p.padding);
		el.mergeWithin.value = String(p.mergeWithin);
		el.minKeep.value = String(p.minKeep);
		el.presetHint.textContent = p.hint;
		clearPlan();
		saveSettings();
	}

	function buildPresetPicker() {
		var ids = core.presetIds();
		for (var i = 0; i < ids.length; i++) {
			var o = document.createElement('option');
			o.value = ids[i];
			o.textContent = core.PRESETS[ids[i]].label;
			el.preset.appendChild(o);
		}
	}

	// ---- settings persistence ----------------------------------------------------

	var SAVED = ['noise', 'minSilence', 'padding', 'mergeWithin', 'minKeep', 'preset', 'ffmpegPath'];

	function saveSettings() {
		for (var i = 0; i < SAVED.length; i++) {
			localStorage.setItem('autocut.' + SAVED[i], el[SAVED[i]].value);
		}
	}

	function loadSettings() {
		for (var i = 0; i < SAVED.length; i++) {
			var v = localStorage.getItem('autocut.' + SAVED[i]);
			if (v !== null) { el[SAVED[i]].value = v; }
		}
	}

	// ---- misc -----------------------------------------------------------------------

	function baseName(p) {
		try { return nodePath.basename(p); } catch (e) { return String(p); }
	}

	function onCancel() {
		state.cancelled = true;
		if (state.proc) {
			try { state.proc.kill(); } catch (e) { /* already gone */ }
		}
		log('Cancelling…');
	}

	// ---- wiring ---------------------------------------------------------------------

	function init() {
		buildPresetPicker();
		loadSettings();
		if (!localStorage.getItem('autocut.preset')) {
			el.preset.value = 'talking-head';
			applyPreset('talking-head');
		} else {
			var p = core.PRESETS[el.preset.value];
			if (p) { el.presetHint.textContent = p.hint; }
		}

		el.preset.addEventListener('change', function () { applyPreset(el.preset.value); });
		el.measureBtn.addEventListener('click', onMeasure);
		el.analyzeBtn.addEventListener('click', onAnalyze);
		el.applyBtn.addEventListener('click', onApply);
		el.cancelBtn.addEventListener('click', onCancel);
		el.refreshBtn.addEventListener('click', function () {
			refreshSequence().then(function () { good('Sequence reloaded.'); }).catch(function () {});
		});
		el.diagBtn.addEventListener('click', onDiagnostics);
		el.settingsBtn.addEventListener('click', function () {
			el.settingsPanel.classList.toggle('hidden');
		});

		['noise', 'minSilence', 'padding', 'mergeWithin', 'minKeep'].forEach(function (id) {
			el[id].addEventListener('change', function () { clearPlan(); saveSettings(); });
		});
		el.ffmpegPath.addEventListener('change', function () { saveSettings(); checkFfmpeg(); });

		// Re-read the timeline whenever the user switches sequences in Premiere.
		try {
			cs.addEventListener('com.adobe.csxs.events.SequenceActivated', function () {
				if (!state.busy) { refreshSequence().catch(function () {}); }
			});
		} catch (e) { /* older CEP hosts */ }

		checkFfmpeg();
		clearPlan();
		refreshSequence()
			.then(function () { log('Ready.'); })
			.catch(function () { log('Open a sequence, then click Reload.'); });
	}

	// A throw in init() would leave the panel looking alive but wired to
	// nothing — no listeners, no log, buttons that do nothing. Surface it in
	// the panel instead of only in a devtools console nobody has open.
	function bootstrap() {
		try {
			if (!core) {
				throw new Error('core.js did not load (window.AutoCutCore is undefined)');
			}
			init();
		} catch (e) {
			var box = document.getElementById('log');
			if (box) {
				var line = document.createElement('div');
				line.className = 'line err';
				line.textContent = 'Auto Cut failed to start: ' + (e && e.message ? e.message : e);
				box.appendChild(line);
				var hint = document.createElement('div');
				hint.className = 'line warn';
				hint.textContent = 'Open http://localhost:8092 for the full stack trace.';
				box.appendChild(hint);
			}
			var pill = document.getElementById('ffmpegStatus');
			if (pill) { pill.textContent = 'startup failed'; pill.className = 'pill bad'; }
			console.error('[autocut] startup failed', e);
		}
	}

	if (document.readyState === 'loading') {
		document.addEventListener('DOMContentLoaded', bootstrap);
	} else {
		bootstrap();
	}
}());
