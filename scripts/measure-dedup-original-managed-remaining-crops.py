"""Measure two still-omitted original-resolution crops with frozen collection API."""
import hashlib, json, subprocess
from pathlib import Path
root=Path('/Users/themoretheless/.codex/tmp/rrrah-dedup-copydays')
exe=root/'gradient-scales-probe-original-managed-collection'
baseline=Path('docs/research/dedup-original-resolution-three-crops.json')
out=Path('docs/research/dedup-original-managed-remaining-crops.json')
assert not out.exists()
pairs=[(root/'original-resolution-hard-crops'/f'{origin}.jpg',root/'original-resolution-hard-crops'/f'{query}.jpg') for origin,query in [('200100','200101'),('200300','200301')]]
h=lambda p:hashlib.sha256(Path(p).read_bytes()).hexdigest()
pins={str(p):h(p) for p in [exe,baseline,Path(__file__),*[p for pair in pairs for p in pair]]}
r={'status':'running','input_hashes':pins,'results':[],'scope':'Two known publisher-origin crops previously omitted, through frozen source-resolution managed collection; exact diagnostic-field parity. Broader corpus/negative precision not established.'}
save=lambda:out.write_text(json.dumps(r,indent=2)+'\n')
save()
for left,right in pairs:
 assert all(h(k)==v for k,v in pins.items())
 run=subprocess.run([str(exe),'original-managed-collection',str(left),str(right)],capture_output=True,text=True)
 row={'query_id':right.name,'returncode':run.returncode,'stdout':run.stdout,'stderr':run.stderr};r['results'].append(row);save()
 if run.returncode:r['status']='native_failed';save();raise SystemExit(run.returncode)
 e=json.loads(run.stdout);row['evidence']=e;save()
 reference=next(v['evidence']['stats'][1]['distinct'] for v in json.loads(baseline.read_text())['results'] if v['query_id']==right.name)
 keys=['correspondences','matrix','inliers','regions','region_support_count']
 canonical=lambda v:json.dumps({k:v[k] for k in keys},sort_keys=True,allow_nan=False)
 if canonical(e)!=canonical(reference):r['status']='native_output_parity_failed';save();raise SystemExit(1)
 assert e['status']=='ok' and type(e['managed_used']) is int and e['managed_used']==0
 assert all(h(k)==v for k,v in pins.items())
 row['exact_diagnostic_parity']=True;save()
r['status']='complete';save()
