"""Pinned dense-grid candidate experiment; no copy acceptance."""
import hashlib,json,subprocess,shutil
from pathlib import Path
root=Path('/Users/themoretheless/.codex/tmp/rrrah-dedup-copydays');exe=root/'dense-patch-bounded-probe-wrinkled';assert not exe.exists();shutil.copy2('target/release/examples/dense_patch_bounded_probe',exe)
paths=[root/'original-resolution-negative-200201/200300.jpg',root/'original-resolution-strong-all/200302.jpg'];manifest=root/'prepared.json';m=json.loads(manifest.read_text());h=lambda p:hashlib.sha256(Path(p).read_bytes()).hexdigest()
for p,split in zip(paths,['original','strong']):
 row=next(v for v in m['images'][split] if v['filename']==p.name);assert row['group_id']==200300 and h(p)==row['source_sha256']
pins={str(p):h(p) for p in [exe,*paths,manifest,Path(__file__),Path('crates/rrrah-dedup/examples/dense_patch_bounded_probe.rs')]};out=Path('docs/research/dedup-wrinkled-dense-patches-bounded.json');assert not out.exists();r={'status':'running_native','input_hashes':pins,'scope':'One known wrinkled-print positive, dense gradient proposals only. No geometry/pixel acceptance, negatives, production integration or full RSS qualification.'}
def save():
 tmp=out.with_suffix('.tmp');tmp.write_text(json.dumps(r,indent=2)+'\n');tmp.replace(out)
save();v=subprocess.run([str(exe),*[str(p) for p in paths]],capture_output=True,text=True);r.update(returncode=v.returncode,stdout=v.stdout,stderr=v.stderr);assert all(h(k)==v for k,v in pins.items())
if v.returncode:r['status']='native_failed';save();raise SystemExit(v.returncode)
e=json.loads(v.stdout);assert e['status']=='ok' and all(type(v) is int and 0<=v<=20000 for v in e['feature_counts']);r.update(status='complete_native_proposals',evidence=e);save()
