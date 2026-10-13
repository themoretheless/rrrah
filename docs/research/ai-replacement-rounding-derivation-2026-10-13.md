# Arithmetic envelope for the centroid comparison sum

The new optional diagnostic separates two sample streams: accepted bucket-centroid samples and unchanged per-fragment fallback samples. Each stream encloses exact products of its reported area, alpha and native components. Their interval merge encloses the final addition between streams, retaining a valid maximum component bound for their nonnegative-alpha weighted mean.

The renderer's original pixel and the existing centroid comparison keep their original numeric operations/order. Envelope export measures the candidate's alpha/integral rounding error. Separate quotient-error bounds for the original and candidate streams are added outward; this bounds their arithmetic contribution to the difference of native ratios, regardless of the source/moment discrepancy between their exact target integrals. It is not the complete replacement bound.

Individual fragment areas remain strictly limited to1. A separate aggregate method accepts up to1+1e-12, matching the existing compensated moment-area admission tolerance. This avoids falsely rejecting a valid rounded aggregate while preserving fragment validation. It does not reinterpret the excess as true geometric area; the moment geometry error remains a separate term.

If an exact field/route identity check fails, the existing comparison retains the original bucket reference integral. That reference was produced by compensated accumulation; its interval is not currently retained. The candidate envelope is therefore invalidated and reports incomplete. Neither an exact-looking reference value nor the small measured centroid discrepancy is accepted as a substitute for that missing bound.

Current original-AI evidence:

- Pixel147,59: complete centroid/fallback accumulation envelope, candidate alpha error9.64784e-14, maximum candidate integral error7.31637e-14, original-plus-candidate native arithmetic term1.24134e-10.
- Pixel145,50: incomplete because exactly one bucket takes the original reference-integral fallback. Existing original-integral behavior and all pixel values remain unchanged.

251library tests pass, including merged streams, every short merge budget/cancellation checkpoint and aggregate-area validation. All five original native pixel values, identity checks, source evidence and centroid comparisons remain unchanged; final retained credit is zero. No source color sampling is skipped. Per-bucket reference intervals are the next required input before the general combined error budget can be accepted.
