"""Parser rejection controls for release evidence, not native recall."""
import copy,hashlib,json,subprocess,tempfile
from pathlib import Path
source=Path('docs/research/dedup-combined-domain-release-checkpoint-1.json');verifier=Path('scripts/verify-dedup-combined-domain-release.py');base=json.loads(source.read_text());cases={}
def mutate(name,fn):
 r=copy.deepcopy(base);fn(r);cases[name]=r
def evidence(r,k,v):
 r['results'][0]['evidence'][k]=v;r['results'][0]['stdout']=json.dumps(r['results'][0]['evidence'])
mutate('truncated_terminal',lambda r:r.update(status='verified_native_release_parity'))
mutate('wrong_query',lambda r:r['results'][0].update(query='200701.jpg'))
mutate('bool_returncode',lambda r:r['results'][0].update(returncode=False))
mutate('bool_peak',lambda r:evidence(r,'managed_peak',True))
mutate('stdout_disagreement',lambda r:r['results'][0].update(stdout='{}'))
mutate('native_error',lambda r:r['results'][0].update(returncode=101))
with tempfile.TemporaryDirectory(prefix='rrrah-release-audit-controls-') as tmp:
 root=Path(tmp)
 def run(r,name):
  p=root/(name+'.json');out=root/(name+'-audit.json');p.write_text(json.dumps(r)+'\n');v=subprocess.run(['python3',str(verifier),'--checkpoint',str(p),str(out)],capture_output=True,text=True);return v,out
 v,out=run(base,'valid');assert v.returncode==0,v.stderr
 for name,r in cases.items():
  v,out=run(r,name);assert v.returncode!=0 and not out.exists(),name
h=lambda p:hashlib.sha256(p.read_bytes()).hexdigest();out=Path('docs/research/dedup-combined-domain-release-auditor-controls.json');assert not out.exists();out.write_text(json.dumps({'status':'parser_controls_passed','rejected_cases':list(cases),'input_hashes':{str(source):h(source),str(verifier):h(verifier),str(Path(__file__)):h(Path(__file__))},'scope':'Six synthetic report corruptions rejected and actual first-row prefix accepted. No additional native comparisons or corpus recall.'},indent=2)+'\n');print('six corruptions rejected')
