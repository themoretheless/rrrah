#!/usr/bin/env python3
"""Diagnose published planar correspondences without treating them as duplicate labels."""
import argparse,collections,hashlib,json,math,pathlib,statistics
p=argparse.ArgumentParser();p.add_argument('manifest',type=pathlib.Path);p.add_argument('report',type=pathlib.Path);p.add_argument('output',type=pathlib.Path);a=p.parse_args()
manifest=json.loads(a.manifest.read_text());report=json.loads(a.report.read_text());assert hashlib.sha256(a.manifest.read_bytes()).hexdigest()==report['manifest_sha256']
required={(r['sequence'],r['target_index']):r for r in manifest['pairs']};assert len(required)==580

def project(h,point):
 if h is None:return None
 denominator=sum(v*x for v,x in zip(h[2],[*point,1.]))
 if not math.isfinite(denominator) or denominator==0:return None
 result=[sum(v*x for v,x in zip(row,[*point,1.]))/denominator for row in h[:2]]
 return result if all(math.isfinite(v) for v in result) else None

def residual(model,pair):
 if model is None:return None
 sw,sh=pair['left']['normalized_size'];tw,th=pair['right']['normalized_size'];errors=[];invalid=0
 for y in range(5):
  for x in range(5):
   point=[x*(sw-1)/4,y*(sh-1)/4];expected=project(pair['normalized_homography'],point)
   if expected is None or not (0<=expected[0]<tw and 0<=expected[1]<th):continue
   actual=project(model,point)
   if actual is None:invalid+=1;continue
   distance=math.hypot(actual[0]-expected[0],actual[1]-expected[1])
   if math.isfinite(distance):errors.append(distance)
   else:invalid+=1
 return {'valid_points':len(errors),'invalid_predictions':invalid,'median_pixels':statistics.median(errors) if errors else None,'maximum_pixels':max(errors) if errors else None}

def accepted(counts):
 return counts is not None and all(compared>=1000 and compared>=source*.3 and matched>=compared*.9 for matched,compared,source in counts)

def grid_diagnostic(grid):
 return 'no_model' if grid is None else 'insufficient_valid_grid' if grid['valid_points']<4 else 'invalid_predictions' if grid['invalid_predictions'] else 'median_above_10_pixels' if grid['median_pixels']>10 else 'median_above_2_pixels' if grid['median_pixels']>2 else 'median_at_most_2_pixels'

rows=[];seen=set()
for result in report['results']:
 key=(result['sequence'],result['target_index']);assert key in required and key not in seen;seen.add(key);pair=required[key];e=result.get('evidence',{})
 row={'sequence':key[0],'target_index':key[1],'status':result['status'],'candidate':e.get('candidate',False),'geometry_grid':residual(e.get('geometry'),pair),'registered_grid':residual(e.get('registered'),pair),'published_geometry_filtered_pass':accepted(e.get('oracle_filtered_counts'))}
 grid=row['registered_grid'] or row['geometry_grid']
 row['model_grid_diagnostic']=grid_diagnostic(grid)
 row['diagnosis']='execution_error' if result['status']!='ok' else 'no_geometry' if e.get('geometry') is None else 'candidate' if row['candidate'] else 'published_geometry_also_fails_pixels' if not row['published_geometry_filtered_pass'] else 'estimated_model_pixel_rejection'
 portfolio=e.get('portfolio')
 if isinstance(portfolio,dict):
  lanes={}
  for index,name in enumerate(['anchored','unanchored']):
   lane=portfolio[name];lane_grid=residual(lane['registered'],pair);passed=accepted(lane['filtered_counts'])
   assert portfolio['accepted_lanes'][index]==passed,'portfolio lane admission does not match fixed criteria'
   lanes[name]={'registered_grid':lane_grid,'model_grid_diagnostic':grid_diagnostic(lane_grid),'filtered_pass':passed}
  assert e['candidate']==any(v['filtered_pass'] for v in lanes.values())
  row['portfolio']={'chosen_lane':portfolio['chosen_lane'],'lanes':lanes}
 rows.append(row)
summary={'completed':len(rows),'required':580,'complete':len(rows)==580,'diagnoses':dict(collections.Counter(r['diagnosis'] for r in rows)),'by_sequence_type':{kind:dict(collections.Counter(r['diagnosis'] for r in rows if r['sequence'].startswith(kind+'_'))) for kind in ['i','v']},'model_grid_diagnostics':dict(collections.Counter(r['model_grid_diagnostic'] for r in rows)),'rows':rows,'scope':'Incomplete reports remain incomplete. Publisher geometric labels are not duplicate truth; pixel criterion retains the frozen policy. Published-geometry pixel rejection is not proof of accurate estimated geometry or a photometric cause. Finite grid samples and their medians are diagnostics, not continuous transform certificates. Grid residuals assume the recorded pixel-center convention.'}
if report.get('feature_selection','').startswith('portfolio_'):
 summary['portfolio_grid_diagnostics']={name:dict(collections.Counter(r.get('portfolio',{}).get('lanes',{}).get(name,{}).get('model_grid_diagnostic','no_model') for r in rows)) for name in ['anchored','unanchored']}
 summary['portfolio_accepted_lane_grid_diagnostics']={name:dict(collections.Counter(r['portfolio']['lanes'][name]['model_grid_diagnostic'] for r in rows if r.get('portfolio',{}).get('lanes',{}).get(name,{}).get('filtered_pass'))) for name in ['anchored','unanchored']}
 summary['portfolio_selection_counts']=dict(collections.Counter('no_lane_evidence' if 'portfolio' not in r else 'neither_accepted' if not r['candidate'] else ['anchored','unanchored'][r['portfolio']['chosen_lane']] for r in rows))
a.output.write_text(json.dumps(summary,indent=2,allow_nan=False)+'\n');print({k:summary[k] for k in ['completed','required','complete','diagnoses','by_sequence_type','model_grid_diagnostics']})
