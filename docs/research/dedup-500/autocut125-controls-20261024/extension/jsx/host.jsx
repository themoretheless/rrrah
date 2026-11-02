/* Auto Cut — ExtendScript host for the CEP panel.
 *
 * ExtendScript is ES3: `var` only, no let/const, no arrow functions, no
 * Array.prototype.map/filter/forEach. Every entry point takes and returns a
 * JSON string, because evalScript can only carry strings.
 *
 * ── Why this file is built around capability probing ──────────────────────
 * Premiere's documented ExtendScript API can trim a clip (`start`/`end` are
 * writable) but cannot *split* one, and cannot move a clip along its track.
 * Both operations live in the QE DOM (`app.enableQE()` → `qe`), which Adobe
 * has never documented and which has drifted between versions.
 *
 * So instead of betting on one call signature, the engine declares several
 * removal strategies, asks each whether it can run on this build, and uses
 * the first that works — verifying the timeline actually shrank after every
 * range. `diagnostics()` reports exactly what this Premiere exposes, so a
 * failure is a readable report rather than a guess.
 *
 * ── The cut algorithm ─────────────────────────────────────────────────────
 * Ranges arrive sorted descending and are applied right-to-left, so times
 * below the current cut stay valid as later content shifts left. Each range
 * is removed from ALL unlocked tracks at once and the gap closed uniformly,
 * which is what keeps b-roll and music frame-aligned with the voice track.
 */

$._autocut = {

	VERSION: '1.0.0',
	TICKS_PER_SECOND: 254016000000,
	EPS: 0.012, // seconds; under one frame at 60fps

	// ------------------------------------------------------------- utilities

	_err: function (message, extra) {
		var o = extra || {};
		o.ok = false;
		o.message = String(message);
		return JSON.stringify(o);
	},

	_ok: function (obj) {
		var o = obj || {};
		o.ok = true;
		return JSON.stringify(o);
	},

	_seq: function () {
		if (!app.project) { return null; }
		return app.project.activeSequence || null;
	},

	// Frames per second of a sequence. `timebase` is ticks per frame, which is
	// exact for both 29.97 (drop frame) and integer rates.
	_fps: function (seq) {
		try {
			var tb = Number(seq.timebase);
			if (tb > 0) { return this.TICKS_PER_SECOND / tb; }
		} catch (e) { /* fall through */ }
		try {
			var settings = seq.getSettings();
			var frameDur = settings.videoFrameRate.seconds;
			if (frameDur > 0) { return 1 / frameDur; }
		} catch (e2) { /* fall through */ }
		return 0;
	},

	// Tolerance for "is this the same instant". A fixed 12ms is finer than a
	// frame at every normal rate (40ms at 25fps), so comparing clip edges with
	// it made a clip that sat half a frame off look like it fell outside the
	// cut — it survived while every other track rippled past it. Times are
	// already frame-snapped by the planner, so half a frame is the largest
	// error that can legitimately show up.
	_eps: function (fps) {
		if (!fps || !isFinite(fps) || fps <= 0) { return this.EPS; }
		return Math.max(this.EPS, 0.6 / fps);
	},

	// True for rates that Premiere displays as drop-frame timecode.
	_isDropFrame: function (fps) {
		return Math.abs(fps - 29.97) < 0.01 || Math.abs(fps - 59.94) < 0.01;
	},

	_pad: function (n, width) {
		var s = String(Math.floor(Math.abs(n)));
		while (s.length < width) { s = '0' + s; }
		return s;
	},

	// Seconds -> "HH:MM:SS:FF", or "HH:MM:SS;FF" at 29.97/59.94. QE's razor
	// takes a timecode string and it must match the sequence's own format.
	//
	// Drop-frame is not just a different separator: NTSC rates run slightly
	// slow, so the numbering skips frames (2 per minute at 29.97, 4 at 59.94,
	// except every tenth minute) to keep timecode aligned with wall clock.
	// Emitting a semicolon without renumbering drifts ~3.6s per hour, which is
	// far enough to razor in the wrong place.
	_toTimecode: function (seconds, fps) {
		var f = fps > 0 ? fps : 25;
		var nominal = Math.round(f);
		var frameNumber = Math.round(seconds * f);
		var sep = ':';

		if (this._isDropFrame(f)) {
			sep = ';';
			var dropFrames = Math.round(f * 0.066666);        // 2 @29.97, 4 @59.94
			var framesPerMin = nominal * 60 - dropFrames;      // 1798 / 3596
			var framesPer10Min = nominal * 600 - 9 * dropFrames; // 17982 / 35964
			var tenMinBlocks = Math.floor(frameNumber / framesPer10Min);
			var remainder = frameNumber % framesPer10Min;
			// Renumber: every minute skips `dropFrames`, except each 10th.
			frameNumber += dropFrames * 9 * tenMinBlocks;
			if (remainder > dropFrames) {
				frameNumber += dropFrames * Math.floor((remainder - dropFrames) / framesPerMin);
			}
		}

		var frames = frameNumber % nominal;
		var totalSeconds = Math.floor(frameNumber / nominal);
		var secs = totalSeconds % 60;
		var mins = Math.floor(totalSeconds / 60) % 60;
		var hours = Math.floor(totalSeconds / 3600);
		return this._pad(hours, 2) + ':' + this._pad(mins, 2) + ':' +
			this._pad(secs, 2) + sep + this._pad(frames, 2);
	},

	// ------------------------------------------------------- sequence reading

	// Walk every clip on every video and audio track.
	// fn(clip, trackType, trackIndex, track) — return false to stop.
	_eachClip: function (seq, fn) {
		var t, i, j, track, clips;
		var kinds = [['video', seq.videoTracks], ['audio', seq.audioTracks]];
		for (t = 0; t < kinds.length; t++) {
			var kind = kinds[t][0];
			var collection = kinds[t][1];
			for (i = 0; i < collection.numTracks; i++) {
				track = collection[i];
				clips = track.clips;
				for (j = 0; j < clips.numItems; j++) {
					if (fn(clips[j], kind, i, track) === false) { return; }
				}
			}
		}
	},

	_timelineEnd: function (seq) {
		var max = 0;
		this._eachClip(seq, function (clip) {
			var e = clip.end.seconds;
			if (e > max) { max = e; }
		});
		return max;
	},

	// Every track as one flat, ordered list — video first, then audio — with
	// the lock state already read. The cut engine works track by track, and
	// this order is stable enough to compare a before/after snapshot index by
	// index.
	_trackList: function (seq) {
		var out = [];
		var kinds = [['video', seq.videoTracks, 'V'], ['audio', seq.audioTracks, 'A']];
		for (var k = 0; k < kinds.length; k++) {
			var collection = kinds[k][1];
			for (var i = 0; i < collection.numTracks; i++) {
				var track = collection[i];
				var locked = false;
				try { locked = !!track.isLocked(); } catch (e) { /* older builds */ }
				out.push({
					track: track,
					kind: kinds[k][0],
					index: i,
					name: kinds[k][2] + (i + 1),
					locked: locked
				});
			}
		}
		return out;
	},

	// End time of the last clip on each track, in _trackList order.
	//
	// Taken before and after every range. The whole-timeline length cannot
	// catch the failure that matters: if one track keeps its content while the
	// others ripple left, the timeline still gets shorter and the run looks
	// like it worked — the desync only shows up later as audio playing over a
	// gap in the video.
	_trackEnds: function (seq) {
		var tracks = this._trackList(seq);
		var out = [];
		for (var i = 0; i < tracks.length; i++) {
			var end = 0;
			try {
				var clips = tracks[i].track.clips;
				for (var j = 0; j < clips.numItems; j++) {
					var e = clips[j].end.seconds;
					if (e > end) { end = e; }
				}
			} catch (e2) { /* unreadable track */ }
			out.push({ name: tracks[i].name, locked: tracks[i].locked, end: end });
		}
		return out;
	},

	_maxEnd: function (ends) {
		var max = 0;
		for (var i = 0; i < ends.length; i++) {
			if (ends[i].end > max) { max = ends[i].end; }
		}
		return max;
	},

	// Tracks that still held content past the cut but did not move left with
	// everything else. Naming them is the difference between "your edit is
	// subtly broken somewhere" and "A2 did not ripple".
	_desyncedTracks: function (before, after, a, b, fps) {
		var eps = this._eps(fps);
		var span = b - a;
		var bad = [];
		var n = Math.min(before.length, after.length);
		for (var i = 0; i < n; i++) {
			if (before[i].locked) { continue; }
			// Nothing beyond the cut on this track, so nothing to ripple.
			if (before[i].end <= b + eps) { continue; }
			if (after[i].end > before[i].end - span + eps) { bad.push(before[i].name); }
		}
		return bad;
	},

	// Lock state per track, so the panel can warn before a transaction fails.
	_trackInfo: function (seq) {
		var out = { video: [], audio: [] };
		var kinds = [['video', seq.videoTracks], ['audio', seq.audioTracks]];
		for (var t = 0; t < kinds.length; t++) {
			var kind = kinds[t][0];
			var collection = kinds[t][1];
			for (var i = 0; i < collection.numTracks; i++) {
				var track = collection[i];
				var locked = false;
				try { locked = !!track.isLocked(); } catch (e) { /* older builds */ }
				var count = 0;
				try { count = track.clips.numItems; } catch (e2) { /* ignore */ }
				out[kind].push({
					index: i,
					name: (kind === 'video' ? 'V' : 'A') + (i + 1),
					locked: locked,
					clips: count
				});
			}
		}
		return out;
	},

	// ---------------------------------------------------------- entry points

	// Full clip map for the panel's planner.
	getSequenceMap: function () {
		var seq = this._seq();
		if (!seq) {
			return this._err('No active sequence — open one in the timeline first.');
		}
		var self = this;
		var clips = [];
		var unreadable = 0;
		try {
			this._eachClip(seq, function (clip, kind, index) {
				var mediaPath = null;
				try {
					var pi = clip.projectItem;
					if (pi) {
						var p = pi.getMediaPath();
						// Titles, colour mattes and adjustment layers report '' here.
						if (p && String(p).length > 0) { mediaPath = String(p); }
					}
				} catch (e) { /* stills, offline media, generators */ }
				if (!mediaPath) { unreadable++; }

				var speed = 1;
				try { speed = Number(clip.getSpeed()); } catch (e2) { /* assume 1x */ }

				clips.push({
					name: String(clip.name),
					mediaPath: mediaPath,
					trackType: kind,
					trackIndex: index,
					start: clip.start.seconds,
					end: clip.end.seconds,
					inPoint: clip.inPoint.seconds,
					speed: speed
				});
			});
		} catch (e3) {
			return this._err('Could not read the sequence: ' + e3);
		}
		return this._ok({
			seqName: String(seq.name),
			fps: this._fps(seq),
			timelineEnd: this._timelineEnd(seq),
			tracks: this._trackInfo(seq),
			clips: clips,
			unreadable: unreadable
		});
	},

	// ------------------------------------------------------------- QE access

	_qeCache: null,

	_qe: function () {
		if (this._qeCache) { return this._qeCache; }
		try {
			if (typeof qe === 'undefined') { app.enableQE(); }
			if (typeof qe === 'undefined') { return null; }
			this._qeCache = qe;
			return qe;
		} catch (e) {
			return null;
		}
	},

	_qeSeq: function () {
		var q = this._qe();
		if (!q) { return null; }
		try { return q.project.getActiveSequence(); } catch (e) { return null; }
	},

	// Probe which of the operations we need actually exist on this build.
	// Surfaced in the panel's Diagnostics view — this is the first thing to
	// look at when a cut fails on an unfamiliar Premiere version.
	diagnostics: function () {
		var report = {
			autocut: this.VERSION,
			app: '', version: '',
			qeAvailable: false,
			qeSequence: false,
			capabilities: {},
			strategies: [],
			notes: []
		};
		// app.name is not defined on every build (Premiere 2026 omits it).
		try {
			report.app = (app.name === undefined || app.name === null)
				? 'Premiere Pro' : String(app.name);
		} catch (e) { report.app = 'Premiere Pro'; }
		try { report.version = String(app.version); } catch (e2) { /* ignore */ }

		var seq = this._seq();
		if (!seq) {
			report.notes.push('No active sequence — open one for a complete probe.');
		} else {
			report.fps = this._fps(seq);
			report.timelineEnd = this._timelineEnd(seq);
		}

		var q = this._qe();
		report.qeAvailable = !!q;
		if (!q) {
			report.notes.push('QE DOM unavailable: app.enableQE() did not expose `qe`. ' +
				'Splitting clips is impossible without it.');
		}

		var qseq = this._qeSeq();
		report.qeSequence = !!qseq;

		var has = function (obj, name) {
			if (!obj) { return false; }
			try { return typeof obj[name] === 'function'; } catch (e) { return false; }
		};

		report.capabilities['qeSeq.razor'] = has(qseq, 'razor');
		report.capabilities['qeSeq.extract'] = has(qseq, 'extract');
		report.capabilities['qeSeq.removeInToOut'] = has(qseq, 'removeInToOut');
		report.capabilities['seq.setInPoint'] = has(seq, 'setInPoint');
		report.capabilities['seq.setOutPoint'] = has(seq, 'setOutPoint');

		if (qseq) {
			var qTrack = null;
			try { qTrack = qseq.getVideoTrackAt(0); } catch (e2) { /* ignore */ }
			report.capabilities['qeTrack.razor'] = has(qTrack, 'razor');
			report.capabilities['qeTrack.getItemAt'] = has(qTrack, 'getItemAt');
			report.capabilities['qeTrack.setSyncLock'] = has(qTrack, 'setSyncLock');
			if (!has(qTrack, 'setSyncLock')) {
				report.notes.push('Sync lock cannot be set from script on this build. ' +
					'Turn it on manually for every track (the box left of each track ' +
					'name) or extract will leave some tracks behind.');
			}
			var qItem = null;
			try { if (qTrack && qTrack.numItems > 0) { qItem = qTrack.getItemAt(0); } } catch (e3) { /* ignore */ }
			report.capabilities['qeItem.remove'] = has(qItem, 'remove');
			report.capabilities['qeItem.move'] = has(qItem, 'move');
			report.capabilities['qeItem.roll'] = has(qItem, 'roll');
		}

		if (seq) {
			var firstClip = null;
			this._eachClip(seq, function (clip) { firstClip = clip; return false; });
			report.capabilities['trackItem.remove'] = has(firstClip, 'remove');
			report.capabilities['trackItem.move'] = has(firstClip, 'move');
			var track = null;
			try { track = seq.videoTracks[0]; } catch (e4) { /* ignore */ }
			report.capabilities['track.isLocked'] = has(track, 'isLocked');
			report.capabilities['track.overwriteClip'] = has(track, 'overwriteClip');
		}

		var strategies = this._strategies();
		for (var i = 0; i < strategies.length; i++) {
			report.strategies.push({
				id: strategies[i].id,
				label: strategies[i].label,
				available: strategies[i].available.call(this)
			});
		}
		return this._ok(report);
	},

	// ----------------------------------------------------------- strategies

	// Each strategy removes one sequence-time range and closes the gap across
	// every unlocked track. They are tried in order; the first available one
	// wins and is reused for the rest of the run.
	_strategies: function () {
		return [
			{
				id: 'extract',
				label: 'Sequence in/out + extract (native ripple, all tracks)',
				available: function () {
					var seq = this._seq();
					var qseq = this._qeSeq();
					if (!seq || !qseq) { return false; }
					var hasIO = false;
					try {
						hasIO = typeof seq.setInPoint === 'function' &&
							typeof seq.setOutPoint === 'function';
					} catch (e) { return false; }
					if (!hasIO) { return false; }
					try {
						return typeof qseq.extract === 'function' ||
							typeof qseq.removeInToOut === 'function';
					} catch (e2) { return false; }
				},
				// Extract obeys two separate switches: targeting decides which
				// tracks lose the marked range, sync lock decides which tracks
				// close the gap. A track with sync lock off keeps its content
				// exactly where it is while every other track slides left —
				// which is precisely how audio ends up playing over a hole in
				// the video. Force both on, restore both when the run ends.
				setup: function () {
					var seq = this._seq();
					this._savedTargeting = this._saveTargeting(seq);
					this._setAllTargeted(seq, true);
					this._savedSyncLock = this._forceSyncLock(seq);
				},
				teardown: function () {
					if (this._savedSyncLock) {
						this._restoreSyncLock(this._savedSyncLock);
						this._savedSyncLock = null;
					}
					if (this._savedTargeting) {
						this._restoreTargeting(this._savedTargeting);
						this._savedTargeting = null;
					}
				},
				apply: function (a, b) { return this._applyByExtract(a, b); }
			},
			{
				id: 'razor-lift-shift',
				label: 'QE razor + lift + uniform shift',
				available: function () {
					var qseq = this._qeSeq();
					if (!qseq) { return false; }
					try {
						var t = qseq.getVideoTrackAt(0);
						return !!t && typeof t.razor === 'function';
					} catch (e) { return false; }
				},
				apply: function (a, b) { return this._applyByRazor(a, b); }
			}
		];
	},

	// The value Premiere reports for an unset in/out point.
	NO_POINT: -400000,

	// Record every track's targeting so it can be put back exactly.
	_saveTargeting: function (seq) {
		var saved = [];
		var kinds = [seq.videoTracks, seq.audioTracks];
		for (var k = 0; k < kinds.length; k++) {
			for (var i = 0; i < kinds[k].numTracks; i++) {
				var track = kinds[k][i];
				var was = false;
				try { was = !!track.isTargeted(); } catch (e) { /* assume off */ }
				saved.push({ track: track, targeted: was });
			}
		}
		return saved;
	},

	_setAllTargeted: function (seq, on) {
		var kinds = [seq.videoTracks, seq.audioTracks];
		for (var k = 0; k < kinds.length; k++) {
			for (var i = 0; i < kinds[k].numTracks; i++) {
				try { kinds[k][i].setTargeted(on, true); } catch (e) { /* locked */ }
			}
		}
	},

	_restoreTargeting: function (saved) {
		for (var i = 0; i < saved.length; i++) {
			try { saved[i].track.setTargeted(saved[i].targeted, true); } catch (e) { /* ignore */ }
		}
	},

	// Turn sync lock on everywhere, returning what to put back. Sync lock only
	// exists on the QE track, and not on every build, so a build without it
	// simply returns an empty list — the extract still runs, it just relies on
	// whatever the user had set.
	_forceSyncLock: function (seq) {
		var qseq = this._qeSeq();
		if (!qseq) { return []; }
		var saved = [];
		var tracks = this._trackList(seq);
		for (var i = 0; i < tracks.length; i++) {
			var info = tracks[i];
			var qTrack = this._qeTrack(qseq, info);
			if (!qTrack || typeof qTrack.setSyncLock !== 'function') { continue; }
			// When the state cannot be read, assume it was on (Premiere's
			// default) so teardown does not silently turn it off for the user.
			var was = true;
			try { was = !!qTrack.isSyncLocked(); } catch (e) { /* assume on */ }
			saved.push({ qTrack: qTrack, syncLock: was });
			try { qTrack.setSyncLock(true); } catch (e2) { /* leave as found */ }
		}
		return saved;
	},

	_restoreSyncLock: function (saved) {
		for (var i = 0; i < saved.length; i++) {
			try { saved[i].qTrack.setSyncLock(saved[i].syncLock); } catch (e) { /* ignore */ }
		}
	},

	// The QE counterpart of a _trackList entry, or null.
	_qeTrack: function (qseq, info) {
		try {
			return info.kind === 'video'
				? qseq.getVideoTrackAt(info.index)
				: qseq.getAudioTrackAt(info.index);
		} catch (e) {
			return null;
		}
	},

	_saveInOut: function (seq) {
		var io = { inPoint: this.NO_POINT, outPoint: this.NO_POINT };
		try { io.inPoint = Number(seq.getInPoint()); } catch (e) { /* unset */ }
		try { io.outPoint = Number(seq.getOutPoint()); } catch (e2) { /* unset */ }
		return io;
	},

	// There is no clearInOutPoints on Premiere's Sequence, but writing the
	// sentinel back restores the "no in/out set" state.
	_restoreInOut: function (seq, io) {
		try { seq.setInPoint(io.inPoint); } catch (e) { /* ignore */ }
		try { seq.setOutPoint(io.outPoint); } catch (e2) { /* ignore */ }
	},

	// Strategy 1 — set the sequence in/out points to the range and let
	// Premiere extract it. This is exactly what a human does (mark in, mark
	// out, Extract): native, frame-accurate, ripples every track together,
	// and preserves effects on the clips it splits.
	//
	// Two things make or break it, both learned the hard way:
	//  - setInPoint/setOutPoint must be given NUMBERS. A string argument is
	//    silently coerced to 0, so the extract marks from the top of the
	//    timeline and, finding no valid range, does nothing at all.
	//  - Extract only touches TARGETED tracks. Untargeted tracks keep their
	//    content while everything else ripples left, which desyncs the edit.
	//    Targeting is forced on for the whole run and restored afterwards
	//    (see the strategy's setup/teardown).
	_applyByExtract: function (a, b) {
		var seq = this._seq();
		var qseq = this._qeSeq();
		if (!seq || !qseq) { throw new Error('lost the active sequence'); }

		seq.setInPoint(a);
		seq.setOutPoint(b);

		// Confirm the marks actually took before destroying anything.
		var gotIn = Number(seq.getInPoint());
		if (Math.abs(gotIn - a) > 0.5) {
			throw new Error('in point did not take (asked ' + a.toFixed(2) +
				's, sequence reports ' + gotIn + ')');
		}
		var gotOut = Number(seq.getOutPoint());
		if (Math.abs(gotOut - b) > 0.5) {
			throw new Error('out point did not take (asked ' + b.toFixed(2) +
				's, sequence reports ' + gotOut + ')');
		}

		if (typeof qseq.extract === 'function') {
			qseq.extract();
		} else if (typeof qseq.removeInToOut === 'function') {
			qseq.removeInToOut(true); // true = ripple (extract, not lift)
		} else {
			throw new Error('no extract entry point on this build');
		}
	},

	// Strategy 2 — razor both edges of the range on every unlocked track,
	// delete what now sits fully inside it, then shift everything after the
	// cut left by the same amount on every track. The shift is uniform and
	// manual precisely so tracks cannot drift apart, which per-track ripple
	// delete would cause.
	_applyByRazor: function (a, b) {
		var seq = this._seq();
		var qseq = this._qeSeq();
		if (!seq || !qseq) { throw new Error('lost the active sequence'); }
		var fps = this._fps(seq);
		var span = b - a;

		// Razor the later edge first: cutting at `a` cannot invalidate `b`,
		// but doing it in this order keeps item indices stable per track.
		this._razorAllTracks(seq, qseq, b, fps);
		this._razorAllTracks(seq, qseq, a, fps);

		// Delete everything now fully inside [a, b] on unlocked tracks, then
		// close the gap uniformly across every track.
		this._removeInside(seq, a, b, fps);
		this._shiftAfter(seq, b, -span, fps);
	},

	// Delete every clip sitting fully inside [a, b] on unlocked tracks.
	//
	// Walk each track backwards. TrackItem references are index-based, so
	// removing one invalidates every reference after it but none before it —
	// collecting the doomed clips up front and deleting them front-to-back
	// removes the wrong clip, or misses one and leaves it stranded inside the
	// cut while the rest of the timeline ripples past.
	_removeInside: function (seq, a, b, fps) {
		var eps = this._eps(fps);
		var tracks = this._trackList(seq);
		for (var t = 0; t < tracks.length; t++) {
			if (tracks[t].locked) { continue; }
			var clips = tracks[t].track.clips;
			var count = 0;
			try { count = clips.numItems; } catch (e) { continue; }
			for (var j = count - 1; j >= 0; j--) {
				var clip = clips[j];
				if (clip.start.seconds >= a - eps && clip.end.seconds <= b + eps) {
					try {
						clip.remove(false, false); // no ripple: we shift manually below
					} catch (e2) {
						throw new Error('could not delete a clip inside the cut on ' +
							tracks[t].name + ': ' + e2);
					}
				}
			}
		}
	},

	// Razor every unlocked video and audio track at `seconds`, and prove it
	// happened.
	//
	// A razor that quietly does nothing is the worst failure this panel has:
	// the clip survives the cut, every other track ripples around it, and the
	// run reports success because the timeline as a whole did get shorter.
	// Refusing to continue leaves an edit the user can simply undo.
	_razorAllTracks: function (seq, qseq, seconds, fps) {
		var tc = this._toTimecode(seconds, fps);
		var tracks = this._trackList(seq);
		for (var i = 0; i < tracks.length; i++) {
			var info = tracks[i];
			if (info.locked) { continue; }
			// Nothing to razor if no clip actually spans this instant.
			if (!this._spansTime(info.track, seconds, fps)) { continue; }
			var qTrack = this._qeTrack(qseq, info);
			if (!qTrack || typeof qTrack.razor !== 'function') {
				throw new Error('cannot split ' + info.name +
					': this build exposes no razor for that track');
			}
			this._razorTrack(qTrack, tc, seconds);
			if (this._spansTime(info.track, seconds, fps)) {
				throw new Error('razor did not split ' + info.name + ' at ' + tc +
					' (' + seconds.toFixed(3) + 's)');
			}
		}
	},

	// Razor one track, verifying it actually happened. Builds differ on
	// whether razor wants a timecode string or plain seconds, so try
	// timecode and fall back to seconds if the item count did not change.
	_razorTrack: function (qTrack, timecode, seconds) {
		var before = -1;
		try { before = qTrack.numItems; } catch (e) { /* ignore */ }
		try { qTrack.razor(timecode); } catch (e2) { /* try the other form */ }
		var after = -1;
		try { after = qTrack.numItems; } catch (e3) { /* ignore */ }
		if (before >= 0 && after === before) {
			try { qTrack.razor(String(seconds)); } catch (e4) { /* give up quietly */ }
		}
	},

	// Does any clip on this track cover `seconds` strictly inside its body?
	// Razoring exactly on an existing edit point is a no-op we can skip.
	_spansTime: function (track, seconds, fps) {
		var eps = this._eps(fps);
		try {
			for (var j = 0; j < track.clips.numItems; j++) {
				var c = track.clips[j];
				if (c.start.seconds < seconds - eps && c.end.seconds > seconds + eps) {
					return true;
				}
			}
		} catch (e) { /* ignore */ }
		return false;
	},

	// Move every clip starting at or after `fromSeconds` by `deltaSeconds`.
	// Clamped so nothing is pushed before time 0.
	//
	// Done one track at a time, addressing clips by index and re-reading each
	// one immediately before it moves. Holding TrackItem references across
	// edits is what makes this drift: the reference is a position on the
	// track, not a handle on the clip. Within a track the moves run left to
	// right, so a clip never lands on one that has not moved yet, and since
	// they only ever move left into space that was just emptied, the ordering
	// — and therefore the indices — hold for the whole pass.
	_shiftAfter: function (seq, fromSeconds, deltaSeconds, fps) {
		var eps = this._eps(fps);
		var tracks = this._trackList(seq);
		for (var t = 0; t < tracks.length; t++) {
			if (tracks[t].locked) { continue; }
			var clips = tracks[t].track.clips;
			var indices = [];
			try {
				for (var j = 0; j < clips.numItems; j++) {
					if (clips[j].start.seconds >= fromSeconds - eps) { indices.push(j); }
				}
			} catch (e) { continue; }

			for (var m = 0; m < indices.length; m++) {
				var clip = clips[indices[m]];
				var start = clip.start.seconds;
				var delta = deltaSeconds;
				if (start + delta < 0) { delta = -start; }
				if (delta > -0.0005) { continue; }
				this._moveClip(clip, delta, fps);
			}
		}
	},

	// Move one clip along its track by a signed offset in seconds.
	// Prefers the documented TrackItem.move(Time); falls back to QE's
	// timecode-string move for builds that lack it.
	_moveClip: function (clip, deltaSeconds, fps) {
		var moved = false;
		try {
			if (typeof clip.move === 'function') {
				var t = new Time();
				t.seconds = deltaSeconds;
				clip.move(t);
				moved = true;
			}
		} catch (e) { moved = false; }
		if (moved) { return; }
		throw new Error('this Premiere build exposes no way to move a clip ' +
			'(TrackItem.move missing) — run Diagnostics and use the extract strategy');
	},

	// ------------------------------------------------------------ apply cuts

	// paramsJson: { ranges: [[a,b], ...] descending, strategy?: 'auto'|id }
	applyCuts: function (paramsJson) {
		var params, seq;
		try { params = JSON.parse(paramsJson); } catch (e) {
			return this._err('Bad parameters: ' + e);
		}
		var ranges = params.ranges || [];
		seq = this._seq();
		if (!seq) { return this._err('No active sequence — open one in the timeline first.'); }
		if (ranges.length === 0) { return this._err('The plan contained no ranges.'); }

		// Warn about locked tracks up front: a locked track silently keeps its
		// content while everything else shifts, which desyncs the timeline.
		var tracks = this._trackInfo(seq);
		var lockedNames = [];
		var groups = [tracks.video, tracks.audio];
		for (var g = 0; g < groups.length; g++) {
			for (var t = 0; t < groups[g].length; t++) {
				if (groups[g][t].locked && groups[g][t].clips > 0) {
					lockedNames.push(groups[g][t].name);
				}
			}
		}

		// Pick a strategy once and keep it for the whole run.
		var chosen = null;
		var available = [];
		var strategies = this._strategies();
		for (var i = 0; i < strategies.length; i++) {
			var isAvailable = false;
			try { isAvailable = strategies[i].available.call(this); } catch (e2) { isAvailable = false; }
			if (isAvailable) { available.push(strategies[i].id); }
			if (isAvailable && !chosen) {
				if (!params.strategy || params.strategy === 'auto' ||
					params.strategy === strategies[i].id) {
					chosen = strategies[i];
				}
			}
		}
		if (!chosen) {
			return this._err(
				'No usable cut strategy on this Premiere build (' +
				(function () { try { return String(app.version); } catch (e3) { return '?'; } }()) +
				'). Run Diagnostics in the panel and send the report.',
				{ available: available }
			);
		}

		var fps = this._fps(seq);
		var endBefore = this._timelineEnd(seq);
		var applied = 0;
		var failed = 0;
		var stopped = false;
		var errors = [];
		var results = [];

		// The run borrows the sequence's in/out marks and track targeting;
		// both are handed back exactly as they were, even if a cut throws.
		var savedInOut = this._saveInOut(seq);
		if (typeof chosen.setup === 'function') {
			try { chosen.setup.call(this); } catch (eSetup) {
				return this._err('could not prepare the sequence: ' + eSetup);
			}
		}

		try {
			for (var r = 0; r < ranges.length; r++) {
				var a = Number(ranges[r][0]);
				var b = Number(ranges[r][1]);
				if (!isFinite(a) || !isFinite(b) || b <= a) {
					failed++;
					errors.push('range ' + r + ': invalid bounds');
					continue;
				}
				var endsBefore = this._trackEnds(seq);
				var before = this._maxEnd(endsBefore);
				try {
					chosen.apply.call(this, a, b);
					var endsAfter = this._trackEnds(seq);
					var after = this._maxEnd(endsAfter);
					var shrank = before - after;
					var desynced = this._desyncedTracks(endsBefore, endsAfter, a, b, fps);
					// A range that changed nothing means the strategy silently
					// no-oped; report it rather than claiming success.
					if (shrank < (b - a) * 0.5 - this.EPS) {
						failed++;
						errors.push('cut ' + a.toFixed(2) + '-' + b.toFixed(2) +
							's: timeline only shrank ' + shrank.toFixed(2) + 's of ' +
							(b - a).toFixed(2) + 's expected');
						results.push({ a: a, b: b, ok: false, shrank: shrank });
					} else if (desynced.length > 0) {
						// The timeline got shorter but not every track came
						// with it, so the edit is already out of sync. Every
						// further cut compounds it and makes the undo longer,
						// so stop here and let the user undo cleanly.
						failed++;
						stopped = true;
						errors.push('cut ' + a.toFixed(2) + '-' + b.toFixed(2) + 's: ' +
							desynced.join(', ') + ' did not ripple with the rest of the ' +
							'timeline. Stopped to avoid compounding the desync — undo, ' +
							'then run Diagnostics.');
						results.push({ a: a, b: b, ok: false, shrank: shrank, desynced: desynced });
						break;
					} else {
						applied++;
						results.push({ a: a, b: b, ok: true, shrank: shrank });
					}
				} catch (e4) {
					failed++;
					errors.push('cut ' + a.toFixed(2) + '-' + b.toFixed(2) + 's: ' + e4);
					results.push({ a: a, b: b, ok: false });
				}
			}
		} finally {
			if (typeof chosen.teardown === 'function') {
				try { chosen.teardown.call(this); } catch (eDown) { /* best effort */ }
			}
			this._restoreInOut(seq, savedInOut);
		}

		var endAfter = this._timelineEnd(seq);

		return this._ok({
			strategy: chosen.id,
			applied: applied,
			failed: failed,
			stopped: stopped,
			remaining: stopped ? ranges.length - applied - failed : 0,
			errors: errors,
			results: results,
			endBefore: endBefore,
			endAfter: endAfter,
			removedSeconds: endBefore - endAfter,
			lockedTracks: lockedNames
		});
	}
};

// evalScript resolves with whatever the last expression returns; make loading
// the file itself a cheap, harmless success value.
'$._autocut ' + $._autocut.VERSION + ' loaded';
