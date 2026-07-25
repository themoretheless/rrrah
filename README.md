# Rrrah

Rrrah is a native Rust viewer whose first displayed image is developed from the
sensor mosaic itself. It never substitutes the embedded JPEG for the main view.

The current fast paths are deliberately bounded and measurable:

1. parse Canon EOS R8 CR3/CRX, TIFF/DNG, or a TIFF-family camera RAW (CR2,
   NEF, ARW, ORF, PEF, RW2, RAF) and decode the sensor samples in native Rust;
2. cache that decoded mosaic;
3. upload it once as an integer GPU texture;
4. normalize, demosaic, white-balance, color-convert, and tone-map only the
   visible viewport in WGSL.

The CR3 backend accepts the confirmed full-resolution, one-tile, 14-bit Canon
EOS R8 profile. The DNG backend accepts bounded classic-TIFF and BigTIFF CFA
DNGs in either byte order, with 8–16-bit uncompressed or lossless-JPEG
strip/tile storage. It applies `LinearizationTable`, preserves levels, crop,
orientation, white balance and `ColorMatrix1`, and currently requires a 2×2 RGB
Bayer CFA for display.

Unsupported DNG features are rejected explicitly: LinearRaw/non-CFA images,
lossy JPEG, Deflate and JPEG XL compression, opcodes, `BlackLevelDeltaH/V`,
fractional display crops, and non-Bayer CFA layouts. The default build has no
external RAW-decoder dependency.

Seven camera formats share a bounded clean-room TIFF reader with the DNG
backend and decode real sensor data through the same mosaic pipeline. Each
one accepts only the storage variants it can verify and rejects the rest with
typed errors — the embedded JPEG is never substituted:

- **Canon CR2** — single-strip lossless JPEG with CR2 vertical-slice scatter.
  Other compressions, subsampled sRAW/mRAW, tile/multi-strip storage, and the
  EOS-1D two-column width quirk are rejected.
- **Nikon NEF** — uncompressed 12/14-bit MSB-packed strips and Nikon lossless
  (compression 34713: makernote Huffman tree + linearization curve, decoded
  by a native bitstream decoder modelled on rawspeed's `NikonDecompressor`).
  The Z-series high-efficiency/lossy variants, multi-strip 34713, and exotic
  curve variants are rejected.
- **Sony ARW** — uncompressed 16-bit and LSB-packed 12/14-bit rows, ARW 2.x
  cRAW block-delta, and lossless JPEG (Alpha 1+). The ARW 1.0 Huffman/curve
  encoding is rejected.
- **Olympus ORF** — `IIRO`/`IIRS`/`MMOR`/`MMSR` containers with Olympus
  12-bit packing, uncompressed 16-bit, or single-strip lossless JPEG. The
  C-series Huffman and OM System bitstreams are rejected as unverifiable.
- **Pentax PEF** — uncompressed MSB-packed 12/14/16-bit, TIFF/EP lossless
  JPEG (K-x and newer), and Pentax lossless (compression 65535: makernote
  Huffman table 0x0220, custom predictor). Multi-strip 65535, odd widths, and
  files missing the makernote table are rejected.
- **Panasonic RW2** — `IIU\0` container with 16-bit uncompressed rows, the
  Panasonic packed 12/14-bit bitstream (dcraw `panasonic_load_raw`
  semantics), or single-strip lossless JPEG. Legacy Panasonic compression
  codes are rejected.
- **Fujifilm RAF** — `FUJIFILMCCD-RAW` container with Bayer 12/14-bit
  LSB-packed, 16-bit, or lossless-JPEG storage. X-Trans, rotated Super-CCD,
  and Fuji's proprietary compressed format are rejected.

Like the DNG backend, all seven require a 2×2 RGB Bayer CFA for display.
Makernote coverage is deliberately limited (documented per format in
`crates/rrrah-decode/src/camtiff/`): white balance and color matrices fall
back to documented neutral values where the format carries no DNG-style tags.
Camera compatibility is validated by synthetic unit tests and opt-in
real-file regression tests (`RRRAH_<FORMAT>_FIXTURE`, see
[`docs/DECODE_FORMAT_AUDIT.md`](docs/DECODE_FORMAT_AUDIT.md) §4), not yet by
a licensed camera corpus.

Detailed design, equations, budgets, and benchmark protocol live in
[`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md).
The full-resolution tiled phase is implemented as a first atlas-backed step and
specified in
[`docs/TILED_PIPELINE.md`](docs/TILED_PIPELINE.md) and
[`docs/TILED_MATH.md`](docs/TILED_MATH.md); it replaces the temporary
large-RAW downsample fallback with GPU tile residency.

The production editor roadmap is [EDITOR_100.md](docs/EDITOR_100.md), with the
mathematical contract in [EDITOR_MATH.md](docs/EDITOR_MATH.md) and the canonical
benchmark matrix in [BENCHMARK_MATRIX.md](docs/BENCHMARK_MATRIX.md).
The live HUD/telemetry design is [LIVE_BENCHMARKS.md](docs/LIVE_BENCHMARKS.md),
and the twenty-role review is [BENCH_AGENT_REVIEW.md](docs/BENCH_AGENT_REVIEW.md).
The native DNG correctness and paired wall-time comparison is
[DNG_BENCHMARK_2026-07-23.md](docs/DNG_BENCHMARK_2026-07-23.md).
The parameter-sweep matrix and runnable synthetic GPU smoke are documented in
[PARAMETER_SWEEP_ARCHITECTURE.md](docs/PARAMETER_SWEEP_ARCHITECTURE.md) and
[GPU_SYNTHETIC_SWEEP_GATES.md](docs/GPU_SYNTHETIC_SWEEP_GATES.md).
The extension backlog is [EDITOR_101_200.md](docs/EDITOR_101_200.md), with its
math contract in [EDITOR_200_MATH.md](docs/EDITOR_200_MATH.md) and critical stop
conditions in [EDITOR_100_CRITIQUE.md](docs/EDITOR_100_CRITIQUE.md).

The 50-role competitor/paper/practice audit, current implementation scorecard,
parallelism model, innovation review, and production gates are consolidated in
[`docs/RESEARCH_DEEP_DIVE.md`](docs/RESEARCH_DEEP_DIVE.md). Supporting evidence is
split into [competitors](docs/RESEARCH_COMPETITORS.md),
[papers](docs/RESEARCH_PAPERS.md), and [practice/forums](docs/RESEARCH_PRACTICE.md).

The execution breakdown for ingest, scheduler/GPU residency, quality, and their
adversarial critic gates is [IMPLEMENTATION_AGENT_PLAN.md](docs/IMPLEMENTATION_AGENT_PLAN.md).
The three detailed work packages are [ingest/tiles](docs/PLAN_INGEST_TILES.md),
[scheduler/residency](docs/PLAN_SCHEDULER_RESIDENCY.md), and
[quality/critic](docs/PLAN_QUALITY_CRITIC.md).

Dependency, security, test, benchmark and lint audits are tracked in
[DEPENDENCY_UPDATE_AUDIT.md](docs/DEPENDENCY_UPDATE_AUDIT.md),
[SECURITY_DEPENDENCY_AUDIT.md](docs/SECURITY_DEPENDENCY_AUDIT.md), and
[TEST_BENCH_LINT_AUDIT.md](docs/TEST_BENCH_LINT_AUDIT.md).
The latest GPU, decoder, fuzz, cache and final adversarial reviews are
[GPU_VALIDATION_AUDIT.md](docs/GPU_VALIDATION_AUDIT.md),
[DECODE_FORMAT_AUDIT.md](docs/DECODE_FORMAT_AUDIT.md),
[FUZZ_HARDENING_AUDIT.md](docs/FUZZ_HARDENING_AUDIT.md),
[CACHE_STRESS_AUDIT.md](docs/CACHE_STRESS_AUDIT.md),
[CI_LINT_AUDIT.md](docs/CI_LINT_AUDIT.md), and
[FINAL_AUDIT_CRITIC.md](docs/FINAL_AUDIT_CRITIC.md).
The latest UI benchmark run and its explicit fixture gate are recorded in
[UI_BENCHMARK_RUN_2026-07-21.md](docs/UI_BENCHMARK_RUN_2026-07-21.md).
The folder gallery architecture, preload policy, security gates, and benchmark
contract are recorded in [GALLERY_ARCHITECTURE.md](docs/GALLERY_ARCHITECTURE.md).

## Run

```bash
cargo run --release -p rrrah -- --no-cache path/to/image.CR3
cargo run --release -p rrrah -- --no-cache path/to/image.DNG
cargo run --release -p rrrah -- --no-cache --inspect path/to/image.DNG
```

Controls: drop a supported RAW file or folder onto the window; a dropped folder
opens its first CR3/DNG/TIFF and `←`/`→` navigate the folder. Mouse wheel zooms, left-drag
pans, `+`/`-` changes exposure, `F` returns to fit, and `R` resets the view.
The in-image HUD reports decode/cache/adapt/upload/open timings and a live frame
encode sample.

### CR3 streaming buffer tuning

The “four buffers” often visible in CR3 diagnostics are the four independent
parity planes (R, G₁, G₂, B), not a user-selectable queue count. The streaming
assembler defaults to 32 rows per batch and queue depth 1, with two reusable
batch vectors per plane.

For repeatable experiments only, the bounded alternatives can be selected with:

```bash
RRRAH_CR3_STREAM_BATCH_ROWS=8|16|32|64|128
RRRAH_CR3_STREAM_QUEUE_DEPTH=1|2|4
```

Invalid values fall back to 32 rows / depth 1. The defaults remain the measured
low-memory choice; deeper queues reserve more scratch memory and did not show a
stable wall-time improvement on the current EOS R8 fixture.

## Status

This is an architecture-first prototype. It provides real native EOS R8 CR3,
bounded CFA DNG, and CR2/NEF/ARW/ORF/PEF/RW2/RAF full-RAW decode, full-resolution tiled GPU upload for adapters
with texture-array capacity, per-stage timing instrumentation, total wall time,
and warm-open cache. It is not yet a replacement for a color-managed production
raw developer. Additional camera profiles and DNG feature families require
separate framing, metadata and pixel-oracle validation.
