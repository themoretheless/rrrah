#!/usr/bin/env python3
import json,hashlib,statistics
from pathlib import Path
R=Path(__file__).resolve().parents[1];D=R/'docs/research';P=D/'dedup-managed-cache-benchmark-20261014.json';sha=lambda p:hashlib.sha256(Path(p).read_bytes()).hexdigest()
d=json.loads(P.read_text());assert d['status']=='complete' and d['build_returncode']==0
assert all(sha(p)==h for p,h in d['pins'].items())
F=Path('/Users/themoretheless/.codex/tmp/rrrah-managed-cache-benchmark-20261014');assert sha(F/'target/release/managed-cache-benchmark')==d['executable_sha256']
assert len(d['results'])==len(d['cases'])==8
rows=[]
for case,result in zip(d['cases'],d['results']):
 assert sha(result['report'])==result['sha256'] and result['returncode']==0
 r=json.loads(Path(result['report']).read_text());assert r['case']==case and r['returncode']==0 and not r['stderr']
 samples=[json.loads(l) for l in r['stdout'].splitlines()];assert len(samples)==9
 for i,x in enumerate(samples):
  assert x['pair']==i and x['bits_identical'] and x['released'] and x['uncached_ns']>0 and x['cached_ns']>0
  assert x['ratio']==x['cached_ns']/x['uncached_ns']
  assert x['cached_peak']-x['uncached_peak']==8*1024*1024
 rows.append(dict(index=case['index'],label=case['label'],median_ratio=statistics.median(x['ratio'] for x in samples),matches=samples[0]['matches'],paired_measurements=9))
O=D/'dedup-managed-cache-benchmark-audit-20261014.json';assert not O.exists();O.write_text(json.dumps(dict(status='verified_production_managed_component_paired_benchmark',producer_sha256=sha(P),auditor_sha256=sha(__file__),cases=rows,paired_measurements=72,timed_calls=144,scope=d['scope'],limitations='Eight natural feature directions, seven source images, fixed1024features/bank and8MiBcache. Not full-operation speedup, larger matrices, fresh full-corpus parity or all-case coverage.'),indent=2)+'\n');print(rows)
