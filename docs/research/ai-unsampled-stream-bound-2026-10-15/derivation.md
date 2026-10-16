# Rounding bound without observing source colors

This is a CPU arithmetic building block, not an enabled rendering replacement.

Let each exact term be a product of k+1 independently certified factors in [0,1], where 0 <= k <= 2. Let n terms be accumulated by ordinary f64 additions from zero, and let S independently bound their exact nonnegative sum. Geometry membership, source range, clipping, affine-source error, profile conversion and display quantization must be proved separately.

Use u = f64::EPSILON, a conservative relative roundoff bound, and eta = the smallest positive f64 subnormal. Under gradual underflow, each operation has an absolute rounding error bounded by u times its exact magnitude plus eta. Unit factors cannot amplify preceding product underflow by more than one before subsequent rounding.

Every contribution passes through at most m = n+k operations. For m*u < 1, the usual product expansion is bounded by 1/(1-m*u). Summing relative errors and at most n*(k+1) additive underflow errors gives:

E <= (m*u/(1-m*u))*S + n*(k+1)*eta/(1-m*u).

The implementation rounds the integer conversion, operation counts, products, divisions and final sum outward; the denominator rounds downward. Invalid/nonfinite input, k>2, exhausted work, cancellation, a nonpositive denominator or a nonfinite result refuses the bound. Count zero returns zero. It requires neither the individual colors nor their products. Flush-to-zero execution is outside this contract.

The independent dyadic control sums one plus 4096 copies of 2^-54. The exact sum is representable as 1+2^-42, whereas the original order loses every small term. Controls cover zero, one and two product operations, reversed accumulation and two half-subnormal products whose exact total is one subnormal. Additional tests exercise all smaller work budgets, every cancellation checkpoint and invalid domains.

The existing AI comparison bound still checks every original source point. This new bound only removes one dependency on observed accumulation values; it does not authorize skipping source range/owner-domain checks or enable live merging. Ordinary 4M/full-page acceptance remains unresolved.

## Recovering the exact mass without a per-term interval stream

The bound at S=1 gives c>=gamma, and the bound at S=0 gives d>=the additive underflow term. For a reported ordinary sum r, |r-S|<=c*S+d. When c<1 this implies max(0,(r-d)/(1+c)) <= S <= (r+d)/(1-c). Numerators and denominators round outward in the appropriate directions. This uses count, product count and r only, with the same independently certified unit-factor/gradual-underflow contract. It refuses inconsistent zero-count sums, nonfinite/negative reports, excessive unit-term sums, c>=1, short work and cancellation. The current observer retains the old outward interval stream solely to compare the recovered enclosure; live source/geometry/profile certification is still separate.
