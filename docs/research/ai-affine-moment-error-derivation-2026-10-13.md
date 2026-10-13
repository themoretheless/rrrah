# Affine moment geometry contribution

This is one error term, not the complete optimized pixel certificate.

Let A and p be the exact area and weighted centroid of the reported fragment inputs. The moment table evaluates Â and p̂. Its interval side table gives |A-Â|≤δA and coordinate bounds |p-p̂|≤δx,δy. For an exact affine field F with absolute device gradients Gx,Gy, and a bound V on |F(p̂)|:

    |A*F(p) - Â*F(p̂)| ≤ A_upper*(Gx*δx + Gy*δy) + δA*V.

The kernel accumulates this expression outward and refuses invalid/nonfinite bounds, budget exhaustion and cancellation. It does not cover floating summation of final pixel integrals, source-sampling discrepancy or preexisting polygon geometry error.

The source gradient API encloses exact affine derivative coefficients using the same retained inverse transform, rounded constant source edge differences and denominator as the source's affine error model. It rejects functions/color conversions. It does not use the derivative of the projected/normalized sampler; that discrepancy has a separate pointwise envelope.

The diagnostic aggregates geometry terms with unit x/y gradients and value bound2. Scaling their sum by max(1,Gx_max,Gy_max) gives a conservative geometry-only term for fields bounded by those source gradients and |F(p̂)|≤2. This remains conditional on those model/value assumptions; it is not a certified full route error or source-domain membership result. In particular, no sample skipping is enabled.

Original AI conditional geometry terms are approximately6.22e-11 and1.28e-12 for the two large moment collections, much smaller than the previously bounded source-component rounding discrepancy of about1.21e-7. Rounded clipping/domain membership and integral summation remain separate requirements.
