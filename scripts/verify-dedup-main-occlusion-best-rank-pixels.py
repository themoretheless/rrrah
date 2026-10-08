"""Independent NumPy window reductions on both best occlusion regions."""
import ast,hashlib,json
from pathlib import Path
import numpy as np
D=Path('docs/research');T=Path('/Users/themoretheless/.codex/tmp/rrrah-dedup-copydays');h=lambda p:hashlib.sha256(Path(p).read_bytes()).hexdigest()
report=D/'dedup-main-occlusion-all-rank-regions.json';audit=D/'dedup-main-occlusion-all-rank-regions-audit.json';r=json.loads(report.read_text());a=json.loads(audit.read_text());assert r['status']=='complete' and a['status']=='verified_all_existing_occlusion_rank_region_diagnostic'
assert all(h(k)==v for k,v in r['input_hashes'].items()) and all(h(k)==v for k,v in a['pins'].items())
helper=Path('scripts/verify-dedup-rank-pixel-oracle.py');nodes={v.name:v for v in ast.parse(helper.read_text()).body if isinstance(v,ast.FunctionDef)};ns={'np':np};exec(compile(ast.Module(body=[nodes['coordinates'],nodes['measured']],type_ignores=[]),str(helper),'exec'),ns)
def read(name):
 p=T/'rank-normalized-pixel-oracle'/('occlusion-'+Path(name).stem+'.rgba32');magic=b'RRRAH-RANK-RGBA32-V1\n'
 with p.open('rb') as f:
  assert f.read(len(magic))==magic;w=int.from_bytes(f.read(4),'little');height=int.from_bytes(f.read(4),'little')
 pixels=np.memmap(p,dtype='<f4',mode='r',offset=len(magic)+8,shape=(height,w,4));assert np.all(np.isfinite(pixels)) and np.all(pixels[:,:,3]==1) and np.min(pixels[:,:,:3])>=0 and np.max(pixels[:,:,:3])<=1
 return pixels[:,:,0].astype('f8')*.2126+pixels[:,:,1].astype('f8')*.7152+pixels[:,:,2].astype('f8')*.0722
summary=[]
for query,best in a['best'].items():
 row=next(v for v in r['results'] if v['query']==query and v['region_index']==best['region_index']);source=read(row['original']);target=read(query)
 model=T/f"occlusion-all-rank-{Path(query).stem}-{row['region_index']}.txt";values=list(map(float,model.read_text().split()));m=np.array(values[:9]).reshape(3,3);inverse=np.linalg.inv(m);sr,tr=row['domains']
 for native in row['rank']:
  direction=native['direction'];forward=direction=='forward'
  calculated=ns['measured'](source if forward else target,target if forward else source,inverse if forward else m,sr if forward else tr,tr if forward else sr,native['filter_radius'])
  for key in ('valid_sites','informative_pairs','agreeing_pairs'):assert calculated[key]==native[key],(query,direction,native['filter_radius'],key,calculated[key],native[key])
  summary.append({'query':query,'region_index':row['region_index'],'direction':direction,'filter_radius':native['filter_radius'],**calculated});print(query,direction,native['filter_radius'],'verified',flush=True)
assert all(h(k)==v for k,v in r['input_hashes'].items())
out=D/'dedup-main-occlusion-best-rank-pixel-audit.json';assert not out.exists();out.write_text(json.dumps({'status':'verified_independent_best_occlusion_rank_pixel_counts','comparisons':12,'summary':summary,'pins':{str(p):h(p) for p in [report,audit,helper,Path(__file__)]},'scope':'Independent inverse matrix, bilinear luminance and sliding window means/ordinal counts. Twelve exact count checks on two best existing regions. Shared normalized native decoder pixels; no independent decoding, copy admission, held-out selection or broad negative precision.'},indent=2)+'\n')
