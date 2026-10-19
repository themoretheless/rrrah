#!/usr/bin/env python3
"""Full public API frame/count gate; exits1 until selected JXL animation is supported."""
from pathlib import Path
import argparse,hashlib,json,subprocess,tempfile
p=Path(__file__).resolve().parent;root=p.parents[2]
ap=argparse.ArgumentParser(description=__doc__);ap.add_argument('--dump',type=Path,required=True);ap.add_argument('--report',type=Path,default=p/'viewer-current.json');a=ap.parse_args()
m=json.loads((root/'tests/fixtures/raster/jxl-animation.json').read_text());rows=[]
with tempfile.TemporaryDirectory() as directory:
 for source in m['fixtures']:
  path=root/'tests/fixtures/raster'/source['name'];assert hashlib.sha256(path.read_bytes()).hexdigest()==source['sha256']
  for index in range(3):
   expected=Path(str(path)+f'.{index}.rgba').read_bytes();assert hashlib.sha256(expected).hexdigest()==source['rgba_sha256'][index]
   dest=Path(directory)/'native.rgba';run=subprocess.run([str(a.dump),str(path),str(index),str(dest)],capture_output=True,text=True)
   row=dict(file=path.name,index=index,exit=run.returncode,passed=False)
   if run.returncode==0:
    info=json.loads(run.stdout);row['metadata']=info;row['rgba_exact']=dest.read_bytes()==expected
    row['passed']=row['rgba_exact'] and info['image_index']==index and info['image_count']==3
   else:row['error']=run.stderr
   rows.append(row)
r=dict(scope='public selected frames/count and independent source RGBA, not timing, memory or GPU gates; failures intentionally remain failures',decoder_sha256=hashlib.sha256(a.dump.read_bytes()).hexdigest(),rows=rows)
a.report.write_text(json.dumps(r,indent=2)+'\n')
raise SystemExit(0 if all(x['passed'] for x in rows) else 1)
