#!/usr/bin/env python3
"""Independently confirm every pair of the declared collection."""
import argparse
import hashlib
import itertools
import json
import subprocess
from pathlib import Path

p = argparse.ArgumentParser()
for name in ('collection', 'probe', 'output'):
    p.add_argument(name, type=Path)
p.add_argument("--reuse-oracle",type=Path)
a = p.parse_args()
digest = lambda path: hashlib.sha256(Path(path).read_bytes()).hexdigest()
collection = json.loads(a.collection.read_text())
paths = collection['paths']
assert len(paths) >= 2 and len(set(paths)) == len(paths)
n=len(paths); pair_count=n*(n-1)//2
pins = {str(a.probe): digest(a.probe), **{path: digest(path) for path in paths}}
assert all(pins[path] == collection['input_hashes'][path] for path in paths)
reused = {}
if a.reuse_oracle:
    seed = json.loads(a.reuse_oracle.read_text())
    assert seed['status']=='complete' and 2 <= len(seed['paths']) <= n
    assert len(set(seed['paths'])) == len(seed['paths']) and seed['paths']==paths[:len(seed['paths'])]
    assert all(path in seed['input_hashes'] for path in seed['paths'])
    assert seed['input_hashes'][str(a.probe)]==pins[str(a.probe)]
    assert all(digest(path)==value for path,value in seed['input_hashes'].items())
    count=len(seed['paths']); expected=set(itertools.combinations(range(1,count+1),2))
    assert all(type(r['left']) is int and type(r['right']) is int
               and type(r['returncode']) is int for r in seed['results'])
    reused={(r['left'],r['right']):r for r in seed['results']}
    assert len(reused)==len(seed['results'])==len(expected) and set(reused)==expected
    assert all(r['returncode']==0 and
               json.dumps(json.loads(r['stdout']), sort_keys=True, allow_nan=False)==
               json.dumps(r['evidence'], sort_keys=True, allow_nan=False)
               for r in reused.values())
    pins[str(a.reuse_oracle)]=digest(a.reuse_oracle)
result = {'status': 'running', 'paths': paths, 'input_hashes': pins, 'required_pairs': pair_count,
          'results': [], 'reused_pairs': len(reused), 'scope': f'All {pair_count} independent two-source confirmations (explicitly pinned reused observations allowed) for the fixed {n}-source input; no full corpus or broad precision.'}

def save():
    a.output.write_text(json.dumps(result, indent=2) + '\n')

save()
for left, right in itertools.combinations(range(n), 2):
    if (left+1,right+1) in reused:
        result['results'].append({**reused[(left+1,right+1)],'reused_from':str(a.reuse_oracle)})
        assert all(digest(path)==value for path,value in pins.items())
        save()
        continue
    run = subprocess.run([str(a.probe), '--six-regions-collection-pair', paths[left], paths[right]], capture_output=True, text=True)
    row = {'left': left + 1, 'right': right + 1, 'returncode': run.returncode,
           'stdout': run.stdout, 'stderr': run.stderr}
    result['results'].append(row)
    if run.returncode:
        result['status'] = 'native_failed'
        save()
        raise SystemExit(run.returncode)
    row['evidence'] = json.loads(run.stdout)
    assert row['evidence']['status'] == 'ok'
    assert all(digest(path) == value for path, value in pins.items())
    save()
result['status'] = 'complete'
save()
