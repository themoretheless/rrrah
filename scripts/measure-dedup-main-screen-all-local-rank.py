"""All existing screen regions, fixed global model, predeclared normalized residuals."""
import ast,hashlib,json,math,subprocess
from pathlib import Path
D=Path('docs/research');T=Path('/Users/themoretheless/.codex/tmp/rrrah-dedup-copydays');h=lambda p:hashlib.sha256(Path(p).read_bytes()).hexdigest();base=D/'dedup-main-photometric-misses-radius8.json';b=json.loads(base.read_text());e=next(v['evidence'] for v in b['results'] if v['query']=='200501.jpg');exe=T/'projective-rank-regions-probe-qualified';pixels=[T/'rank-normalized-pixel-oracle'/f'{name}.rgba32' for name in ['200500','200501']]
helper=Path('scripts/verify-dedup-rank-region-real.py');ns={};nodes=[n for n in ast.parse(helper.read_text()).body if isinstance(n,ast.FunctionDef) and n.name=='inverse'];exec(compile(ast.Module(body=nodes,type_ignores=[]),str(helper),'exec'),ns);inv=ns['inverse'](e['matrix']);dims=[]
for p in pixels:
 with p.open('rb') as f:assert f.read(21)==b'RRRAH-RANK-RGBA32-V1\n';dims.append([int.from_bytes(f.read(4),'little'),int.from_bytes(f.read(4),'little')])
tol=2*math.hypot(*dims[0])/math.hypot(*dims[1]);out=D/'dedup-main-screen-all-local-rank.json';assert not out.exists();r={'status':'running','required_regions':len(e['regions']),'source_tolerance':tol,'results':[],'input_hashes':{str(p):h(p) for p in [base,exe,helper,Path(__file__),*pixels]},'generated_hashes':{},'scope':'All existing regions of200501, fixed geometry, normalized-diagonal source residual/target2/min10 prefilter, native cached rank. Diagnostic only; no combined native API or copy admission/broad precision.'}
def save():out.write_text(json.dumps(r,indent=2)+'\n')
def inside(p,rect):x,y,w,height=rect;return x<=p[0]<x+w and y<=p[1]<y+height
def apply(m,p):
 q=[sum(row[k]*[*p,1.][k] for k in range(3)) for row in m];return [q[0]/q[2],q[1]/q[2]]
save()
for index,region in enumerate(e['regions']):
 ids=[i for i,pair in enumerate(e['correspondences']) if all(inside(p,rect) for p,rect in zip(pair,region['domains'])) and math.dist(apply(e['matrix'],pair[0]),pair[1])<=2 and math.dist(apply(inv,pair[1]),pair[0])<=tol]
 row={'region_index':index,'domains':region['domains'],'witness_indices':ids}
 if len(ids)>=10:
  model=T/f'all-screen-local-rank-{index}.txt';assert not model.exists();values=[x for part in e['matrix'] for x in part]+[x for rect in region['domains'] for x in rect];model.write_text(' '.join(map(str,values))+'\n');r['generated_hashes'][str(model)]=h(model)
  p=subprocess.run([str(exe),*map(str,pixels),str(model)],capture_output=True,text=True);row.update(returncode=p.returncode,stdout=p.stdout,stderr=p.stderr)
  if p.returncode==0:row['rank']=[json.loads(x) for x in p.stdout.splitlines()]
 r['results'].append(row);save()
assert all(h(p)==v for p,v in r['input_hashes'].items());r['status']='complete';save()
