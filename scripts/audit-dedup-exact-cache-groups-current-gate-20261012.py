#!/usr/bin/env python3
"""Verify terminal exact/cache/group integration execution without broadening scope."""
import hashlib
import json
import re
import sys
from pathlib import Path

R = Path(__file__).resolve().parents[1]
P = R / 'docs/research/dedup-exact-cache-groups-current-gate-20261012.json'

def sha(path):
    return hashlib.sha256(Path(path).read_bytes()).hexdigest()

assert len(sys.argv) == 2 and sys.argv[1] == '0', 'Pass the observed cargo exit code'
r = json.loads(P.read_text())
assert r['status'] == 'running'
assert all(sha(R / p) == h for p, h in r['source_before'].items())
base = Path(r['base_unit_evidence'])
b = json.loads(base.read_text())
assert b['status'] == 'verified_current_base_units_passed'
assert all(sha(p) == h for p, h in b['pins'].items())
log = Path(r['log'])
text = log.read_text()
blocks = re.split(r'Running tests/(cache|exact_files|groups)\.rs \(([^)]+)\)', text)
assert len(blocks) == 10
rows = []
executables = []
for i in range(1, len(blocks), 3):
    name, executable, output = blocks[i:i+3]
    summary = re.findall(r'test result: ok\. (\d+) passed; (\d+) failed; (\d+) ignored; (\d+) measured; (\d+) filtered out;', output)
    assert len(summary) == 1
    passed, failed, ignored, measured, filtered = map(int, summary[0])
    assert passed > 0 and failed == ignored == measured == filtered == 0
    names = re.findall(r'^test ([^\n]+) \.\.\. ok$', output, re.M)
    assert len(names) == len(set(names)) == passed
    assert re.search(rf'^running {passed} tests$', output, re.M)
    rows.append(dict(target=name, passed=passed, failed=failed, ignored=ignored, names=names))
    executables.append(Path(executable))
assert {x['target'] for x in rows} == {'cache', 'exact_files', 'groups'}
r.update(status='verified_current_exact_cache_groups_passed', returncode=0, targets=rows,
         passed=sum(x['passed'] for x in rows), source_after=dict(r['source_before']),
         pins={str(p): sha(p) for p in [base, log, *executables, Path(__file__)]},
         qualification_limits=['macOS only; two non-UTF8 physical-path tests compile only on other Unix platforms',
                               'cache_publication_child is a subprocess helper; its no-environment parent invocation is not a standalone persistence scenario',
                               'no decode/Torch/image corpus or other integration targets tested by this command'])
P.write_text(json.dumps(r, indent=2) + '\n')
print('Verified exact/cache/group targets:', [(x['target'], x['passed']) for x in rows])
