"""One source-resolution blurred-crop candidate-smoothing experiment."""
import hashlib,json,subprocess
from pathlib import Path
root=Path('/Users/themoretheless/.codex/tmp/rrrah-dedup-copydays');exe=root/'gradient-scales-probe-asymmetric4-reverse'
left=root/'original-resolution-strong-all/201303.jpg';right=root/'original-resolution-negative-200201/201300.jpg';manifest=root/'prepared.json'
output=Path('docs/research/dedup-201303-asymmetric4-reverse.json');assert not output.exists()
h=lambda p:hashlib.sha256(Path(p).read_bytes()).hexdigest()
m=json.loads(manifest.read_text())
for path,split in [(left,'strong'),(right,'original')]:assert h(path)==next(v['source_sha256'] for v in m['images'][split] if v['filename']==path.name)
pins={str(p):h(p) for p in [exe,left,right,manifest,Path(__file__)]}
r={'status':'running_native','input_hashes':pins,'original':left.name,'query':right.name,'candidate_smoothing_radii':[0,4],'scope':'Explicit left query0/right original4 candidate-only recipe;12.8M combined-domain admission; low-contrast lane, original-pixel region criteria and min10 geometry unchanged. Single known miss experiment, no default promotion, semantic precision or complete RSS bound.'}
def save():
 tmp=output.with_suffix('.tmp');tmp.write_text(json.dumps(r,indent=2)+'\n');tmp.replace(output)
save();v=subprocess.run([str(exe),'original-managed-candidate-union-asymmetric4-reverse',str(left),str(right)],capture_output=True,text=True);assert all(h(k)==v for k,v in pins.items());r.update(returncode=v.returncode,stdout=v.stdout,stderr=v.stderr)
if v.returncode:r['status']='native_failed';save();raise SystemExit(v.returncode)
r['evidence']=json.loads(v.stdout);e=r['evidence'];assert e['status']=='ok' and type(e['managed_used']) is int and e['managed_used']==0 and e['managed_peak']<=512*1024*1024
r['status']='complete_native_experiment';save()
