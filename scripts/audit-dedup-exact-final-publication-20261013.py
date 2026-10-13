#!/usr/bin/env python3
"""Pin exact publication correction and distinguish manual timing omissions."""
import hashlib
import json
import re
import shutil
from pathlib import Path

R = Path(__file__).resolve().parents[1]
T = Path('/Users/themoretheless/.codex/tmp')
S = T / 'rrrah-exact-final-publication-20261013'
D = R / 'docs/research'


def sha(p):
    return hashlib.sha256(Path(p).read_bytes()).hexdigest()


m = json.loads((S / 'validation-snapshot.json').read_text())
assert all(sha(S / p) == h for p, h in m['files'].items())
affected = ['crates/rrrah-dedup/src/exact.rs', 'crates/rrrah-dedup/tests/exact_files.rs']
assert all(sha(R / p) == m['files'][p] for p in affected)
logs = [T / 'rrrah-exact-final-publication-20261013.log', T / 'rrrah-exact-final-publication-lib-20261013.log']
dest = T / 'rrrah-exact-final-publication-qualified-20261013'
dest.mkdir(exist_ok=True)
saved = {}
for log, expected, name in zip(logs, [(27, 0, 0), (39, 0, 2)], ['exact_files', 'library_tests']):
    text = log.read_text()
    summaries = re.findall(r'test result: ok\. (\d+) passed; (\d+) failed; (\d+) ignored;', text)
    assert [tuple(map(int, v)) for v in summaries] == [expected]
    binary = Path(re.search(r'Running .+? \(([^)]+)\)', text).group(1))
    out = dest / name
    if out.exists():
        assert sha(out) == sha(binary)
    else:
        shutil.copy2(binary, out)
    assert sha(out) == sha(binary)
    saved[str(out)] = sha(out)
baseline = T / 'rrrah-exact-final-callback-baseline-20261013.log'
assert 'stale exact group: Report' in baseline.read_text() and '1 failed' in baseline.read_text()
ignored = re.findall(r'^test (.+) \.\.\. ignored, (.+)$', logs[1].read_text(), re.M)
assert len(ignored) == 2 and all(reason == 'manual alternating exact matcher timing control' for _, reason in ignored)
result = dict(status='verified_exact_final_publication', passed=66, failed=0, ignored=2,
              counts=dict(exact_files=27, default_library=39), ignored_tests=ignored,
              source=str(S), source_files=len(m['files']), source_sha256=m['files'],
              affected_root_sha256={p: sha(R / p) for p in affected}, executables=saved,
              log_sha256={str(p): sha(p) for p in logs},
              baseline_log=str(baseline), baseline_log_sha256=sha(baseline),
              baseline_exact_source=str(T / 'rrrah-exact-final-callback-baseline-source-20261013.rs'),
              scope='Actual stale scan_verified group reproduced. Fixed ordinary/verified late callback controls plus full exact_files/default-feature library suites. No full decode/native/format/platform/scale gate. Final owner-only hashes add full reads; no atomic filesystem snapshot or bounded callback cancellation latency.')
out = D / 'dedup-exact-final-publication-audit-20261013.json'
assert not out.exists()
out.write_text(json.dumps(result, indent=2) + '\n')
print(result['status'], result['counts'], result['ignored'], result['source_files'])
