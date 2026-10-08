"""Automatic target-grid screen diagnostic, no manual ROI or copy decision."""
import json,hashlib,shutil,subprocess
from pathlib import Path
root=Path('/Users/themoretheless/.codex/tmp/rrrah-dedup-copydays');base=Path('docs/research');h=lambda p:hashlib.sha256(Path(p).read_bytes()).hexdigest();exe=root/'affine-color-grid-image-probe-screen';assert not exe.exists();shutil.copy2('target/debug/examples/affine_color_grid_image_probe',exe);left=root/'original-resolution-negative-200201/200500.jpg';right=root/'original-resolution-strong-all/200501.jpg';anchor=base/'dedup-screen-global-translation-anchor.txt';manifest=root/'prepared.json';m=json.loads(manifest.read_text())
for p,split in [(left,'original'),(right,'strong')]:assert h(p)==next(v['source_sha256'] for v in m['images'][split] if v['filename']==p.name)
pins={str(p):h(p) for p in [exe,left,right,anchor,manifest,Path(__file__),Path('crates/rrrah-dedup/examples/affine_color_grid_image_probe.rs'),*[Path('crates/rrrah-dedup/src')/name for name in ['affine_region_grid.rs','affine_region.rs','region_footprint.rs','affine_color.rs','region_grid.rs','lib.rs']]]};out=base/'dedup-native-screen-affine-color-grid.json';assert not out.exists();r={'status':'running','input_hashes':pins,'grid':[8,8],'required_maximum_regions':64,'minimum_samples':1000,'fraction':.9,'scope':'Native automatic target-grid regions under supplied baseline geometry. No file guard, automatic feature geometry or broad precision claim.'}
def save():
 tmp=out.with_suffix('.tmp');tmp.write_text(json.dumps(r,indent=2)+'\n');tmp.replace(out)
save();v=subprocess.run([str(exe),str(left),str(right),str(anchor)],capture_output=True,text=True);assert all(h(k)==v for k,v in pins.items());r.update(returncode=v.returncode,stdout=v.stdout,stderr=v.stderr)
if v.returncode==0:
 e=json.loads(v.stdout);assert e['managed_used']==0;r.update(status='terminal',evidence=e)
 if e['status']=='ok':r['both_direction_fraction_supports']=sum(all('matched' in d and d['matched']>=.9*d['samples'] for d in region['directions']) for region in e['regions'])
else:r['status']='native_failure'
save();print(json.dumps({k:v for k,v in r.items() if k in ['status','both_direction_fraction_supports','returncode']}));print(r.get('evidence',{}).get('status'));raise SystemExit(v.returncode)
