"""Finite corrupt-report controls for native inlier-bounded pixel reports."""
import copy,hashlib,json,subprocess,tempfile
from pathlib import Path
source=Path('docs/research/dedup-folded-witness-common-footprint-regions.json')
verifier=Path('scripts/verify-dedup-folded-witness-common-footprint-regions.py')
output=Path('docs/research/dedup-folded-witness-auditor-controls.json');assert not output.exists()
digest=lambda p:hashlib.sha256(Path(p).read_bytes()).hexdigest()
original=json.loads(source.read_text());assert original['status']=='complete_native_experiment'
pins={str(p):digest(p) for p in [source,verifier,Path(__file__)]}
def shifted(v):v['rows'][0]['domains'][1][0]+=1
def model(v):v['rows'][0]['matrix'][0][0]+=0.01
def radius(v):v['rows'][0]['evidence']['directions'][1]['radius']+=1
def samples(v):v['rows'][0]['evidence']['directions'][1]['samples']+=1
def matched(v):
 d=v['rows'][0]['evidence']['directions'][0];d['matched']=d['samples']+1
def supports(v):v['rows'][0]['supported']=True
def duplicate(v):v['rows'][-1]=copy.deepcopy(v['rows'][0])
def mode(v):v['rows'][0]['evidence']['footprints']=['target','target']
def source_hash(v):v['rows'][0]['input_sha256']='0'*64
cases=[('valid',None),('shifted_domain',shifted),('changed_model',model),('radius',radius),('sample_count',samples),('matched_count',matched),('false_support',supports),('duplicate_case',duplicate),('wrong_footprint',mode),('input_digest',source_hash)]
results=[]
with tempfile.TemporaryDirectory(prefix='rrrah-witness-controls-') as folder:
 for name,mutation in cases:
  report=copy.deepcopy(original)
  if mutation:
   mutation(report)
   for row in report['rows']:row['stdout']=json.dumps(row['evidence'])
  path=Path(folder)/(name+'.json');audit=Path(folder)/(name+'-audit.json');path.write_text(json.dumps(report));result=subprocess.run(['python3',str(verifier),str(path),str(audit)],capture_output=True,text=True)
  assert (result.returncode==0)==(mutation is None),(name,result.stderr[-2000:]);assert audit.exists()==(mutation is None);results.append({'case':name,'returncode':result.returncode});print(name,result.returncode,flush=True)
assert all(digest(k)==v for k,v in pins.items())
output.write_text(json.dumps({'status':'passed','input_hashes':pins,'controls':results,'scope':'One valid six-case report and nine deliberately malformed controls. Not pixel resampling or broad fold recovery.'},indent=2)+'\n')
