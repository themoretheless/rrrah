#!/usr/bin/env python3
"""Audit finite-statistics gate shared by direct/indexed comparison and cache."""
import hashlib
import json
import re
import shutil
from pathlib import Path
R=Path(__file__).resolve().parents[1]
T=Path('/Users/themoretheless/.codex/tmp')
D=R/'docs/research'
S=T/'rrrah-fingerprint-nonfinite-20261013'
sha=lambda p:hashlib.sha256(Path(p).read_bytes()).hexdigest()
prior_path=D/'dedup-exact-cancel-flag-audit-20261013.json'
prior=json.loads(prior_path.read_text())
affected=['crates/rrrah-dedup/src/pixels.rs','crates/rrrah-dedup/src/cache.rs','crates/rrrah-dedup/tests/visual.rs']
pins=dict(prior['source_sha256'])
assert all(sha(S/p)==h for p,h in pins.items() if p not in affected)
for p in affected:
    pins[p]=sha(S/p)
    assert sha(R/p)==pins[p]
logs=[T/'rrrah-fingerprint-nonfinite-20261013.log',T/'rrrah-fingerprint-nonfinite-information-20261013.log']
expected=[[('40','0','2'),('7','0','0'),('9','0','0')],[('5','0','0')]]
all_binaries=[]
for log,summaries in zip(logs,expected):
    text=log.read_text()
    assert re.findall(r'test result: ok\. (\d+) passed; (\d+) failed; (\d+) ignored;',text)==summaries
    all_binaries.extend(re.findall(r'Running (unittests src/lib.rs|tests/\w+.rs) \(([^)]+)\)',text))
assert len(all_binaries)==4
assert 'nonfinite_fingerprint_statistics_never_claim_informative_evidence ... ok' in logs[0].read_text()
baseline=T/'rrrah-fingerprint-nonfinite-baseline-20261013.log'
assert 'nonfinite_fingerprint_statistics_never_claim_informative_evidence ... FAILED' in baseline.read_text()
dest=T/'rrrah-fingerprint-nonfinite-qualified-20261013'
assert not dest.exists();dest.mkdir()
saved={}
for label,binary in all_binaries:
    name='library_tests' if label.startswith('unittests') else Path(label).stem
    output=dest/name;shutil.copy2(binary,output);assert sha(binary)==sha(output);saved[str(output)]=sha(output)
result=dict(status='verified_finite_statistics_comparison_and_cache',passed=61,failed=0,ignored=2,
    counts=dict(default_library=40,cache=7,visual=9,decode_information=5),
    source=str(S),source_files=len(pins),source_sha256=pins,
    affected_root_sha256={p:sha(R/p) for p in affected},executables=saved,
    log_sha256={str(p):sha(p) for p in logs},baseline_log=str(baseline),baseline_log_sha256=sha(baseline),
    baseline_pixels_source=str(T/'rrrah-fingerprint-nonfinite-baseline-pixels-20261013.rs'),
    prior_audit=str(prior_path),prior_audit_sha256=sha(prior_path),
    controls='NaN/+Inf/-Inf in all3 mean channels and stddev, both orientations and transform policies; zero-distance indexed candidates retain informative=false; healthy finite control. Existing real-image hash collision, uniform/transparent and cache rejection controls pass.',
    scope='One new compound visual test plus full named focused suites on one3773-file snapshot. Baseline aborts on first NaN-mean assertion; no claim every malformed variant separately failed old code. Caller-supplied statistics still cannot prove image provenance. No general precision/all-format/full integration claim.')
path=D/'dedup-fingerprint-nonfinite-audit-20261013.json';assert not path.exists();path.write_text(json.dumps(result,indent=2)+'\n');print(result['status'],result['passed'],result['ignored'])
