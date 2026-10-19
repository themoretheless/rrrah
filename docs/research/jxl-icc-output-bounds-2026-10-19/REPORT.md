# JPEG XL reconstructed ICC bounds — 2026-10-19

## Reproduced defects and change

The pinned jxl-color 0.11.0 ICC command decoder could append more bytes than
`output_size`, potentially growing its output Vec beyond the advertised size.
At EOF while decoding a tag table it also returned `Ok(out)` without checking
that length. Two tiny direct ICC command streams (129 and 144 declared bytes)
were accepted with the wrong length by the original code; both red tests are
in `before.log`. These are command-stream controls, not full malicious JXL files.

The local same-version patch checks every output append before writing. Literal,
shuffle and predictor lengths are checked before shuffle allocation. Tag-table
EOF now requires the same exact output length as the main command path. Checks
use checked addition. No output Vec can grow beyond its declared logical size.

`vendor/jxl-color/RRRAH_PATCH.md` records the registry checksum and scope.
Original licenses are retained. Only original `src/icc/decode.rs` changed; the
lockfile only changes jxl-color from registry to the same-version local path.

## Controls and runtime evidence

Four new tests cover the two previously accepted streams, 18 oversize command
cases (all expanding main command types and ordinary/implicit tag descriptors),
and three exact-length controls. The complete dependency suite has 18 tests.

A new CC0 authored 5x3 RGBA16 three-frame animation embeds the existing linear
RGB ICC fixture explicitly. `scripts/generate-jxl-embedded-icc-fixtures.py` builds
an independent libjxl C producer/decoder derived from the finite16 oracle.
The C decoder checks all 180 authored u16 samples, 2 loops, fractional timing,
and original ICC bytes. Metadata and hashes are in `jxl-embedded-icc.json`.
The ICC retains the existing fixture's provenance; CC0 applies to authored pixels
and producer code.

The public rrrah decoder preserves all three frames' samples, selection, timing,
and the codec's rendered-profile description; managed root usage returns to zero
following display preparation. The low-level original ICC is byte-identical to
the source. The codec can synthesize an equivalent recognized rendered profile:
rendered ICC bytes are not claimed identical to original ICC bytes, nor is this
an independent whole-color-pipeline accuracy certificate.

Validation logs:

- `color-tests.log`: 18 passed.
- `decoder.log`: animation 35 passed /1 ignored; decode 224 passed /5 ignored.
- `app.log`: 288 passed /81 ignored.
- `metal.log`: actual Apple M4 Max Metal, 15 frames across five JXL controls,
  SDR/half attachments, pinned RAM, native/prepared-float swap, full-pressure
  refusal/retry, metadata preserved and final managed root zero.
- `format.log`: workspace formatting; source diff whitespace check passes.

Ordinary suites: 565 passed /87 ignored. The explicitly selected Metal test is
additional. One pre-existing dependency lifetime-syntax warning remains; no
strict Clippy claim is made.

## Remaining memory and qualification gaps

This is a prerequisite for trusting declared sizes, not full ICC parent-budget
admission. The encoded/reconstructed profile buffers, entropy state, header and
TOC metadata, rendered ICC copy, allocator overhead, and parser copies still
need accounting. The existing 256 MiB encoded/decoded ICC format ceiling is
unchanged; this is not a strict heap/RSS bound. Decoder cancellation inside the
ICC parser and large-profile runtime qualification also remain open.

Full compressed JXL animation history is still loaded for each uncached selected
frame. Broader producer coverage, six pending catalog formats, physical HDR,
NVIDIA/CUDA, physical slow disks and live GUI endurance remain outside this
proof. The global goal is incomplete.


Follow-up: the current checkout adds optional working admission for encoded,
reconstructed and shuffle ICC buffers. The no-admission statement above records
this report's original milestone; see
[ICC working admission](../jxl-icc-working-admission-2026-10-19/REPORT.md)
for subsequent changes, new source hashes and verification.
