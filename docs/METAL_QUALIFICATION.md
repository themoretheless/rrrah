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
