"""Offline whole-domain mesh pixel diagnostic; no copy-admission decision."""
import hashlib,json
from pathlib import Path
import numpy as np
base=Path(__file__).resolve().parents[1]/'docs/research'
def digest(p):return hashlib.sha256(Path(p).read_bytes()).hexdigest()
manifest=base/'dedup-rank-normalized-pixels.json'
dumps=json.loads(manifest.read_text())
images={};pins={str(manifest):digest(manifest)}
magic=b'RRRAH-RANK-RGBA32-V1\n'
for row in dumps['rows']:
 stem=Path(row['source']).stem
 if stem not in ['201700','201702','201900']:continue
 path=Path(row['pixels']);assert digest(path)==row['pixels_sha256'];pins[str(path)]=digest(path)
 with path.open('rb') as f:
  assert f.read(len(magic))==magic;w=int.from_bytes(f.read(4),'little');h=int.from_bytes(f.read(4),'little')
 a=np.memmap(path,dtype='<f4',mode='r',offset=len(magic)+8,shape=(h,w,4))
 images[stem]=a[:,:,0].astype('f8')*.2126+a[:,:,1].astype('f8')*.7152+a[:,:,2].astype('f8')*.0722
points_path=base/'dedup-folded-qualified-landmarks.json';mesh_path=base/'dedup-folded-topology-native.json'
p=json.loads(points_path.read_text())['rows'];mesh=json.loads(mesh_path.read_text());assert mesh['source_mesh_status']=='accepted'
for path in [points_path,mesh_path,Path(__file__)]:pins[str(path)]=digest(path)

def warp(source,shape,from_key,to_key):
 output=np.full(shape,np.nan);covered=np.zeros(shape,dtype=bool)
 for face in mesh['triangles']:
  a,b,c=np.array([p[i][from_key] for i in face]);dest=np.array([p[i][to_key] for i in face]);area=(b[0]-a[0])*(c[1]-a[1])-(b[1]-a[1])*(c[0]-a[0])
  x0=max(0,int(np.ceil(min(a[0],b[0],c[0]))));x1=min(shape[1],int(np.floor(max(a[0],b[0],c[0])))+1)
  y0=max(0,int(np.ceil(min(a[1],b[1],c[1]))));y1=min(shape[0],int(np.floor(max(a[1],b[1],c[1])))+1)
  yy,xx=np.indices((y1-y0,x1-x0),dtype='f8');xx+=x0;yy+=y0
  w0=((b[0]-xx)*(c[1]-yy)-(b[1]-yy)*(c[0]-xx))/area
  w1=((c[0]-xx)*(a[1]-yy)-(c[1]-yy)*(a[0]-xx))/area;w2=1-w0-w1
  valid=(w0>=-1e-12)&(w1>=-1e-12)&(w2>=-1e-12)
  sx=w0*dest[0,0]+w1*dest[1,0]+w2*dest[2,0];sy=w0*dest[0,1]+w1*dest[1,1]+w2*dest[2,1]
  valid &= (sx>=0)&(sy>=0)&(sx+1<source.shape[1])&(sy+1<source.shape[0])
  ix=np.clip(np.floor(sx),0,source.shape[1]-2).astype(int);iy=np.clip(np.floor(sy),0,source.shape[0]-2).astype(int);tx=sx-ix;ty=sy-iy
  value=(source[iy,ix]*(1-tx)+source[iy,ix+1]*tx)*(1-ty)+(source[iy+1,ix]*(1-tx)+source[iy+1,ix+1]*tx)*ty
  output[y0:y1,x0:x1][valid]=value[valid];covered[y0:y1,x0:x1]|=valid
 return output,int(covered.sum())

def box(a,r):
 if r==0:return a
 size=2*r+1;safe=np.isfinite(a)
 def sums(v):
  integral=np.pad(v,((1,0),(1,0))).cumsum(0).cumsum(1)
  return integral[size:,size:]-integral[:-size,size:]-integral[size:,:-size]+integral[:-size,:-size]
 means=sums(np.where(safe,a,0.))/(size*size);means[sums(safe.astype('i8'))!=size*size]=np.nan
 return means

def rank(a,b,r):
 a=box(a,r);b=box(b,r);h,w=a.shape
 av=[];bv=[]
 for dx,dy in [(0,0),(-8,-8),(0,-8),(8,-8),(-8,0),(8,0),(-8,8),(0,8),(8,8)]:
  av.append(a[8+dy:h-8+dy,8+dx:w-8+dx]);bv.append(b[8+dy:h-8+dy,8+dx:w-8+dx])
 valid=np.logical_and.reduce([np.isfinite(v) for v in av+bv]);info=agree=0
 for va,vb in zip(av[1:],bv[1:]):
  da=va-av[0];db=vb-bv[0];usable=valid&(np.abs(da)>=.005)&(np.abs(db)>=.005)
  info+=int(usable.sum());agree+=int((usable&((da>0)==(db>0))).sum())
 return {'valid_sites':int(valid.sum()),'informative_pairs':info,'agreeing_pairs':agree,'agreement_fraction':agree/info if info else None}
rows=[]
for name,source,target,from_key,to_key in [('fold_forward',images['201700'],images['201702'],'target','source'),('fold_reverse',images['201702'],images['201700'],'source','target'),('unrelated_forward',images['201900'],images['201702'],'target','source')]:
 warped,covered=warp(source,target.shape,from_key,to_key)
 for radius in [0,3,8]:
  row={'case':name,'radius':radius,'domain_sites':int(target.size),'mesh_covered_sites':covered,**rank(warped,target,radius)};rows.append(row);print(row,flush=True)
assert all(digest(k)==v for k,v in pins.items())
(base/'dedup-folded-piecewise-pixel-diagnostic.json').write_text(json.dumps({'status':'complete_offline_piecewise_pixel_diagnostic','rows':rows,'pins':pins,'scope':'Whole integer-pixel mesh domains with strict complete filter windows, contrast .005 and neighbor spacing8. Offline NumPy only; native parity, retrieval, all-negative precision and copy admission remain pending. Reverse filter uses source axes.'},indent=2)+'\n')
