# Local bounded ICC output patch

Based on crates.io `jxl-color` 0.11.0, checksum
`f316b1358c1711755b3ee8e8cb5c4a1dad12e796233088a7a513440782de80b2`.
Original MIT and Apache-2.0 licenses are preserved. Modified original files:
`src/icc/decode.rs`, `src/icc/synthesize.rs`, `src/icc.rs`, `src/error.rs` (apart from this note and
omitted Cargo cache markers).

Every reconstructed ICC output append checks the declared output size before
writing. Shuffle/predictor command lengths are checked before allocating shuffle
scratch. EOF in the tag table also verifies the final output length, instead of
returning a short or oversized successful profile.

Four regression tests exercise two previously accepted malformed streams,
all expanding command families, and exact-length valid controls. The optional
256 MiB format ceiling is unchanged. This patch does not add parent memory-budget
admission for ICC entropy state, encoded/reconstructed profile buffers, parsing,
or the rendered ICC copy; those remain separate work.

## Follow-up: optional working-buffer admission

`read_icc_with_admission` reports `encoded_size + 18` before entropy parsing and
`encoded_size + 2 * decoded_size + 18` after the quick ICC size header, before
remaining encoded storage or reconstruction. The 18-byte term covers overlap
with the small initial header during exact reallocation; the second decoded
term covers the largest single shuffle scratch buffer. Growth uses
`try_reserve_exact` rather than geometric capacity growth.

The legacy `read_icc` uses a no-op callback. New `Error::AllocationDenied` retains
callback/allocation errors; jxl-oxide unwraps callback errors for typed caller
handling. Additional modified original files are `src/icc.rs` and `src/error.rs`.
The rrrah caller now retains these working reservations through image teardown.
Entropy state, parser copies, retained rendered ICC and allocator overhead still
need separate accounting. The earlier no-admission scope above describes the
first output-bounds patch; this section supersedes it for the covered buffers.


## Retained synthesized output

`colour_encoding_to_icc_with_admission` calls a callback with the exact final
profile length before allocating the output Vec with `try_reserve_exact`.
Descriptor/header/tag/transfer-table synthesis still precedes this callback and
needs separate working-memory accounting. The legacy helper uses a no-op
callback; bytes of the generated ICC are unchanged.


## ICC entropy cancellation polling

The optional admission callback is called again at each 4096-symbol boundary
while reading the remaining encoded ICC stream, with the same absolute high-water
bound. Callers can check cancellation there without adding reservation credit.
The legacy no-op callback preserves ordinary decode behavior. This does not add
checkpoints inside entropy-table construction, ICC command reconstruction,
profile parsing, or synthesis.

## Reconstruction cancellation

`decode_icc_with_check` checks before final reconstruction output allocation,
between tag/main commands, every 4096 shuffle/predictor positions/iterations,
and every 64 KiB of literal/shuffled output copying. Legacy `decode_icc` uses a
no-op check. This supersedes the earlier reconstruction-checkpoint gap; entropy
table construction and profile parsing/synthesis still lack equivalent checks.
