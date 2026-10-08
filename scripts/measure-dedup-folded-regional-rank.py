"""Eight native4x4 fits, evidence over their full fitting cells, not copy admission."""
import hashlib,json,math,shutil,subprocess
from pathlib import Path
from dedup_jpeg_domain import oriented_dimensions
base=Path('docs/research');root=Path('/Users/themoretheless/.codex/tmp/rrrah-dedup-copydays');digest=lambda p:hashlib.sha256(Path(p).read_bytes()).hexdigest()
geometry_path=base/'dedup-folded-201702-regional-geometry.json';geometry=json.loads(geometry_path.read_text());assert geometry['status']=='complete_native_local_geometry'
audit_path=base/'dedup-folded-201702-regional-geometry-audit.json';audit=json.loads(audit_path.read_text());assert audit['status']=='verified_folded_crop_local_geometry' and audit['input_hashes'][str(geometry_path)]==digest(geometry_path)
regions=next(v['evidence']['regions'] for v in geometry['results'] if v['divisions']==4);models=[v for v in regions if v['matrix'] is not None];assert len(models)==8
for m in models:assert len(m['inliers'])>=10
exe=root/'regional-rank-region-probe';assert not exe.exists();shutil.copy2('target/debug/examples/regional_rank_region_probe',exe)
left=root/'original-resolution-negative-200201/201700.jpg';right=root/'original-resolution-strong-all/201702.jpg';prepared=root/'prepared.json';manifest=json.loads(prepared.read_text());sizes=[]
for path,split in [(left,'original'),(right,'strong')]:
 item=next(v for v in manifest['images'][split] if v['filename']==path.name);assert digest(path)==item['source_sha256'];sizes.append(oriented_dimensions(path,item['source_size']))
paths=[geometry_path,audit_path,exe,left,right,prepared,Path(__file__),Path(__file__).with_name('dedup_jpeg_domain.py'),Path('crates/rrrah-dedup/examples/regional_rank_region_probe.rs'),Path('crates/rrrah-dedup/src/rank_region.rs'),Path('crates/rrrah-dedup/src/geometry.rs'),Path('crates/rrrah-dedup/src/lib.rs')];pins={str(p):digest(p) for p in paths}
output=base/'dedup-folded-regional-rank.json';assert not output.exists();report={'status':'running','required_models':8,'input_hashes':pins,'rows':[],'scope':'Same eight native4x4 geometric models, full disjoint fitting-cell target domains, clipped mapped source bounds. Explicit neighbor/filter8, contrast.005/minpairs1000, two-million-site/read work caps. Records ordinal evidence, not automatic regional search or copy decisions.'}
def save():
 temporary=output.with_suffix('.tmp');temporary.write_text(json.dumps(report,indent=2)+'\n');temporary.replace(output)
save()
for index,model in enumerate(models):
 h=model['matrix'];scale=max(abs(v) for axis in h for v in axis);m=[[v/scale for v in axis] for axis in h];a,b,c=m[0];d,e,f=m[1];g,j,k=m[2];inverse=[[e*k-f*j,c*j-b*k,b*f-c*e],[f*g-d*k,a*k-c*g,c*d-a*f],[d*j-e*g,b*g-a*j,a*e-b*d]]
 target=list(map(int,model['target_domain']));x,y,w,ht=target;corners=[(x,y),(x+w-1,y),(x,y+ht-1),(x+w-1,y+ht-1)];denominators=[inverse[2][0]*px+inverse[2][1]*py+inverse[2][2] for px,py in corners]
 row={'model_index':index,'matrix':h,'target_domain':target,'native_inliers':model['inliers']};report['rows'].append(row)
 if not(min(denominators)>0 or max(denominators)<0):row['status']='refusal:projective_horizon';save();continue
 mapped=[[(inverse[i][0]*px+inverse[i][1]*py+inverse[i][2])/den for i in range(2)] for (px,py),den in zip(corners,denominators)]
 lo=[max(0,min(sizes[0][i],math.floor(min(v[i] for v in mapped)))) for i in range(2)];hi=[max(0,min(sizes[0][i],math.ceil(max(v[i] for v in mapped))+1)) for i in range(2)];source=[*lo,hi[0]-lo[0],hi[1]-lo[1]];row['source_domain']=source
 if not all(source[i]>0 for i in [2,3]):row['status']='refusal:empty_mapped_domain';save();continue
 path=root/f'folded-regional-rank-{index}.txt';assert not path.exists();path.write_text(' '.join(map(str,[*[v for axis in h for v in axis],*source,*target]))+'\n');pin=digest(path)
 result=subprocess.run([str(exe),str(left),str(right),str(path),'8'],capture_output=True,text=True);assert digest(path)==pin and all(digest(k)==v for k,v in pins.items())
 row.update(input=str(path),input_sha256=pin,returncode=result.returncode,stdout=result.stdout,stderr=result.stderr);save();assert result.returncode==0
 row['evidence']=json.loads(result.stdout);assert row['evidence']['managed_used']==0;row['status']='measured';save();print(index,source,target,row['evidence']['directions'],flush=True)
report['status']='complete_native_regional_rank_diagnostic';save()
