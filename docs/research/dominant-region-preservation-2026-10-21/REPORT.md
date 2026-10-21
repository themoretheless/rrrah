# Rejected dominant-region shortcut — 2026-10-21

## Decision

Rejected and removed from production. The current ordered_pixel.rs production prefix is byte-identical to HEAD before its test module. Only additional controls are retained. The rejected implementation is preserved in rejected-shortcut.txt and candidate.patch.

The candidate proved old source priority over the entire incoming triangle, retained the original old region and subtracted it only from the incoming remainder. The prior implementation clips the old region first and later restores it if the overlap wholly wins. This apparently equivalent shortcut is not numerically equivalent in the present boundary/partition representation.

## Synthetic evidence and its limit

Sixteen opaque vertical source strips followed by two earlier full-pixel triangles retain their exact analytic color/coverage integrals and fragment counts in both implementations. Work ticks fell from 2584 to 1532 for full strips (16 fragments), and 2995 to 1955 for half-width strips with gaps (48 fragments). This logical quota reduction does not establish wall-time or real-page benefit.

The candidate vector suite passed 220 tests/11 ignored. A new four-strip gap control expects red area 1/2 and green area 7/32, total coverage 23/32; it verifies all 7 reached allocation refusals, 222 cancellation checkpoints and 222 short work quotas, original partition/live-byte restoration and successful retry. These tests were insufficient to qualify the optimization.

## Real-file rejection

Pinned public VectorApple.ai refuses with total-coverage checks:

- pixel (106,57): 1.0002352951797064
- pixel (122,59): 1.0169797747093217
- pixel (138,35): 1.00006521568347
- pixel (66,97): 1.0002146480054956

Raw source/decoder hashes and stderr are in poppler.json. The prior provenance checkpoint instead reached shared work limits. No tolerance was raised and overcoverage was not clamped. one.ai page 0 remains 2522 differing pixels/max58; page 1 exact.

This exposes a boundary/partition equivalence issue that must be isolated before such pruning can be accepted. Next: capture a small deterministic failing ownership sequence, verify disjoint area independently, and fix the actual clipping/lineage issue. Full AI, 100 formats, long memory-pressure collections and hardware/HDR qualification remain incomplete; NVIDIA stays deferred.

## Restored-code checks

After removing the candidate, native: 319 passed/13 ignored. All 13 focused ordered-pixel tests passed; the strip work counts returned exactly to 2584 and 2995, confirming the rollback. The restored failure control reached 7 admissions, 354 cancellation checkpoints and 354 short quotas, all checked with retry and unchanged prior partition/credit. The candidate interpreter suite also completed (153 passed/1 ignored), reinforcing that synthetic suites alone missed the real-page regression.
