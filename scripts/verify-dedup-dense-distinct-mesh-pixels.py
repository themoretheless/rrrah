"""Independent full-domain resampling/counts for the194-landmark mesh."""
import ast,hashlib,json
from pathlib import Path
import numpy as np
root=Path(__file__).resolve().parents[1];base=root/'docs/research'
def digest(p):return hashlib.sha256(Path(p).read_bytes()).hexdigest()
landmarks=base/'dedup-folded-dense-small-window-distinct-landmarks.txt';topology=base/'dedup-folded-dense-distinct-topology.json';native=base/'dedup-folded-dense-distinct-native-mesh-rank.jsonl';manifest=base/'dedup-rank-normalized-pixels.json';helper=root/'scripts/measure-dedup-piecewise-pixels.py'
v=[list(map(float,line.split())) for line in landmarks.read_text().splitlines()];assert len(v)==194;p=[{'source':r[:2],'target':r[2:]} for r in v];mesh=json.loads(topology.read_text());assert mesh['source_mesh_status']=='accepted' and mesh['landmarks']==194
original=json.loads((base/'dedup-folded-qualified-landmarks.json').read_text())['rows'];assert all(a['source']==b['source'] and a['target']==b['target'] for a,b in zip(original,p[:156]))
assert set(i for face in mesh['triangles'] for i in face)==set(range(194))
for face in mesh['triangles']:
 for key in ['source','target']:
  a,b,c=[p[i][key] for i in face];assert (b[0]-a[0])*(c[1]-a[1])-(b[1]-a[1])*(c[0]-a[0])>1e-6
ns={'np':np,'p':p,'mesh':mesh};tree=ast.parse(helper.read_text());nodes=[n for n in tree.body if isinstance(n,ast.FunctionDef) and n.name in ['warp','box','rank']];assert len(nodes)==3;exec(compile(ast.Module(body=nodes,type_ignores=[]),str(helper),'exec'),ns)
images={};d=json.loads(manifest.read_text());pins={str(path):digest(path) for path in [landmarks,topology,native,manifest,helper,Path(__file__),root/'docs/research/dedup-folded-dense-distinct-state.json',Path('/Users/themoretheless/.codex/tmp/rrrah-dedup-copydays/wide-topology-probe-qualified'),Path('/Users/themoretheless/.codex/tmp/rrrah-dedup-copydays/mesh-rank-probe-qualified')]}
magic=b'RRRAH-RANK-RGBA32-V1\n'
for stem in ['201700','201702']:
 row=next(r for r in d['rows'] if Path(r['source']).stem==stem);path=Path(row['pixels']);assert digest(path)==row['pixels_sha256'];pins[str(path)]=digest(path);assert digest(row['source'])==row['source_sha256'];pins[row['source']]=row['source_sha256']
 with path.open('rb') as f:assert f.read(len(magic))==magic;w=int.from_bytes(f.read(4),'little');h=int.from_bytes(f.read(4),'little')
 data=np.memmap(path,dtype='<f4',mode='r',offset=len(magic)+8,shape=(h,w,4));images[stem]=data[:,:,0].astype('f8')*.2126+data[:,:,1].astype('f8')*.7152+data[:,:,2].astype('f8')*.0722
rows=[json.loads(line) for line in native.read_text().splitlines()];assert len(rows)==6;summary=[]
for name,source,target,from_key,to_key in [('forward',images['201700'],images['201702'],'target','source'),('reverse',images['201702'],images['201700'],'source','target')]:
 warped,covered=ns['warp'](source,target.shape,from_key,to_key)
 for radius in [0,3,8]:
  expected=ns['rank'](warped,target,radius);row=next(r for r in rows if r['direction']==name and r['filter_radius']==radius);assert row['covered']==covered
  for key in ['valid_sites','informative_pairs','agreeing_pairs']:assert expected[key]==row[key],(name,radius,key,expected[key],row[key])
  summary.append({'direction':name,'filter_radius':radius,'covered':covered,**expected});print(name,radius,'exact',flush=True)
assert all(digest(k)==v for k,v in pins.items())
(base/'dedup-folded-dense-distinct-pixel-audit.json').write_text(json.dumps({'status':'verified_independent_augmented_mesh_pixel_counts','landmarks':194,'triangles':len(mesh['triangles']),'comparisons':6,'summary':summary,'pins':pins,'scope':'All original156 landmarks preserved; native194 topology has positive source/target face orientations. Six complete native count sets equal independent NumPy mesh resampling and means/ordinal math. Decoder shared; no all-negative precision, native proposal API, or copy admission.'},indent=2)+'\n')
