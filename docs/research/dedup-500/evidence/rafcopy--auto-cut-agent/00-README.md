# Auto Cut — Premiere Pro extension

Removes silences from a Premiere Pro sequence and closes the gaps, cutting
**every track at the same points** so b-roll, screen recordings and music stay
frame-aligned with the voice track.

```
Timeline ──► panel reads clips (media paths, in-points, fps)
                    │
                    ▼
             ffmpeg silencedetect      ← runs inside the panel, no server
                    │  silences in source time
                    ▼
             cut planner               ← merge, absorb slivers, snap to frames
                    │  ranges in sequence time
                    ▼
             [ preview — you approve ]
                    │
                    ▼
             ExtendScript engine       ← removes ranges right-to-left,
                                         all unlocked tracks, gaps closed
```

One folder, no dependencies, nothing to run in a terminal. The panel talks to
ffmpeg itself through CEP's Node integration.

## Requirements

- **Premiere Pro 22.0 or newer** (built and tested against 2026)
- **ffmpeg** — `brew install ffmpeg`
- macOS (the installer is macOS-only; the panel itself is cross-platform)

No Node server, no API keys, no accounts.

## Install

```bash
cd auto-cut-agent
./install.sh
```

Then **fully quit and reopen Premiere**, and open the panel:

- **PPro 2025 / 2026:** `Window > Extensions (Legacy) > Auto Cut`
- **PPro 2024 and earlier:** `Window > Extensions > Auto Cut`

`./install.sh --uninstall` removes it.

The panel is symlinked into `~/Library/Application Support/Adobe/CEP/extensions/`,
so edits to this repo take effect as soon as you reopen the panel.

## Use

1. Open a sequence. **Duplicate it first** — cuts are undoable, but a spare
   sequence is cheaper than trusting undo.
2. Pick a **preset** (talking head, interview, screencast, aggressive).
3. Click **Measure from audio** — reads your actual noise floor with ffmpeg and
   sets the threshold. This beats guessing at dB values.
4. Click **Analyze**. The panel reports how many cuts it found and how much
   time they remove, and lists the longest ones.
5. Happy with it? Click **Apply cuts**.

Nothing touches the timeline until you press Apply.

### Settings

| Setting | What it does |
|---|---|
| **Noise threshold** | Audio below this level counts as silence. Less negative = cuts more. |
| **Min silence** | Quiet spots shorter than this are kept, protecting natural word gaps. |
| **Cut padding** | Leaves this much air at each cut edge so cuts don't feel clipped. |
| **Merge cuts within** | Two cuts closer than this become one — avoids machine-gun micro-cuts. |
| **Min kept clip** | Kept fragments shorter than this are absorbed into the surrounding cut. |
| **Analyze which tracks** | Which video/audio track drives detection. Everything else is cut in sync. |

### Track model

Detection runs only on the tracks you select (default V1/A1 — your raw clip).
Every other **unlocked** track is cut at the same ranges without being
analyzed, which is what keeps the timeline in sync.

**Locked tracks are not cut.** The panel warns you before and after a run,
because a locked track keeps its content while everything else shifts left —
that silently desyncs your edit. Unlock everything before cutting.

## How the engine works

Premiere's documented ExtendScript API can *trim* a clip but cannot *split* one
or *move* one along a track. Both live in the QE DOM (`app.enableQE()`), which
Adobe has never documented and which has drifted between versions.

Rather than betting on one call signature, the engine declares several removal
strategies, probes which the running Premiere actually supports, and uses the
first that works:

| Strategy | How | Notes |
|---|---|---|
| `extract` | Set sequence in/out to the range, extract | Native ripple across all tracks. Exactly what a human does. Preferred. |
| `razor-lift-shift` | QE razor both edges, delete inside, shift the rest left | Uniform manual shift, so tracks cannot drift apart. |

Two non-obvious constraints govern the `extract` path, both found by watching it
fail silently on a real timeline:

- `setInPoint`/`setOutPoint` must be passed **numbers**. A string argument is
  coerced to `0`, so the extract marks from the top of the timeline and quietly
  does nothing.
- Extract only touches **targeted** tracks. The engine therefore targets every
  track for the duration of the run and restores your targeting afterwards —
  along with your in/out marks — even if a cut throws.

Safety properties:

- Ranges are applied **right-to-left**, so times below the current cut stay
  valid as later content shifts.
- Cut points are **snapped inward to the frame grid** — rounding inward can
  keep a frame that should have gone, but never eats a frame of speech.
- After every range the engine checks the timeline **actually shrank** by
  roughly the expected amount. A strategy that silently no-ops is reported as a
  failure instead of being counted as success.
- Speed-ramped clips are skipped with a warning: source time cannot be mapped
  1:1 through a speed change.

**Diagnostics** (gear icon → Run diagnostics) prints exactly which APIs your
Premiere build exposes and which strategies are available. That report is the
first thing to look at if a cut fails.

## Development

```bash
npm test    # 51 tests, no Premiere needed
```

`extension/js/core.js` is deliberately free of Premiere, ffmpeg and DOM
references, so the planner is testable under plain Node. The tests cover
ffmpeg output parsing, source→sequence time mapping, range merging, frame
snapping and parameter clamping.

They also cover `host.jsx`'s timecode conversion, by shimming ExtendScript's
`$` global and evaluating the real file — that function decides where the
razor lands, and NTSC drop-frame is easy to get subtly wrong.

Debug the live panel at <http://localhost:8092> while it is open.

## Layout

```
extension/
  CSXS/manifest.xml   CEP manifest (PPRO 22.0+, Node enabled)
  .debug              devtools on port 8092
  index.html, css/    panel UI
  js/core.js          pure planning logic — shared with the tests
  js/main.js          panel: ffmpeg, preview, orchestration
  jsx/host.jsx        ExtendScript: sequence reading + cut engine
scripts/test.js       test suite (npm test)
install.sh            symlink installer + PlayerDebugMode
```

## Honest limits

- **CEP is end-of-life.** Adobe is retiring CEP/ExtendScript around September
  2026. This panel targets it because the cut operations it needs are only
  reachable from the QE DOM; a UXP port will be needed when Premiere's UXP API
  gains split and move.
- **The engine leans on undocumented QE calls.** Capability probing and
  post-cut verification contain the risk, but an unfamiliar Premiere build can
  still come up unsupported — run Diagnostics if so.
- **Speed-ramped clips are skipped**, not cut.
- **Nested sequences** are cut as opaque blocks; the panel cannot analyze
  audio inside them.
- Detection is level-based, not speech-aware: it removes *silence*, not filler
  words. Removing "um" and "uh" needs a transcript, which needs a speech model
  — deliberately out of scope here to keep the panel self-contained.
- Media must exist on disk; the panel analyzes the real file, not Premiere's
  render.

## License

MIT — see [LICENSE](LICENSE).
