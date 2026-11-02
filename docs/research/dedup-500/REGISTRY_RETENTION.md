# Registry report retention

Current retained inventory: [registry-coverage-audit-current.json](registry-coverage-audit-current.json).

Repeated full registry snapshots were removed. `registry-snapshot-retention.json` records their original paths, SHA-256, sizes and line counts. Historical versions remain in Git history. Historical snapshot references describe their original run; the retained report is the current inventory, not a replacement for historical bytes.

Incremental source-review reports are retained: their `new_whole_files_read`, controls, findings and `previous_audit` chains contain unique evidence. Original test logs, failures, fixtures, source pins and benchmark controls are retained.

Regenerate an inventory with `python3 scripts/audit-dedup-500-registry-coverage.py /tmp/rrrah-registry-audit.json`. The audit uses the retained inventory as its seed and verifies current evidence independently. Do not add numbered full snapshots: retain the current inventory and unique test evidence.
