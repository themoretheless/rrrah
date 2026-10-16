# Swap strict lint completion

All rrrah-swap targets pass strict Clippy (`-D warnings`). Full crate tests, integration and doctests: **24 passed / 1 ignored / zero failures**.

Segmented block offsets/lengths use checked conversions. Test data explicitly selects its low byte or uses checked normalized conversion, preserving previous byte patterns. Constant scanline chunks use fixed arrays. Large expected constant data is checked by exact length plus every byte rather than constructing a large stack copy. Two long tests delegate their later rejection assertions to helpers, preserving original scenario order and shared store/handle/budget state.

The two64KiB stream buffers remain fixed stack storage. Each construction has a narrow `expect(clippy::large_stack_arrays)` with an explicit memory-admission reason: moving to heap would require managed admission. This expectation is checked by the compiler; it is not a crate-wide lint allowance. Stream block size, physical I/O, wire digests, disk quotas and managed memory behavior are unchanged by this follow-up. No unmeasured speedup is claimed.

This supersedes the rrrah-swap lint failure reported in `../texture-swap-clippy-2026-10-16/REPORT.md`. Workspace strict Clippy is still unproven and previously failed in dedup; no unrelated dedup changes were made. The full workspace immutable-input report remains valid only for its recorded earlier hashes; these localized follow-ups have this separate test/lint gate.
