"""Summarize audited screen-copy regional refusals without changing acceptance."""
import hashlib,json
from pathlib import Path
h=lambda p:hashlib.sha256(p.read_bytes()).hexdigest()
report=Path('docs/research/dedup-fresh-union-original-full-strict-checkpoint-9-next.json');audit=report.with_name(report.stem+'-audit.json');a=json.loads(audit.read_text());assert a['input_hashes'][str(report)]==h(report)
r=next(x for x in json.loads(report.read_text())['results'] if x['query']=='200501.jpg');e=r['evidence'];assert e==json.loads(r['stdout']) and r['returncode']==0
rows=[];failures={}
for i,v in enumerate(e['regions']):
 if v['pixels'] is None:
  k=v['fit_failure'];failures[k]=failures.get(k,0)+1;continue
 p=v['pixels'];fractions=[matched/compared if compared else 0 for matched,compared,total in p['counts']];coverage=[compared/total if total else 0 for matched,compared,total in p['counts']]
 rows.append({'region':i,'matched_fractions':fractions,'coverage':coverage,'both_direction_min_fraction':min(fractions),'offset_components_at_bound':sum(abs(x)>=.1-1e-12 for offsets in p['offsets'] for x in offsets),'accepted':v['accepted']})
assert not any(v['accepted'] for v in rows)
result={'status':'verified_refusal_summary','query':r['query'],'correspondences':len(e['correspondences']),'inliers':len(e['inliers']),'regions_with_pixel_evidence':len(rows),'fit_failures':failures,'best_bidirectional_region':max(rows,key=lambda x:x['both_direction_min_fraction']),'regions':rows,'input_hashes':{str(report):h(report),str(audit):h(audit),str(Path(__file__)):h(Path(__file__))},'visual_observation':'Query visibly contains periodic screen/moire bands and a color cast around the original sign image. This is visual observation, not an independently proven causal explanation.','next_requirement':'Investigate periodic capture artifacts and photometric model mismatch using controlled positives and negatives; retain original pixel criteria until a qualified alternative exists.'}
Path('docs/research/dedup-screen-copy-refusal.json').write_text(json.dumps(result,indent=2)+'\n');print(json.dumps({k:v for k,v in result.items() if k not in ['regions','input_hashes']},indent=2))
