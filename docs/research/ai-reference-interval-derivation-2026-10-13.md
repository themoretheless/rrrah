# Original per-bucket reference integral intervals

The original centroid comparison retains per-bucket compensated sums of `area * reference`, where reference is `[alpha, alpha*C0, ...]`. These are computed in a different floating order from the baseline integral. A compensated numeric value alone is not an error interval.

The optional reference side vector now encloses each reference input one f64 step outward, then encloses multiplication by positive fragment area and accumulation outward. One input step contains the exact product of the reported alpha/component operands before their single rounded multiplication; exact alpha inputs are also contained. The resulting interval therefore encloses the original reported-fragment integral target independently of the compensated value or its rounding order. This contract does not cover arbitrary unknown upstream operations.

Reference intervals, geometric bounds and ordinary compensated moments are prepared in local values and committed together only after every required work debit/cancellation check. The reference vector is separately admitted; its capacity cannot exceed the admitted capacity. Ordinary tables keep reference tracking disabled. Allocation refusals release all earlier credits.

An identity-guard fallback still adds its original compensated reference value to the candidate comparison. The candidate envelope now also adds the matching original reference interval from the same stable entry index. Its maximum native-component bound is supplied independently by the complete original-sample envelope; it is not inferred from unrelated interval endpoints. Comparing the final candidate numeric sum to this merged target includes both compensated-reference and later addition rounding.

Original AI results:

- Pixel147,59: candidate integral rounding bound7.31637e-14; original-plus-candidate native arithmetic term1.24134e-10.
- Pixel145,50: its one identity fallback now has a retained interval; candidate integral rounding bound1.36003e-13 and native arithmetic term4.78288e-11. The original integral fallback remains active and the centroid is not substituted for that bucket.

All five original pixel values, source/identity/centroid controls and peak credit remain unchanged; final retained credit is zero.256library tests pass, including fused-residual validation of upstream product padding, insert/update transaction failures at every budget/cancellation point, all three admission failure stages, and interval-addition refusal checks.

This closes the missing candidate arithmetic term. It does not itself combine source, route, moment and quotient error into a final native replacement tolerance, establish independent PDF/display accuracy or satisfy the ordinary4Mfull-page work budget. No source color calculation is skipped.
