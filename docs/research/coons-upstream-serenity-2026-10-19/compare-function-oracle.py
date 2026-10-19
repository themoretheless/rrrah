#!/usr/bin/env python3
"""Compare identical selected source pixels against the independent tint integral."""
from pathlib import Path
import argparse,json,hashlib,subprocess
import pypdfium2 as pdfium
from PIL import Image
p=Path(__file__).resolve().parent
ap=argparse.ArgumentParser(description=__doc__);ap.add_argument('corpus',type=Path);a=ap.parse_args()
r=json.loads((p/'function-oracle.json').read_text());path=a.corpus/r['source']['name'];assert hashlib.sha256(path.read_bytes()).hexdigest()==r['source']['sha256']
doc=pdfium.PdfDocument(path);page=doc[0];images={}
for scale in [1,16]:
 bitmap=page.render(scale=scale);im=bitmap.to_pil().convert('RGB');bitmap.close()
 if scale!=1:im=im.resize((300,520),resample=Image.Resampling.BOX)
 images['pdfium'+str(scale)]=im
page.close();doc.close()
prefix=a.corpus/'function-oracle-poppler';subprocess.run(['pdftoppm','-r','72','-singlefile',str(path),str(prefix)],check=True,capture_output=True);images['poppler1']=Image.open(prefix.with_suffix('.ppm')).convert('RGB');rows=[]
for x in r['rows']:
 expected=x['integrated_rgb']['16'];native=x['native'][:3];row=dict(x=x['x'],y_bottom=x['y_bottom'],integrated=expected,rounded=[round(c) for c in expected],native=native,native_equals_rounded=native==[round(c) for c in expected],references={})
 for name,im in images.items():
  rgb=list(im.getpixel((x['x'],519-x['y_bottom'])));row['references'][name]=dict(rgb=rgb,max_error=max(abs(a-b) for a,b in zip(rgb,expected)))
 rows.append(row)
report=dict(scope='Same18interior source pixels versus independent numerical quadrature; native value rounding is checked separately. This is not a complete-page proof or a new tolerance.',source=r['source'],rows=rows,native_all_rounded=all(x['native_equals_rounded'] for x in rows),reference_max_error={name:max(x['references'][name]['max_error'] for x in rows) for name in images},pdfium_version=str(pdfium.PDFIUM_INFO))
(p/'function-oracle-renderers.json').write_text(json.dumps(report,indent=2)+'\n');print({k:v for k,v in report.items() if k in ['native_all_rounded','reference_max_error']})
