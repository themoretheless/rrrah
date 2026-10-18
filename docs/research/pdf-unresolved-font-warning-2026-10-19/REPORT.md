# Reject unresolved selected fonts instead of accepting Helvetica substitution

The PDF resource preflight accepts supported font subtypes; it cannot establish whether the font actually loads. Hayro Context::resolve_font caches Font::new failures as None and substitutes Helvetica. The existing UnsupportedFont warning was never emitted anywhere. A strict importer therefore accepted the fallback as a successful native rendering.

The unresolved-font branch now emits InterpreterWarning::UnsupportedFont before preserving Hayro's existing fallback. The rrrah PDF warning sink already treats this flag as InvalidPdf("unsupported font") and drops the rendered output. Other clients that ignore warnings keep their previous permissive behavior. This is a post-render diagnostic, unlike the separate missing-resource preflight fix; no early allocation-refusal claim is made here.

Regression controls declare supported Type0 fonts without the required Encoding. One source is a page font; the other is used inside an invoked Form inheriting parent resources. Same-length substitutions preserve stream lengths and object offsets. The regression fails before the warning change and passes after it. Both failures release all accounted memory and allow a subsequent valid inherited Helvetica page to render nontransparent pixels and release its owners.

Validation: vector lib 182 passed/10 ignored; app bin 287 passed/81 ignored; public decoder lib 216 passed/5 ignored. Total ordinary tests 685 passed/96 ignored/zero failed. Workspace format check and diff whitespace check pass. Locked/offline builds used /tmp/rrrah-alpha-isolated-target-20261016.

Additionally, explicitly ignored foreground_loader_tests::real_ai_cached_pages_preserve_metal_sdr_hdr_under_full_pressure passed on Metal Apple M4 Max, with the pinned external corpus SHA256 verified by the test. All three selected pages retain shared ownership at zero available budget and identical 128x96 SDR/half-float outputs; final accounted root zero. This is native-reference cache/viewport transport, not independent appearance accuracy, full-page GPU qualification or physical HDR calibration.

The previously measured VectorApple/one.ai differences against Poppler and PostScript AI support remain unresolved. No whole-workspace/all-feature/Clippy gate was run for this change. Adjacent logs and selected source hashes identify exact evidence. Vendored image emits pre-existing unused-code warnings in the standalone vector feature subset.
