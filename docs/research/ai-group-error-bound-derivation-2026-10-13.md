# Same-space group error boundary and nonlinear route guard

All compared fields must share the same alpha/opacity parameters and native coordinates. Error inputs are bounds on additive premultiplied storage. Three operations are covered:

- Removing the initial backdrop: E_out ≤ E_current +(1-group_alpha)*E_initial + roundoff. Applying the same clamp to0..group_alpha on both sides cannot increase absolute discrepancy. This does not make the clamped field affine.
- Scaling opacity: E_out ≤ factor*E_current + roundoff. Factor0 yields exact zero storage and zero error.
- Extracting native components: E_out ≤ E_storage/alpha + roundoff, including CMYK complement. Alpha0 yields the explicit zero-component result. Positive alpha below1e-12 refuses this envelope. This requires fixed identical alpha, not just an alpha error bound.

Native Pixel inputs are bounded by their checked storage limits; conservative64/4/8*f64EPS absolute allowances cover the corresponding removal/scaling/native extraction arithmetic. Every bound calculation rounds outward. Nonfinite/negative/inactive bounds, invalid parameters, work exhaustion and cancellation refuse. Tests compare3456pairs of actual group computations across Gray/RGB/CMYK, alpha/backdrop-alpha/opacity choices. Their three operation stages contain27648active component comparisons.

## Counterexample to the old route predicate

Let the opaque parent Gray field be q. A non-isolated child paints constant Gray0.25 with HardLight, giving child current color0.5*q. Group alpha1 removal retains that color. Apply group opacity0.5 and blend the child back into the parent with HardLight. The resulting field is0.5*q*q+0.5*q.

Actual Pixel arithmetic gives outputs0 and1 at q=0 and1, but0.375 at q=0.5 instead of their mean0.5. Every child paint can have an unchanged constant cell identity. Thus a constant-source child after a variable parent does not establish affine final output.

The previous affine route predicate explicitly accepted this ordering. It now refuses a group beginning after a variable mesh field until a specific lifecycle proof is available. Constant groups before the variable field still qualify. Regular baseline composition remains available for refused routes; no rendering support is removed and no samples are skipped by this change.

Original five AI pixel values, moment counts, identity checks and source/centroid error diagnostics remain unchanged with the stronger guard. This result does not certify rounded clipped-domain membership or area/centroid accumulation, nor the full viewer/ordinary4Mwork budget.
