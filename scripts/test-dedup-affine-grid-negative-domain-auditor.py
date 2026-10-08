"""Finite malformed-report controls on an immutable real negative-grid prefix."""
import copy,hashlib,json,subprocess,tempfile
from pathlib import Path
source=Path('docs/research/dedup-native-screen-affine-color-grid-negatives-meanfix-checkpoint.json');verifier=Path('scripts/verify-dedup-native-screen-affine-color-grid-negatives-domains.py');out=Path('docs/research/dedup-affine-grid-negative-domain-auditor-controls.json');assert not out.exists();h=lambda p:hashlib.sha256(Path(p).read_bytes()).hexdigest();r=json.loads(source.read_text());assert len(r['rows'])>=2;pins={str(p):h(p) for p in [source,verifier,Path(__file__)]}
def support(v):v['rows'][0]['supports']+=1
def digest(v):v['rows'][0]['source_sha256']='0'*64
def radius(v):v['rows'][0]['evidence']['regions'][0]['source_radius']+=1;v['rows'][0]['stdout']=json.dumps(v['rows'][0]['evidence'])
def domain(v):v['rows'][0]['evidence']['regions'][0]['domains'][0][2]=0;v['rows'][0]['stdout']=json.dumps(v['rows'][0]['evidence'])
def samples(v):
 for row in v['rows']:
  for region in row['evidence']['regions']:
   for direction in region['directions']:
    if 'matched' in direction:direction['matched']=direction['samples']+1;row['stdout']=json.dumps(row['evidence']);return
 raise AssertionError('no successful direction in real prefix')
def duplicate(v):v['rows'][-1]=copy.deepcopy(v['rows'][0])
def shifted(v):
 v['rows'][0]['evidence']['regions'][0]['domains'][0][0]+=1;v['rows'][0]['stdout']=json.dumps(v['rows'][0]['evidence'])
def missing(v):
 v['rows'][0]['evidence']['regions'].pop();v['rows'][0]['stdout']=json.dumps(v['rows'][0]['evidence'])
results=[]
with tempfile.TemporaryDirectory(prefix='rrrah-grid-auditor-') as folder:
 for name,mutation in [('valid',None),('false_support',support),('source_digest',digest),('radius',radius),('domain',domain),('matched_count',samples),('duplicate_source',duplicate),('shifted_nonempty_source',shifted),('missing_grid_cell',missing)]:
  v=copy.deepcopy(r)
  if mutation:mutation(v)
  path=Path(folder)/(name+'.json');audit=Path(folder)/(name+'-audit.json');path.write_text(json.dumps(v));result=subprocess.run(['python3',str(verifier),str(path),str(audit)],capture_output=True,text=True)
  assert (result.returncode==0)==(mutation is None),(name,result.stdout,result.stderr);assert audit.exists()==(mutation is None);results.append({'case':name,'returncode':result.returncode})
assert all(h(k)==v for k,v in pins.items());out.write_text(json.dumps({'status':'passed','input_hashes':pins,'controls':results,'scope':'Finite auditor mutation controls only, not pixel/copy precision evidence.'},indent=2)+'\n');print(results)
