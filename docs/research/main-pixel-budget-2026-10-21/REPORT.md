# Rejected 8k main-pixel transition pilot — 2026-10-21

## Decision and implementation

Rejected and removed from production. Exact phase accounting identified 91M/100M painting work before dense fallback in the 323-patch sampled attempt. The pilot limited each initial tile pixel to 8*1024 painting ticks, debited spent work on all outcomes, and reused the existing dense replay path on local exhaustion. Once every pixel in a full 4x4 tile required replay, remaining first-pass records were skipped. All replay used the complete original records; global quota, fragment/depth bounds and color tolerances were unchanged.

The implementation and its tests are retained in rejected-pilot.patch. Production is byte-identical to the prior validated sampler after rollback.

## Correctness controls

Eight targeted sampler controls passed. New tests verify exact work debit for zero/short quota and cancellation, equivalence to an uncapped paint, and a forced transition that preserves full-square coverage/constant color. All 48 reached allocation refusals and 899 cancellation checkpoints release all managed bytes and permit retry under 8 MiB. Full pilot results: vector 225 passed/11 ignored, interpreter 158/1, native 319/13, zero failures.

These are correctness/resource controls, not performance proof. The actual pinned VectorApple.ai still returned four incomplete sampled attempts after 100M each. one.ai page 0 remained 2522 differing pixels/max58; page 1 exact. pilot-poppler.json contains the input/decoder hashes and all structured outcomes. No representative end-to-end completion improvement was established, so the new default is not kept.

## Restored state and next evidence

The sampler source was restored byte-for-byte to the checkpoint validated by vector 225/11, interpreter 156/1 and native 319/13 in ../cost-triggered-cells-2026-10-21. Its normal decoder rebuilt and is byte-identical to that report's validated decoder. Therefore those current-state correctness results remain applicable; the rejected pilot's larger test count is not claimed for production.

The exact phase ledger is in ../sample-work-ledger-2026-10-21. Next: attribute work within OrderedPixel itself, including old-region visits/copies and geometric subtraction. Further indexing or scheduling changes require measured benefit on representative geometry. Full AI and the original broader goal remain incomplete.
