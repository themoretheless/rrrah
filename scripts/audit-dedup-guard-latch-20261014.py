#!/usr/bin/env python3
from pathlib import Path
import json,hashlib,statistics
R=Path(__file__).resolve().parents[1];D=R/'docs/research';P=D/'dedup-guard-latch-20261014.json';d=json.loads(P.read_text());sha=lambda p:hashlib.sha256(Path(p).read_bytes()).hexdigest();assert d['status']=='completed_pending_independent_audit'and sha(R/'scripts/benchmark-dedup-guard-latch-20261014.py')==d['producer_sha256'];assert all(sha(k)==h for k,h in d['source_pins'].items());K=D/'dedup-guard-latch-20261014';a=(K/'baseline.rs').read_text();b=(K/'candidate.rs').read_text();assert a.count('latch.set(stop);')==1 and a.replace('latch.set(stop);','if stop { latch.set(true); }')==b
for v in d['builds']:
 assert v['controls']==272 and sha(K/(v['version']+'.rs'))==v['source_sha256']and sha(v['executable'])==v['executable_sha256'];assert json.loads((K/(v['version']+'-controls.json')).read_text())==dict(controls=272)
assert len(d['results'])==72;summary=[];calls=0
for label in sorted({r['case']for r in d['results']}):
 ratios=[]
 for pair in range(9):
  rows=[r for r in d['results']if r['case']==label and r['pair']==pair];assert len(rows)==2 and [r['version']for r in rows]==(['baseline','candidate']if pair%2==0 else['candidate','baseline']);values={}
  for r in rows:
   p=R/r['raw'];assert sha(p)==r['raw_sha256'];v=json.loads(p.read_text());assert v['returncode']==0 and v['stderr']==''and r['warmup']==(pair==0);assert v['cpu_seconds']==r['cpu_seconds']==v['after'][0]-v['before'][0]+v['after'][1]-v['before'][1];out=json.loads(v['stdout']);assert out['cancelled']==0 and out['calls']==int(v['command'][-1]);assert v['command'][0]==next(x['executable']for x in d['builds']if x['version']==r['version']);calls+=out['calls'];values[r['version']]=v['cpu_seconds'];assert values[r['version']]>0
  if pair:ratios.append(values['candidate']/values['baseline'])
 summary.append(dict(case=label,median_cpu_ratio=statistics.median(ratios),ratios=ratios))
assert summary==d['summary'];O=D/'dedup-guard-latch-audit-20261014.json';assert not O.exists();O.write_text(json.dumps(dict(status='verified_diagnostic_guard_expression_benchmark',pins={str(p):sha(p)for p in [P,Path(__file__)]},processes=72,user_callback_calls=calls,controls_per_variant=272,summary=summary,root_adopted=False,scope=d['scope']+' CPUdeltas/order/rawoutputs independently recomputed. Sharedrequest-adapter microbenchmark only; no descriptor/body-layout/fullsource/neuralroute or wholeoperation speedup proof.'),indent=2)+'\n');print(summary)
