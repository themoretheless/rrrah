# Metal storage and HDR qualification

The 2026-10-07 selected-scenario result is recorded in
[the soak report](research/metal-soak-qualification-2026-10-07.json) and
[the live HDR report](research/metal-live-hdr-2026-10-07.json).
These results do not qualify every image format, indefinite stability, a physical
slow disk, absolute HDR luminance or calibrated color output.

## Reproduce on a Mac with Metal

Run from the repository root. The delayed-I/O test uses temporary real swap files,
2 ms delays per codec I/O call, 256 KiB objects, a 2 MiB shared memory root and a
16-entry swap. It occupies the entire root before each restore, verifies refusal,
then releases it and verifies all pixel bytes and metadata on retry.

```sh
RRRAH_STRESS_SECONDS=600 cargo test -p rrrah-cache --test sustained_swap sustained_large_keyspace_delayed_swap_pressure_and_retry --locked --offline -- --include-ignored --nocapture
cargo test -p rrrah-cache --test sustained_swap cancelled_delayed_restore --locked --offline
cargo test -p rrrah-cache --test sustained_swap idle_ttl --locked --offline
RRRAH_STRESS_SECONDS=30 cargo test -p rrrah-cache --test sustained_swap sustained_byte_cap --locked --offline -- --include-ignored --nocapture
RRRAH_GPU_BACKEND=metal RRRAH_METAL_POLICY_CYCLES=200 cargo test -p rrrah --test ttl_swap_metal live_count_size_and_ttl_changes_preserve_hdr_frames_through_swap --locked --offline -- --include-ignored --nocapture
```

The Metal test checks exact frames, float bits, consumer leases, live TTL/count/size
changes, restores and final release of managed memory on each cycle. It requires a
GPU and cannot silently pass through the optional-GPU path.

## Live HDR viewer

```sh
RRRAH_GPU_BACKEND=metal RRRAH_HDR_SURFACE=1 RUST_LOG=info cargo run -p rrrah --bin rrrah --locked --offline -- --no-cache tests/fixtures/raster/float-none-rec709-declared.exr
```

Require `backend=Metal`, `hdr=true`, `format=Rgba16Float` and
`color_space=ExtendedSrgbLinear` in the log, and a ready image in the window.
Screen headroom can be queried independently without changing display settings:

```sh
clang -fobjc-arc -framework AppKit scripts/probe-metal-edr.m -o /tmp/rrrah-edr-probe
/tmp/rrrah-edr-probe
```

Headroom is dynamic OS state, not a luminance measurement. A screenshot cannot
verify HDR nits. NVIDIA/CUDA qualification is deferred and has its own hardware
requirement.

Models also support the linear RGBA16Float HDR surface. Run the same viewer
command with `tests/fixtures/models/triangle.stl` and require a ready model in
the window. `rrrah-gpu --test model_readback` checks linear HDR material values,
depth, geometry and budget release. The recorded numerical and live-window
result is in `research/metal-model-hdr-2026-10-07.json`.

## One combined regression and soak run

```sh
python3 scripts/qualify-viewer-storage-gpu.py --target-dir /tmp/rrrah-metal-target --output-dir /tmp/rrrah-metal-results --gpu-backend metal --storage-soak-seconds 600 --metal-policy-cycles 200
```

The output directory must be new. Each suite keeps its own log, and `report.json`
records failures and ignored tests. Each suite must record at least one passing
test; a zero-test successful command is
rejected. This proves execution, while fixture skips still remain explicit.
The specified soak duration applies to each
of the two storage modes separately: count/TTL pressure and independent byte
pressure. The runner also includes cancellation, idle physical-file cleanup and
writer-overload checks. The GPU policy test uses the requested number of cycles.
GPU availability is required; this runner overrides optional-GPU skipping.
Add `--full-sensor-compute` to execute both normally ignored 6188×4120 compute
tests, including exact per-pixel output and tracked CPU/GPU budget release.
Add `--external-viewer-corpus` with `RRRAH_SRF_SOURCE`, `RRRAH_EIP_APP_FIRST`,
`RRRAH_EIP_APP_SECOND` and `RRRAH_AI_CORPUS` set to the qualified local sources
to execute all prefetch tests and EIP TTL transport. The AI directory must
contain `VectorApple.ai` and `one.ai`; `tests/IMG_9043.CR3` is also required.
Missing paths fail before any suite starts. Paths are recorded in the report;
these tests do not prove universal RAW/EIP/AI format coverage.

The combined runner does not launch or certify the live HDR window. Use the
separate live-viewer and screen-probe steps above for that evidence. Passing
ignored-free soak modes does not turn unrelated ignored fixture tests into
verified results.

The 2026-10-07 run after the HDR model fix completed all nine suites on Metal:
273 passing test executions, zero failures, and 15 ignored test instances.
This includes repeated tests; selected manual soak/policy tests ran separately,
while other ignored fixtures and NVIDIA hardware remain unverified. This run
used 10 seconds per storage soak mode and three GPU policy cycles; longer
individual runs are recorded separately. See
`research/metal-final-regression-2026-10-07.json` for commands and suite results.

The external PICT corpus was restored on 2026-10-07 and all four source hashes
were verified. Fresh TwelveMonkeys 3.13.0 RGBA outputs match the pinned oracle
hashes. The PICT qualifier passes 23 native tests and three Metal transport
tests (seven source cases), including swap pressure. See
`research/metal-pict-restored-2026-10-07.json`. Reproduce with
`scripts/qualify-pict.py --corpus /tmp/rrrah-pict-restored-20261007 --target-dir /tmp/rrrah-required-corpus-target --report /tmp/rrrah-pict-report.json --metal`
and `RRRAH_GPU_BACKEND=metal RRRAH_GPU_OPTIONAL=0` in the environment.

The SD10 source and independent dcraw source were also restored with pinned
hashes. Fresh linear sRGB output matches the prior qualified PPM hash.
Full-frame Metal readback checks all 3,429,971 pixels with maximum RGBA8
deviation one and releases CPU leases. See
`research/metal-x3f-restored-2026-10-07.json`. This closes the selected X3F
fixture skip through a separate run, not universal X3F or physical HDR coverage.

HERO9 dual-illuminant ColorMatrix resolution now uses AsShotNeutral and
reciprocal-temperature interpolation in native Rust. Its transform matches
an independent Adobe DNG SDK matrix to 5.04e-9 in the same output space.
The parser admits this solver only for three channels with absent/identity
CameraCalibration and AnalogBalance and absent ForwardMatrix. Other profiles
retain the existing path and need separate qualification.
Seven camera-color tests and ten ordinary raster tests pass on Metal Apple
M4 Max. A separately invoked full-frame HERO9 transport test checks all
23,251,968 pixels against CPU display output, with maximum RGB8 difference
one. Reproduce the latter with `RRRAH_GPU_BACKEND=metal RRRAH_GPU_OPTIONAL=0
RRRAH_HERO9_LINEAR_DUMP=/path/to/production.rgba32fle` and
`cargo test -p rrrah-gpu --test raster_readback hero9_full_frame -- --ignored --nocapture`.
The dump is produced by `raw_developed_dump` from the official HERO9 source.
See `research/gpr-hero9-neutral-resolution-2026-10-07.json` for input/output
hashes and color diagnostics. Full-image demosaic differences against Adobe
remain; this is not format-wide GPR color qualification.

The same production color path is independently checked for all six official
GPR inputs: HERO5/6/7/9 and Fusion front/back. Maximum transform discrepancy
in the oracle's output space is 2.12e-7 after native f32 metadata storage.
An external regression also checks AsShotNeutral WB and managed-memory release;
24 authored sensor color fields pass on Metal against the independent matrices.
See `research/gpr-six-camera-matrices-2026-10-07.json` and reproduce with
`RRRAH_GPR_CORPUS=/path/to/official/sources cargo test -p rrrah-decode --test gpr_color_oracle -- --ignored --nocapture`.
The common qualification runner now always includes camera-color GPU tests;
`--gpr-corpus` additionally validates the six source hashes and runs the
production external regression. The SDK's bilinear demosaic and native AHD
are different algorithms; full-image quality remains a separate qualification.

The combined 2026-10-07 regression with the pinned GPR corpus executes nine
suites: cache, CUDA host, prefetch, model swap/display, HDR/TTL, camera color,
GPU shaders/readback, and external native GPR color. All 276 passing test
executions complete without failures; 16 ignored instances are recorded
separately and are not included as proof. This is a regression run, not a new
long storage soak or physical NVIDIA/HDR qualification. See
`research/metal-six-gpr-regression-2026-10-07.json`.

Full scene-linear GPR development has also been generated for all six official
inputs, with independent SDK Stage3 images. All 77,875,968 output pixels are
finite with opaque alpha. Source orientation is read independently from the
converted DNG: HERO5/6/7 and both Fusion inputs are horizontally reflected;
Stage3 must be reflected before comparing to final native development.
Across all six inputs, the largest channel mean absolute difference of 32x32
linear block averages is 0.000951. Whole-frame overviews show matching framing
and Fusion geometry; residuals concentrate at fine edges and highlights.
These are diagnostic measurements, not a perceptual pass/fail criterion or
universal GPR coverage. See
`research/gpr-six-camera-full-image-diagnostics-2026-10-07.json` and
`scripts/analyze-gpr-linear-quality.py` for reproducible calculations.

A later highlight audit found and fixed a provenance error: rectilinear
OpcodeList3 moved RGB but left the sensor-clipping mask in sensor coordinates.
The mask now follows each channel's nonzero bicubic support. Authored
regressions verify relocation, channel-specific geometry, identity/cancellation,
and saturated-green reconstruction to the expected neutral level at the moved
location. Four regressions and the full 104-test core suite pass (two separate
manual/corpus tests remain ignored).
Fresh Fusion back/front development changes 889/173 pixels respectively,
releases managed memory, and retains the existing scratch admission bound.
Separately invoked Metal transport tests check all 18,624,000 corrected pixels
with maximum RGB8 deviation one. See
`research/gpr-fusion-clipping-warp-2026-10-07.json`; the earlier whole-frame
image diagnostics remain measurements from before this correction.
DNG adapter revision 2 also invalidates cached color metadata predating the
neutral-dependent dual-illuminant transform; its regression passes.

On 2026-10-08, paired fresh HERO9 development with highlight reconstruction
on/off checks all 23,251,968 pixels against an independently decoded sensor.
The 110 saturated photosites define a conservative 5x5 support of 2,394 pixels;
all 646 changed output pixels lie inside that support. Every other pixel is
bit-identical. 28,498 output pixels retain values above one, and managed memory
returns to zero. The external regression passes without skips. See
`research/gpr-hero9-highlight-support-2026-10-08.json`.
Reproduce with `RRRAH_GPR_SOURCE` and `RRRAH_GPR_SENSOR_ORACLE` and
`cargo test -p rrrah-decode --test gpr_highlight_support -- --ignored --nocapture`.
The common runner includes it when supplied both `--gpr-corpus` and
`--gpr-hero9-sensor-oracle`; it verifies the independent sensor SHA256 before
starting. This proves bounded effects and HDR retention on HERO9, not recovery
of unknown scene colors or quality on every clipped camera input.

The 2026-10-08 combined regression after color, clipping-provenance and cache
revision fixes runs ten suites, including pinned GPR color and real highlight
support: 277 passing executions, zero failures, 18 ignored instances recorded
separately. See `research/metal-highlight-regression-2026-10-08.json`.

A fresh native viewer launch also verifies actual EDR activation on the built-in
Retina display. The declared Rec.709 RGBE fixture contains linear values from
0.5 to 6.625. The Metal window reports RGBA16Float/ExtendedSrgbLinear and a ready
8x2 raster. Read-only NSScreen queries report current maximum component value
1 before launch, 6.153846 while the HDR window is running, and 1 after stopping
the owned test process; potential maximum is 16. No display settings were
changed. Reproduce the read-only query with `swift scripts/probe-metal-edr.swift`.
See `research/metal-live-display-edr-2026-10-08.json`. This closes live EDR
activation evidence for this display; physical nits/calibration and other HDR
format/profile coverage remain separate requirements.

Three app control-path regressions separately pass after the combined run:
HERO9 defaults to corrected quality development, cancellation after float
admission releases ownership while pressure refusal retains the sensor, and
Fusion selects its geometry correction without explicit quality settings.
See `research/gpr-viewer-quality-regression-2026-10-08.json`. The runner now
includes these app suites with `--gpr-corpus`; their separate executions are
not added to the historical 277-count report.

### External input availability audit (2026-10-11)

Run `python3 scripts/audit-metal-required-inputs.py --output /tmp/rrrah-metal-input-audit.json` before repeating the five manual camera-corpus tests. It streams SHA-256 over every pinned input, reports all missing/mismatched/unreadable inputs together, and exits nonzero unless every input is verified. Historical successful test reports do not establish current input availability. The current audit finds all five pinned temporary inputs absent (Fusion front/back linear dumps, HERO9 linear dump, qualified SD10 linear-sRGB PPM, F828 sensor oracle). No substitute files were used or promoted to equivalent evidence. Recover the matching pinned inputs or regenerate and requalify them before asserting a fresh pass for these cases.

The later recovery run restored all five original pinned inputs under `tests/fixtures/data/metal-recovery/`. Run `python3 scripts/audit-metal-required-inputs.py --input-root tests/fixtures/data/metal-recovery` to verify them at the time of use. The current recovered audit exits0; evidence is `research/metal-required-inputs-all-recovered-audit-2026-10-11.json`. The earlier missing-input audit remains historical evidence of the temporary-directory gap.

The default non-`--gpu-only` qualification with `--gpu-backend metal` now mandates explicit swapped viewport and source-navigation suites. It requires at least7 and6 passing tests respectively, zero ignored tests, and workload markers covering exact36MP cached LODs, pressure/corruption and stale source cancellation. Ordinary viewer success alone cannot substitute for these cases. The verified default run is recorded in `research/metal-default-gate-requires-swapped-residency-2026-10-11.json`.

### Original MEF full-sensor compute

Pass `--mef-sensor-source /absolute/path/Mamiya-ZD.MEF` to `scripts/qualify-viewer-storage-gpu.py` with `--gpu-backend metal`. The source must match `tests/fixtures/mef-investigation.json`; missing/mismatched input or another backend fails before execution. The suite checks all 21461504 original sensor positions through explicitly authored channels, and requires exact output plus final zero CPU/GPU credits. It does not qualify Mamiya WB, demosaic, camera color, viewer routing or physical HDR. Can be combined with `--gpu-only` for all ordinary GPU tests plus this case.

### Native AI replay-plane transport

With the pinned original corpus and finer-contour replay outputs under the recovery root:

```sh
RRRAH_GPU_BACKEND=metal RRRAH_AI_METAL_ROOT="$PWD/tests/fixtures/data/metal-recovery" cargo test -p rrrah-gpu --test ai_native_transport --locked -- --ignored --nocapture
```

This checks all three white-composited replay planes at 1:1 through actual Metal and managed CPU/texture/upload ownership. Source and output SHA256 pins are mandatory. It does not invoke or qualify production AI decoding, original appearance, or a physical HDR display. [Measured evidence](research/ai-native-managed-metal-transport-2026-10-11.json).

The same pinned transport can be required by the combined driver:

```sh
python3 scripts/qualify-viewer-storage-gpu.py --target-dir target --output-dir /tmp/rrrah-ai-metal-gate --gpu-backend metal --gpu-only --ai-native-root "$PWD/tests/fixtures/data/metal-recovery"
```

Use a fresh output directory. The driver verifies five pinned inputs before creating output, requires all three per-plane workload markers and final ownership release, and records the test logs and hashes. Omit `--gpu-only` to include the ordinary viewer/storage suites as well.

For pinned AI RAM leases and both native/prepared swap representations, with pressure refusal, cancellation, retry and exact restored Metal frames:

```sh
RRRAH_GPU_BACKEND=metal RRRAH_AI_METAL_ROOT="$PWD/tests/fixtures/data/metal-recovery" cargo test -p rrrah --test ai_swap_metal --locked -- --ignored --nocapture
```

This remains test-only replay-plane storage/transport qualification. [Measured evidence](research/ai-native-ram-swap-pressure-metal-2026-10-11.json).

When `--ai-native-root` is set without `--gpu-only`, the combined driver also requires `ai_swap_metal`: all three pages in sRGB8 and linear RGBA32F, live RAM lease behavior, pressure/cancellation retry, exact restored GPU frames and zero final credit. Missing or duplicate cases cannot satisfy this workload check.

Thumbnail completion and retry without redraw, while metadata inspection is blocked:

```sh
RRRAH_GPU_BACKEND=metal cargo test -p rrrah --bin rrrah thumbnail_budget_callbacks_progress_without_surface_redraw --locked -- --ignored --nocapture
```

Requires actual Metal and checks two bounded queue/texture ownership cycles. [Measured evidence](research/metal-thumbnail-completion-pressure-retry-2026-10-11.json).

Full attachments above the portable texture width and at the adapter's actual supported width:

```sh
RRRAH_GPU_BACKEND=metal cargo test -p rrrah --bin rrrah viewer_uses_supported_metal_texture_extent_above_default --locked -- --ignored --nocapture
```

Checks managed source/texture release, portable tiled admission, all SDR pixels and exact HDR attachment samples. Physical HDR remains separate. [Measured evidence](research/metal-full-supported-texture-extent-2026-10-11.json).

The texture-extent test now uses a coordinate pattern, compares every full SDR/HDR pixel, and checks eight cropped HDR views on the portable tiled device. [Updated measured evidence](research/metal-texture-coordinate-pattern-2026-10-11.json).

Original AI production-loader RAM hits and independent page keys under full managed root pressure, with exact cached SDR/HDR Metal transport:

```sh
RRRAH_AI_CORPUS="$PWD/tests/fixtures/data/metal-recovery/ai-corpus" RRRAH_GPU_BACKEND=metal cargo test -p rrrah --bin rrrah real_ai_ --locked -- --ignored --nocapture
```

This invokes production loading rather than loading test-only replay dumps. Independent original appearance and physical HDR remain unqualified. [Measured evidence](research/real-ai-production-cache-metal-pressure-2026-10-11.json).

Queued original AI page reversal, stale blocked preload cancellation and latest-page Metal cache preservation:

```sh
RRRAH_AI_CORPUS="$PWD/tests/fixtures/data/metal-recovery/ai-corpus" RRRAH_GPU_BACKEND=metal cargo test -p rrrah --bin rrrah queued_ai_page_reversal --locked -- --ignored --nocapture
```

Uses production submit/cache routines and a test-owned worker; no window or mid-render cancellation claim. [Measured evidence](research/ai-queued-page-reversal-metal-2026-10-11.json).

Original AI cancellation after observing live renderer scratch admission, followed by exact production-cache and Metal recovery:

```sh
RRRAH_AI_CORPUS="$PWD/tests/fixtures/data/metal-recovery/ai-corpus" RRRAH_GPU_BACKEND=metal cargo test -p rrrah --bin rrrah real_ai_cancel_after_render_scratch_admission --locked -- --ignored --nocapture
```

The observer requires render-stage live credit before cancellation; it is not a test of an arbitrary page-completion percentage. [Measured evidence](research/real-ai-render-scratch-cancel-metal-recovery-2026-10-11.json).

2026-10-11 current-code refresh: the owned live HDR surface probe presented 178 frames over 3.003 seconds with no validation errors; OS-reported EDR headroom rose from1 to16 and returned to1 after window exit. This refreshes EDR activation evidence, not physical luminance/color calibration. [Measured evidence](research/metal-live-hdr-surface-refresh-2026-10-11.json).


### Integrated mandatory Metal gate, 2026-10-11

`qualify-viewer-storage-gpu.py` now requires thumbnail upload completion under pressure and supported texture extents in the default full Metal run. With `--ai-native-root`, it also requires production AI shared-cache hits, queued page reversal and cancellation after render scratch admission, with typed cancellation/retry and exact SDR/HDR transport. Each added case requires one actual passing test, zero ignored tests and final workload markers. Texture extents are validated against the adapter-reported limit.

Reproduction:

```sh
python3 scripts/qualify-viewer-storage-gpu.py --target-dir target --output-dir /tmp/rrrah-metal-fresh-results --gpu-backend metal --ai-native-root tests/fixtures/data/metal-recovery
```

Current run: 18 suites, 508 passing executions, 76 ignored tests; every executed suite verified. Log hashes were rechecked. The final generalized texture validator passed the real log and rejected four malformed/missing/duplicate evidence variants. Evidence: `research/metal-integrated-final-2026-10-11.json`.

This gate qualifies the exercised Metal/storage paths. It does not qualify all 100 formats, AI appearance fidelity, physical HDR luminance/color calibration or NVIDIA hardware. The latter remains deferred at user request.


### 2026-10-11: native declared Radiance BT.2020/D65

Radiance RGBE now recognizes explicit BT.2020/D65 primaries and prepares scene-linear RGB through the shared native BT.2020-to-sRGB matrix. The source float samples are retained; output preserves values above one and negative gamut components. Conversion uses managed output admission and cancellation; unfamiliar primaries/white points stay unqualified. CICP recognition is restricted to primaries 9, linear transfer 8, identity matrix 0 and full range.

A separate Gaussian inversion of published BT.2020/BT.709 xy/D65 coordinates confirms the matrix. Authored 1×1 RGBE red=2 exercises decode, bounded output refusal/retry, native color preservation through disk swap, pressure/cancellation recovery and actual Metal SDR/HDR frame equality against the analytical reference, with root credit returning to zero. Core: 109 passed / 2 ignored; HDR decoder: 4 passed / 0 ignored; focused Metal: 1 passed / 0 ignored; full HDR regression: 15 passed / 3 ignored. The focused GPU case is included in the existing ordinary `hdr_raster_swap` suite and therefore in the default integrated gate.

Evidence: `research/radiance-bt2020-matrix-oracle-2026-10-11.json` and its preserved hashed logs. This authored fixture does not qualify arbitrary Radiance primaries, exposure/color-correction semantics, broad producer/orientation coverage or physical HDR calibration.


### 2026-10-11: declared Radiance Display P3/D65

Extended the native linear-primary matrix path to CICP 12/8/0/full and explicit Radiance Display P3/D65. RGBE source samples remain unchanged; prepared floats preserve above-one and negative gamut components. A four-pixel RGBW HDR fixture covers every matrix column and white neutrality using a separate Gaussian inversion of H.273 primary coordinates. Actual Metal SDR/HDR frames match the analytical reference before and after source swap, memory refusal/cancel/retry; managed root credit returns to zero. The shared integration helper also strengthens the earlier BT.2020 case with preparation-pressure/cancellation and direct per-sample analytical checks.

Core tests: 109 passed / 2 ignored. Entire HDR Metal regression: 16 passed / 3 ignored, no failures. Both declared-primary tests are ordinary tests included by the default integrated runner. Evidence and preserved hashed logs: `research/radiance-p3-matrix-oracle-2026-10-11.json`. Arbitrary primaries/non-D65 whites, exposure semantics, broad original corpus and physical HDR calibration remain unqualified.


### 2026-10-11: explicit OpenEXR BT.2020/P3 color declarations

Native color declaration parsing now accepts linear BT.2020/D65 and P3/D65 via explicit chromaticities or standard `lin_rec2020_scene` / `lin_p3d65_scene` IDs. Conflicting known IDs/chromaticities and incompatible ACES flags are refused. Unknown IDs override otherwise familiar chromaticities; non-D65 adopted neutral remains unqualified.

14 authored metadata variants retain independently encoded EXR channel chunks byte-for-byte, with relocated scanline offsets. Generation is deterministic; all 16 fixture/oracle pins verified. Six accepted metadata cases preserve analytical samples and SDR/HDR frames on actual Metal before/after swap and preparation/storage memory refusal, cancellation and retry; managed root credit returns to zero. Four conflicting and four unknown/neutral cases exercise refusal policies. Full HDR regression: 17 passed / 3 ignored. Final focused current-source test: 1 passed / 0 ignored. Existing integrated gate includes the new ordinary test. Evidence: `research/exr-wide-color-metal-2026-10-11.json` and preserved hashed logs. Other EXR primaries/white points, layers/deep/tiled, zero-alpha emission rendering and physical HDR calibration remain open.


### 2026-10-11: general native linear-primary matrix constructor

Added public `linear_rgb_to_srgb_from_xy([f64; 8])`, using stack-only XYZ basis inversion and existing Bradford adaptation for a different source white, without transfer/tone mapping or clipping. Unnormalized primary XYZ supports imaginary AP0/AP1 and XYZ-axis y=0 primaries; physical white and nonsingular finite bases are required. Existing linear BT.2020/P3 display preparation now uses this constructor. Current core tests: 112 passed / 2 ignored; entire HDR actual-Metal regression: 17 passed / 3 ignored. Evidence: `research/linear-primary-matrix-2026-10-11.json`.

This establishes native matrix calculation, not arbitrary-source viewer support. Arbitrary per-frame chromaticity metadata, versioned swap serialization and decoder routing remain to implement. Non-D65 independent matrix and arbitrary-source pixel/frame qualification also remain open; neutral-only AP0/AP1 checks are not full color qualification.


### 2026-10-11: arbitrary linear chromaticity frame metadata and swap V3

Added validated `RgbChromaticities` and `RasterColorSpace::LinearRgb`: eight exact f64 xy coordinates held inline as fixed little-endian bytes, with exact Eq/Hash and native linear-sRGB preparation through the general matrix constructor. Raster payload V3 stores a 64-byte coordinate block; metadata reading uses stack storage, with no separate heap/scratch admission. Existing V1/V2 payloads remain readable. New tag/version, metadata length and invalid basis/nonfinite combinations are refused.

Exact full-frame metadata/sample/aspect round trip succeeds with pixel-only output budget. Segmented swapped-region coverage now exercises both CICP and explicit chromaticities, with aspect, scale, selection, hotspot and every sample bit retained after full-source release. Direct explicit-xy P3 frames also pass analytical preparation and actual Metal SDR/HDR equality before/after preparation/storage pressure, cancellation and retry, with root credit zero. Current core: 112 passed / 2 ignored; full cache crate: 147 passed / 5 ignored; full HDR Metal: 18 passed / 3 ignored. Evidence: `research/linear-chromaticity-metadata-2026-10-11.json`.

File decoder routing for arbitrary source primaries and non-D65 independent full-color/frame qualification remain open. This milestone qualifies frame metadata and its storage/render route, not arbitrary EXR/Radiance originals or physical HDR calibration.


### 2026-10-11: Radiance arbitrary explicit primary/white decoder routing

RGBE `PRIMARIES` now routes valid invertible declared bases and physical source white through `LinearRgb(RgbChromaticities)`; known Rec.709/BT.2020/P3 compact tags remain supported. Imaginary AP1/AP0 primaries are accepted by the native mathematical basis validator. Singular/nonfinite/invalid-white/duplicate declarations are refused; absent primaries stay unqualified and XYZE is never relabeled RGB.

Three authored non-D65 RGBW files (custom white, AP1, AP0) preserve every native source sample and exact color coordinates through preparation/storage full pressure, cancellation, retry and disk swap. Independent partial-pivot Gaussian XYZ/Bradford oracles cover every matrix column and neutral; double-precision and f32-cone precision variants bound approximation loss. Prepared samples and complete actual Metal SDR/HDR frames match analytical references, with root credit zero. Full HDR Metal suite: 20 passed / 3 ignored, no failures. Evidence: `research/radiance-arbitrary-color-metal-2026-10-11.json`, per-case matrix reports and preserved hashed log. Reusable oracle: `scripts/probe-linear-color-oracle.py`.

This qualifies the authored explicit-primary route, not all producers, missing declarations, exposure/COLORCORR calibration, scan orders, XYZE or physical HDR display calibration. Arbitrary EXR declaration routing is still pending.


### 2026-10-11: arbitrary OpenEXR chromaticities and ACES linear IDs

Single-part RGB EXR now routes valid arbitrary chromaticities/physical source white to native exact chromaticity metadata, including imaginary AP1/AP0 primary bases. Added standard `lin_ap1_scene` and `lin_ap0_scene` ID routing. Known IDs are authoritative when accompanying chromaticities agree; conflicts remain explicit errors. Matching adopted neutral is supported for any qualified source white; a different adopted neutral remains unqualified. Compatible AP0 ACES flags are accepted for color interpretation; non-AP0 conflicts are refused. Unknown IDs suppress chroma fallback. A standalone ACES flag without explicit chromaticities/ID is not qualified, and accepting the color flag does not certify SMPTE container layout.

26 metadata derivatives retain independently encoded EXR channel chunks unchanged; deterministic generation and all 38 file pins verified. Twelve accepted cases match independent Gaussian XYZ/Bradford analytical samples and full Metal SDR/HDR frames before/after managed preparation/swap pressure, cancellation and retry. Every accepted source retains original sample bits and coordinate metadata; tracked credit is zero after each accepted/refused/unknown case. Eight conflicting cases are refused; six unknown/neutral cases remain unqualified. Full HDR regression: 21 passed / 3 ignored. Final focused test: 1 passed / 0 ignored. Evidence: `research/exr-arbitrary-color-metal-2026-10-11.json`, preserved hashed logs and `tests/fixtures/exr-arbitrary-color.json`.

Missing or unknown declarations, different adopted neutral, broader producer/layer/deep/tile/associated-emission qualification and physical HDR calibration remain open.


### 2026-10-11: native independent adopted-neutral matrix calculation

Added `linear_rgb_to_srgb_with_adopted_neutral_from_xy`, preserving the declared RGB/XYZ basis while Bradford-adapting the independently declared adopted neutral. The existing helper delegates with the original white to preserve prior behavior. Two independent Gaussian oracles (Rec.709 and AP1 with different adopted neutral) verify all matrix coefficients and neutral mapping; tests distinguish incorrect source-basis re-normalization and reject invalid adopted neutral. The oracle generator now supports `--neutral`, including the correct transformed source-white RGBW reference.

Core tests: 114 passed / 2 ignored. Existing entire HDR Metal regression: 21 passed / 3 ignored. Evidence: `research/adopted-neutral-native-matrix-2026-10-11.json` and preserved hashed logs. This establishes matrix computation only. Frame/swap adopted-neutral metadata and distinct-neutral EXR source routing remain to implement; no actual distinct-neutral source is yet qualified on Metal.


### 2026-10-11: adopted-neutral frame metadata and swap V4

`RgbChromaticities` now retains optional adopted-neutral xy separately from the original RGB basis. The legacy coordinate API remains 64 bytes; ordinary metadata stays V3. Explicit adopted-neutral metadata uses an inline 80-byte block in V4, read on stack without separate metadata heap/scratch admission. V1/V2/V3 remain readable. Full and segmented restores retain adopted neutral, aspect, scale, selection, hotspot and sample bits. Corrupt version/length/tag or invalid adopted neutral is refused without leaked admission.

V4 round trips succeed with pixel-only output budget. Native Rec.709 and AP1 RGBW frames with different adopted neutral match independent Gaussian/Bradford analytical samples and exact Metal SDR/HDR frames before/after preparation/storage memory refusal, cancellation and retry, with root zero. Full cache crate: 148 passed / 5 ignored. Core library: 114 passed / 2 ignored, plus 3 core integration passes. Entire HDR Metal: 22 passed / 3 ignored. Evidence: `research/adopted-neutral-metadata-2026-10-11.json` and preserved hashed logs.

Distinct-neutral EXR decoder routing remains to connect. This qualifies typed frame/storage/render metadata; no distinct-neutral EXR original is qualified yet, and physical HDR calibration remains open.


### 2026-10-11: distinct adopted neutral in EXR on Metal

EXR decoding now retains a valid distinct `adoptedNeutral` independently of the original RGB basis, through typed frame preparation and V4 swap. This supersedes earlier notes that distinct adopted neutral remains unconnected. Invalid physical neutral coordinates are refused; unknown color IDs still suppress fallback. Known IDs use canonical basis coordinates.

On Apple M4 Max Metal, the full HDR suite passed 22 tests (3 ignored). Arbitrary EXR variants: 17 accepted, 11 refused, 6 unknown; wide-gamut variants: 10 qualified, 4 refused, 2 unknown. Accepted cases preserve native sample bits and metadata, match independent analytical colors and exact SDR/HDR frames before and after swap, and pass memory refusal/cancellation/retry with tracked root credit zero. All 73 fixture pins and deterministic regeneration verified. Evidence: `research/exr-adopted-neutral-metal-2026-10-11.json`.

These are authored metadata variants of independently encoded samples. Broader EXR producer/layer/deep/tile qualification and physical HDR calibration remain open. NVIDIA hardware is deferred at the user's request.


### 2026-10-11: integrated Metal after EXR adopted-neutral routing

Current integrated driver completed 29 suites: 530 passed, zero failed, 76 ignored. All suite execution checks and preserved log SHA256 values verified. In addition to default cache/viewer/GPU/AI transport gates, this run requires mixed-format prefetch navigation, all five linear import paths, 12MP float HDR, viewer TTL/prefetch, large upload cancellation, compute RAM/swap/Metal and two model swap cycles. Mandatory thumbnail completion and supported texture extent checks passed.

Evidence: `research/metal-integrated-exr-neutral-2026-10-11.json`. Ignored hardware/corpus/manual tests remain explicitly skipped; this does not prove the complete 100-format goal or physical HDR calibration. CUDA host checks do not qualify NVIDIA hardware, which remains deferred.


### 2026-10-11: sustained full-frame Metal HDR and storage revalidation

Eight 4000x3000 float sources (192 MB each) completed a 60-second actual Metal collection loop. All 83 full SDR frames and 83 full RGBA16Float HDR frames matched references, including 83 mid-batch cancellations. Final tracked root and queue credit are zero. The cache sustained suite passed all eight tests with no ignores, including 100000 distinct raster keys, three independent 30-second delayed-I/O modes, byte/count eviction, physical TTL deletion, overload and cancellation/retry.

Evidence: `research/metal-sustained-full-hdr-current-2026-10-11.json` and preserved hashed logs. Managed pressure and delayed I/O do not qualify OS-wide pressure or a physical slow disk. Physical HDR calibration and NVIDIA hardware remain open.


### 2026-10-11: mandatory Metal linear color evidence

The integrated HDR suite now captures output and requires four named successful color tests plus their workload markers and actual Metal adapter evidence. Missing any of four markers, marking any required test ignored, or replacing Metal with Vulkan is rejected. Current actual HDR/TTL run passes (24 passed, 8 ignored). Evidence: `research/metal-linear-color-required-gate-2026-10-11.json`. This hardens execution evidence; broader format and physical HDR qualification remain open.


### 2026-10-11: current RAW reversal on Metal

Actual EOS R8 worker reversal passed with 25494560 native samples exact and complete 6188x4120 Metal readback, plus 1920x1080 output. Cancelled forward work stays absent; backward work is warmed; forward/backward subsequent restores preserve all samples/frames with disk count one and final tracked root zero. The optional integrated worker scenario now requires these workload markers; deleting any required marker or marking execution ignored is rejected. Evidence: `research/raw-reversal-metal-current-2026-10-11.json`. Two pinned EOS R8 sources do not qualify every camera or the full format goal.


### 2026-10-11: full-frame multi-neighbor RAW Metal

Expanded the behind=1/ahead=2 actual worker scenario from 1920x1080-only readback to additional full sensor-sized Metal readback. Six neighbor restores across forward/backward order preserve all original samples, metadata and full frames for ORF/PEF/NEF/CR2 sources. Selected source is not speculatively decoded, disk count remains three, failures and final tracked credits are zero. The integrated optional scenario now requires six full-frame results; removing any result is rejected. Focused test passed, no ignores. Evidence: `research/raw-neighbors-full-metal-2026-10-11.json`. This qualifies worker/storage/GPU transport for these four files, not independent camera-color fidelity.


### 2026-10-11: nine original RAW worker sources on Metal

The actual worker test passed for all nine pinned manifest sources: CR2/NEF/ARW/ORF/PEF/RW2/RAF and two EOS R8 CR3 files. Every source preserves all sensor samples, metadata and full-sized Metal output through prefetch/disk restore. Alternating directions and one-entry disk eviction pass, with previous source absent and tracked root zero. All nine distinct source paths and SHA256 pins recorded in `research/nine-raw-worker-metal-2026-10-11.json`. This establishes native storage/render transport against direct native decoding, not independent camera-color fidelity.


### 2026-10-11: sustained nine-source RAW RAM/swap/Metal collection

Current real-camera collection completed 85 alternating traversal rounds, 765 disk restores over 60.458 seconds, with nine initial writes. Every restore compares all native samples/metadata and 1920x1080 Metal output, including memory refusal, cancellation/retry, visible-frame pinning and one-entry RAM/spill policy. Final tracked root/queue credit and swap errors are zero. Focused test passed without ignores. Evidence: `research/nine-raw-collection-current-2026-10-11.json`. Full-sized per-source transport was qualified separately; this sustained loop uses 1920x1080 output. Physical slow disk, independent camera color and physical HDR remain open.


### 2026-10-11: bounded RAW viewing latency measurement

The sustained nine-source collection now measures swap restore and GPU upload/render/readback separately, excluding reference comparison and refusal/cancellation controls. Bookkeeping is bounded to the last 4096 timings per stage. On Apple M4 Max, 450 restores across 50 rounds passed with zero errors: restore p50=41.346 ms, p95=70.207 ms, max=142.440 ms; Metal 1920x1080 upload/render/readback p50=24.916 ms, p95=42.225 ms, max=91.302 ms. Source values/output remain exact, tracked root/queue zero. Evidence: `research/raw-collection-latency-2026-10-11.json`. These are warm local filesystem restores and offscreen GPU readback, not first RAW decode, physical display latency or universal performance guarantees.


### 2026-10-11: initial native RAW latency baseline

Nine-source test now records initial native mosaic decode separately from GPU 1920x1080 upload/render/readback. Single-run decode ranges 105-1840 ms: ORF 1839.866 ms, CR2 1581.452 ms, PEF 1276.728 ms, NEF 662.066 ms, RW2 255.952 ms, ARW 153.359 ms, RAF 139.392 ms, CR3 105.495/111.345 ms. Regression passed including 90 swap restores and exact samples/output. Evidence: `research/raw-initial-latency-2026-10-11.json`. These are per-file baselines with no forced cold filesystem state, not format-wide guarantees; expensive native decode paths are candidates for profiling.


### 2026-10-11: native Olympus entropy extraction optimization

Replaced per-bit residual reads with bounded covering-byte extraction, retaining checked truncation and exact bit order. Differential tests compare all alignments and widths 0..16 against the original bitwise definition, including truncated tails and unchanged cursor on refusal. Three entropy tests and twelve ORF tests pass. Two actual nine-source Metal regressions (90 restores each) pass. The pinned 1084.orf decode measured 659.654/630.439 ms after, versus prior single-run baseline 1839.866 ms. Evidence: `research/orf-native-bit-window-2026-10-11.json`. This is a per-file performance improvement with non-interleaved baseline, not a universal format speed guarantee or GPU entropy decoder.


### 2026-10-11: ORF performance claim correction

Supersedes the earlier 2.8x ORF speedup claim: unrelated CR2 decode also improved substantially without CR2 code changes, so non-interleaved complete-decoder timings are confounded. Production ORF bit reader is restored to its original checked algorithm; byte-window candidate remains test-only. A corrected same-process checked-reader microbenchmark alternates order for six rounds with identical checksums (reference median 22.030 ms, candidate median 20.270 ms), a modest synthetic result that does not establish complete original-file decode improvement. Four entropy tests pass; restored production nine-source Metal/90-restore regression passes. Evidence: `research/orf-performance-correction-2026-10-11.json`. Initial unchecked-reference microbenchmark is superseded. Full decoder paired A/B profiling remains necessary before promoting the candidate.


### 2026-10-11: paired complete original ORF entropy reconstruction

Six alternating same-process rounds reconstruct the entire pinned Olympus entropy strip with both readers and compare every one of 16110080 sensor values after each round. All values match. Median original reader 530.482 ms; byte-window candidate 567.190 ms. Candidate is rejected for production; original reader retained. Evidence: `research/orf-paired-real-2026-10-11.json`. Unlike the synthetic microbenchmark, this tests complete real entropy reconstruction; TIFF/container parsing, allocation and GPU are outside timing.


### 2026-10-11: validated inline short-window ORF production path

A revised byte-window candidate adds same-byte extraction and inline call integration. Six alternating same-process complete original entropy reconstructions improve in every pair; median original 517.072 ms, candidate 420.772 ms (18.6% lower time). All 16110080 values match after each round. This revised candidate is enabled in production; checked original reader remains available to test specialization. Three entropy tests, twelve ORF format tests and actual nine-source Metal/90-restore regression pass. Evidence: `research/orf-short-inline-production-2026-10-11.json`. This is one-file entropy reconstruction, not an end-to-end or format-wide guarantee; the earlier 2.8x claim remains withdrawn.


### 2026-10-11: full native decoder regression after ORF optimization

Full `rrrah-decode` ordinary test command passes: 1001 passed, zero failed, 77 ignored. Preserved report enumerates all ignored tests instead of treating unavailable external corpora/manual benchmarks as coverage. The two ignored Olympus tests are manual benchmarks, exercised separately in preceding qualification; no additional independent original ORF oracle test is present in this suite. Evidence: `research/decode-full-after-orf-2026-10-11.json`. This verifies regression coverage, not complete 100-format or camera-color qualification.


### 2026-10-11: independent original ORF sensor validation

The optimized public NativeRawDecoder with 128 MiB managed budget matches independent LibRaw 0.22.2 unprocessed_raw for all 16110080 samples of pinned original 1084.orf (4640x3472), zero mismatches. Oracle output is unprocessed 16-bit PGM without gamma/autoscale. Production remains native Rust; LibRaw is test-only. Source/native/oracle SHA256 pins are preserved in `research/orf-independent-sensor-2026-10-11.json`. This qualifies that original sensor plane, not WB/color/HDR or all Olympus cameras.


### 2026-10-11: independent sensor planes for nine RAW originals

Public native Rust decoding under a 128 MiB managed output budget matches independent LibRaw 0.22.2 unprocessed_raw exactly for all 187104320 samples across nine pinned originals: CR2/NEF/ARW/ORF/PEF/RW2/RAF and two CR3. Zero mismatches; sample counts and source/oracle/native SHA256 pins recorded in `research/nine-raw-independent-sensors-2026-10-11.json`. This independently validates the sensor planes subsequently exercised by worker/RAM/swap/Metal tests. LibRaw remains test-only. Nine-file sensor qualification does not establish every camera, independent WB/color/orientation or physical HDR.


### 2026-10-11: independent RAW WB and level audit

Normalized RGB camera WB matches independent LibRaw for all nine originals within 2e-6. Sensor equality alone does not prove full development: saturation differs for ORF (native4095 vs oracle4065) and RW2 (3967 vs3989); black level differences in CR2/RAF/CR3 require coordinate/channel and calibration audit. PEF cblack channel order must not be compared directly to native coordinate black grid. Evidence: `research/nine-raw-metadata-audit-2026-10-11.json`. These discrepancies remain explicit open color qualification items; no oracle values blindly copied into production.


### 2026-10-11: RAW level provenance audit

Direct original CR2 TIFF/EXIF/Canon MakerNote inspection confirms ColorData v13 (1592 words), black values at word778 =512,512,511,511, exactly matching native stored-level extraction. Independent LibRaw level state changes from all-zero black before unpack to black511+cblack[0,0,1,1] after unpack. These are different stages of processing; equality of stored data must not be conflated with post-unpack calibration. Six original level-stage observations preserved in `research/raw-level-provenance-2026-10-11.json`. Canon coordinate mapping/calibration, ORF/RW2 saturation policy and RAF black policy remain unresolved. No production values changed solely to match the oracle.


### 2026-10-11: Canon stored black channel mapping resolved for original CR2

Independent Canon parsing evidence reads four sequential black words in RGGB order (LibRaw converts via RGGB_2_RGBG to its channel array). Pinned 1294.cr2 has native raw RGGB CFA, so stored words512,512,511,511 map directly to native 2x2 coordinate grid. No channel permutation bug is demonstrated for this original. ExifTool independently identifies ColorData v13 black offset0x30a. Evidence: `research/canon-black-order-2026-10-11.json`. Post-unpack calibrated black policy and other camera/CFA phases remain open.


### 2026-10-11: conflicting independent saturation calibration

LibRaw model table uses E-M5 white4065 / GH2 white3989, while RawSpeed camera database uses4095 /3800 (E-M5 black255). Native GH2 reads explicit file RGB linearity limits3967; native E-M5 falls back to12-bit full scale4095. Thus third-party equality alone is not a valid saturation oracle. Preserved source hashes and active-crop parity minima/maxima/above-white counts: `research/raw-saturation-provenance-2026-10-11.json`. Existing file values retained; physical saturation requires controlled exposure or independent manufacturer calibration and remains unqualified.


### 2026-10-11: RAW values above file white retained on Metal

Extended scene-linear RAW attachment test with authored encoded3989/black15/white3967 field (GH2 original file levels), identity color transform and +1 stop. Independent normalized expected RGB exceeds2 and matches both Rgba16Float/Rgba32Float within separately defined attachment precision. SDR output remains bounded; switching back to HDR reproduces exact prior output; managed GPU budget releases. Actual Metal two-test suite passes. Evidence: `research/raw-above-white-metal-2026-10-11.json`. This qualifies above-white headroom arithmetic/transport, not photographic GH2 color or physical display calibration.


### 2026-10-11: invalid explicit ORF white levels refused

Fixed ORF metadata reader accepting zero/out-of-depth explicit white and silently taking first element of an invalid multi-value declaration. Explicit WhiteLevel now requires exactly one finite positive integer within the sensor bit depth. Missing declaration retains full-scale fallback. Added empty/zero/4096-for12-bit/multiple-value refusal regressions. Thirteen ORF tests and actual nine-source Metal collection with90 restores pass. Evidence: `research/orf-white-validation-2026-10-11.json`. Metadata validity is separate from physical camera saturation calibration, which remains open.


### 2026-10-11: RW2 explicit white bit-depth validation

Explicit RGB linearity limits now reject values above the sensor bit depth (new4096-for12-bit regression), in addition to existing zero/partial limits refusal. Focused RW2 suite passes. Public native original GH2 decode under128MiB remains bit-identical for all16526720 samples to prior independently qualified sensor dump, with file RGB white3967 retained. Evidence: `research/rw2-white-validation-2026-10-11.json`. Physical saturation calibration remains separate.


### 2026-10-11: current cross-format raster collection on Metal

Actual current source passes all65 raster sources,94 selections,188 native/prepared storage cases with exact pixel bits/color metadata and SDR/HDR frames, bounds refusal, memory refusal/cancellation/retry and zero tracked root/queue at completion. Independent bright/negative constant HDR control passes. Strict workload validator passes and missing completion marker is rejected; all65 source hashes pinned. Evidence: `research/raster-collection-current-2026-10-11.json`. This is transport against direct decode, not independent fidelity of every format or100-format completion.


### 2026-10-11: nine independent camera matrix declarations and Metal transforms

All nine original RAW native XYZ-to-camera matrices match independent LibRaw coefficients within6e-8 absolute error. Actual camera-color Metal suite passes12 tests without ignores, covering independent transforms, authored fields and cropped/minified/oriented/tiled paths. Evidence: `research/nine-raw-camera-matrices-metal-2026-10-11.json`. GPU cases are authored fields, not photographic per-original appearance or physical calibration. The six Pending format implementations remain Pending.


### 2026-10-11: ORF black metadata and positive normalization range

ORF scalar black parsing now rejects empty/multiple declarations and nonfinite/negative offsets instead of silently taking first value. Metadata construction requires black below white, refusing collapsed/inverted normalization ranges. Regression covers empty/multiple/equal/greater cases and valid nonzero offset. Fourteen ORF tests pass; public original sensor decode under128MiB retains all16110080 samples exactly against the previously independent-qualified dump. Evidence: `research/orf-black-validation-2026-10-11.json`. Per-coordinate black metadata and physical calibration remain separate work.


### 2026-10-11: declared ORF black repeat grid

ORF metadata now supports explicit BlackLevelRepeatDim 1x1/2x2 and retains row-major sensor-coordinate offsets. Distinct authored10/20/30/40 values roundtrip metadata exactly; empty/malformed/unsupported dimension vectors and mismatched value counts are refused. Missing black uses zero grid of declared shape; all values remain checked below white. Fifteen ORF tests pass. Evidence: `research/orf-black-grid-2026-10-11.json`. No original ORF with nonuniform explicit repeat grid or new original-file Metal qualification is claimed.


### 2026-10-11: combined current Metal after native RAW changes

Integrated driver completes 34 suites, 535 passed, zero failed, 76 ignored. Every suite execution validator and preserved log hash verified. Includes raster65/94/188 collection, nine-source RAW collection and full-frame worker, multi-neighbor full-frame restore, direction reversal, linear imports, TTL/preload,12MP HDR, upload cancellation, compute cache, model swap and AI/thumbnail/texture-extent checks. Evidence: `research/metal-integrated-raw-levels-2026-10-11.json`. CUDA checks are host-only; NVIDIA remains deferred. Ignored manual/external cases, six pending formats, complete fidelity and physical HDR calibration remain open.


### 2026-10-11: invalid RAW range preserves existing Metal frame

Extended actual Metal scene-linear test with16 malformed uploads: equal/inverted black-white range independently at each CFA phase for Rgba16Float/Rgba32Float. Every upload refuses with InvalidLevels, previous valid HDR readback remains exact, managed credit stays unchanged, final renderer drop releases admission. Two-test suite passes. Evidence: `research/invalid-raw-levels-metal-2026-10-11.json`. This validates failure atomicity of existing upload guards, not physical camera/display calibration.

### Exact black-grid admission (2026-10-11)

Removed silent top-left 2x2 approximation for unusual black-level repeat grids. Exact larger periodic grids remain accepted; nonrepresentable grids are refused, including odd-size wrap cases. Core: 115 passed, 2 ignored. Real Metal RAW linear readback: 2 passed. Arbitrary repeat-grid shader support remains open. Evidence: `research/black-grid-metal-2026-10-11.json`.

### Full Bayer viewport black repeat grid (2026-10-11)

The Bayer Metal viewport now consumes the full single-component black-level repeat grid in a read-only storage buffer. This supersedes the viewport refusal recorded above; `bayer_quad` remains strict for consumers requiring a quad. Exactly quad-representable grids reuse the uniform allocation. Grid bytes join the atlas reservation and resource lease; failed admission preserves the prior frame. Independent authored 3x5 neutral-field check passes on Rgba16Float and Rgba32Float, including memory pressure, invalid final cell, and return to fast-path credit release. GPU suites: 18 passed; library: 52 passed. RGBE calibration and compute arbitrary-grid qualification remain open. Evidence: `research/repeat-grid-metal-2026-10-11.json`.

### Fractional RAW level normalization (2026-10-11)

Metal Bayer viewport no longer rounds a positive white-minus-black range up to one. Independent encoded=1, black=.75, white=1.25, exposure=1 fixture yields RGB=1 on both float attachments. Non-finite subtraction is refused before replacing the visible frame or its reservation. RAW linear and camera-color suites passed. Evidence: `research/fractional-levels-metal-2026-10-11.json`; physical calibration remains unqualified.

### RGBE full-grid calibration (2026-10-11)

RGBE calibration now resolves per-site black levels over the full repeat grid and validates the joint grid/CFA period. Quad-equivalent grids retain allocation-free calibration; unusual grids own bounded metadata copies. A 3x5 authored grid passes 144-site four-gain normalization and Metal signed HDR viewport checks on both float targets. Core RGBE: 13 passed; GPU RAW/RGBE suites: 7 passed, 1 ignored. Dedicated arbitrary-grid normalization-to-compute integration and genuine camera appearance remain unqualified. Evidence: `research/rgbe-repeat-grid-2026-10-11.json`.

### Full-grid RGBE normalization-to-compute integration (2026-10-11)

Managed calibration now reserves the unusual-grid payload and shares its storage/reservation across calibration clones. Metal test normalizes a 3x5 grid over a 17x33 sensor, checks independent four-plane values at all 561 sites and across multiple compute dispatches, and verifies final CPU/GPU credit zero. Core: 13 passed; GPU compute/linear: 6 passed, 1 ignored. Very small binding limits that cannot fit an aligned row chunk remain explicitly refused (17-wide/1024-byte case); physical HDR and original unusual-grid camera appearance are not qualified. Evidence: `research/rgbe-grid-compute-2026-10-11.json`.

### Integrated refresh after RAW grid changes (2026-10-11)

34 integrated suites: 535 passed, 0 failed, 76 ignored; all execution validators and 34 log hashes checked. Separate expanded GPU refresh includes new RAW/RGBE grid and fractional-level tests with actual Metal markers. These tests are now in the default qualification driver GPU suite. Evidence: `research/metal-integrated-repeat-grid-2026-10-11.json`. This regression does not close original format fidelity, physical HDR/NVIDIA, genuine slow-disk or OS-pressure requirements.

### 100k distinct-key storage refresh (2026-10-11)

100000 distinct tiny raster writes and exact restores passed in 26.045 seconds, including 1000 full-root pressure refusals/retries. Logical disk quota and physical files stayed within 17 objects/4096 bytes; evicted keys absent, final root/queue credit zero. Small payloads isolate index/key churn; this is not large-image throughput or OS-pressure qualification. Evidence: `research/100k-swap-current-2026-10-11.json`.

### CPU/Metal level-range refusal consistency (2026-10-11)

CPU Bayer development now rejects an overflowing white-minus-black subtraction before normalization, matching Metal viewport range validation. Authored regression verifies typed refusal and complete managed scratch/output rollback. Core: 118 passed, 2 ignored. Evidence: `research/cpu-level-overflow-2026-10-11.json`.

### Ten-minute sustained storage pressure (2026-10-11)

Three delayed-local-I/O scenarios ran concurrently for 600 seconds each: independent byte quota, independent count quota, combined limits. All passed; 13388 writes and exact restores, each with full managed-root refusal/retry. Physical and logical quotas checked every cycle; final root/queue credit zero and swap errors zero. Sampled process RSS max 5824 KiB over 16 samples (not an exact peak). Evidence: `research/ten-minute-swap-2026-10-11.json`. This qualifies simulated slow I/O and managed pressure, not a physical slow disk, OS-wide pressure, or simultaneous ten-minute GPU viewing.

### Ten-minute real RAW Metal collection (2026-10-11)

Nine original files completed 957 alternating traversals in 600.074 seconds: 8613 full-sensor restores and exact 1920x1080 offscreen Metal frames. Per-cycle managed pressure refusal, cancellation, retry, visible pin, spill, root/queue-zero, and zero swap-error assertions passed. Source hashes rechecked after run. Last4096 latency: restore p50=41.462/p95=71.050 ms; GPU upload/render/readback p50=25.043/p95=50.197 ms. Warm local filesystem, fresh renderer per frame, SDR offscreen target; not physical HDR, persistent-renderer soak, OS-wide pressure, or independent appearance qualification. Evidence: `research/ten-minute-raw-metal-2026-10-11.json`.

### Persistent RAW renderer accounting regression (2026-10-11)

Collection test now reuses one RAW renderer and applies separate resident/upload/queued GPU payload budgets. Thirty-second nine-original regression: 495 exact restores/frames; resident peak 146800640 bytes, packing peak 2097152, queued peak 73400320. Active credit equals current atlas after each readback; upload/queue zero each frame, all GPU roots zero after drop. Driver requires final residency evidence and rejects missing/nonzero-final markers. A new 600-second persistent run is in progress; no long-run result claimed here. Evidence: `research/persistent-raw-metal-2026-10-11.json`.

### Ten-minute persistent RAW renderer completion (2026-10-11)

Nine-original collection completed 926 alternating traversals and 8334 exact sensor restores/Metal frames in 600.006 seconds using a single persistent renderer. Resident/packing/queued GPU payload peaks: 140/2/70 MiB. Active credit equals current atlas after readback, temporary credit zero each frame, all GPU/storage roots zero at final drop. Driver workload validator and original input hashes reverified. Last4096 restore p50/p95: 41.830/68.318 ms; GPU upload/render/readback: 22.928/41.585 ms. Warm local SDR offscreen transport; not physical HDR, OS pressure, driver-allocation accounting, or independent appearance qualification. Evidence: `research/ten-minute-persistent-raw-metal-2026-10-11.json`.

### Real RAW GPU replacement pressure atomicity (2026-10-11)

Nine-original persistent-renderer regression completed 441 exact replacements over 30.188 seconds, injecting GPU resident-budget exhaustion before all 440 replacements with an existing frame. Every refusal retained previous frame pixels and resident credit; retries exact, final GPU/storage roots zero. Driver now requires refusal count=restores-1 and preservation/retry marker. Reported 512 MiB budget peak includes artificial pressure reservation, not atlas footprint. Evidence: `research/real-raw-gpu-pressure-2026-10-11.json`. Physical VRAM exhaustion and HDR remain separate unqualified requirements.

### Partial real RAW GPU upload cancellation (2026-10-11)

Nine-original reuse regression completed 306 exact restores/new frames over 30.482 seconds, with 305 GPU pressure refusals and 305 partial upload cancellations. Forced 512-sample tiles, halo2, check64 crosses tile writes before cancellation in later packing under the current checked loop. Previous frame and resident atlas preserved; scratch credit zero on cancellation, cancelled atlas/queue released after GPU completion, retries exact, all final roots zero. Driver validates exact cancellation counts and release/retry evidence. Progress stage inferred from source loop rather than hardware tracing; submitted work completes normally. Evidence: `research/real-raw-partial-cancel-2026-10-11.json`.

### Native RAW live HDR probe extension (2026-10-11)

Live HDR example now accepts `RRRAH_HDR_RAW_SOURCE`, decodes via native managed RAW path, uploads to scene-linear RawRenderer, and verifies source-credit release. Redraw retries now honor their deadline instead of spinning on about_to_wait. Build passed. Current original 6188x4120 CR3 setup/configuration succeeded without validation errors, but live surface returned Occluded after retries; zero presented frames. The raster control also returned Occluded. No current live RAW HDR presentation success or physical calibration claimed. Evidence: `research/live-raw-hdr-occluded-2026-10-11.json`.

### Live HDR occlusion cause verified (2026-10-11)

Read-only macOS Root registry reports `CGSSessionScreenIsLocked=true`, explaining the current live-probe Occluded result. Local winit documentation confirms activation already enabled by default; no activation workaround added. Live presentation requires an unlocked interactive desktop. Offscreen qualification remains independently actionable; successful presentation would still not prove physical calibration. Evidence: `research/live-hdr-screen-lock-2026-10-11.json`.

### Real RAW HDR float swap/reuse (2026-10-11)

Nine-original collection now verifies both SDR and scene-linear RGBA16Float 1920x1080 frames with persistent renderers sharing GPU budgets. Thirty-second run: 216 sensor restores, 216 exact SDR and 216 bit-exact HDR frames; original HDR references contain above-one and negative components and are all finite. Shared resident credit equals both active atlases, all final GPU/storage roots zero. Existing 215 pressure/cancel failures target SDR renderer. Driver requires HDR source/frame evidence. GPU timing now includes both SDR and HDR, so it is not comparable to prior SDR-only measurements. Offscreen transport is not physical HDR/appearance qualification. Evidence: `research/real-raw-hdr-swap-2026-10-11.json`.

### HDR-specific RAW replacement failure atomicity (2026-10-11)

Nine-original combined collection completed 135 exact SDR/HDR restore frames over 31.782 seconds. HDR renderer now independently undergoes 134 full logical GPU-budget refusals and 134 partial upload cancellations, with previous binary16 frame read back after every failure. Previous atlas credit retained, cancelled temporary credit released on completion, retries exact, all final roots zero. Driver validates HDR failure counts and preservation evidence. This closes the HDR-specific failure readback gap recorded above; physical VRAM pressure, live display calibration and independent appearance remain unqualified. Evidence: `research/real-raw-hdr-failures-2026-10-11.json`.

### Full ordinary GPU regression after HDR failure tests (2026-10-11)

All ordinary rrrah-gpu suites passed: 117 passed, 0 failed, 13 ignored. Actual Metal adapter markers and log hash verified. This refresh covers shared helper changes used by real HDR replacement/failure tests; manual ignored external-camera tests are not implicitly qualified. Live HDR still awaits an unlocked desktop, confirmed through read-only registry; no redundant probe restart. Evidence: `research/gpu-hdr-failures-refresh-2026-10-11.json`.

### Normalized sensor overflow refusal (2026-10-11)

CPU Bayer development now rejects a non-finite normalization result before clipping recovery or stage2 correction. Smallest-positive-f32 range and finite-gain multiplication overflow regressions verify typed refusals, complete reservation rollback and successful same-budget valid retry. Core: 119 passed, 2 ignored. This does not qualify extreme-range Bayer GPU shader input arithmetic; that remains separate. Evidence: `research/normalized-overflow-2026-10-11.json`.

### GPU extreme sensor correction admission (2026-10-11)

RAW upload certifies normal u16 correction bounds from metadata, scanning actual samples only for potentially non-finite gain/normalization or subnormal division. Unsafe nonzero sites refused before replacement; exact zero sites remain accepted via shader zero-numerator branch. Actual Metal 16/32Float regressions preserve prior frame/credit on both normalization and gain overflow, with exact valid retry. Focused: 14 passed; full ordinary GPU: 117 passed, 13 ignored. Later matrix/interpolation/exposure and half-float range remain separate numerical qualification concerns. Evidence: `research/gpu-extreme-correction-2026-10-11.json`.

### RAW GPU decoded-buffer length admission (2026-10-11)

Upload now checks width*height sample count before calibration or allocation. Empty, truncated and oversized buffers produce typed InvalidPixelCount errors while preserving previous HDR frame/credit on both float targets. Additional extreme-range scan cancellation regression confirms first-row cancellation leaves old zero frame and credit exact. Focused Metal suite: 2 passed. Evidence: `research/gpu-invalid-pixels-2026-10-11.json`.

### RGBE aligned output-window compute (2026-10-11)

Compute now binds aligned windows including any prefix before the first output row, with an explicit global output index in a budgeted 48-byte uniform. The previously refused 17x13/1024-byte case executes five dispatches; 17x33/8192-byte managed integration remains covered. Nonconstant CPU reference and tail sentinel pass, as do independent four-plane odd-grid values and cancellation/credit ownership checks. Library: 52 passed; actual Metal compute: 4 passed, 1 ignored. Input must still fit one binding and each output row must fit a window; no column segmentation claimed. Evidence: `research/rgbe-aligned-windows-2026-10-11.json`.

### Full original F828 compute refresh after window alignment change (2026-10-11)

Pinned independent 3360x2460 F828 sensor dump passed actual Metal managed compute: all 33062400 channel values agree with CPU lattice reference within 1e-5, CPU/GPU final credit zero; updated 48-byte uniform budget peak asserted. Full ordinary GPU regression: 117 passed, 13 ignored; the full F828 case was then explicitly run and passed separately. Both execution validators and log hashes verified. Authored normalization/gains test transport and arithmetic, not missing as-shot Emerald WB or photographic appearance; LibRaw oracle remains test-only. Evidence: `research/rgbe-window-full-sensor-refresh-2026-10-11.json`.

### Sustained current RAW SDR/HDR replacement failures on Metal (2026-10-11)

Current combined scenario passed 120.288 seconds, 49 forward/backward collection rounds, 441 exact full-sensor restores, and 441 exact SDR plus finite binary16 HDR frames across nine original RAW files. Each renderer passed 440 GPU-budget refusals and 440 partial-upload cancellations while preserving its prior frame and reservation; retries were exact. Final CPU, GPU resident, packing, and queue reservations were zero. All 117 ordinary GPU tests also passed (13 ignored). The qualification driver verified all required workload markers and log hashes.

Restore latency p50/p95/max: 43.459/110.205/405.155 ms. Combined SDR+HDR upload/render/readback: 84.529/190.688/411.452 ms; these combined measurements cannot be compared directly with earlier SDR-only runs. Logical GPU resident peak reached the injected 512 MiB budget; packing peak 2 MiB, queue peak 77,276,160 bytes. This is managed budget pressure and local swap, not physical slow-disk or OS-memory-pressure qualification. Live HDR presentation/calibration remains separate. Evidence: `research/raw-hdr-pressure-120s-2026-10-11.json` and preserved log.

### Explicit Metal HDR extreme and 36 MP LOD checks (2026-10-11)

All three normally ignored `raster_readback hdr_` tests were explicitly executed with Metal required and passed. Authored exposure extremes produced expected finite binary16 outputs; opposite finite f32 extremes interpolated to the expected center without Inf/NaN anywhere in the HDR target. The 36 MP area-LOD scenario checked all 1,000,000 output pixels exactly, SDR/HDR Metal frames, and output root release. These are offscreen empirical Metal results; extreme conversion remains outside the portable binary16 caller contract and the tests remain explicitly opt-in. No NVIDIA, physical HDR, or original-camera appearance claim follows. Evidence: `research/metal-hdr-extremes-2026-10-11.json` and hashed log.

### Large raster independent GPU admission failures (2026-10-11)

Strengthened the shared raster cancellation helper with separate texture-budget and upload-queue-budget refusals. Both must return a typed memory error, retain the prior full frame and its reservations, and release all temporary reservations. Existing cancellation checkpoints and successful replacement/retry remain checked, including final zero credits. The explicit 4000x3000 (12 MP, 192 MB float payload) case passed on Metal Apple M4 Max, with six validation/admission cancellation checkpoints. The ordinary raster suite passed 16 tests (8 ignored). This helper does not test cancellation after partial GPU writes; that is a separate tiled/RAW scenario. Evidence: `research/metal-large-admission-2026-10-11.json` and hashed logs.

### Compute-to-RAM-to-swap HDR frame preservation (2026-10-11)

Extended compute-cache integration tests to compare finite signed binary16 HDR frames after compute, while RAM-pinned despite a zero cache limit, and after disk restore. Each oracle includes negative and above-one RGB values. Both explicit manual tests passed on Metal Apple M4 Max: 164x50 and full 4000x3000 (12 MP) render/readback. Existing all-component float-bit and SDR checks also passed; one swap write/read per case, final CPU/GPU/queue credits zero. This validates authored exposure transport and retention, not original-camera appearance or physical HDR calibration. Evidence: `research/metal-compute-cache-hdr-2026-10-11.json` and hashed log.

### Compute/cache HDR qualification gate (2026-10-11)

The integrated driver now requires both explicit compute/cache cases, exact SDR extents, finite signed HDR computed/pinned/restored markers, zero final CPU/GPU credit, one swap write/read per case, and actual Metal adapter markers. A valid current log was accepted; seven variants with missing HDR, duplicated extents, wrong full-frame extent, retained credit, wrong backend, missing execution, or ignored execution were rejected. The updated driver completed 117 ordinary GPU tests (13 ignored) and both explicit compute/cache tests with workload evidence verified. Logs and hashes preserved in `research/compute-cache-hdr-driver-2026-10-11.json`. Earlier SDR-only reports do not satisfy this new HDR gate.

### Viewer TTL/prefetch binary16 HDR frame checks (2026-10-11)

Extended the existing production viewer EXR/Radiance TTL test with finite, above-one binary16 HDR frame oracles, in addition to exact float payload and SDR checks. All 128 TTL restores and two actual preloads from swap while the decoder gate is blocked must reproduce the HDR frame exactly. The real Metal test passed and all final tracked CPU credits were zero. The integrated driver now requires the new 130-frame HDR marker, the no-decoder-fallback/blocked-preload marker, actual Metal, and executed tests. Updated integrated run passed 117 ordinary GPU tests (13 ignored) and the explicit TTL/prefetch scenario. Zero TTL tests expiry paths, not elapsed nonzero-TTL scheduling; small authored files do not qualify large originals or physical HDR. Evidence: `research/viewer-ttl-hdr-binary16-2026-10-11.json` and hashed logs.

### RGBE input storage binding windows (2026-10-11)

Removed the requirement that a complete normalized RGBE sensor fit one storage binding. Each dispatch now binds an aligned input window containing two neighboring sensor rows on either side for lattice interpolation; the shader translates global sample indices to the window. Output windows remain independently aligned. Checked planning limits rows to both windows, and full buffers must still fit max_buffer_size and managed budgets. This is binding segmentation, not streaming allocations or column segmentation.

A 17x33 sensor now passes with a 1024-byte binding cap despite its 2244-byte input: nonconstant four-plane CPU/GPU comparisons cover all pixels, odd boundaries, three CFA permutations and output-tail preservation; managed repeating-black-grid compute also passes this cap and releases credits. Focused validation: 53 library and four GPU tests passed, one full-sensor test ignored there. The integrated run separately executed the original full F828 compute successfully and all 118 ordinary GPU tests passed (13 ignored). Full-sensor authored normalization/gains do not qualify as-shot camera appearance. Evidence: `research/rgbe-input-windows-2026-10-11.json` and hashed logs.

### Full F828 with forced segmented input binding (2026-10-11)

Strengthened the explicit full-sensor test to run both default 128 MiB bindings (one dispatch) and required 1 MiB bindings (130 dispatches). The latter input is over 31 MiB and cannot fit one binding. Actual Metal Apple M4 Max passed every one of 33,062,400 channel comparisons per case against the CPU reference within 1e-5, including all input/output window boundaries. GPU peak equals each plan's explicit buffers and 48-byte uniforms; output drop releases CPU credit and final CPU/GPU credits are zero. Independent F828 sensor values use authored normalization/gains, not as-shot appearance. Buffers remain fully allocated. Evidence: `research/full-f828-input-windows-2026-10-11.json` and hashed log.

### Every segmented RGBE cancellation checkpoint (2026-10-11)

Added actual-GPU regression for a nonconstant signed 17x33 input (2244 bytes) under 1024-byte bindings and exact CPU/GPU budget limits. The successful baseline matches independent CPU lattice interpolation for every channel. All 18 cancellation checks, including validation, admission, each dispatch encoding, pre-submit and post-readback, return Cancelled and release reservations; every immediately following retry exactly matches the baseline. Five RGBE GPU tests passed, one explicit full-sensor test ignored in this focused run. The GPU submission is still one command buffer: dispatch-encoding cancellation does not claim interruption of already submitted partial work. Evidence: `research/rgbe-segment-cancel-2026-10-11.json` and hashed log.

### Current full-resolution linear exposure compute refresh (2026-10-11)

Both explicit 6188x4120 compute tests passed on Metal Apple M4 Max. Managed and interleaved paths checked every one of 25,494,560 pixels exactly, preserving negative/HDR channels and alpha. End-to-end times were 169.132 ms managed and 215.877 ms interleaved. Tracked peaks: GPU buffers 1,223,738,992 bytes and CPU buffers 815,825,920 bytes, final usage zero. The separate low-level warmed encode/submit/wait sample p50 was 2.609 ms (max 2.712 ms); it excludes upload/readback and checks only three samples, so it is not full viewer latency. Authored pattern, explicit buffers only, no original-camera or physical HDR claim. Evidence: `research/full-compute-current-2026-10-11.json` and hashed log.

### RGBE window geometry sweep (2026-10-11)

Added independent lattice-neighbor containment checks across 16,384 dimension/binding combinations (width/height 2..65; caps 256/512/1024/4096). All 7,557 admitted plans contain every needed input lattice row, have aligned in-bounds input/output bindings, cover every output row contiguously, and account exactly for full buffers plus per-dispatch uniforms. Four planner tests passed. Refused layouts remain unsupported by row-only segmentation; the sweep does not classify them as successful or prove GPU execution. Read-only macOS registry recheck still reports screen locked, so live RAW HDR remains pending. Evidence: `research/rgbe-window-geometry-2026-10-11.json` and hashed log.

### 3D model cancellation after geometry admission (2026-10-11)

Strengthened actual-Metal model swap cycles to cancel after a nonzero geometry reservation is observed, in addition to immediate cancellation. Every such cancellation must release geometry credit, leave cache errors zero, and retain the disk entry for exact retry. Seven model tests passed, including six STL/OBJ/OFF/PLY fixtures across 20 repeated cycles (120 restores) and the ordinary one-cycle case. Geometry/metadata payloads and SDR plus binary16 HDR frames remain exact; final CPU/queue credits zero. These are small subset fixtures, not full 3D-format or large-collection qualification. Evidence: `research/model-admitted-cancel-2026-10-11.json`, test-source hash, and preserved log.

### Cancellable model upload API (2026-10-11)

Added `ModelRenderer::upload_with_cancel` with typed Cancelled and checkpoints before work, every 1024 triangles during both validation and mapped-buffer filling, after admission, and before committing replacement. The existing upload API delegates with cancellation disabled. Temporary buffer/reservation ownership rolls back on cancellation, preserving existing resident geometry. Seven actual Metal model tests passed; new regression exercises all five checkpoints for a triangle, exact retained resident credit/bytes, successful retries and final zero credit. Application check passed. This new regression does not read back the previous frame, and viewer event-loop scheduling is unchanged. Evidence: `research/model-upload-cancel-2026-10-11.json` and hashed logs.

### Model upload failure frame preservation on SDR/HDR (2026-10-11)

Closed the frame-readback gap in the new cancellable model upload regression. On actual Metal, both Rgba8UnormSrgb and Rgba16Float attachments retain every 63x65 frame sample after each of five cancellation checkpoints and an independent GPU-budget refusal. Resident credit stays unchanged; a successful replacement with different geometry changes the image, proving the retry is not a no-op. Renderer drop releases all geometry/depth credit. Eight model GPU tests passed. Small authored single-triangle geometry and logical budget pressure, not large-model or physical HDR qualification. Evidence: `research/model-cancel-frame-2026-10-11.json` and hashed log.

### Multi-chunk model upload cancellation (2026-10-11)

Expanded SDR/HDR replacement regression to 2049 triangles, crossing three 1024-triangle polling blocks in both validation and mapped-buffer filling. All nine cancellation checkpoints per attachment retain the exact previous frame and reservation, including cancellation after partial vertex-buffer filling. Independent budget refusal, changed-image retry, and final zero credit also pass. Eight actual Metal model tests passed. Viewer source audit confirms generation changes are handled by the same synchronous event loop; merely wiring a generation callback would not provide responsive UI cancellation, so no such behavior is claimed. Evidence: `research/model-multichunk-cancel-2026-10-11.json` and hashed log.

### Worker-safe prepared model geometry (2026-10-11)

Split model GPU preparation from visible installation. `PreparedModelUpload` owns the GPU vertex buffer and reservation; static cancellable preparation can run on a worker, while installation moves the accepted result into a renderer. Existing upload wrappers preserve behavior. Actual Metal regression prepares 2049 triangles in a std::thread, checks exact previous SDR/HDR frames while prepared geometry is retained, drops the obsolete result and verifies released temporary credit, then installs accepted geometry and verifies changed pixels. All eight model tests pass, including cancellation and pressure. Viewer worker/event integration remains pending; callers must use the same device and intended shared budget. Evidence: `research/model-prepared-2026-10-11.json` and hashed log.

### Prepared geometry budget admission guard (2026-10-11)

Closed a limit-bypass possibility in the prepared model API: a budgeted renderer now accepts nonempty geometry only when its preparation budget is the renderer budget or a descendant. Unbudgeted and independent-root preparations return typed BudgetMismatch before visible replacement; consumed rejected geometry drops and releases its reservation. Actual Metal tests verify both refusal modes preserve exact previous SDR/HDR frames and release unrelated-root credit, while matching-budget worker preparation/install still succeeds. Eight model tests and application check passed. Same-device use remains a caller contract; viewer worker integration remains pending. Evidence: `research/model-prepared-budget-2026-10-11.json`, source hash and preserved log.

### Renderer-bound model worker preparation (2026-10-11)

Added cloneable `ModelUploadPreparer`, carrying the originating device, shared budget and private Arc owner identity into a worker. Prepared uploads require matching owner identity and compatible budget before installation. Unbound low-level results cannot be installed. A direct wgpu Device-equality experiment failed because independent Instances yielded equal DeviceId(0,1); that equality guard was removed in favor of owner identity. Actual Metal regression now prepares geometry on a second independent Instance/device with the same budget and proves typed RendererMismatch, temporary credit release and exact retained SDR/HDR pixels. Matching worker context still installs successfully. Eight model tests and application check passed. Viewer worker integration remains pending. Evidence: `research/model-preparer-owner-2026-10-11.json`, source hash and preserved log.

### Bounded application model upload worker (2026-10-11)

Added model upload worker module with one active job, one replaceable pending source, and one completed prepared geometry. Revision plus decode-generation cancellation rejects stale work; replacement drops previous pending/completed owners, publication releases source geometry before waking UI, and shutdown cancels/joins the worker. Actual Metal test submitted 1000 replacements, accepted only the latest job ID/generation, installed it, then cancelled a subsequent generation and proved only installed credit remained; final renderer drop credit zero. Application check and explicit worker test passed. Module is not yet wired to ModelReady dispatch, so interactive behavior is not claimed. Evidence: `research/model-upload-worker-2026-10-11.json` and hashed log.

### Model worker integrated into viewer events (2026-10-11)

Production ModelReady handling now queues geometry validation/normalization and mapped-buffer filling on the bounded model worker, retaining the original source event/consumer lease until completion. The worker wakes the application event loop. UI accepts only matching job ID and current loader generation, then resizes depth and installs the prepared geometry; generation changes clear pending work/results and cancel active preparation. The accepted UI branch no longer traverses triangles. Completion handling avoids consuming simultaneous raster-LOD output over an accepted model result.

Application check passed; 22 ordinary model cache/swap/prefetch tests passed, one manual worker test ignored there and then explicitly executed successfully on Metal with the 1000-replacement burst. Locked screen prevents live interactive navigation/frame-time qualification. Initial pipeline creation and depth resize remain on UI thread. Evidence: `research/model-worker-integrated-2026-10-11.json` and hashed logs.

### Completed model GPU result cancellation races (2026-10-11)

Extended actual Metal worker regression to wait deterministically for a completed result retained in its slot while the original geometry is installed. GPU credit is exactly 72 bytes (36 installed + 36 prepared). Changing the loader generation before consumption rejects the stale result and releases credit to 36; explicit cancel of a second already-completed result does likewise. The 1000-replacement burst and active-generation cancellation still pass; final worker/renderer drop credit zero. This uses observed completed state rather than sleeps to assume a race. Live-window responsiveness remains pending. Evidence: `research/model-completed-races-2026-10-11.json` and hashed log.

### Integrated model GPU worker gate (2026-10-11)

Added `--metal-model-upload-worker` to the qualification driver. It explicitly runs the manual bounded worker test and requires both the 1000-replacement/latest-only/final-zero marker and the completed-result generation-change/explicit-cancel marker, with exactly one executed test and none ignored. The integrated run passed 122 ordinary GPU tests plus the explicit worker test; every suite execution and log hash verified. This includes current RGBE window and model upload API changes in the ordinary GPU run. Live-window responsiveness and physical HDR remain separate pending checks. Evidence: `research/model-worker-gate-2026-10-11.json` and hashed logs.

### Model worker managed source and cache-lease lifecycle (2026-10-11)

Worker qualification now decodes its STL source under a CPU MemoryBudget and holds an actual LeaseCache consumer lease while preparing GPU geometry. Source usage equals model capacity; zero-cap eviction is refused while the lease is held, including after worker completion. Dropping the consumer lease permits eviction. The last source owner moves into a cancelled job; worker shutdown releases all CPU source credit, while installed GPU credit remains until renderer drop. Actual Metal test passed with prior 1000-replacement and completed-result race checks retained. Driver now additionally requires this ownership marker; older gate reports do not qualify it. Evidence: `research/model-worker-source-2026-10-11.json` and hashed log.

### Deterministic bounded model queue under overload (2026-10-11)

Added test-only pre-preparation rendezvous to hold the first active request while 1000 newer jobs replace the pending slot. The test observes exactly one latest pending job, no completed output and zero admitted GPU credit while blocked; releasing the first request rejects its obsolete revision and processes only the latest. The rendezvous uses a bounded timeout to avoid hanging teardown on an assertion failure. Actual Metal test passed with managed source/lease lifecycle, completed-result race checks and final zero credits retained. Driver now also requires the deterministic queue marker; older reports do not prove this strengthened test. Evidence: `research/model-worker-bounded-2026-10-11.json` and hashed log.

### Prepared model replacement versus GPU submission lifetime (2026-10-11)

Added actual-Metal regression encoding the old model before preparing/installing replacement. Its geometry/depth lease must retain old credit across replacement and renderer drop. The encoded old commands are then submitted and the lease retained until completion; no scoped GPU validation error and final credit zero. This verifies explicit budget ownership does not release old in-use resources early during async replacement. Nine model GPU tests passed. Small authored geometry, not a large-model benchmark. Evidence: `research/model-submission-lifetime-2026-10-11.json` and hashed log.

### Native 3D swap-to-worker SDR/HDR pipeline (2026-10-11)

Added actual Metal end-to-end worker test for six STL/OBJ/OFF/PLY originals, including ASCII and 64-bit/big-endian PLY variants. Each is decoded under CPU budget, spilled once, restored separately for SDR/HDR, moved into bounded worker preparation, installed and rendered. All frame samples exactly match pre-swap direct-render references; CPU source credit is released before completion consumption and GPU credit reaches zero after each renderer drop. Six writes, twelve reads, zero swap errors. This is transport/retention equivalence for subset fixtures, not independent appearance or full 3D syntax qualification.

The existing driver worker option now explicitly runs both manual worker tests and requires all six source markers plus deterministic overload, managed-source lease and completed-result cancellation markers. Integrated run passed 123 ordinary GPU tests and both worker tests; all suite evidence/log hashes verified. Evidence: `research/native-model-worker-2026-10-11.json` and hashed logs.

### Application-wide regression after model worker integration (2026-10-11)

All ordinary rrrah binary tests executed after the model event-loop changes: 208 passed, zero failed, 52 ignored. This broadens validation beyond the 22 model-focused tests to ordinary navigation, RAW/raster caching, prefetch, scalar/window and foreground-loader behavior. Manual GPU and external-fixture tests remain separate and are not counted as passing; the two current model worker Metal tests were independently executed in the native-model worker report. Live event-loop/window behavior still awaits an unlocked desktop. Evidence: `research/app-after-model-worker-2026-10-11.json` and hashed log.

### Model worker GPU pressure and retry, loading-state fix (2026-10-11)

All six native model swap-to-worker cases now force full logical GPU-budget pressure before both SDR and HDR preparation. Twelve typed memory refusals release restored CPU geometry without invalidating swap entries; after pressure release, all twelve rereads/worker preparations render exact reference frames. Six writes, 24 reads, zero swap errors, final CPU/queue/GPU zero. Both manual Metal worker tests passed. Driver now requires the pressure marker.

Fixed production error handling for failed preparation or worker startup: clear pending-first-present and reset pipeline/telemetry to idle while retaining previous visible frame. All 208 ordinary application tests passed (52 ignored); live error-state UI is not yet verified due locked desktop. Evidence: `research/model-worker-pressure-2026-10-11.json` and hashed logs.

### Million-triangle native model worker (2026-10-11)

Added manual native binary-STL workload with 1,000,000 authored repeated triangles. Source decode uses a 256 MiB managed CPU root; worker preparation uses an exact 36,000,000-byte GPU geometry cap. Accepted output owns exactly 36 MB; source CPU credit reaches zero before completion consumption, and worker/renderer drop leaves both roots zero. First focused run: decode 38.312 ms, UI-side submit including worker startup 0.096 ms, submitted-to-ready 113.610 ms. Single-run stage timings, not a live UI or million-triangle rendering benchmark.

Driver now explicitly runs all three model worker tests and requires the million-triangle completion marker alongside all queue/lease/race/swap-pressure evidence. Current integrated gate passed 123 ordinary GPU tests and three manual worker tests; hashes/workloads verified. Evidence: `research/million-model-worker-2026-10-11.json` and hashed logs (integrated run has its own timings).

### Million-triangle Metal draw/readback smoke test (2026-10-11)

Extended the million-triangle prepared-worker test through actual drawing and readback. All 16x16 SDR pixels exactly match a single-triangle reference of the repeated coplanar authored surface. Draw plus target/readback setup took 21.160 ms in this single run; geometry credit is 36 MB, depth 1024 bytes, final CPU/GPU credit zero. This is a small-target million-vertex-upload/draw smoke test, not complex-scene or full-viewer-resolution performance qualification and not million-triangle HDR qualification. Driver now requires the draw marker; previous gate reports do not cover it. Evidence: `research/million-model-render-2026-10-11.json` and hashed log.

### Million-triangle grid at full HD SDR/HDR (2026-10-11)

Added native binary STL with a 500x1000-cell grid (1,000,000 distinct coplanar triangles), avoiding the earlier repeated-overdraw smoke geometry. Each attachment independently decodes under a managed CPU root, prepares via bounded worker, installs and renders 1920x1080. All SDR and finite binary16 HDR samples exactly match an independently constructed two-triangle plane with the same bounds, including internal grid boundaries. Source CPU releases before completion consumption; explicit geometry/depth usage is exactly 36,000,000 + 8,294,400 bytes, final zero. Test target/readback and driver allocations are outside that explicit budget.

First focused run: SDR prepare 125.315 ms, render/readback 30.201 ms; HDR prepare 164.368 ms, render/readback 43.283 ms. Single-run host stage timings, not live UI FPS or complex-scene performance. Driver now requires full-HD completion and executes four worker tests. Integrated gate passed 123 ordinary GPU tests plus all four worker tests, workloads/hashes verified. Evidence: `research/million-grid-full-hd-2026-10-11.json` and hashed logs (integrated timings recorded separately).

### Million-triangle swap/restore failures through full-HD Metal (2026-10-11)

Extended the native million-triangle grid scenario to spill its managed decoded geometry once, release the source, and independently restore for SDR and HDR. Both complete serialized payloads equal the pre-spill geometry/metadata. Before each successful restore, full CPU-budget pressure must refuse admission and cancellation after a nonzero restore reservation must release partial ownership without invalidating the swap entry. Two refusals and two admitted cancellations passed, followed by exact full 1920x1080 SDR/HDR frames via bounded GPU worker. One write, two successful reads, no swap errors, final CPU/queue/GPU credit zero.

Focused Metal test passed. Driver additionally requires swap/failure markers; older integrated reports do not prove these additions. Local disk and managed pressure, authored planar geometry; oracle allocations/readback targets are outside managed source/geometry budgets. Evidence: `research/million-grid-swap-2026-10-11.json` and hashed log.

Integrated swap/failure gate now passed on Metal: 123 ordinary GPU tests (13 ignored) and all four explicitly selected worker tests. Required million-grid pressure, admitted cancellation, payload equality and full-HD SDR/HDR markers verified. Evidence: `research/million-grid-swap-gate-2026-10-11.json` with hashed preserved logs. This supersedes the focused-only status for this scenario; physical HDR and complex-scene/live UI performance remain unqualified.

### Application regression after model-worker integration (2026-10-11)

Ordinary application tests passed: 208 passed, zero failed, 54 ignored. The four explicitly selected worker tests are covered separately by the integrated Metal gate. Evidence: `research/app-metal-regression-2026-10-11.json` and hashed log/source files. Screen lock remains confirmed, so no live HDR probe was launched. Model shader/pipeline construction and depth resize remain on the UI thread; background geometry preparation does not prove first-open UI responsiveness. These software and live qualification limits remain open.

### First model pipeline created on worker (2026-10-11)

First-open model jobs now carry device/target/budget to the bounded worker, which creates the renderer and prepares owner-bound geometry. The event loop accepts the matching generation and installs the returned renderer; production model event handling no longer constructs a pipeline. Stale jobs are checked before compilation and discarded after it; driver compilation itself cannot be interrupted. Existing renderer replacement still reuses its bound preparer. Depth resize remains on UI. Added explicit first-open SDR/HDR worker completion, owner-bound installation, and final CPU/GPU zero checks to the bounded-worker test; qualification driver requires their marker. Integrated Metal gate: 123 passed / 13 ignored ordinary GPU tests, four explicitly selected worker tests passed; application regression: 208 passed / 54 ignored. Evidence: `research/model-first-open-2026-10-11.json` and preserved hashed logs. Live responsiveness and physical HDR remain unverified.

### First-open worker frame parity (2026-10-11)

Extended first-open SDR/HDR tests beyond owner-bound installation: render all 63x65 samples through the returned worker-created pipeline and compare exactly with the synchronous renderer path, including binary16 HDR bits. Explicit geometry/depth admission exactly equals 36 + 63*65*4 bytes and returns to zero. This is transfer parity against the same renderer, not independent appearance qualification; output/readback/driver allocations remain outside that budget. Integrated driver requires the new frame marker and passed 123 ordinary GPU tests (13 ignored) plus four explicitly selected worker tests. Evidence: `research/model-first-open-frames-2026-10-11.json` with source/log hashes.

### Discarding completed first-open pipelines (2026-10-11)

Six deterministic cases now hold a newly compiled renderer plus prepared geometry in the completion slot, then invalidate before UI consumption: generation change, explicit cancellation, and worker shutdown, each for SDR and HDR. Source CPU credit is zero before invalidation; prepared geometry credit is 36 bytes while queued and zero after rejection/drop. Generation/cancel paths return no completed result. Driver requires the six-case marker. Integrated Metal gate passed 123 ordinary GPU tests (13 ignored) and all four selected worker tests. Evidence: `research/model-first-open-cancel-2026-10-11.json` and hashed logs. This verifies completed-result ownership, not interruption of driver shader compilation or physical HDR.

### Preserve model on refused viewport resize (2026-10-11)

Fixed the production viewport path: depth replacement admission now precedes surface/configuration mutation. On refusal it retains the prior viewport and active model instead of disabling model rendering. Added Metal SDR/HDR regression with 63x65 resident depth, forced 67x69 admission refusal, exact old-frame readback, unchanged-size reuse under full pressure, pressure release and exact retry frame against synchronous reference. Final managed GPU credit zero. All 10 model readback tests passed; application regression 208 passed, 54 ignored. Evidence: `research/model-resize-retention-2026-10-11.json` and hashed logs. Live surface presentation at a physically resized window remains unverified while desktop is locked.

### Deferred model viewport recovery (2026-10-11)

Production resize refusal for managed memory now retains the desired size and retries after 250 ms from about_to_wait, including idle windows. Retry deadline participates in the event-loop minimum deadline alongside thumbnail work; pending GPU completion polling remains higher cadence. Success clears the pending resize and requests redraw; later resize requests supersede earlier ones. Geometry-limit failures are not retried. App regression passed 208 tests, 54 ignored; `git diff --check` clean. Evidence: `research/model-resize-retry-2026-10-11.json`. This is implemented and compiled behavior; live surface/event-loop recovery still requires an unlocked desktop and is not inferred from unit tests.

### Combined current Metal gate (2026-10-11)

One sequential driver invocation passed 142 tests across 14 suites: 124 ordinary GPU tests (13 ignored), mixed-format preload navigation, five linear import cases, 12 MP float HDR, viewer TTL/preload, large upload refusal/cancellation, two compute-cache cases, four model-worker tests, 20-cycle model swap, and two full-sensor compute tests. Required workload markers and preserved log hashes verified. Evidence: `research/combined-metal-current-2026-10-11.json` and `research/combined-metal-current-2026-10-11/`. This proves selected software paths at this source state, not simultaneous soak, physical HDR, live surface resize recovery, NVIDIA, or full 100-format qualification.

### Reproducible qualification selection (2026-10-11)

Qualification reports now retain the complete invocation and every parsed requested option, including previously omitted model-worker selection. Verified with an actual Metal run: model-worker true, GPU-only true, full-sensor compute false, exact invocation flag retained; 124 ordinary GPU tests (13 ignored) plus four selected worker tests passed. Evidence: `research/metal-reproducible-report-2026-10-11.json`. Older reports retain their original schema; the combined 142-test checkpoint remains evidence at its recorded source state.

### Ten-minute original RAW SDR/HDR replacement soak (2026-10-11)

Current combined persistent SDR/HDR workload passed 600.102 seconds: nine original RAW sources, 195 rounds, 1755 exact restorations/frames, nine swap writes. Each renderer passed 1754 forced replacement refusals and 1754 partial-upload cancellations, preserving previous frames/credit and succeeding on retry. Final CPU, resident GPU, upload and queue credits zero. Restore p50/p95/max 65.181/111.069/276.472 ms; combined upload/render/readback 115.744/198.576/399.158 ms. Includes selected source/metadata/frame comparisons, not independent camera appearance or physical HDR. Driver also passed 124 ordinary GPU tests (13 ignored). Local disk, managed pressure; RSS samples begin partway through and do not prove absence of every leak. Evidence: `research/raw-sdr-hdr-600s-2026-10-11.json` with preserved logs, input hashes and RSS samples.

### Match view parameters to accepted viewport (2026-10-11)

Fixed two production recovery details found by source inspection. Zero-sized resize now clears deferred model resize, avoiding obsolete GPU allocations while minimized. Resize event view parameters now follow the accepted GPU size instead of the requested window size on refusal. Successful deferred recovery updates view parameters before requesting redraw. Ordinary app regression: 208 passed, 54 ignored. Evidence: `research/viewport-recovery-view-2026-10-11.json` and log. Earlier depth readback proved GPU transaction behavior but did not cover application view parameters; live window recovery remains unverified.

### Bind model depth to renderer device (2026-10-11)

Depth allocation/limit validation now uses the renderer-owned device. The existing resize device argument remains for source compatibility but cannot redirect allocation to a foreign device, avoiding mixed-device render attachments. A regression creates a second independent instance/device and passes it into resize, then verifies every SDR/HDR sample equals the renderer-device reference and final managed credit is zero. All 11 model readback tests passed on Metal. Evidence: `research/model-device-depth-2026-10-11.json` and log.

### Open RAW shader range audit (2026-10-11)

Source review found that finite photosite normalization/gain admission does not prove finite downstream filter sums, color dot products, exposure multiplication or binary16 conversion. Four-sample averages and area accumulation currently sum before normalization. This is an open extreme-range contract gap, not a demonstrated failure on ordinary camera files or hardware. Next step is a finite admitted-input Metal reproduction before deciding between range-safe arithmetic and explicit refusal; preserve ordinary appearance. Evidence: `research/raw-shader-range-audit-2026-10-11.json`.

### Reproduced finite RAW filter overflow and admission fix (2026-10-11)

A finite 1e38 corrected Bayer field produced [inf, inf, NaN, 1] on Metal RGBA32Float. Reordered shader arithmetic did not resolve it and was withdrawn; original shader hash restored. Upload now bounds corrected samples for worst full-sensor filter accumulation, scanning actual samples only when metadata bounds require it and returning ShaderFilterRangeExceeded before resource replacement. Regression verifies typed refusal, retained previous center pixel and valid retry for float16/float32. Current GPU suite: 126 passed, 13 ignored. Evidence: `research/raw-filter-range-2026-10-11.json` with before/after logs. Matrix/exposure/binary16 range handling remains open; this is a filter admission contract, not complete extreme-HDR qualification.

### RAW color/exposure admission and explicit float16 saturation (2026-10-11)

Expanded admission bound by maximum absolute color-matrix row sum, +10-stop exposure and headroom for gamut/quadratic ACES operations. Profiles/samples beyond the conservative bound refuse before replacing GPU resources; ordinary ranges use metadata fast bounds. Float16 SceneLinear uses a distinct uniform output mode and clamps to signed +/-65504 explicitly; float32 preserves larger representable values. Regression covers unsafe finite correction refusal and retained previous center, then gain100/exposure10: RGB approximately102400 in float32, exactly65504 in float16. Current full GPU suite: 126 passed, 13 ignored. Evidence: `research/raw-color-range-2026-10-11.json` and hashed log. This is conservative bounded-input handling, not an exhaustive proof for arbitrary nonfinite view parameters or physical HDR.

### Nonfinite RAW view input retention (2026-10-11)

Reproduced changed/invalid output from nonfinite view controls on Metal. RAW update_view now retains previous finite viewport/pan/zoom/exposure components independently, applying ordinary bounds to accepted controls. Regression checks NaN and both infinities preserve the previous center pixel in float16/float32, then mixed invalid geometry/valid exposure updates change the frame and match an all-valid request. All four raw-linear readback tests passed. Evidence: `research/raw-view-nonfinite-2026-10-11.json` with before/after logs. Finite extreme geometry and live window behavior remain separate qualification.

### Raster nonfinite controls full-frame regression (2026-10-11)

Existing raster normalized_view already replaces nonfinite viewport/pan/zoom/exposure with finite defaults. Added actual Metal regression for NaN and both infinities, valid/invalid viewport, SDR and binary16 HDR: all 64x64 samples equal finite controls and all HDR samples are finite. One selected test passed. Existing raster fallback semantics retained; unlike RAW, fallback uses defaults rather than prior components. Evidence: `research/raster-controls-nonfinite-2026-10-11.json` and log.

### Original RAW acceptance after shader-range hardening (2026-10-11)

Current Metal gate accepts all nine original CR2/NEF/ARW/ORF/PEF/RW2/RAF/CR3 inputs after range admission, float16 saturation and nonfinite control changes. Ten rounds/90 restorations preserve native payload and SDR/HDR frames; each renderer passes 89 replacement pressure refusals and cancellations, retries exact and final tracked resources zero. Full ordinary GPU suite also passed (128 tests, 13 ignored). Evidence: `research/raw-range-originals-2026-10-11.json` with input/source/log hashes. This is current corpus acceptance and transport, not independent camera appearance or a rerun of the ten-minute workload.

### Signed RGBE float16 range conversion (2026-10-11)

Added authored RGBE exposure10 regression at zoom2/.3/.125. Float32 center RGB remains finite with components beyond both signed binary16 limits; float16 endpoints equal exactly +65504/-65504, preserving sign, with in-range components agreeing within binary16 tolerance. All five raw-linear readback tests passed on Metal. Evidence: `research/signed-rgbe-hdr-limit-2026-10-11.json` and log. This is center/zoom range conversion, not full-frame camera appearance or physical HDR.

### Exceptional RAW range scan ownership/cancellation (2026-10-11)

Extended the range-admission regression with a managed GPU root and deterministic cancellation at checks2/9/17, spanning the sixteen-row actual-sample scan. Float16/float32 retain previous center frame and resident credit on cancellation/refusal. Zero-valued sensor with gain1e16 is admitted, proving that a dangerous metadata bound alone does not refuse safe actual samples. Valid retry exact and final GPU credit zero. All five raw-linear tests passed. Evidence: `research/raw-range-cancel-2026-10-11.json` and log.

### Mandatory numerical regression execution (2026-10-11)

The ordinary GPU qualification row now requires four named numerical tests to complete successfully: finite RAW range admission, nonfinite RAW controls, signed RGBE half saturation, and nonfinite raster controls. Actual Metal gate passed 129 tests (13 ignored), all four required names verified. Validator rejected 13 altered evidence cases (each required test ignored/failed/missing plus aggregate-only fake success). Evidence: `research/numeric-required-gate-2026-10-11.json` and hashed log.

### RGBE compute signed-extreme interpolation fix (2026-10-11)

Reproduced NaN from finite +/-f32MAX interpolation on Metal. Shader now returns exact lattice endpoints; exceptional midpoint interpolation uses IEEE exponent halving before addition, avoiding an overflowing difference during optimization. Ordinary midpoint arithmetic remains unchanged. All100 output channels of authored5x5 field exactly match independent f64 bilinear oracle, including signed extremes, edges and midpoint zeros; managed CPU/GPU credit returns to zero. Focused suite6 passed/1 ignored; full GPU gate130 passed/13 ignored. New test required by numeric execution validator. Evidence: `research/rgbe-extreme-2026-10-11.json` and before/focused/full logs. Full-sensor performance remains separate.

### Full F828 after extreme interpolation fix (2026-10-11)

Re-executed explicitly ignored full-sensor test after RGBE blend fix: 3360x2460, all33,062,400 channel values match CPU reference within existing1e-5 tolerance in both128MiB/one-dispatch and1MiB/130-dispatch layouts. Final CPU/GPU credit zero. One test passed with no ignores in selected execution. Evidence: `research/rgbe-full-sensor-current-2026-10-11.json` and hashed log/oracle/shader. Authored normalization/gains do not qualify as-shot Emerald WB or appearance; no comparative performance claim.

### Full F828 managed compute stage timing (2026-10-11)

Added three managed executions per binding layout, timing validation/admission/upload/execute/wait/readback while excluding pipeline/source preparation and CPU oracle. Metal Apple M4 Max:128MiB one-dispatch min/p50/max45.331/48.645/60.622 ms;1MiB130-dispatch56.609/57.127/75.236 ms. GPU credit zero every run; final result in each configuration verified against all33,062,400 CPU-reference values, final CPU/GPU zero. Evidence: `research/rgbe-full-timing-2026-10-11.json`. Three-run descriptive timings, no paired old-shader speedup or live display FPS claim.

### Current native RAW to completed SDR GPU frame timing (2026-10-11)

Measured IMG_9043.CR3 at1920x1080 offscreen sRGB on Metal Apple M4 Max:15 samples after3 warmups, total p50/p95 126.323/155.527ms; native decode84.145/112.936, upload enqueue27.907/33.606, completion8.990/16.964ms. Sixty resident zoom/pan frames p50/p95 1.779/3.157ms. Managed CPU peak124,360,466 bytes, GPU146,800,640, final bothzero. Evidence: `research/raw-view-current-timing-2026-10-11.json` and log. This is one warm fixture/offscreen completion, not live UI or physical HDR. Source audit confirms separate RgbeCompute API is not called from production app main/gallery; RAW viewport performs interpolation in fragment shader. Compute timings must not be reported as viewer opening latency.

### Current RAW swap-to-frame timing (2026-10-11)

Same IMG_9043.CR3 warm local-disk fixture, 1920x1080 offscreen sRGB,15 measured after3 warmups: restore p50/p95 32.715/33.875ms, upload15.489/16.603, completion8.410/11.455, total56.925/59.262ms. Sixty resident frames1.656/2.859ms. Source samples/metadata exact across18 restores; one write, zero swap errors; final managed CPU/GPU zero. Evidence: `research/raw-swap-view-current-timing-2026-10-11.json` and log. Earlier native decode total126.323/155.527ms is a separate unpaired invocation, so this is descriptive one-fixture comparison rather than universal speedup or cold-disk/UI latency.

### Nine-original native RAW viewer timing (2026-10-11)

Sequential current-source warm-file benchmark:15 measured/3 warmups per original,1920x1080 offscreen SDR,60 resident zoom/pan frames per source. All9 completed and final managed CPU/GPU zero. ORF has the slowest total in this corpus at581.074ms median, followed by NEF546.965ms and CR2 545.282ms. Native decode dominates these slow cases; for NEF/CR2 it accounts for522.595/519.795ms rather than upload13.643/14.456ms or GPU completion9.044/9.466ms. Evidence: `research/nine-raw-native-timing-2026-10-11.json` contains per-file stage distributions/input hashes and log. This identifies optimization candidates for this corpus, not universal format ranking or paired speedup; CR3 medians varied versus earlier isolated invocation, demonstrating host/warmup sensitivity.

### ORF fused entropy prefix (2026-10-11)

Combined sign and two-bit remainder into one bounded three-bit read. Six alternating-order pairs on original1084.orf (4640x3472) match all16,110,080 samples exactly per pair. Existing-window median 312.897ms versus fused 282.858ms (9.60% reduction). Full decoder regression:1004 passed, zero failed,78 ignored. Evidence: `research/orf-prefix-paired-2026-10-11.json` and hashed logs/source/original. Entropy-loop timing on one original, excluding file IO/metadata/upload/render; not whole-viewer speedup or fresh independent decoding oracle.

### Current original ORF Metal opening and swap timing (2026-10-11)

After fused entropy prefix, original1084.orf4640x3472 to completed1920x1080 offscreen sRGB frame:15 measured loads after3 warmups. Native decode p50/p95 358.741/417.112ms; upload9.347/13.157; completion7.243/8.820; total374.563/434.679ms. Separate warm local-disk swap invocation:restore20.709/20.910; upload9.318/9.723; completion7.300/7.517; total37.303/37.788ms. Sixty resident zoom/pan frames p50/p95 native1.780/2.063ms and swap1.788/2.234ms. Source sample/metadata assertions passed; final managed CPU/GPU zero both runs. Evidence: `research/orf-metal-view-timing-2026-10-11.json` and hashed logs/source/original. Separate descriptive invocations; historical nine-original timing is not a paired baseline. Excludes pipeline creation, physical presentation and physical HDR; not a general format or live UI qualification.

### Nonzero background swap TTL (2026-10-11)

New ordinary regression uses2-second TTL, checks fresh explicit prune retains physical object/bytes, then observes only physical accounting until background worker removes expired file/index and releases quota. Source budget zero, no swap errors. Full cache regression:{'passed': 149, 'failed': 0, 'ignored': 5}. Evidence: `research/nonzero-swap-ttl-2026-10-11.json` and hashed focused/full logs. Small authored raster on real local filesystem; not full RAW viewer or physical slow disk.

### Independent sustained swap caps, current source (2026-10-11)

Strengthened byte-cap workload to verify capacity-evicted keys are unreadable, as already required for count cap. Two concurrent60-second tests passed:byte448 cycles/refusals;count449 cycles/refusals. Each reaches786,624bytes/3objects; byte cap786,624 with count100, count cap3 with byte8MiB, TTL3600s. Every pressure refusal followed by exact restore, physical/logical limits checked each cycle; final managed root/queue zero. Evidence: `research/independent-swap-caps-current-2026-10-11.json` and hashed logs/source. Authored256x256 rasters with injected2ms/64KiB I/O delays, not physical slow disk or GPU viewer qualification.

### Current integrated Metal gate (2026-10-11)

Full storage/viewer-selected Metal driver passed544 tests across19 suites (81 explicit ignores), including nine-original RAW collection, mixed-camera and multi-neighbor workers, actual CR3 reversal, viewer TTL/preload and bounded model upload worker. Separate full ordinary GPU gate passed130 tests (13 ignored), with all five required numeric regressions execution-verified. Both reports mark every executed suite verified; hashed logs preserved in `research/metal-integrated-current-2026-10-11/` and `research/metal-numeric-current-2026-10-11/`. These totals overlap and must not be added as unique tests. Explicit ignores, subset formats, physical HDR/live presentation and NVIDIA remain outside this evidence.

### Mandatory cache TTL execution evidence (2026-10-11)

Cache qualification now requires four exact passing test names: leased physical-file lifetime, idle background expiry, busy-barrier starvation resistance, and nonzero-TTL fresh retention/background expiry. A successful aggregate harness alone no longer verifies cache row. Validator accepts preserved current149-pass cache log and rejects13 altered cases (each of four required names missing/ignored/failed plus aggregate-only fake success). Evidence: `research/cache-ttl-required-gate-2026-10-11.json`; current preserved source log in `research/metal-integrated-current-2026-10-11/cache.log`. Validation used the existing execution log; no claim of a fresh full driver execution after this evidence-policy edit. PTX independent producer fixture remains missing despite existing PEF-compatible routing.

### Nonzero RAM TTL with live RAW leases (2026-10-11)

Added ordinary1-second TTL regression:expired RAW rejects new resident lookup while two consumer leases preserve exact sensor pixels and4-byte managed reservation. First release cannot spill; last release allows exactly one swap write, source credit zero after drain. Restored samples/metadata exact, no swap errors. Focused test and RAM adapter tests passed (28 passes). Evidence: `research/raw-lease-nonzero-ttl-2026-10-11.json` and hashed logs/source. Small authored mosaic/local filesystem; no live GPU presentation claim.

### ORF source-lease submission ordering timing (2026-10-11)

20 alternating-order measured pairs after4 warmup pairs:cached ORF upload-to-completed1920x1080 frame combined p50/p95 17.080/17.383ms; additional empty submit retaining source lease17.063/17.592ms; paired delta -0.016/0.835ms. Final managed budgets zero (example asserts before printing peaks). Both paths retain existing production queued-upload guard; this does not measure disabling that guard. No useful median advantage demonstrated; preserve lifetime ownership. Evidence: `research/orf-upload-submit-paired-2026-10-11.json` and hashed log/source. Native decode/physical presentation excluded; not a fresh per-pair independent pixel oracle. Session remains locked, physical/live HDR unverified.

### Failed-nearest RAW prefetch fallback (2026-10-11)

New explicitly executed manual real-CR3 regression verifies both directions:count1 and two-neighbor window, corrupted nearest source records one failure without consuming retained-entry slot; next valid original is stored, exactly one disk entry. All native sensor samples and metadata match direct decode after restoration; managed memory zero. Initial test incorrectly waited only for first completed attempt; corrected synchronization waits for both attempts, production worker unchanged. Evidence: `research/raw-prefetch-failed-nearest-2026-10-11.json` and preserved initial/corrected logs. Worker/local-disk coverage, not GPU presentation or independent camera appearance proof.

### Distinct-entry RAW prefetch count (2026-10-11)

RAW worker now tracks protected unique keys in a HashSet:duplicate paths/content keys record skip instead of consuming another count slot. Explicit real-CR3 test with repeated IMG_9043 and distinct IMG_9074 passes:count2/ahead3 stores two unique readable entries, expected full dimensions, no failures and final managed budget zero. Ordinary app package regression:270 passes/125 explicit ignores. Initial fixture used identical-byte copy (same content key); initial failing logs are not independent distinct-neighbor reproduction. This test checks restored dimensions, not full pixel oracle; separate tests cover pixels. Evidence: `research/raw-prefetch-distinct-count-2026-10-11.json` and preserved logs/source.

### Compute HDR restore pressure/cancellation recovery (2026-10-11)

Extended164x50 and4000x3000 Metal compute/RAM/swap pipeline:full root-budget refusal and cancellation only after nonzero restore reservation, then exact all float bits and full SDR/HDR frames on retry; credits zero after cancellation/final shutdown, one write/read, no swap errors. Both explicitly selected tests passed. Driver now requires recovery marker for both sizes; real log accepted/four altered cases rejected. Initial assertion expected Err on cancellation, corrected to documented Ok(None); production unchanged. Evidence: `research/compute-cache-recovery-2026-10-11.json` and hashed logs/source. Authored signed HDR, local disk/offscreen Metal; no physical HDR or OS pressure proof.

### Exposure compute signed f32 boundaries (2026-10-11)

New ordinary Metal test executes0/1/+32/-32 stops with admitted RGB magnitudes up to f32MAX (scaled before positive stops). Positive/negative RGB and signed zero bit-exact to CPU multiplication; negative f32MAX alpha unchanged, outputs finite, CPU/GPU zero each case. Full ordinary compute readback suite5 passed/2 explicitly ignored. Existing overflow/nonfinite rejection remains preallocation. Evidence: `research/exposure-signed-boundary-2026-10-11.json` and hashed logs/source. One-pixel buffer boundary qualification, not filtering/physical display/full-sensor qualification.

### Full sensor exposure distinct window oracle (2026-10-11)

Strengthened authored6188x4120 managed/interleaved compute field:nonperiodic green encodes position across blocks, independent blue/alpha periods; explicitly requires7 dispatches of up to4,194,240pixels with distinct expected window starts. Both variants compare all25,494,560pixels exactly, final CPU/GPU zero; peaks815,825,920/1,223,738,992bytes. Latest single-run end-to-end182.935/141.338ms, no comparative performance claim. Existing periodic1024 field could hide shifts by multiples1024; actual chunk is not a1024 multiple, so this is broader offset sensitivity, not evidence that prior exact-window boundaries were blind. Evidence: `research/full-exposure-distinct-windows-2026-10-11.json` and logs/source. No production bug, physical HDR or camera appearance claim.

### Explicit unqualified MEF color route (2026-10-11)

MEF extension now resolves to explicit unqualified-color route instead of generic DNG fallback; recipe/decode return MEF white-balance/color-support diagnostic. Generic TIFF keeps MEF choice, strong supported container magic still wins. Real pinned ZD public refusal has zero managed allocation peak and no preview; full independent native sensor comparisons remain exact. Eleven MEF tests passed with complete source/oracle environment (initial run omitted oracle variable); full decoder regression1005 passed/78 ignored. Evidence: `research/mef-explicit-color-refusal-2026-10-11.json` and hashed logs/source. Color/viewer acceptance remains pending; sensor-only import unchanged.

### RAW prefetch duplicate path avoids repeated fingerprint I/O (2026-10-11)

Added per-command path set before source fingerprinting so repeated path skips large-file hash/recipe work; separate copied paths still resolve content identity, then existing unique-key policy skips duplicates. Real four-neighbor test(count2):repeated original path, identical-content copy and second original produce two unique disk entries, four completed attempts, no failures. Both full sensor arrays and metadata exact against direct native decode; managed memory zero. Gallery ordinary regression:38 passed/14 explicit ignores. Evidence: `research/raw-prefetch-path-content-dedup-2026-10-11.json` and hashed logs/source. No comparative speedup, independent color or live GPU presentation claim.

### Post-prefetch Metal gate and robust harness output (2026-10-11)

GPU-all now captures successful output with --show-output rather than parallel --nocapture:uncaptured diagnostic stderr split the signed HDR passing harness line, causing a correct conservative evidence rejection despite zero harness failures. Strict mandatory-name regex unchanged. Fresh captured driver:139 passes/13 explicit ignores across7 suites, all execution verified:ordinaryGPU, nine-original RAW collection, mixed/multi-neighbor workers, actual RAW reversal, compute-cache SDR/HDR failure/retry, and full-sensor compute. Current numeric validator rejects16 altered/missing/ignored/failed/aggregate-only cases. Both initial rejected and final accepted reports/logs preserved in `research/metal-post-prefetch-current-2026-10-11.json` and linked directories. Explicit format/physical display/NVIDIA gaps remain.

### Model float64 extremes in HDR (2026-10-11)

Extended existing model normalization test to full64x64 binary16 HDR equality:translation(+1e12,-1e12,+1e12), scale1e300, smallest positive f64 subnormal and f64MAX signed coordinates all match baseline HDR and SDR frames exactly; baseline HDR every channel finite. All11 model readback tests passed. Evidence: `research/model-hdr-extreme-scale-2026-10-11.json` and hashed log/source. Authored triangle/offscreen targets, not full3D materials/textures or physical HDR. Production normalization unchanged.

### PNG physical pixel aspect to Metal and swap (2026-10-11)

Implemented bounded pHYs parsing before IDAT with CRC/length/duplicate/unit validation; positive densities produce pixel width/height=Y/X, zero density keeps unknown aspect. Ordinary PNG and APNG result metadata retain aspect; PNG EXIF axis-swapping rotations invert it. Decoder regression1006 passed/78 ignored including actual encoded sample preservation, ratios0.5/2, zero density, duplicate/CRC and EXIF90 cases. Explicit authored4x4 PNG Metal test:linear preparation, exact64x64 SDR/HDR against aspect0.5 reference, square-pixel control differs and analytic center/side checks, swap preserves aspect/frames, final source/queue zero, one write/read/no errors. Evidence: `research/png-physical-aspect-2026-10-11.json` and hashed logs/source/fixture. Initial GPU test incorrectly supplied encoded samples to prepared-only helper; corrected test preparation, shader unchanged. APNG propagation lacks a dedicated pHYs selected-frame case; physical display/full PNG acceptance remains open. Source: https://www.w3.org/TR/png-3/#11pHYs .

### Selected APNG physical aspect Metal gate (2026-10-11)

Authored two-frame4x4 APNG (opaque white/green SOURCE, pHYs2/1, repeat3) through common selected-frame decode:indices/count and aspect0.5 retained, both full64x64 SDR/HDR frames exactly match independent authored linear reference rasters. Two swap writes and reverse-order reads preserve metadata/frames, source/queue finalzero/errorszero. New --metal-png-physical-aspect driver option requires both PNG/APNG workload markers and actualMetal adapters; fresh gate133 passed/13 ignored across ordinaryGPU plus selectedPNG/APNG suite, every executed row verified. Evidence: `research/png-apng-physical-aspect-current-2026-10-11.json` and linked hashed logs/report/source/fixture. APNG transient canvas/scratch and direct decode_apng ownership are not yet fully request-budgeted; wrapper retained-pixel admission is insufficient proof of full peak bounds. PhysicalHDR/playback/variants remain open.

2026-10-11 unlocked desktop refresh: actual Metal live RGBA16Float / ExtendedSrgbLinear presentation passed for authored raster (173 frames / 3.010 s) and native CR3 6188x4120, 25,494,560 samples (129 frames / 3.024 s). No validation errors; source CPU credit released after RAW upload as asserted by probe. macOS reported EDR active with current headroom 8.791209, potential 16. These are presentation checks with deliberate redraw pacing, not throughput benchmarks, physical luminance/color calibration, or full viewer resize/recovery qualification. Evidence: research/metal-live-hdr-unlocked-2026-10-11.json and hashed logs.

2026-10-11 APNG memory ownership: float composition canvas and PREVIOUS saved canvas reserve before allocation; selected output freezes the same managed canvas, keeping credit until its last reference drops. Color preparation now receives the same budget and cancellation predicate. Three APNG tests passed, including exact composition oracles and insufficient canvas/color-buffer admission with final zero credits. Full decoder library results are retained in research/apng-managed-canvas-2026-10-11.json with hashed logs. Encoded frame reconstruction and PNG codec RGBA8 scratch remain untracked; this is not complete transient-memory qualification.

2026-10-11 APNG frame admission: reconstructed PNG reserves exact encoded capacity before allocation and retains adopted credit through decoding; decoder pixel output and RGBA conversion are both admitted before conversion, with managed RGBA ownership through color preparation. Pressure sweep 240..2048 bytes in steps of16 covers success/refusal, PREVIOUS restoration, final zero credits and peak <= limit. Full decoder library passed 1004 tests, 71 ignored. Initial test compilation missing a test-only Arc import was corrected; failed and passing logs preserved in research/apng-frame-admission-2026-10-11.json. PNG codec private inflater/row allocations remain controlled by codec limits rather than the shared budget; do not infer complete transient-memory accounting.

2026-10-11 APNG constructor limit and Metal refresh: image0.25.10 PngDecoder::new uses no limits; subsequent set_limits does not update png::Reader internal allocations (dependency source TODO). Use PngDecoder::with_limits before header/row initialization. Three APNG tests passed after correction (composition oracles, pressure sweep, malformed/cancel paths); actual Metal PNG/APNG pair passed2 tests with exact SDR/HDR frames before and after reverse swap restore, final budgets zero. Evidence: research/apng-constructor-limits-metal-2026-10-11.json and hashed logs. Codec internal scratch still uses its own cap rather than shared reservation; no claim of full transient accounting or physical display calibration.

2026-10-11 selected16-bit APNG: replaced unconditional RGBA8 intermediate with depth-aware RGBA16/RGBA8 conversion and doubled conversion reservation for16-bit. Earlier16-bit sources were explicitly refused (not silently truncated); now the supported selected-frame path retains low bits and alpha. Two1x1 authored sRGB frames with adjacent16-bit RGB/alpha values match independent f64 transfer-function expectations; output lifetime charged16bytes then zero. Initial regression correctly exposed the old8-bit-only guard; removed that guard for16-bit and also installed codec limits at initial metadata decoder construction. Full decoder1005 passed/71 ignored. Actual Metal suite3 passed, including two16-bit frames against independent linear SDR/HDR references and existing PNG/APNG swap cases; integrated report all executed suites verified. Evidence: research/apng-16bit-precision-2026-10-11.json and retained full logs/report/source/fixture hashes. Full16-bit variants, color profiles, playback and shared accounting of private codec scratch remain unqualified.

2026-10-11 packed APNG: removed8/16-only admission guard; PNG codec validates depth/color legality and expands packed samples. Authored1-bit grayscale,2-bit palette+tRNS(0/64/128/255 alpha),4-bit grayscale sources each have two SOURCE frames in reverse pixel order. Six selected frames match independently calculated f64 sRGB/alpha oracles, indices/count and retained64byte managed canvas; final zero credit. Full decoder1006 passed/71 ignored. Actual Metal selected PNG/APNG suite4 passed, including packed six-frame SDR/HDR comparisons before/after six swap writes/reads, errors0/source+queue0. Integrated ordinaryGPU and manual suite both verified. Evidence: research/apng-packed-depths-2026-10-11.json and retained report/log/hash directory. Full packed variants, playback and private codec shared-budget accounting remain unqualified.

2026-10-11 live resize probe preparation: opt-in RRRAH_HDR_RESIZE_PROBE=1 requests800x400 after30 presents, consumes actual window resize event, reconfigures HDR surface/viewports, checks acquired texture dimensions and requires at least30 presentations after confirmed resize. Raster zoom retained. Final probe cargo check passed. Live attempt was Occluded and Root registry reports screen locked again; no resize/presentation success claimed. Eleven current actual-Metal model readback tests passed including pressure refusal, retained SDR/HDR old frame and retry. This is separate offscreen evidence, not production live event-loop recovery. Evidence: research/metal-live-resize-preparation-2026-10-11.json and hashed logs.

2026-10-11 APNG chunk ownership: track default-image IDAT run, current animation data ownership and completion in CRC-validated scan. Refuse IDAT attached to a later animation frame, fdAT attached to default image, empty controlled frames and partial default-frame geometry before pixel admission. Four valid-CRC structural mutations refuse with zero used/peak shared credit. Positive separate static preview plus split consecutive IDAT reproduces all selected last-frame float samples and animation count4. Full decoder1007 passed/71 ignored; current actual Metal PNG/APNG four manual tests passed, SDR/HDR/swap and packed/16-bit precision markers retained. Specification: https://www.w3.org/TR/png-3/#11fcTL . Evidence: research/apng-chunk-ownership-2026-10-11.json and hashed logs/source. Lazy skipped compressed payloads are not fully inflated/validated; playback/all variants remain unqualified.

2026-10-11 application APNG TTL/cache/prefetch: ordinary and explicit actual-Metal tests use production load_cached_raster_mode/preload_raster helpers with16-bit and2-bit palette APNG sources. Each source stores two distinct selected frames under RAM128bytes/count2/TTL1second, keeps fresh entries, spills both after elapsed TTL, then restores1→0 via real prefetch while foreground decoder gate is held (5-second cancellation watchdog). RAM disabled by count0 for restore isolation; disk count2/65536bytes, queue128/count2, restore128. All serialized payloads/index/count and SDR/HDR Metal frames exact;2writes/2reads per source/errors0 and final shared credit0. Updated --metal-viewer-ttl-prefetch gate now requires new APNG suite as well as existing EXR/HDR scenario:131 ordinaryGPU passed/13 ignored plus1 HDR and1 APNG manual tests passed; all rows verified. Evidence: research/apng-viewer-ttl-prefetch-2026-10-11.json and retained report/log/hash directory. This is bounded real-time TTL and selected-frame integration, not animation playback/long collections/physical slow disk qualification.

2026-10-11 large APNG admission: replace arbitrary four-times-output512MiB dimension rejection with actual RGBA32Float output bound; managed canvas/frame/linear/PREVIOUS reservations govern simultaneous working admission. Authored4000x3000 two-frame16-bit APNG now decodes selected1 under512MiB shared limit. Full and128MiB partial pressure cause clean refusal; retry preserves all48million components within1e-7 of independent f64 sRGB oracle and bit-exact original/restored constant field. Managed decode peak480554140bytes; retained192000000; final CPU/queue0. Actual Metal128x96 SDR/HDR exact against1x1 equivalent-aspect constant reference before/after192MB swap; one write/read/errors0. Full decoder1007 passed/71 ignored; new --metal-large-apng integrated gate131 ordinaryGPU+1 manual passed/13 ignored, all rows verified. Evidence: research/large-apng-memory-metal-2026-10-11.json and retained logs/report/source/fixture hashes. Constant authored content is capacity/numeric evidence, not varied-content offset qualification; PNG private scratch excluded from tracked peak; no physical display claim.

2026-10-11 APNG PREVIOUS patch ownership: save only frame rectangle into managed buffer and restore rows in place, retaining original canvas allocation. Cancellation checked per save/restore row. Independent authored4x3 canvas/2x2 offset patch regression admits192+64bytes (full canvas copy would exceed256-byte limit), proves snapshot offsets, restores every original component including surroundings, finalzero. Full decoder1008 passed/71 ignored with existing composition and pressure oracles. New actual-Metal test selected3→1→2 disposal frames against independent5x3 float composition files; full64x64 SDR/HDR exact, finalzero. Updated PNG/APNG integrated gate131 ordinaryGPU plus5 manual passed/13 ignored, all rows verified. Evidence: research/apng-previous-rectangle-2026-10-11.json and retained report/log/hash directory. Memory savings equal patch-vs-canvas difference; runtime speedup not benchmarked. Full playback/variant/codec scratch accounting still unqualified.

2026-10-11 PREVIOUS paired timing and full-frame fast restore: on12MP canvas/64x64 region,9 alternating measured pairs after3 warmups give prior managed full-copy median38.176250ms versus production rectangle helper0.019000ms. Managed peaks384000000 versus192065536bytes; all48million components match authored position-dependent field bit-exact after both paths; final0. This isolates save/overwrite/restore/release, not full decode/viewer speedup; other host work affects timings. Preserve full-canvas PREVIOUS ownership transfer rather than copying back all samples; new3-frame regression full-white→full-green PREVIOUS→partial-blue OVER proves white base restored and untouched pixels intact, final64byte output then0. Full decoder1009 passed/72 ignored (benchmark separately executed). Current Metal gate131 ordinaryGPU+5 PNG/APNG+1 largeAPNG passed/13 ignored, all rows verified. Evidence: research/apng-previous-paired-current-2026-10-11.json and retained paired/raw logs/report/source hash.

2026-10-11 raster foreground prefetch priority: protect each successfully cached nearest-first raster neighbor with CacheLease until the current neighbor queue finishes or a new foreground request replaces it. Clear protections before idle wait/new request, leaving TTL/normal eviction active afterward. Reuse extracted raster_display_key for lookups and protection, including assumed-color/scalar interpretation; scalar fallback protects actual loaded window. Regression with visible16-bit APNG plus nearest palette APNG/count2 proves farther raster cannot evict nearest while protected, nearest RAM hit bypasses decoder, release allows farther replacement, final shared credit0. All210 ordinary app tests passed/57 ignored. Current integrated actualMetal131 ordinaryGPU + navigation + HDR TTL + APNG TTL manual suites passed, all rows verified. Evidence: research/raster-prefetch-priority-2026-10-11.json and retained report/log/hash directory. Priority regression is bounded; no claim all queue/failure/byte-cap cases or RAW/model parity. Lease lookup currently repeats sampled source fingerprint IO; no speedup timing claim.

2026-10-11 raster priority count/byte/failure coverage: expanded loader/cache retention regression to count2/1MiB and independent80-byte/no-count policies. Real truncated PNG prefetch fails without cache membership or retained credit, leaving only selected16-byte APNG; next64-byte palette frame admitted and leased, farther64-byte candidate cannot displace it. Nearest hit bypasses decoding; releasing protection permits farther replacement; final root0. All210 ordinary app tests passed/57 ignored. Production unchanged from prior actualMetal navigation/TTL report; no redundant GPU run claimed. Evidence: research/raster-prefetch-count-byte-failure-2026-10-11.json and hashed log/source. Equivalent model/foregroundRAW scoped queue protection remains to audit; this helper test does not prove complete scheduler/collection behavior.

2026-10-11 model prefetch priority: foreground neighbor worker retains successfully cached models using scoped CacheLease protection, cleared alongside raster protection at new request/idle waiting. Shared model_display_key helper preserves path/fingerprint/subimage identity for cache and prefetch lease. Regression STL/OFF/OBJ/PLY under count2 and independent two-model byte cap: real corrupt STL fails without membership, nearest retained against farther candidate, nearest hit bypasses decoder, release permits farther replacement, final shared credit0. All211 ordinary app tests passed/57 ignored. Current actual-Metal gate ordinaryGPU+mixed navigation+model upload worker suites verified; logs retain queue replacement/cancellation/source-lifetime evidence. Evidence: research/model-prefetch-priority-2026-10-11.json and retained report/log/source hashes. Small authored helper scenarios are not full scheduler/collection qualification; foregroundRAW equivalent protection and redundant fingerprint IO remain to audit.

2026-10-11 foregroundRAW scoped prefetch priority: preload_raw_with_lease returns admission plus optional resident CacheLease using already resolved key; worker retains admitted RAW neighbors until queue completion/new request, clearing beside raster/model protections. Boolean test wrapper preserves prior API/admission semantics including TTL0. No extra source fingerprint IO for RAW lease. Manual native original IMG9043.CR3 copied to five distinct timestamp identities: forward/backward nearest-first plans, count2 and independent two-mosaic byte cap, corrupt neighbor rejection, farther admission refused while nearest leased, full shared-budget occupancy still allows pointer-identical nearestRAM hit, release permits farther replacement, visible retained, finalzero. All211 ordinary app tests passed/58 ignored; manual original priority test separately passed. Current actual-Metal ordinaryGPU+mixed navigation+RAW worker navigation suites verified. Evidence: research/raw-scoped-prefetch-priority-2026-10-11.json and retained report/log/source hashes. One camera source and helper scenarios do not prove all cameras/full scheduler/long collections; disk writer persistence priority remains separate.

2026-10-11 single-resolution raster/model prefetch protection: lazy FnOnce cache-key resolver after cancellation check supplies same key to load and returned CacheLease; worker consumes returned lease instead of rereading source fingerprint. Scalar-window fallback and assumed-color flag use actual resolved load identity. Ordinary regression proves cancelled requests never invoke key resolver/decode and returned STL/APNG lease shares exact loaded allocation/subimage; finalzero after last lease. Initial refactor made legacy helper acquire lease, eagerly removingTTL0 entries and failing4 linear-import tests; explicit retain mode fixes legacy semantics, failed/passing logs preserved. All213 ordinary app tests passed/58 ignored. Current actual-Metal ordinaryGPU+navigation+model worker+HDR/APNG TTL suites all verified. Evidence: research/prefetch-resolved-key-2026-10-11.json and retained report/log/source hash directory. Removes redundant sampled fingerprint IO, but no whole-viewer timing speedup claimed.

2026-10-11 current prefetch large-collection Metal refresh: 100000 hardlink paths to four small authored raster sources, 512 navigation visits, exact128x96 SDR/HDR frames, scoped nearest-first leases, full managed-budget pressure and stale cancellation; scan/navigation8.1918s, total78.45s including path setup/cleanup. Separately eight spatially varying12MP float frames completed110 swap restores in120.230s, full12MP SDR exact and63x65 HDR exact each cycle; 110 mid-batch cancellations; writes remain8; final managed/queue0. OrdinaryGPU131 passed/13 ignored, sustained manual1 passed. Evidence: research/metal-collection-priority-refresh-2026-10-11.json and hashed report/log directory. Not100k independent source decodes, full-resolution HDR, physical slow disk, OS-wide pressure or physical display calibration.

2026-10-11 live HDR resize after user unlock reply: two actual Metal launches configured RGBA16Float but surface remained Occluded; neither presented a frame. Resize remains unverified. Logs/hashes: research/metal-live-resize-unlock-attempt-2026-10-11.json.

2026-10-11 full-resolution HDR collection: eight spatially varying authored12MP float frames on AppleM4Max/Metal; 150 successful restores and150 admitted mid-batch cancellations across120.803s. Every4000x3000 SDR byte and HDR binary16 component equals pre-spill render; CPU components bit-exact against authored field. Writes remain8; final managed/queue0. New explicit --metal-collection-full-hdr requires duration; driver sets environment deterministically, reports output extent and rejects missing workload markers. Separate actual driver gate131 ordinaryGPU passed/13 ignored plus1 manual collection passed, output4000x3000 verified; invalid flag combination rejected. Evidence: research/metal-full-hdr-collection-2026-10-11.json and retained hashes/logs. Render equality is not independent color/renderer fidelity, physical HDR, diverse decoder qualification, slow-disk or OS-wide pressure. Partial concurrent GPU qualification means elapsed soak duration is not a throughput benchmark.

2026-10-11 collection evidence audit: qualification now requires exactly one matching Metal seed, complete duration, at least8 restores, read count equal to cycles, exactly8 writes, matching SDR/HDR/parallel cancellation counts, requested HDR extent and final root/queue0. Four verifier regression tests reject truncated, duplicate, wrong-resolution/duration/count/accounting/backend and skipped evidence; serial no-parallel-scratch host remains valid. Revalidated retained actual-Metal120s fullHDR,10s fullHDR and120s63x65HDR logs successfully without rerunning unchanged GPU workload. Evidence: research/metal-collection-evidence-audit-2026-10-11.json. Full100-format, independent color fidelity, physical HDR, live resize, slow disks/OS pressure and NVIDIA remain open.

2026-10-11 live HDR probe correction: total redraw attempts incorrectly counted successful presentation toward120-attempt failure threshold. Probe now bounds consecutive surface unavailability to10seconds, reset on each successful frame; diagnostic reports actual presented count. ActualMetal remainedOccluded and terminated after10.007s without validation errors or presentation; live resize still unverified. Evidence: research/metal-live-probe-unavailability-2026-10-11.json and logs. This changes only qualification example, not production viewer.

2026-10-11 fitting cache policy updates: WeightedLru::set_limits returns without cloning/sorting candidate keys when current byte/count membership already fits, after pin feasibility validation. Existing deadlines and recency remain unchanged; no expiration semantics changed. New100000-entry regression verifies zero key clones, existing no-TTL memberships remain live afterTTL0 policy change, and touched-key LRU ordering survives. Memory/cache suites192 passed/5 manual ignored; actualMetal eight12MP collection10s/full4000x3000 SDR-HDR verified by strict evidence helper. Evidence: research/cache-fitting-limit-update-2026-10-11.json and hashes/logs. Removes unnecessary planning allocation/work; no measured whole-viewer speedup claimed.

2026-10-11 broad post-policy regression: current ordinary rrrah-decode, rrrah-gpu and rrrah app library/binary/integration suites completed70 harnesses,1418 passed,0 failed,226 explicitly ignored; actualMetal required with optionalGPU disabled. App213 passed/59 ignored. Evidence: research/cross-format-cache-metal-2026-10-11.json and full hashed log. This broad regression covers implemented fixture paths; skipped manual/corpus cases remain unverified and it does not prove full100-format, independent color or physical HDR qualification.

2026-10-12 selected TIFF viewer metadata/cache/Metal: bounded complete top-levelIFD count now attached with selected index to first and later TIFF/BigTIFF pages; source pointer restored after decode before metadata/color conversion. Full DICOM signature retains precedence over arbitrary TIFF-like preamble, caught by existing DICOM oracle regression; failed/passing logs preserved. Decoder1012 passed/72 ignored. ActualMetal four authored classic/BigTIFF LE/BE gray8 files with second-pageorientation6: explicit assumed-sRGB then linear preparation, independently authored pixel references,63x65 SDR/HDR exact, count1/32byteRAM, full-budget liveRAM hit, TTL0 visible protection then spill, reverse1->0->1 restore with selection metadata, writes2/reads3/errors0 each; final CPU/queue0. Evidence: research/tiff-pages-cache-metal-2026-10-12.json. Initial test GPU refusal without color preparation and expired-lease refusal preserved in logs; these are required API semantics, not bypassed. Page-specific ICC/associatedalpha/compressed16/float and full TIFF variants remain unqualified.

2026-10-12 independent compressed TIFF precision: eight existing tifffile/imagecodecs-produced Deflate TIFF uint16/float32 files, both byte orders, original and last-page one-low-bit changed variants; all24 selected pages match authored RGB component bits and opaque alpha exactly, retain3-page metadata and ICC. Native output then ICC preparation,32 reverse2->0->1->2 swap restores,63x65 SDR/HDR transport exact, writes3/reads4/errors0 perfile and finalroot/queue0. Manifest source hashes verified. Explicit --metal-tiff-pages driver requires both tests, four classic/BigTIFF cases and all eight precision cases; actual gate131 ordinaryGPU+2manual passed/13 ignored, all executed suites verified. Evidence: research/tiff-compressed-precision-metal-2026-10-12.json and retained logs/report. Floatproducer0..1 does not qualify above-one HDR; distinct page ICC, associatedalpha and independent color correctness remain open.

2026-10-12 signed HDR TIFF and explicit linear policy: decode previously ignored assume_untagged_linear_srgb on unprofiled floatTIFF, retainingUnspecified and refusing display. Explicit opt-in now declares onlyfloatTIFF linear-sRGB; default unknown color still refuses preparation, integerpolicy unchanged, embeddedICC precedence tested with independent precisionfiles. Two authored DeflateRGBA32 TIFFs LE/BE, threepages each: RGBabove1/negative/signedzero, straight/associatedalpha, orientation6. Allnativebits match authored straight/oriented references; Metal63x65 HDR target containsfiniteabove1andnegativecomponents; SDR/HDR everycomponent equals independent authored raster. Pressure/cancel restores publishnothing then retryexact; reverse2->1->0->2 payloadbits/selection retained, writes3/reads4/errors0, finalCPU/queue0. Actual driver131ordinaryGPU+3manual passed/13ignored allverified; finaldecoder1012passed/72ignored. Evidence: research/tiff-signed-hdr-linear-policy-2026-10-12.json, authoredgenerator/manifest and retained failed/passinglogs. No independent HDRproducer/color, physicalHDR or full TIFFvariant qualification claimed.

2026-10-12 distinct page ICC qualification: authored3-page Deflate RGB8 TIFF contains identical codes with LittleCMS-generated sRGB, explicitly gamma1-edited/independentlyparsed RGB profile, sRGB. Native ICCbytes differ onlywhereexpected, first/thirdprofiles/preparedframes identical, middle differs. Both fallbackflags do notoverrideICC. Independent LittleCMS relative-colorimetric RGB8 oracle agrees after preparedfloat->sRGB encoding within1code; initial idealcurve0.00015 testfailed and preserved, replaced by actual-profile quantizedoracle ratherthan claiming exactICCfloatprecision. Four reverse1->2->0->1 swaprestores retainselection/floatbits and63x65 SDR/HDRexact; writes3/reads4/errors0; finalCPU/queue0. Fixture/oracle/manifest regenerate bitidentically with normalizedICCtimestamp/ID. Updated explicitTIFFgate requires4manualtests; actual131ordinaryGPU+4manualpassed/13ignored allverified. Evidence: research/tiff-distinct-page-icc-2026-10-12.json. Full floatingICCaccuracy, arbitraryprofiles and physicalHDR remain unqualified. Production unchanged from final1012decoderpass revision.

2026-10-12 viewer TIFF selected-page integration: actual production raster loader/cache/preload helpers retain3 page-specific keys for one TIFF with distinctICC.96byte/count3RAM, full-root-pressure RAMhits bypassdecoder,1secondTTL spillsall3 backgrounds; count0RAM then reverse1->2->0 swap preloads bypass heldforegrounddecoder ticket (watchdog). Payload/metadata/128x96MetalSDR/HDR exact, writes3/reads3/errors0, finalroot0. CPU-only marker now reports payloadonly; Metalmarker separately requiresactualadapter andSDR/HDR. All214ordinaryapp passed/60ignored; explicitTIFFgate131ordinaryGPU+4fixturemanual+1viewerhelper passed/13ignored allverified. Evidence: research/tiff-viewer-selected-cache-2026-10-12.json and preserved logs (initial misplacedtest compile failure retained). This is productionhelper integration, not liveUIeventloop/workercollection or largeTIFF qualification.

2026-10-12 rapid page navigation fix: App previously based every bracket-key step on lastdisplayedindex, so whilepage1loaded asecondforward againrequestedpage1 and reversefromvisible0 didnothing. Generation-bound pendingtarget now accumulates0->1->2 and reversesbeforecompletion without changingvisibleindex; currentterminalevent/newfile clearsit, cancellation/obsoletegeneration ignored. Scientificwindow adjustment retainspendingpage. HeadlessactualApp/queue regressions cover0->1->2->1->0, boundary, cancelledtokens, staleRasterReady/Failed, currentfailure reset. All216ordinaryapp passed/60ignored; separatelyactualMetal TIFFviewer cache/TTL/blockdecoder helper passed1 with128x96 SDR/HDRexact/root0. Evidence: research/pending-page-navigation-2026-10-12.json. Livephysicalkeyboard/window interaction remains unverified.

2026-10-12 selected ICO images: native bounded directory selector provides depth/area-ranked imageindices, preservingcodec defaultbest/tiebreak; validates4096-entry cap and payloadbounds, managedshared-budget rank scratch released beforepixels. Redirects selectedrecord/count in existing admitted source and restoresheader afterresult, no fullfilecopy; selectedindex/count retained. Three PillowPNG authored entries prove exactRGBA/alpha/dimensions and rankorder, malformed/memoryrefusal cleanup; existingBMP-backed baseline remainspassing. ActualMetal64byte/count1RAM full-pressurehit, visibleTTL thenreverse2->0->1->2 swap keepsmetadata and63x65 SDR/HDRexact, writes3/reads4/errors0/finalCPUqueue0. Decoder1014passed/72ignored; app216passed/60ignored; combinedICO/TIFFgate131ordinaryGPU+6manual passed/13ignored allverified. Evidence: research/ico-selected-images-2026-10-12.json and source/fixture/log hashes. Multi-DIB/masks, embeddedcolor and multiCUR remain unqualified; ICOrow not fully qualified.

2026-10-12 selected CUR and hotspot tie fix: boundedmanagedICOselector now supportsCUR, validateshotspots and ranksbyarea without treatinghotspotY asbitdepth. Selectedentry pixels/point/imageordinal/count staytogether; header restored afterdecode. Legacycursor normalizer usedlastmaxarea hotspot whileICOcodec couldchoosefirstmaxarea pixels whenlastdirectoryentrywassmaller; tie rule corrected toactualcodecdefault. Authored3PNG-backedentries (twolargesttieddifferentpoints+smalllast) reproduceandprotectthiscase; invalidhotspot refusal/root0/inputunchanged. Decoder1016passed/72ignored, app216passed/60ignored. Actual combinedgate131ordinaryGPU+7manual passed/13ignored allverified: CUR/ICO64byte/count1RAM fullpressurehits, visibleTTL, reverse2->0->1->2 restorekeepspoint/metadata and63x65SDR/HDRexact/finalrootqueue0. Evidence: research/cur-selected-hotspots-2026-10-12.json and hashes/logs. Multi-DIBmaskvariants andliveOS cursorpositioning remainunqualified; CURrow notfullyqualified.

2026-10-12 unlocked desktop: actual Apple M4 Max / Metal raster HDR window probe passed:176 presentations/3.015s, actual800x400 resize confirmed and146 subsequent frames; RGBA16Float/ExtendedSrgbLinear, no validation errors. macOS reported active EDR headroom11.983057/potential16. Evidence: research/metal-live-resize-unlocked-2026-10-12.json and hashed log. This qualifies the live raster probe resize/presentation, not physical luminance/color calibration or production viewer recovery/native RAW resize.

2026-10-12 native CR3 live HDR resize: Apple M4 Max/Metal, NativeRawDecoder6188x4120 (25,494,560 samples),131 presentations/3.020s; actual800x400 resize confirmed,101 subsequent frames. RGBA16Float/ExtendedSrgbLinear, no validation errors; source managed CPU credit zero after upload (probe assertion). Evidence: research/metal-live-raw-resize-unlocked-2026-10-12.json and hashed log/source. This validates native RAW live probe presentation/resize, not production viewer memory-pressure recovery or physical color/luminance calibration.

2026-10-12 current-source recovery refresh: all11 model readback tests passed with Metal required and optional GPU disabled. ExactSDR/HDR retained on depth resize admission refusal and replacement cancellation; pressure release/retry, same-size reuse and finalcredit0 verified. Inspected production resize retains surface/config until admission and schedules250ms retry. Evidence: research/metal-model-recovery-current-2026-10-12.json and hashed sources/log. Offscreen checks still do not prove production window event-loop recovery under pressure.

2026-10-12 CUR DIB selected-image qualification: new authored24bitDIB fixture includes padded rows and AND-mask transparency, independently read by Pillow as ICO. Three selected CUR images retain native pixels, matched hotspots, selection metadata through RAM64bytes/count1, full-pressure hits, visibleTTL and reverse swap; exact63x65SDR/HDR on Metal, finalroot/queue0. All3ICO/CUR integration tests passed; driver now requires third DIB workload marker/adapter. Combined actualMetal gate passed. Evidence: research/cur-dib-mask-metal-2026-10-12.json and hashed logs/fixtures. Other DIB bitdepth/mask variants and liveOS cursor placement remain open.

2026-10-12 CUR DIB depth extension: authored1/4/8bit indexed,16bitRGB555 and32bitBGRA three-selected-entry fixtures independently decoded per-entry by Pillow. Exactnativepixels, matching hotspots, partial32bitalpha orANDmask transparency survive64byte/count1RAM/fullpressurehits/visibleTTL/reverse swap and exact63x65MetalSDR/HDR; finalroot/queue0. Combined gate135passed/13ignored, all executed suites verified; driver requires allfive depth markers and adapters. Evidence: research/cur-dib-depths-metal-2026-10-12.json. Uniform-color authored fixtures do not qualify arbitrary palette packing, compression/bitfields, mixed per-pixel masks or liveOS cursor placement.

2026-10-12 mixed CUR DIB coverage: six9x3 authoredfixtures (1/4/8/16/24/32bits) vary pixels across packedbyte/row boundaries with bottom-up rows, mixedANDmask and32bitpartialalpha; per-entryPillow decode independently validates expectedRGBA. Native pixels/hotspot and exactMetalSDR/HDR preserved through full-pressureRAMhit, TTLexpiry andswap; finalroot/queue0. Combinedgate136passed/13ignored, all executed suites verified. Twoinitialtest setupfailures retained: ZEROttl beforehit andfixedexistingTTLdeadlines; corrected sequencing, no productionchange. Evidence: research/cur-dib-patterns-metal-2026-10-12.json. UncompressedBITMAPINFOHEADER only; compression/bitfields/extendedheaders/liveOScursor placement remainopen.

2026-10-12 ICO/CUR viewer selected cache: generalized existingTIFF application qualifier to production load_cached_raster_mode/load_raster_for_display_typed/preload_raster for ICO,PNG-CUR,DIB-CUR three selected entries. Separate keys survive full-rootpressure RAMhits withoutdecode,1sTTL,3swapwrites and reverseprefetch fromswap whileforegroundDecodeGate held; exactserializedpayload/hotspot and128x96MetalSDR/HDR,3reads/errors0/finalCPU0. App217passed/61ignored; combinedgate142passed/13ignored allverified (131GPU+5ICO/CUR+1ICO/CURviewer+4TIFF+1TIFFviewer). Evidence: research/ico-cur-viewer-selected-cache-2026-10-12.json. Headlesshelper evidence does not prove livekeyboard navigation.

2026-10-12 liveCUR keyboard navigation: current normalviewer onAppleM4Max/Metal SDRsurface(Bgra8UnormSrgb), cachecount1/TTL1s, actual bracketkey events producedindices0->1->2->1->0 with matching dimensions/windowtitles; targetedwindow screenshots captured (image2 visuallyinspected green). Viewerclosednormally/exit0. UnprofiledPNG-in-CUR strictmode refused as designed; explicit--untagged-color srgb enabled display. Evidence: research/ico-cur-live-navigation-2026-10-12.json and screenshots/logs. LiveSDR only; no inferredspecificswap-hit orHDRsurface/calibratedcolor claim.

2026-10-12 actual mainviewerHDR: AppleM4Max/Metal surfaceRgba16Float/ExtendedSrgbLinear selected three authoredfloatTIFFpages0->1->2->1->0 using keyboard; explicitlinear-sRGBpolicy/cachecount1/TTL1s. ActualAXwindowsizes640x480then900x650 confirmed; targetedpage3screenshots at bothsizes visuallyinspected correctrotated2x3content. Normalclose/exit0. Evidence: research/metal-live-hdr-viewer-resize-2026-10-12.json and screenshots/logs. This closes selected mainviewerHDR presentation/resize path only, not injectedpressure recovery, physicalHDR calibration or inferredswap-hit.

2026-10-12 live mainviewer model budget recovery: actualMetalHDRRgba16Float/ExtendedSrgbLinear,16MiBmodelbudget. Enlarging1100x800logical refuses13,516,800byte depth replacement with7,680,036bytesresident;39deferredretryrefusals retain model visibly. Shrink600x432then400x332 releasesdepth; reissued identical1100x800 renders model withno furtherrefusals. Before/refused/recovered/retry targeted screenshots, two large states visuallyinspected; normalexit0. Evidence: research/metal-live-model-budget-recovery-2026-10-12.json. This is managedbudget pressure/supersedingresize recovery, not OS-widepressure or autonomoussamependingrequest recovery afterexternalrelease; physicalHDR remainsuncalibrated.

2026-10-12 largefloat delayed storage: new ignored64MiB2048x2048RGBA32Float soak passed3cycles/96.628s. Eachcycle realfilewrite, full128MiBroot pressure refusal, mid-read cancellation and retry compares everyfloatbit includingnegativezero/signedHDRextremes. Disk2objects/134217856bytes bound, thirdobject evictsfirst;3writes/3reads/errors0/finalrootqueue0. Ordinarydelayedstorage4passed/5ignored. Storagegate verifier now requires marker,requested duration and>=3cycles; syntax checked, fullcombinedstoragegate notrerun. Evidence: research/large-float-delayed-swap-2026-10-12.json and hashedlogs. ArtificialdelayedlocalI/O/managedpressure only; physicalslowdrive,OS-widepressure andcolor/display remainseparate.

2026-10-12 combined storage/GPUgate completed:140passed/13ignored, all executed suites verified (131ordinaryGPU+9storage). Storage331.09s includes100000distinctkeys/100000writes/100000reads/1000pressure refusals, independentbyte/count10s modes, idleTTL/physicalfilereclamation, delayedwriter burst, admittedreadcancel/stalewaiter andlargefloat3cycles/101.586s with64MiBpayloads. Finaltrackedcredits0. Evidence: research/storage-large-float-combined-2026-10-12.json and fullhashedreports/logs. Largefloatcancellationtimer25ms doesnot independently assertallocationphase; separateordinarycanceltest doesassertpixeladmission. Localdelay/managedpressure notphysicalslowdisk/OSpressure. Currentcatalogue audit: research/goal-open-format-audit-2026-10-12.json;94subsets/6pending,0fullyqualified.

2026-10-12 largefloat cancellation evidence strengthened: replaced25mstimer with observed64MiBmanaged pixel admission and>=65536+64bytesactualSlowIo read beforecancel, assertedbothconditions. Three64MiBcycles/87.84s passed exactfloatretry/pressure/eviction/finalrootqueue0; ordinarydelayedstorage4passed/5ignored. Newpureverifier rejects oldtimermarker, duplicate/truncated/shortcycle/shortduration/wrongextent logs;7tests passed andactualnewlog accepted/oldlog rejected. Evidence: research/large-float-admitted-cancellation-2026-10-12.json and hashedlogs. No productioncodechange; currentfocusedtests do notconstitute rerun offull9-testcombinedgate. Physicalslowdrive/OSpressure remainopen.

2026-10-12 planned container prefetch Metal: selected-container qualifier now obtains restore order from shared production neighbour planner (backward2->[1,0], forward1->[2]) instead of fixed indices. Actual AppleM4Max/Metal TIFF,ICO,PNG-CUR,DIB-CUR selected payload and SDR/HDR outputs exact after nonzeroTTL/swap with foreground decoder blocked; each source3writes/3reads/errors0/finalCPU0. Two manual tests passed. Evidence: research/planned-container-prefetch-metal-2026-10-12.json and hashed logs. Headless helper integration does not establish live automatic worker telemetry or physicalHDR calibration.

2026-10-12 combined selected-prefetch Metal gate: after production automatic selected-image queue and debug telemetry, current combined run passed142/failed0/ignored13 (131ordinaryGPU+5ICO/CUR+1ICO/CURviewer+4TIFF+1TIFFviewer), all_executed_suites_verified=true. Driver now additionally requires exactly one shared-planner reverse/forward marker per TIFF/ICO/CUR fixture, preventing earlier fixed-order helper output from satisfying current qualification. Evidence: research/planned-container-metal-gate-2026-10-12.json and retained report/log hashes. Live automatic window queue, physicalHDR, NVIDIA and complete100format scope remain open.

2026-10-12 selected-container stale generation: TIFF/ICO/PNG-CUR/DIB-CUR qualifier now rejects all three planned indices under alreadyobsolete generation before swapread, RAMentry or sharedcredit (all0), while foregrounddecodegate held. Freshgeneration then restores3pages with exactpayload/MetalSDR/HDR and3writes/3reads/errors0/final0. Two current actualMetal manualtests passed; driverrequires additionalstale markers/syntaxvalid, fullcombinedgate notrerun after testonlyaddition. Evidence: research/stale-container-prefetch-metal-2026-10-12.json. Midreadcancel/livewindow stillseparate.

2026-10-12 admitted-cancellation combined storage gate:140passed/failed0/ignored13; all_executed_suites_verified=true. Nine sustainedstorage tests273.72s:100k writes+reads/1000pressurerefusals105.437s; separatebyte/count/combinedlimits; idleTTLphysicalreclaim; writerburst; stalewaiters;64MiBfloat3cycles117.519s with cancellation after actualallocation and>=64KiBreadprogress eachcycle, everyrestoredfloatbitexact, diskcount2/bytesbounded, finalrootqueue0. This supersedes prior timer-based combined cancellation evidence. Evidence: research/storage-admitted-cancel-combined-2026-10-12.json and retainedreport/log/sourcehashes. ArtificiallocalI/O/managedpressure only; physicalslowdrive/OSpressure/RSS/physicalHDR/NVIDIA/all100 remainopen.

2026-10-12 actual automatic container worker: current built normalviewer opened authored3pagefloatTIFF in isolatedonefilefolder. Productiondebug confirms automaticselectedindices ahead0->[],ahead1->[1],ahead2->[1,2], retained=true undercachecount3. ActualMetalAppleM4Max/HDRRgba16Float/ExtendedSrgbLinear; desktoplocked/occluded, no presentationclaim. Threeownqualificationprocesses stoppedwithSIGTERM; no normalcloseclaim. Evidence: research/live-automatic-container-prefetch-2026-10-12.json plusbuild/runlogs/settings/hashes. This closes initialforwardautomaticworker/configuredcount path, not livebackwardnavigation, generationreversal, physicalHDR orallsources.

2026-10-12 actualmiddle/end containerprefetch: currentnormalviewer sixevent-deadline runs(twoeach) exact index1/ahead1/behind1->[2,0],index2/ahead0/behind2->[1,0],index2/ahead0/behind1->[1]. ActualMetalAppleM4Max, retainedleases. Addedproductiondebugplan event withselected/count/direction/limits/indices. Initialfixed2sobservation lackedend-pageprefetch records, causeunresolved/originallogretained; no schedulerfixclaim. Currentbuildpassed. Evidence: research/live-middle-end-prefetch-2026-10-12.json. Desktoplocked/occluded; SIGTERMshutdown; no dynamicnavigationreversal/physicalpresentation claim.

2026-10-12 raster replacement HDR recovery: shared GPU qualifier now checks both Rgba8UnormSrgb and Rgba16Float using existing respective renderer constructors. ActualMetalAppleM4Max small+12MP manualtests2passed; ordinaryraster17passed/8ignored. Texture/queuepressure and cancellation atvalidation/admission preservepreviousframebitexact; validblueretryexact; finalGPUqueue0. InitialSDRconstructor/HDRassertionfailed retained/correctedtestonly. Sourcecolors0..1, offscreenonly; physicalHDR/livewindow andsignedHDRqualification separate. Evidence: research/metal-raster-hdr-upload-recovery-2026-10-12.json and hashedlogs.

2026-10-12 signed/above-one HDR replacement recovery: strengthened12MP authoreduniformsource now retains[4,-.5,0,1] andvalidretry[-.25,0,4,1]. ActualMetalAppleM4Max small+large2passed. Exactbinary16centers independentlyspecified; everyframecomponent exactafter texture/queuepressure andsixcancellationcheckpoints; SDRclipsasexpected; finalGPUqueue0. Existingordinary17pass is priorrevision; currentfocused2rerun only. Testonlysource/helperstrengthening, offscreen/authoreduniformcolors only; physicalHDR, independentdecoding andlivewindow remainseparate. Evidence: research/metal-signed-hdr-upload-recovery-2026-10-12.json and hashedlog/source.

2026-10-12 admitted selected-page prefetch cancellation: actualproductionhelper/manualMetalAppleM4Max testpassed1/3.44s. Twoauthored64MiB2048squarefloatpayloads understablecontainerfingerprint; observedrestoreallocation>=64MiB then generation1->2 cancelsoldpage1(nooutput/lease/RAM/root0). Newpage2restorebypassesheldforegrounddecodegate; allfloatsexact/signedHDR,128x96SDR/HDRwholeplaneequalsauthoredreference; writes2/successfulreads1/errors0/finalroot0. Watchdogboundswaiting. Sourcefile notdecoded; no minimumphysicalreadprogress/liveeventloop/physicalHDRclaim. Evidence: research/selected-page-admitted-cancel-metal-2026-10-12.json and current/initiallogs/sourcehashes.

2026-10-12 combined recovery Metal gate: currentdriver gpu-only+metal-large-upload-cancel+newmetal-selected-page-cancel completed133passed/0failed/13ignored:131ordinaryGPU+1signed12MPupload+1admitted64MiBselectedpagecancel. All3suites verified_execution=true; bothnewrecoveryworkloads requireexactmarkers/phases/backend/signedvalues/finalaccounting. Pureevidenceverifier3tests rejectmissing/duplicate/skipped/wrongbackend/range/accounting; existingSDRonlylogs cannotqualifyHDRrecovery. Evidence: research/metal-recovery-combined-gate-2026-10-12.json and fullreport/logs. Offscreenonly,13manualskips retained; no100format/physicalHDR/NVIDIAcompletionclaim.

2026-10-12 manual36MPHDRareaLOD: actualMetalAppleM4Max1passed/currentrepeat.6000square/576MBauthoredperiodicfloatinput->1000square/16MB; all1millionoutputRGBAfloats exactly[-2,8,.5,.375], excludingtransparentRGB.64x64SDR/HDRGPUplanesexactagainstauthoredsameaspectreference; outputroot16MBthen0. TwoCPUdownsampletimes0.621642/1.338911s uncontrolledload, notbenchmark. SourceCPUuntracked; GPUreadback64x64 not1millionpixels; nodecoder/physicalHDR/all100claim. Evidence: research/metal-large-hdr-lod-current-2026-10-12.json and bothlogs.

2026-10-12 productionLODqueue stale submission fix: obsoleteGenerationToken now refusesInterrupted beforethreadcreation and rechecksunderqueue lock before revision/completed/pending mutation. Regressionfailedbefore; afterchecks currentready8x8level/1024bytecredit/revision survive stale request, originalresultconsumable/final0. Worker5passed/1ignored; fullviewer218passed/62ignored; actualMetal delayedoldswapnavigation1passed/onlynewHDRsource/finalcredits0. Evidence: research/lod-stale-submit-preserves-current-2026-10-12.json plusbefore/after/fullviewer/Metal logs andsourcehash. HeadlessAPI/actualGPU only; nolivewindow/physicalHDR oroverallperformanceclaim.

2026-10-12 productionmodelqueue stale submit fix: obsolete token refusesInterrupted beforethreadcreation and rechecksunderqueue lock before revision/slot mutation. ActualMetal regressionfailedbefore atstaleacceptance withcurrentpreparedgeometry; aftertestcurrentrevision/72byteGPUcredit retained, staleinitialstartsnone. All4manualmodelworkertests passed(including1milliontriangle1920x1080SDR/HDR/swap/1000coalescedrequests/sourceleases); fullviewer218passed/62ignored. Driverrequiresnewstalesubmitmarker, currentcombinedMetal135passed/13ignored/allsuitesverified. Evidence: research/model-stale-submit-preserves-current-2026-10-12.json and model-stale-submit-metal-gate-2026-10-12 report/logs. HeadlessMetal only; nofull100/physicalHDR/NVIDIAclaim.

Current combined recheck (2026-10-12), following successful live raster/RAW HDR resize probes: 25 executed suites,580passed/0failed/90explicitignored; every executed suite verified. Includes sustained100k-key storage,64MiBfloat admitted-read cancellation/retry,selected-page prefetch,3Duploadworker andfullsensorcompute. CUDAhost-only tests do not qualifyNVIDIA. Artificiallocalstorage/managedpressure anduncalibratedHDR limitations remain. Evidence: `research/metal-storage-current-2026-10-12-final-live-summary.json`.

### 2026-10-12: current Metal HDR render-pass timing

Extended manual timestamp reporting with nearest-rank p95, maximum, warmup count and output format. Actual Apple M4 Max Metal, 4096x4096 float source to 2048x2048 RGBA16Float: 5 warmups, 30 samples, GPU pass median 0.108271 ms, p95 0.554042 ms, max 0.651125 ms. 1 passed, 0 failed; final managed GPU credit zero. Evidence: `research/metal-hdr-gpu-timestamps-current-2026-10-12.json`. Pass timestamps exclude decode/upload/CPU submission/readback/presentation; authored constant input and sequential completion do not establish whole-viewer latency or saturated throughput. No speedup claim.
