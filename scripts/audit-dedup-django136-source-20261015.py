from pathlib import Path
import json,hashlib,ast
R=Path(__file__).resolve().parents[1];D=R/'docs/research/dedup-500';P=D/'django136-complete-source-20261015.json';d=json.loads(P.read_text());h=lambda p:hashlib.sha256(p.read_bytes()).hexdigest();sha=lambda kind,b:hashlib.sha1(kind+b' '+str(len(b)).encode()+b'\0'+b).hexdigest();rows=[]
for b in d['blobs']:
 raw=b['content'].encode();row=b['row'];assert sha(b'blob',raw)==row['sha'] and len(raw)==row['size'];rows.append(dict(path=row['path'],git_blob=row['sha'],bytes=len(raw),sha256=hashlib.sha256(raw).hexdigest()))
tree=d['tree'];assert not tree['truncated'] and len(rows)==22
for parent,want in [('',d['commit']['tree'])]+[(x['path'],x['sha']) for x in tree['tree'] if x['type']=='tree']:
 children=[]
 for row in tree['tree']:
  path=Path(row['path']);folder=str(path.parent);folder='' if folder=='.' else folder
  if folder==parent:children.append((path.name,row))
 children.sort(key=lambda x:x[0]+('/' if x[1]['type']=='tree' else ''))
 raw=b''.join(('40000' if row['type']=='tree' else row['mode']).encode()+b' '+name.encode()+b'\0'+bytes.fromhex(row['sha']) for name,row in children);assert sha(b'tree',raw)==want,parent
logs=[D/f'django136-adapter-controls-v2-run{i}-20261015.log' for i in [1,2]]
for p in logs:
 s=p.read_text();assert s.count('PASS ')==10 and json.loads(s.splitlines()[-1])['passed']==10
review=dict(repository='mighty-justice/django-super-deduper',commit=d['commit']['sha'],tree=d['commit']['tree'],status='complete_source_read_with_explicit_orm_adapter_controls',whole_repository_complete=True,license='MIT LICENSE.md fully read; no production code adopted',scope='All22 tracked text blobs and4 trees including root read and independently Git-object verified. Whole merge/meta modules,21 authored upstream test methods, factories/models/conftest, README, license, buildkite/docker/setup/config/dependencies. No tracked raster/media; external README status/version badges and reference websites excluded from executable/visual product review.',blobs=rows,source_archive=str(P.relative_to(R)),source_archive_sha256=h(P),runtime=dict(adapter='Unchanged class AST with explicit in-memory ModelMeta/ORM/serialization adapters',controls_per_run=10,runs=2,upstream_tests_run=False,limits='No Django dependency installed, actual database/SQL transaction/concurrency, package install or upstream suite run. Initial adapter missing _meta failure preserved; v2 only adds required adapter metadata, upstream class unchanged.',logs=[dict(path=str(p.relative_to(R)),sha256=h(p)) for p in logs]),findings=['Alias deletion precedes primary save; failure has no local rollback wrapper. Earlier aliases remain modified if later alias fails.', 'M2M removal precedes add; add failure leaves link removed and no audit append.', 'O2O alias detachment saves before primary save.', 'Nonnullable reverse uniqueness conflict defaults to deleting related object; explicit raise option avoids deletion.', 'Debug serialize calls eagerly evaluated even if logger level disables debug.'],application='Use failure checkpoints, caller transaction ownership and append-only outcomes before destructive action; guard expensive diagnostic serialization. Domain is record merging, not image similarity; no source copied or measured library speedup.')
p=D/'django136-complete-source-review-20261015.json';assert not p.exists();p.write_text(json.dumps(review,indent=2)+'\n');print('Verified22blobs4trees,2x10adapter controls; not database qualification')
