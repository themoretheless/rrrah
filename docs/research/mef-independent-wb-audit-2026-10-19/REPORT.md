# Independent MEF white-balance reference audit

The existing pinned MEF investigation reports camera multipliers [0,1,0,0], while the later metadata audit found no usable gains in its seven non-text MakerNote blocks. Those are bounded prior file observations; this turn did not rerun sensor decoding or assert every file region lacks WB.

Fresh source inspection of LegacyDenoiser revision1b346c05524e21c4c784406dd49991196824a151 confirms explicit Mamiya ZD/.mef support. Its DNG conversion reads camera WB and requires positive components; otherwise it derives a D65 neutral from its color matrix and later writes AsShotNeutral. That fallback is an assumed illuminant, not independent evidence of this source camera's chosen WB. It therefore cannot qualify the missing native as-shot color route. No converter was executed and no dependency was added.

The fpexif repository revisionc4b85b9bd100f1717a64b9f18df26c4453100418 advertised three Leaf/Mamiya samples, but its tracked download helper contains instructions/placeholders rather than a new MEF fixture URL. This audit did not inspect every external repository mentioned by that project or establish that no other originals exist.

The photographer's current Mamiya review links the original Windows PhotoStudio archive and LegacyDenoiser. Neither software was installed or run. A primary producer/reference result with documented source WB remains a possible next route; the archive link alone is not such a result.

The reproducible read-only script records exact URLs, source hashes and checked branches in result.json. Search snippets, supported-format lists and preview-derived WB are not pixel/color oracles. MEF remains pending; no format count increase is claimed.

Primary sources:
- https://github.com/kristoffer-trolle/LegacyDenoiser/blob/1b346c05524e21c4c784406dd49991196824a151/src/legacy_denoiser/dng.py
- https://github.com/pgray/fpexif/blob/c4b85b9bd100f1717a64b9f18df26c4453100418/test-data/download-samples.sh
- https://www.kristoffertrolle.com/2024/mamiya-zd-digital-back-review/

## Current public refusal and complete-sensor Metal transport — 2026-10-24

The retained original Mamiya ZD source matches SHA-256 `bcd63507c3c4cc3ea1bad2945e3f88cb4677a1a3c762e1acc38df4445714eb7c`. Rebuilt current-code gates each pass 1/0failed/0ignored. `public-current.log` confirms the common image API classifies it as sensor, refuses the unqualified white-balance/color path, returns no preview, and peaks/ends at zero managed bytes. `metal-current.log` confirms native explicit sensor import and actual Apple M4 Max compute for all 21,461,504 source positions; synthetic signed RGB channel transforms/exposure and unchanged alpha are exact at every position. These authored channels are mathematical transport controls, not camera color or demosaicing.

Managed CPU peak 729,691,136 bytes, GPU accounting peak 1,030,152,288 bytes, both final zero. GPU log elapsed is diagnostic only, not a paired performance result. Reproduction: `RRRAH_MEF_SOURCE=/absolute/Mamiya-ZD.MEF cargo +stable test -p rrrah-decode --lib unqualified_mef_refuses_public_view_without_preview_fallback_or_leak --locked --offline -- --include-ignored --nocapture`, then `RRRAH_GPU_BACKEND=metal RRRAH_GPU_OPTIONAL=0 RRRAH_MEF_SOURCE=/absolute/Mamiya-ZD.MEF cargo +stable test -p rrrah --test mef_sensor_compute --locked --offline -- --include-ignored --nocapture --test-threads=1`.

No decoder/color support was enabled by this verification. MEF remains pending normal viewing because native independently validated WB is missing. BAY/CAP original-source and color gaps, other pending rows, physical HDR and NVIDIA are unchanged.

## Second independently published MEF original — 2026-10-24

A separate camera-produced MMFC0399.MEF is published in the photographer's [RawViewerGo sample directory](https://github.com/kristoffer-trolle/RawViewerGo/tree/20bf7501ae2a0f220dec33ed1b57b6f210fd8e4d/sample%20files), revision20bf7501ae2a0f220dec33ed1b57b6f210fd8e4d. The36,575,308-byte source has SHA-256e038801bce8924e519a36c7994086a3dc4cbd2e9d331ca4b755f2b2bdfc75012. It and the full independent sensor output remain outside Git; second-source.json records their exact local paths, sizes, hashes and producer revision.

The new test/reference-only scripts/inspect-mef-libraw-reference.cpp is built separately with c++ -std=c++17 -I/opt/homebrew/include scripts/inspect-mef-libraw-reference.cpp -L/opt/homebrew/lib -lraw -o /absolute/reference-tool. Run it with SOURCE.MEF OUTPUT.u16le. Installed LibRaw0.22.2-Release reports raw/active4016×5344, no margins, black0, maximum4095 and camera multipliers[0,1,0,0]. It only inspects/unpacks, with no demosaicing or color conversion; no production crate/build adds LibRaw. The sensor has42,923,008 bytes/21,461,504 u16 values, SHA-256fcab5343f8f8e2a81b91882711416b9e0d1c33458d2a483eb2bb80751a660621 (second-source-reference.log).

Native Rust file import matches every independent sensor value using streamed reference comparison. Exact42,929,032-byte shared quota succeeds (42,923,008 retained output plus6024 staged row bytes); one byte less refuses and releases all credit after the source-validation pass. Already-cancelled generation refuses before file allocation. The focused source gate passes1/0ignored/0failed (second-source-native.log). Reproduce with RRRAH_MEF_SECOND_SOURCE and RRRAH_MEF_SECOND_SENSOR set to the pinned files and cargo +stable test -p rrrah-raw --lib second_original_all_sensor_values_match_independent_reference_at_exact_budget --locked --offline -- --include-ignored --nocapture --test-threads=1. Full ordinary raw library440 passed/60ignored/0failed (raw-current.log); the new external-input gate is deliberately separate.

Actual Apple M4 Max compute checks all21,461,504 positions with authored signed/HDR channels and unchanged alpha,1 passed/0ignored/0failed (second-source-metal.log). CPU quota peak729,691,136; GPU quota peak1,030,152,288; both final0. Set RRRAH_MEF_SOURCE to this source for the existing mef_sensor_compute target. This is sensor/code transport with mathematical channels, not camera color, demosaicing, physical VRAM or physical HDR qualification; its logged elapsed time is diagnostic, not a paired benchmark.

The metadata auditor now accepts either of two explicit source hashes, keeping its default original pin and structural assertions. For this source the nine MakerNote entries again include seven entirely zero non-text blocks; no standard root/Exif color/WB tags are present (second-source-metadata.json). This is a bounded metadata observation, not proof of absence in every unknown file region. RawViewerGo's pinned raw_jni.cpp selects use_camera_wb=1 and calls LibRaw processing, but that call does not supply an independent positive camera multiplier reference. The current public image route still classifies this original as sensor and refuses camera WB/color before managed allocation,1 passed/0ignored/0failed, peak/final0 (second-source-public.log).

MEF remains Pending for ordinary viewing. There is now a second independent source for full sensor/native/compute correctness, but still no qualified as-shot WB/color reference. No format count was increased and no preview/assumed color was substituted.
