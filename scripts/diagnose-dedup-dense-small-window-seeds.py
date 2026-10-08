"""Bounded offline local translation proposals with disjoint ordinal sites."""
import ast,json,hashlib
from pathlib import Path
import numpy as np
root=Path(__file__).resolve().parents[1];base=root/'docs/research'
def digest(p):return hashlib.sha256(Path(p).read_bytes()).hexdigest()
manifest=base/'dedup-rank-normalized-pixels.json';d=json.loads(manifest.read_text());images={};pins={str(manifest):digest(manifest)};magic=b'RRRAH-RANK-RGBA32-V1\n'
for stem in ['201700','201702','201900']:
 row=next(v for v in d['rows'] if Path(v['source']).stem==stem);path=Path(row['pixels']);assert digest(path)==row['pixels_sha256'];pins[str(path)]=digest(path)
 with path.open('rb') as f:
  assert f.read(len(magic))==magic;w=int.from_bytes(f.read(4),'little');h=int.from_bytes(f.read(4),'little')
 a=np.memmap(path,mode='r',dtype='<f4',offset=len(magic)+8,shape=(h,w,4));images[stem]=a[:,:,0].astype('f8')*.2126+a[:,:,1].astype('f8')*.7152+a[:,:,2].astype('f8')*.0722
helper=root/'scripts/measure-dedup-piecewise-pixels.py';tree=ast.parse(helper.read_text());nodes=[n for n in tree.body if isinstance(n,ast.FunctionDef) and n.name=='box'];ns={'np':np};exec(compile(ast.Module(body=nodes,type_ignores=[]),str(helper),'exec'),ns);pins[str(helper)]=digest(helper)
path=Path('/Users/themoretheless/.codex/tmp/rrrah-dedup-copydays/folded-native-mesh-grid-128m-forward.xy64');pins[str(path)]=digest(path);magic=b'RRRAH-MESH-XY64-V1\n';coords=np.memmap(path,mode='r',dtype='<f8',offset=len(magic)+8,shape=(600,800,2))
centers=np.array([(dx,dy) for dy in range(-8,9,2) for dx in range(-8,9,2)]);neighbors=np.array([[0,0],[-4,0],[4,0],[0,-4],[0,4]]);sites=centers[:,None,:]+neighbors[None,:,:];train=np.array([(i//9+i%9)%2==0 for i in range(81)])
offsets=np.array([(dx,dy) for dy in range(-24,25,2) for dx in range(-24,25,2)]);zero=int(np.flatnonzero(np.all(offsets==0,axis=1))[0]);rows=[]
for name in ['201700','201900']:
 source=images[name];target=ns['box'](images['201702'],3);sx0=coords[:,:,0];sy0=coords[:,:,1]
 safe=np.isfinite(sx0)&np.isfinite(sy0)&(sx0>=24)&(sy0>=24)&(sx0+25<source.shape[1])&(sy0+25<source.shape[0])
 # A common support mask covers every candidate offset and filter tap.
 safe_windows=np.isfinite(ns['box'](np.where(safe,0.,np.nan),3))
 seeds=[]
 for y in range(32,568,16):
  for x in range(32,768,16):
   tx=sites[:,:,0]+x;ty=sites[:,:,1]+y
   if not np.all(safe_windows[ty-3,tx-3]):continue
   target_values=target[ty-3,tx-3];td=target_values[:,1:]-target_values[:,:1];informative=np.abs(td)>=.005
   ntrain=int(informative[train].sum());nheld=int(informative[~train].sum())
   if ntrain<64 or nheld<64:continue
   seeds.append((x,y,tx,ty,td,informative,ntrain,nheld))
 train_scores=np.zeros((len(offsets),len(seeds)));held_scores=np.zeros_like(train_scores)
 for oi,(dx,dy) in enumerate(offsets):
  sx=np.nan_to_num(sx0)+dx;sy=np.nan_to_num(sy0)+dy;ix=np.clip(np.floor(sx),0,source.shape[1]-2).astype(int);iy=np.clip(np.floor(sy),0,source.shape[0]-2).astype(int);a=sx-ix;b=sy-iy
  warped=(source[iy,ix]*(1-a)+source[iy,ix+1]*a)*(1-b)+(source[iy+1,ix]*(1-a)+source[iy+1,ix+1]*a)*b;warped[~safe]=np.nan
  filtered=ns['box'](warped,3)
  for si,(x,y,tx,ty,td,informative,ntrain,nheld) in enumerate(seeds):
   values=filtered[ty-3,tx-3];assert np.all(np.isfinite(values));sd=values[:,1:]-values[:,:1];agreement=(np.abs(sd)>=.005)&((sd>0)==(td>0))&informative
   train_scores[oi,si]=agreement[train].sum()/ntrain;held_scores[oi,si]=agreement[~train].sum()/nheld
 for si,(x,y,tx,ty,td,informative,ntrain,nheld) in enumerate(seeds):
  best=int(np.argmax(train_scores[:,si]));score=float(train_scores[best,si]);held=float(held_scores[best,si])
  rows.append({'case':name,'target':[x,y],'initial_source':coords[y,x].tolist(),'offset':offsets[best].tolist(),'source':(coords[y,x]+offsets[best]).tolist(),'training_pairs':ntrain,'heldout_pairs':nheld,'training_fraction':score,'heldout_fraction':held,'baseline_training_fraction':float(train_scores[zero,si]),'baseline_heldout_fraction':float(held_scores[zero,si]),'eligible_proposal':bool(score>=.9 and held>=.9)})
 print(name,'finished',len(seeds),'seeds',flush=True)
summary={name:{'seeds':sum(v['case']==name for v in rows),'eligible_proposals':sum(v['case']==name and v['eligible_proposal'] for v in rows)} for name in ['201700','201900']}
pins[str(Path(__file__))]=digest(__file__);assert all(digest(k)==v for k,v in pins.items())
(base/'dedup-folded-dense-small-window-seed-diagnostic.json').write_text(json.dumps({'status':'complete_offline_disjoint_site_translation_proposals','policy':{'seed_stride':16,'center_extent':8,'center_step':2,'neighbor_radius':4,'source_offset_radius':24,'offset_step':2,'source_target_axis_box_radius':3,'target_axis_box_radius':3,'contrast':.005,'minimum_pairs_per_partition':64,'proposal_fraction':.9,'offsets_per_seed':625},'summary':summary,'rows':rows,'pins':pins,'scope':'Offline geometry proposals only, training selects offset and disjoint centers assess heldout score. Neighbor/filter pixels can overlap between partitions; not independent spatial holdout. Common target-axis filter windows at every candidate offset. Fixed worst-offset support prevents support changes. No native proposal API, mesh conformity, final pixel validation or copy admission.'},indent=2)+'\n');print(json.dumps(summary,indent=2))
