#!/usr/bin/env python3
"""Locate the pinned Tensor solid-coverage disagreement; no acceptance tolerance."""
import argparse, hashlib, json, subprocess
from pathlib import Path
import numpy as np
import pypdfium2 as pdfium
from PIL import Image
p=argparse.ArgumentParser(description=__doc__)
p.add_argument('corpus',type=Path);p.add_argument('--dump',type=Path,required=True);p.add_argument('--report',type=Path,required=True)
a=p.parse_args()
manifest=json.loads((Path(__file__).parent.parent/'coons-upstream-serenity-2026-10-19/solid-sources.json').read_text())
source=next(r for r in manifest['sources'] if r['name']=='shade-tensor-sh1-solid.pdf')
path=a.corpus/source['name'];data=path.read_bytes()
assert hashlib.sha256(data).hexdigest()==source['sha256'] and len(data)==source['bytes']
out=a.corpus/'tensor-solid-current.rgba'
info=json.loads(subprocess.check_output([str(a.dump),str(path),'0',str(out)],text=True))
assert (info['width'],info['height'])==(300,520)
raw=np.frombuffer(out.read_bytes(),dtype=np.uint8).reshape(520,300,4)
im=Image.fromarray(raw);white=Image.new('RGBA',im.size,'white')
native=np.asarray(Image.alpha_composite(white,im).convert('RGB')).astype(np.int16)
doc=pdfium.PdfDocument(path);page=doc[0];bmp=page.render(scale=16)
ref=np.asarray(bmp.to_pil().convert('RGB').resize((300,520),Image.Resampling.BOX)).astype(np.int16)
bmp.close();page.close();doc.close()
diff=np.max(np.abs(native-ref),axis=2);ys,xs=np.where(diff>32)
rows=[dict(x=int(x),y_top=int(y),y_bottom=int(519-y),native_rgba=raw[y,x].tolist(),native_white=native[y,x].tolist(),pdfium16_white=ref[y,x].tolist(),max_difference=int(diff[y,x])) for y,x in zip(ys,xs)]
report=dict(source=source,decoder_sha256=hashlib.sha256(a.dump.read_bytes()).hexdigest(),native_sha256=hashlib.sha256(out.read_bytes()).hexdigest(),pdfium_version=str(pdfium.PDFIUM_INFO),scale=16,scope='Numerical PDFium comparison and candidate localization only; threshold32 selects investigation points and is not an acceptance tolerance',selected_pixels=len(rows),max_difference=int(diff.max()),rows=rows)
a.report.write_text(json.dumps(report,indent=2)+'\n')
print(json.dumps(report,indent=2))
