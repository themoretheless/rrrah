"""One source-resolution screen-copy radius8 linear-color experiment."""
import hashlib,json,subprocess
from pathlib import Path
root=Path('/Users/themoretheless/.codex/tmp/rrrah-dedup-copydays');exe=root/'gradient-scales-probe-screen-regional-model-pixels'
left=root/'original-resolution-negative-200201/200500.jpg';right=root/'original-resolution-strong-all/200501.jpg';manifest=root/'prepared.json'
output=Path('docs/research/dedup-screen-refined-models-pixels.json');assert not output.exists()
h=lambda p:hashlib.sha256(Path(p).read_bytes()).hexdigest()
m=json.loads(manifest.read_text())
for path,split in [(left,'original'),(right,'strong')]:assert h(path)==next(v['source_sha256'] for v in m['images'][split] if v['filename']==path.name)
pins={str(p):h(p) for p in [exe,left,right,manifest,Path(__file__),Path('docs/research/dedup-screen-refined-models-input.txt'),Path('docs/research/dedup-screen-filter8.json'),Path('docs/research/dedup-screen-original-refinement-work.json'),Path('docs/research/dedup-screen-original-refinement-work-audit.json')]}
r={'status':'running_native','input_hashes':pins,'original':left.name,'query':right.name,'candidate_smoothing_radius':2,'color_space':'Linear','confirmation_filter_radius':8,'scope':'Explicit linear-color radius8 regional comparison;12.8M combined domain admission, radius2 candidate-only smoothing; low-contrast lane, original-pixel region criteria and min10 geometry unchanged. Nine bounded target translations of the frozen supplied model, no refit or independent geometry admissibility claim; single known miss experiment, no default promotion, semantic precision or complete RSS bound.'}
def save():
 tmp=output.with_suffix('.tmp');tmp.write_text(json.dumps(r,indent=2)+'\n');tmp.replace(output)
r['scope']='Two supplied pixel-refined full-domain models retaining original min10 geometry; radius8 linear original-pixel grid4x4, unchanged thresholds, accepted support counted inside each fitting target domain only. Other model domains and refusals remain explicit; single diagnostic, no default or broad precision proof.';save();v=subprocess.run([str(exe),'screen-regional-model-pixels',str(left),str(right),'docs/research/dedup-screen-refined-models-input.txt'],capture_output=True,text=True);assert all(h(k)==v for k,v in pins.items());r.update(returncode=v.returncode,stdout=v.stdout,stderr=v.stderr)
if v.returncode:r['status']='native_failed';save();raise SystemExit(v.returncode)
r['evidence']=json.loads(v.stdout);e=r['evidence'];assert e['status']=='ok' and type(e['managed_used']) is int and e['managed_used']==0 and e['managed_peak']<=512*1024*1024
r['status']='complete_native_experiment';save()
