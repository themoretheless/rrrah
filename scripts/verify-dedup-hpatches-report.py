#!/usr/bin/env python3
"""Check evaluation coverage/provenance, never certify image matching correctness."""
import argparse,hashlib,json,pathlib,math,collections
p=argparse.ArgumentParser();p.add_argument('manifest',type=pathlib.Path);p.add_argument('report',type=pathlib.Path);p.add_argument('probe',type=pathlib.Path);p.add_argument('output',type=pathlib.Path);a=p.parse_args()
manifest_bytes=a.manifest.read_bytes();manifest=json.loads(manifest_bytes);report=json.loads(a.report.read_text());errors=[]
if hashlib.sha256(manifest_bytes).hexdigest()!=report['manifest_sha256']:errors.append('prepared manifest changed')
if hashlib.sha256(a.probe.read_bytes()).hexdigest()!=report['probe_sha256']:errors.append('probe binary changed since run start')
required={(r['sequence'],r['target_index']):r for r in manifest['pairs']};seen=set();verified_images=set()
if len(required)!=580 or report['required_pairs']!=580:errors.append('required pair count differs from 580')
for r in report['results']:
 key=(r['sequence'],r['target_index'])
 if key not in required or key in seen:errors.append(f'unknown or repeated pair {key}');continue
 seen.add(key);pair=required[key]
 for side in ['left','right']:
  image=pair[side];path=image['normalized_path']
  if path not in verified_images:
   if hashlib.sha256(pathlib.Path(path).read_bytes()).hexdigest()!=image['normalized_sha256']:errors.append(f'image changed {path}')
   verified_images.add(path)
 e=r.get('evidence')
 if e is None:
  if r['status'] not in ['process_error','timeout','error']:errors.append(f'missing explicit failure {key}')
  continue
 if r['returncode']!=0 or r['status']!=e['status']:errors.append(f'inconsistent status {key}')
 for field in ['managed_used','managed_peak','correspondences','inliers']:
  value=e.get(field)
  if not isinstance(value,int) or isinstance(value,bool) or value<0:errors.append(f'invalid integer {key}/{field}')
 if not 0<=e['inliers']<=e['correspondences']:errors.append(f'impossible support {key}')
 if not 0<=e['managed_used']<=e['managed_peak']<=64*1024*1024:errors.append(f'impossible managed accounting {key}')
 for field in ['geometry','registered']:
  matrix=e.get(field)
  if matrix is None:continue
  if not isinstance(matrix,list) or len(matrix)!=3 or any(not isinstance(row,list) or len(row)!=3 for row in matrix):errors.append(f'invalid matrix shape {key}/{field}');continue
  if any(not isinstance(v,(int,float)) or isinstance(v,bool) or not math.isfinite(v) for row in matrix for v in row):errors.append(f'nonfinite matrix {key}/{field}');continue
  scale=max(abs(v) for row in matrix for v in row)
  if scale==0:errors.append(f'zero matrix {key}/{field}');continue
  h=[[v/scale for v in row] for row in matrix]
  determinant=h[0][0]*(h[1][1]*h[2][2]-h[1][2]*h[2][1])-h[0][1]*(h[1][0]*h[2][2]-h[1][2]*h[2][0])+h[0][2]*(h[1][0]*h[2][1]-h[1][1]*h[2][0])
  if determinant==0 or not math.isfinite(determinant):errors.append(f'singular matrix {key}/{field}')
 if e.get('geometry') is None and (e['inliers']!=0 or e.get('registered') is not None or e.get('filtered_counts') is not None):errors.append(f'evidence without geometry {key}')
 if e['candidate'] and (e.get('geometry') is None or e.get('filtered_counts') is None):errors.append(f'candidate without confirmation {key}')
 if e['managed_used']!=0:errors.append(f'managed memory retained {key}')
 if not isinstance(e['candidate'],bool):errors.append(f'invalid candidate flag {key}')
 for field in ['strict_counts','filtered_counts','oracle_filtered_counts']:
  counts=e.get(field)
  if counts is None:continue
  if len(counts)!=2:errors.append(f'invalid directional counts {key}/{field}');continue
  for values,side in zip(counts,['left','right']):
   width,height=pair[side]['normalized_size']
   if len(values)!=3 or not all(isinstance(n,int) and not isinstance(n,bool) for n in values):errors.append(f'invalid counts {key}/{field}');continue
   matched,compared,source=values
   if not 0<=matched<=compared<=source or source!=width*height:errors.append(f'impossible pixel counts {key}/{field}')
 # Portfolio branches are required and independently checked when geometry exists.
 portfolio=e.get('portfolio')
 portfolio_mode=report.get('feature_selection','').startswith('portfolio_')
 if portfolio_mode and e.get('geometry') is not None and not isinstance(portfolio,dict):
  errors.append(f'missing portfolio branches {key}')
 if isinstance(portfolio,dict):
  flags=portfolio.get('accepted_lanes');chosen=portfolio.get('chosen_lane')
  if not isinstance(flags,list) or len(flags)!=2 or any(not isinstance(v,bool) for v in flags):
   errors.append(f'invalid portfolio admission flags {key}');continue
  if not isinstance(chosen,int) or isinstance(chosen,bool) or chosen not in [0,1]:
   errors.append(f'invalid portfolio selected lane {key}');continue
  if chosen!=int(not flags[0] and flags[1]) or e['candidate']!=any(flags):errors.append(f'portfolio admission/selection mismatch {key}')
  for index,name in enumerate(['anchored','unanchored']):
   lane=portfolio.get(name)
   if not isinstance(lane,dict):errors.append(f'missing portfolio lane {key}/{name}');continue
   matrix=lane.get('registered')
   if not isinstance(matrix,list) or len(matrix)!=3 or any(not isinstance(row,list) or len(row)!=3 for row in matrix) or any(not isinstance(v,(int,float)) or isinstance(v,bool) or not math.isfinite(v) for row in matrix for v in row):
    errors.append(f'invalid portfolio matrix {key}/{name}');continue
   scale=max(abs(v) for row in matrix for v in row)
   if not scale:errors.append(f'zero portfolio matrix {key}/{name}');continue
   h=[[v/scale for v in row] for row in matrix]
   det=h[0][0]*(h[1][1]*h[2][2]-h[1][2]*h[2][1])-h[0][1]*(h[1][0]*h[2][2]-h[1][2]*h[2][0])+h[0][2]*(h[1][0]*h[2][1]-h[1][1]*h[2][0])
   if not math.isfinite(det) or not det:errors.append(f'singular portfolio matrix {key}/{name}')
   valid=True
   for field in ['strict_counts','filtered_counts']:
    counts=lane.get(field)
    if not isinstance(counts,list) or len(counts)!=2:errors.append(f'missing portfolio counts {key}/{name}/{field}');valid=False;continue
    for values,side in zip(counts,['left','right']):
     width,height=pair[side]['normalized_size']
     if not isinstance(values,list) or len(values)!=3 or any(not isinstance(v,int) or isinstance(v,bool) for v in values):errors.append(f'invalid portfolio counts {key}/{name}/{field}');valid=False;continue
     matched,compared,source=values
     if not 0<=matched<=compared<=source or source!=width*height:errors.append(f'impossible portfolio counts {key}/{name}/{field}');valid=False
   if valid:
    passed=all(c>=1000 and c>=s*0.3 and m>=c*0.9 for m,c,s in lane['filtered_counts'])
    if flags[index]!=passed:errors.append(f'portfolio lane flag disagrees with fixed admission {key}/{name}')
   if chosen==index and (e.get('registered')!=matrix or any(e.get(field)!=lane.get(field) for field in ['strict_counts','filtered_counts'])):errors.append(f'primary evidence differs from selected portfolio lane {key}')
if report.get('status_counts')!=dict(collections.Counter(r['status'] for r in report['results'])):errors.append('status totals differ from recorded outcomes')
if report.get('candidates')!=sum(r.get('evidence',{}).get('candidate',False) for r in report['results']):errors.append('candidate total differs from recorded outcomes')
complete=len(seen)==580
if report.get('completed_pairs')!=len(seen):errors.append('completed count differs from unique recorded pairs')
if report.get('status')=='complete_measurement_not_qualification' and not complete:errors.append('incomplete report claims completed measurement')
result={'integrity_passed':not errors,'complete':complete,'recorded_pairs':len(seen),'required_pairs':580,'verified_prepared_images':len(verified_images),'errors':errors,'scope':'Current input/probe hashes, record coverage and count/memory invariants; no correctness, duplicate-label or historical ABA proof'}
a.output.write_text(json.dumps(result,indent=2)+'\n');print(result);raise SystemExit(bool(errors))
