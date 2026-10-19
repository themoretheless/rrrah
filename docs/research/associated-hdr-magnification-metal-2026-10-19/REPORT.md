# Associated extreme HDR magnification on actual Metal

The existing opposite-sign HDR magnification test now covers straight opaque samples and associated emission. Authored two-pixel linear-float input is [MAX,MAX,0.5,A0] and [-MAX,MAX,0.5,A1], with alpha1/1 for straight and0/0.5 for associated. This is a synthetic renderer control, not a new camera original or independent EXR decode fixture. It extends the previously separate EXR/area/compute evidence for the associated representation.

Actual adapter: Metal Apple M4 Max. A63x65 output at zoom2 exercises ordinary bilinear magnification with black background. The full central row (63 pixels per mode) matches exact RGBA16F expectations: signed red saturation on the two sides, exact red zero at the symmetric center, green65504 and blue0.5 inside the image, and exact background outside. Both complete output frames contain no Inf/NaN. The existing renderer-owned input budget helper checks release after completed submission. This is not an all-pixel f64 appearance oracle: numerical expectations cover the central row, finiteness covers the rest.

The targeted hdr_ ignored-test run passed three actual Metal tests: the expanded interpolation case, the existing finite-range exposure control at-10/0/+10 stops, and the existing36MP HDR area-LOD control. The latter checks every output component at64x64 and1000x1000 against its independent constant reference, plus held-owner/one-byte-pressure/cancel/retry accounting, final managed output root zero. No runtime shader change was needed. No physical display, NVIDIA/CUDA or general performance claim follows.

An initial edit replaced the adjacent exposure test and duplicated a function name; compilation rejected it. The adjacent test was restored unchanged and the intended interpolation test alone expanded. The failed compiler log is retained; the final three-test run is green. Formatting and whitespace checks pass.

Reproduce: RRRAH_GPU_BACKEND=metal CARGO_TARGET_DIR=/tmp/rrrah-alpha-isolated-target-20261016 cargo +stable test -p rrrah-gpu --test raster_readback hdr_ --locked --offline -- --ignored --nocapture
