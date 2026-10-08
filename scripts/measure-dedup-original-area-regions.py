"""Measure explicit area sampling on all three hard source-resolution crops."""
import hashlib,json,subprocess
from pathlib import Path
root=Path('/Users/themoretheless/.codex/tmp/rrrah-dedup-copydays')
exe=root/'gradient-scales-probe-original-area-regions'
baseline=Path('docs/research/dedup-original-resolution-three-crops.json')
out=Path('docs/research/dedup-original-area-regions.json');assert not out.exists()
ids=[('200300','200301'),('200100','200101'),('200200','200201')]
pairs=[(root/'original-resolution-hard-crops'/f'{o}.jpg',root/'original-resolution-hard-crops'/f'{q}.jpg') for o,q in ids]
h=lambda p:hashlib.sha256(Path(p).read_bytes()).hexdigest()
pins={str(p):h(p) for p in [exe,baseline,Path(__file__),*[p for pair in pairs for p in pair]]}
r={'status':'running','input_hashes':pins,'results':[],'scope':'All three known original-resolution hard crops with explicit area descriptor sampling, unchanged matcher/geometric/pixel thresholds; includes known recovered point-sampling control. No default promotion, broad recall or negative precision claim.'}
save=lambda:out.write_text(json.dumps(r,indent=2)+'\n')
save()
for left,right in pairs:
 assert all(h(k)==v for k,v in pins.items())
 run=subprocess.run([str(exe),'original-managed-regions-area',str(left),str(right)],capture_output=True,text=True)
 row={'query_id':right.name,'returncode':run.returncode,'stdout':run.stdout,'stderr':run.stderr};r['results'].append(row);save()
 if run.returncode:r['status']='native_failed';save();raise SystemExit(run.returncode)
 e=json.loads(run.stdout);row['evidence']=e;save()
 assert e['status']=='ok' and type(e['managed_used']) is int and e['managed_used']==0
 prior=next(v['evidence']['stats'][1]['distinct'] for v in json.loads(baseline.read_text())['results'] if v['query_id']==right.name)
 row['previous_matches']=len(prior['correspondences']);row['previous_inliers']=len(prior['inliers']);row['previous_region_support_count']=prior['region_support_count']
 assert all(h(k)==v for k,v in pins.items());save()
r['status']='complete';save()
