# Rejected sampler fastpath

Identity-transform coefficient reuse and clip-bound hoisting passed 135 interpreter tests and preserved all three fixture pixel hashes. Six paired runs showed only noisy 1–2% differences on complex cases, with some regressions. Candidate reverted exactly to the previous streaming encoder; candidate.patch and raw measurements are retained for traceability. This is not a shipped optimization. See ../tensor-stream-planning-2026-10-19/REPORT.md for the subsequent accepted planning change.
