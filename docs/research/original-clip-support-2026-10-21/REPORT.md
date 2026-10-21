# Original clip-support reconstruction — 2026-10-21

## Root cause and change

A rounded descendant edge can lose its original slope. In a five-triangle real-input subsequence, an edge only a few ULPs long became horizontal near (0.5231209418923303, 0.6230073090824249), while its retained line ID referred to an oblique source edge. Subtraction evaluated the new horizontal segment but forced previously known source-line vertices onto it. The inconsistent boundary interpretation created a positive-area extra triangle.

The line pool now retains original exact keys in an ID-indexed vector, covered by the same admitted replacement allocation as the hash table. On growth both structures remain admitted simultaneously; rollback restores the original map, vector capacities, IDs and credit. Copy work is charged and cancellable. Subtraction evaluates the original support (anchored source edge, affine field or axis) and uses the descendant edge only to choose inward orientation. Unresolved orientation is refused. No fragment epsilon, clamping or enlarged memory/work limit is introduced.

The strict dominant-region shortcut rejected in the preceding report is being requalified together with this underlying support fix. It retains a winning old region and cuts only the incoming remainder; ties/crossing/ambiguous source fields use the general path.

## Reproducer and independent oracle

An opt-in temporary diagnostic captured 285 triangle inputs at pixel (106,57) of pinned VectorApple.ai. A patch-index reset at input 220 separated two shadings. The first 220 reproduce exactly the earlier coverage 1.0002352951797064. Temporary capture/minimizer code is removed from source.

Checking only global coverage needed almost the whole sequence. An independent convex-intersection checker reduced the positive pair overlap to source-order inputs [43,44,54,56,61]. This five-record subsequence has maximum pair overlap 0.00020470188447682788 before the fix. It is deletion-minimal for that reducer criterion, not a proof of globally smallest input.

The five inputs are retained as bit patterns in tests/fixtures/geometry/clip-support-five.json and generated Rust data. scripts/generate-support-line-control.py computes each prefix union independently using exact Fraction arithmetic, convex half-plane clipping and inclusion-exclusion of all triangle subsets. The Rust regression checks every prefix against the rounded exact union to absolute 1e-14 and separately checks every retained polygon pair with an independent numerical clipper to the same bound. All five corrected prefixes report zero pair overlap. Final union area: 0.01705208169393837, six retained fragments.

The fixed pilot log contains an intentional failed assertion: the old temporary minimizer still asserted that the bug existed, and failed because its observed overlap became zero. The final permanent regression has the correct no-overlap/union assertions and passes in vector.log.

## Validation

Vector: 221 passed/11 ignored. New dominant-strip rollback checks cover seven allocation refusals, all 222 cancellation checkpoints and 222 shorter work quotas, including original partition/live-byte restoration and retry. Existing line-pool exhaustive controls now also check reverse-vector length, allocation size and key/ID consistency after rollback. Full interpreter/native results are recorded below when complete.

Sixteen-strip logical work remains 1532/1955 ticks versus the prior baseline 2584/2995 for full/gapped strips; colors, analytic areas and fragment counts match. This is not a wall-time or page-throughput claim. Additional persistent support-vector memory is admitted and is not free.

## Real-page limits

Pinned fresh-binary comparison: VectorApple.ai still refuses. The previously captured coverage-over-one failures do not appear in the new trace; remaining terminal paths include shared work exhaustion and bounded deep-cell Limit refusals. Numerical tiny fragments persist. one.ai page 0 remains 2522 differing pixels/max58 and page 1 exact. Raw source/decoder hashes and diagnostics are in poppler.json. This fixes the isolated support inconsistency but does not qualify full AI, all 100 formats or hardware/HDR.

Next: isolate the bounded-cell failure at (151.0625,59.9375), side 1/256 (51 candidates, 49 processed, 119 published fragments), and distinguish vertex/transient partition growth from source coverage. Preserve every positive-area contribution and the current limits. NVIDIA remains deferred by the user.

Final library verification: interpreter 153 passed/1 ignored; native 319 passed/13 ignored. Zero failures. All original work limits and numerical acceptance thresholds remain unchanged.
