#!/usr/bin/env python3
"""Run every prepared pair, preserving refusals and timeouts as required outcomes."""
import argparse,collections,hashlib,json,pathlib,subprocess,time
p=argparse.ArgumentParser();p.add_argument('manifest',type=pathlib.Path);p.add_argument('probe',type=pathlib.Path);p.add_argument('report',type=pathlib.Path);p.add_argument('--timeout',type=float,default=180);p.add_argument('--spatial',action='store_true');p.add_argument('--correspondences',action='store_true');p.add_argument('--sampled',action='store_true');p.add_argument('--anchored',action='store_true');p.add_argument('--portfolio',action='store_true');p.add_argument('--spatial-portfolio',action='store_true',help='Fixed4x4/32-per-cell features with both original registration lanes.');p.add_argument('--resume',action='store_true',help='Validate pinned checkpoint and continue only the unexecuted suffix.');a=p.parse_args()
if sum([a.spatial,a.correspondences,a.sampled,a.anchored,a.portfolio,a.spatial_portfolio])>1:p.error('select one probe mode')
if a.timeout<=0:p.error('positive timeout required')
m=json.loads(a.manifest.read_text());assert len(m['pairs'])==m['required_pairs']==580
state={'manifest_sha256':hashlib.sha256(a.manifest.read_bytes()).hexdigest(),'probe_sha256':hashlib.sha256(a.probe.read_bytes()).hexdigest(),'required_pairs':580,'feature_selection':'global_500_sampled_2048_anchor_1_photo_0.2_5_0.1' if a.anchored else ('global_500_sampled_2048_seed_1234abcd' if a.sampled else ('correspondence_limit_grid_diagnostics' if a.correspondences else ('spatial_4x4_quota_2' if a.spatial else 'global_top_32'))),'results':[],'scope':'All publisher within-sequence pairs; geometric correspondence labels do not prove file/pixel identity'}
if a.portfolio:state['feature_selection']='portfolio_global_500_sampled_2048_anchor_1_and_raw'
if a.spatial_portfolio:state['feature_selection']='portfolio_spatial_4x4_quota_32_max_500_sampled_2048_anchor_1_and_raw'
probe_mode='--projective-pair-spatial-portfolio' if a.spatial_portfolio else '--projective-pair-portfolio' if a.portfolio else ('--projective-pair-anchored' if a.anchored else ('--projective-pair-sampled' if a.sampled else ('--projective-correspondences' if a.correspondences else ('--projective-pair-spatial' if a.spatial else '--projective-pair'))))
pairs=m['pairs']
if a.report.exists() and not a.resume:
 raise RuntimeError('Existing report requires explicit validated --resume.')
if a.resume:
 checkpoint_bytes=a.report.read_bytes();previous=json.loads(checkpoint_bytes)
 for key in ('manifest_sha256','probe_sha256','required_pairs','feature_selection'):
  assert previous[key]==state[key],f'Checkpoint {key} differs from requested measurement.'
 completed=previous['results']
 assert previous['completed_pairs']==len(completed)<=len(pairs)
 for row,pair in zip(completed,pairs):
  assert (row['sequence'],row['target_index'])==(pair['sequence'],pair['target_index'])
  assert row['status'] in ('ok','error','timeout','process_error')
  for side in ('left','right'):
   image=pair[side]
   assert hashlib.sha256(pathlib.Path(image['normalized_path']).read_bytes()).hexdigest()==image['normalized_sha256']
 # Use the independent evidence-invariant verifier before modifying the checkpoint.
 audit=a.report.with_name(a.report.stem+'-resume-integrity.json')
 subprocess.run(['python3',str(pathlib.Path(__file__).with_name('verify-dedup-hpatches-report.py')),
                 str(a.manifest),str(a.report),str(a.probe),str(audit)],check=True)
 state=previous
 state.pop('status',None)
 state.setdefault('resume_events',[]).append({'completed_pairs':len(completed),
   'checkpoint_sha256':hashlib.sha256(checkpoint_bytes).hexdigest(),
   'integrity_report':str(audit),'per_pair_timeout_seconds':a.timeout,
   'reason':'Explicit resume after process disappearance; ordered records, pinned inputs, probe and native evidence invariants validated.'})
 pairs=pairs[len(completed):]
for pair in pairs:
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
