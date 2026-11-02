# Research report retention

Keep one current cumulative inventory and unique evidence: original test logs, counterexamples, before/after benchmark controls, source pins, independent oracles and fixtures. A newer successful run does not supersede a distinct failure, hardware configuration or long-running test.

Repeated registry inventories and their repeated generators are removed; their paths, original SHA-256, sizes and line counts are recorded in [registry-snapshot-retention.json](dedup-500/registry-snapshot-retention.json). The retained [current inventory](dedup-500/registry-coverage-audit-current.json) describes current evidence; historical bytes remain in Git history. See [registry policy](dedup-500/REGISTRY_RETENTION.md).

Byte-identical JSON reports share one physical copy through relative symbolic links. [report-duplicate-retention.json](report-duplicate-retention.json) records aliases and original hashes, preserving existing evidence paths. Pinned upstream source files, dependency metadata and benchmark runtime files are not report duplicates and remain intact.

For subsequent checks, update the corresponding current report/log. Keep a separate artifact only when it contributes unique evidence; do not add numbered full inventory snapshots.
