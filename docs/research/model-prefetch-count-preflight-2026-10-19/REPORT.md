# Avoid unretainable cold model prefetch

The worker's retained-model prefetch previously parsed a cold OBJ/STL/PLY/OFF source even if every RAM count slot was pinned or leased. Insertion then failed and the background result provided no retained neighbour. A mode-specific count preflight now runs before cold native decode, after normal cache lookups and cancellation checks. Foreground loading and the existing non-retaining helper retain their prior behavior. Source identity resolution still performs filesystem/fingerprint work; this is not a no-I/O admission promise.

The regression covers triangle.stl, off-attributes-binary.off, obj-shared-metadata.obj and ply-64-le.ply. With two count slots, a selected visible model and leased nearest neighbour occupy both. Far retained prefetch returns the explicit protected-count refusal, does not increase managed peak geometry usage, and a decoder panic guard proves cold decode is bypassed. A resident nearest hit still succeeds with a lease. Releasing that lease allows far cold decode/admission while retaining the selected pin; final managed root is zero. The temporary copies establish distinct path keys, not independent large models.

The focused regression passes; the ordinary application suite passes289 tests,82 ignored, including model swap/restoration, cancellation and prefetch priority tests. Formatting and whitespace checks pass. No throughput benchmark, full model-format qualification, GPU correctness, physical slow-disk or live GUI endurance claim is made. Full-count speculative swap restoration is not optimized by this change: cache lookup behavior intentionally remains before the cold-decode preflight.


Follow-up: retained worker prefetch now also avoids unretainable swap reads; the earlier cold-only limitation is superseded by [specific admission policy](../prefetch-swap-admission-2026-10-19/REPORT.md).
