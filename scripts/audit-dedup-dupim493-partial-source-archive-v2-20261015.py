"""Verify8blob archive identity; semantic/source implementation review remains partial."""
from pathlib import Path
import ast,hashlib,json
R=Path(__file__).resolve().parents[1];D=R/'docs/research'
h=lambda p:hashlib.sha256(Path(p).read_bytes()).hexdigest()
ns={'Path':Path,'hashlib':hashlib}
tree=ast.parse((R/'scripts/audit-dedup-five-complete-source-trees-20261015.py').read_text())
exec(compile(ast.Module(body=[n for n in tree.body if isinstance(n,ast.FunctionDef)],type_ignores=[]),'canonical-tree-functions','exec'),ns)
assert ns['git_object']('tree',b'')=='4b825dc642cb6eb9a060e54bf8d69288fbee4904'
rows=[]
for index,slug in [(493,'dupim493')]:
 registry_path=D/'dedup-500/reviews'/f'{index:03}.json';registry=json.loads(registry_path.read_text())
 pinpath=D/f'{slug}-commit-tree-pin-20261015.json';pin=json.loads(pinpath.read_text())
 review_path=D/'dupim493-available-source-and-opaque-component-review-20261015.json';review=json.loads(review_path.read_text())
 assert not registry['full_repository_read'] and not review['whole_repository_complete'] and review['available_text_source_complete']
 assert registry['repository']==review['repository']==pin['repository']
 assert registry['commit']==review['commit']==pin['commit']
 base=D/'dedup-500/evidence'/registry['repository'].replace('/','--')
 treepath=base/'tree.json';tree=json.loads(treepath.read_text());assert tree['truncated'] is False
 assert tree['sha']==pin['tree']==review['tree']
 built=ns['reconstruct'](tree['tree']);assert built['']==pin['tree']
 entries=[e for e in tree['tree'] if e['type']=='blob'];expected={e['path'] for e in entries}
 assert {str(p.relative_to(base/'full-source')) for p in (base/'full-source').rglob('*') if p.is_file()}==expected
 assert pin['blobs']==review['blobs'];blobs={b['path']:b for b in pin['blobs']};assert len(blobs)==len(entries) and set(blobs)==expected
 for e in entries:
  p=base/'full-source'/e['path'];raw=p.read_bytes();b=blobs[e['path']]
  assert len(raw)==e['size']==b['bytes'] and h(p)==b['sha256'] and e['mode']==b['mode']
  assert ns['git_object']('blob',raw)==e['sha']==b['git_blob']
 rejected=False
 try: rejected=ns['reconstruct']([e for e in tree['tree'] if e is not entries[-1]])['']!=pin['tree']
 except AssertionError:rejected=True
 assert rejected
 rows.append(dict(index=index,repository=pin['repository'],commit=pin['commit'],tree=pin['tree'],blobs=len(entries),trees=len(built),pin_sha256=h(pinpath),review_sha256=h(review_path),registry_sha256=h(registry_path),missing_blob_rejected=rejected))
assert sum(r['blobs'] for r in rows)==8 and sum(r['trees'] for r in rows)==4
output=D/'dedup-dupim493-partial-source-archive-audit-v2-20261015.json';assert not output.exists()
output.write_text(json.dumps(dict(status='verified8_blobs4_trees_archive_identity_only_semantic_review_partial',rows=rows,auditor_sha256=h(__file__),canonical_helper_sha256=h(R/'scripts/audit-dedup-five-complete-source-trees-20261015.py'),limits=['Archive/commit identity and inventories, not automated source understanding.','Fresh public GitHub commit/tree pin observations retained. Runtime/adapters explicitly scoped in per-repository review; archive completeness does not establish backend/quality or opaque binary semantic review.']),indent=2)+'\n')
print('8 blobs,4 trees,archive omission control verified;semantic review partial')
