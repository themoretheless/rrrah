"""Fixed-mesh per-face residual localization; no refit or copy admission."""
import ast,hashlib,json
from pathlib import Path
import numpy as np
root=Path(__file__).resolve().parents[1];base=root/'docs/research'
def digest(p):return hashlib.sha256(Path(p).read_bytes()).hexdigest()
manifest=base/'dedup-rank-normalized-pixels.json';d=json.loads(manifest.read_text());images={};pins={str(manifest):digest(manifest)};magic=b'RRRAH-RANK-RGBA32-V1\n'
for stem in ['201700','201702']:
 row=next(v for v in d['rows'] if Path(v['source']).stem==stem);path=Path(row['pixels']);assert digest(path)==row['pixels_sha256'];pins[str(path)]=digest(path)
 with path.open('rb') as f:
  assert f.read(len(magic))==magic;w=int.from_bytes(f.read(4),'little');h=int.from_bytes(f.read(4),'little')
 a=np.memmap(path,mode='r',dtype='<f4',offset=len(magic)+8,shape=(h,w,4));images[stem]=a[:,:,0].astype('f8')*.2126+a[:,:,1].astype('f8')*.7152+a[:,:,2].astype('f8')*.0722
path=Path('/Users/themoretheless/.codex/tmp/rrrah-dedup-copydays/folded-native-mesh-grid-128m-forward.xy64');pins[str(path)]=digest(path);magic=b'RRRAH-MESH-XY64-V1\n';coords=np.memmap(path,mode='r',dtype='<f8',offset=len(magic)+8,shape=(600,800,2));source=images['201700'];target=images['201702'];sx=coords[:,:,0];sy=coords[:,:,1]
valid=np.isfinite(sx)&np.isfinite(sy)&(sx>=0)&(sy>=0)&(sx+1<source.shape[1])&(sy+1<source.shape[0]);ix=np.floor(np.nan_to_num(sx)).astype(int);iy=np.floor(np.nan_to_num(sy)).astype(int);ix=np.clip(ix,0,source.shape[1]-2);iy=np.clip(iy,0,source.shape[0]-2);tx=sx-ix;ty=sy-iy
warped=(source[iy,ix]*(1-tx)+source[iy,ix+1]*tx)*(1-ty)+(source[iy+1,ix]*(1-tx)+source[iy+1,ix+1]*tx)*ty;warped[~valid]=np.nan
helper=root/'scripts/measure-dedup-piecewise-pixels.py';tree=ast.parse(helper.read_text());nodes=[n for n in tree.body if isinstance(n,ast.FunctionDef) and n.name=='box'];ns={'np':np};exec(compile(ast.Module(body=nodes,type_ignores=[]),str(helper),'exec'),ns);pins[str(helper)]=digest(helper)
a=ns['box'](warped,8);b=ns['box'](target,8);h,w=a.shape;av=[];bv=[]
for dx,dy in [(0,0),(-8,-8),(0,-8),(8,-8),(-8,0),(8,0),(-8,8),(0,8),(8,8)]:
 av.append(a[8+dy:h-8+dy,8+dx:w-8+dx]);bv.append(b[8+dy:h-8+dy,8+dx:w-8+dx])
valid=np.logical_and.reduce([np.isfinite(v) for v in av+bv]);info=np.zeros(valid.shape,dtype='u1');agree=np.zeros(valid.shape,dtype='u1')
for va,vb in zip(av[1:],bv[1:]):
 da=va-av[0];db=vb-bv[0];usable=valid&(np.abs(da)>=.005)&(np.abs(db)>=.005);info+=usable.astype('u1');agree+=(usable&((da>0)==(db>0))).astype('u1')
reference=base/'dedup-folded-native-mesh-rank-audit.json';r=json.loads(reference.read_text());e=next(v for v in r['summary'] if v['direction']=='forward' and v['filter_radius']==8);assert int(valid.sum())==e['valid_sites'];assert int(info.sum())==e['informative_pairs'];assert int(agree.sum())==e['agreeing_pairs']
landmarks=base/'dedup-folded-qualified-landmarks.json';topology=base/'dedup-folded-auto-diagonal-triangulation.json';p=json.loads(landmarks.read_text())['rows'];faces=json.loads(topology.read_text())['triangles'];owners=np.full(valid.shape,-1,dtype='i4');yy,xx=np.indices(valid.shape,dtype='f8');xx+=16;yy+=16;rows=[]
for i,face in enumerate(faces):
 a,b,c=np.array([p[v]['target'] for v in face]);area=(b[0]-a[0])*(c[1]-a[1])-(b[1]-a[1])*(c[0]-a[0]);inside=np.ones(valid.shape,dtype=bool)
 for start,end in [(a,b),(b,c),(c,a)]:inside&=(end[0]-start[0])*(yy-start[1])-(end[1]-start[1])*(xx-start[0])>=-area*1e-12
 # Shared edges assigned deterministically once to avoid duplicated counts.
 owners[(owners<0)&inside]=i
 edge=max(float(np.linalg.norm(v-u)) for u,v in [(a,b),(b,c),(c,a)]);rows.append({'face':i,'indices':face,'target_area':float(area/2),'maximum_edge':edge})
assert np.all(owners[valid]>=0)
for i,row in enumerate(rows):
 mask=(owners==i)&valid;row['valid_sites']=int(mask.sum());row['informative_pairs']=int(info[mask].sum());row['agreeing_pairs']=int(agree[mask].sum());row['fraction']=row['agreeing_pairs']/row['informative_pairs'] if row['informative_pairs'] else None
assert sum(v['informative_pairs'] for v in rows)==e['informative_pairs'];assert sum(v['agreeing_pairs'] for v in rows)==e['agreeing_pairs']
quartiles=[];order=sorted(rows,key=lambda v:v['maximum_edge'])
for i in range(4):
 group=order[i*len(order)//4:(i+1)*len(order)//4];n=sum(v['informative_pairs'] for v in group);k=sum(v['agreeing_pairs'] for v in group);quartiles.append({'edge_quartile':i,'maximum_edge_range':[group[0]['maximum_edge'],group[-1]['maximum_edge']],'valid_sites':sum(v['valid_sites'] for v in group),'informative_pairs':n,'agreement_fraction':k/n if n else None})
for path in [reference,landmarks,topology,Path(__file__)]:pins[str(path)]=digest(path)
assert all(digest(k)==v for k,v in pins.items())
(base/'dedup-folded-mesh-face-rank.json').write_text(json.dumps({'status':'complete_fixed_mesh_face_rank_diagnostic','filter_radius':8,'neighbor_radius':8,'faces':rows,'edge_quartiles':quartiles,'pins':pins,'scope':'Fixed geometry, full supported windows, per-face forward-only counts partition native total exactly. No fitting, pruning, threshold changes, classifier or causal density claim.'},indent=2)+'\n');print(json.dumps(quartiles,indent=2))
