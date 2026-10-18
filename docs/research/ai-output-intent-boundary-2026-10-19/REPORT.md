# Rule out missing OutputIntent as the pinned AI mismatch cause

The current renderer uses a fixed default DeviceCMYK policy. Before changing that policy, inspect whether the actual discrepant sources declare an OutputIntent. Both pinned original AI files use plain Catalog object 1 0, contain no object streams, and have no OutputIntents entry in that Catalog. The bounded inspection tool checks exact source bytes/SHA256, rejects escaped catalog names or unexpected layouts, and records catalog hashes. It is deliberately not a general PDF parser. Artwork remains external to the checkout.

Result: neither VectorApple.ai nor one.ai declares an OutputIntent. Therefore ignoring such a declaration cannot explain these particular sources' measured differences. This does not prove general OutputIntent support or correct default DeviceCMYK policy.

After the missing-resource and unresolved-font diagnostic changes, a fresh locked/offline native dump build and strict Poppler comparison still produce:
- VectorApple.ai page0: 33167 differing pixels, max channel error214.
- one.ai page0: 2522 differing pixels, max channel error58.
- one.ai page1: exact.

All three pages decoded with expected dimensions/selection metadata; no font diagnostic or rendering error replaced a page. Comparison exits1 because two pages differ, not because execution failed. No threshold was changed. Exact source identities, native binary hash, Poppler version and full metrics are in current-poppler.json.

This rejects an incorrect causal hypothesis and confirms that stricter font diagnostics did not fix the existing appearance mismatch. Historical same-profile CMYK and mesh-support diagnostics must remain separate: they do not establish current whole-page source geometry or color correctness. Next appearance work should isolate mesh coverage/integration from color policy rather than add an OutputIntent rule to files that have no such declaration.

Reproduce:
```
python3 docs/research/ai-output-intent-boundary-2026-10-19/inspect.py /tmp/rrrah-ai-current-corpus-20261016 /tmp/ai-catalogs.json
python3 scripts/qualify-ai-upstream.py /tmp/rrrah-ai-current-corpus-20261016 --dump /tmp/rrrah-alpha-isolated-target-20261016/debug/examples/raster_fixture_dump --report /tmp/rrrah-ai-profile-policy-current.json
```

No production renderer code changed in this investigation. AI remains pending, including PostScript support, independent appearance qualification and full-page GPU verification.
