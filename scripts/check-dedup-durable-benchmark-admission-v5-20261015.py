"""Require both complete durable correctness audits and currently available evidence before timing."""
from pathlib import Path
import json,hashlib
R=Path(__file__).resolve().parents[1];D=R/'docs/research';h=lambda p:hashlib.sha256(Path(p).read_bytes()).hexdigest();manifest=D/'dedup-durable-source-pair-v5-prepared-20261015.json';pair=json.loads(manifest.read_text())
for variant in ['baseline','candidate']:
 audit=D/f'dedup-durable-{variant}-audit-v5-20261015.json';assert audit.is_file(),f'{variant} complete durable audited gate not ready';a=json.loads(audit.read_text());assert a['gate_passed'] and a['failed']==0 and len(a['targets'])==162 and a['manifest_sha256']==h(manifest);assert h(R/'scripts/audit-dedup-durable-pair-v5-20261015.py')==a['auditor_sha256'];report=D/f'dedup-durable-{variant}-gate-v5-20261015.json';assert h(report)==a['producer_report_sha256'];d=json.loads(report.read_text());assert h(d['cargo_log'])==d['cargo_log_sha256']
 for b in d['retained_binaries']:assert h(b['copy'])==b['sha256'] and h(b['inventory'])==b['inventory_sha256']
 for row in next(v for v in pair['variants'] if v['variant']==variant)['source_files']:assert h(Path(next(v for v in pair['variants'] if v['variant']==variant)['source'])/row['path'])==row['sha256']
assert set(pair['only_candidate_differences'])=={'crates/rrrah-dedup/src/exact.rs','crates/rrrah-dedup/src/collection.rs'};print('Full durable paired correctness evidence available; opt3 samebinarybenchmark source/compiler/protocol and quiescentownedheavywork still required. No timing launched.')
