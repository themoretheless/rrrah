"""Independent pixel counts for native combined screen-region API."""
import ast,hashlib,json
from pathlib import Path
import numpy as np
D=Path('docs/research');T=Path('/Users/themoretheless/.codex/tmp/rrrah-dedup-copydays');h=lambda p:hashlib.sha256(Path(p).read_bytes()).hexdigest();pins_path=D/'dedup-main-screen-local-rank-api-pins.json';pins=json.loads(pins_path.read_text());assert all(h(k)==v for k,v in pins.items());report=D/'dedup-main-screen-local-rank-api.jsonl';rows=[json.loads(x) for x in report.read_text().splitlines()];assert [x['filter_radius'] for x in rows]==[0,3,8]
helper=Path('scripts/verify-dedup-rank-pixel-oracle.py');nodes={x.name:x for x in ast.parse(helper.read_text()).body if isinstance(x,ast.FunctionDef)};ns={'np':np};exec(compile(ast.Module(body=[nodes['coordinates'],nodes['measured']],type_ignores=[]),str(helper),'exec'),ns)
def read(name):
 p=T/'rank-normalized-pixel-oracle'/f'{name}.rgba32';magic=b'RRRAH-RANK-RGBA32-V1\n'
 with p.open('rb') as f:assert f.read(len(magic))==magic;w=int.from_bytes(f.read(4),'little');height=int.from_bytes(f.read(4),'little')
 a=np.memmap(p,dtype='<f4',mode='r',offset=len(magic)+8,shape=(height,w,4));assert np.all(a[:,:,3]==1) and np.all(np.isfinite(a)) and np.min(a[:,:,:3])>=0 and np.max(a[:,:,:3])<=1
 return a[:,:,0].astype('f8')*.2126+a[:,:,1].astype('f8')*.7152+a[:,:,2].astype('f8')*.0722
source=read('200500');target=read('200501');v=list(map(float,(T/'all-screen-local-rank-58.txt').read_text().split()));model=np.array(v[:9]).reshape(3,3);inv=np.linalg.inv(model);sr=list(map(int,v[9:13]));tr=list(map(int,v[13:]));summary=[]
region_audit=json.loads((D/'dedup-main-screen-all-local-rank-audit.json').read_text());expected=next(x for x in region_audit['supported_cases'] if x['region_index']==58 and x['filter_radius']==8);assert expected['witnesses']==22
for row in rows:
 assert row['status']=='ok' and row['witnesses']==22
 for native,a,b,mapping,ar,br in zip(row['directions'],[source,target],[target,source],[inv,model],[sr,tr],[tr,sr]):
  actual=ns['measured'](a,b,mapping,ar,br,row['filter_radius'])
  for key in ['valid_sites','informative_pairs','agreeing_pairs']:assert actual[key]==native[key],(row['filter_radius'],key)
  summary.append({'filter_radius':row['filter_radius'],**actual})
 passes=all(x['informative_pairs']>=1000 and x['valid_sites']/x['sites']>=.3 and x['agreeing_pairs']/x['informative_pairs']>=.9 for x in row['directions']);assert passes==row['supported']==(row['filter_radius']==8)
assert all(h(k)==v for k,v in pins.items());files=[pins_path,report,helper,Path(__file__)];out=D/'dedup-main-screen-local-rank-api-audit.json';assert not out.exists();out.write_text(json.dumps({'status':'verified_combined_screen_local_rank_independent_pixels','comparisons':6,'regional_witnesses':22,'summary':summary,'pins':{**pins,**{str(p):h(p) for p in files}},'scope':'Actual combined API, six independent inverse/bilinear/sliding-window/ordinal count checks and independently region-qualified22 witnesses. Shared decoder dumps, selected region, not broad precision or automatic file admission.'},indent=2)+'\n');print('Verified6 combined screen pixel comparisons')
