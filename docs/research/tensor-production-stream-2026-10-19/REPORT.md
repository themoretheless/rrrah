# Streaming adaptive Tensor shading in ordinary PDF/AI rendering

Ordinary one-to-four-component Tensor shading now uses admitted adaptive geometry at geometry1/64pixel, source-component1/1024, max depth12 and1048576 triangles. Coincident-source-patch filtering and source iteration order are preserved. Coons patches and Tensor sources with more than4 components retain their existing routes. These floating-point bounds are not interval or nonlinear ICC/output-color certificates; arbitrary partial-overlap integration is not qualified.

A retained-mesh implementation failed the unchanged64MiB original-AI cache gate: repeated prefix growth exceeded its budget, and preallocating the complete output revealed more than60MiB of triangles alone. The production route therefore keeps the shared-edge/leaf plan but emits triangles directly into the existing pixel sampler. It does not retain the full geometry vector. The sampler is shared with iterator-based producers so accumulation/normalization order remains consistent. On error/cancellation its partial map is discarded. The new streaming API documents that callers must discard a partial sink on error; tests verify source-order bit equivalence, consumer refusal, cancellation, allocation refusal and retry.

Owned-mesh APIs retain the separate capacity improvements: compact edge tables and plan global output capacity before emitting. Their interface remains available; the viewer avoids their unnecessary retained geometry.

Failures in adaptive encoding propagate through all nested renderer/mask instances to the PDF warning sink. rrrah returns a typed shading decode/precision diagnostic instead of caching an incomplete transparent result; cancellation and concrete memory errors keep priority. A finite depth-limit PDF verifies refusal, zero retained credit and a subsequent valid retry. The earlier research fixture used scientific-number syntax, which PDF does not support; it was not a valid depth-limit control. The new generated fixture uses valid decimal tokens and rebuilt cross-reference offsets. Historical artifacts remain historical.

Independent analytic control:32x64 parabolic solid-red patch, x=32u,y=2+8v+128u(1-u). Piecewise polynomial integration over each pixel defines its alpha. The old fixed722-triangle route has maximum alpha error19; the ordinary adaptive route now matches all2048 alpha bytes exactly, with correct visible red channels. The generator and fixtures are CC0.

Actual Metal Apple M4 Max pipeline gate: source decoding, independent complete RGBA oracle, float preparation, pinned RAM under full root pressure, native RGBA8 and prepared-float swap restore, and10 complete SDR/half-float frame comparisons. A64x96 viewport gives fit=1 with16-pixel margins, covering every32x64 source pixel. Decoder accounted peak3632714bytes fits the unchanged8MiB root; last-owner credit is zero. Golden preparation/GPU resources are separate from that CPU root. This is not whole-process RSS or physical HDR calibration.

The pinned3-page AI cache pressure gate also passes on Metal at its unchanged64MiB root, with shared full-pressure hits and final accounted memory zero. This uses native-reference128x96 viewports, not full-page independent appearance proof. Fresh strict Poppler26.10.0 comparison still fails2/3pages: VectorApple33167 differing pixels, MAE4.009534613618562; one.ai page0 has2522 differences and page1 is exact. AI remains pending, including PostScript support.

Warm CPU timing,4alternating pairs/fixture with9iterations after3warmups per process (development optimized profile, shared host):

| Fixture | Fixed-grid ready p50 | Stream ready p50 | Stream/fixed pair median | Fixed/stream accounted peak |
|---|---:|---:|---:|---:|
| Solid planar Tensor |0.445ms|0.198ms|0.447|507132 /163148 bytes|
| Analytic curved Tensor |0.563ms|18.399ms|32.627|402426 /3632810 bytes|
| Original VectorApple |395.167ms|456.923ms|1.153|5601482 /26534362 bytes|

These are warm-file in-process decode+display-preparation times, excluding startup, hashing, destruction, GPU and decoded-image caching. Baseline and candidate differ only in encoder implementation, with the streaming source restored after the baseline build. Both binaries use the same remaining sources, lockfile, compiler and settings. Repeated pixel hashes are validated; the flat control requires identical cross-variant hashes. Curved/original output changes intentionally, backed by the separate pixel comparisons. There is no general speedup claim: the curved precision cost and approximately15% VectorApple slowdown remain optimization work. The4-pair exploratory experiment does not establish release performance or cold/slow-disk latency.

Final validation: vector185passed/10ignored, app287/81, public decoder216/5, internal interpreter134/1 and renderer312/12:1134ordinary passes, zero failures,109ignored. Two explicitly ignored actual-Metal tests were run separately and passed. Workspace fmt and diff whitespace checks pass. No fresh all-workspace/all-target/all-feature/strict-Clippy release gate was run for this change. Adjacent logs, binary/source hashes, timing samples and Poppler metrics provide exact evidence.

The original100-format/cache/GPU objective remains incomplete. The memory-safe ordinary Tensor integration is implemented; complex-mesh CPU speed, broader geometry/color qualification, six pending formats, NVIDIA/CUDA hardware and calibrated physical HDR remain open.
