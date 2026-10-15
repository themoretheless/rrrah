#!/usr/bin/env python3
import hashlib
import json
from pathlib import Path
import re
import statistics

ROOT = Path(__file__).resolve().parents[1]
DIRECTORY = ROOT / 'docs/research'
PRODUCER = DIRECTORY / 'dedup-ownerless-actual-engine-benchmark-20261015.json'
report = json.loads(PRODUCER.read_text())
assert report['status'] == 'completed_pending_independent_audit'
sha = lambda path: hashlib.sha256(Path(path).read_bytes()).hexdigest()
assert sha(ROOT / 'scripts/benchmark-dedup-ownerless-actual-engine-20261015.py') == report['producer_sha256']
assert sha(DIRECTORY / 'dedup-ownerless-actual-engine-audit-20261015.json') == report['correctness_audit_sha256']
assert sha(report['executable']) == report['executable_sha256']
rows = report['rows']
assert len(rows) == 378
keys = {(row['case'], row['owners'], row['round'], row['variant']) for row in rows}
expected = {(case[0], owners, round_index, variant) for case in report['cases'] for owners in ['none', 'all', 'mixed'] for round_index in range(7) for variant in ['baseline', 'candidate']}
assert len(keys) == len(rows) and keys == expected
for row in rows:
    assert sha(row['log']) == row['log_sha256']
    text = Path(row['log']).read_text()
    found = re.search(r'DEDUP_ENGINE_NS=(\d+) ACCEPTED=(\d+) VISUAL_CALLS=(\d+) CHECKPOINTS=(\d+)', text)
    assert found and list(map(int, found.groups())) == [row[key] for key in ['nanoseconds', 'accepted', 'visual_calls', 'checkpoints']]
    assert row['nanoseconds'] > 0 and row['cpu_seconds'] > 0
    assert '1 passed; 0 failed; 0 ignored; 0 measured;' in text
    _, count, size, iterations, mode = next(case for case in report['cases'] if case[0] == row['case'])
    pairs = count * (count - 1) // 2
    matches = pairs if mode == 'equal' else sum(mode == 'mixed' and i % 3 == j % 3 for i in range(count) for j in range(i + 1, count))
    assert row['accepted'] == iterations * pairs and row['visual_calls'] == iterations * (pairs - matches)
for result in report['results']:
    wall = []
    cpu = []
    for round_index in range(7):
        ordered = [row for row in rows if row['case'] == result['case'] and row['owners'] == result['owners'] and row['round'] == round_index]
        assert [row['variant'] for row in ordered] == (['baseline', 'candidate'] if round_index % 2 == 0 else ['candidate', 'baseline'])
        pair = {row['variant']: row for row in ordered}
        assert pair['baseline']['checkpoints'] == pair['candidate']['checkpoints']
        wall.append(pair['candidate']['nanoseconds'] / pair['baseline']['nanoseconds'])
        cpu.append(pair['candidate']['cpu_seconds'] / pair['baseline']['cpu_seconds'])
    assert wall == result['paired_wall_ratios'] and cpu == result['paired_cpu_ratios']
    assert statistics.median(wall) == result['median_wall_ratio'] and statistics.median(cpu) == result['median_cpu_ratio']
assert len(report['results']) == 27
OUTPUT = DIRECTORY / 'dedup-ownerless-actual-engine-benchmark-audit-20261015.json'
assert not OUTPUT.exists()
OUTPUT.write_text(json.dumps(dict(status='verified378_actual_engine_processes_not_full_library_qualification', producer_report_sha256=sha(PRODUCER), auditor_sha256=sha(__file__), executable_sha256=sha(report['executable']), results=report['results'], scope=report['scope'], adopted=False), indent=2) + '\n')
print('378logs,27paired result families and checkpoint equivalence verified')
