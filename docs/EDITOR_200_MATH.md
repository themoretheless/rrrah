# F101–F200: математический контракт расширенного RAW-редактора

Этот документ расширяет `EDITOR_100.md` ещё на 100 функций. Функция считается
готовой только если есть: (1) детерминированный математический контракт,
(2) CPU/GPU реализация с bounded memory, (3) reference/golden corpus,
(4) benchmark gate и (5) отрицательные тесты. Здесь описан проектный контракт;
это не утверждение, что весь список уже реализован.

## Глобальные численные правила

Все вычисления до display transfer выполняются в scene-linear RGB. Для
аккумуляторов HDR, статистик и матриц используется `f32` runtime с `f64`
accumulation там, где ошибка суммирования может быть заметна. FP16 допустим
только для промежуточных GPU tiles после оценки error budget.

```text
relative_error(a,b) = |a-b| / max(|b|, 1e-6)
finite(x)            = isfinite(x) && !NaN
clamp01(x)           = min(max(x, 0), 1)
```

Базовые release gates:

```text
ΔE00 median ≤ 0.5, p95 ≤ 1.5 на ColorChecker
linear-light PSNR ≥ 45 dB для lossless reference
SSIM ≥ 0.995 для resize/merge без intentional detail loss
geometric reprojection p95 ≤ 0.5 px (планарные сцены)
seam error ≤ 1 LSB (u16) или ΔE00 ≤ 0.2 (RGB)
deterministic hash одинаков при 1/2/4/8 workers
```

## F101–F110 — HDR, bracket merge и highlight recovery

| ID | Функция | Математический контракт | Benchmark/тест |
|---|---|---|---|
| F101 | Bracket exposure grouping | Группировать кадры по `EV = log2(t·N²/ISO)`; допуск группы `|ΔEV|≤1/12 stop`, timestamp gap — отдельный gate | grouping precision/recall на EXIF corpus |
| F102 | RAW radiance calibration | `E_i(x)=f^{-1}(R_i(x))/(t_i·g_i)` после black/white linearization; `f` — camera response LUT | inverse-response RMSE, monotonicity |
| F103 | Debevec response solve | Минимизировать `Σ w(z)[g(z)-ln E-ln Δt]² + λΣg''²`, фиксировать `g(z_mid)=0` | residual <1%, no non-finite |
| F104 | Robertson merge | Weighted radiance `E=Σw_i E_i/Σw_i`; zero denominator маркируется invalid, не заменяется чёрным | PSNR/ΔE vs synthetic ground truth |
| F105 | Motion-compensated HDR | Optical flow `u_i` применяется до merge; confidence `c=exp(-|∇I·u+I_t|/σ)` входит в `w` | ghost area <0.5%, endpoint error |
| F106 | Ghost detection | `G=median_i |L_i−median(L)|/(σ+ε)`; pixels `G>k` исключаются, `k` калибруется на noise | false ghost/false reject ROC |
| F107 | Highlight reconstruction | Для clipped channel решать chroma-preserving `Ĉ = α·C_valid`, `α=min(valid)/max(valid)`; confidence уменьшается по saturation | clipped-area ΔE00 p95 ≤ 3 |
| F108 | HDR local tone map | Base/detail: `L=Gσ*Y`, `D=Y/(L+ε)`, compress `L' = L/(1+L)`; detail gain bounded `[0.5,2]` | halo/gradient reversal tests |
| F109 | PQ/HLG HDR output | PQ `N= ((c1+c2Y^m)/(1+c3Y^m))^n`, map scene nits to mastering peak; no direct gamma substitution | luminance error ≤2% at 1/10/100/1000 nits |
| F110 | HDR merge memory pipeline | Streaming tiles with overlap; peak bytes `O(k·tile_area·planes)`, not `O(k·frame_area)` | RSS slope vs bracket count; deterministic output |

Опасные shortcut-решения: усреднять уже gamma-encoded JPEG, игнорировать
выдержку/ISO, считать clipped channel равным white level, либо применять tone
mapping до merge. Это даёт визуально привлекательный, но физически неверный
результат и ломает ΔE/PSNR.

## F111–F120 — panorama, registration и multi-image geometry

| ID | Функция | Математический контракт | Benchmark/тест |
|---|---|---|---|
| F111 | Feature extraction | Scale-space extrema с affine descriptor; descriptor distance — ratio test `d1/d2<0.75` | repeatability under scale/rotation |
| F112 | Pairwise matching | RANSAC homography `x'~Hx`; minimal sample 4 points, symmetric transfer error | inlier precision ≥99% на synthetic |
| F113 | Robust homography | Minimize `Σρ(||x'_i−π(Hx_i)||²)` with Huber loss; reject degenerate `det(A)<ε` | p95 reprojection ≤0.5 px |
| F114 | Cylindrical projection | `θ=atan(x/f), h=y/sqrt(x²+f²)`; preserve vertical lines and f estimate | line bending RMS |
| F115 | Spherical projection | `p=R·normalize(K⁻¹x)`; seam domain uses longitude wrap, no Euclidean edge test | sphere reprojection p95 |
| F116 | Global pose graph | Optimize `Σ e_ijᵀΩ_ij e_ij` with gauge fixed; robust kernel for bad links | loop-closure drift <0.2% |
| F117 | Exposure/color balancing | Solve per-image log gain/bias `l_i(x)=a_i·l_ref+b_i`; regularize `a≈1` | luminance seam ΔE ≤1 |
| F118 | Seam optimization | Min-cut cost `C=|∇I_a−∇I_b|+λ|I_a−I_b|`; feather only after path | visible seam rate <1% |
| F119 | Multi-band blend | Laplacian pyramid `B_l=G_l(M)A_l+(1−G_l(M))B_l`; normalize border energy | PSNR vs reference, no ringing |
| F120 | Gigapixel tiled panorama | Tile coordinates in 64-bit; pyramid level `l` scale `2^-l`, bounded queue by viewport | 100k×50k scene RSS < budget; tile p95 |

Опасно использовать affine-only blend без geometric registration, сшивать по
фиксированной вертикали или оптимизировать homography без gauge constraint:
появятся drift, curved horizon и накопленная ошибка по циклу.

## F121–F130 — spectral/color science и camera profiles

| ID | Функция | Математический контракт | Benchmark/тест |
|---|---|---|---|
| F121 | Spectral sensitivity model | `RGB=∫S(λ)E(λ)dλ`; quadrature error контролируется Δλ и smoothness | ΔE vs 1 nm reference |
| F122 | Illuminant estimation | Solve nonnegative coefficients `E≈Σc_j I_j`, `c_j≥0`, L2+smoothness | CCT Δuv error |
| F123 | CCT/tint solver | Robertson/Planckian nearest point in uv; return uncertainty if distance > threshold | ±2 mired, tint ≤1 |
| F124 | DCP dual-illuminant interpolation | `M(T)=α(T)M_A+(1−α)M_B`; interpolate in reciprocal temperature, not Celsius | ColorChecker ΔE p95 |
| F125 | Chromatic adaptation | Bradford `M_Bradford·LMS`, scale white responses, inverse transform; preserve neutral | neutral ΔE00 ≤0.3 |
| F126 | ICC parametric TRC | Piecewise ICC curve, explicit breakpoint and sign; use LUT only after monotonic validation | round-trip ΔE/monotonicity |
| F127 | Spectral metamer analysis | Null-space of sensor matrix estimates illuminant-sensitive residual; flag out-of-gamut | false metamer rate |
| F128 | Gamut compression | Jz/Cz or OKLCH chroma compression preserving hue/lightness; solve `C'≤Cmax(L,h)` | hue error ≤1°, ΔE p95 |
| F129 | Color appearance mode | CAM16/Jzazbz adaptation with viewing surround/white luminance; parameters serialized | cross-device ΔE |
| F130 | Reference profile validation | Matrix/LUT profile must pass identity, white, primary, neutral and inversion tests | profile gate; no NaN/negative determinant |

Опасно интерполировать matrices в sRGB, смешивать D50/D65 без adaptation,
применять gamma до matrix, либо использовать clamp вместо gamut mapping: это
скрывает ошибки и разрушает neutral axis.

## F131–F140 — denoise, super-resolution и AI, контролируемые физикой

| ID | Функция | Математический контракт | Benchmark/тест |
|---|---|---|---|
| F131 | Sensor noise model | `σ²(r)=a·r+b`; estimate `a,b` из flat/dark frames, propagate through WB | SNR error ≤5% |
| F132 | Variance-stabilizing transform | Poisson-Gaussian Anscombe `2/√a·sqrt(a·r+3a²/8+b)`; inverse bias correction | low-light PSNR |
| F133 | Edge-aware denoise | Weighted least squares `Σw_p(x_p−y_p)²+λΣg(|∇x|)|∇x|²` | texture retention/acutance |
| F134 | Non-local means | Patch distance in linear/raw domain, bounded search; deterministic top-K | noise reduction vs edge loss |
| F135 | Burst denoise | Align frames, robust temporal median/M-estimator with confidence; reject motion | ghost pixels <0.2% |
| F136 | AI denoise inference | Model input/output color space/version/hash serialized; tiles with receptive-field halo | PSNR/SSIM and reproducibility hash |
| F137 | Super-resolution | Scale factor `s`, anti-alias prefilter; no hallucinated detail in “strict” mode | MTF gain + false-detail detector |
| F138 | Deblur | Wiener `X=H*Y/(|H|²+K)` or constrained Richardson-Lucy; cap iterations | ringing/edge overshoot |
| F139 | Defective pixel repair | Interpolate by CFA color neighborhood; never copy opposite-color sample directly | defect residual and CFA purity |
| F140 | AI safety fallback | Detect model OOM/NaN/unsupported GPU, preserve pre-AI output, mark provenance | identical fallback checksum |

Опасно измерять AI только по perceptual score: сеть может дорисовать детали,
которых не было в RAW. Нужны strict mode, provenance, no-hallucination corpus,
raw-space error и golden checksum.

## F141–F150 — temporal/video RAW и tracking

| ID | Функция | Математический контракт | Benchmark/тест |
|---|---|---|---|
| F141 | Frame timestamp sync | PTS в integer nanoseconds; drift `d(t)=a·t+b`, fit least squares | sync error ≤0.5 frame |
| F142 | Temporal denoise | `x_t=α_t y_t+(1−α_t)warp(x_{t−1})`, `α` confidence-based | flicker variance and ghost |
| F143 | Optical flow | Pyramid Lucas–Kanade/modern flow; confidence from residual and occlusion | endpoint error, occlusion recall |
| F144 | Flow-guided warp | Backward sampling with validity mask; never forward splat without normalization | warp hole rate |
| F145 | Object tracking | Kalman state `[x,y,vx,vy]`, covariance propagated; Mahalanobis gate `d²<χ²` | MOTA/ID switches |
| F146 | Feature track graph | Track points by descriptor + flow; RANSAC re-anchor after drift | track survival % |
| F147 | Rolling-shutter correction | Row time `t(y)=t0+y·readout/H`; warp pose interpolation per row | reprojection p95 |
| F148 | Deflicker | Estimate low-frequency exposure `g_t`; solve `min Σ(g_t−g_{t−1})²+λdata` | luminance flicker RMS |
| F149 | Video HDR merge | Merge bracket/exposure slices per PTS; temporal confidence decays with motion | ghost/flicker joint gate |
| F150 | Frame cache residency | Ring buffer memory `N·frame_tile_bytes`, backpressure producer on high-water mark | dropped frames, RSS bound |

Опасно усреднять соседние кадры без flow/occlusion, сравнивать PTS в float
seconds или интерполировать rolling shutter одним global transform.

## F151–F160 — print, proofing и hardcopy color

| ID | Функция | Математический контракт | Benchmark/тест |
|---|---|---|---|
| F151 | Printer ICC transform | Source PCS → printer PCS → device LUT; preserve profile intent metadata | ΔE00 chart |
| F152 | Rendering intents | Relative, absolute, perceptual, saturation — explicit enum; no silent fallback | intent-specific golden |
| F153 | Black point compensation | Map source/destination black using piecewise affine; preserve shadow ordering | shadow ΔE/monotonicity |
| F154 | Soft proof | Simulate printer gamut then display transform; proofing white/black configurable | screen-to-proof ΔE |
| F155 | Ink limit | `Σ channel ink ≤ limit`; preserve hue using constrained projection | TAC violations =0 |
| F156 | Screening/resolution | Dot gain model `D_out=f(D_in, frequency)`; resample with sinc/Lanczos bounds | MTF, moire |
| F157 | Tiling print raster | Render strips/tiles with overlap and exact integer pixel origin | tile seam ≤1 LSB |
| F158 | ICC profile embedding | Output bytes must embed exact profile hash and intent | profile parse round-trip |
| F159 | Print size/DPI solver | `pixels = round(mm/25.4·dpi)`; report fractional crop, not implicit stretch | dimension exactness |
| F160 | Proof comparison | Difference in Lab/Jz with ΔE heatmap and clipping mask | operator agreement / threshold |

Опасно отправлять linear RGB принтеру, считать DPI метаданными без изменения
пикселей, или использовать relative colorimetric без black-point compensation.

## F161–F170 — focus stack, depth и computational photography

| ID | Функция | Математический контракт | Benchmark/тест |
|---|---|---|---|
| F161 | Focus measure | Tenengrad/Laplacian energy on linear luminance; normalize by local variance | focus ranking accuracy |
| F162 | Focus-stack alignment | ECC/phase correlation before selection; subpixel translation | alignment p95 <0.25 px |
| F163 | Depth-from-focus | Fit sharpness curve `q(z)` with unimodal spline; confidence from curvature | depth RMSE |
| F164 | Stack fusion | Select max-focus with guided smoothness; avoid hard seams via Laplacian blend | edge continuity/SSIM |
| F165 | Depth-map regularization | `E(d)=Σρ(data)+λΣw|∇d|`; preserve discontinuities | bad-pixel rate |
| F166 | Relighting normal estimate | `n` from multi-light least squares `I=ρ(n·l)`; enforce `||n||=1` | angular normal error |
| F167 | Synthetic bokeh | Circle of confusion `c=A|z−zf|/z`; depth-aware gather with foreground dilation | edge halo rate |
| F168 | Light-field refocus | Shift-and-sum `Iα(x)=ΣI_i(x+α·u_i)`; normalize valid count | focus-plane sharpness |
| F169 | Multi-camera calibration | Bundle adjustment over intrinsics/extrinsics; reprojection residual gate | calibration RMS |
| F170 | Depth confidence/provenance | Confidence combines curvature, texture, occlusion; invalid stays invalid | no fabricated depth in flat regions |

Опасно строить depth из одной sharpness map без alignment и confidence: это
даёт halos, foreground bleeding и ложную геометрию.

## F171–F180 — advanced masks, semantic and selective processing

| ID | Функция | Математический контракт | Benchmark/тест |
|---|---|---|---|
| F171 | Semantic segmentation | logits → calibrated probability; threshold serialized with model version | mIoU, calibration ECE |
| F172 | Matting | Alpha `α∈[0,1]`, compositing invariant `C=αF+(1−α)B`; trimap-aware loss | SAD/MAD/gradient error |
| F173 | Subject-aware mask | Combine semantic and edge matting by normalized product, not raw sum | boundary F-score |
| F174 | Sky replacement mask | Horizon/occlusion confidence; depth-aware feathering | sky edge ΔE/halo |
| F175 | Color relight mask | Preserve luminance/chroma constraints in OKLCH; clamp only after transform | hue drift ≤1° |
| F176 | Frequency mask | FFT band-pass with Hermitian symmetry; inverse realness error bounded | reconstruction RMSE |
| F177 | Local contrast mask | Guided filter coefficients solved per tile; halo-free radius bound | flat-field uniformity |
| F178 | Mask vector export | Raster→Bezier contour with Hausdorff tolerance ε; preserve holes/winding | contour Hausdorff ≤ε |
| F179 | Mask temporal propagation | Warp mask with flow and confidence, re-seed after occlusion | mask IoU over time |
| F180 | Mask audit/provenance | Every mask stores source/model/threshold/coordinate space and hash | replay checksum |

Опасно делать selection в gamma RGB, складывать alpha без premultiplication,
или бинаризовать soft matte до blur/composite.

## F181–F190 — catalog search, measurement и scientific analysis

| ID | Функция | Математический контракт | Benchmark/тест |
|---|---|---|---|
| F181 | Perceptual thumbnail embedding | Embedding normalized `v/||v||`; cosine similarity for near-duplicate search | recall@K/latency |
| F182 | Duplicate clustering | Union-find over perceptual distance + temporal/file constraints | precision/recall |
| F183 | Exposure histogram | Histogram in scene-linear and display spaces separately; weighted percentile | bin determinism |
| F184 | False-color/clip map | `clip_hi = r≥w−δ`, `clip_lo=r≤b+δ`; sensor-space before tone | pixel classification accuracy |
| F185 | Waveform/vectorscope | Aggregate tiles into bounded bins; vectorscope uses chroma coordinates | throughput, bin error |
| F186 | Focus peaking | Gradient magnitude after luminance linearization; hysteresis threshold | false positive/negative |
| F187 | Noise/flatness analysis | Robust MAD `σ≈1.4826·median|x−median(x)|` per patch | estimator bias |
| F188 | MTF/SFR analysis | ISO 12233 edge spread → derivative LSF → FFT MTF; report MTF50/10 | frequency error ≤2% |
| F189 | Color checker auto-detect | Homography to chart, patch medians excluding borders | patch localization p95 |
| F190 | Scientific measurement export | Units/white/reference encoded; values stored float64 and rounded only presentation | round-trip exactness |

Опасно строить histogram после tone map, считать clipping по display white или
измерять MTF на sharpened preview: это не измерение сенсора.

## F191–F200 — reproducibility, performance governance и production safety

| ID | Функция | Математический контракт | Benchmark/тест |
|---|---|---|---|
| F191 | Deterministic reduction | Pairwise/Kahan sums; fixed tile order for golden mode | bitwise equality workers 1/2/8 |
| F192 | Numeric precision tiers | `preview=f16`, `balanced=f32`, `reference=f64 accumulator`; explicit error budget | tier ΔE/PSNR gates |
| F193 | Tile roofline model | `T≥max(bytes/BW, flops/FLOPS)+latency`; include upload/sync separately | model residual ≤20% |
| F194 | Adaptive worker controller | Target queue latency; PID bounded `[1,max_workers]`, no oscillation | p95 latency/RSS stability |
| F195 | Thermal throttling policy | Detect sustained freq/temp, lower concurrency before frame deadline miss | long-run throughput drift |
| F196 | Crash/recovery journal | Edit transaction `prepare→fsync→commit`; replay idempotent by operation UUID | kill-at-every-step corruption=0 |
| F197 | Reproducible benchmark runner | Pin CPU affinity, governor, GPU adapter, corpus hash, build hash; report uncertainty | rerun CI overlap |
| F198 | Quality/performance Pareto | Pareto frontier over `(latency, RSS, ΔE, PSNR)`; reject dominated configs | frontier stability |
| F199 | Cross-backend conformance | CPU reference vs Metal/Vulkan/DX12/WGSL within tolerance; coordinate spaces identical | max error gates |
| F200 | Capability negotiation | Feature matrix from device/decoder/profile; unsupported path explicit, never silent JPEG fallback | negative tests and UX error |

Опасно оптимизировать только среднее время, смешивать cold/warm cache,
сравнивать разные quality tiers, или разрешать недетерминированные atomic
reductions в golden tests.

## Минимальный benchmark contract для F101–F200

Каждый benchmark сохраняет JSONL запись:

```json
{
  "feature": "F105",
  "fixture_sha256": "…",
  "build_sha": "…",
  "backend": "wgsl-metal",
  "workers": 8,
  "cache": "cold",
  "quality": "balanced",
  "wall_ms": 0.0,
  "p50_ms": 0.0,
  "p95_ms": 0.0,
  "peak_rss_bytes": 0,
  "peak_vram_bytes": 0,
  "delta_e00_p95": 0.0,
  "psnr_db": 0.0,
  "ssim": 0.0,
  "determinism_hash": "…"
}
```

Latency suites: `n≥30` после двух warmups; export/large HDR: `n≥10`.
Публикуются median, p95, p99, MAD и bootstrap 95% CI. Release regression
прекращает сборку при `p95 +10%`, RSS/VRAM `+10%`, ΔE00 p95 `+0.2`, PSNR
`−0.5 dB`, SSIM `−0.002`, seam `>1 LSB` или нарушении deterministic hash.

### Corpus

Минимальный corpus должен включать: CR2 без restart markers, CR2 с markers,
tiled/strip DNG, float LinearRaw, dual-illuminant DCP, clipped highlights,
moving HDR bracket, panorama с loop closure, low-light burst, X-Trans и
malformed adversarial files. Один Canon CR2 недостаточен для вывода о scaling.

### Acceptance rule

Нельзя объявлять функцию «быстрой», если ускорение получено ценой смены
quality tier. Отчёт обязан показывать Pareto frontier и отдельно first-frame,
interactive frame-time и final export. Все shortcut-режимы маркируются как
preview и не используются для scientific/print claims.
