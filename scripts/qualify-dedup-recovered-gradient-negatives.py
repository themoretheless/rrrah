#!/usr/bin/env python3
"""Sequentially qualify the other 25 recovered queries on one pinned executable."""
import argparse
import hashlib
import json
from pathlib import Path
import subprocess
import sys

p = argparse.ArgumentParser()
for key in ('manifest', 'probe', 'recovery_audit', 'output_directory'):
    p.add_argument(key)
a = p.parse_args()
digest = lambda path: hashlib.sha256(Path(path).read_bytes()).hexdigest()
pins = {f: digest(f) for f in (a.manifest, a.probe, a.recovery_audit)}
recovery = json.loads(Path(a.recovery_audit).read_text())
assert recovery['verified_pairs'] == recovery['required_pairs'] == 58
queries = recovery['recovered_pairs']
assert len(queries) == len(set(queries)) == 26 and '208101.jpg' in queries
# The 208101 query has its own already-running current-executable control.
queries = [q for q in queries if q != '208101.jpg']
root = Path(a.output_directory)
root.mkdir(parents=True, exist_ok=True)
rows = []
state = root / 'state.json'
def save(status):
    state.write_text(json.dumps({'status': status, 'input_hashes': pins,
        'required_queries': queries, 'completed_queries': rows,
        'scope': '25 recovered queries against 156 different-origin originals each; separate 208101 control. No semantic/burst or full-library accuracy claim.'}, indent=2) + '\n')
save('running')
for query in queries:
    assert all(digest(f) == h for f, h in pins.items())
    report = root / (query + '.json')
    audit = root / (query + '.audit.json')
    with (root / (query + '.log')).open('w') as log:
        r = subprocess.run([sys.executable, 'scripts/qualify-dedup-gradient-region-negatives.py',
            a.manifest, a.probe, query, str(report)], stdout=log, stderr=log)
    if r.returncode:
        save('failed')
        raise SystemExit(r.returncode)
    r = subprocess.run([sys.executable, 'scripts/verify-dedup-gradient-region-negatives.py',
        a.manifest, a.probe, str(report), query, str(audit)], capture_output=True, text=True)
    if r.returncode:
        save('audit_failed')
        raise RuntimeError(r.stderr)
    result = json.loads(audit.read_text())
    assert result['verified_pairs'] == result['required_pairs'] == 156
    rows.append({'query': query, 'audit_sha256': digest(audit),
        'whole_positive_pairs': result['whole_positive_pairs'],
        'region_supported_pairs': result['region_supported_pairs']})
    save('running')
    print(json.dumps(rows[-1]), flush=True)
assert all(digest(f) == h for f, h in pins.items())
save('complete')
