# Native Metal HDR window presentation — 2026-11-03

The current probe binary passes11 live native-window cases on Apple M4 Max: six HEIF/EXR controls, the original local EOS R8 CR3 and four STL/OBJ/OFF/PLY controls. Every case presents RGBA16Float/ExtendedSrgbLinear frames for at least3 seconds, confirms800x400 physical resize, presents at least30 frames after resize, releases managed input/sensor/geometry credit after upload and exits without GPU validation errors. Source shape/identity, source-credit release, same-frame OS HDR activation and headroom>1 are validated, not inferred from successful window creation.

| Inputs | Cases | Presented frames |
| --- | ---: | ---: |
| HEIF alpha/rotation/mirror and shifted-window EXR | 6 | 1044 |
| Original CR3,6188x4120/25494560 sensor samples | 1 | 136 |
| STL/OBJ/OFF/PLY,1/3/1/1 triangles | 4 | 697 |
| Total | 11 | 1877 |

The display API reports active HDR with current headroom reaching6.1538463. This is OS-reported relative headroom, not a measured luminance in nits. RAW uses the common SceneLinear RawRenderer; models use the common ModelRenderer, including the corrected OBJ admission path. The input budget is512MiB per case; its source/pixel/geometry credit is asserted zero after upload. Renderer/GPU/allocator bookkeeping has separate accounting and is not included in that zero claim.

[Case results](result.json) record every source, binary and log hash, exit status, duration and resize frame count. [Source pins](sources.json) and [build](build.log) identify the current code; media/pixels from the local photograph are not copied into this folder. The driver checks source and binary hashes again after each case to reject changes during the batch. Individual logs retain the source declarations, credit release, Metal adapter, display state and completion.

Seven [synthetic verifier tests](parser-tests.log) pass: missing/contradictory evidence, wrong backend/source/shape/credit, duplicate completion/source/credit, inconsistent RAW sample dimensions and empty model geometry cannot qualify. These controls are verifier tests, not additional hardware runs. Formatting/whitespace and Python syntax checks pass.

```sh
cargo build --locked -p rrrah --example hdr_surface_probe
python3 scripts/qualify-live-raster-hdr.py --binary target/debug/examples/hdr_surface_probe --output-dir /fresh/evidence --include-local-cr3 --include-models
```

The default script still covers the six raster controls; the optional original CR3 must match its pinned SHA256, and the optional model controls use the four authored fixture files. Source-kind environment selectors are cleared between cases and untagged color remains strict.

This is actual live probe-window presentation with production renderer APIs. It does not qualify full application navigation/cache/prefetch, independently calibrated photographic color/luminance, arbitrary source variants, or NVIDIA. Offscreen exact pixel/compute and sustained-worker proofs remain separate. The earlier separate positive raster/RAW surface reports and default-pattern baseline are superseded by this current batch; their duplicate artifacts were removed.
