#!/usr/bin/env python3
"""Compare same-geometry colored/solid alpha independently of white RGB."""
import argparse,hashlib,json,subprocess
from pathlib import Path
import numpy as np
import pypdfium2 as pdfium
from PIL import Image
p=argparse.ArgumentParser(description=__doc__);p.add_argument('corpus',type=Path);p.add_argument('--dump',type=Path,required=True);p.add_argument('--report',type=Path,required=True);a=p.parse_args()
base=Path(__file__).parent.parent/'coons-upstream-serenity-2026-10-19'
rows=[];arrays={};sources=[]
for manifest,name in [('isolated-sources.json','shade-tensor-sh1.pdf'),('solid-sources.json','shade-tensor-sh1-solid.pdf')]:
 source=next(s for s in json.loads((base/manifest).read_text())['sources'] if s['name']==name)
 path=a.corpus/name;data=path.read_bytes();assert hashlib.sha256(data).hexdigest()==source['sha256']
 sources.append(source)
 dest=a.corpus/(name+'.alpha-probe.rgba');info=json.loads(subprocess.check_output([str(a.dump),str(path),'0',str(dest)],text=True));assert (info['width'],info['height'])==(300,520)
 arrays[(name,'native')]=np.frombuffer(dest.read_bytes(),dtype=np.uint8).reshape(520,300,4)[:,:,3].astype(np.int16)
 doc=pdfium.PdfDocument(path);page=doc[0];bmp=page.render(scale=16,fill_color=(0,0,0,0));image=bmp.to_pil();assert image.mode=='RGBA',image.mode
 arrays[(name,'pdfium16')]=np.asarray(image.resize((300,520),Image.Resampling.BOX))[:,:,3].astype(np.int16)
 bmp.close();page.close();doc.close()
for engine in ['native','pdfium16']:
 c=arrays[('shade-tensor-sh1.pdf',engine)];s=arrays[('shade-tensor-sh1-solid.pdf',engine)];delta=np.abs(c-s)
 ys,xs=np.where(delta>32)
 rows.append(dict(engine=engine,different_alpha_pixels=int(np.count_nonzero(delta)),maximum_alpha_difference=int(delta.max()),above32=len(xs),selected=[dict(x=int(x),y_bottom=int(519-y),colored_alpha=int(c[y,x]),solid_alpha=int(s[y,x])) for y,x in zip(ys,xs)]))
result=dict(scope='Same control geometry, source-component codes changed to zero; direct alpha with transparent background; numerical independent-renderer evidence, no area certificate',sources=sources,decoder_sha256=hashlib.sha256(a.dump.read_bytes()).hexdigest(),pdfium_version=str(pdfium.PDFIUM_INFO),rows=rows)
a.report.write_text(json.dumps(result,indent=2)+'\n');print(json.dumps(result,indent=2))
