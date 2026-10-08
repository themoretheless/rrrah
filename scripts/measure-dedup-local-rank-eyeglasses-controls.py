"""Full candidate geometry plus explicit local-rank policy on one positive and156 origin negatives."""
import ast,hashlib,json,math,subprocess
from pathlib import Path
from dedup_jpeg_domain import oriented_dimensions
D=Path('docs/research');T=Path('/Users/themoretheless/.codex/tmp/rrrah-dedup-copydays');h=lambda p:hashlib.sha256(Path(p).read_bytes()).hexdigest()
manifest=T/'prepared.json';m=json.loads(manifest.read_text());query='214402.jpg';target=T/'original-resolution-strong-all'/query;record=next(v for v in m['images']['strong'] if v['filename']==query);originals=m['images']['original'];positive=[v for v in originals if v['group_id']==record['group_id']];negative=[v for v in originals if v['group_id']!=record['group_id']];assert len(positive)==1 and len(negative)==156
candidate=T/'gradient-scales-probe-screen-filter8';exe=T/'local-rank-resolution-probe-qualified';dump=T/'rank-pixel-dump';target_pixels=T/'rank-normalized-pixel-oracle'/'occlusion-214402.rgba32'
helper=Path('scripts/verify-dedup-rank-region-real.py');nodes=[n for n in ast.parse(helper.read_text()).body if isinstance(n,ast.FunctionDef) and n.name=='inverse'];ns={};exec(compile(ast.Module(body=nodes,type_ignores=[]),str(helper),'exec'),ns);inverse=ns['inverse']
work=T/'local-rank-eyeglasses-controls';work.mkdir(exist_ok=False)
files=[manifest,target,target_pixels,candidate,exe,dump,helper,Path(__file__),Path('scripts/dedup_jpeg_domain.py')]
for v in originals:
 source=T/'original-resolution-negative-200201'/v['filename'];assert h(source)==v['source_sha256'];files.append(source)
assert h(target)==record['source_sha256'];pins={str(p):h(p) for p in files};out=D/'dedup-local-rank-eyeglasses-controls.json';assert not out.exists()
r={'status':'running','required_pairs':157,'required_negatives':156,'query':query,'query_group':record['group_id'],'results':[],'input_hashes':pins,'generated_hashes':{},'scope':'Same frozen candidate recipe, all proposed regions with>=10 original distinct regional points under target2/source2*diagonal ratio. Native local-rank API checks complete original points and filter8 support. Positive followed by156 different-origin negatives; no semantic/burst precision or default promotion.'}
def save():
 tmp=out.with_suffix('.tmp');tmp.write_text(json.dumps(r,indent=2)+'\n');tmp.replace(out)
def inside(p,rect):x,y,w,height=rect;return x<=p[0]<x+w and y<=p[1]<y+height
def apply(m,p):
 q=[sum(row[k]*[*p,1.][k] for k in range(3)) for row in m];return [q[0]/q[2],q[1]/q[2]]
save()
for original in positive+negative:
 source=T/'original-resolution-negative-200201'/original['filename'];assert all(h(p)==pins[str(p)] for p in [source,target,candidate,exe])
 p=subprocess.run([str(candidate),'original-managed-candidate-union-filter8',str(source),str(target)],capture_output=True,text=True)
 row={'original':original['filename'],'original_group':original['group_id'],'positive_control':original['group_id']==record['group_id'],'candidate_returncode':p.returncode,'candidate_stdout':p.stdout,'candidate_stderr':p.stderr,'regions':[]}
 if p.returncode==0:
  e=json.loads(p.stdout);assert e['status']=='ok' and e['managed_used']==0;row['candidate']=e
  if e['matrix'] is not None:
   matrix=e['matrix'];inv=inverse(matrix);sd=oriented_dimensions(source,original['source_size']);td=oriented_dimensions(target,record['source_size']);tol=2*math.hypot(*sd)/math.hypot(*td);eligible=[]
   for index,region in enumerate(e['regions']):
    witnesses=sum(all(inside(point,rect) for point,rect in zip(pair,region['domains'])) and math.dist(apply(matrix,pair[0]),pair[1])<=2 and math.dist(apply(inv,pair[1]),pair[0])<=tol for pair in e['correspondences'])
    if witnesses>=10:eligible.append((index,region,witnesses))
   row['eligible_region_indices']=[v[0] for v in eligible];row['source_tolerance']=tol
   if eligible:
    source_pixels=work/(Path(original['filename']).stem+'.rgba32');p=subprocess.run([str(dump),str(source),str(source_pixels)],capture_output=True,text=True);assert p.returncode==0,p.stderr;r['generated_hashes'][str(source_pixels)]=h(source_pixels)
    points=work/(Path(original['filename']).stem+'-points.txt');points.write_text('\n'.join(' '.join(map(str,p[0]+p[1])) for p in e['correspondences'])+'\n');r['generated_hashes'][str(points)]=h(points)
    for index,region,witnesses in eligible:
     model=work/f"{Path(original['filename']).stem}-{index}-model.txt";values=[x for part in matrix for x in part]+[x for rect in region['domains'] for x in rect];model.write_text(' '.join(map(str,values))+'\n');r['generated_hashes'][str(model)]=h(model)
     run=subprocess.run([str(exe),str(source_pixels),str(target_pixels),str(model),str(points)],capture_output=True,text=True);result={'region_index':index,'prefilter_witnesses':witnesses,'returncode':run.returncode,'stdout':run.stdout,'stderr':run.stderr}
     if run.returncode==0:result['evidence']=[json.loads(x) for x in run.stdout.splitlines()]
     row['regions'].append(result)
  row['local_rank8_supported']=any(x.get('supported',False) for region in row['regions'] for x in region.get('evidence',[]) if x['filter_radius']==8)
 assert all(h(p)==pins[str(p)] for p in [source,target,candidate,exe]);r['results'].append(row);save()
assert all(h(p)==v for p,v in pins.items()) and all(h(p)==v for p,v in r['generated_hashes'].items());r['status']='complete';save()
