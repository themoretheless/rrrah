# Exact occlusion rejection before fragment allocation

OrderedPixel checks whether the last retained convex region contains the entire clipped incoming triangle and strictly outranks it. Patch priority is compared directly; same-patch v/u dominance is accepted only when every polygon vertex has a strictly positive affine difference beyond a conservative arithmetic filter. Ambiguous values, exact ties and crossing fields use the ordinary path. Convex containment uses the existing filtered/exact orientation predicate. Successful rejection needs no heap allocation and leaves retained ownership unchanged.

When the incoming remainder becomes empty in the ordinary overlay loop, subsequent retained regions are copied directly instead of clipped and reassembled against an already hidden triangle. Admission, monotone work and cancellation checks remain active.

## Completed validation

- Full vector suite: 218 passed, 11 ignored.
- Full native suite: 319 passed, 13 ignored.
- Full interpreter suite: 143 passed, 1 ignored.
- Fresh extended filtered control: 1 passed, also covered by the vector suite count.

The extended control verifies zero new admissions for proved-hidden triangles, unchanged integrals, every observed cancellation checkpoint, short work, exact priority ties, crossing fields, conservative subnormal refusal and retry with admission restored. All original validation runs completed with exit 0. Earlier partial logs and process-state snapshots document repeated external SIGSTOP suspension; the original runs subsequently resumed and finished without restart. Wall times include interference and are not benchmarks.

## Public page outcome

The pinned-page comparison exited 1. VectorApple.ai reached the 300-second wall-clock render timeout, during an interval with host interference; this neither qualifies the renderer's CPU speed nor provides an appearance comparison. one.ai page 0 retains 2522 differing pixels/max58, and page 1 is exact. Full AI support is not repaired by this control-level improvement. No speedup is claimed.

## Timeout diagnostics

The qualification runner retains stderr carried by TimeoutExpired as well as CalledProcessError. A captured-exception integration control verifies bytes, text bounded to 16 KiB, absent stderr, three explicit page failures and strict exit 1. Short live sleeping-renderer controls did not consistently provide stderr before their deadlines and are not counted as proof that timed-out processes always produce diagnostics. The behavior preserves available diagnostics without inventing absent output.

Next work is reducing dense ownership candidate comparisons/partition growth with the original priority and tie semantics intact, followed by an independent full-page appearance comparison. The broader 100-format, cache, GPU and physical HDR objective remains open.
