#!/usr/bin/env python3
import json,hashlib,re
from pathlib import Path
R=Path(__file__).resolve().parents[1];D=R/'docs/research/dedup-500';sha=lambda p:hashlib.sha256(Path(p).read_bytes()).hexdigest();names=['fclones-complete-source-review-20261012.json','anew-full-review-20261012.json','anew-full-review-audit-20261012.json','img-hash-full-review-20261012.json','deduplicator-full-review-20261012.json','go-find-duplicates-complete-source-review-20261012.json','blockhash-js-full-review-20261012.json','photohash-full-review-20261012.json','dupe-krill-full-review-20261011.json'];rows=[]
for name in names:
 visited=set();pins={};errors=[];manifests=[]
 def verify_file(p,h,kind):
  p=Path(p)
  if not p.is_file():errors.append(dict(path=str(p),kind='missing_'+kind));return False
  actual=sha(p)
  if actual!=h:errors.append(dict(path=str(p),kind='changed_'+kind,expected=h,actual=actual));return False
  pins[str(p)]=h;return True
 def inspect(p):
  p=Path(p)
  if str(p)in visited:return
  visited.add(str(p));data=json.loads(p.read_text())
  if not isinstance(data,dict):return
  for path,h in data.get('pins',{}).items():
   if isinstance(h,str) and re.fullmatch('[0-9a-f]{64}',h):
    q=Path(path);q=q if q.is_absolute() else R/q
    if verify_file(q,h,'pin') and q.suffix=='.json':inspect(q)
  root=next((Path(data[k]) for k in ['snapshot','source','source_root'] if isinstance(data.get(k),str) and Path(data[k]).is_dir()),None)
  if root:
   source=data.get('file_hashes',data.get('source_file_hashes',data.get('files',{})))
   if isinstance(source,list):source={r['path']:r['sha256'] for r in source if isinstance(r,dict) and 'path'in r and 'sha256'in r}
   if isinstance(source,dict):
    count=0
    for path,h in source.items():
     if isinstance(h,str) and re.fullmatch('[0-9a-f]{64}',h):verify_file(root/path,h,'source');count+=1
    if count:manifests.append(dict(artifact=str(p),root=str(root),declared_source_files=count))
 p=D/name;data=json.loads(p.read_text());pins[str(p)]=sha(p);inspect(p)
 for path,h in pins.items():
  if sha(path)!=h:errors.append(dict(path=path,kind='changed_during_verification'))
 rows.append(dict(artifact=str(p.relative_to(R)),artifact_sha256=sha(p),repository=data.get('repository'),original_status=data.get('status'),original_scope=data.get('scope',data.get('limitations')),pins_checked=len(pins),source_manifests=manifests,pins=pins,errors=errors,status='verified_existing_pins_not_new_execution' if not errors else 'historical_artifact_has_current_pin_errors'))
out=D/'unlinked-reviews-pin-audit-20261013.json';assert not out.exists();out.write_text(json.dumps(dict(status='reconciled_candidate_artifact_pins_with_explicit_errors',verifier_sha256=sha(__file__),rows=rows,scope='Existing source/artifact/executable/log hashes revalidated through declared pin chains and source manifests. No new source reading, runtime execution or whole-repository/platform completion inferred. Missing/changed pins stay explicit; old scopes and adapters are retained. Duplicate manifest counts must not be summed as distinct reviews.'),indent=2)+'\n')
for row in rows:print(Path(row['artifact']).name,row['pins_checked'],[(m['declared_source_files']) for m in row['source_manifests']],len(row['errors']));
for row in rows:
 for e in row['errors'][:3]:print('ERROR',e['kind'],e['path'])
