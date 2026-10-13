# Reported-sample integral and quotient arithmetic envelope

The opt-in envelope tracks exact sums of products of the reported f64fragment area, pixel alpha and native component values. It encloses area*alpha, each weighted component product, and all additions outward. Signed finite components are supported. Areas are positive and alpha is nonnegative, so the exact ratio of weighted components to total alpha is a weighted average; maximum absolute reported component values bound that ratio.

The actual baseline sums keep their original operations and order. Distance from those sums to the interval endpoints bounds their rounding discrepancy from exact arithmetic on the reported inputs. This does not certify the geometric/source accuracy of the inputs.

Let I,A be reference numerator and alpha, and Î,Â the compared values. If both alphas exceed positive L and |I/A|≤B, then

    |Î/Â - I/A| ≤ (|Î-I| + B*|Â-A|)/L.

The quotient kernel rounds bound arithmetic outward and adds a conservative division-rounding allowance for both compared ratios, including a subnormal allowance. It refuses zero/nonfinite lower alpha, invalid errors and overflow. Tiny positive alpha is permitted when finite bounds remain available. The transparent alpha0 convention remains a separate case; no division by zero is certified.

Diagnostic construction has separately admitted112byte stack state and uses the existing bounded diagnostic allowance. Failed certificate admission/exhaustion disables the envelope rather than changing the baseline pixel. Ordinary rendering does not run the interval operations.

Tests enclose4096dyadic sample products with known exact integral/ratio, exercise every insufficient budget and every cancellation checkpoint for accumulation/export/quotient, and compare differing-alpha ratios down to1e-300. Nonfinite/negative inputs and overflowing quotient envelopes refuse.

The source domain/clipping, pointwise source discrepancy, route error, centroid replacement and later Pixel storage/profile/display conversion are separate terms. This envelope alone does not authorize sampling replacement or establish the ordinary4Mfullpage budget.
