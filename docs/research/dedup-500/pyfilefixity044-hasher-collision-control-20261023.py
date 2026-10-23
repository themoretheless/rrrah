import importlib.util,sys,types,json,hashlib
from pathlib import Path
root=Path('.dedup-work/pyfilefixity044-20261023')
pkg=types.ModuleType('fixity_review');pkg.__path__=[str(root)];sys.modules[pkg.__name__]=pkg
spec=importlib.util.spec_from_file_location('fixity_review.hasher',Path('docs/research/dedup-500/evidence/lrq3000--pyFileFixity/03-hasher.py'));m=importlib.util.module_from_spec(spec);spec.loader.exec_module(m)
results={}
for algo in ['minimd5','minisha256','shortmd5','shortsha256']:
 h=m.Hasher(algo);seen={}
 for i in range(100000):
  data=f'file-content-{i}'.encode();key=h.hash(data)
  if key in seen:
   old=seen[key];assert old!=data and hashlib.sha256(old).digest()!=hashlib.sha256(data).digest()
   results[algo]={'collision_key':key.decode(),'first':old.decode(),'second':data.decode(),'attempts':i+1};break
  seen[key]=data
 else:raise AssertionError('No collision within bound')
print(json.dumps(results,indent=2))
(root/'collision-results.json').write_text(json.dumps(results,indent=2)+'\n')
