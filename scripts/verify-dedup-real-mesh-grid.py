"""Independent affine-system and half-plane oracle for every real atlas site."""
import hashlib,json
from pathlib import Path
import numpy as np
root=Path(__file__).resolve().parents[1];base=root/'docs/research'
def digest(p):return hashlib.sha256(Path(p).read_bytes()).hexdigest()
landmarks=base/'dedup-folded-qualified-landmarks.json';topology=base/'dedup-folded-auto-diagonal-triangulation.json';summary=base/'dedup-folded-native-mesh-grid-128m.jsonl'
p=json.loads(landmarks.read_text())['rows'];mesh=json.loads(topology.read_text());rows=[json.loads(v) for v in summary.read_text().splitlines()];assert len(rows)==2
pins={str(path):digest(path) for path in [landmarks,topology,summary,Path(__file__),root/'crates/rrrah-dedup/src/mesh_grid.rs',root/'crates/rrrah-dedup/examples/mesh_grid_dump.rs']};reports=[]
for row in rows:
 name=row['direction'];path=Path('/Users/themoretheless/.codex/tmp/rrrah-dedup-copydays')/f'folded-native-mesh-grid-128m-{name}.xy64';pins[str(path)]=digest(path)
 magic=b'RRRAH-MESH-XY64-V1\n'
 with path.open('rb') as f:
  assert f.read(len(magic))==magic;w=int.from_bytes(f.read(4),'little');h=int.from_bytes(f.read(4),'little')
 assert (w,h)==(row['width'],row['height']);assert path.stat().st_size==len(magic)+8+w*h*16
 actual=np.memmap(path,mode='r',dtype='<f8',offset=len(magic)+8,shape=(h,w,2));expected=np.full((h,w,2),np.nan)
 from_key,to_key=('target','source') if name=='forward' else ('source','target')
 for face in mesh['triangles']:
  a=np.array([p[i][from_key] for i in face]);b=np.array([p[i][to_key] for i in face]);basis=np.column_stack((a,np.ones(3)));coefficients=np.linalg.solve(basis,b)
  low=np.maximum(0,np.ceil(a.min(0)-1e-8)).astype(int);high=np.minimum([w,h],np.floor(a.max(0)+1e-8)+1).astype(int)
  yy,xx=np.indices((high[1]-low[1],high[0]-low[0]),dtype='f8');xx+=low[0];yy+=low[1];inside=np.ones(xx.shape,dtype=bool)
  area=(a[1,0]-a[0,0])*(a[2,1]-a[0,1])-(a[1,1]-a[0,1])*(a[2,0]-a[0,0])
  for i in range(3):
   start=a[i];end=a[(i+1)%3];signed=(end[0]-start[0])*(yy-start[1])-(end[1]-start[1])*(xx-start[0]);inside &= signed>=-area*1e-12
  mapped=xx[:,:,None]*coefficients[0]+yy[:,:,None]*coefficients[1]+coefficients[2]
  expected[low[1]:high[1],low[0]:high[0]][inside]=mapped[inside]
 valid=np.isfinite(expected[:,:,0]);native_valid=np.isfinite(actual[:,:,0]);assert np.array_equal(valid,native_valid),(name,int(np.count_nonzero(valid!=native_valid)))
 assert int(valid.sum())==row['covered'];error=float(np.max(np.abs(actual[valid]-expected[valid])));assert error<1e-7,(name,error)
 reports.append({'direction':name,'sites':w*h,'covered':int(valid.sum()),'maximum_coordinate_error':error,'holes_exact':True});print(reports[-1],flush=True)
assert all(digest(k)==v for k,v in pins.items())
(base/'dedup-folded-native-mesh-grid-audit.json').write_text(json.dumps({'status':'verified_independent_real_mesh_grid_coordinates','directions':reports,'pins':pins,'scope':'Every native real atlas hole and coordinate compared with independent half-plane tests and affine linear-system solve. No image resampling/rank/copy evidence.'},indent=2)+'\n')
