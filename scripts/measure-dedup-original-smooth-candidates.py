"""Candidate-only 5x5 box smoothing; final regional check reads source JPEGs."""
import hashlib,json,subprocess
from pathlib import Path
root=Path('/Users/themoretheless/.codex/tmp/rrrah-dedup-copydays');exe=root/'gradient-scales-probe-original-smooth-candidates';left=root/'original-resolution-hard-crops/200300.jpg';right=root/'original-resolution-hard-crops/200301.jpg';out=Path('docs/research/dedup-original-smooth-candidates-200301.json');assert not out.exists()
h=lambda p:hashlib.sha256(Path(p).read_bytes()).hexdigest();pins={str(p):h(p) for p in [exe,left,right,Path(__file__),Path('docs/research/dedup-original-area-regions.json'),Path('docs/research/dedup-original-resolution-hard-crops.json')]}
r={'status':'running_native','input_hashes':pins,'query':'200301.jpg','scope':'Symmetric candidate-only separable 5x5 box, clamped borders, f64 accumulation/f32 intermediate; unchanged default detector and matching/geometry thresholds. Final supplied-model regional confirmation re-decodes original source pixels at grid8/radius7. Managed smoothing/feature storage, plain diagnostic matching/geometry allocations excluded; no whole decision, collection integration, negative precision or default promotion.'}
def save():
 t=out.with_suffix('.tmp');t.write_text(json.dumps(r,indent=2)+'\n');t.replace(out)
save();v=subprocess.run([str(exe),'original-smooth-candidates',str(left),str(right)],capture_output=True,text=True);r.update(returncode=v.returncode,stdout=v.stdout,stderr=v.stderr);save();assert all(h(k)==v for k,v in pins.items())
if v.returncode:r['status']='native_failed';save();raise SystemExit(v.returncode)
e=json.loads(v.stdout);assert e['status']=='ok' and e['managed_used']==0 and e['managed_peak']<=512*1024*1024;r.update(status='complete_native',evidence=e);save()
