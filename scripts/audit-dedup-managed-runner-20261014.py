#!/usr/bin/env python3
import json,hashlib,statistics
from pathlib import Path
R=Path(__file__).resolve().parents[1];D=R/'docs/research';P=D/'dedup-managed-runner-benchmark-20261014.json';sha=lambda p:hashlib.sha256(Path(p).read_bytes()).hexdigest()
d=json.loads(P.read_text());assert d['status']=='complete' and d['build_returncode']==0
assert all(sha(p)==h for p,h in d['pins'].items())
F=Path('/Users/themoretheless/.codex/tmp/rrrah-managed-runner-benchmark-20261014');assert sha(F/'target/release/managed-cache-benchmark')==d['executable_sha256']
assert len(d['results'])==len(d['cases'])==1
rows=[]
for case,result in zip(d['cases'],d['results']):
 assert sha(result['report'])==result['sha256'] and result['returncode']==0
 r=json.loads(Path(result['report']).read_text());assert r['case']==case and r['returncode']==0 and not r['stderr']
 samples=[json.loads(l) for l in r['stdout'].splitlines()];assert len(samples)==9
 for i,x in enumerate(samples):
  assert x['pair']==i and x['bits_identical'] and x['released'] and x['uncached_ns']>0 and x['cached_ns']>0
  assert x['ratio']==x['cached_ns']/x['uncached_ns']
  assert x['cached_peak']-x['uncached_peak']==2*1537*16
 rows.append(dict(index=case['index'],label=case['label'],median_ratio=statistics.median(x['ratio'] for x in samples),matches=samples[0]['matches'],paired_measurements=9))
O=D/'dedup-managed-runner-benchmark-audit-20261014.json';assert not O.exists();O.write_text(json.dumps(dict(status='verified_production_managed_linear_runner_component',producer_sha256=sha(P),auditor_sha256=sha(__file__),cases=rows,paired_measurements=9,timed_calls=18,scope=d['scope'],limitations='One derived synthetic1537feature bank from retained natural features, repeated prefix with shifted coordinates,49184bytes optional linear scratch. Not full-operation speedup, larger matrices, fresh full-corpus parity or all-case coverage.'),indent=2)+'\n');print(rows)
