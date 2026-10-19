# Paired mixed-collection prefetch admission measurement

The post-change single rerun retained all correctness assertions but took10.58s versus the earlier9.29s snapshot. That was insufficient to establish either a speedup or regression. A test-only RRRAH_BENCH_LEGACY_PREFETCH_SWAP=1 control now disables only the raster worker's early protected-count swap bypass. It is absent from production builds; the model path and all other code are identical. Reports record which policy ran.

One binary ran six sequential full-collection tests in legacy/admission/admission/legacy/legacy/admission order, three repetitions per policy. No build or second benchmark from this task overlapped measurement; unrelated host activity was uncontrolled. The already-selected1024-source manifest was revalidated by SHA256 (zero mismatches). Warm local filesystem, separate in-process cache lifetimes per run. Reported time includes baseline decode/preparation plus962 navigation steps, prefetch, synthetic managed-credit pressure and TTL/swap checks; it excludes compilation/process startup and is not GUI frame latency.

Legacy times:8.9422,8.2350,8.2148s, median8.2350s. Admission times:6.9780,7.0283,7.3373s, median7.0283s:14.65% lower in this bounded test. Median swap reads fall from1327 to628 (52.68%). Keep the admission optimization on this evidence; do not generalize this percentage to all formats, disks or hardware. The possible LRU-touch effect noted during investigation was not isolated as a cause; no new LRU policy change was made.

All runs match source/status/prepared pixel+metadata digests exactly, retain962 accepted navigation sources,720 nearest-prefetch foreground hits and61 TTL restores. Each ends with managed usage zero and swap errors zero. Bounded queue drops vary2..7 and writes960..962; they are reported, not called lossless persistence. Existing assertions protect required nearest/TTL results. This is the mixed repository/fixture corpus, not1024 independent photographs or independent color qualification. Managed peak includes artificial reservation-only pressure, not physical RAM exhaustion.

Raw per-run reports/logs, binary/source/manifest hashes, runner and summary are retained. The earlier incomparable single-run slowdown does not override the paired measurement, and the paired measurement does not qualify physical slow disks, live GUI, HDR or NVIDIA.

Final verification with the benchmark control unset:291 ordinary app tests pass,82 ignored. Formatting and tracked whitespace checks pass.
