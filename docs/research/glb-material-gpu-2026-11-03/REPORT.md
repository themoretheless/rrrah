# Material texture upload and Metal mip proof

**Current application status:** native GLB selected scenes are now connected to
ordinary discovery, loader, mixed model RAM/swap policy, retained previous/next
prefetch, bounded GPU worker, camera/resize and frame submission. Earlier staged
status statements below describe intermediate component milestones. See the
final integration section for current evidence and remaining qualification limits.

Implemented `MaterialTextureUploader::upload_rgba8` and `upload_rgba16`, an immutable candidate separate from
visible model publication. This is a building block for glTF rendering, **not an
integrated GLB viewer or complete PBR qualification**. No decoder dependency was added to
the production GPU crate; callers supply admitted raw RGBA8/RGBA16 pixels.

Color roles use `Rgba8UnormSrgb`: texture loads decode RGB before averaging and
render-target writes encode RGB afterward. Data roles use `Rgba8Unorm`. Alpha
remains straight and linear in either case. Area-weighted box mip generation
covers odd extents, including edges, and handles one-dimensional tails. Each
next extent is floor(previous/2), minimum one. RGBA8 mips quantize to 8 bits per level.

The core glTF minification/magnification and wrap enum mappings are explicit;
invalid values refuse before GPU writes. Undefined filters choose linear
magnification/trilinear minification. Non-mip filters allocate only level zero
and clamp maximum LOD to zero. Sampler mapping is tested structurally; all wrap
and minification modes are not yet qualified by rendered readback.

Admission reserves all mip texel bytes and 256-byte-row-aligned queued upload
occupancy before allocating/writing. Queued credits and resident leases survive
cancellation and candidate drop until queue completion. These are logical
resource credits, **not a physical driver/RSS bound**: pipeline, sampler, view,
encoder and driver-internal storage are not included. Later draws must retain a
resource lease through submission completion. Existing visible textures are
never mutated by candidate preparation.

The reusable `MaterialTextureUploader` binds uploads to one device/queue. Its
clones share three lazy `OnceLock` pipelines: two RGBA8 encodings and linear
RGBA32Float. Uploads
without mips do not compile a pipeline. Repeated uploads retain separate texture
credits; the shared context does not retain evicted texture data. Pipeline
construction is now outside the repeated upload path after each format's first
mip upload. No wall-clock speedup is claimed without a controlled benchmark.

## Verified evidence

- [Actual Metal readback](metal-readback.log): Apple M4 Max, Metal backend;
  six material tests passed, including Naga validation and all core sampler mappings.
- 2x2 black/white RGBA checker: data mip `[128,128,128,128]`, sRGB mip
  `[188,188,188,128]`, matching independent analytic averages within one code.
- 3x1 and 1x3 white/black/white: data `[170,170,170,170]`, sRGB
  `[213,213,213,170]`, proving odd-edge contributions and linear alpha.
- Repeated Metal uploads of all six size/encoding combinations use exactly two
  shared initialized pipelines. A prior non-mip upload allocates level zero only,
  leaves both pipelines uninitialized, and releases its logical root to zero.
- Exact 2x2 logical peak 532 bytes (20 resident + 512 queued); 531 refuses
  without leaking. Queued cancellation and draw lease retention end at root zero.
- [GPU regression](gpu-regression.log): 80 passed, 0 failed, 10 manual hardware
  tests ignored; all three material Metal probes were run separately and passed above.
- [Production check](production-check.log), cargo fmt and git diff checks passed.
- [Source hashes](sources.json) pin the implementation and lockfile.

## 16-bit source precision and filtering

`upload_rgba16` converts normalized source samples to RGBA32Float. Data samples
use n/65535; color RGB uses the glTF sRGB transfer before filtering. Alpha stays
linear. This avoids both 8-bit and half-float narrowing. GPU mip generation and
sampling operate on linear float values. This is floating-point rendering, not
an integer-exact representation of arbitrary normalized rational values.

The device must explicitly enable `FLOAT32_FILTERABLE`; otherwise upload refuses
before allocation. No silent lower-precision fallback is installed. The app's
normal device initialization/GLB renderer is not connected yet. Both color and
data float32 uploads reuse one linear mip pipeline. This path requires four
times the GPU texel storage of RGBA8, plus admitted CPU float conversion storage
and aligned queued upload credit. Codec scratch limitations remain as documented
in the native decoder report. Performance remains unmeasured.

Actual M4 Max/Metal proof in the linked log:

- All 65536 source codes in all four channels survive level-zero GPU readback
  and inverse transfer/rounding for both data and sRGB roles.
- First 2x2 source footprint codes 0,1,256,257 have analytic average 128.5;
  float mip agrees within 1e-9, with color RGB using /12.92 and alpha unchanged.
- The native RGBA16 PNG adapter feeds actual GPU uploads for Normal/BaseColor;
  every sample agrees with the independent LE16 fixture oracle after readback.
- 2x2 logical managed peak is 656 bytes (64 CPU conversion + 80 GPU mips +
  512 queued); 655 refuses without leaks, 656 succeeds and retains only 80.
- Cancellation after queue write, invalid dimensions including u32::MAX, bad
  sample count and feature-unavailable refusal release all logical credits.
- Source-to-GPU shared roots return to zero after resources and requests drop.

## Material draw pass and native source-to-frame proof

`MaterialRenderer` now prepares immutable expanded triangle draws from world-space
`MaterialVertex` records and `MaterialParameters`. A cached pipeline is selected
by alpha mode, sidedness and winding. The caller supplies the projection,
Depth32Float attachment, five material textures and their role-specific UVs.
No model/scene is silently routed through this path yet.

The WGSL pass samples the five roles, uses base RGBA/vertex factors, MR green and
blue, AO red/strength on indirect light, emissive RGB and normal XY scale. Core
lighting is a single directional GGX lobe with Schlick Fresnel, height-correlated
Smith visibility and Lambert diffuse. Neutral diffuse environment intensity is
explicit; this is not image-based environment lighting. Roughness zero is
regularized at alpha-squared 1e-8. Output saturates at finite half-float 65504.
Color samples arrive linear via sRGB views or the admitted float32 source path.

MASK discards below the cutoff; OPAQUE ignores alpha; BLEND uses straight-alpha
Porter-Duff over, disables depth writes and requires caller sorting. Back-face
culling honors the winding selection; double-sided lighting reverses the final
back normal. Missing normals select flat derivative normals and ignore supplied
tangents. Missing tangents select a derivative UV frame; this **has not been
qualified as a replacement for MikkTSpace across general meshes/seams**.

Texture handles retain their upload-context and color/data identities. Renderer
preparation rejects another context or an incorrectly tagged role. Texture view
and sampler getters are immutable. Draws own cloned texture handles/credits;
callers retain a draw resource lease through submission completion. Geometry and
uniform bytes are admitted before buffers are created. Pipeline/bind-group/driver
storage and caller-owned depth/output/CPU preparation are not counted by these
credits. The caller must attach all managed budgets to the intended common root.
Host geometry/parameters must be finite and within |value| <= 1e15; rebase/fit
source coordinates before f32 packing. More extreme ranges explicitly refuse.

Actual Apple M4 Max/Metal readback in the canonical log:

| Case | Observed center RGBA8 |
| --- | --- |
| Emissive OPAQUE | 137,188,255,255 |
| MASK below cutoff | 0,0,0,0 |
| Straight BLEND over transparent | 71,99,137,64 |
| Dielectric direct GGX at N=V=L | 110,80,57,255 |
| Explicit tangent, derivative tangent, flat normal | 110,80,58,255 each |
| Native authored GLB + material/vertex streams + PNG decoder + draw | 34,6,25,48 |

The first cases agree with analytic linear emission/alpha/GGX expectations within
one code. MASK equality at 0.25 is opaque, while cutoff 0.25001 discards. The
native GLB case uses material-controls.glb's actual factors, normal UV1 versus
other UV0 streams, PNG bytes and straight alpha; its emissive/alpha pixel matches
an independent f64 calculation. It is an authored control, **not original producer
appearance or application navigation qualification**. Culling, winding and reversed
back lighting are checked independently. Float32 textures from 16-bit source
produce the same white-map draw as RGBA8. A linear RGBA16Float target yields
[2.494140625,1.2724609375,0.66162109375,1], matching the directional GGX reference
within half-float rounding (0.002). This is offscreen HDR radiance, not physical
HDR luminance/display calibration.

A 3-vertex draw requires exactly 464 logical bytes (288 vertex + 176 uniform):
463 refuses; 464 succeeds; a held lease preserves credits after draw drop and
releases them afterward. Cancellation after buffer creation, invalid numeric
ranges/NaNs, incompatible contexts/role tags and unsupported target formats
refuse without leaking; final shared root is zero. Naga validates the WGSL and
host size checks verify the 96-byte vertex/176-byte uniform layout.

Primary semantics and BRDF guidance:
[glTF 2.0 materials and Appendix B](https://registry.khronos.org/glTF/specs/2.0/glTF-2.0.html).
This probe is a bounded implementation proof, not full glTF conformance.

Owned scene/image preparation and actual LeaseCache composition now have
[separate native evidence](../glb-native-2026-11-03/REPORT.md#owned-glb-sceneimage-asset-and-real-cache-composition).
The existing GPU probes were rerun successfully after these native changes.

## Remaining work

Connect the owned scene/vertex/material/texture preparation to the application's
model worker, renderer selection, cache and generation publication. Preserve
per-instance transforms, reuse geometry and image data, qualify general tangent
frames/MikkTSpace, transparent ordering and full five-role sampling across a
broader independent corpus. Add original producer render comparisons, HDR window
presentation and benchmark the actual connected path. The GLB format is still
not registered as ordinary viewable content. Full multi-level/large-texture/
odd-grid and adversarial cancellation qualification remains beyond this Metal
slice. NVIDIA/CUDA remains deferred by the user.

## Selected-scene viewport component

`GlbViewportPreparer` now shares material and mip pipelines across prepared scenes.
The viewport traverses the selected hierarchy, reuses resolved primitive streams,
shares GPU images by source image and color/data interpretation, and supplies
role-specific samplers. Geometry is expanded per instance on the GPU; geometry
instancing remains an optimization. Hierarchy rebasing before composition retains
small child offsets under large root translations. Normals use inverse transpose;
tangents use forward transformation and determinant handedness. Normal maps
without supplied normals/tangents refuse pending general MikkTSpace support.

Camera updates reserve a complete uniform batch before mutation. Candidate
cancellation and admission failure preserve existing frames. Depth attachments
are admitted and leased through caller-owned submissions. OPAQUE/MASK precede
BLEND; blend center sorting does not qualify intersecting triangle transparency.
Preview lighting and orthographic projection are explicit.

[Metal viewport proof](viewport-metal.log): two actual Metal tests pass. Original
BoxTextured renders at three distinct camera angles in sRGB8 and linear HDR16;
native snapshot restoration yields byte-identical frames. HDR-to-sRGB transfer
agrees within two codes. Three instances upload one source image, and a root
translation of 1e24 yields exactly the same rebased frame. Cancellation, failed
camera admission and final logical credit release are checked.
[Readback image](viewport.png) is the native 128x128 rendered frame.

[Regressions](viewport-regression.log): mesh 100, decode 288 and GPU 80 pass
(with 9 decode/10 GPU manual tests ignored). All six material controls, including
Metal probes and shared-image sampler rebinding, pass in [the hardware log](metal-readback.log).
The five [application GLB cache controls](viewport-cache.log) also pass.

This component is staged behind an explicit unused-module allowance. Ordinary
GLB loader/worker dispatch, mixed model publication and navigation/prefetch
integration remain outstanding. These offscreen tests do not qualify a physical
HDR display, NVIDIA/CUDA, full glTF semantics, physical RSS bounds or speed.

## Mixed application cache value

`ModelAsset` provides one cache value/key space for legacy geometry and GLB.
Its spill dispatcher preserves existing native payloads and selects RRGLB001 by
magic; it does not wrap or transcode geometry. A one-entry real ImageSwapCache
control alternates OFF and GLB, verifies resident shared-storage hits, restores
both native families without invoking a source decoder, and releases the root
to zero. See [mixed cache control](mixed-cache.log). Worker dispatch/publication
still uses the legacy value; replacing that path remains outstanding.

## Application CLI integration

The production `--inspect` path now dispatches GLB through `ModelAsset::decode`,
reporting instanced triangles separately from unique primitives and decoded
images. [Inspection control](inspection.log) verifies the original BoxTextured
asset, successful final credit release, unsupported image/window refusal and
failed admission under a one-byte root without leakage. GUI loader/worker
publication remains outstanding.

## Shared asynchronous GPU worker

The existing bounded `ModelUploadWorker` now accepts both geometry and selected
GLB viewport preparations. Both families share one active job, one replaceable
pending slot and one completed-result occupancy. Submitting either family clears
old completions from both families before replacement admission. Generation
tokens, revision checks, cancellation and shutdown fence publication; the worker
releases its source asset before waking the UI. GLB mip/material contexts remain
reusable. No GLB call from the GUI event loop is installed yet.

[Actual Metal worker control](worker-metal.log) compares every SDR8/HDR16 frame
sample against synchronous preparation, checks generation-stale GLB completion
discard, replaces a completed GLB with STL in the shared queue, and checks final
source/GPU root release. [Existing worker regressions](worker-regression.log)
pass, including bounded 1000-request replacement, generation cancellation,
first-open pipeline preparation and native model SDR/HDR frame comparisons.
Production cargo check and formatting also pass.

## Ordinary viewer integration and current qualification

The production loader now uses one `ModelCache<ModelAsset>` for GLB and
STL/OBJ/PLY/OFF. Byte/count/TTL and spill/restore limits therefore cover a common
model cache rather than separate per-format pools. Both foreground loads and
retained neighbour prefetch use the same typed decoder, source fingerprint,
generation token and admission/retry machinery. GLB enters application discovery
and folder/gallery navigation. A production headless loader control confirms
OFF publication, retained GLB neighbour preparation, GLB publication with lease,
and zero final source credit.

GLB `ModelReady` events use the existing pending-upload lifecycle. GPU preparation
uses the shared bounded worker and reusable per-device material/mip context;
CPU preparation scratch uses the application's managed staging budget. The
device enables FLOAT32_FILTERABLE only when advertised, preserving explicit
16-bit filtering refusal otherwise. A candidate depth attachment and initial
camera batch are admitted before viewport publication. Successful GLB publication
releases the old legacy renderer; raster/RAW publication drops the GLB viewport.
Frame leases retain geometry, textures and depth until queue completion. Camera
and resize admission failures preserve prior resources.

- [Full app regression](viewer-app-regression.log): 362 passed, 0 failed,
  108 manual/external tests ignored. Includes production mixed loader/prefetch.
- [Focused actual Metal](viewer-metal.log): three viewport controls passed,
  including synchronous/worker SDR/HDR equality, snapshots, generation fencing,
  large-coordinate rebasing and resource release.
- [Production build](viewer-build.log): passed; only existing vendored jxl-color
  lifetime warning. Formatting and diff checks passed.
- [Live ordinary viewer](viewer-live-recovery.log): original BoxTextured was
  presented on M4 Max/Metal Bgra8UnormSrgb, then presented again after simulated
  device-loss notification and device recreation. Selection/camera state exact.
- [Live mixed navigation/prefetch](viewer-live-prefetch.log): three PNG→GLB→PNG
  roundtrips, one previous/one next, one retained neighbour per generation,
  distinct presented generations on the same device. Every PNG return used RAM
  with fallback raster decode forbidden. This guard covers raster returns; it
  does not independently forbid GLB decode in that live probe.
- [Integration manifest](viewer-integration.json) pins inputs, binary and logs.

Live windows used SDR. Linear HDR rendering is covered by offscreen controls,
not a newly qualified physical HDR display. No new latency/speed or RSS claim.
GLB snapshot restoration remains decoder-blocked in mixed-cache component tests;
the guarded ordinary-viewer GLB count-pressure probe is now recorded below; slow/cold disk and broader collections remain unqualified.
General MikkTSpace/seams, intersecting transparent geometry, broader independent
producer/render comparisons, external resources, extensions, skin/morph/animation
and point/line geometry remain open. This selected-scene integration is not full
glTF or 100-format qualification. NVIDIA/CUDA remains deferred by the user.

## Guarded ordinary-viewer GLB native swap cycling

[Reproducible runner](../../../scripts/qualify-glb-viewer-swap.py) now drives the
ordinary production loader, cache, asynchronous GPU worker and surface presentation.
It uses original BoxTextured and Box fixtures (textured/untextured), a one-entry
model RAM cache, 1 MiB managed CPU root, 2 MiB/two-object native swap, 1 MiB
write/restore budgets and 32 MiB model GPU cap. Previous/next prefetch is disabled
for this eviction probe; retained prefetch is qualified separately above.

[Result and hashes](viewer-swap-result.json): 1000 roundtrips completed on actual
M4 Max/Metal SDR. Every guarded return to BoxTextured increments native swap
read count by exactly one, then presents a distinct current generation on the
same recovered device. Source decoder fallback is forbidden after switching
away. The model GPU credit at all 1000 return checkpoints equals its initial
post-recovery baseline. This proves stable logical credits at those checkpoints,
not a physical driver/RSS or process-peak measurement.

The same runner repeats the transition with `--no-cache`. It must exit nonzero,
record the explicitly refused native decoder fallback, and record no successful
guarded roundtrip. This negative control passed: a missing cache/swap cannot
masquerade as a successful restore. The hooks exist only in debug qualification
builds and are dormant without the explicit probe environment.

Complete logs are preserved losslessly as
[positive proof](viewer-swap-positive.log.gz) and
[negative control](viewer-swap-negative.log.gz), with small readable
[positive](viewer-swap-positive-excerpt.log)/[negative](viewer-swap-negative-excerpt.log)
excerpts. The result pins binary, source fixtures and both full-log hashes.
[Full app regression](viewer-swap-app-regression.log) remains 362 passed/108 ignored;
[production build](viewer-swap-build.log), format and diff checks pass.

Reproduce with the current debug binary and an unlocked Mac:

```sh
python3 scripts/qualify-glb-viewer-swap.py /absolute/path/to/rrrah /tmp/glb-swap-proof --rounds 1000
```

This is a two-source count-pressure cycling proof, not a long-duration large
collection, slow/cold disk, latency, full producer coverage or physical HDR test.
Those gates, broader glTF semantics and deferred NVIDIA/CUDA remain open.
