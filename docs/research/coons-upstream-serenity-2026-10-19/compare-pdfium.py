#!/usr/bin/env python3
"""Pinned solid-mesh coverage against independent PDFium, including box supersampling."""
import argparse,json,hashlib,subprocess
from pathlib import Path
import pypdfium2 as pdfium
from PIL import Image,ImageChops,ImageStat
p=Path(__file__).resolve().parent
ap=argparse.ArgumentParser(description=__doc__);ap.add_argument('corpus',type=Path);ap.add_argument('--dump',required=True,type=Path);ap.add_argument('--manifest',type=Path,default=p/'solid-sources.json');ap.add_argument('--report',type=Path,default=p/'pdfium-solid.json');a=ap.parse_args()
manifest=json.loads(a.manifest.read_text());rows=[]
for source in manifest['sources']:
 path=a.corpus/source['name'];data=path.read_bytes();assert len(data)==source['bytes'] and hashlib.sha256(data).hexdigest()==source['sha256']
 destination=a.corpus/(source['name']+'.native.rgba')
 info=json.loads(subprocess.check_output([str(a.dump),str(path),'0',str(destination)],text=True))
 size=(info['width'],info['height']);native=Image.frombytes('RGBA',size,destination.read_bytes());white=Image.new('RGBA',size,'white');native=Image.alpha_composite(white,native).convert('RGB')
 doc=pdfium.PdfDocument(path);page=doc[0]
 for scale in [1,4,8,16]:
  bitmap=page.render(scale=scale);oracle=bitmap.to_pil().convert('RGB');bitmap.close()
  if scale!=1:oracle=oracle.resize(size,resample=Image.Resampling.BOX)
  difference=ImageChops.difference(native,oracle);values=list(difference.getdata());maximum=max(max(v) for v in values)
  row=dict(file=source['name'],scale=scale,different_pixels=sum(any(v) for v in values),max_channel_error=maximum,mean_absolute_channel_error=sum(sum(v) for v in values)/(size[0]*size[1]*3),pixels_above_32=sum(max(v)>32 for v in values),pixels_above_128=sum(max(v)>128 for v in values))
  rows.append(row);print(row,flush=True)
 page.close();doc.close()
report=dict(scope='Pinned selected source codes are recorded in manifest. PDFium direct and high-resolution BOX averages are independent numerical references, not exact mathematical area certificates. Native source/memory limits unchanged.',pdfium_version=str(pdfium.PDFIUM_INFO),pypdfium_version=str(pdfium.PYPDFIUM_INFO),decoder_sha256=hashlib.sha256(a.dump.read_bytes()).hexdigest(),sources=manifest,rows=rows)
a.report.write_text(json.dumps(report,indent=2)+'\n')
