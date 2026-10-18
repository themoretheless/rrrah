# Missing-font PDF recovery through the viewer cache

Regression uses actual foreground and prefetch loaders, not a fabricated decode result. A pinned red PDF page is made visible. A temporary malformed PDF keeps the original source length but renames the font resource, leaving its Tf invocation unresolved.

Both foreground decode and background preload must report missing font resource. RAM cache remains exactly one entry and accounted root use returns to its previous value. The visible lease remains valid. While all remaining budget is reserved, the visible frame must hit RAM without decoding, retain the same shared pixel owner and preserve every red pixel. No failure payload enters the cache.

Repairing the same pathname to the original PDF must permit successful preload using the same DecodeGate. Cache becomes exactly two entries; the repaired 100x20 frame subsequently hits RAM without decode and shares pixel ownership. Dropping all frame/cache owners returns accounted root use to zero. This covers failure cleanup, prefetch permit release and retry through current source identity. It does not force same-mtime identity or exercise disk swap.

Focused regression passed. Full app ordinary test suite: 287 passed, zero failed, 81 ignored. Workspace formatting and diff whitespace checks pass. Prior vector/public decoder gate is documented in ../pdf-missing-font-refusal-2026-10-19/REPORT.md. This adds a viewer integration regression; no additional production change, GPU qualification or AI appearance fix is claimed.

Command: CARGO_TARGET_DIR=/tmp/rrrah-alpha-isolated-target-20261016 cargo +stable test -p rrrah --bin rrrah --locked --offline. Logs and selected source hashes are adjacent.
