#!/usr/bin/env python3
"""Color differences away from both native and PDFium16 solid coverage boundaries."""
from pathlib import Path
import argparse,hashlib,json,subprocess
from PIL import Image,ImageFilter,ImageChops
import pypdfium2 as pdfium
p=Path(__file__).resolve().parent
ap=argparse.ArgumentParser(description=__doc__);ap.add_argument('corpus',type=Path);ap.add_argument('--dump',required=True,type=Path);a=ap.parse_args()
def native(path):
 dest=path.with_suffix('.interior.rgba');info=json.loads(subprocess.check_output([str(a.dump),str(path),'0',str(dest)],text=True));size=info['width'],info['height'];im=Image.frombytes('RGBA',size,dest.read_bytes());return Image.alpha_composite(Image.new('RGBA',size,'white'),im).convert('RGB')
def oracle(path):
 doc=pdfium.PdfDocument(path);page=doc[0];bitmap=page.render(scale=16);im=bitmap.to_pil().convert('RGB').resize((300,520),resample=Image.Resampling.BOX);bitmap.close();page.close();doc.close();return im
sources=json.loads((p/'isolated-sources.json').read_text());solids=json.loads((p/'solid-sources.json').read_text());rows=[]
for source,solid in zip(sources['sources'],solids['sources']):
 for item in [source,solid]:
  data=(a.corpus/item['name']).read_bytes();assert len(data)==item['bytes'] and hashlib.sha256(data).hexdigest()==item['sha256']
 path=a.corpus/source['name'];solid_path=a.corpus/solid['name'];n=native(path);o=oracle(path);sn=native(solid_path);so=oracle(solid_path)
 expected=(255,0,0) if '-sh2' in source['name'] else (0,0,0)
 mask=Image.new('L',n.size);mask.putdata([255 if v==expected and w==expected else 0 for v,w in zip(sn.getdata(),so.getdata())]);mask=mask.filter(ImageFilter.MinFilter(5))
 values=[v for v,m in zip(ImageChops.difference(n,o).getdata(),mask.getdata()) if m]
 row=dict(file=source['name'],interior_pixels=len(values),max_channel_error=max(max(v) for v in values),mean_absolute_channel_error=sum(sum(v) for v in values)/(3*len(values)),pixels_above_1=sum(max(v)>1 for v in values),pixels_above_8=sum(max(v)>8 for v in values),pixels_above_32=sum(max(v)>32 for v in values));rows.append(row);print(row,flush=True)
(p/'interior.json').write_text(json.dumps(dict(scope='Intersection of fully covered native and PDFium16 solid masks, eroded by 2pixels in all directions; excludes boundary pixels; PDFium16 BOX remains a numerical reference, not a certificate. No pass threshold.',decoder_sha256=hashlib.sha256(a.dump.read_bytes()).hexdigest(),pdfium_version=str(pdfium.PDFIUM_INFO),rows=rows),indent=2)+'\n')
