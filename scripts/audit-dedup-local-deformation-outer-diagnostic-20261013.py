#!/usr/bin/env python3
import json,hashlib
from pathlib import Path
R=Path(__file__).resolve().parents[1];D=R/'docs/research';sha=lambda p:hashlib.sha256(Path(p).read_bytes()).hexdigest();P=D/'dedup-local-deformation-outer-diagnostic-20261013.json';d=json.loads(P.read_text());assert d['status']=='complete_pins_verified_pending_interpretation';assert all(sha(k)==v for k,v in d['pins'].items());a=json.loads(Path(d['source_audit']).read_text());assert all(sha(Path(a['source'])/k)==v for k,v in a['source_sha256'].items());prior=json.loads((D/'dedup-local-deformation-diagnostic-20261013.json').read_text());assert len(d['cases'])==len(prior['cases'])==12;rows=[]
for c,old in zip(d['cases'],prior['cases']):
 assert c['label']==old['label'] and c['returncode']==0 and sha(c['result'])==c['result_sha256'];v=json.loads(Path(c['result']).read_text());assert v['managed_used']==0 and v['directions']==c['directions'];directions=[]
 for direction,old_direction in zip(v['directions'],old['directions']):
  assert direction['seed_coordinates']==old_direction['seed_coordinates'];rounds=[]
  for round,old_round in zip(direction['rounds'],old_direction['rounds']):
   assert round['radius']==old_round['radius'];count=refused=n=b=k=0
   for seed,e,oe in zip(direction['seed_coordinates'],round['seeds'],old_round['seeds']):
    assert {key:value for key,value in e.items() if key!='outer'}==oe
    if e['state']!='scored':continue
    assert len(e['outer'])==4
    for delta,o in zip([[-48,0],[48,0],[0,-48],[0,48]],e['outer']):
     if 'refusal' in o:refused+=1;continue
     cx=seed[0]+delta[0];cy=seed[1]+delta[1];assert o['region']==[cx-2,cy-2,5,5]
     # Baseline target taps span +/-6 around the outer center, whereas
     # all local-fit target taps span +/-15 around this seed: gap >=27.
     assert abs(delta[0])>=48 or abs(delta[1])>=48
     assert o['sites']==25 and 0<o['valid_sites']<=25 and 0<o['informative_pairs']<=8*o['valid_sites'];assert 0<=o['baseline_agreeing_pairs']<=o['informative_pairs'] and 0<=o['best_competitor_agreeing_pairs']<=o['informative_pairs'];count+=1;n+=o['informative_pairs'];b+=o['baseline_agreeing_pairs'];k+=o['best_competitor_agreeing_pairs']
   rounds.append(dict(radius=round['radius'],scored_windows=count,refused_windows=refused,informative_pairs=n,baseline_fraction=b/n if n else None,best_competitor_fraction=k/n if n else None,pooled_margin=(b-k)/n if n else None))
  directions.append(rounds)
 rows.append(dict(label=c['label'],directions=directions))
out=D/'dedup-local-deformation-outer-diagnostic-audit-20261013.json';assert not out.exists();out.write_text(json.dumps(dict(status='verified_pins_fixed_offsets_target_context_separation_and_invariants',producer_sha256=sha(P),prior_producer_sha256=sha(D/'dedup-local-deformation-diagnostic-20261013.json'),verifier_sha256=sha(__file__),cases=rows,scope='Exact same fitted offsets as prior diagnostic, no feedback from outer scores. Baseline outer target taps disjoint from same seed local-fit target taps. Other seed/source/global/neural contexts not certified independent. Natural pixel counts not independently recomputed. Aggregates use available windows whose availability and source denominator can differ between radii; no paired-population quality gain or copy verdict.'),indent=2)+'\n')
for r in rows:print(r['label'],[[round['scored_windows'],None if round['pooled_margin'] is None else round['pooled_margin']] for ds in r['directions'] for round in ds])
