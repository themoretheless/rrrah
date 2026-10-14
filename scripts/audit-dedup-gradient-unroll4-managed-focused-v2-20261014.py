#!/usr/bin/env python3
from pathlib import Path
import hashlib
import json
import re
import subprocess

R=Path(__file__).resolve().parents[1]
P=R/'docs/research/dedup-gradient-unroll4-managed-focused-v2-20261014.json'
d=json.loads(P.read_text());sha=lambda p:hashlib.sha256(Path(p).read_bytes()).hexdigest()
assert d['status']=='passed_pending_audit' and d['build_returncode']==d['test_returncode']==0 and d['source_unchanged']
M=Path(d['manifest']);assert sha(M)==d['manifest_sha256']
m=json.loads(M.read_text());S=Path(m['source'])
assert all(sha(S/name)==pin for name,pin in m['source_sha256'].items())
assert sha(R/'scripts/run-dedup-gradient-unroll4-managed-focused-v2-20261014.py')==d['producer_sha256']
assert all(sha(p)==h for p,h in d['dependency_pins'].items()) and all(sha(p)==h for p,h in d['pins'].items())
logs=[Path(p) for p in d['pins'] if p.endswith('/tests.log')];assert len(logs)==1
text=logs[0].read_text();summary=re.search(r'test result: ok\. (\d+) passed; (\d+) failed; (\d+) ignored;.*?(\d+) filtered out;',text);assert summary
passed,failed,ignored,filtered=map(int,summary.groups());assert (passed,failed,ignored,filtered)==(15,0,2,45)
names=[line.split(' ... ')[0].removeprefix('test ') for line in text.splitlines() if line.endswith(' ... ok')]
assert len(names)==15 and len(set(names))==15
for suffix in ['unrolled_distance_blocks_preserve_scalar_bits_and_partial_lanes','linear_runner_large_banks_preserve_results_checkpoints_and_budget_fallback','cached_checkpoints_and_cancellation_match_recomputation_without_credit_leaks','managed_cache_releases_before_output_and_respects_ancestor_pressure','production_mutual_competitors_match_exhaustive_at_every_checkpoint']:
    assert any(name.endswith('::'+suffix) for name in names)
F=Path('/private/tmp/rrrah-gradient-unroll4-managed-qualified-20261014');F.mkdir(exist_ok=False)
binary=F/'gradient-tests';assert sha(d['executable'])==d['executable_sha256'];subprocess.run(['cp','-cp',d['executable'],str(binary)],check=True);assert sha(binary)==d['executable_sha256']
O=R/'docs/research/dedup-gradient-unroll4-managed-focused-v2-audit-20261014.json';assert not O.exists()
O.write_text(json.dumps(dict(status='verified_frozen_unroll_candidate_managed_gradient_tests',passed=passed,failed=failed,ignored=ignored,filtered=filtered,tests=names,executable=str(binary),executable_sha256=sha(binary),pins={str(p):sha(p) for p in [P,M,logs[0],Path(__file__)]},source=str(S),scope=d['scope']+' No throughput measurement in this correctness gate. Two ignored manual timing tests and45non-gradient library tests not executed; no whole native operation or full workspace claim.'),indent=2)+'\n');print(O)
