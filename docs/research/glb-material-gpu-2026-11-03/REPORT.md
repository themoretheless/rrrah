# Material texture upload and Metal mip proof

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
- [Source hashes](sources.json) pin the implementation and unchanged lockfile.

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
