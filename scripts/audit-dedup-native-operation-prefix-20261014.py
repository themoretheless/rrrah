#!/usr/bin/env python3
from pathlib import Path
import json,hashlib
R=Path(__file__).resolve().parents[1];D=R/'docs/research';P=D/'dedup-native-operation-benchmark-20261014.json';d=json.loads(P.read_text());sha=lambda p:hashlib.sha256(Path(p).read_bytes()).hexdigest();assert all(sha(k)==h for k,h in d['pins'].items());rows=[];group={}
for r in d['results']:
 assert sha(r['report'])==r['sha256'] and r['returncode']==0;v=json.loads(Path(r['report']).read_text());assert v['returncode']==0 and v['elapsed_ns']>0;out=json.loads(v['stdout']);assert out['managed_used']==0
 sem={k:out[k] for k in ['support','forward_support','reverse_support','forward_model_attempted','reverse_model_attempted']};key=(v['case'],v['pair']);group.setdefault(key,{})[v['version']]=(v['elapsed_ns'],sem);rows.append(dict(case=v['case'],pair=v['pair'],version=v['version'],elapsed_ns=v['elapsed_ns'],semantic_result=sem,managed_peak=out['managed_peak']))
assert rows;paired=[]
for (case,pair),versions in group.items():
 if set(versions)=={'baseline','cache_v3'}:
  a,b=versions['baseline'],versions['cache_v3'];assert a[1]==b[1];paired.append(dict(case=case,pair=pair,cache_v3_over_baseline_wall_ratio=b[0]/a[0]))
n=len(rows);O=D/f'dedup-native-operation-prefix-{n:04}-audit-20261014.json';F=D/f'dedup-native-operation-prefix-{n:04}-20261014.json';assert not O.exists() and not F.exists();F.write_text(json.dumps(d,indent=2)+'\n');O.write_text(json.dumps(dict(status='verified_execution_prefix_not_speedup_proof',pins={str(F):sha(F),str(Path(__file__)):sha(__file__)},rows=rows,paired=paired,scope=d['scope']+' Uncontrolled shared-host wall timings cannot establish isolated cache improvement/regression. Retain slow outcomes; do not publish component gain as whole-search speedup.'),indent=2)+'\n');print(paired)
