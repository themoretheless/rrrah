"""Audit complete-input mesh trials and retain explicit admission refusals."""
import hashlib,json
from pathlib import Path
D=Path('docs/research');p=D/'dedup-main-remaining-mesh.json';h=lambda p:hashlib.sha256(Path(p).read_bytes()).hexdigest();r=json.loads(p.read_text());assert r['status']=='complete' and len(r['results'])==r['required_cases']==6;assert all(h(k)==v for k,v in r['input_hashes'].items()) and all(h(k)==v for k,v in r['generated_hashes'].items());b=json.loads((D/'dedup-combined-domain-release-original-full.json').read_text());expected=[(q,kind) for q in ['202502.jpg','204702.jpg','212602.jpg'] for kind in ['all','global_inliers']];summary=[]
for row,(query,kind) in zip(r['results'],expected):
 assert row['query']==query and row['point_set']==kind;e=next(v['evidence'] for v in b['results'] if v['query']==query);indices=list(range(len(e['correspondences']))) if kind=='all' else e['inliers'];assert row['point_indices']==indices
 path=next(Path(k) for k in r['generated_hashes'] if Path(k).name==f'{Path(query).stem}-{kind}-points.txt');points=[list(map(float,x.split())) for x in path.read_text().splitlines()];assert points==[e['correspondences'][i][0]+e['correspondences'][i][1] for i in indices]
 result={'query':query,'point_set':kind,'original_points_retained':len(indices),'outcome':'explicit_geometry_refusal' if row['returncode'] else 'rank_measured'}
 if row['returncode']:assert row['stderr'].strip()=='Error: Invalid' and not row['stdout']
 else:
  assert row['rank']==[json.loads(x) for x in row['stdout'].splitlines()];assert [(x['direction'],x['filter_radius']) for x in row['rank']]==[(d,f) for d in ['forward','reverse'] for f in [0,3,8]]
  for x in row['rank']:assert 0<=x['valid_sites']<=x['covered']<=x['sites'] and x['pixel_reads']==5*x['sites'] and 0<=x['agreeing_pairs']<=x['informative_pairs']<=8*x['valid_sites']
  result['filter8_directions']=[{'direction':x['direction'],'agreement':x['agreeing_pairs']/x['informative_pairs'],'coverage':x['valid_sites']/x['sites']} for x in row['rank'] if x['filter_radius']==8]
 summary.append(result)
result={'status':'verified_complete_input_remaining_mesh_diagnostic','attempts':6,'mesh_refusals':5,'summary':summary,'pins':{str(x):h(x) for x in [p,Path(__file__)]},'scope':'Whole original point list or whole baseline inlier list retained; source/executable/pixel pins, native admission refusal and rank bounds/counts. No independent real geometry/mesh pixel oracle, local-region validation or copy admission/broad precision.'};out=D/'dedup-main-remaining-mesh-audit.json';assert not out.exists();out.write_text(json.dumps(result,indent=2)+'\n');print(result['status'])
