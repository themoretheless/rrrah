# Material texture upload and Metal mip proof

Implemented `MaterialTextureUploader::upload_rgba8` and `upload_rgba16`, an immutable candidate separate from
visible model publication. This is a building block for glTF rendering, **not an
integrated GLB viewer or PBR qualification**. No decoder dependency was added to
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
  four tests passed, including Naga validation and all core sampler mappings.
- 2x2 black/white RGBA checker: data mip `[128,128,128,128]`, sRGB mip
  `[188,188,188,128]`, matching independent analytic averages within one code.
- 3x1 and 1x3 white/black/white: data `[170,170,170,170]`, sRGB
  `[213,213,213,170]`, proving odd-edge contributions and linear alpha.
- Repeated Metal uploads of all six size/encoding combinations use exactly two
  shared initialized pipelines. A prior non-mip upload allocates level zero only,
  leaves both pipelines uninitialized, and releases its logical root to zero.
- Exact 2x2 logical peak 532 bytes (20 resident + 512 queued); 531 refuses
  without leaking. Queued cancellation and draw lease retention end at root zero.
- [GPU regression](gpu-regression.log): 79 passed, 0 failed, 9 manual hardware
  tests ignored; both material Metal probes were run separately and passed above.
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

## Remaining work

Connect decoded glTF roles and sampler metadata to the model renderer and its
cache; request float32 filtering where supported and expose explicit refusal
otherwise. Benchmark the actual connected path before speed claims. Implement
full material sampling/UVs, normal maps, PBR lighting, alpha/culling, cache
eviction and render comparisons. Full multi-level/large-texture/odd-grid and
adversarial cancellation qualification remains beyond this Metal slice.
NVIDIA/CUDA remains deferred by the user.
