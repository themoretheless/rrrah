# Owned one-pass BMP RLE4/RLE8 — 2026-11-02

Ordinary Windows-header BMP RLE now uses an owned Rust decoder that writes
directly into the admitted RGBA output. It borrows the bounded source, keeps a
fixed 256-entry RGBA palette and allocates no command buffers or intermediate
RGB image. Existing BMP ICC and physical-aspect handling remain common to both
compressed and uncompressed images. Supported header sizes are 40/52/56/108/124.

The decoder checks encoded and absolute spans, palette indices, delta targets,
word-aligned command payloads, bottom-up dimensions and an explicit end marker.
`biSizeImage`, when nonzero, bounds the stream; the enclosing BMP profile boundary
also applies. Cancellation is checked at every command and during bounded gap
fills. Only skipped positions are initialized to opaque black, so a successful
image defines every channel without an initial whole-output fill. These commands
follow [Microsoft's bitmap compression description](https://learn.microsoft.com/en-us/windows/win32/gdi/bitmap-compression).

Row-crossing RLE8 runs now fail explicitly; the prior image decoder clipped them
for nonconforming producer compatibility. Such producer output remains an
interoperability limitation. Sparse black-fill behavior is a declared policy;
the fixtures do not establish identical behavior in every historical viewer.

## Independent samples, admission and transport

Twenty CC0 authored files cover both bit depths, encoded runs, absolute lengths
3 through 9, odd packed nibbles, word padding, horizontal/vertical deltas, row
ends, early bitmap ends and skipped regions. Every output byte independently
matches FFmpeg. The generator records its actual FFmpeg version and fixture/output
hashes in `tests/fixtures/raster/bmp-rle-reference.json`.

The full decoder library passes **275 tests, 9 ignored**. Public file decoding
needs exactly source bytes plus four output bytes per pixel in the managed
budget; one byte less refuses with final usage zero. Fixed decoder metadata is
outside that managed pixel/source measurement; this is not a total RSS bound.
All truncated prefixes and malformed run/delta/index/end-marker controls refuse
without retained output. A nonzero caller buffer proves complete gap/early-end
initialization. Palette validation exhausts all 256 counts, all 256 index values
and positions spanning words and tails; unused RLE4 low nibbles remain ignored.
Cancellation after output admission refuses publication.

The later RLE4 span matrix adds 1,044,480 encoded combinations (all 16 palette
counts, 256 packed index bytes and run lengths 1–255), plus 32,512 odd absolute
tail cases. It checks every emitted encoded pixel and leaves bytes beyond the
requested span untouched. This is test-only coverage; production decoding is
unchanged from the CPU-timed revision pinned below.

Required Metal Apple M4 Max passes **40 native/prepared storage lifecycles**:
20 independent sources, exact serialized restored payloads, exhausted-root
refusal, cancellation and retry, independent sRGB-to-linear CPU references and
31×29 SDR/HDR frames within one output code/half-float ULP, final managed root
and queue zero. These are finite offscreen checks, not live display calibration.

Evidence: [decoder](decode.log), [Metal](metal.log), [prefetch regression](prefetch.log),
[production check](production-check.log), [source pins](sources.json).
The latest prefetch group passes 66 tests (23 manual ignored), including the separate
cold-memory recovery repair described in the sustained-worker report.

## Paired CPU decode timing

The final configured Cargo test profile compares the owned and vendored image
decoders on the same authored 512×512 eight-color inputs. It alternates order,
uses four decodes per observation and records 30 pairs per group. Input generation,
file I/O, color preparation, GPU and presentation are excluded. Full output
equality is checked before timing. These are the repository's configured
workspace/dependency optimization profiles, not a release build qualification.

| Mode | CPU time reduction | Paired bootstrap 95% interval |
|---|---:|---:|
| RLE4 encoded | 59.23% | 58.38–60.05% |
| RLE4 absolute | 46.08% | 45.45–46.61% |
| RLE8 encoded | 54.07% | 52.56–55.17% |
| RLE8 absolute | 36.94% | 36.24–37.73% |

The estimator is `exp(mean(log(native_ns / legacy_ns)))`; reduction is one minus
that ratio. The 5,000-resample paired bootstrap, deterministic seeds, raw-log hash
and complete values are recorded in [timing.json](timing.json), with [raw pairs](cpu.log).
Those CPU results are pinned to revision `e905cd7a7`; the later RLE4 exhaustive
test adds no production decoder changes. The current decoder source hash and
the timing revision's hash are recorded separately.
No whole-viewer, photographic corpus or FPS gain is inferred.

The first correct prototype was rejected for an approximately 8–9× encoded-mode
slowdown. A later palette implementation still regressed RLE8 absolute controls.
Their [raw data and reverse-patch identities](rejected-prototypes.json) preserve
these negative controls. Prototype workload partitions differ, so comparisons
are only within each log's native/legacy pairs. Final code uses bulk encoded
fills, preconverted fixed palette entries, bounded word validation and writes
only required pixels/gaps; rejected prototypes are not runtime paths.

```sh
python3 scripts/generate-bmp-rle-fixtures.py
cargo test --locked -p rrrah-decode --lib bmp_rle
cargo test --locked -p rrrah-decode --lib paired_native_rle_decode_cpu -- --ignored --nocapture --test-threads=1
RRRAH_GPU_BACKEND=metal RRRAH_GPU_OPTIONAL=0 RRRAH_GPU_VENDOR=any cargo test --locked -p rrrah --test bmp_rle_metal -- --ignored --nocapture --test-threads=1
```

Broader producer interoperability, calibrated BMP RGB, complete BMP/100-format
qualification, sustained mixed-worker recovery, physical HDR and NVIDIA remain
open. These RLE controls do not supersede the earlier sustained-worker failures.
