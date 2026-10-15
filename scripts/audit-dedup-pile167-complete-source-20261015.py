"""Verify13 tracked text blobs and one canonical tree; source read is not backend qualification."""
from pathlib import Path
import ast,hashlib,json
R=Path(__file__).resolve().parents[1];D=R/'docs/research'
h=lambda p:hashlib.sha256(Path(p).read_bytes()).hexdigest()
helper=R/'scripts/audit-dedup-five-complete-source-trees-20261015.py'
ns={'Path':Path,'hashlib':hashlib};parsed=ast.parse(helper.read_text())
exec(compile(ast.Module(body=[n for n in parsed.body if isinstance(n,ast.FunctionDef)],type_ignores=[]),'canonical-tree-functions','exec'),ns)
assert ns['git_object']('tree',b'')=='4b825dc642cb6eb9a060e54bf8d69288fbee4904'
registry_path=D/'dedup-500/reviews/167.json';registry=json.loads(registry_path.read_text());review_path=R/registry['complete_source_review'];review=json.loads(review_path.read_text());pinpath=D/'pile167-commit-tree-pin-20261015.json';pin=json.loads(pinpath.read_text())
assert registry['full_repository_read'] and review['whole_repository_complete']
assert registry['repository']==review['repository']==pin['repository']=='EleutherAI/pile_dedupe'
assert registry['commit']==review['commit']==pin['commit']
base=D/'dedup-500/evidence/EleutherAI--pile_dedupe';treepath=base/'tree.json';tree=json.loads(treepath.read_text());assert tree['truncated'] is False and tree['sha']==pin['tree']==review['tree']
built=ns['reconstruct'](tree['tree']);assert len(built)==1 and built['']==pin['tree']
entries=[e for e in tree['tree'] if e['type']=='blob'];assert len(entries)==13
assert {str(p.relative_to(base/'full-source')) for p in (base/'full-source').rglob('*') if p.is_file()}=={e['path'] for e in entries}
assert pin['blobs']==review['blobs'];blobs={b['path']:b for b in pin['blobs']};assert len(blobs)==13
for e in entries:
 p=base/'full-source'/e['path'];raw=p.read_bytes();b=blobs[e['path']]
 assert e['size']==b['bytes']==len(raw) and e['mode']==b['mode']
 assert ns['git_object']('blob',raw)==e['sha']==b['git_blob'] and h(p)==b['sha256']
assert ns['reconstruct'](tree['tree'][:-1])['']!=pin['tree']
raw=(base/'full-source'/entries[0]['path']).read_bytes();changed=bytes([raw[0]^1])+raw[1:];assert ns['git_object']('blob',changed)!=entries[0]['sha']
controls=R/review['controls'];assert h(controls)==review['controls_sha256'];runtime=json.loads(controls.read_text());assert runtime['tests']==11
for artifact in runtime['artifacts']:assert h(R/artifact['path'])==artifact['sha256']
out=D/'dedup-pile167-complete-source-tree-audit-20261015.json';assert not out.exists();out.write_text(json.dumps(dict(status='verified13_blobs1_tree_archive_and_control_artifact_pins',repository=pin['repository'],commit=pin['commit'],tree=pin['tree'],blobs_verified=13,trees_verified=1,missing_blob_rejected=True,changed_blob_rejected=True,registry_sha256=h(registry_path),review_sha256=h(review_path),pin_sha256=h(pinpath),auditor_sha256=h(__file__),canonical_helper_sha256=h(helper),limits=['Fullarchive/Gitobject/source/control-artifact identity, not automated understanding, complete backend execution or quality/scale/performance proof.','Original selectedbody tests plus explicit adapters, no actual Pile/NLTK/datasketch/compressedReader/multiprocess pipeline.']),indent=2)+'\n');print('13blobs,1tree,omission/corruption controls andruntime artifact pins verified')
