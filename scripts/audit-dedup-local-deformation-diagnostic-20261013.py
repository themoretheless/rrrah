#!/usr/bin/env python3
import json,hashlib
from pathlib import Path
R=Path(__file__).resolve().parents[1];D=R/'docs/research';sha=lambda p:hashlib.sha256(Path(p).read_bytes()).hexdigest();P=D/'dedup-local-deformation-diagnostic-20261013.json';d=json.loads(P.read_text());assert d['status']=='complete_pins_verified_pending_interpretation';assert all(sha(k)==v for k,v in d['pins'].items());a=json.loads(Path(d['source_audit']).read_text());assert all(sha(Path(a['source'])/k)==v for k,v in a['source_sha256'].items());assert len(d['cases'])==12;rows=[]
for c in d['cases']:
 assert c['returncode']==0 and sha(c['result'])==c['result_sha256'];v=json.loads(Path(c['result']).read_text());assert v['managed_used']==0 and v['directions']==c['directions'] and len(v['directions'])==2;directions=[]
 for direction in v['directions']:
  seeds=direction['seed_coordinates'];rounds=direction['rounds'];assert len(seeds)==9 and [x['radius'] for x in rounds]==[0,16] and all(len(x['seeds'])==9 for x in rounds);comparisons=[]
  for i,(baseline,search) in enumerate(zip(rounds[0]['seeds'],rounds[1]['seeds'])):
   for e,radius in [(baseline,0),(search,16)]:
    assert e['state'] in ['scored','unavailable','uninformative']
    if e['state']=='scored':
     assert all(-radius<=x<=radius and x%4==0 for x in e['offset']);assert 0<e['training_pairs']<=164 and 0<e['validation_pairs']<=160
     for k in ['training','validation']:assert 0<=e[k+'_agreeing']<=e[k+'_pairs']
   if baseline['state']!='scored' or search['state']!='scored':comparisons.append(dict(seed=i,baseline=baseline['state'],search=search['state']));continue
   # Complete 31pixel contexts in both runs imply the same target-only denominator.
   assert baseline['training_pairs']==search['training_pairs'] and baseline['validation_pairs']==search['validation_pairs'];assert search['training_agreeing']>=baseline['training_agreeing']
   comparisons.append(dict(seed=i,offset=search['offset'],training_gain=(search['training_agreeing']-baseline['training_agreeing'])/search['training_pairs'],validation_gain=(search['validation_agreeing']-baseline['validation_agreeing'])/search['validation_pairs'],baseline_validation_fraction=baseline['validation_agreeing']/baseline['validation_pairs'],search_validation_fraction=search['validation_agreeing']/search['validation_pairs']))
  directions.append(comparisons)
 rows.append(dict(label=c['label'],directions=directions))
out=D/'dedup-local-deformation-diagnostic-audit-20261013.json';assert not out.exists();out.write_text(json.dumps(dict(status='verified_pins_and_evidence_invariants',producer_sha256=sha(P),verifier_sha256=sha(__file__),cases=rows,scope='Pinned source/executable/inputs and score arithmetic; natural pixel counts not independently recomputed. Fixed nine anchor-derived seeds, existing overlapping train/validation taps, no independent heldout support or copy verdict.'),indent=2)+'\n')
for r in rows:
 print(r['label'],[dict(scored=len([e for e in ds if 'training_gain'in e]),mean_validation_gain=round(sum(e.get('validation_gain',0) for e in ds)/max(1,len([e for e in ds if 'training_gain'in e])),4),mean_search_validation=round(sum(e.get('search_validation_fraction',0) for e in ds)/max(1,len([e for e in ds if 'training_gain'in e])),4)) for ds in r['directions']])
