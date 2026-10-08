import json,hashlib,subprocess,time
from pathlib import Path
root=Path('/Users/themoretheless/.codex/tmp/rrrah-dedup-copydays');exe=root/'gradient-scales-probe-fallback-release'
positive_path=Path('docs/research/dedup-fallback-known-pairs-retry.json');positive_audit=Path('docs/research/dedup-fallback-known-pairs-retry-audit.json')
negative_path=Path('docs/research/dedup-fallback-original-negatives-201303-checkpoint-1-screen-retry.json');negative_audit=Path('docs/research/dedup-fallback-original-negatives-201303-checkpoint-1-screen-retry-audit.json')
h=lambda p:hashlib.sha256(Path(p).read_bytes()).hexdigest()
positive=json.loads(positive_path.read_text());negative=json.loads(negative_path.read_text());assert positive['status']=='complete' and len(positive['results'])==2 and len(negative['results'])==1
for document in [positive,negative,json.loads(positive_audit.read_text()),json.loads(negative_audit.read_text())]:assert all(h(p)==v for p,v in document['input_hashes'].items())
cases=[(v['original'],v['query'],v['evidence']) for v in positive['results']]+[(negative['results'][0]['original'],negative['query'],negative['results'][0]['evidence'])]
paths=[exe,Path(__file__),positive_path,positive_audit,negative_path,negative_audit,Path('docs/research/dedup-fallback-release-build.log'),*sorted((root/'fallback-release-source-snapshot').iterdir())]
for a,b,e in cases:paths.extend([root/'original-resolution-negative-200201'/a,root/'original-resolution-strong-all'/b])
pins={str(p):h(p) for p in paths};out=Path('docs/research/dedup-fallback-release-parity.json');assert not out.exists()
r={'status':'running_pairs','required_pairs':3,'input_hashes':pins,'results':[],'scope':'Two positive and one negative exact debug/release fallback parity. Elapsed times under concurrent workload are observations, not controlled speed claims.'}
def save():
 tmp=out.with_suffix('.tmp');tmp.write_text(json.dumps(r,indent=2)+'\n');tmp.replace(out)
def verify():assert all(h(p)==v for p,v in pins.items())
keys=['correspondences','matrix','inliers','regions','region_support_count','retained_before_drop','managed_used','smoothing_radii','attempted_recipes'];canonical=lambda e:json.dumps({k:e[k] for k in keys},sort_keys=True,allow_nan=False)
save()
for a,b,baseline in cases:
 verify();argv=[str(exe),'original-managed-candidate-union-fallback',str(root/'original-resolution-negative-200201'/a),str(root/'original-resolution-strong-all'/b)];started=time.monotonic();v=subprocess.run(argv,capture_output=True,text=True);verify()
 row={'original':a,'query':b,'argv':argv,'returncode':v.returncode,'stdout':v.stdout,'stderr':v.stderr,'elapsed_seconds':time.monotonic()-started}
 if v.returncode==0:row['evidence']=json.loads(v.stdout)
 r['results'].append(row);save()
 if v.returncode:r['status']='native_failed';save();raise SystemExit(v.returncode)
 if canonical(row['evidence'])!=canonical(baseline):r['status']='parity_failed';save();raise SystemExit(1)
verify();r['status']='complete_native_release_parity';save()
