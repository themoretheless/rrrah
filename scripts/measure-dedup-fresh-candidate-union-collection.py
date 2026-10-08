"""Same known hard crop through managed two-recipe descriptor retrieval and fresh confirmation."""
import hashlib,json,subprocess
from pathlib import Path
root=Path('/Users/themoretheless/.codex/tmp/rrrah-dedup-copydays');exe=root/'gradient-scales-probe-fresh-candidate-union-collection';left=root/'original-resolution-hard-crops/200300.jpg';right=root/'original-resolution-hard-crops/200301.jpg';baseline=Path('docs/research/dedup-fresh-candidate-union-200301.json');audit=Path('docs/research/dedup-fresh-candidate-union-200301-audit.json');out=Path('docs/research/dedup-fresh-candidate-union-collection-200301.json');assert not out.exists();h=lambda p:hashlib.sha256(Path(p).read_bytes()).hexdigest()
for p in [baseline,audit]:
 r=json.loads(p.read_text());assert all(h(k)==v for k,v in r['input_hashes'].items())
prior=json.loads(baseline.read_text());assert prior['status']=='verified_fresh_native_diagnostic_parity'
pins={str(p):h(p) for p in [exe,left,right,baseline,audit,Path(__file__)]};r={'status':'running_native','input_hashes':pins,'scope':'Two-source real known crop through both-recipe descriptor proposal index and fresh guarded compound file confirmation. All retained model/regional fields must equal separate file measurement. Finite positive, no full collection recall, negative precision or complete allocation/RSS bound.'}
def save():
 t=out.with_suffix('.tmp');t.write_text(json.dumps(r,indent=2)+'\n');t.replace(out)
save();v=subprocess.run([str(exe),'original-managed-candidate-union-collection',str(left),str(right)],capture_output=True,text=True);r.update(returncode=v.returncode,stdout=v.stdout,stderr=v.stderr);save();assert all(h(k)==v for k,v in pins.items())
if v.returncode:r['status']='native_failed';save();raise SystemExit(v.returncode)
e=json.loads(v.stdout);keys=['status','correspondences','matrix','inliers','regions','region_support_count','managed_used','retained_before_drop'];assert {k:e[k] for k in keys}=={k:prior['evidence'][k] for k in keys};assert e['managed_used']==0 and 0<e['retained_before_drop']<=e['managed_peak']<=512*1024*1024;r.update(status='verified_fresh_collection_file_parity',evidence=e);save()
