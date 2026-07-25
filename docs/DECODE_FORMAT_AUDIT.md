# RAW decoder / format audit

Статус: compatibility contract для `rrrah-decode`, проверено против
`rawler = 0.7.2` (2026-07-21). Документ описывает то, что реально проверяется
сейчас, и то, что ещё нельзя выдавать за production support. В тестах нет
embedded JPEG: `RawlerDecoder` всегда вызывает `Decoder::raw_image(..., false)`.

## 1. Что делает текущий adapter

Путь ingest сегодня такой:

```text
Path -> RawSource::new -> rawler::get_decoder -> raw_image(dummy=false)
     -> RawImageData::Integer -> DecodedMosaic<u16>
```

`RawSource::new` в rawler открывает файл через memory map с `populate`, а
`raw_image` материализует полный `RawImage`. Поэтому этот adapter совместим с
full-frame fallback, но не является metadata-only probe или независимым DNG tile
decoder. Это существенное ограничение для первого кадра и RSS.

Проверенные точки API rawler:

- [`RawSource`](https://docs.rs/rawler/0.7.2/rawler/rawsource/struct.RawSource.html)
  отвечает за источник байтов, но не за наши quotas и process isolation;
- [`Decoder::raw_image`](https://docs.rs/rawler/0.7.2/rawler/trait.Decoder.html)
  возвращает готовый полный `RawImage`;
- [`RawImageData`](https://docs.rs/rawler/0.7.2/rawler/rawimage/enum.RawImageData.html)
  бывает `Integer(Vec<u16>)` или `Float(Vec<f32>)`;
- [`RawPhotometricInterpretation`](https://docs.rs/rawler/0.7.2/rawler/rawimage/enum.RawPhotometricInterpretation.html)
  в 0.7.2 имеет `Cfa`, `LinearRaw` и `BlackIsZero`; неизвестный photometric не
  представим этим public enum и должен отбрасываться парсером до adapter.

Adapter намеренно:

1. проверяет generation до I/O и после каждой крупной стадии;
2. ловит Rust panic вокруг недоверенного decoder-а и возвращает
   `DecodeError::DecoderPanicked`;
3. не вызывает `thumbnail_image`, `preview_image` или `full_image`;
4. отвергает `RawImageData::Float` типизированным `UnsupportedFloatRaw`, а не
   округляет float в `u16` молча;
5. сохраняет полную CFA и black-level grid, даже если текущий GPU fast path
   умеет только 2x2 RGB Bayer.

`catch_unwind` не защищает от OOM, `abort`, бесконечного времени или ошибок
внутри unsafe/FFI. Для внешних файлов production-путь обязан иметь worker
process с лимитами RSS, CPU/wall time и длины IPC.

## 2. Семантическая матрица форматов

| Format / property | Current rawler adapter | Current rrrah contract | Release condition |
|---|---|---|---|
| Canon CR2 lossless JPEG, integer CFA | full-frame decode to `u16` | supported fallback, no JPEG preview | bit-exact corpus + truncation/fuzz suite |
| CR2 restart markers / slices | rawler-owned decode; no split API | one sequential entropy lane | restart-aware proof and differential oracle before splitting |
| DNG uncompressed strips | full-frame decode | supported fallback | independent `TilePlan` with exact `u16` oracle |
| DNG independent tiles | rawler currently materializes frame | metadata is preserved, viewport-first not claimed | bounded tile reader, offsets/counts checked |
| DNG lossless JPEG / Deflate | decoder-dependent full path | supported only where rawler succeeds | per-compression fixture and tile-vs-monolithic diff |
| BigTIFF / LONG8 / IFD8 | not separately probed by rrrah | compatibility is unverified | explicit BigTIFF corpus and checked 64-bit planner |
| DNG `LinearizationTable` | rawler may apply semantics internally | output must carry decoder ABI | table fixture, exact pre/post-linearization oracle |
| DNG OpcodeList 1/2/3 | not exposed as a rrrah capability report | never silently ignored | opcode digest, supported/degraded/rejected status |
| DNG floating-point / LinearRaw | adapter returns `UnsupportedFloatRaw` | explicit negative result; no quantization | separate linear-float pipeline + calibrated oracle |
| `BlackIsZero` photometric | metadata is preserved | GPU fast path rejects non-CFA explicitly | grayscale/raw-channel path or visible degraded mode |
| X-Trans / 6x6 / four-color CFA | metadata is preserved | `bayer_quad()` rejects fast path | quality demosaic backend and camera corpus |
| unknown photometric / malformed IFD | rawler error or panic caught | typed error, no JPEG fallback | independent bounded probe and fuzz campaign |
| Canon CR2 | rawler full-frame decode | native clean-room decode: single-strip lossless JPEG with CR2 slice scatter; typed rejection of other compressions, sRAW/mRAW, tile/multi-strip storage, EOS-1D width quirk | `cr2-canon-24mp`/`cr2-canon-52mp` corpus + exact pixel oracle (`RRRAH_CR2_FIXTURE` smoke gate in place) |
| Nikon NEF | rawler full-frame decode | native decode of uncompressed 12/14-bit MSB-packed strips and Nikon lossless (34713: makernote Huffman tree + linearization curve, native bitstream decoder per rawspeed `NikonDecompressor`); Z-series high-efficiency/lossy variants, multi-strip 34713 and exotic curve variants rejected explicitly | `nef-uncompressed` + `nef-lossless` corpus (`RRRAH_NEF_FIXTURE`) |
| Sony ARW | rawler full-frame decode | native decode: uncompressed 16-bit / LSB-packed 12/14-bit, ARW 2.x cRAW block delta, lossless JPEG; ARW 1.0 Huffman/curve rejected | `arw-uncompressed` + `arw-craw` + `arw-ljpeg` corpus (`RRRAH_ARW_FIXTURE`) |
| Olympus ORF | rawler full-frame decode | native decode: 0x4F52/0x5352 magic accepted as classic TIFF; 12-bit packed / 16-bit / single-strip LJPEG; C-series Huffman and OM System bitstreams rejected as unverifiable | `orf-12bit` corpus + 12-bit packing oracle (`RRRAH_ORF_FIXTURE`) |
| Pentax PEF | rawler full-frame decode | native decode: uncompressed MSB-packed 12/14/16-bit, TIFF/EP lossless JPEG, and Pentax lossless (65535: makernote Huffman table 0x0220, custom predictor); multi-strip 65535, odd widths and missing-makernote files rejected | `pef-uncompressed` + `pef-ljpeg` + `pef-65535` corpus (`RRRAH_PEF_FIXTURE`) |
| Panasonic RW2 | rawler full-frame decode | native decode: 0x55 magic accepted as classic TIFF; 16-bit uncompressed, Panasonic packed 12/14-bit (dcraw `panasonic_load_raw` semantics), single-strip LJPEG; legacy 34316/34826/34828/34830 codes rejected | `rw2-packed` + `rw2-ljpeg` corpus + packed-stream oracle (`RRRAH_RW2_FIXTURE`) |
| Fujifilm RAF | rawler full-frame decode | native decode: FUJIFILMCCD-RAW container, Bayer 12/14-bit LSB-packed / 16-bit / LJPEG; X-Trans, rotated Super-CCD and Fuji proprietary compression rejected | `raf-bayer` corpus + X-Trans negative case (`RRRAH_RAF_FIXTURE`) |

Important: preserving a non-Bayer CFA is not the same as supporting it. The
adapter test checks that `RRGG` is not rewritten as Bayer; the GPU tests must then
reject it with `UnsupportedCfa` and the UI must report the degraded path.

## 3. Deterministic tests already in the adapter

`crates/rrrah-decode/src/lib.rs` includes tests which do not require licensed
camera files:

- stale generation is rejected before opening a path and remains observable as
  the global generation advances;
- float RAW returns `DecodeError::UnsupportedFloatRaw`;
- a non-Bayer CFA is preserved exactly and fails `bayer_quad()` with a typed
  `FrameError::UnsupportedCfa`;
- `BlackIsZero` remains explicit metadata and is not replaced by an embedded
  preview or a fake CFA.

`crates/rrrah-decode/src/camtiff/` adds deterministic camera-format tests
built from synthetic in-memory containers (no licensed camera files): per
format they validate header/signature handling, raw-IFD selection, metadata
mapping, the pixel unpackers against hand-computed or dcraw-style reference
vectors, and every typed rejection listed in §2. The shared TIFF reader has
unit coverage for the ORF (0x4F52/0x5352) and RW2 (0x55) magics alongside the
classic (42) and BigTIFF (43) magics. These tests prove parser and decoder
arithmetic, never camera compatibility.

The test module has no fixture bytes and therefore cannot create a false claim of
camera compatibility. It currently passes with the workspace's normal
`cargo test --workspace --locked` invocation.

## 4. Licensed release corpus (exact IDs, no fake fixtures)

Every row below must be a real file referenced by a SHA-256 and licence entry in
`tests/fixtures/SHA256SUMS`. The repository must not commit camera files unless
their redistribution terms permit it. The ID is stable; the filename and hash
are supplied by the fixture manifest.

| ID | Required file property | Positive/negative | Expected check |
|---|---|---|---|
| `cr2-canon-24mp` | Canon CR2, integer 12/14-bit Bayer, no restart split assumption | positive | dimensions, CFA, exact sample count |
| `cr2-canon-52mp` | large CR2 (roughly 50 MP), cold/warm benchmark | positive | full RAW decode, bounded RSS, no preview calls |
| `cr2-restart` | CR2 with restart markers and nontrivial slice table | positive | same output as sequential oracle; no unsafe split |
| `cr2-truncated` | each JPEG marker boundary truncated | negative | typed error, no panic, bounded wall time |
| `dng-strip-uncompressed` | little- and big-endian strips, `LONG` offsets | positive | exact `u16` output and offset bounds |
| `dng-tiled-uncompressed` | independent tiles, edge tile smaller than nominal | positive | tile-vs-full max error 0 LSB |
| `dng-tiled-ljpeg` | independent lossless-JPEG tiles | positive | tile-vs-full <= 1 LSB after linearization |
| `dng-bigtiff` | BigTIFF header, `LONG8`/`IFD8` offsets, large counts | positive | checked 64-bit arithmetic; no truncation |
| `dng-float-linear` | floating samples, LinearRaw photometric | negative for current adapter | `UnsupportedFloatRaw`, never cast to `u16` |
| `dng-opcodes-123` | OpcodeList1/2/3, nontrivial black/linearization metadata | capability | supported/degraded/rejected is explicit |
| `dng-black-grid` | 2x2 and non-uniform repeat black-level grids | positive | full grid retained; no top-left-only oracle |
| `dng-xtrans` | 6x6 CFA, orientation not normal | capability | metadata exact; Bayer fast path rejects |
| `dng-malformed-ifd` | cycles, huge count, duplicate/overflowed offsets | negative | no allocation proportional to attacker count |
| `cr2-real-bayer` | Canon CR2, single-strip lossless JPEG, 2x2 Bayer | positive | `decode_file` succeeds; geometry/CFA/pixel-count invariants (`RRRAH_CR2_FIXTURE`) |
| `nef-real-uncompressed` | Nikon NEF, uncompressed 12/14-bit MSB-packed strips | positive | same invariant suite (`RRRAH_NEF_FIXTURE`) |
| `nef-lossless` | Nikon NEF, compression 34713 (single strip, documented 12/14-bit curve) | positive | same invariant suite; linearization curve applied exactly (`RRRAH_NEF_FIXTURE`) |
| `nef-zseries-rejected` | Nikon NEF, Z-series high-efficiency/lossy compression | negative | typed rejection, no misdecoded pixels |
| `arw-real-uncompressed` | Sony ARW, compression 1 (16-bit or LSB-packed) | positive | same invariant suite (`RRRAH_ARW_FIXTURE`) |
| `arw-craw` | Sony ARW 2.x cRAW block-delta (compression 32767) | positive | same invariant suite; cRAW oracle digest |
| `arw-huffman-rejected` | Sony ARW 1.0 Huffman/curve | negative | typed rejection, no misdecoded pixels |
| `orf-real-12bit` | Olympus ORF (`IIRO`/`IIRS`), 12-bit packed | positive | same invariant suite (`RRRAH_ORF_FIXTURE`) |
| `pef-real` | Pentax PEF, uncompressed packed or TIFF/EP LJPEG | positive | same invariant suite (`RRRAH_PEF_FIXTURE`) |
| `pef-65535` | Pentax PEF, compression 65535 (single strip, even width, makernote table 0x0220) | positive | same invariant suite (`RRRAH_PEF_FIXTURE`) |
| `rw2-real-packed` | Panasonic RW2 (`IIU\0`), packed 12/14-bit or 16-bit | positive | same invariant suite (`RRRAH_RW2_FIXTURE`) |
| `raf-real-bayer` | Fujifilm RAF, Bayer 12/14-bit LSB-packed or 16-bit | positive | same invariant suite (`RRRAH_RAF_FIXTURE`) |
| `raf-xtrans-rejected` | Fujifilm RAF, X-Trans 6x6 | negative | typed rejection; not mislabeled as Bayer |

The current shell fixture helper verifies `SHA256SUMS`; before release it should
also verify a sidecar manifest with format, expected metadata, licence, decoder
version and oracle digest. A missing manifest row is a release failure, not an
invitation to generate a synthetic file and call it a camera test.

### Fixture-gated commands

The adapter tests are opt-in for real files:

```sh
RRRAH_CR2_FIXTURE=/licensed/path/sample.CR2 \
  cargo test -p rrrah-decode configured_cr2_fixture_decodes_sensor_samples --locked

RRRAH_NEF_FIXTURE=/licensed/path/sample.NEF \
  cargo test -p rrrah-decode configured_nef_fixture_decodes_sensor_samples --locked

RRRAH_ARW_FIXTURE=/licensed/path/sample.ARW \
  cargo test -p rrrah-decode configured_arw_fixture_decodes_sensor_samples --locked

RRRAH_ORF_FIXTURE=/licensed/path/sample.ORF \
  cargo test -p rrrah-decode configured_orf_fixture_decodes_sensor_samples --locked

RRRAH_PEF_FIXTURE=/licensed/path/sample.PEF \
  cargo test -p rrrah-decode configured_pef_fixture_decodes_sensor_samples --locked

RRRAH_RW2_FIXTURE=/licensed/path/sample.RW2 \
  cargo test -p rrrah-decode configured_rw2_fixture_decodes_sensor_samples --locked

RRRAH_RAF_FIXTURE=/licensed/path/sample.RAF \
  cargo test -p rrrah-decode configured_raf_fixture_decodes_sensor_samples --locked

RRRAH_DNG_FIXTURE=/licensed/path/sample.DNG \
  cargo test -p rrrah-decode configured_dng_fixture_decodes_sensor_samples --locked

RRRAH_DNG_FLOAT_FIXTURE=/licensed/path/linear-float.DNG \
  cargo test -p rrrah-decode configured_float_dng_fixture_has_an_explicit_typed_result --locked
```

If an environment variable is absent the test reports a skip; it never invents
fixture bytes. The camera-format tests decode through the production router
and assert routing, geometry, CFA, levels and pixel-count invariants only —
an exact pixel oracle additionally requires the staged manifest above. CI
with a licensed corpus should set all variables and fail if any is missing.

## 5. Differential oracle

The oracle is linear sensor data, not an embedded JPEG. For each positive
fixture, record:

1. source SHA-256, file size, endian and format;
2. dimensions, cpp, bits, CFA phase, orientation, black/white levels,
   linearization digest and opcode digest;
3. a canonical little-endian `u16` sensor dump from a trusted reference;
4. rawler version and rrrah decode ABI;
5. per-stage checksum (entropy output, linearized output, tile output).

Use at least two independent references where licensing permits:

- [LibRaw](https://www.libraw.org/docs) for broad camera compatibility;
- [RawSpeed](https://darktable-org.github.io/rawspeed/) for lossless RAW decode;
- an independently reviewed DNG/TIFF reader for BigTIFF offsets and OpcodeList
  semantics.

Reference outputs must be converted to the same sensor coordinate convention
before comparison. Do not compare display RGB or gamma-encoded JPEGs. For
uncompressed integer DNG the expected maximum absolute error is exactly 0 LSB;
for compressed lossless paths it is <= 1 LSB after the documented
linearization/rounding step. Tile output must be independent of worker order and
must match a monolithic decode at every tile boundary.

## 6. Fuzz and hostile-input checks

The decoder boundary is untrusted input. The minimum mutation set is:

- TIFF and BigTIFF endian swaps, malformed type/count, IFD cycles, nested
  SubIFDs, `LONG8` overflow and huge array counts;
- DNG duplicate/overlapping tile offsets, wrong byte counts, edge padding,
  predictor mismatch, malformed OpcodeList and float NaN/Inf;
- CR2 truncation at every JPEG marker, invalid DHT/SOS, restart interval and
  slice-count mismatches, dimensions inconsistent with the payload;
- cancellation at every scheduler checkpoint, generation reordering and
  partial cache writes.

Each input must terminate with either a deterministic validated output or a
typed error. “No panic” is not enough: the worker must also stay below an RSS
budget and a wall-time limit. `catch_unwind` is a diagnostic guard; process
isolation is the production boundary.

## 7. Critic stop conditions

Do not mark decoder/format work complete when any of these is true:

1. a test invokes `thumbnail_image`, `preview_image` or trusts an embedded JPEG;
2. an arbitrary CR2 byte range is decoded in parallel without a restart-marker
   proof and an equality oracle;
3. DNG tile offsets/counts are multiplied in unchecked `usize` arithmetic;
4. float/LinearRaw data is silently rounded into integer Bayer samples;
5. unsupported CFA or photometric data is mislabeled as 2x2 Bayer;
6. OpcodeList, black grids or linearization are ignored while reporting
   “full-quality RAW”;
7. a stale generation can publish after a newer request, even if the pixels look
   plausible;
8. a malformed file can allocate from untrusted width/height/count before
   quotas are checked;
9. a benchmark reports only mean throughput and omits p95/p99, RSS, cancellation
   and cold/warm page-cache state;
10. the corpus lacks a hash/licence/metadata manifest or uses a synthetic file
    as evidence of camera compatibility.

The adapter is therefore a useful compatibility fallback, not yet the final
P0 ingest implementation. The next implementation step is a bounded metadata
probe plus a DNG `TilePlan`; rawler full-frame decode remains the explicit
fallback until that path has the corpus and oracle above.
