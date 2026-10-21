# Rounded clipped-corner topology correction

A two-triangle numeric regression now reproduces the previous pixel (102,142) failure without the rejected unit-domain elision. This establishes a pre-existing geometry defect, newly reached by that rejected experiment. The baseline test fails with represented area 0.000015359495987247272 versus independent exact union 0.000015317596329047092. A floating convex-pair intersection checker finds excess overlap 4.1899658193264465e-8.

## Reproducer and independent oracle

A temporary capture of pinned VectorApple.ai (SHA c5fc34346629c3379cee6065c45b7de0b620be822604551d1463d8e691b03d06) yielded 138 triangle/affine-owner records at this pixel in the 323-patch shading. The normal algorithm with a fresh local 100M quota reproduces overlap, independently of page scheduling or the elision. Greedy chunk/deletion reduction yields source-order indices [103,105]. It is deletion-minimal for the measured criterion, not a global minimality proof. Only these two numeric inputs are committed, with source metadata, in tests/fixtures/geometry/unit-border-overlap.json.

Run `python3 scripts/generate-dense-lineage-control.py tests/fixtures/geometry/unit-border-overlap.json` to regenerate the Rust numeric table and exact Fraction oracle. The independent implementation clips to the unit pixel, gathers all segment crossings and vertex events, and integrates union vertical intervals in each exact rational slab. It uses no Rust provenance or clipping helpers. fraction-union.json records all exact fractions and the rounded result.

## Cause and correction

Source-UV clipping creates two intersections near one old corner. Rounded coordinates exchange their relative order. The ordinary geometric hull reorders four retained vertices and turns known boundary tags [1,11,2,10] into [0,11,0,10], reversing a tiny field boundary and reconstructing neighboring edges from rounded points. Subsequent subtraction then retains a positive-area overlap. baseline-raw-clip.log and baseline-supports.log retain this diagnostic evidence.

After the usual hull calculation, clipping now retains its original ordered result only when the hull retains every vertex, every input edge has an original support ID, and the hull order differs beyond a cyclic rotation. Unknown boundaries and ordinary vertex-removing hull simplification use the existing hull path. No epsilon, area clamp, fragment discard, quota, depth or memory-cap increase is introduced. This preserves original support topology for the isolated rounded-corner exchange.

A broader skip-hull experiment fixed the pair but failed the earlier 50-input dense regression at record 49 with Limit; it was rejected. The retained narrower correction passes that exact-area control with 179 fragments and the 75-input control with 70 fragments. These operation/fragment observations are not wall-clock performance evidence.

## Validation and limits

The new pair agrees with the exact union within 1e-14 and has no pair overlap above 1e-14. Every reached 5 admission refusals, 178 cancellation checkpoints and 178 shorter work budgets preserves the old integral, fragment count and retained budget, then retries successfully and releases all credit. Final vector: 227 passed/11 ignored; interpreter: 157/1; native: 319/13. Kernel: 12 controls passed. Full 138-record replay ends at area 1, max floating pair overlap 7.806255641895632e-18, 93 fragments, no measured overlap above the reducer threshold. This is a floating overlap check, not an exact rational proof for all 138 records or color/source ownership.

The first full-page comparison was started before the new build completed and is excluded as stale-binary-rejected.json. Its end-of-run binary hash cannot establish the executable used at process start. The valid poppler.json was rerun only after build completion. It shows all four VectorApple.ai attempts still exhausting 100M, with no observed coverage-over-one refusal in the captured stderr. one.ai page 0 remains 2522 differing pixels/max58 and page 1 exact. Full AI correctness and performance remain unqualified; the broader 100-format, long cache pressure, physical HDR and NVIDIA gates remain open. Temporary capture/diagnostic code is removed.
