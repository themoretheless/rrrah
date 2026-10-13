#!/usr/bin/env python3
import hashlib,json,re,shutil
from pathlib import Path
from PIL import Image
R=Path(__file__).resolve().parents[1];D=R/'docs/research';T=Path('/Users/themoretheless/.codex/tmp')
def sha(p):return hashlib.sha256(Path(p).read_bytes()).hexdigest()
def main():
 out=D/'dedup-palette-transparency-verification-20261012.json';copy=T/'rrrah-palette-transparency-qualified-20261012';assert not out.exists()and not copy.exists()
 mp=D/'dedup-palette-transparency-snapshot-20261012.json';m=json.loads(mp.read_text());assert all(sha(Path(k))==v for k,v in m['pins'].items());parent=json.loads(Path(m['parent']).read_text());s=Path(parent['snapshot']);assert all(sha(s/k)==v for k,v in parent['file_hashes'].items())
 p=R/'crates/rrrah-dedup/tests/fixtures/palette-transparency';images={f.stem:Image.open(f)for f in p.glob('*.png')};assert len(images)==6 and all(im.size==(9,9)for im in images.values())
 for name in ['hidden-black','hidden-gradient']:assert images[name].mode=='P' and len(images[name].info['transparency'])==256
 for name in ['hidden-black','hidden-gradient','hidden-rgba']:assert all(px[3]==0 for px in images[name].convert('RGBA').getdata())
 visible=list(images['visible-center'].convert('RGBA').getdata());assert sum(px[3]!=0 for px in visible)==1 and visible[40]==(180,180,180,255)
 assert all(px==(0,0,0)for px in images['black'].getdata());assert all(px==(255,255,255)for px in images['white'].getdata())
 log=T/'rrrah-palette-transparency-tests-20261012.log';text=log.read_text();assert re.search(r'test result: ok\. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out',text);assert 'original_palette_alpha_tables_ignore_hidden_rgb_across_rgba_encoding ... ok'in text and 'original_uniform_png_hash_collision_does_not_prove_pixel_equality ... ok'in text
 executable=Path(re.search(r'Running tests/palette_transparency\.rs \(([^)]+)\)',text).group(1));shutil.copy2(executable,copy);pins={str(p):sha(p)for p in [mp,log,copy,Path(__file__)]};out.write_text(json.dumps({'status':'verified_two_real_png_decoder_palette_and_uniform_controls','tests':2,'original_png_fixtures':6,'pins':pins,'scope':'Real selected-frame decode and equality on fully transparent P/alpha-table and RGBA variants, visible one-pixel alpha change and black/white negatives; released managed credit. Frozen184 production source unchanged. Independent Pillow fixture semantics verified; no fresh186 gate, allPNG/decode/platform/RSS/full500 completion.'},indent=2)+'\n');print('Verified two real PNG file regressions')
if __name__=='__main__':main()
