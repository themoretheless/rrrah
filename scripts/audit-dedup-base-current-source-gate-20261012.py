#!/usr/bin/env python3
"""Finalize current base-unit evidence only after successful cargo completion."""
import hashlib
import json
import re
import sys
from pathlib import Path

R = Path(__file__).resolve().parents[1]
P = R / 'docs/research/dedup-base-current-source-gate-20261012.json'

def sha(path):
    return hashlib.sha256(Path(path).read_bytes()).hexdigest()

assert len(sys.argv) == 2 and sys.argv[1] == '0', 'Pass the observed cargo exit code'
r = json.loads(P.read_text())
assert r['status'] == 'running'
assert all(sha(R / p) == value for p, value in r['source_before'].items())
log = Path(r['log'])
text = log.read_text()
summaries = re.findall(r'test result: ok\. (\d+) passed; (\d+) failed; (\d+) ignored; (\d+) measured; (\d+) filtered out;', text)
assert len(summaries) == 1
passed, failed, ignored, measured, filtered = map(int, summaries[0])
assert passed > 0 and failed == measured == filtered == 0
assert re.search(rf'^running {passed + ignored} tests$', text, re.M)
names = re.findall(r'^test ([^\n]+) \.\.\. ok$', text, re.M)
assert len(names) == len(set(names)) == passed
skipped = re.findall(r'^test ([^\n]+) \.\.\. ignored, ([^\n]+)$', text, re.M)
assert len(skipped) == ignored
assert all(reason == 'manual alternating exact matcher timing control' for _, reason in skipped)
executable = Path(re.search(r'Running unittests [^\n]+ \(([^)]+)\)', text)[1])
r.update(status='verified_current_base_units_passed', returncode=0, passed=passed,
         failed=failed, ignored=ignored, filtered=filtered, test_names=names, ignored_tests=skipped,
         executable=str(executable), source_after=dict(r['source_before']),
         pins={str(p): sha(p) for p in [log, executable, Path(__file__)]})
P.write_text(json.dumps(r, indent=2) + '\n')
print('Verified current base units:', passed)
