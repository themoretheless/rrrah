#!/usr/bin/env python3
import hashlib,json,math
from pathlib import Path
R=Path(__file__).resolve().parents[1];D=R/'docs/research';T=Path('/Users/themoretheless/.codex/tmp')
def sha(p):return hashlib.sha256(Path(p).read_bytes()).hexdigest()
def main():
 rp=D/'dedup-loftr-selected-windows-real-model-v2-20261012.json';r=json.loads(rp.read_text());assert r['status']=='complete_two_direction_nonempty_raw_output_parity_not_adopted';assert all(sha(p)==h for p,h in r['pins'].items());assert [(v['mode'],v['direction']) for v in r['results']]==[('baseline',0),('selected',0),('selected',1),('baseline',1)]
 original=R/'scripts/dedup-loftr-native-probe/src/main.rs';code=original.read_text().replace('if count > 5000 {','if count == 0 || count > 5000 {').replace('"confidence":confidence}', '"confidence":confidence,"batch_indexes":Vec::<i64>::try_from(&result.batch_indexes)?}');code=code.replace('Vec::<Vec<f32>>','Vec::<Vec<f64>>').replace('Vec::<f32>','Vec::<f64>').replace('"batch_indexes":','"kinds":[format!("{:?}",result.keypoints0.kind()),format!("{:?}",result.keypoints1.kind()),format!("{:?}",result.confidence.kind())],"batch_indexes":')
 for b in r['builds']:
  assert b['returncode']==0;h=T/f'rrrah-loftr-selected-windows-real-model-v2-{b["mode"]}-20261012';assert (h/'src/bin'/f'selected_window_model_{b["mode"]}.rs').read_text()==code
 assert (T/'rrrah-loftr-selected-windows-real-model-v2-baseline-20261012/Cargo.lock').read_bytes()==(T/'rrrah-loftr-selected-windows-real-model-v2-selected-20261012/Cargo.lock').read_bytes()
 comparisons=[]
 for direction in [0,1]:
  rows={x['mode']:x for x in r['results'] if x['direction']==direction};a=rows['baseline'];b=rows['selected'];assert Path(a['stdout_path']).read_bytes()==Path(b['stdout_path']).read_bytes()
  for row in rows.values():
   v=json.loads(Path(row['stdout_path']).read_text());n=len(v['confidence']);assert n==row['matches'] and 0<n<=5000 and v['kinds']==['Float','Double','Float'];assert v['batch_indexes']==[0]*n
   for key in ['keypoints0','keypoints1']:
    assert len(v[key])==n and all(len(x)==2 and all(math.isfinite(y) for y in x) for x in v[key])
   assert all(math.isfinite(x) and 0<=x<=1 for x in v['confidence'])
  comparisons.append({'direction':direction,'matches':a['matches'],'full_serialized_output_sha256':sha(a['stdout_path']),'kinds':['Float','Double','Float'],'baseline_rss_bytes':a['rss_bytes'],'selected_rss_bytes':b['rss_bytes']})
 out=D/'dedup-loftr-selected-windows-real-model-v2-audit-20261012.json';assert not out.exists();pins=dict(r['pins']);pins.update({str(p):sha(p) for p in [rp,Path(__file__)]});out.write_text(json.dumps({'status':'verified_nonempty_full_precision_model_outputs_both_directions_not_adopted','comparisons':comparisons,'pins':pins,'scope':'Actual pretrained full public model.forward on fixed real Copydays raw inputs. Float and Double coordinates/confidences serialized as f64 and complete bytes identical for each direction, including order and batch indices. These runs show no meaningful whole-model RSS improvement. Four fresh processes, concurrent workload; not isolated timing, original-pixel duplicate evidence, recall/fullcorpus/otherplatforms/full500/adoption or hard cap qualification. Earlier v1 raw probe projected Double coordinates to f32; this v2 replaces that weaker numerical scope.'},indent=2)+'\n');print('Verified full-precision equality:',[(x['matches'],x['kinds']) for x in comparisons])
if __name__=='__main__':main()
