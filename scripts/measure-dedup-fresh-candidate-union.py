"""Fresh compound managed file extraction versus pinned diagnostic recovery."""
import hashlib,json,subprocess
from pathlib import Path
root=Path('/Users/themoretheless/.codex/tmp/rrrah-dedup-copydays');exe=root/'gradient-scales-probe-fresh-candidate-union';left=root/'original-resolution-hard-crops/200300.jpg';right=root/'original-resolution-hard-crops/200301.jpg';baseline=Path('docs/research/dedup-original-correspondence-union-200301.json');out=Path('docs/research/dedup-fresh-candidate-union-200301.json');assert not out.exists()
h=lambda p:hashlib.sha256(Path(p).read_bytes()).hexdigest();pins={str(p):h(p) for p in [exe,left,right,baseline,Path(__file__)]};prior=json.loads(baseline.read_text());assert prior['status']=='complete_native' and all(h(k)==v for k,v in prior['input_hashes'].items())
r={'status':'running_native','input_hashes':pins,'scope':'Fresh native compound file API, both candidate recipes extracted from source JPEGs under one source/cancellation guard; managed smoothing/features/matcher/union/retained evidence. Exact model/regional parity to pinned diagnostic. Decoder and geometry construction allocation completeness, whole decision, collection and negative precision remain outside this gate.'}
def save():
 t=out.with_suffix('.tmp');t.write_text(json.dumps(r,indent=2)+'\n');t.replace(out)
save();v=subprocess.run([str(exe),'original-managed-candidate-union',str(left),str(right)],capture_output=True,text=True);r.update(returncode=v.returncode,stdout=v.stdout,stderr=v.stderr);save();assert all(h(k)==v for k,v in pins.items())
if v.returncode:r['status']='native_failed';save();raise SystemExit(v.returncode)
e=json.loads(v.stdout);keys=['status','correspondences','matrix','inliers','regions','region_support_count','managed_used'];assert {k:e[k] for k in keys}=={k:prior['evidence'][k] for k in keys};assert e['managed_used']==0 and 0<e['retained_before_drop']<=e['managed_peak']<=512*1024*1024;r.update(status='verified_fresh_native_diagnostic_parity',evidence=e);save()
