# Material texture upload and Metal mip proof

Implemented `MaterialTextureUploader::upload_rgba8`, an immutable candidate separate from
visible model publication. This is a building block for glTF rendering, **not an
integrated GLB viewer or PBR qualification**. No decoder dependency was added to
the production GPU crate; callers supply admitted raw RGBA8 pixels.

Color roles use `Rgba8UnormSrgb`: texture loads decode RGB before averaging and
render-target writes encode RGB afterward. Data roles use `Rgba8Unorm`. Alpha
remains straight and linear in either case. Area-weighted box mip generation
covers odd extents, including edges, and handles one-dimensional tails. Each
next extent is floor(previous/2), minimum one. Mips quantize to 8 bits per level.

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
clones share two lazy `OnceLock` pipelines, one per RGBA8 target encoding. Uploads
without mips do not compile a pipeline. Repeated uploads retain separate texture
credits; the shared context does not retain evicted texture data. Pipeline
construction is now outside the repeated upload path after each format's first
mip upload. No wall-clock speedup is claimed without a controlled benchmark.

## Verified evidence

- [Actual Metal readback](metal-readback.log): Apple M4 Max, Metal backend;
  three tests passed, including Naga validation and all core sampler mappings.
- 2x2 black/white RGBA checker: data mip `[128,128,128,128]`, sRGB mip
  `[188,188,188,128]`, matching independent analytic averages within one code.
- 3x1 and 1x3 white/black/white: data `[170,170,170,170]`, sRGB
  `[213,213,213,170]`, proving odd-edge contributions and linear alpha.
- Repeated Metal uploads of all six size/encoding combinations use exactly two
  shared initialized pipelines. A prior non-mip upload allocates level zero only,
  leaves both pipelines uninitialized, and releases its logical root to zero.
- Exact 2x2 logical peak 532 bytes (20 resident + 512 queued); 531 refuses
  without leaking. Queued cancellation and draw lease retention end at root zero.
- [GPU regression](gpu-regression.log): 79 passed, 0 failed, 8 manual hardware
  tests ignored; the new Metal probe was run separately and passed above.
- [Production check](production-check.log), cargo fmt and git diff checks passed.
- [Source hashes](sources.json) pin the implementation and unchanged lockfile.

## Remaining work

Connect decoded glTF roles and sampler metadata to the model renderer and its
cache. Benchmark the actual connected path before speed claims. Add 16-bit PNG GPU
representation without reducing data to 8 bits, full material sampling/UVs,
normal maps, PBR lighting, alpha/culling, cache eviction and render comparisons.
Full multi-level/large-texture/odd-grid and adversarial cancellation qualification
remains beyond this small Metal slice. NVIDIA/CUDA remains deferred by the user.
