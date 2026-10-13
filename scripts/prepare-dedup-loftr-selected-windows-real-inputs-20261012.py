#!/usr/bin/env python3
import hashlib,json
from pathlib import Path
from PIL import Image
import PIL
R=Path(__file__).resolve().parents[1];D=R/'docs/research';T=Path('/Users/themoretheless/.codex/tmp')
def sha(p):return hashlib.sha256(Path(p).read_bytes()).hexdigest()
def main():
 inputs=[T/'rrrah-dedup-copydays/original-resolution-strong-all/209101.jpg',T/'rrrah-dedup-copydays/original-resolution-negative-200201/209100.jpg'];folder=T/'rrrah-loftr-selected-windows-real-inputs-20261012';assert not folder.exists();folder.mkdir();rows=[]
 for i,p in enumerate(inputs):
  image=Image.open(p).convert('L');w,h=image.size;scale=min(1.0,640/max(w,h));size=tuple(max(8,int(v*scale)//8*8) for v in [w,h]);out=folder/f'{i}.gray';out.write_bytes(image.resize(size,Image.Resampling.LANCZOS).tobytes());rows.append({'source':str(p),'source_sha256':sha(p),'raw':str(out),'raw_sha256':sha(out),'dimensions':size})
 out=D/'dedup-loftr-selected-windows-real-inputs-20261012.json';assert not out.exists();out.write_text(json.dumps({'status':'prepared_fixed_real_publisher_pair_model_inputs_before_execution','images':rows,'pins':{str(p):sha(p) for p in [Path(__file__),Path(PIL.__file__),*[p for p in inputs],*[Path(r['raw']) for r in rows]]},'pillow_version':PIL.__version__,'scope':'Two actual Copydays publisher images, standard Pillow L conversion then Lanczos resize with maxside640 and dimensions floored to multiples8. Fixed raw inputs for whole-model numerical parity/RSS only. This is not rrrah original-linear-pixel preprocessing or a duplicate verdict.'},indent=2)+'\n');print([r['dimensions'] for r in rows])
if __name__=='__main__':main()
