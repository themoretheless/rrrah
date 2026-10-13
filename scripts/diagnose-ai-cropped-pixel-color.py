#!/usr/bin/env python3
"""Diagnostic original-resource crop oracle; stderr invalidates each sample.
Requires pypdf, Pillow and NumPy. This never qualifies full-page fidelity.
"""
from pathlib import Path
import argparse, hashlib, json, subprocess, re
from pypdf import PdfReader, PdfWriter
from pypdf.generic import RectangleObject
from PIL import Image
import numpy as np

parser=argparse.ArgumentParser()
parser.add_argument('--scales',default='64,128')
args=parser.parse_args()
scales=[int(v) for v in args.scales.split(',')]
assert scales and all(1<=v<=256 for v in scales)
root=Path(__file__).resolve().parents[1]
source=root/'tests/fixtures/data/metal-recovery/ai-corpus/VectorApple.ai'
profile=root/'vendor/hayro-interpret/assets/CGATS001Compat-v2-micro.icc'
oracle=Path('/tmp/rrrah-poppler-profile-oracle-20261012/build/utils/pdftoppm')
out=root/'docs/research'
reader=PdfReader(source)
assert list(reader.pages[0].cropbox)==[0,0,300.001,245.5]
writer=PdfWriter();writer.add_page(reader.pages[0])
writer.pages[0].cropbox=RectangleObject([199,219.5,200,220.5])
pdf=out/'ai-pixel-199-25-crop-oracle-2026-10-13.pdf'
with pdf.open('wb') as f:writer.write(f)
runs=[]
for scale in scales:
    prefix=out/f'ai-cropped-pixel-oracle-{scale}x-2026-10-13'
    cmd=[str(oracle),'-r',str(72*scale),'-f','1','-l','1','-singlefile','-cropbox',
         '-defaultcmykprofile',str(profile),str(pdf),str(prefix)]
    result=subprocess.run(cmd,capture_output=True,text=True)
    path=prefix.with_suffix('.ppm')
    data=np.asarray(Image.open(path),dtype=np.float64) if result.returncode==0 and path.exists() else None
    valid=result.returncode==0 and not result.stderr.strip() and data is not None and data.shape==(scale,scale,3)
    runs.append({'scale':scale,'valid':valid,'returncode':result.returncode,'stderr':result.stderr,
                 'mean_rgb':data.mean(axis=(0,1)).tolist() if valid else None,
                 'command':cmd,'sha256':hashlib.sha256(path.read_bytes()).hexdigest() if path.exists() else None})
valid=[r for r in runs if r['valid']]
native_log=out/'ai-incremental-routing-original-2026-10-13.log'
log=native_log.read_text()
assert 'REAL_AI_SPATIAL_FINAL_CREDIT=0' in log
match=re.search(r'REAL_AI_SPATIAL_PIXEL pixel=\(199,25\).*completed=true.*white_rgb=Some\((\[[^]]+\])\)',log)
assert match, 'Completed original pixel evidence required'
native=json.loads(match[1])
report={'scope':'Original-resource crop diagnostic at199,25; original AI unchanged',
        'cropbox':[199,219.5,200,220.5],'runs':runs,'native_rgb':native,
        'last_valid_max_rgb8_difference':max(abs(a-b) for a,b in zip(native,valid[-1]['mean_rgb'])) if valid else None,
        'last_valid_scale_delta':max(abs(a-b) for a,b in zip(valid[-1]['mean_rgb'],valid[-2]['mean_rgb'])) if len(valid)>1 else None,
        'converged':False,'limitations':['Derived crop PDF; no full-page acceptance','Valid scales still require convergence','Native diagnostic values pinned to incremental-routing-original log'],
        'sha256':{str(p.relative_to(root)) if p.is_relative_to(root) else str(p):hashlib.sha256(p.read_bytes()).hexdigest() for p in [source,profile,oracle,native_log,Path(__file__)]}}
(out/'ai-cropped-pixel-color-verified-2026-10-13.json').write_text(json.dumps(report,indent=2)+'\n')
print('valid scales', [r['scale'] for r in valid], 'difference',report['last_valid_max_rgb8_difference'],'scale delta',report['last_valid_scale_delta'])
raise SystemExit(0 if all(r['valid'] for r in runs) else 1)
