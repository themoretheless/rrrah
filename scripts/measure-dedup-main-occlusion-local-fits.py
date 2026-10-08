"""Fit local geometry on five highest-ranked sufficiently populated regions per pair."""
import hashlib,json,subprocess
from pathlib import Path
D=Path('docs/research');T=Path('/Users/themoretheless/.codex/tmp/rrrah-dedup-copydays');h=lambda p:hashlib.sha256(Path(p).read_bytes()).hexdigest()
base=D/'dedup-main-photometric-misses-radius8.json';rank=D/'dedup-main-occlusion-all-rank-regions.json';b=json.loads(base.read_text());r=json.loads(rank.read_text());geometry=T/'geometry-domain-miss-probe-qualified';pixels=T/'projective-rank-regions-probe-qualified';out=D/'dedup-main-occlusion-local-fits.json';assert not out.exists()
report={'status':'running','results':[],'input_hashes':{str(p):h(p) for p in [base,rank,geometry,pixels,Path(__file__)]},'scope':'Five highest rank8 eligible existing regions having10..42 original correspondences per pair. Exhaustive local model, min10 tolerance2, full image-domain eligibility. Diagnostic only; selection reuses pixels, not held-out validation or copy admission.'}
def save():out.write_text(json.dumps(report,indent=2)+'\n')
def inside(p,r):x,y,w,height=r;return x<=p[0]<x+w and y<=p[1]<y+height
for q in ['214402.jpg','212602.jpg']:
 old=next(v for v in b['results'] if v['query']==q);e=old['evidence'];candidates=[]
 for row in r['results']:
  if row['query']!=q or row['returncode']:continue
  ids=[i for i,point in enumerate(e['correspondences']) if all(inside(p,rect) for p,rect in zip(point,row['domains']))]
  ds=[x for x in row['rank'] if x['filter_radius']==8]
  if 10<=len(ids)<=42 and all(x['informative_pairs']>=1000 and x['valid_sites']/x['sites']>=.3 for x in ds):candidates.append((min(x['agreeing_pairs']/x['informative_pairs'] for x in ds),row['region_index'],ids,row))
 pixelpaths=[T/'rank-normalized-pixel-oracle'/('occlusion-'+Path(name).stem+'.rgba32') for name in [old['original'],q]];dims=[]
 for p in pixelpaths:
  report['input_hashes'][str(p)]=h(p)
  with p.open('rb') as f:
   assert f.read(21)==b'RRRAH-RANK-RGBA32-V1\n';dims.extend([int.from_bytes(f.read(4),'little'),int.from_bytes(f.read(4),'little')])
 for fraction,index,ids,row in sorted(candidates,key=lambda x:(x[0],x[1]),reverse=True)[:5]:
  pointfile=T/f'occlusion-local-{Path(q).stem}-{index}-points.txt';assert not pointfile.exists();pointfile.write_text('\n'.join(' '.join(map(str,point[0]+point[1])) for point in [e['correspondences'][i] for i in ids])+'\n');report['input_hashes'][str(pointfile)]=h(pointfile)
  p=subprocess.run([str(geometry),str(pointfile),*map(str,dims)],capture_output=True,text=True)
  result={'query':q,'region_index':index,'domains':row['domains'],'original_point_indices':ids,'dimensions':dims,'baseline_rank8_fraction':fraction,'geometry_returncode':p.returncode,'geometry_stdout':p.stdout,'geometry_stderr':p.stderr}
  if p.returncode==0:
   fit=json.loads(p.stdout);result['geometry']=fit
   if fit['status']=='model':
    model=T/f'occlusion-local-{Path(q).stem}-{index}-model.txt';assert not model.exists();values=[v for part in fit['matrix'] for v in part]+[v for rect in row['domains'] for v in rect];model.write_text(' '.join(map(str,values))+'\n');report['input_hashes'][str(model)]=h(model)
    p=subprocess.run([str(pixels),*map(str,pixelpaths),str(model)],capture_output=True,text=True);result.update(rank_returncode=p.returncode,rank_stdout=p.stdout,rank_stderr=p.stderr)
    if p.returncode==0:result['rank']=[json.loads(x) for x in p.stdout.splitlines()]
  report['results'].append(result);save()
assert all(h(p)==v for p,v in report['input_hashes'].items());report['status']='complete';save()
