# Signed linear float/HDR JPEG XL animation

## Independent producer and native values

Authored CC0 three-frame5x3RGBA32F source declares linear sRGB,32bit/8exponent-bit color and alpha,24000/1001TPS, ticks3/7/11 and2plays. Source RGB includes negative values(-2,-0.25), zero, values above one(up to16), and adjacent1+2^-23/1+2^-22codes. Alpha is0/0.25/0.5/1 with nonzero invisible RGB. Independent libjxl C decode compares every source float bit against the authored input and exports canonical little-endian float samples. Native Rust decode matches all180float values exactly across3frames.

Installed reference libjxl0.12.0 initially returned generic encoding failure for mixed-sign float input. Effort7 plus disabled patches/RCT still failed; adding zero modular predictor produced valid bit-exact files without changing input values. This is consistent with upstream [issue4902](https://github.com/libjxl/libjxl/issues/4902), not a traced proof of the internal error location. Workaround applies solely to reference encoding; Rust production decoding does not link libjxl. Initial failures and accepted configuration are preserved. Producer/sample hashes are in jxl-float-animation.json; regeneration to a separate directory is byte-exact. Other float predictors/blends are not thereby qualified.

## Reproduced production display defects and fix

Before change, native bit comparison passed but ordinary prepare-for-display returned InvalidInput for valid signed linear ICC samples. Input validation previously required nonnegative values even for exactly linear matrix/gray profiles. It now permits finite signed values only when the existing strict linear-TRC/no-A2B-LUT predicate holds. Nonlinear/LUT profiles remain normalized-input-only, alpha remains finite0..1, and output must fit finite float32. No memory limit or numeric tolerance changed.

Simply admitting negatives exposed another issue: gamma-one represented as a parametric curve clipped negative operands, while explicit identity curves preserved them. A five-profile independent test failed by about2units on a negative channel. Exact linear curves are now locally canonicalized to the mathematically equivalent empty/identity ICC curve before building the transform. Original profile bytes/metadata remain in the source raster. This does not approximate nonlinear curves or extend LUT domains. Existing endpoint-safe pure-gamma handling remains for other curves.

## Independent color reference

The actual jxl-oxide synthesized ICC bytes used in production are exported by jxl_animation_probe FILE --icc OUTPUT and byte-checked by the JXL test. generate-signed-linear-icc-oracles.py applies LittleCMS2.19 f64 relative-colorimetric conversion with optimizations/cache disabled to these same profile bytes, targeting linear sRGB/D65. References cover3RGB profiles(JXL, PSD-tools, explicit identity) and2gray profiles(PSD-tools, identity), plus each of the45JXL pixels. Finite signed/HDR input vectors and normalized alpha are pinned separately.

Maximum absolute difference across all five ICC profiles and all JXL frames is1.9073486328125e-6, below the unchanged2e-5bound. JXL native/source bits and alpha are exact; transformed RGB is compared numerically, not claimed bit-identical after matrix conversion. The largest source16 maps to about16.00657 through the quantized ICC matrix, so comparing display RGB directly with original linear numbers would use a different color transform. Same-profile LittleCMS avoids that false reference. Script, source/profile/oracle hashes and raw per-profile maxima are preserved.

## Memory/cache/Metal and validation

Final animation library35passed/1ignored; public decoder221passed/5ignored; ordinary app288passed/81ignored:544passes/87ignored/0failures. Separate actual-Metal test passed on Apple M4 Max. The repeated metric runs use the same compiled test binaries for captured numeric output and are not counted as additional independent cases. Formatting and source whitespace checks pass; no full all-target/all-feature/strict-Clippy gate is asserted.

Selected-frame Metal gate now covers12presentations: preceding6RGB8/binary-alpha,3finiteRGBA16 and3newlinearRGBA32F. Native/prepared swap buffers match in length and every8/16bit sample or float bit; timing/index/count persist through RAM and swap. Full-root restore refuses then retries; each swap writes1/reads1/errors0. SDR and half-float frames match their native-prepared reference. For each new float frame, the actual half-float attachment must additionally contain finite negative RGB and positive RGB above one, preventing a shared-clamping reference from hiding range loss. CPUroot1MiB per frame ends0; GPU resources are accounted separately. No allocation/budget increase was needed for signed ICC conversion.

These are tiny full-frame replacement, linear-sRGB samples with finite repeat metadata. They do not qualify complex blend/reference histories, every floating predictor, PQ/HLG JXL producers, gray/HDR animation, wide-gamut HDR profiles, infinity/NaN encodings, all malformed containers, large-animation latency, live GUI endurance, physical HDR calibration or deferred NVIDIA/CUDA. Source header/parser/profile scratch is not a whole-process RSS admission guarantee. The100-format objective remains active.

## Reproduction

python3 scripts/generate-jxl-float-animation-fixtures.py [--output TEMP]; build jxl_animation_probe then export --icc to animation-linear-float.icc; python3 scripts/generate-signed-linear-icc-oracles.py. With CARGO_TARGET_DIR=/tmp/rrrah-alpha-isolated-target-20261016: cargo +stable test -p rrrah-animation -p rrrah-decode --lib --locked --offline; cargo +stable test -p rrrah --bin rrrah --locked --offline. RRRAH_GPU_BACKEND=metal cargo +stable test -p rrrah --test pdf_readback --locked --offline jxl_animation_frames_preserve_metadata_ram_swap_and_metal -- --ignored --exact --nocapture.
