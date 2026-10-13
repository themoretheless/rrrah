#!/usr/bin/env python3
import json,hashlib
from pathlib import Path
R=Path(__file__).resolve().parents[1];D=R/'docs/research';sha=lambda p:hashlib.sha256(Path(p).read_bytes()).hexdigest();P=D/'dedup-competitive-rank-diagnostic-20261013.json';d=json.loads(P.read_text());assert d['status']=='complete_pins_verified_pending_interpretation';assert all(sha(k)==v for k,v in d['pins'].items());a=json.loads(Path(d['source_audit']).read_text());assert all(sha(Path(a['source'])/k)==v for k,v in a['source_sha256'].items());assert len(d['cases'])==12
rows=[]
for c in d['cases']:
 assert c['returncode']==0 and sha(c['result'])==c['result_sha256'];v=json.loads(Path(c['result']).read_text());assert v['managed_used']==0 and v['directions']==c['directions'] and len(v['directions'])==2
 directions=[]
 for e in v['directions']:
  if 'refusal' in e:directions.append(e);continue
  n=e['informative_pairs'];b=e['baseline_agreeing_pairs'];cs=e['competitors'];assert [x['offset'] for x in cs]==[[32,0],[-32,0],[0,32],[0,-32]];best=max(x['agreeing_pairs'] for x in cs);assert e['best_competitor_agreeing_pairs']==best and n>0 and 0<=b<=n and all(0<=x['agreeing_pairs']<=n for x in cs);assert 0<e['valid_sites']<=e['sites']<=1024 and n<=8*e['valid_sites'] and e['pixel_reads']<=100000
  directions.append(dict(informative_pairs=n,baseline_fraction=b/n,best_competitor_fraction=best/n,margin_fraction=(b-best)/n,valid_sites=e['valid_sites']))
 rows.append(dict(label=c['label'],directions=directions))
out=D/'dedup-competitive-rank-diagnostic-audit-20261013.json';assert not out.exists();out.write_text(json.dumps(dict(status='verified_pins_and_evidence_invariants',producer_sha256=sha(P),verifier_sha256=sha(__file__),cases=rows,scope='Source/executable/input/plan pins, counts and score arithmetic verified. Pixel counts are not independently recomputed for natural images. Only fixed centered at-most32x32 windows, no heldout independence or acceptance fix.'),indent=2)+'\n')
for r in rows:print(r['label'],['refusal: '+x['refusal'] if 'refusal' in x else round(x['margin_fraction'],4) for x in r['directions']])
