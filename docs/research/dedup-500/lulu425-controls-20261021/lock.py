import json,base64
from pathlib import Path
root=Path('.dedup-work/lulu425-20261021');d=json.loads((root/'package-lock.json').read_text());counts={};records=0
for path,p in d['packages'].items():
 if path:assert p.get('version')
 for item in p.get('integrity','').split():
  alg,value=item.split('-',1);assert len(base64.b64decode(value,validate=True))=={'sha1':20,'sha512':64,'sha256':32}[alg];counts[alg]=counts.get(alg,0)+1
 records+=1
out=Path(__file__).resolve().parent/'lock-result.json';out.write_text(json.dumps({'packages':records,'integrity_algorithms':counts,'scope':'Every package lock record structurally assessed; successful npm ci with exact lock. Not dependency implementation audit.'},indent=2)+'\n')
