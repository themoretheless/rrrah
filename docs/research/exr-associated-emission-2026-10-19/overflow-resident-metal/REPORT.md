# Extreme EXR resident compute to raster on Metal

The independent FLOAT Rec709 EXR overflow fixture now passes the queue-ordered compute-to-raster path without intermediate CPU pixel readback. The public execute_raster_resident API retains associated-alpha metadata; upload_resident_with_aspect feeds the ordinary raster renderer. Actual adapter: Apple M4 Max, Metal; adapter absence fails the test with RRRAH_GPU_OPTIONAL=0.

Six cases cover direct raster 0 stops and resident compute 0/-1 stops, each to RGBA32F and RGBA16F. Area minification at 1x1 matches exact float [MAX/4, -MAX/4, 1.25, 1] at zero stops, halved RGB at -1. Half output saturates extreme channels to +/-65504 while retaining blue 1.25 or 0.625. Each owned input/resident GPU budget returns to zero after completed rendering and owner release. The existing blocking zero-stop compute readback still preserves all source bits; CPU/compute roots release to zero.

At +1 stop, the source MAX samples would overflow before filtering. The resident API returns its explicit finite-output error before any budget reservation (peak zero). This is an intentional typed refusal of unrepresentable compute output, not support for arbitrary finite source/exposure combinations. No runtime change was required for these checks.

The actual hardware test passes. Physical HDR, NVIDIA/CUDA and positive-exposure rendering of otherwise-overflowing source samples remain outside this proof.

Reproduce: RRRAH_GPU_OPTIONAL=0 CARGO_TARGET_DIR=/tmp/rrrah-alpha-isolated-target-20261016 cargo +stable test -p rrrah-gpu --lib associated_exr_extreme_float32_and_half_metal --locked --offline -- --nocapture
