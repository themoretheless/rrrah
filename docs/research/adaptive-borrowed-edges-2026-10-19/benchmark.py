#!/usr/bin/env python3
"""Paired warm-file, in-process CPU decode/display timings; no GPU or cold-I/O claim."""
import argparse
import hashlib
import json
from pathlib import Path
import re
import statistics
import subprocess

parser=argparse.ArgumentParser(description=__doc__)
parser.add_argument('--baseline',required=True,type=Path)
parser.add_argument('--candidate',required=True,type=Path)
parser.add_argument('--corpus',required=True,type=Path)
parser.add_argument('--report',required=True,type=Path)
parser.add_argument('--pairs',type=int,default=4)
parser.add_argument('--iterations',type=int,default=9)
a=parser.parse_args()
root=Path(__file__).resolve().parents[3]
fixtures=[root/'tests/fixtures/pdf/tensor-solid-rgb.pdf',root/'tests/fixtures/pdf/tensor-parabolic-rgb.pdf',root/'tests/fixtures/pdf/coons-parabolic-rgb.pdf',a.corpus/'VectorApple.ai']
expected=json.loads((root/'docs/research/ai-upstream-poppler-2026-10-06.json').read_text())['sources'][0]
assert fixtures[-1].name==expected['name']
assert hashlib.sha256(fixtures[-1].read_bytes()).hexdigest()==expected['sha256']
result={'scope':'Same-source baseline/candidate except adaptive edge interval storage; dev optimized profile; warm filesystem, per-process3 warmups; in-process load+prepare times exclude hashing, destruction, startup and GPU; shared host', 'status':'running','pairs':[],'source_sha256':{str(p):hashlib.sha256(p.read_bytes()).hexdigest() for p in fixtures},'binary_sha256':{k:hashlib.sha256(p.read_bytes()).hexdigest() for k,p in [('baseline',a.baseline),('candidate',a.candidate)]}}
for fixture in fixtures:
 for pair in range(a.pairs):
  rows={}
  for key in (['baseline','candidate'] if pair%2==0 else ['candidate','baseline']):
   run=subprocess.run([str(getattr(a,key)),str(fixture),'0',str(a.iterations),'--managed-srgb'],capture_output=True,text=True,check=True)
   row=json.loads(run.stdout)
   row['stderr']=run.stderr
   peak=re.search(r'peak_bytes=(\d+)',run.stderr)
   row['accounted_peak_bytes']=int(peak[1]) if peak else None
   rows[key]=row
  assert rows['baseline']['display_blake3']==rows['candidate']['display_blake3'], fixture.name
  result['pairs'].append({'fixture':fixture.name,'pair':pair,'rows':rows,'ready_p50_ratio':rows['candidate']['cpu_ready_p50_ms']/rows['baseline']['cpu_ready_p50_ms']})
  a.report.write_text(json.dumps(result,indent=2)+'\n')
  print(f"{fixture.name} pair={pair} ratio={result['pairs'][-1]['ready_p50_ratio']:.3f}",flush=True)
result['summary']=[]
for fixture in fixtures:
 pairs=[p for p in result['pairs'] if p['fixture']==fixture.name]
 result['summary'].append({'fixture':fixture.name,'pairs':len(pairs),'median_pair_ratio':statistics.median(p['ready_p50_ratio'] for p in pairs),'baseline_ready_p50_ms':statistics.median(p['rows']['baseline']['cpu_ready_p50_ms'] for p in pairs),'candidate_ready_p50_ms':statistics.median(p['rows']['candidate']['cpu_ready_p50_ms'] for p in pairs),'baseline_accounted_peak':max(p['rows']['baseline']['accounted_peak_bytes'] for p in pairs),'candidate_accounted_peak':max(p['rows']['candidate']['accounted_peak_bytes'] for p in pairs)})
result['status']='completed'
a.report.write_text(json.dumps(result,indent=2)+'\n')
