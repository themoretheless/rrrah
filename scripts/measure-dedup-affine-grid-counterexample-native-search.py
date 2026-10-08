"""Fresh native feature geometry on the observed color-only counterexample."""
import json,hashlib,subprocess
from pathlib import Path
root=Path('/Users/themoretheless/.codex/tmp/rrrah-dedup-copydays');base=Path('docs/research');h=lambda p:hashlib.sha256(Path(p).read_bytes()).hexdigest();counter=base/'dedup-affine-grid-201900-counterexample.json';c=json.loads(counter.read_text());left=Path(c['source_row']['source']);assert h(left)==c['source_row']['source_sha256'];right=root/'original-resolution-strong-all/200501.jpg';exe=root/'affine-color-search-image-probe-screen';positive=base/'dedup-native-screen-affine-color-search.json';r0=json.loads(positive.read_text());assert r0['status']=='terminal' and all(h(k)==v for k,v in r0['input_hashes'].items());pins={str(p):h(p) for p in [counter,left,right,exe,positive,Path(__file__)]};out=base/'dedup-affine-grid-201900-native-search.json';assert not out.exists();r={'status':'running','input_hashes':pins,'scope':'Same frozen complete native search as real screen positive on unrelated201900 color-only counterexample; no supplied model/ROI. A single targeted control, not broad precision proof.'}
def save():
 tmp=out.with_suffix('.tmp');tmp.write_text(json.dumps(r,indent=2)+'\n');tmp.replace(out)
save();v=subprocess.run([str(exe),str(left),str(right)],capture_output=True,text=True);assert all(h(k)==v for k,v in pins.items());r.update(returncode=v.returncode,stdout=v.stdout,stderr=v.stderr)
if v.returncode==0:r.update(status='terminal',evidence=json.loads(v.stdout));assert r['evidence']['managed_used']==0
else:r['status']='native_failure'
save();print(json.dumps(r.get('evidence',{})));raise SystemExit(v.returncode)
