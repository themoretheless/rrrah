"""Native model membership and independent full-cell ordinal pixel counts."""
import ast,hashlib,json
from pathlib import Path
import numpy as np
base=Path('docs/research');digest=lambda p:hashlib.sha256(Path(p).read_bytes()).hexdigest()
report_path=base/'dedup-folded-regional-rank.json';r=json.loads(report_path.read_text());assert r['status']=='complete_native_regional_rank_diagnostic' and len(r['rows'])==8
assert all(digest(k)==v for k,v in r['input_hashes'].items())
geometry_path=base/'dedup-folded-201702-regional-geometry.json';geometry=json.loads(geometry_path.read_text());models=[v for v in next(v['evidence']['regions'] for v in geometry['results'] if v['divisions']==4) if v['matrix'] is not None];assert len(models)==8
oracle_helper=Path('scripts/verify-dedup-rank-pixel-oracle.py');math_helper=Path('scripts/verify-dedup-rank-region-real.py');ns={'np':np}
for path,names in [(math_helper,['inverse']),(oracle_helper,['coordinates','measured'])]:
 nodes=[v for v in ast.parse(path.read_text()).body if isinstance(v,ast.FunctionDef) and v.name in names];assert len(nodes)==len(names);exec(compile(ast.Module(body=nodes,type_ignores=[]),str(path),'exec'),ns)
inverse,measured=ns['inverse'],ns['measured'];dump_path=base/'dedup-rank-normalized-pixels.json';dump=json.loads(dump_path.read_text());assert dump['status']=='complete_native_normalized_pixel_dump'
images={};pixel_hashes={};magic=b'RRRAH-RANK-RGBA32-V1\n'
for name in ['201700.jpg','201702.jpg']:
 row=next(v for v in dump['rows'] if Path(v['source']).name==name);path=Path(row['pixels']);assert digest(path)==row['pixels_sha256'] and digest(row['source'])==row['source_sha256'];pixel_hashes[str(path)]=row['pixels_sha256'];w,h=row['evidence']['width'],row['evidence']['height'];data=np.memmap(path,dtype='<f4',mode='r',offset=len(magic)+8,shape=(h,w,4));assert np.all(data[:,:,3]==1.)
 images[name]=np.asarray(data[:,:,0],dtype=np.float64)*.2126+np.asarray(data[:,:,1],dtype=np.float64)*.7152+np.asarray(data[:,:,2],dtype=np.float64)*.0722
a,b=images['201700.jpg'],images['201702.jpg'];summary=[];target_area=0;ordinal_eligible_area=0
for index,(row,model) in enumerate(zip(r['rows'],models)):
 assert row['model_index']==index and row['matrix']==model['matrix'] and row['native_inliers']==model['inliers'] and len(model['inliers'])>=10
 assert row['target_domain']==list(map(int,model['target_domain']))
 assert row['status']=='measured' and row['returncode']==0 and digest(row['input'])==row['input_sha256'];sr=row['source_domain'];tr=row['target_domain'];assert list(map(float,Path(row['input']).read_text().split()))==[v for axis in model['matrix'] for v in axis]+sr+tr
 inv=inverse(model['matrix']);e=row['evidence'];assert e==json.loads(row['stdout']) and e['managed_used']==0;directions=[];fractions=[]
 for native,source,target,mapping,left,right in zip(e['directions'],[a,b],[b,a],[inv,inverse(inv)],[sr,tr],[tr,sr]):
  assert native['status']=='ok' and native['sites']==right[2]*right[3]<=2000000 and native['pixel_reads']<=native['sites']*45*289
  independent=measured(source,target,mapping,left,right,8)
  assert all(independent[k]==native[k] for k in ['valid_sites','informative_pairs','agreeing_pairs'])
  directions.append(independent);fractions.append(native['agreeing_pairs']/native['informative_pairs'])
 target_area+=tr[2]*tr[3]
 if min(fractions)>=.9:ordinal_eligible_area+=e['directions'][0]['valid_sites']
 summary.append({'model_index':index,'fractions':fractions,'directions':directions});print(index,'independent full-cell ordinal parity',flush=True)
assert target_area==240000 and all(digest(k)==v for k,v in r['input_hashes'].items()) and all(digest(k)==v for k,v in pixel_hashes.items())
output=base/'dedup-folded-regional-rank-pixel-audit.json';assert not output.exists();paths=[report_path,geometry_path,dump_path,Path(__file__),oracle_helper,math_helper]
output.write_text(json.dumps({'status':'verified_full_cell_rank_pixel_math','models':8,'directions':16,'target_cell_area':target_area,'query_area':b.shape[0]*b.shape[1],'ordinal_eligible_target_sites_at_fraction_point9':ordinal_eligible_area,'summary':summary,'input_hashes':{str(p):digest(p) for p in paths},'pixel_hashes':pixel_hashes,'scope':'Eight unchanged native4x4 models, full fitting-cell membership and independent luminance/warp/window/ordinal counts. Target cells cover half the query but none admits both directions at.9; no copy recovery or automatic piecewise search. Source bbox/read counts only bounded here; shared native decoding, not independent normalization.'},indent=2)+'\n')
