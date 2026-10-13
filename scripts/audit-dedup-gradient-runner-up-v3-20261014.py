#!/usr/bin/env python3
from pathlib import Path
import json,hashlib,statistics
R=Path(__file__).resolve().parents[1];D=R/'docs/research';sha=lambda p:hashlib.sha256(Path(p).read_bytes()).hexdigest();P=D/'dedup-gradient-runner-up-v3-experiment-20261014.json';Q=D/'dedup-gradient-natural-runner-up-v3-benchmark-20261014.json';A=D/'dedup-gradient-natural-runner-up-v3-benchmark-plan-20261014.json'
p=json.loads(P.read_text());q=json.loads(Q.read_text());a=json.loads(A.read_text());assert p['returncode']==0 and q['status']=='complete_eight_direction_runner_up_microbench_pending_audit'
assert all(sha(k)==h for k,h in p['pins'].items()) and all(sha(k)==h for k,h in a['pins'].items());assert q['plan_sha256']==sha(A)
rows=[]
for case,r in zip(a['cases'],q['results']):
 assert r['returncode']==0 and sha(r['report'])==r['sha256'];v=json.loads(Path(r['report']).read_text());assert v['command']==case['command'];s=[json.loads(l) for l in v['stdout'].splitlines()];assert len(s)==19 and s[0]['fixtures']==164 and s[0]['cancel_checkpoints_preserved'];assert all(x['nearest_tables_bit_identical'] for x in s[1:])
 groups=[]
 for mode in [False,True]:
  g=[x for x in s[1:] if x['distinct']==mode];assert len(g)==9
  groups.append(dict(distinct=mode,median_ratio=statistics.median(x['candidate_ns']/x['baseline_ns'] for x in g)))
 rows.append(dict(index=v['index'],label=v['label'],modes=groups))
assert len(rows)==8
O=D/'dedup-gradient-runner-up-v3-audit-20261014.json';assert not O.exists();O.write_text(json.dumps(dict(status='verified_standalone_runner_up_experiment',pins={str(p):sha(p) for p in [P,Q,A,Path(__file__)]},rows=rows,scope='Exact historical matcher source extraction and standalone prototype only.164synthetic fixtures preserve errors/callback counts/cancellation/results; natural nearest tables bit-identical in8direction runs/2modes/9paired timings. Linear extra runner indices and flags have no production admission yet. Not adopted, no large-bank/whole-search/all-platform proof.'),indent=2)+'\n');print(rows)
