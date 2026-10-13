#!/usr/bin/env python3
from pathlib import Path
import json,hashlib,statistics
R=Path(__file__).resolve().parents[1];D=R/'docs/research';P=D/'dedup-managed-version-benchmark-20261014.json';d=json.loads(P.read_text());sha=lambda p:hashlib.sha256(Path(p).read_bytes()).hexdigest();assert d['status']=='complete_pending_audit' and all(sha(k)==h for k,h in d['pins'].items());groups={}
for r in d['results']:
 assert sha(r['report'])==r['sha256'] and r['returncode']==0;v=json.loads(Path(r['report']).read_text());samples=[json.loads(l) for l in v['stdout'].splitlines()];assert v['returncode']==0 and v['cpu_seconds']>0 and len(samples)==9;assert all(x['bits_identical'] and x['released'] and x['cached_peak']==x['uncached_peak'] for x in samples);groups.setdefault((v['case'],v['pair']),{})[v['version']]=v
assert len(groups)==8 and len(d['results'])==16
pairs=[]
for (case,pair),g in groups.items():
 assert set(g)=={'baseline','cache_disabled'};a,b=g['baseline'],g['cache_disabled'];assert [x['matches'] for x in map(json.loads,a['stdout'].splitlines())]==[x['matches'] for x in map(json.loads,b['stdout'].splitlines())];pairs.append(dict(case=case,pair=pair,cpu_ratio=b['cpu_seconds']/a['cpu_seconds'],wall_ratio=b['wall_ns']/a['wall_ns']))
summary={c:statistics.median(p['cpu_ratio'] for p in pairs if p['case']==c) for c in ['self','different']};O=D/'dedup-managed-version-benchmark-audit-20261014.json';assert not O.exists();O.write_text(json.dumps(dict(status='verified_uncached_managed_version_component_comparison',pins={str(P):sha(P),str(Path(__file__)):sha(__file__)},pairs=pairs,median_cpu_ratio=summary,scope=d['scope']+' Only1024banks and standalone release compiler context; does not rule out large-bank/callback/whole-search regression. Does not establish cross-version point-bit equality from counts.'),indent=2)+'\n');print(summary)
