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
images={k:ns['box'](v,3) for k,v in images.items()}
path=Path('/Users/themoretheless/.codex/tmp/rrrah-dedup-copydays/folded-native-mesh-grid-128m-forward.xy64');pins[str(path)]=digest(path);magic=b'RRRAH-MESH-XY64-V1\n';coords=np.memmap(path,mode='r',dtype='<f8',offset=len(magic)+8,shape=(600,800,2))
centers=np.array([(dx,dy) for dy in range(-12,13,3) for dx in range(-12,13,3)]);neighbors=np.array([[0,0],[-6,0],[6,0],[0,-6],[0,6]]);sites=centers[:,None,:]+neighbors[None,:,:];train=np.array([(i//9+i%9)%2==0 for i in range(81)])
offsets=np.array([(dx,dy) for dy in range(-12,13,2) for dx in range(-12,13,2)]);zero=int(np.flatnonzero(np.all(offsets==0,axis=1))[0]);rows=[]
for name in ['201700','201900']:
 source=images[name];target=images['201702']
 for y in range(32,568,32):
  for x in range(32,768,32):
   tx=sites[:,:,0]+x;ty=sites[:,:,1]+y;mapped=coords[ty,tx]
   if not np.all(np.isfinite(mapped)):continue
   sx=mapped[:,:,0][None,:,:]+offsets[:,None,None,0]-3;sy=mapped[:,:,1][None,:,:]+offsets[:,None,None,1]-3
   if np.min(sx)<0 or np.min(sy)<0 or np.max(sx)+1>=source.shape[1] or np.max(sy)+1>=source.shape[0]:continue
   ix=np.floor(sx).astype(int);iy=np.floor(sy).astype(int);a=sx-ix;b=sy-iy
   values=(source[iy,ix]*(1-a)+source[iy,ix+1]*a)*(1-b)+(source[iy+1,ix]*(1-a)+source[iy+1,ix+1]*a)*b
   target_values=target[ty-3,tx-3];sd=values[:,:,1:]-values[:,:,:1];td=target_values[:,1:]-target_values[:,:1];informative=np.abs(td)>=.005
   ntrain=int(informative[train].sum());nheld=int(informative[~train].sum())
   if ntrain<64 or nheld<64:continue
   # Fixed target-defined support: weak source contrast counts as disagreement,
   # so candidates cannot improve scores by dropping difficult comparisons.
   agreement=(np.abs(sd)>=.005)&((sd>0)==(td[None,:,:]>0))&informative[None,:,:]
   scores=agreement[:,train,:].sum(axis=(1,2))/ntrain;best=int(np.argmax(scores));held=float(agreement[best,~train,:].sum()/nheld)
   rows.append({'case':name,'target':[x,y],'initial_source':coords[y,x].tolist(),'offset':offsets[best].tolist(),'source':(coords[y,x]+offsets[best]).tolist(),'training_pairs':ntrain,'heldout_pairs':nheld,'training_fraction':float(scores[best]),'heldout_fraction':held,'baseline_training_fraction':float(scores[zero]),'baseline_heldout_fraction':float(agreement[zero,~train,:].sum()/nheld),'eligible_proposal':bool(scores[best]>=.9 and held>=.9)})
summary={name:{'seeds':sum(v['case']==name for v in rows),'eligible_proposals':sum(v['case']==name and v['eligible_proposal'] for v in rows)} for name in ['201700','201900']}
pins[str(Path(__file__))]=digest(__file__);assert all(digest(k)==v for k,v in pins.items())
(base/'dedup-folded-dense-seed-diagnostic.json').write_text(json.dumps({'status':'complete_offline_disjoint_site_translation_proposals','policy':{'seed_stride':32,'source_offset_radius':12,'offset_step':2,'source_axis_box_radius':3,'target_axis_box_radius':3,'contrast':.005,'minimum_pairs_per_partition':64,'proposal_fraction':.9,'offsets_per_seed':169},'summary':summary,'rows':rows,'pins':pins,'scope':'Offline geometry proposals only, training selects offset and disjoint centers assess heldout score. Neighbor/filter pixels can overlap between partitions; not independent spatial holdout. Different filter footprints from final whole-mesh ordinal gate. No native proposal API, mesh conformity, final pixel validation or copy admission.'},indent=2)+'\n');print(json.dumps(summary,indent=2))
