# Unit-domain clipping requalification after topology correction

The original unit-domain elision was rejected when it first reached a coverage-over-one error. A separate two-triangle regression subsequently established that the error predates elision; rounded-corner boundary order was corrected independently (../rounded-boundary-order-2026-10-21). This experiment reapplies the same algebraic elision to that corrected implementation.

## Implementation and independent controls

Subtraction subjects are already clipped to the unit pixel by OrderedPixel. A cut whose original support is exactly an inward x=0/x=1/y=0/y=1 boundary cannot remove positive-area content. Such a cut skips both clipping passes. The original Axis key is required: descendant coordinates that only appear axis-aligned do not qualify. Opposite directions, interior cuts, degenerate directions, oblique original edges and oblique fields are explicitly rejected. Edge-loop and line-interning debits remain; no caps or tolerances change.

The analytic interior triangle/unit-square case retains identical represented points and moments with 47 baseline versus 15 candidate work units. All prior exact Fraction union fixtures, including the isolated rounded-corner pair and both dense controls, remain enabled. On the captured 138-record replay, every prefix has identical summed area and fragment count; final area is 1 with 93 fragments and max floating pair overlap 7.806255641895632e-18. Local work falls from 56,892 to 49,606 units (12.81%). This is reduced metered kernel work, not wall-clock/page/display throughput. local-comparison.json and all138.log preserve the observations. New-case rollback/retry covers all reached 5 admissions, 162 cancellations and 162 short quotas, retaining memory accounting and releasing all credit.

## Real-file gate and reproducibility

The comparison runner now executes a private frozen copy of the decoder; report hash is computed from those exact bytes before any page render. A regression replaces the original source executable in place and atomically, and confirms subsequent invocations still execute the frozen version with the recorded hash. Four Python checks pass. The real report confirms execution of this private copy; no concurrent rebuild can substitute a different decoder between pages. The copied native Mach-O executes successfully with its existing absolute/system dynamic dependencies.

Pinned real-file comparison still returns incomplete at 100M for all four VectorApple.ai attempts. No coverage-over-one refusal appears in the captured terminal trace. one.ai is unchanged: page 0 differs from Poppler in 2522 pixels/max58, page 1 is exact. This does not establish full AI support, page performance, all 100 formats, cache pressure or hardware qualification. The source contains no temporary profiling instrumentation. Full-suite results are retained in vector.log, interpreter.log, native.log and kernel.log.

Final checks: vector 227 passed/11 ignored; interpreter 158 passed/1 ignored; native 319 passed/13 ignored; kernel 13 passed; Python 4 passed. No failures.
