#!/usr/bin/env python3
import json,hashlib,math
from pathlib import Path
R=Path(__file__).resolve().parents[1];D=R/'docs/research';sha=lambda p:hashlib.sha256(Path(p).read_bytes()).hexdigest();P=D/'dedup-mesh-deformation-replay-20261013.json';d=json.loads(P.read_text());assert d['status']=='complete_pins_verified_pending_interpretation';assert all(sha(k)==v for k,v in d['pins'].items());a=json.loads(Path(d['source_audit']).read_text());assert all(sha(Path(a['source'])/k)==v for k,v in a['source_sha256'].items());plan=json.loads((D/'dedup-mesh-deformation-replay-plan-20261013.json').read_text());assert len(d['cases'])==12;rows=[]
for c,pc in zip(d['cases'],plan['cases']):
 assert c['label']==pc['label'] and c['returncode']==0 and sha(c['result'])==c['result_sha256'];v=json.loads(Path(c['result']).read_text());assert v['managed_used']==0 and v['directions']==c['directions'] and len(v['directions'])==2;prior=json.loads(Path(pc['replay']).read_text());ds=[]
 for direction,old in zip(v['directions'],prior['directions']):
  sw,sh=direction['source_dimensions'];tw,th=direction['target_dimensions'];assert min(sw,sh,tw,th)>0;rs=[]
  for e,old_round in zip(direction['rounds'],old['rounds']):
   assert e['radius']==old_round['radius'];records=old_round['seeds'];complete=all(x['state']=='scored' for x in records)
   if not complete:assert e['refusal']=='incomplete_nine_seed_grid';rs.append(e);continue
   if 'refusal' in e:rs.append(e);continue
   residuals=[[r['offset'][0]/sw,r['offset'][1]/sh] for r in records];seeds=old['seed_coordinates'];disp=max(math.hypot(*r) for r in residuals);slope=0.
   for face in plan['faces']:
    for i,j in zip(face,face[1:]+face[:1]):
     slope=max(slope,math.hypot(residuals[i][0]-residuals[j][0],residuals[i][1]-residuals[j][1])/math.hypot((seeds[i][0]-seeds[j][0])/tw,(seeds[i][1]-seeds[j][1])/th))
   assert math.isclose(e['maximum_displacement'],disp,rel_tol=1e-10,abs_tol=1e-12) and math.isclose(e['maximum_edge_slope'],slope,rel_tol=1e-10,abs_tol=1e-12);assert e['checks']==48 and e['faces']==8 and e['conforms']==(disp<=1.0 and slope<=10.0);rs.append(e)
  ds.append(rs)
 rows.append(dict(label=c['label'],directions=ds))
out=D/'dedup-mesh-deformation-replay-audit-20261013.json';assert not out.exists();out.write_text(json.dumps(dict(status='verified_pinned_replay_and_independent_normalized_residual_metrics',producer_sha256=sha(P),verifier_sha256=sha(__file__),cases=rows,scope='Native dimensions and fixed prior offsets; normalized displacement/edge slope independently recomputed from offsets and target coordinates. Existing topology verdicts are not independently reimplemented. Incomplete nine-seed grids refuse; no subset rescue, fitting, content confirmation, calibrated thresholds or acceptance repair.'),indent=2)+'\n')
for r in rows:print(r['label'],[[e.get('refusal',[round(e['maximum_displacement'],4),round(e['maximum_edge_slope'],4)]) if 'refusal' not in e else e['refusal'] for e in ds] for ds in r['directions']])
