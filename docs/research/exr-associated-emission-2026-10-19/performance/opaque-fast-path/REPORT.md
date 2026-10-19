# Skip redundant opaque EXR alpha work

The retained change skips overflow checks for alpha=1 and avoids a second pixel pass when no sample needs unassociation. Validation still checks every channel is finite and alpha is within 0..1, with cancellation checkpoints. Fractional alpha retains the previous three-channel f64 overflow test and division. Associated emission and overflow preserve original float samples.

Four alternating AB/BA pairs, seven measured samples plus three warmups each, compare the published association implementation against the candidate. Same synthetic source files and warm CPU-only benchmark as the parent report; exact prepared hashes across all runs, managed root zero. Median per-process CPU-ready p50: opaque 30.2866 → 27.6360 ms (-8.75%); fractional 24.4488 → 23.8858 ms (-2.30%, small/noisy). Background machine activity is uncontrolled. This does not prove elimination of the fractional-alpha overhead relative to the original straight-only decoder.

An earlier max-channel overflow-check candidate was rejected: direct measurement regressed fractional alpha by 7.04%, despite improving opaque. Its result is retained. The initial candidate run overlapping compilation was excluded; final retained measurements ran after this task's builds completed.

Final verification: 231 decoder tests passed (five ignored); actual M4 Max Metal emission/RAM/swap/compute/LOD integration passed, final managed root zero. Formatting and tracked whitespace checks passed.
