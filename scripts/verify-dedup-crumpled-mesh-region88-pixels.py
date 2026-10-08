"""Independent triangle solves plus prior independent box/ordinal pixel math."""
import ast,hashlib,json
from pathlib import Path
import numpy as np
D=Path('docs/research');T=Path('/Users/themoretheless/.codex/tmp/rrrah-dedup-copydays');h=lambda p:hashlib.sha256(Path(p).read_bytes()).hexdigest();pointsfile=T/'remaining-mesh/204702-global_inliers-points.txt';points=np.array([list(map(float,x.split())) for x in pointsfile.read_text().splitlines()]);assert points.shape==(52,4);facefile=D/'dedup-crumpled-mesh-region88-faces.json';faces=json.loads(facefile.read_text())['faces'];assert all(len(set(f))==3 and all(0<=i<52 for i in f) for f in faces)
helper=Path('scripts/verify-dedup-rank-pixel-oracle.py');node=next(x for x in ast.parse(helper.read_text()).body if isinstance(x,ast.FunctionDef) and x.name=='measured')
def coordinates(mapping,x,y):
 domain,values=mapping;ox=np.full(x.shape,-1.);oy=np.full(y.shape,-1.);assigned=np.zeros(x.shape,dtype=bool)
 for face in faces:
  tri=domain[face];out=values[face];matrix=np.column_stack([tri[1]-tri[0],tri[2]-tri[0]]);coeff=np.linalg.solve(matrix,np.stack([(x-tri[0,0]).ravel(),(y-tri[0,1]).ravel()]));a=coeff[0].reshape(x.shape);b=coeff[1].reshape(x.shape);mask=(a>=-1e-12)&(b>=-1e-12)&(1-a-b>=-1e-12)&~assigned
  ox[mask]=(out[0,0]+a*(out[1,0]-out[0,0])+b*(out[2,0]-out[0,0]))[mask];oy[mask]=(out[0,1]+a*(out[1,1]-out[0,1])+b*(out[2,1]-out[0,1]))[mask];assigned|=mask
 return ox,oy
ns={'np':np,'coordinates':coordinates};exec(compile(ast.Module(body=[node],type_ignores=[]),str(helper),'exec'),ns)
def read(name):
 p=T/'all-photometric-local-rank'/f'{name}.rgba32';magic=b'RRRAH-RANK-RGBA32-V1\n'
 with p.open('rb') as f:assert f.read(len(magic))==magic;w=int.from_bytes(f.read(4),'little');height=int.from_bytes(f.read(4),'little')
 a=np.memmap(p,dtype='<f4',mode='r',offset=len(magic)+8,shape=(height,w,4));assert np.all(a[:,:,3]==1);return a[:,:,0].astype('f8')*.2126+a[:,:,1].astype('f8')*.7152+a[:,:,2].astype('f8')*.0722
source=read('204700');target=read('204702');regionfile=T/'crumpled-mesh-regions/88-regions.txt';v=list(map(int,regionfile.read_text().split()));sr=v[:4];tr=v[4:];report=D/'dedup-crumpled-mesh-region88-trace.jsonl';rows=[json.loads(x) for x in report.read_text().splitlines()];assert len(rows)==6;summary=[]
for row in rows:
 forward=row['direction']=='forward';actual=ns['measured'](source if forward else target,target if forward else source,(points[:,2:] if forward else points[:,:2],points[:,:2] if forward else points[:,2:]),sr if forward else tr,tr if forward else sr,row['filter_radius'])
 for key in ['valid_sites','informative_pairs','agreeing_pairs']:assert actual[key]==row[key],(row['direction'],row['filter_radius'],key,actual[key],row[key])
 summary.append({'direction':row['direction'],'filter_radius':row['filter_radius'],**actual})
files=[pointsfile,facefile,helper,regionfile,report,Path(__file__),T/'all-photometric-local-rank/204700.rgba32',T/'all-photometric-local-rank/204702.rgba32'];result={'status':'verified_independent_crumpled_local_mesh_pixel_counts','comparisons':6,'faces':len(faces),'summary':summary,'pins':{str(p):h(p) for p in files},'scope':'Independent linear-system barycentric mapping on exported native face topology plus independent bilinear/window/ordinal counts. Native topology/decoder shared; no independent triangulation/conformity or broad precision/copy admission.'};out=D/'dedup-crumpled-mesh-region88-pixel-audit.json';assert not out.exists();out.write_text(json.dumps(result,indent=2)+'\n');print(result['status'])
