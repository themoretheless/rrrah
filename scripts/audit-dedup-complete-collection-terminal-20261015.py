#!/usr/bin/env python3
"""Account for terminal inventories without converting failed gates into success."""
from pathlib import Path
import hashlib
import json
import re
import subprocess


def account(section, names):
    rows = re.findall(r'^test (.+) \.\.\. (ok|FAILED|ignored[^\n]*)$', section, re.M)
    assert len(rows) == len(dict(rows)), 'duplicate executed test'
    results = dict(rows)
    assert set(results) == set(names), 'execution/inventory mismatch'
    summary = re.search(
        r'test result: (ok|FAILED)\. (\d+) passed; (\d+) failed; '
        r'(\d+) ignored; (\d+) measured; (\d+) filtered out;', section)
    assert summary, 'missing terminal summary'
    counts = [sum(v == 'ok' for v in results.values()),
              sum(v == 'FAILED' for v in results.values()),
              sum(v.startswith('ignored') for v in results.values())]
    assert counts == [int(summary[i]) for i in (2, 3, 4)]
    assert summary[5] == summary[6] == '0'
    assert (summary[1] == 'FAILED') == bool(counts[1])
    return dict(passed=counts[0], failed=counts[1], ignored=counts[2],
                failures=sorted(k for k, v in results.items() if v == 'FAILED'),
                complete_inventory=sorted(names))


def main():
    root = Path(__file__).resolve().parents[1]
    report = root / 'docs/research/dedup-complete-collection-baseline-v2-20261015.json'
    output = root / 'docs/research/dedup-complete-collection-terminal-audit-20261015.json'
    sha = lambda p: hashlib.sha256(Path(p).read_bytes()).hexdigest()
    data = json.loads(report.read_text())
    assert data['status'] in ('terminal_failure', 'terminal_pending_inventory_audit')
    assert 'retained_binaries' in data, 'producer has not finished retaining inventories'
    assert not output.exists()
    assert sha(root / 'scripts/qualify-dedup-complete-collection-baseline-v2-20261015.py') == data['producer_sha256']
    assert sha(data['source_manifest']) == data['source_manifest_sha256']
    for row in json.loads(Path(data['source_manifest']).read_text())['source_files']:
        assert sha(Path(data['source']) / row['path']) == row['sha256']
    assert sha(data['cargo_log']) == data['cargo_log_sha256']
    body = Path(data['cargo_log']).read_text()
    headers = list(re.finditer(r'^\s*Running .+ \((/[^\n]+)\)$', body, re.M))
    retained = {r['original']: r for r in data['retained_binaries']}
    assert headers and len(headers) == len(retained)
    assert set(retained) == {h[1] for h in headers}
    targets = []
    for i, header in enumerate(headers):
        row = retained[header[1]]
        assert sha(row['copy']) == row['sha256']
        assert sha(row['inventory']) == row['inventory_sha256']
        assert row['inventory_returncode'] == 0
        inventory = Path(row['inventory']).read_text()
        names = re.findall(r'^(.+): test$', inventory, re.M)
        assert len(names) == len(set(names))
        assert not re.findall(r'^(.+): benchmark$', inventory, re.M)
        fresh = subprocess.run([row['copy'], '--list'], capture_output=True, text=True)
        assert fresh.returncode == 0
        assert set(re.findall(r'^(.+): test$', fresh.stdout, re.M)) == set(names)
        section = body[header.end():headers[i + 1].start() if i + 1 < len(headers) else len(body)]
        targets.append(dict(binary=row['copy'], sha256=row['sha256'], **account(section, names)))
    failed = sum(t['failed'] for t in targets)
    assert (data['returncode'] != 0) == bool(failed)
    result = dict(status='verified_failed_terminal_inventory' if failed else 'verified_passing_terminal_inventory',
                  gate_passed=not failed, producer_report=str(report), producer_report_sha256=sha(report),
                  auditor_sha256=sha(__file__), targets=targets,
                  passed=sum(t['passed'] for t in targets), failed=failed,
                  ignored=sum(t['ignored'] for t in targets),
                  scope=data['scope'] + ' Inventory accounting does not repair failures or qualify current Root.')
    output.write_text(json.dumps(result, indent=2) + '\n')
    print(result['status'], result['passed'], failed, result['ignored'])


if __name__ == '__main__':
    main()
