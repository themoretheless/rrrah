"""Independent all-original-point selection and normalized pixel count oracle."""
import ast,hashlib,json,math
from pathlib import Path
import numpy as np
D=Path('docs/research');T=Path('/Users/themoretheless/.codex/tmp/rrrah-dedup-copydays');raw=(D/'dedup-anchor-automatic-full.json').read_bytes();data=json.loads(raw);selected=[x for x in data['results'] if x['query'] in ['202502.jpg','212602.jpg','204702.jpg','200501.jpg']];n=len(selected);assert n>0
snapshot=D/f'dedup-anchor-automatic-pixels-prefix-{n}.json';assert not snapshot.exists();snapshot.write_bytes(raw);h=lambda p:hashlib.sha256(Path(p).read_bytes()).hexdigest()
for p,digest in data['pins'].items():assert h(p)==digest,p
helper=Path('scripts/verify-dedup-rank-pixel-oracle.py');nodes={x.name:x for x in ast.parse(helper.read_text()).body if isinstance(x,ast.FunctionDef)};ns={'np':np};exec(compile(ast.Module(body=[nodes['coordinates'],nodes['measured']],type_ignores=[]),str(helper),'exec'),ns)
files=[snapshot,helper,Path(__file__)];rows=[]
def read(name):
 p=T/'all-photometric-local-rank'/f'{name[:-4]}.rgba32';files.append(p)
 with p.open('rb') as f:assert f.read(21)==b'RRRAH-RANK-RGBA32-V1\n';w=int.from_bytes(f.read(4),'little');height=int.from_bytes(f.read(4),'little')
 a=np.memmap(p,dtype='<f4',mode='r',offset=29,shape=(height,w,4));assert np.all(a[:,:,3]==1);return a[:,:,0].astype('f8')*.2126+a[:,:,1].astype('f8')*.7152+a[:,:,2].astype('f8')*.0722
for row in selected:
 assert row['returncode']==0;e=row['evidence'];assert e['status']=='ok'
 source=read(row['original']);target=read(row['query']);model=np.array(e['matrix']);inverse=np.linalg.inv(model);assert np.all(np.isfinite(inverse));compatible=[];points=e['points']
 for index,(left,right) in enumerate(points):
  assert all(math.isfinite(v) for point in [left,right] for v in point)
  assert 0<=left[0]<source.shape[1] and 0<=left[1]<source.shape[0] and 0<=right[0]<target.shape[1] and 0<=right[1]<target.shape[0]
  for earlier in points[:index]:assert math.dist(left,earlier[0])>2 and math.dist(right,earlier[1])>2
  fx,fy=ns['coordinates'](model,np.array(left[0]),np.array(left[1]));rx,ry=ns['coordinates'](inverse,np.array(right[0]),np.array(right[1]))
  if np.hypot(fx-right[0],fy-right[1])<=2 and np.hypot(rx-left[0],ry-left[1])<=row['source_tolerance']:compatible.append(index)
 for direction,a,t,mapping,side in [('forward',source,target,inverse,1),('reverse',target,source,model,0)]:
  windows=[]
  for index in compatible:
   center=points[index][side];x=int(math.floor(center[0]))-2;y=int(math.floor(center[1]))-2
   if x<0 or y<0 or x+5>t.shape[1] or y+5>t.shape[0]:continue
   if any(x<px+5 and px<x+5 and y<py+5 and py<y+5 for px,py,_,_ in windows):continue
   windows.append([x,y,5,5])
  i=0 if direction=='forward' else 1;assert len(windows)==e['anchors'][i]>=10
  total={'sites':25*len(windows),'valid_sites':0,'informative_pairs':0,'agreeing_pairs':0}
  for region in windows:
   counts=ns['measured'](a,t,mapping,[0,0,a.shape[1],a.shape[0]],region,8)
   for k in ['valid_sites','informative_pairs','agreeing_pairs']:total[k]+=counts[k]
  assert total==e['directions'][i],(row['query'],direction,total,e['directions'][i])
  rows.append({'query':row['query'],'direction':direction,'anchors':len(windows),'counts':total})
 assert e['supported']==all(v['informative_pairs']>=1000 and v['valid_sites']/v['sites']>=.3 and v['agreeing_pairs']/v['informative_pairs']>=.9 for v in e['directions'])
files=list(dict.fromkeys(files));out=D/f'dedup-anchor-automatic-pixels-prefix-{n}-audit.json';out.write_text(json.dumps({'status':'verified_automatic_anchor_original_point_selection_independent_pixels','pairs':n,'directions':len(rows),'rows':rows,'pins':{str(p):h(p) for p in files},'scope':'Frozen completed prioritized prefix: all original automatic union points, independent aliases/bounds/bidirectional residuals/disjoint selection, exact NumPy original-normalized pixel count parity and support predicate. Native JPEG normalization shared via prior dumps; no independent decoder or full corpus/precision proof.'},indent=2)+'\n');print('Verified',n,'automatic real pairs independent pixels')
