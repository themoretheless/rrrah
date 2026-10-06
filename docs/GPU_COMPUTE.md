# Linear HDR compute qualification

Current locked qualification after the graphics dependency update uses
wgpu/Naga 30.0.0. Forced Metal, vendor any, with GPU optional disabled passes
74 unit/integration tests; the separately requested ignored full-sensor compute
test also passes on Apple M4 Max. Logs: `/tmp/rrrah-gpu-current-full.log` and
`/tmp/rrrah-gpu-current-large-compute.log`. The latter processes 25,494,560
pixels (407,912,960 bytes per buffer), observing five GPU-completion samples
with median 3.173 ms and maximum 3.907 ms. Its existing readback assertions
pass; these figures exclude input/output allocation and upload. This verifies
the current tested Metal contracts, not CUDA/NVIDIA or physical HDR output.

`rrrah_gpu::LinearExposureCompute` multiplies linear RGBA32F RGB by 2^stops and
preserves alpha. It encodes caller-owned separate storage buffers, using aligned
subrange bindings and bounded workgroup dispatches. It performs no tone mapping,
color-space conversion, submission, buffer allocation or physical HDR output.
The caller still owns resource admission and device buffer-size qualification.

## Small exact readback

```sh
cargo test -p rrrah-gpu --test compute_readback -- --nocapture
```

The 129-pixel test constrains storage bindings to 1024 bytes, forcing three
passes. Every pixel is checked exactly with varying HDR RGB, negative channels
and alpha; a sentinel after the output range must remain untouched. Invalid
exposure, buffer aliasing and insufficient source length are rejected.

## Full sensor-sized workload

```sh
cargo test -p rrrah-gpu --test compute_readback full_sensor_linear_exposure_chunked_readback -- --ignored --nocapture
```

Measured 2026-10-05, optimized dev profile, actual Metal Apple M4 Max adapter.
The optional test uses 6188x4120 (25,494,560) synthetic linear HDR pixels, with
407,912,960 bytes each for source and output. It requests the necessary maximum
buffer size and processes the full range through the normal chunking API.
Source upload/device/pipeline setup precedes timing and a completed setup
submission. Three warmups precede five measured iterations. The timer covers
command encoding, submission and blocking completion; sampled readback is
outside timing. It is not a GPU timestamp/kernel-only measurement.

Sorted measured milliseconds: 2.329500, 2.370792, 2.394334, 2.644792, 6.996791.
Median: 2.394 ms; maximum: 6.997 ms. Five samples do not qualify tail latency.
Readback verifies the first, middle and last pixel exactly, including a varying
RGB pattern that checks source offsets, HDR output above one, negative values
and unchanged alpha. Only those three samples are checked at this resolution;
full-range exact readback belongs to the small constrained-binding test.
Evidence: `/tmp/rrrah-full-resolution-compute.log`.

This is a synthetic full-resolution compute workload. It does not establish RAW
request-to-visible latency, viewer compute integration, physical HDR display,
CUDA support, NVIDIA execution, sustained workloads or a global GPU budget.

## Linear HDR raster intermediate

`RasterRenderer::new_linear_hdr(&device)` uses the production raster sampling,
straight-alpha composition and exposure shader with an RGBA16F attachment. It
bypasses attachment sRGB transfer and normalized-target clipping, preserving
negative and greater-than-one channels within binary16 precision/range. Callers
must keep exposed/composited values in that range; this is not lossless RGBA32F
storage. The original sRGB constructor retains its existing display behavior.

`cargo test -p rrrah-gpu --test raster_readback -- --nocapture` passed four tests
on Metal Apple M4 Max. The new float-target test verifies exact binary16 output
for `[8, 4, -1, 1]` after exposure; existing channel/row, alpha and sRGB reference
tests remain enabled. Evidence: `/tmp/rrrah-linear-hdr-readback.log`. The new
constructor is not selected by the current window path; physical HDR output,
primaries, transfer and luminance negotiation remain unqualified.

## Managed raster upload regression (2026-10-05)

The complete regular `rrrah-gpu` suite passed on Metal Apple M4 Max after
RasterPixels migrated to ownership-bound PixelBuffer storage: 67 tests passed,
one full-resolution compute test remained intentionally ignored. Hardware
integration tests exercised RAW color, orientation, raster alpha/channel layout,
model rendering, chunked exposure compute and the linear HDR attachment.
The raster gate now requires hardware through `qualification_gpu`; only an
explicit `RRRAH_GPU_OPTIONAL=1` permits a missing-adapter skip.

The HDR attachment regression adopts its 1x1 RGBA32F input into a 16-byte CPU
budget, uploads it, drops the CPU frame before drawing and verifies the CPU
reservation is released. The existing exact RGBA16F readback then still verifies
[8, 4, -1, 1] after one exposure stop. GPU resource memory is separate and remains
unbudgeted; this establishes CPU/GPU upload ownership, not GPU quota enforcement,
physical HDR presentation, CUDA or NVIDIA qualification.

## Raster texture admission

`RasterRenderer::new_with_budget` and viewer `--raster-gpu-mb` apply a separate
MemoryBudget to renderer-owned RGBA32F texture footprint. Upload reserves before
creation, retains the old reservation until replacement and keeps the old image
on refusal. A Metal regression exercises 16-byte initial upload, refusal of a
32-byte replacement under a 32-byte budget, successful 16-byte replacement with
32-byte overlap peak, and release on clear/drop. This accounts logical owned
texture bytes only; GPU command retention and staging are not covered.

## Model GPU admission

ModelRenderer::new_with_budget and viewer --model-gpu-mb reserve vertex buffer
and Depth32Float attachment footprints before creation. A Metal regression uses
a 100-byte budget: one triangle (36 bytes) plus 4x4 depth (64 bytes) fits exactly;
vertex replacement and 2x2 depth replacement refuse while both old resources are
retained. Same-size depth reuse succeeds without new admission. Clearing geometry
releases 36 bytes, depth resize then succeeds with overlap admission, and renderer
drop releases all reservations. This excludes uniforms, staging, driver and
in-flight resource retention, and does not establish physical VRAM limits.

## Shared raster/model GPU parent

Viewer --gpu-memory-mb supplies parent admission to both renderer budgets; local
--raster-gpu-mb/--model-gpu-mb caps continue to apply. A Metal test fills 68 bytes
with one raster texel (16), one triangle (36) and 2x2 depth (16). Raster replacement
refuses under parent exhaustion. After clearing raster, depth replacement still
refuses under the model's 52-byte child cap despite parent room. Clearing vertices
allows depth replacement and subsequent model/raster upload; peak stays 68 and
renderer drop releases all managed credit. Unmanaged GPU resources and physical
VRAM remain outside this contract.

## RAW atlas admission

RawRenderer::new_with_budget and viewer --raw-gpu-mb admit the complete R16Uint
atlas plan, including halo padding and all array layers, before creation or
parameter mutation. RAW now joins the viewer --gpu-memory-mb parent. A Metal
regression uses explicit 32-pixel tiles/2-pixel halo, verifies atlas bytes exceed
sensor bytes, refuses larger replacement while preserving the old resource,
checks old+new overlap peak and releases credit on clear/drop. Source tile packing,
staging, uniforms, driver and in-flight retention are still outside the budget.

## RAW CPU packing admission

RawRenderer::with_upload_memory_budget is wired to the viewer shared managed CPU
budget. It reserves one halo tile plus aligned-row owned copy before packing,
reuses the reservation across sequential atlas layers and admits extra actual
capacity if necessary. The 32-pixel tile/2-pixel halo Metal regression checks
11808 bytes of peak packing credit (2592-byte u16 tile plus 9216-byte row copy),
zero retained packing bytes, refusal under 11807 bytes and subsequent retry.
Parameter changes stay local until upload commits. This does not include queue
staging, driver allocations or GPU in-flight retention.

## Explicit API selection (2026-10-05)

The viewer accepts `--gpu-backend auto|metal|vulkan|dx12|gl`. The public
`rrrah_gpu::GpuBackend` masks instance discovery to the selected API and refuses
an API absent from the compiled/platform backend set before instance creation.
Explicit selection never falls back to another API. Actual device availability
is checked separately by adapter acquisition. Main and telemetry windows use the
same selection. Main-window logs identify actual API, name, vendor and device.
API selection is independent of the vendor filter described below.

GPU readback/compute tests and the view-timing example use the same mask through
`RRRAH_GPU_BACKEND`. Invalid values fail. Hardware qualification can now be run
with a specified API:

```sh
RRRAH_GPU_BACKEND=metal cargo test --locked -p rrrah-gpu -- --nocapture
RRRAH_GPU_BACKEND=vulkan cargo test --locked -p rrrah-gpu -- --nocapture
```

The Metal command passed 72 tests on actual Apple M4 Max (44 unit, 28 integration;
one large optional compute test ignored). This includes RAW camera colors,
raster HDR readback, model/depth buffers, resource-budget lifetimes and compute
exposure. Adapter logs confirm Metal; this is not physical display qualification.
`RRRAH_GPU_BACKEND=dx12 RRRAH_GPU_OPTIONAL=1` fails the small compute test on this
host with unavailable-backend error rather than falling back/skipping.
Evidence: `target/bench/gpu-backend/metal-tests.log` and `dx12-denied.log`.
Vulkan/NVIDIA execution needs compatible hardware/drivers and is not qualified
by this host. CUDA is a separate unimplemented compute backend and is rejected
as a wgpu API name; no CUDA/NVIDIA execution claim is made by these results.


## Explicit NVIDIA adapter filter (2026-10-05)

`--gpu-vendor any|nvidia` is independent of `--gpu-backend`. `any` preserves wgpu
adapter selection. `nvidia` enumerates adapters in the configured instance,
filters reported vendor ID 0x10de and surface compatibility, rejects CPU/fallback
adapters, then prefers discrete hardware for high-performance requests (integrated
hardware for low-power requests). Numeric filtering does not depend on names.
The ID follows [NVIDIA's vendor-ID documentation](https://developer.nvidia.com/docs/drive/drive-os/7.0.3/public/drive-os-linux-sdk/production-deployment/calibration.html).
No matching adapter is an error; another vendor is never substituted. Both viewer
windows use the filter, with their respective power preferences. This does not
select a particular device among several GPUs from the same vendor.

The test harness and view timing use `RRRAH_GPU_VENDOR=any|nvidia`. All GPU
readback/compute adapter creation uses the same selector. Explicit vendor absence
fails even with `RRRAH_GPU_OPTIONAL=1`, so a NVIDIA qualification command cannot
pass by skipping unavailable hardware:

```sh
RRRAH_GPU_BACKEND=vulkan RRRAH_GPU_VENDOR=nvidia cargo test --locked -p rrrah-gpu -- --nocapture
```

This command has not been qualified on NVIDIA hardware here. On Apple M4 Max,
forced Metal/any passed 73 tests (45 unit, 28 integration; one large optional
compute test ignored). Forced Metal/nvidia with GPU_OPTIONAL=1 failed the small
compute test with `no compatible GPU adapter for vendor nvidia`, proving rejection
rather than substitution. Pure candidate tests cover vendor IDs, surface
compatibility, power preference and CPU exclusion. Evidence:
`target/bench/gpu-vendor/metal-any-tests.log` and `nvidia-missing-denied.log`.
CUDA execution and actual NVIDIA Vulkan/DX12 rendering remain pending.
# Managed compute exposure and readback

Managed exposure now rejects non-finite RGBA input and RGB multiplication
overflow before CPU result allocation, GPU reservation or submission. Tests
verify positive/negative f32::MAX with +1 stop, infinity and NaN alpha are
refused with both budget peaks still zero; +/-f32::MAX at zero exposure remain
exact on Metal. Finite negative RGB and HDR values above one remain valid.
The low-level GPU-only `encode` interface keeps caller-owned sample validation.
All three compute readback tests, including both opt-in full-resolution cases,
were explicitly executed on forced Metal with no skips after this change:
`/tmp/rrrah-compute-finite.log`. This does not qualify physical HDR output.

HDR raster swap integration now verifies a managed linear RGBA32F raster
through the production asynchronous `RasterSwapCache`, shared RAM pressure
and retry, and actual Metal rendering to `Rgba16Float`. The restored values
4,2,-0.5,1 remain exact; +1 exposure renders exact binary16 8,4,-1,1.
Queue, restore and uploaded texture ownership are checked independently;
CPU and renderer texture reservations return to zero after their final owners
release. This synthetic test verifies the connected swap/render path, not
all HDR decoder profiles or physical display output. Test framebuffer and
readback allocations are outside the renderer's texture budget.
Evidence: `/tmp/rrrah-hdr-raster-swap-metal.log`, forced Metal M4 Max with
`RRRAH_GPU_OPTIONAL=0`. CPU-only CI may explicitly opt out with optional=1;
such a skip is not hardware qualification.

Full-resolution managed qualification was explicitly run on Apple M4 Max /
Metal with `RRRAH_GPU_OPTIONAL=0`: 6188 x 4120 (25,494,560) pixels, every RGBA
value checked exactly against the independent arithmetic expectation. Explicit
GPU buffer peak was 1,223,738,992 bytes (three 407,912,960-byte buffers plus
dispatch uniforms); managed CPU peak was 815,825,920 bytes (input and output).
Both budgets returned to zero after the last owners released. One end-to-end
sample took 324.801 ms, including result allocation, GPU allocation/upload,
dispatch, synchronization, readback and CPU copy; this is not steady-state
shader timing or screen presentation latency. Evidence:
`/tmp/rrrah-managed-compute-full-sensor.log`.

Command: `RRRAH_GPU_BACKEND=metal RRRAH_GPU_VENDOR=any RRRAH_GPU_OPTIONAL=0
cargo test --locked -p rrrah-gpu --test compute_readback full_sensor_managed
-- --ignored --nocapture`. This large-memory test remains opt-in in ordinary CI.

`LinearExposureCompute::execute_managed` provides a blocking exposure/readback
path with independent CPU/GPU budgets. It admits all three RGBA32F buffers
(source, output, readback) and each dispatch's 16-byte uniform before creating
GPU buffers. CPU output uses managed immutable shared ownership. Submitted work
retains its reservation through a queue-completion callback even if polling
fails. Accounting covers explicit buffer sizes, not driver/pipeline overhead
or the caller-owned input CPU storage. Existing `encode` remains the caller-
managed asynchronous interface; viewer integration of this new path is pending.

Forced Metal readback tests use 129 pixels and a 64-pixel binding ceiling to
exercise three dispatches. They verify exact HDR/negative RGB and alpha,
one-byte-short CPU/GPU refusal, zero released GPU accounting, and CPU ownership
through the last result clone. Evidence: `/tmp/rrrah-managed-compute-test.log`.
This is offscreen compute qualification, not physical HDR presentation, CUDA,
or NVIDIA execution evidence.

`LinearExposureCompute::execute_interleaved_with_cancel` accepts and returns
flat RGBA32F samples directly, matching `DecodedRaster` storage. It borrows
the input as four-channel chunks and writes readback into the final managed
float buffer; it does not allocate an array-of-pixels conversion or flattened
result copy. The existing array API shares the same validation, dispatch and
accounting implementation. Incomplete pixels fail before admission.

The existing `raw_view_timing` example supports
`--raster-view --compute-exposure STOPS SOURCE`. This runs native raster decode,
linear preparation, WGSL compute with upload/readback, raster upload and a
completed offscreen frame. `RRRAH_VIEW_CPU_MB` covers managed source/results;
`RRRAH_VIEW_GPU_MB` is shared between compute buffers and the resident render
texture. It is a blocking end-to-end benchmark, not interactive viewer compute
integration or zero-copy GPU rendering. CUDA and WGSL exposure are mutually
exclusive. Bayer RAW development is outside this raster route.

The RAW/raster viewer now has explicit HDR surface opt-in:
`RRRAH_HDR_SURFACE=1`. It requires an advertised RGBA16Float plus
ExtendedSrgbLinear pair from wgpu 30's per-format surface capabilities. A missing
pair refuses initialization. The RAW renderer switches to SceneLinear output;
the raster renderer uses its linear RGBA16F path. Default behavior remains the
existing SDR surface selection. `RRRAH_HDR_SURFACE=0` selects that default.
The linked wgpu 30 Metal HAL maps ExtendedSrgbLinear to extended linear sRGB and
enables CAMetalLayer EDR for HDR color spaces. Capability advertisement is not
proof that the connected display is actively displaying HDR. Live surface/window
configuration, display headroom, SDR UI luminance and physical color/HDR behavior
remain unqualified. Only synthetic format-selection policy plus existing actual
Metal offscreen RAW/raster rendering regressions have passed for this change.
