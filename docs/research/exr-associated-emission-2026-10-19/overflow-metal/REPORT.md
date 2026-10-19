# Associated EXR extreme range on actual Metal

The independent four-pixel OpenEXR FLOAT fixture from the CPU overflow boundary is decoded and prepared through the native Rust pipeline, then uploaded to the ordinary raster renderer on Apple M4 Max Metal. The test requires Metal availability and fails on adapter absence (RRRAH_GPU_OPTIONAL=0).

At 1x1 area minification, RGBA32Float readback is exactly [f32::MAX/4, -f32::MAX/4, 1.25, 1]. Source alpha is associated; background is zero. Existing per-channel exponent scaling prevents accumulation overflow without erasing the small blue component. RGBA16Float output is exactly [0x7bff, 0xfbff, 0x3d00, 0x3c00], finite saturation to +/-65504, blue 1.25, alpha 1.

The managed exposure compute path at zero stops reads back every original source bit exactly, including signed zero and the tiny positive alpha. CPU and explicit compute budgets return to zero. This is the blocking compute/readback API, not proof of extreme-range resident compute-to-raster transport.

The test passes on actual Metal. No runtime shader change was necessary: this closes missing evidence for existing range handling. The linear-HDR constructor documentation now states the implemented finite saturation behavior. This does not qualify physical display transfer, EDR headroom, NVIDIA/CUDA, other interpolation geometries, nonzero-exposure overflow or all EXR variants.

Reproduce: RRRAH_GPU_OPTIONAL=0 CARGO_TARGET_DIR=/tmp/rrrah-alpha-isolated-target-20261016 cargo +stable test -p rrrah-gpu --lib associated_exr_extreme_float32_and_half_metal --locked --offline -- --nocapture
