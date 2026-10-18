# Missing selected PDF font refuses before rendering

Production preflight previously validated a font only if its resource lookup returned a dictionary. Hayro's interpreter substitutes Helvetica when a Tf resource is missing. This can display invented font metrics/appearance for a broken PDF-compatible AI/PDF instead of diagnosing the import.

Preflight now requires the selected font resource and returns InvalidPdf("missing font resource") before final/pixmap render reservations. Existing supported font and inherited-form behavior is retained. The default upstream fallback remains untouched for unrelated Hayro users.

Regression controls mutate only the resource name /F0 to /F1, leaving the Tf reference /F0 unchanged. Both source byte length and all offsets are preserved. Controls cover page contents and an invoked Form inheriting parent resources. The added regression failed before the fix and passes afterward. Both cases assert managed peak equals source length and final accounted memory zero; a subsequent valid inherited-font decode must produce nontransparent pixels and release all credit. Parser-private scratch is not claimed to be managed by these peak assertions.

Validation: rrrah-vector lib 181 passed/10 ignored; rrrah-decode lib 216 passed/5 ignored; zero failures. cargo +stable fmt --all -- --check and git diff --check pass. Locked/offline commands used CARGO_TARGET_DIR=/tmp/rrrah-alpha-isolated-target-20261016.

This fixes malformed-resource behavior. It does not explain or fix the pinned VectorApple interior/color or one.ai edge-coverage differences, does not qualify PostScript AI, and does not establish full PDF/AI correctness. Existing AI pending status remains. No fresh GPU test or whole-workspace gate was run for this pre-render-only change. Standalone vector build emitted existing unused-code warnings from the vendored image feature subset.
