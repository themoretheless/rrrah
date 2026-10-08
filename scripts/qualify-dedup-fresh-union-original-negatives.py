"""One-query full different-origin gate for frozen fresh candidate-union recipe."""
import hashlib,json,subprocess
from pathlib import Path
root=Path('/Users/themoretheless/.codex/tmp/rrrah-dedup-copydays')
exe=root/'gradient-scales-probe-fresh-candidate-union'
manifest=root/'prepared.json';source_dir=root/'original-resolution-negative-200201'
query=root/'original-resolution-hard-crops/200301.jpg'
baseline=Path('docs/research/dedup-fresh-candidate-union-200301.json')
out=Path('docs/research/dedup-fresh-union-original-negatives-200301.json');assert not out.exists()
h=lambda f:hashlib.sha256(Path(f).read_bytes()).hexdigest()
m=json.loads(manifest.read_text());q=next(v for v in m['images']['strong'] if v['filename']==query.name);assert h(query)==q['source_sha256']
originals=m['images']['original'];assert len(originals)==157
for row in originals:assert h(source_dir/row['filename'])==row['source_sha256']
negatives=[v for v in originals if v['group_id']!=q['group_id']];assert len(negatives)==156
positive=next(v for v in originals if v['group_id']==q['group_id'])
audit=Path('docs/research/dedup-fresh-candidate-union-200301-audit.json');a=json.loads(audit.read_text());assert a['status']=='verified_fresh_union_file_parity' and all(h(k)==v for k,v in a['input_hashes'].items())
inputs=[exe,manifest,query,baseline,audit,Path(__file__),*[source_dir/v['filename'] for v in originals]]
pins={str(p):h(p) for p in inputs}
r={'status':'running_positive_control','input_hashes':pins,'query':'200301.jpg','query_group':q['group_id'],'required_negatives':156,'results':[],'scope':'Frozen fresh candidate-union source-resolution one-query156 different publisher-origin comparisons; positive raw-output parity required. Explicit native errors/false-positive stops; no semantic/burst/all-query precision claim.'}
def save():
 tmp=out.with_suffix('.tmp');tmp.write_text(json.dumps(r,indent=2)+'\n');tmp.replace(out)
def run(left):
 assert all(h(k)==v for k,v in pins.items())
 result=subprocess.run([str(exe),'original-managed-candidate-union',str(left),str(query)],capture_output=True,text=True)
 row={'returncode':result.returncode,'stdout':result.stdout,'stderr':result.stderr}
 if result.returncode:return row
 row['evidence']=json.loads(result.stdout);assert all(h(k)==v for k,v in pins.items())
 return row
save()
control=run(source_dir/positive['filename']);r['positive_control']=control;save()
if control['returncode']:r['status']='positive_native_failed';save();raise SystemExit(control['returncode'])
reference=json.loads(baseline.read_text());assert reference['status']=='verified_fresh_native_diagnostic_parity'
keys=['correspondences','matrix','inliers','regions','region_support_count','retained_before_drop','managed_used']
canonical=lambda v:json.dumps({k:v[k] for k in keys},sort_keys=True,allow_nan=False)
if canonical(control['evidence'])!=canonical(reference['evidence']):r['status']='positive_parity_failed';save();raise SystemExit(1)
r['status']='running_negatives';save()
for source in negatives:
 row=run(source_dir/source['filename']);row.update(original=source['filename'],original_group=source['group_id']);r['results'].append(row);save()
 if row['returncode']:r['status']='negative_native_failed';save();raise SystemExit(row['returncode'])
 e=row['evidence'];assert e['status']=='ok' and type(e['managed_used']) is int and e['managed_used']==0
 if e['region_support_count']:r['status']='false_positive_observed';save();raise SystemExit(2)
 save()
assert all(h(k)==v for k,v in pins.items());r['status']='complete';save()
