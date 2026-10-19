# Normative refusals and qualified EXR emission witness — 2026-10-19

## Corrected AVIF conclusion

The previous mixed-collection report incorrectly called positive 2:1 `pasp` an
AVIF feature gap. The official [AVIF 1.2 specification, section 9.1.2](https://aomediacodec.github.io/av1-avif/v1.2.0.html)
requires a present `pasp` to signal 1:1. Both the 2:1 control and the zero-spacing
control are nonconforming. The existing integer-positive-square guard is correct;
its acceptance policy remains unchanged. The prior feature-gap statement has
been corrected in the report and goal ledger.

The four metadata-only AVIF fixtures were reproduced exactly, with unchanged AV1
media. Their metadata manifest/generator now explicitly record the normative
source and expected conformance: 2:2 accepted (essential and non-essential), 2:1
and 0:0 refused. The public decoder regression passes, preserving native/prepared
samples for the conforming controls and returning typed errors/root zero for the
nonconforming controls.

## Refusal taxonomy, measured on the same 1024-source manifest

The opt-in collection qualifier now records categories from typed errors, rather
than implying that every refusal means either a broken file or a missing codec:

- 46 require qualified source color/profile/interpretation information.
- 13 concern invalid or unsupported EXR color metadata constraints.
- 2 are nonconforming AVIF `pasp` declarations.
- 1 is unsupported valid EXR associated-alpha emission.

All 62 refusals are categorized; none falls into the fallback category. The same
962 sources still pass CPU navigation/prefetch/TTL/swap transport. This rerun
reports 962 writes, 1881 reads, no dropped writes/errors, 61 TTL restores and
final managed root zero. Source selection and pressure limitations are unchanged
from the mixed-collection report: bounded local assets/controls, same-decoder
transport hashes, and reservation-only full-root pressure, not a physical RSS or
full-resolution workload guarantee. `collection-result.json` and `collection.log`
retain the evidence without source media or absolute source paths.

## Actual valid-source gap: associated-alpha EXR

The official [OpenEXR technical introduction](https://openexr.com/en/latest/TechnicalIntroduction.html#premultiplied-vs-un-premultiplied-color-channels)
describes nonzero RGB at alpha zero as valid. Associated over compositing is
`foreground + (1-alpha) * background`; the straight-alpha renderer instead
premultiplies RGB by alpha, discarding emission at zero. The current decoder
explicitly refuses this case, which protects against silent loss but does not
provide display support.

Added a qualified independent reference control, eliminating unknown-primaries
ambiguity from the original emission fixture:

- `scripts/exr-associated-emission-oracle.cpp` writes/reads FLOAT channels through
  OpenEXR 3.5.2 C++, with explicit linear Rec.709/D65 chromaticities.
- Two authored RGBA pixels are `[2, .5, .25, 0]` and `[-.25, 4, .5, .5]`.
- Native F32 reference bits survive independent reading exactly.
- Over background `[.25,.5,.75]`, reference RGB is `[2.25,1,1]` and
  `[-.125,4.25,.875]`, including HDR and a negative component.
- The Rust primitive codec matches every native F32 bit and the same composition;
  the rrrah metadata parser resolves the source as linear sRGB.
- The current public raster path still returns the explicit unsupported-alpha
  error and releases managed memory. A passing refusal test is not display
  implementation or qualification of associated-alpha rendering.

The generator, small CC0 source fixture and native/composited reference buffers
are checked into the fixture set. `exr-associated-emission.json` records library
version and hashes. An additional independent read of the original one-pixel
control is recorded in `original-emission-witness.json`; those values are exactly
representable even through the Rgba HALF interface, not a general FLOAT oracle.

## Implementation boundary for the next step

The remaining feature needs associated/emissive semantics in the owned raster
representation, propagation through color preparation and swap metadata, and
GPU sampling/compositing without multiplying associated RGB by alpha again.
Native sample bits and alpha must remain recoverable. The qualified two-pixel
oracle fixes the required result; simply clearing RGB at alpha zero would fail.
No associated-alpha viewer/GPU support has been delivered in this milestone.

Fresh validation: public AVIF four-control test passed; nine existing EXR tests
passed; new qualified emission reference/refusal test passed; the complete
1024-source CPU/cache qualification passed. Formatting/source whitespace checks
passed. Existing upstream lifetime warning remains; no strict Clippy, physical
HDR or NVIDIA/CUDA proof is added. The full goal remains incomplete.


## Follow-up

The unsupported associated-emission snapshot is superseded by the implemented
float linear RGB path. See [qualification](../exr-associated-emission-2026-10-19/REPORT.md).
