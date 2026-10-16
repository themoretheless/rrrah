# Current pinned AI fidelity and cache/Metal checks

2026-10-16. Fresh locked/offline build of raster_fixture_dump; pinned public artwork fetched outside the checkout, with source byte lengths and SHA256 checked before either renderer. Poppler version 26.10.0.

Strict 72 dpi white-composite comparison exits 1, as required for a mismatch:

| Source/page | Different pixels | Maximum channel error | MAE |
|---|---:|---:|---:|
| VectorApple.ai / 0 | 33167 | 214 | 4.011155227831348 |
| one.ai / 0 | 2522 | 58 | 0.033139260004880426 |
| one.ai / 1 | 0 | 0 | 0 |

The differing-pixel counts match the historical 2026-10-08 check; current VectorApple MAE differs slightly. This comparison locates an unresolved fidelity boundary; it does not by itself prove Poppler is correct at every antialiased edge. No tolerance was relaxed and AI remains pending. PostScript AI is not covered.

Three explicitly ignored tests passed with RRRAH_AI_CORPUS pointing to the verified external corpus and RRRAH_GPU_BACKEND=metal:
- foreground_loader_tests::real_ai_pages_use_shared_ram_hits_and_independent_prefetch_keys
- real_ai_pages_swap_preserves_pixels_and_metal_frame (pdf_readback integration test)
- foreground_loader_tests::real_ai_cached_pages_preserve_metal_sdr_hdr_under_full_pressure

Actual adapter Metal Apple M4 Max. Three independently keyed selected pages preserve native/prepared data and RAM/swap output, with final accounted root memory zero. The cached-page test confirms shared pixel ownership at zero available budget and compares SDR/half-float output at 128x96; swap rendering uses a 96x64 viewport. These are bounded viewport transport comparisons against the native render, not full-page GPU/independent appearance qualification or physical HDR calibration.

Commands:
```
python3 scripts/qualify-ai-upstream.py /tmp/rrrah-ai-current-corpus-20261016 --dump /tmp/rrrah-alpha-isolated-target-20261016/debug/examples/raster_fixture_dump --report /tmp/rrrah-ai-current-poppler-20261016.json --fetch
RRRAH_AI_CORPUS=/tmp/rrrah-ai-current-corpus-20261016 RRRAH_GPU_BACKEND=metal CARGO_TARGET_DIR=/tmp/rrrah-alpha-isolated-target-20261016 cargo +stable test -p rrrah --bin rrrah --test pdf_readback --locked --offline real_ai_pages -- --ignored --nocapture
RRRAH_AI_CORPUS=/tmp/rrrah-ai-current-corpus-20261016 RRRAH_GPU_BACKEND=metal CARGO_TARGET_DIR=/tmp/rrrah-alpha-isolated-target-20261016 cargo +stable test -p rrrah --bin rrrah --locked --offline foreground_loader_tests::real_ai_cached_pages_preserve_metal_sdr_hdr_under_full_pressure -- --ignored --exact --nocapture
```

Exact source manifest, metrics and decoder binary hash are in poppler.json. Build/test logs and selected source hashes are adjacent. Next fidelity work must diagnose VectorApple's substantial interior/color differences independently from one.ai cubic boundary coverage, rather than treating successful transport as a rendering fix.
