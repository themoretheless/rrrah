"""Frozen release output parity on two real positives and two real negatives."""
import hashlib,json,subprocess,time
from pathlib import Path
root=Path('/Users/themoretheless/.codex/tmp/rrrah-dedup-copydays');exe=root/'gradient-scales-probe-fresh-union-release'
third=Path('docs/research/dedup-fresh-candidate-union-200301.json');first=Path('docs/research/dedup-fresh-union-original-full-strict-checkpoint-1.json');neg=Path('docs/research/dedup-fresh-union-negative-controls.json')
output=Path('docs/research/dedup-fresh-union-release-parity.json');assert not output.exists()
h=lambda p:hashlib.sha256(Path(p).read_bytes()).hexdigest()
a=json.loads(third.read_text());b=json.loads(first.read_text());c=json.loads(neg.read_text())
assert a['status']=='verified_fresh_native_diagnostic_parity' and len(b['results'])==1 and c['status']=='complete'
assert b['results'][0]['query']=='200001.jpg'
cases=[('200300.jpg','200301.jpg',a['evidence']),('200000.jpg','200001.jpg',b['results'][0]['evidence']),*[ (v['original'],'200301.jpg',v['evidence']) for v in c['results']]];assert len(cases)==4
originals=root/'original-resolution-negative-200201';strong=root/'original-resolution-strong-all'
inputs=[exe,third,first,neg,Path(__file__)]
for left,right,_ in cases:inputs.extend([originals/left,strong/right])
pins={str(p):h(p) for p in inputs}
for doc in [a,b,c]:assert all(h(k)==v for k,v in doc['input_hashes'].items())
r={'status':'running','input_hashes':pins,'results':[],'required_pairs':4,'scope':'Finite same-policy debug/release native parity on two real positives and two different-origin negatives. Elapsed observations under concurrent host load, not a controlled speed benchmark or broad release recall.'}
def save():
 tmp=output.with_suffix('.tmp');tmp.write_text(json.dumps(r,indent=2)+'\n');tmp.replace(output)
keys=['status','correspondences','matrix','inliers','regions','region_support_count','retained_before_drop','managed_used']
canonical=lambda e:json.dumps({k:e[k] for k in keys},sort_keys=True,allow_nan=False)
save()
for left,right,reference in cases:
 assert all(h(k)==v for k,v in pins.items())
 start=time.perf_counter();v=subprocess.run([str(exe),'original-managed-candidate-union',str(originals/left),str(strong/right)],capture_output=True,text=True);elapsed=time.perf_counter()-start
 row={'original':left,'query':right,'returncode':v.returncode,'stdout':v.stdout,'stderr':v.stderr,'elapsed_seconds':elapsed};r['results'].append(row);save()
 if v.returncode:r['status']='native_failed';save();raise SystemExit(v.returncode)
 row['evidence']=json.loads(v.stdout);assert all(h(k)==v for k,v in pins.items())
 if canonical(row['evidence'])!=canonical(reference):r['status']='parity_failed';save();raise SystemExit(1)
 assert row['evidence']['managed_peak']<=512*1024*1024;save()
r['status']='verified_native_release_parity';save()
