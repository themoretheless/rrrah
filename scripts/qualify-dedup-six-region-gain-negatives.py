#!/usr/bin/env python3
"""Run and independently audit both spatial recovery controls through the six-lane collection."""
import argparse
import hashlib
import json
import subprocess
from pathlib import Path

p = argparse.ArgumentParser()
for name in ('manifest', 'probe', 'output_directory'):
    p.add_argument(name, type=Path)
a = p.parse_args()
queries = ['201301.jpg', '204601.jpg']
digest = lambda path: hashlib.sha256(Path(path).read_bytes()).hexdigest()
pins = {str(path): digest(path) for path in (a.manifest, a.probe)}
a.output_directory.mkdir(parents=True, exist_ok=True)
state = {'status': 'running', 'required_queries': queries, 'completed_queries': [],
         'input_sha256': pins,
         'scope': 'Two fixed spatial queries through six-region collection against 156 other publisher origins each; finite controls only.'}
state_path = a.output_directory / 'state.json'

def save():
    state_path.write_text(json.dumps(state, indent=2) + '\n')

save()
for query in queries:
    assert all(digest(path) == value for path, value in pins.items())
    report = a.output_directory / (query + '.json')
    audit = a.output_directory / (query + '.audit.json')
    for stage, command in (
        ('qualifier', ['python3', 'scripts/qualify-dedup-gradient-region-negatives.py',
                       str(a.manifest), str(a.probe), query, str(report), '--six-regions']),
        ('audit', ['python3', 'scripts/verify-dedup-gradient-region-negatives.py',
                   str(a.manifest), str(a.probe), str(report), query, str(audit), '--six-regions'])):
        with (a.output_directory / (query + '.' + stage + '.log')).open('w') as log:
            run = subprocess.run(command, stdout=log, stderr=log)
        if run.returncode:
            state.update(status=stage + '_failed', failed_query=query, exit_code=run.returncode)
            save()
            raise SystemExit(run.returncode)
    result = json.loads(audit.read_text())
    assert result['verified_pairs'] == 156
    state['completed_queries'].append({'query': query, 'audit_sha256': digest(audit),
        'verified_pairs': result['verified_pairs'],
        'whole_positive_pairs': result['whole_positive_pairs'],
        'region_supported_pairs': result['region_supported_pairs']})
    save()
assert all(digest(path) == value for path, value in pins.items())
state['status'] = 'verified'
save()
