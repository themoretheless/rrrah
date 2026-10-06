#!/usr/bin/env python3
"""Measure every captured registration-drift case; do not infer duplicate labels."""
import argparse,json,pathlib,hashlib,subprocess,math,statistics
p=argparse.ArgumentParser();p.add_argument('cases',type=pathlib.Path);p.add_argument('probe',type=pathlib.Path);p.add_argument('report',type=pathlib.Path);p.add_argument('--anchored',action='store_true');a=p.parse_args();inputs=json.loads(a.cases.read_text());rows=[]
probe_hash=hashlib.sha256(a.probe.read_bytes()).hexdigest()
def project(h,p):
 q=[sum(v*x for v,x in zip(row,[*p,1.])) for row in h];return [q[0]/q[2],q[1]/q[2]]
for case in inputs['cases']:
 row={'sequence':case['sequence'],'target_index':case['target_index'],'initial_grid':case['initial_grid'],'old_registered_grid':case['registered_grid']}
 for side in ['left','right']:
  v=case[side];assert hashlib.sha256(pathlib.Path(v['normalized_path']).read_bytes()).hexdigest()==v['normalized_sha256']
 assert hashlib.sha256(a.probe.read_bytes()).hexdigest()==probe_hash
 command=[str(a.probe.resolve()),'--registration-photo-anchored' if a.anchored else '--registration-photo',case['left']['normalized_path'],case['right']['normalized_path']]+[str(v) for r in case['estimated_homography'] for v in r]
 result=subprocess.run(command,capture_output=True,text=True,timeout=180);row['returncode']=result.returncode;row['stderr']=result.stderr
 if result.returncode==0:
  e=json.loads(result.stdout);row['evidence']=e
  if e['status']=='ok':
   sw,sh=case['left']['normalized_size'];tw,th=case['right']['normalized_size'];dist=[]
   for y in range(5):
    for x in range(5):
     point=[x*(sw-1)/4,y*(sh-1)/4];truth=project(case['published_homography'],point)
     if 0<=truth[0]<tw and 0<=truth[1]<th:
      estimated=project(e['registered'],point);distance=math.hypot(estimated[0]-truth[0],estimated[1]-truth[1]);assert math.isfinite(distance);dist.append(distance)
   assert len(dist)>=4;row['new_grid']={'median_pixels':statistics.median(dist),'maximum_pixels':max(dist),'valid_points':len(dist)}
 rows.append(row);a.report.write_text(json.dumps({'required_cases':len(inputs['cases']),'completed_cases':len(rows),'probe_sha256':probe_hash,'cases_sha256':hashlib.sha256(a.cases.read_bytes()).hexdigest(),'results':rows,'scope':'Photometric registration geometry diagnostic, not candidate acceptance or duplicate accuracy'},indent=2)+'\n');print(row['sequence'],row['target_index'],row.get('new_grid',row.get('evidence')),flush=True)
raise SystemExit(0 if all(r.get('evidence',{}).get('status')=='ok' for r in rows) else 1)
