"""Native fresh candidate geometry and guarded automatic RGB-affine regions."""
import json,hashlib,shutil,subprocess
from pathlib import Path
root=Path('/Users/themoretheless/.codex/tmp/rrrah-dedup-copydays');base=Path('docs/research');h=lambda p:hashlib.sha256(Path(p).read_bytes()).hexdigest();exe=root/'affine-color-search-image-probe-screen';assert not exe.exists();shutil.copy2('target/debug/examples/affine_color_search_image_probe',exe);left=root/'original-resolution-negative-200201/200500.jpg';right=root/'original-resolution-strong-all/200501.jpg';manifest=root/'prepared.json';m=json.loads(manifest.read_text())
for path,split in [(left,'original'),(right,'strong')]:assert h(path)==next(v['source_sha256'] for v in m['images'][split] if v['filename']==path.name)
files=[exe,left,right,manifest,Path(__file__),Path('crates/rrrah-dedup/examples/affine_color_search_image_probe.rs'),*[Path('crates/rrrah-dedup/src')/name for name in ['affine_region_file.rs','affine_region_grid.rs','affine_region.rs','affine_color.rs','region_footprint.rs','local_scan.rs','lib.rs']]];pins={str(p):h(p) for p in files};out=base/'dedup-native-screen-affine-color-search.json';assert not out.exists();r={'status':'running','input_hashes':pins,'scope':'Native fresh candidate union supplies geometry; both file phases protected by common source snapshots. Automatic target-grid RGB-affine evidence, no manual ROI/model. One real positive, no broad recall/precision or identity claim.'}
def save():
 tmp=out.with_suffix('.tmp');tmp.write_text(json.dumps(r,indent=2)+'\n');tmp.replace(out)
save();v=subprocess.run([str(exe),str(left),str(right)],capture_output=True,text=True);assert all(h(k)==v for k,v in pins.items());r.update(returncode=v.returncode,stdout=v.stdout,stderr=v.stderr)
if v.returncode==0:r.update(status='terminal',evidence=json.loads(v.stdout));assert r['evidence']['managed_used']==0
else:r['status']='native_failure'
save();print(json.dumps(r.get('evidence',{})));raise SystemExit(v.returncode)
