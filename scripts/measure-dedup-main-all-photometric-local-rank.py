"""All16 photometric misses through explicit cached combined local-rank API."""
import ast,hashlib,json,math,subprocess
from pathlib import Path
from dedup_jpeg_domain import oriented_dimensions
D=Path('docs/research');T=Path('/Users/themoretheless/.codex/tmp/rrrah-dedup-copydays');h=lambda p:hashlib.sha256(Path(p).read_bytes()).hexdigest()
analysis=D/'dedup-main-problem-refusal-analysis.json';baseline=D/'dedup-combined-domain-release-original-full.json';manifest=T/'prepared.json';cases=[v for v in json.loads(analysis.read_text())['rows'] if v['reason']=='photometric'];assert len(cases)==16;b=json.loads(baseline.read_text());m=json.loads(manifest.read_text());exe=T/'local-rank-cached-pixels-probe-qualified';dump=T/'rank-pixel-dump';helper=Path('scripts/verify-dedup-rank-region-real.py');ns={};nodes=[n for n in ast.parse(helper.read_text()).body if isinstance(n,ast.FunctionDef) and n.name=='inverse'];exec(compile(ast.Module(body=nodes,type_ignores=[]),str(helper),'exec'),ns)
work=T/'all-photometric-local-rank';work.mkdir(exist_ok=False);output=D/'dedup-main-all-photometric-local-rank.json';assert not output.exists();files=[analysis,baseline,manifest,exe,dump,helper,Path(__file__),Path('scripts/dedup_jpeg_domain.py')];inputs=[]
for case in cases:
 paths=[T/'original-resolution-negative-200201'/case['original'],T/'original-resolution-strong-all'/case['query']];files.extend(paths);dims=[]
 for p,split,name in zip(paths,['original','strong'],[case['original'],case['query']]):
  v=next(x for x in m['images'][split] if x['filename']==name);assert h(p)==v['source_sha256'];dims.append(oriented_dimensions(p,v['source_size']))
 inputs.append((case,paths,dims))
r={'status':'running','required_cases':16,'results':[],'input_hashes':{str(p):h(p) for p in files},'generated_hashes':{},'scope':'All original16 photometric misses; fixed baseline supplied correspondences/model/proposed regions, predeclared source2*diagonal ratio/target2/min10 and cached actual native API. Filter8 local support only; not file identity, candidate retrieval rerun, collection or broad precision.'}
def save():
 tmp=output.with_suffix('.tmp');tmp.write_text(json.dumps(r,indent=2)+'\n');tmp.replace(output)
def inside(p,rect):x,y,w,height=rect;return x<=p[0]<x+w and y<=p[1]<y+height
def apply(m,p):
 q=[sum(row[k]*[*p,1.][k] for k in range(3)) for row in m];return [q[0]/q[2],q[1]/q[2]]
prepared_pixels={}
save()
for case,paths,dims in inputs:
 e=next(v['evidence'] for v in b['results'] if v['query']==case['query']);assert e['matrix'] is not None;inverse=ns['inverse'](e['matrix']);tol=2*math.hypot(*dims[0])/math.hypot(*dims[1]);eligible=[];witness_counts=[]
 for index,region in enumerate(e['regions']):
  ids=[i for i,pair in enumerate(e['correspondences']) if all(inside(p,rect) for p,rect in zip(pair,region['domains'])) and math.dist(apply(e['matrix'],pair[0]),pair[1])<=2 and math.dist(apply(inverse,pair[1]),pair[0])<=tol];witness_counts.append(len(ids))
  if len(ids)>=10:eligible.append((index,region,ids))
 row={'original':case['original'],'query':case['query'],'dimensions':dims,'source_tolerance':tol,'regional_witness_counts':witness_counts,'regions':[]}
 if eligible:
  pixels=[]
  for path in paths:
   if path not in prepared_pixels:
    p=work/(path.stem+'.rgba32');run=subprocess.run([str(dump),str(path),str(p)],capture_output=True,text=True);assert run.returncode==0,run.stderr;assert json.loads(run.stdout)['managed_used']==0;r['generated_hashes'][str(p)]=h(p);prepared_pixels[path]=p
   pixels.append(prepared_pixels[path])
  points=work/(Path(case['query']).stem+'-points.txt');points.write_text('\n'.join(' '.join(map(str,p[0]+p[1])) for p in e['correspondences'])+'\n');r['generated_hashes'][str(points)]=h(points)
  for index,region,ids in eligible:
   model=work/f"{Path(case['query']).stem}-{index}-model.txt";values=[v for part in e['matrix'] for v in part]+[v for rect in region['domains'] for v in rect];model.write_text(' '.join(map(str,values))+'\n');r['generated_hashes'][str(model)]=h(model)
   run=subprocess.run([str(exe),*map(str,pixels),str(model),str(points)],capture_output=True,text=True);entry={'region_index':index,'witness_indices':ids,'returncode':run.returncode,'stdout':run.stdout,'stderr':run.stderr}
   if run.returncode==0:entry['evidence']=[json.loads(x) for x in run.stdout.splitlines()]
   row['regions'].append(entry)
 row['filter8_local_supported_regions']=sum(any(v['filter_radius']==8 and v.get('supported',False) for v in entry.get('evidence',[])) for entry in row['regions']);r['results'].append(row);save();print(case['query'],row['filter8_local_supported_regions'],len(eligible),flush=True)
assert all(h(p)==v for p,v in r['input_hashes'].items()) and all(h(p)==v for p,v in r['generated_hashes'].items());r['status']='complete';save()
