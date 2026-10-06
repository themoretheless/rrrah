#!/usr/bin/env python3
"""Run every prepared pair, preserving refusals and timeouts as required outcomes."""
import argparse,collections,hashlib,json,pathlib,subprocess,time
p=argparse.ArgumentParser();p.add_argument('manifest',type=pathlib.Path);p.add_argument('probe',type=pathlib.Path);p.add_argument('report',type=pathlib.Path);p.add_argument('--timeout',type=float,default=180);p.add_argument('--spatial',action='store_true');p.add_argument('--correspondences',action='store_true');p.add_argument('--sampled',action='store_true');p.add_argument('--anchored',action='store_true');p.add_argument('--portfolio',action='store_true');a=p.parse_args()
if sum([a.spatial,a.correspondences,a.sampled,a.anchored,a.portfolio])>1:p.error('select one probe mode')
if a.timeout<=0:p.error('positive timeout required')
m=json.loads(a.manifest.read_text());assert len(m['pairs'])==m['required_pairs']==580
state={'manifest_sha256':hashlib.sha256(a.manifest.read_bytes()).hexdigest(),'probe_sha256':hashlib.sha256(a.probe.read_bytes()).hexdigest(),'required_pairs':580,'feature_selection':'global_500_sampled_2048_anchor_1_photo_0.2_5_0.1' if a.anchored else ('global_500_sampled_2048_seed_1234abcd' if a.sampled else ('correspondence_limit_grid_diagnostics' if a.correspondences else ('spatial_4x4_quota_2' if a.spatial else 'global_top_32'))),'results':[],'scope':'All publisher within-sequence pairs; geometric correspondence labels do not prove file/pixel identity'}
if a.portfolio:state['feature_selection']='portfolio_global_500_sampled_2048_anchor_1_and_raw'
probe_mode='--projective-pair-portfolio' if a.portfolio else ('--projective-pair-anchored' if a.anchored else ('--projective-pair-sampled' if a.sampled else ('--projective-correspondences' if a.correspondences else ('--projective-pair-spatial' if a.spatial else '--projective-pair'))))
for pair in m['pairs']:
 row={'sequence':pair['sequence'],'target_index':pair['target_index']};started=time.monotonic()
 try:
  for side in ['left','right']:
   image=pair[side];assert hashlib.sha256(pathlib.Path(image['normalized_path']).read_bytes()).hexdigest()==image['normalized_sha256']
  assert hashlib.sha256(a.probe.read_bytes()).hexdigest()==state['probe_sha256'],'probe changed during measurement'
  command=[str(a.probe.resolve()),probe_mode,pair['left']['normalized_path'],pair['right']['normalized_path']]+[str(v) for r in pair['normalized_homography'] for v in r]
  result=subprocess.run(command,capture_output=True,text=True,timeout=a.timeout);row['returncode']=result.returncode;row['stderr']=result.stderr
  if result.returncode==0:row['evidence']=json.loads(result.stdout);row['status']=row['evidence']['status']
  else:row['status']='process_error';row['stdout']=result.stdout
 except subprocess.TimeoutExpired:row['status']='timeout'
 except Exception as error:row['status']='error';row['error']=str(error)
 row['seconds']=time.monotonic()-started;state['results'].append(row);state['completed_pairs']=len(state['results']);state['status_counts']=dict(collections.Counter(r['status'] for r in state['results']));state['candidates']=sum(r.get('evidence',{}).get('candidate',False) for r in state['results'])
 temporary=a.report.with_suffix('.tmp');temporary.write_text(json.dumps(state,indent=2)+'\n');temporary.replace(a.report)
 print(row['sequence'],row['target_index'],row['status'],flush=True)
state['status']='complete_measurement_not_qualification';a.report.write_text(json.dumps(state,indent=2)+'\n')
raise SystemExit(0 if all(r['status']=='ok' for r in state['results']) else 1)
