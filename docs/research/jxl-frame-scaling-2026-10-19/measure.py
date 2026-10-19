#!/usr/bin/env python3
"""Controlled frame-count scaling; warm decode+prepare, failures retained explicitly."""
from pathlib import Path
import argparse,hashlib,json,subprocess
p=Path(__file__).resolve().parent
ap=argparse.ArgumentParser(description=__doc__);ap.add_argument('corpus',type=Path);ap.add_argument('--bench',required=True,type=Path);ap.add_argument('--report',required=True,type=Path);a=ap.parse_args()
m=json.loads((a.corpus/'manifest.json').read_text());rows=[]
for source in m['sources']:
 path=a.corpus/source['name'];assert hashlib.sha256(path.read_bytes()).hexdigest()==source['sha256']
 for index in source['selected_indices']:
  run=subprocess.run([str(a.bench),str(path),str(index),'9','--managed-srgb'],capture_output=True,text=True)
  row=dict(file=source['name'],frames=source['frames'],index=index,exit=run.returncode,stderr=run.stderr)
  if run.returncode==0:row['timing']=json.loads(run.stdout)
  rows.append(row);print(source['frames'],index,run.returncode,flush=True)
  a.report.write_text(json.dumps(dict(scope='single fresh process per case;3warmups/9timed in-process iterations; warm filesystem/dev optimized/shared host; no decoded cache/GPU/cold-disk or paired speedup claim',source=m,bench_sha256=hashlib.sha256(a.bench.read_bytes()).hexdigest(),rows=rows),indent=2)+'\n')
