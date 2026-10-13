#!/usr/bin/env python3
"""Audit late exact failures with surviving members and an independent group."""
import hashlib
import json
import re
import shutil
from pathlib import Path
R = Path(__file__).resolve().parents[1]
T = Path('/Users/themoretheless/.codex/tmp')
D = R / 'docs/research'
S = T / 'rrrah-exact-healthy-publication-20261013'
sha = lambda p: hashlib.sha256(Path(p).read_bytes()).hexdigest()
prior_path = D / 'dedup-exact-final-publication-audit-20261013.json'
prior = json.loads(prior_path.read_text())
changed = 'crates/rrrah-dedup/tests/exact_files.rs'
pins = dict(prior['source_sha256'])
assert all(sha(S/p) == h for p, h in pins.items() if p != changed)
pins[changed] = sha(S/changed)
assert all(sha(R/p) == pins[p] for p in [changed, 'crates/rrrah-dedup/src/exact.rs'])
log = T / 'rrrah-exact-healthy-publication-frozen-20261013.log'
text = log.read_text()
assert re.findall(r'test result: ok\. (\d+) passed; (\d+) failed; (\d+) ignored;', text) == [('28','0','0')]
assert 'late_exact_source_failures_preserve_healthy_members_and_other_groups ... ok' in text
binary = Path(re.search(r'Running tests/exact_files.rs \(([^)]+)\)', text).group(1))
dest = T / 'rrrah-exact-healthy-publication-qualified-20261013'
assert not dest.exists()
dest.mkdir()
output = dest / 'exact_files'
shutil.copy2(binary, output)
assert sha(binary) == sha(output)
result = dict(status='verified28_exact_tests', passed=28, failed=0, ignored=0,
    source=str(S), source_files=len(pins), source_sha256=pins,
    executable=str(output), executable_sha256=sha(output), log=str(log), log_sha256=sha(log),
    prior_audit=str(prior_path), prior_audit_sha256=sha(prior_path),
    controls=dict(modes=['scan','scan_verified'], orders=['forward','reverse'],
                  final_callback_failures=['removed','same_size_modified_mtime_restored'],
                  combinations=8, files_per_combination=5, healthy_groups_after_failure=2),
    scope='One new test executes8 predefined late-source failure combinations with baseline/retry; full28 exact_files pass on one3773-file snapshot. No native/decode/fullformat/platform/scale claim; test-only extension, production unchanged from exact publication correction.')
path = D / 'dedup-exact-healthy-publication-audit-20261013.json'
assert not path.exists()
path.write_text(json.dumps(result, indent=2)+'\n')
print(result['status'], result['controls']['combinations'])
