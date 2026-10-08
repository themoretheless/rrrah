"""All229 source-resolution strong pairs; preserve explicit native refusals."""
import hashlib,json,subprocess
from pathlib import Path
root=Path('/Users/themoretheless/.codex/tmp/rrrah-dedup-copydays')
exe=root/'gradient-scales-probe-fallback-release'
prepared=root/'prepared.json';strong_manifest=Path('docs/research/dedup-original-strong-inputs.json')
positive=Path('docs/research/dedup-fallback-release-parity.json');audit=Path('docs/research/dedup-fallback-release-parity-terminal-audit.json')
output=Path('docs/research/dedup-fallback-release-original-full.json');assert not output.exists()
h=lambda p:hashlib.sha256(Path(p).read_bytes()).hexdigest()
m=json.loads(prepared.read_text());sm=json.loads(strong_manifest.read_text());assert sm['status']=='verified_original_strong_archive_bytes' and sm['count']==229 and all(h(k)==v for k,v in sm['input_hashes'].items())
p=json.loads(positive.read_text());a=json.loads(audit.read_text());assert p['status']=='complete_native_release_parity' and len(p['results'])==3 and a['status']=='verified_fallback_release_parity' and a['verified_pairs']==3
assert all(h(k)==v for doc in [p,a] for k,v in doc['input_hashes'].items());assert h(exe)==p['input_hashes'][str(exe)]
strong={v['name']:v for v in sm['records']};pairs=m['positive_pairs'];assert len(pairs)==229
inputs=[exe,prepared,strong_manifest,positive,audit,Path(__file__),*[Path(k) for doc in [p,a] for k in doc["input_hashes"]]]
for v in m['images']['original']:
 path=root/'original-resolution-negative-200201'/v['filename'];assert h(path)==v['source_sha256'];inputs.append(path)
for v in strong.values():assert h(v['path'])==v['sha256'];inputs.append(Path(v['path']))
pins={str(v):h(v) for v in inputs}
r={'status':'running_positive_control','input_hashes':pins,'required_pairs':229,'max_combined_domain_pixels':12800000,'results':[],'scope':'All229 source-resolution publisher-origin strong pairs, symmetric-first then4/0 then0/4 fallback,12.8M combined-domain admission, qualified frozen optimized binary. Native refusals retained in denominator; no semantic negatives, indexed full collection or whole-image identity claim.'}
def save():
 tmp=output.with_suffix('.tmp');tmp.write_text(json.dumps(r,indent=2)+'\n');tmp.replace(output)
def run(left,right):
 assert all(h(k)==v for k,v in pins.items())
 v=subprocess.run([str(exe),'original-managed-candidate-union-fallback',str(left),str(right)],capture_output=True,text=True)
 assert all(h(k)==v for k,v in pins.items())
 row={'returncode':v.returncode,'stdout':v.stdout,'stderr':v.stderr}
 if not v.returncode:row['evidence']=json.loads(v.stdout)
 return row
save();control=run(root/'original-resolution-negative-200201/201300.jpg',strong['201303.jpg']['path']);r['positive_control']=control;save()
if control['returncode']:r['status']='positive_native_failed';save();raise SystemExit(control['returncode'])
keys=['correspondences','matrix','inliers','regions','region_support_count','retained_before_drop','managed_used','attempted_recipes','smoothing_radii']
canonical=lambda v:json.dumps({k:v[k] for k in keys},sort_keys=True,allow_nan=False)
if canonical(control['evidence'])!=canonical(p['results'][1]['evidence']):r['status']='positive_parity_failed';save();raise SystemExit(1)
r['status']='running_pairs';save()
for pair in pairs:
 left=root/'original-resolution-negative-200201'/pair['left']['filename'];right=strong[pair['right']['filename']]['path']
 row=run(left,right);row.update(original=pair['left']['filename'],query=pair['right']['filename'],group_id=pair['left']['group_id'])
 if not row['returncode']:
  e=row['evidence'];assert e['status']=='ok' and type(e['managed_used']) is int and e['managed_used']==0 and e['managed_peak']<=512*1024*1024
 r['results'].append(row);save()
r['status']='complete';save()
