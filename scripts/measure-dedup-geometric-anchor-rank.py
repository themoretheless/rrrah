"""Preselected global geometric-anchor windows, ordinal diagnostic only."""
import ast,hashlib,json
from pathlib import Path
import numpy as np
D=Path('docs/research');T=Path('/Users/themoretheless/.codex/tmp/rrrah-dedup-copydays');base=D/'dedup-combined-domain-release-original-full.json';b=json.loads(base.read_text());helper=Path('scripts/verify-dedup-rank-pixel-oracle.py');nodes={x.name:x for x in ast.parse(helper.read_text()).body if isinstance(x,ast.FunctionDef)};ns={'np':np};exec(compile(ast.Module(body=[nodes['coordinates'],nodes['measured']],type_ignores=[]),str(helper),'exec'),ns);h=lambda p:hashlib.sha256(Path(p).read_bytes()).hexdigest();files=[base,helper,Path(__file__)];images={}
def read(name):
 p=T/'all-photometric-local-rank'/f'{name}.rgba32';files.append(p);magic=b'RRRAH-RANK-RGBA32-V1\n'
 with p.open('rb') as f:assert f.read(len(magic))==magic;w=int.from_bytes(f.read(4),'little');height=int.from_bytes(f.read(4),'little')
 a=np.memmap(p,dtype='<f4',mode='r',offset=len(magic)+8,shape=(height,w,4));assert np.all(a[:,:,3]==1);return a[:,:,0].astype('f8')*.2126+a[:,:,1].astype('f8')*.7152+a[:,:,2].astype('f8')*.0722
out=D/'dedup-geometric-anchor-rank.json';assert not out.exists();summary=[]
for original,query in [('202500','202502'),('212600','212602'),('204700','204702'),('200500','200501')]:
 e=next(v['evidence'] for v in b['results'] if v['query']==query+'.jpg');source=read(original);target=read(query);model=np.array(e['matrix']);inv=np.linalg.inv(model)
 for name,a,t,mapping,side in [('forward',source,target,inv,1),('reverse',target,source,model,0)]:
  # Greedy deterministic spatially disjoint5x5 windows, before pixel scoring.
  windows=[];indices=[]
  for index in e['inliers']:
   center=e['correspondences'][index][side];x=int(np.floor(center[0]))-2;y=int(np.floor(center[1]))-2
   if x<0 or y<0 or x+5>t.shape[1] or y+5>t.shape[0]:continue
   if any(x<px+5 and px<x+5 and y<py+5 and py<y+5 for px,py,_,_ in windows):continue
   windows.append([x,y,5,5]);indices.append(index)
  assert len(windows)>=10
  for radius in [0,3,8]:
   total={'valid_sites':0,'informative_pairs':0,'agreeing_pairs':0}
   for region in windows:
    measured=ns['measured'](a,t,mapping,[0,0,a.shape[1],a.shape[0]],region,radius)
    for key in total:total[key]+=measured[key]
   summary.append({'query':query+'.jpg','direction':name,'filter_radius':radius,'anchor_indices':indices,'windows':windows,'sites':len(windows)*25,**total,'agreement':total['agreeing_pairs']/total['informative_pairs'] if total['informative_pairs'] else None})
files=list(dict.fromkeys(files));out.write_text(json.dumps({'status':'complete_independent_geometric_anchor_rank_diagnostic','cases':4,'directions':24,'summary':summary,'pins':{str(p):h(p) for p in files},'scope':'All original global inliers considered in deterministic order, disjoint5x5 anchor windows chosen only by coordinates/bounds before scoring. Independent warp/window counts; no native anchor API or unrelated calibration/automatic identity, not arbitrary region search.'},indent=2)+'\n');print([(x['query'],x['direction'],x['agreement'],x['informative_pairs']) for x in summary if x['filter_radius']==8])
