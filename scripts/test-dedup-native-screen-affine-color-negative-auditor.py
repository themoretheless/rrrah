"""Mutation controls against the real terminal negative-gate auditor."""
import copy,hashlib,json,subprocess,tempfile
from pathlib import Path
source=Path('docs/research/dedup-native-screen-affine-color-negative-gate.json');verifier=Path('scripts/verify-dedup-native-screen-affine-color-negative-gate.py');out=Path('docs/research/dedup-native-screen-affine-color-negative-auditor-controls.json');assert not out.exists();h=lambda p:hashlib.sha256(Path(p).read_bytes()).hexdigest();original=json.loads(source.read_text());pins={str(p):h(p) for p in [source,verifier,Path(__file__)]};results=[]
def accepted(r):r['rows'][0]['accepted']=True
def sample(r):
 row=next(v for v in r['rows'] if any('samples' in d['evidence'] for d in v['evidence']['directions']));d=next(d for d in row['evidence']['directions'] if 'samples' in d['evidence']);d['evidence']['samples']+=1;row['stdout']=json.dumps(row['evidence'])
def geometry(r):r['rows'][0]['roi'][0]+=1
def source_hash(r):r['rows'][0]['source_sha256']='0'*64
def missing(r):r['rows'].pop()
def duplicate(r):r['rows'][-1]=copy.deepcopy(r['rows'][0])
with tempfile.TemporaryDirectory(prefix='rrrah-color-auditor-') as folder:
 for name,mutate in [('valid',None),('false_acceptance',accepted),('sample_count',sample),('roi_geometry',geometry),('source_digest',source_hash),('missing_case',missing),('duplicate_case',duplicate)]:
  r=copy.deepcopy(original)
  if mutate:mutate(r)
  path=Path(folder)/(name+'.json');audit=Path(folder)/(name+'-audit.json');path.write_text(json.dumps(r));v=subprocess.run(['python3',str(verifier),str(path),str(audit)],capture_output=True,text=True)
  assert (v.returncode==0)==(mutate is None),(name,v.stdout,v.stderr)
  assert audit.exists()==(mutate is None)
  results.append({'case':name,'returncode':v.returncode,'expected':'accept' if mutate is None else 'reject'})
assert all(h(k)==v for k,v in pins.items());out.write_text(json.dumps({'status':'passed','input_hashes':pins,'controls':results,'scope':'Finite real-report mutation controls; not pixel or algorithm correctness evidence.'},indent=2)+'\n');print(json.dumps(results))
