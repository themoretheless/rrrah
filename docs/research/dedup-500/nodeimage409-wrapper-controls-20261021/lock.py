import json,base64,hashlib
from pathlib import Path
root=Path('.dedup-work/nodeimage409-20261021');d=json.loads((root/'package-lock.json').read_text());rows=[]
def walk(deps,parent=''):
 for name,v in deps.items():
  integrity=v.get('integrity');parts=[]
  if integrity:
   for token in integrity.split():
    alg,encoded=token.split('-',1);raw=base64.b64decode(encoded,validate=True);assert len(raw)=={'sha1':20,'sha256':32,'sha512':64}[alg];parts.append(alg)
  rows.append({'path':parent+'/'+name,'version':v['version'],'resolved':v.get('resolved'),'integrity_algorithms':parts,'requires':v.get('requires',{})})
  walk(v.get('dependencies',{}),parent+'/'+name)
walk(d['dependencies'])
Path('docs/research/dedup-500/nodeimage409-wrapper-controls-20261021/lock-result.json').write_text(json.dumps({'lockfile_version':d['lockfileVersion'],'dependency_records':len(rows),'integrity_records':sum(bool(r['integrity_algorithms']) for r in rows),'records':rows,'scope':'Every nested dependency record assessed structurally; npm ci subsequently succeeds with public npm registry explicitly selected. Not manual dependency implementation review.'},indent=2)+'\n');print('lock records',len(rows))
