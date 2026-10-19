# JPEG XL ICC entropy cancellation — 2026-10-19

## Reproduction and implementation

The ICC admission callback previously ran only before initial encoded storage and
after the small decoded size header. Once the remaining entropy-coded ICC stream
was being read, a stale request could not be observed until that entire loop
completed. `before.log` records the regression failing on that prior code.
`baseline-source-check.json` verifies the pre-change decoder matches the prior
retained-ICC milestone's source hash.

The optional callback now repeats at each 4096-symbol boundary in the remaining
encoded-ICC loop, using the same absolute high-water byte bound. The rrrah
callback already checks the request generation before reserving any high-water
increase, so repeated calls do not add memory charges. Cancellation remains a
typed `DecodeError::Cancelled`, and callback failure tears down initialization.
Legacy no-op callbacks and small-profile output remain compatible.

## Independent synthetic control

`scripts/generate-jxl-large-icc-fixtures.py` extends the existing 520-byte linear
RGB profile with 8192 deterministic SHA-256-derived bytes in an unreferenced
tail, updating the ICC size while preserving all tags and interpretation. This
is a size-selected synthetic profile, not a complex real-world LUT/camera
profile. It is 8712 bytes; the resulting JXL is 8924 bytes.

The common C/libjxl producer/decoder checks the original profile byte-for-byte,
all 180 authored u16 samples, three frames, fractional timing and two loops.
Source/profile hashes and reference version are in `jxl-large-icc.json`.
The common generator now accepts output/profile/name arguments; default small
ICC generation reproduced all five existing artifacts byte-for-byte.
Authored samples and tail generation are CC0; embedded base ICC retains its
existing fixture provenance.

## Verification and precise scope

The regression deterministically changes the generation in the third admission
callback: the first two admit storage, the third is reached within the entropy
loop at the 4096-symbol boundary. It verifies typed cancellation, exactly three
callbacks, a 26022-byte working high water, no remaining root credit after
teardown, and successful decoding of all three exact native frame references
with a fresh generation and the same root. Retained native pixels and ICC are
fully charged after each retry. This is controlled callback injection, not a
live GUI/worker scheduling or wall-clock cancellation-latency measurement.

Terminal validation:

- `targeted.log`: cancelled decode and three successful retry frames, final root 0.
- `suites.log`: jxl-color 18; animation 35 /1 ignored; decode 228 /5 ignored.
- `app.log`: 288 passed /81 ignored.
- Ordinary total: 569 passed /87 ignored.
- `metal.log`: selected actual Apple M4 Max Metal test passed: six JXL controls,
  18 frames through pinned RAM and native/prepared-float swap; pressure refusal,
  retry, native/SDR/half transport and timing/count/index preserved; final root 0.
- `format.log`: workspace formatting passed; source whitespace check passed.

One upstream lifetime-syntax warning remains; no strict Clippy claim is made.

## Remaining work

No new checkpoints were added inside entropy-table construction, ICC command
reconstruction/shuffle, profile parsing/synthesis, or frame rendering. Those
phases may still delay cancellation. Entropy/parser/TOC/header and temporary
synthesis allocations still need separate accounting. The 4096-symbol check is
not a hard time bound. This synthetic control does not qualify maximum-sized
profiles, all producers, live GUI endurance or physical HDR.

The full 100-format/cache/GPU objective remains open, including broader producer
qualification, six pending catalog formats, full-history JXL animation cost,
physical slow-disk/live-GUI testing, physical HDR and deferred NVIDIA/CUDA.


Follow-up: reconstruction command/copy/shuffle/predictor checkpoints are added in
[the subsequent report](../jxl-icc-reconstruction-cancellation-2026-10-19/REPORT.md).
The reconstruction gap above describes the earlier entropy-only snapshot.
