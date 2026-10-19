#!/usr/bin/env python3
"""Exploratory same-fixture old fixed-grid versus adaptive Coons CPU cost."""
import subprocess,json,hashlib,statistics
from pathlib import Path
root=Path(__file__).resolve().parents[3]
p=Path(__file__).resolve().parent
fixture=root/'tests/fixtures/pdf/coons-parabolic-rgb.pdf'
bins={'fixed':'/tmp/rrrah-stream-raster_load_bench','adaptive':'/tmp/rrrah-alpha-isolated-target-20261016/debug/examples/raster_load_bench'}
r={'scope':'Warm filesystem; dev optimized builds; 3 warmups and 9 timed CPU load+prepare iterations; excludes startup, GPU, hashing and destruction. Shared host; exploratory only. Baseline predates Tensor planning optimization, irrelevant to this Coons-only fixture. Outputs intentionally differ; independent analytic test establishes correctness.','source_sha256':hashlib.sha256(fixture.read_bytes()).hexdigest(),'binary_sha256':{k:hashlib.sha256(Path(v).read_bytes()).hexdigest() for k,v in bins.items()},'pairs':[]}
for i in range(3):
 row={}
 for k in (['fixed','adaptive'] if i%2==0 else ['adaptive','fixed']):
  run=subprocess.run([bins[k],str(fixture),'0','9','--managed-srgb'],capture_output=True,text=True,check=True)
  row[k]={'result':json.loads(run.stdout),'stderr':run.stderr}
 r['pairs'].append(row)
 (p/'timing.json').write_text(json.dumps(r,indent=2)+'\n')
 print(i,{k:v['result']['cpu_ready_p50_ms'] for k,v in row.items()},flush=True)
r['median_p50_ms']={k:statistics.median(x[k]['result']['cpu_ready_p50_ms'] for x in r['pairs']) for k in bins}
r['status']='completed'
(p/'timing.json').write_text(json.dumps(r,indent=2)+'\n')
