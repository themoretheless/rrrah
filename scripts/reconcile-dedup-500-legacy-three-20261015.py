#!/usr/bin/env python3
from pathlib import Path
import json,hashlib,shutil
R=Path(__file__).resolve().parents[1];D=R/'docs/research/dedup-500';sha=lambda p:hashlib.sha256(Path(p).read_bytes()).hexdigest();B=D/'legacy-three-commit-tree-bindings-20261015.json';bindings={r['repository']:r for r in json.loads(B.read_text())};O=D/'legacy-three-source-review-reconciliation-20261015.json';assert not O.exists();prepared=[]
for index,name in [(100,'image-deduplication-tool'),(123,'deduplicate-elasticsearch'),(471,'perceptual-hashes-benchmark')]:
 P=D/f'reviews/{index:03}.json';registry=json.loads(P.read_text());M=D/f'{name}-full-source-20261012.json';V=D/f'{name}-complete-source-review-20261012.json';A=D/f'{name}-complete-runtime-audit-20261012.json';m=json.loads(M.read_text());v=json.loads(V.read_text());a=json.loads(A.read_text());E=D/'evidence'/registry['repository'].replace('/','--');T=E/'tree.json';t=json.loads(T.read_text());binding=bindings[registry['repository']];assert m['tree_complete']and not t['truncated']and binding['tree']==t['sha']and binding['commit']==m['commit']==v['commit']==registry['commit'];blobs={r['path']:r for r in t['tree']if r['type']=='blob'};assert set(v['fully_read_files'])==set(blobs)=={r['path']for r in m['files']};files=[]
 for row in m['files']:
  path=Path(m['source'])/row['path'];data=path.read_bytes();gitsha=hashlib.sha1(b'blob '+str(len(data)).encode()+b'\0'+data).hexdigest();assert len(data)==row['bytes']==blobs[row['path']]['size']and sha(path)==row['sha256']and gitsha==row['git_blob_sha1']==blobs[row['path']]['sha'];files.append(dict(path=row['path'],source=str(path),sha256=sha(path),git_blob_sha1=gitsha,bytes=len(data)))
 drift=[]
 for key,h in a['pins'].items():
  path=Path(key)if Path(key).is_absolute()else R/key
  if not path.is_file()or sha(path)!=h:drift.append(key)
 prepared.append(dict(index=index,repository=registry['repository'],commit=registry['commit'],tree=t['sha'],prior_registry_sha256=sha(D/'registry-history/20261015-legacy-three'/P.name)if(D/'registry-history/20261015-legacy-three'/P.name).exists()else sha(P),manifest=str(M.relative_to(R)),manifest_sha256=sha(M),prior_review=str(V.relative_to(R)),prior_review_sha256=sha(V),prior_runtime_audit=str(A.relative_to(R)),prior_runtime_audit_sha256=sha(A),runtime_pin_drift=drift,prior_runtime_scope=a['scope'],files=files))
H=D/'registry-history/20261015-legacy-three';H.mkdir(exist_ok=True)
for row in prepared:
 P=D/f"reviews/{row['index']:03}.json";registry=json.loads(P.read_text());assert not(H/P.name).exists()or sha(H/P.name)==row['prior_registry_sha256'];
 if not(H/P.name).exists():shutil.copyfile(P,H/P.name)
 E=D/'evidence'/registry['repository'].replace('/','--');known={item['path']for key in ['available_source_evidence','available_readme_license_evidence']for item in registry.get(key,[])}
 for f in row['files']:
  if f['path']in known:continue
  assert f['path']=='.gitignore';local=E/'03-gitignore';assert not local.exists();shutil.copyfile(f['source'],local);assert sha(local)==f['sha256'];registry['available_source_evidence'].append(dict(path=f['path'],local=local.name,url=f"https://github.com/{row['repository']}/blob/{row['commit']}/{f['path']}",sha256=f['sha256'],truncated=False,lines=len(local.read_bytes().splitlines()),source=False,test=False))
 registry.update(full_repository_review_complete=True,prior_complete_source_review=row['prior_review'],prior_runtime_audit=row['prior_runtime_audit'],source_review_reconciliation=str(O.relative_to(R)),review_scope='Prior complete pinned-source review reconciled against all current retained Gitblob bytes and fresh immutablecommit/tree binding; not a new source reading or new runtime execution. '+row['prior_runtime_scope']);P.write_text(json.dumps(registry,indent=2)+'\n')
O.write_text(json.dumps(dict(status='verified_prior_source_reviews_reconciled_not_new_runtime',producer_sha256=sha(__file__),fresh_bindings_sha256=sha(B),entries=prepared,scope='Three prior fullsource readings and bounded adapter controls linked back to original500registry after current fulltree/source/hash checks. Original review findings/limits retained. Runtime audit pin drift listed explicitly; no reruns, server/media/deletion operations, new source adoption or all500completion claim.'),indent=2)+'\n');print('Three prior complete source reviews reconciled; runtime pin drift:',[r['runtime_pin_drift']for r in prepared])
