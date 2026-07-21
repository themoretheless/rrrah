# RAW/ISP research review: what is production-ready in 2026

This document is the research appendix for rrrah. It separates methods that are
safe to ship in a cross-platform editor from methods that are interesting but
still experimental. The main conclusion is deliberately conservative: a fast
viewer needs a deterministic signal-processing path first, and a learned path
only as an opt-in quality tier with a reference implementation and a bounded
memory budget.

## Executive decisions

1. **Do not replace the RAW pipeline with one end-to-end neural network.**
   Learned ISP papers show excellent benchmark scores, but camera/sensor domain
   shift, temporal instability, hallucinated detail and model distribution are
   unacceptable defaults for an editor. Keep a transparent linear-light path.
2. **Use a two-tier demosaic.** Fast preview uses bilinear or MHC; quality uses
   RCD/AMaZE-class directional processing. MHC is attractive because it is a
   5x5 linear filter, has a small halo, and maps efficiently to SIMD/GPU.
3. **Treat DNG tiles as the parallel unit.** A CR2 lossless-JPEG entropy stream
   normally has a serial predictor dependency. DNG strips/tiles and frame
   bursts are independent and should drive the worker scheduler.
4. **Denoise in the physical noise domain.** Use a Poisson-Gaussian model and
   preserve metadata (ISO, black level, gain). Neural denoise is optional and
   must expose a confidence/strength control; it must never modify the
   immutable RAW cache.
5. **Fuse GPU passes by memory traffic, not by FLOP count.** RAW display is
   bandwidth-bound. Keep the mosaic in `R16Uint`/FP16, use halo tiles, fuse
   normalization + WB + demosaic + matrix where practical, and avoid CPU↔GPU
   round-trips.
6. **Benchmark first-frame latency separately from final-quality export.** A
   100 ms preview and a 10 s high-quality export are different products and
   must not be ranked with one number.

## Demosaicing research

### Classical algorithms (production candidates)

**Bilinear.** Four-neighbour interpolation is a good sanity/reference kernel:
it is deterministic, branch-light and has one-pixel halo. Its weaknesses are
zipper/false-colour artifacts and poor diagonal detail. Keep it for `fast` and
as a golden reference, not as the final quality mode.

**Malvar-He-Cutler (MHC).** Malvar, He and Cutler's ICASSP 2004 paper proposes
high-quality linear interpolation with 5x5 filters. The paper reports more than
5.5 dB PSNR improvement over bilinear at much lower complexity than nonlinear
methods. The [paper and abstract](https://www.microsoft.com/en-us/research/wp-content/uploads/2016/02/Demosaicing_ICASSP04.pdf)
give the filter derivation and coefficients. For rrrah, MHC is the best first
quality kernel: separable-ish neighbourhood access, fixed coefficients, SIMD
friendly and a two-pixel halo.

For a red sample, the green estimate can be expressed as a cross-gradient
correction:

\[
\hat G = G_0 + \alpha\,(2R_0-R_{-1}-R_{+1})
\]

and the blue estimate at a red site uses diagonal correlation. Implement the
published coefficients in a constant table, normalize DC gain exactly, and
test all four Bayer phases. The GPU implementation described by McGuire et al.
shows how to optimize MHC with local/shared memory and cache reuse
([Efficient, High-Quality Bayer](https://casual-effects.com/research/McGuire2009Bayer/bayer-jgt09.pdf)).

The implementation detail that matters for a fast kernel is that MHC is
bilinear interpolation plus cross-channel Laplacians. For a red-site green
estimate, `G = G_bl + alpha * Delta_R`; analogous terms use `beta` and `gamma`
for the other color/site combinations. The rounded dyadic coefficients are
`alpha = 1/2`, `beta = 5/8`, and `gamma = 3/4`; the filters can therefore be
stored as integer taps and divided by a power of two. Pascal Getreuer's
[IPOL derivation and reference implementation](https://www.ipol.im/pub/art/2011/g_mhcd/revisions/2011-08-14/g_mhcd.htm)
lists the eight phase-specific 5x5 filters. Use this as the scalar golden
reference and compare SIMD/WGSL output against it, not against a screenshot.

**Directional/AHD, DCB, RCD and AMaZE.** These algorithms estimate local
gradients and interpolate along the low-gradient direction, reducing zippering
at edges. They are the quality reference in mature converters. Their cost is
less predictable: branch divergence, larger neighbourhoods and several passes
make them harder to fuse in a fragment shader. RawTherapee documents the
available demosaic families and trade-offs in its
[demosaicing documentation](https://rawpedia.rawtherapee.com/Demosaicing).

Recommended implementation order:

```text
fast: bilinear (one pass)
balanced: MHC (one pass, radius 2)
quality: RCD/AMaZE-like directional pipeline (multi-pass CPU/GPU)
```

**Non-Bayer CFA.** Fuji X-Trans, Quad Bayer and RGBE cannot be treated as a
phase-swapped RGGB. The CFA must be represented as an arbitrary periodic
pattern, and each algorithm must declare the supported period. Failing closed
is safer than silently producing colour artifacts.

### Learned demosaicing (research/optional)

CNN demosaicing papers demonstrate improved PSNR, but models are trained on
specific sensors and datasets. [Learning Deep Convolutional Networks for
Demosaicing](https://arxiv.org/abs/1802.03769) reports joint demosaic/denoise
models; it is useful as a research baseline, not a universal camera model.

[Joint Demosaicking and Denoising by Fine-Tuning of Bursts of Raw Images](https://openaccess.thecvf.com/content_ICCV_2019/html/Ehret_Joint_Demosaicking_and_Denoising_by_Fine-Tuning_of_Bursts_of_Raw_ICCV_2019_paper.html)
is important because it trains from real RAW bursts rather than synthetic
RGB. It also shows that test-time adaptation to a burst can improve quality.
That is promising for a burst editor, but too expensive and non-deterministic
for first-frame viewing.

The 2024 paper [How to Best Combine Demosaicing and Denoising?](https://arxiv.org/abs/2408.06684) is especially actionable: for
moderate noise, demosaic first and denoise second is a good low-complexity
choice; only at high noise does partial CFA denoise before demosaic provide a
moderate gain. This supports a conventional fast path and avoids a large
always-on neural network.

Recent Mamba/State-Space models such as
[Retinex-RAWMamba](https://arxiv.org/abs/2409.07040) improve low-light RAW
benchmarks by coupling illumination decomposition with demosaic/denoise.
They remain experimental for rrrah because model weights, reproducibility,
camera generalization, and tile-boundary behaviour are unresolved.

## RAW denoising and physical models

The useful first-order sensor model is Poisson shot noise plus read noise:

\[
 y \sim \mathrm{Poisson}(g x) / g + \mathcal N(0,\sigma_r^2),
 \qquad \mathrm{Var}(y|x)=a x+b.
\]

Estimate `a` and `b` per camera/ISO from flat-field calibration frames. Apply
black-level subtraction and gain normalization before fitting; otherwise the
intercept is biased. A variance-stabilizing transform (generalized Anscombe)
can make a denoiser approximately homoscedastic, but the inverse transform must
be unbiased near black.

[Unprocessing Images for Learned Raw Denoising](https://openaccess.thecvf.com/content_CVPR_2019/html/Brooks_Unprocessing_Images_for_Learned_Raw_Denoising_CVPR_2019_paper.html)
is the practical reference for generating realistic RAW training pairs by
inverting an ISP and adding camera noise. It is useful for offline model
training, not as a substitute for real-camera validation.

[Learning to See in the Dark](https://openaccess.thecvf.com/content_cvpr_2018/html/Chen_Learning_to_See_CVPR_2018_paper.html)
introduced a paired short-exposure RAW/long-exposure reference dataset and an
end-to-end RAW network. It established that neural RAW processing can recover
very low-light detail, but also highlights exposure-dependent domain shift.

[Noise2Noise](https://arxiv.org/abs/1803.04189) shows that independent noisy
observations can train a denoiser without clean targets. For a photo editor,
this suggests burst denoise and self-calibration, but not blind modification of
a single frame.

Production order:

```text
P0: metadata-aware chroma/luma denoise with Poisson-Gaussian variance
P1: calibrated profile denoise per camera/ISO
P2: burst alignment + robust merge
P3: optional learned denoise with ONNX/Metal/Vulkan backend
```

All denoisers need a detail-preservation test set (foliage, hair, text, fabric,
stars) and a hallucination check. PSNR alone rewards over-smoothing.

## HDR merge and tone mapping

For exposure brackets, merge in scene-linear space. A robust weighted estimate
for irradiance is:

\[
 E(p)=
 \frac{\sum_i w_i(p)\,f^{-1}(R_i(p))/(t_i g_i)}
      {\sum_i w_i(p)+\epsilon}.
\]

`f` is the camera response (or linearized RAW), `t_i` exposure time, `g_i`
analog/digital gain, and `w_i` rejects saturated/underexposed samples. Align
frames before merge (phase correlation for translation, robust homography for
rotation/parallax). Saturation masks must be computed before tone mapping.

Reinhard's global operator remains a useful baseline:

\[
 L_d=\frac{L'}{1+L'}, \qquad L'=\frac{a}{\bar L}L.
\]

Use luminance-only tone mapping and scale RGB by `L_d/L`; independently
clipping RGB channels creates hue shifts. Local operators (bilateral or
multi-scale) are expensive and can create halos. A recent overview is
[Real-time Tone Mapping: A State of the Art Report](https://arxiv.org/abs/2003.03074).

For HDR display output, retain scene-linear FP16/FP32 until the output
transform. ICtCp/JzAzBz or OKLCH are preferable to per-channel clipping for
gamut reduction; use BT.2100 transfer and luminance limits for HDR10/HLG.

## Color science and validation

Camera matrices, DCP forward matrices and illuminant adaptation must be
explicit nodes. Build/invert matrices in f64, execute uniforms in f32, and
reject near-singular matrices:

\[
M_{cam\rightarrow rgb}=\left(M_{xyz\rightarrow cam}
M_{rgb\rightarrow xyz}\right)^{-1}.
\]

Use Bradford or CAT16 adaptation only when the profile's white points require
it. A DCP dual-illuminant profile should interpolate matrices by correlated
colour temperature rather than linearly mixing already gamma-encoded RGB.

For objective validation use ISO/CIE CIEDE2000. The CIE explains that ΔE00
corrects for lightness, chroma, hue and interaction terms and defines the
reference viewing conditions ([CIE standard](https://www.cie.co.at/publications/colorimetry-part-6-ciede2000-colour-difference-formula-0)).
Use ΔE00 on ColorChecker patches plus neutral-gray drift; use PSNR/SSIM only as
secondary metrics. For HDR emissive displays, CIEDE2000 assumptions do not
strictly apply; use JzAzBz/ICtCp metrics and report the viewing condition.

## Parallel decode and scheduling

Lossless JPEG entropy decoding has a dependency chain (Huffman bit position,
predictor and restart state). The best practical strategy is:

```text
sequential entropy lane
  → parallel row-band postprocess
  → parallel tile upload / mip generation
```

If restart markers or independent DNG tiles exist, partition at those
boundaries. JParEnt formalizes a useful heterogeneous design: a CPU pass finds
entropy boundaries, then GPU workers decode independent segments
([JParEnt](https://onlinelibrary.wiley.com/doi/10.1002/cpe.4111)). GPU Huffman
decoding is an active research topic; [Accelerating JPEG Decompression on
GPUs](https://arxiv.org/abs/2111.09219) and
[Massively Parallel Huffman Decoding on GPUs](https://par.cse.nsysu.edu.tw/resource/paper/2018/181023/Massively%20Parallel%20Huffman%20Decoding%20on%20GPUs.pdf)
show that it is possible, but setup, irregular control flow and PCIe copies
often dominate a single RAW frame. It is attractive for batches of many tiles
or many images, not necessarily for one CR2 first frame.

The scheduler should use weighted credits, not one thread per tile:

\[
 C_{decode}+C_{post}+C_{upload}\le C_{RAM}+C_{VRAM},
 \qquad
 P0>P1>P2>P3.
\]

Use bounded queues, generation cancellation and backpressure. DNG tiles are
the primary parallel unit; CR2 row postprocessing and prefetch are secondary.

## GPU implementation and recent practical innovations

* **Subgroups/wave operations.** Vulkan subgroup operations enable reductions,
  ballot and quad-like neighbourhood work without shared-memory barriers. They
  are optional features and subgroup size varies, so provide scalar fallback
  ([Vulkan subgroup guide](https://docs.vulkan.org/guide/latest/subgroups.html)).
* **Async staging and fused kernels.** Use a ring of persistently sized staging
  buffers, overlap decode→upload→render, and fuse normalization/WB/demosaic
  when the intermediate is not reused. The roofline estimate is
  `T >= max(bytes/BW, FLOPs/peak)+sync`.
* **CUDA graphs / async copy.** CUDA graphs reduce launch overhead for a stable
  tile graph, and `cuda::pipeline` supports asynchronous global→shared copies
  ([CUDA graphs](https://docs.nvidia.com/cuda/cuda-programming-guide/04-special-topics/cuda-graphs.html),
  [CUDA programming guide](https://docs.nvidia.com/cuda/cuda-programming-guide/pdf/cuda-programming-guide.pdf)).
  These are optional vendor backends, not reasons to abandon wgpu.
* **Apple tile/imageblock paths.** Metal exposes imageblocks and tile shaders
  suited to local image filters; use them in an Apple backend only after the
  portable WGSL path is correct ([Metal documentation](https://developer.apple.com/documentation/metal)).
* **Texture arrays + halo tiles.** The portable design for dimensions beyond a
  single texture limit is a texture array or virtual texture. Keep a one/two
  pixel halo and calculate CFA parity in full sensor coordinates.

## What is obsolete or dangerous

* Uploading an embedded JPEG and calling it a RAW viewer.
* Full-frame `f32` intermediates for every node; use tile-local FP16 where the
  error budget permits and keep the immutable mosaic in compact `u16`.
* Unlimited thread pools and unbounded prefetch; they increase tail latency and
  evict useful cache pages.
* Calling `poll(Wait)` or synchronizing the GPU on every tile; this destroys
  overlap and interactive latency.
* Applying exposure, denoise or tone mapping in gamma-encoded RGB.
* Treating all DNG files as simple Bayer; OpcodeList, linearization tables,
  black-level grids and non-Bayer CFA must be validated explicitly. Adobe's
  current DNG SDK release notes include security fixes, a reminder that parser
  hardening is part of feature completeness ([Adobe DNG SDK](https://helpx.adobe.com/au/camera-raw/digital-negative.html)).
* Reporting only average milliseconds. Release decisions require p95/p99,
  peak RSS/VRAM, cache state and quality metrics.

## Recommended implementation order for rrrah

### P0: measurable, deterministic

1. DNG tile/strip planner and bounded scheduler.
2. MHC GPU kernel plus scalar golden reference; retain bilinear fast mode.
3. Complete black-level grids, linearization and DNG OpcodeList support or
   explicit degraded-mode refusal.
4. CPU tile cache + GPU residency LRU + staging ring.
5. Poisson-Gaussian profile denoise (CPU/GPU), with calibrated fixtures.
6. Quality benchmark corpus: ColorChecker, Siemens star, foliage/hair/text,
   saturated highlights, dark flats, X-Trans and Quad Bayer.

### P1: quality and workflow

1. RCD/AMaZE-class quality tier.
2. HDR bracket merge with motion masks.
3. DCP dual-illuminant/ICC output and gamut mapping.
4. Burst denoise/merge with robust alignment.
5. Live profiler: CPU spans, GPU timestamps, queue depth, tile residency,
   dropped frames, RSS/VRAM.

### P2: optional research

1. ONNX/Metal/Vulkan learned denoise/demosaic models.
2. Test-time burst adaptation and RAWMamba-like low-light model.
3. Vendor backends (CUDA graph, Metal tile shader, Vulkan subgroup).

Every P2 model must ship with fixed weights, a deterministic CPU fallback,
model-license metadata, a maximum memory budget and a per-camera validation
report. No learned output should overwrite the original RAW or be silently
presented as physically faithful data.

## References

Primary references are linked inline. Open-source practice references include
[RawSpeed](https://github.com/darktable-org/rawspeed),
[darktable performance/tiling](https://docs.darktable.org/usermanual/development/en/special-topics/mem-performance/),
[darktable pixelpipe](https://docs.darktable.org/usermanual/development/en/darkroom/pixelpipe/the-pixelpipe-and-module-order/),
[RawTherapee](https://github.com/RawTherapee/RawTherapee),
[LibRaw](https://www.libraw.org/docs), and
[RapidRAW](https://github.com/CyberTimon/RapidRAW). These projects are useful
implementation comparators, but their quality, licensing and feature scopes
are not interchangeable; benchmark them only with the same RAW corpus,
quality tier, cache state and output metric.
