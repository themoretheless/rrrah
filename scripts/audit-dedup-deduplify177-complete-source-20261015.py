"""Verify30 tracked text blobs,8canonical trees and actual native test/dependency evidence."""
from pathlib import Path
import ast,hashlib,json
R=Path(__file__).resolve().parents[1];D=R/'docs/research'
h=lambda p:hashlib.sha256(Path(p).read_bytes()).hexdigest()
helper=R/'scripts/audit-dedup-five-complete-source-trees-20261015.py'
ns={'Path':Path,'hashlib':hashlib};parsed=ast.parse(helper.read_text())
exec(compile(ast.Module(body=[n for n in parsed.body if isinstance(n,ast.FunctionDef)],type_ignores=[]),'canonical-tree-functions','exec'),ns)
assert ns['git_object']('tree',b'')=='4b825dc642cb6eb9a060e54bf8d69288fbee4904'
registry_path=D/'dedup-500/reviews/177.json';registry=json.loads(registry_path.read_text());review_path=R/registry['complete_source_review'];review=json.loads(review_path.read_text());pinpath=D/'deduplify177-commit-tree-pin-20261015.json';pin=json.loads(pinpath.read_text())
assert registry['full_repository_read'] and review['whole_repository_complete']
assert registry['repository']==review['repository']==pin['repository']=='Living-with-machines/deduplify'
assert registry['commit']==review['commit']==pin['commit']
base=D/'dedup-500/evidence/Living-with-machines--deduplify';treepath=base/'tree.json';tree=json.loads(treepath.read_text());assert tree['truncated'] is False and tree['sha']==pin['tree']==review['tree']
built=ns['reconstruct'](tree['tree']);assert len(built)==8 and built['']==pin['tree']
entries=[e for e in tree['tree'] if e['type']=='blob'];assert len(entries)==30
assert {str(p.relative_to(base/'full-source')) for p in (base/'full-source').rglob('*') if p.is_file()}=={e['path'] for e in entries}
assert pin['blobs']==review['blobs'];blobs={b['path']:b for b in pin['blobs']};assert len(blobs)==30
for e in entries:
 p=base/'full-source'/e['path'];raw=p.read_bytes();b=blobs[e['path']]
 assert e['size']==b['bytes']==len(raw) and e['mode']==b['mode']
 assert ns['git_object']('blob',raw)==e['sha']==b['git_blob'] and h(p)==b['sha256']
omission_rejected=False
try: omission_rejected=ns['reconstruct'](tree['tree'][:-1])['']!=pin['tree']
except AssertionError: omission_rejected=True
assert omission_rejected
raw=(base/'full-source'/entries[0]['path']).read_bytes();changed=bytes([raw[0]^1])+raw[1:];assert ns['git_object']('blob',changed)!=entries[0]['sha']
controls=R/review['controls'];assert h(controls)==review['controls_sha256'];runtime=json.loads(controls.read_text());assert runtime['tests']==16 and runtime['original_tests']==7 and runtime['authored_tests']==9
for artifact in runtime['artifacts']:assert h(R/artifact['path'])==artifact['sha256']
import re
E=D/'deduplify177-native-controls-20261015'
names=set(re.findall(r'^(tests/[^\s]+::[^\s]+)$',(E/'original-inventory.log').read_text(),re.M));assert len(names)==7
for filename in ['original-run.log','original-replay.log']:
 rows=re.findall(r'^(tests/\S+) (PASSED|FAILED|ERROR|SKIPPED|XFAIL|XPASS)\b',(E/filename).read_text(),re.M)
 assert len(rows)==7 and {name for name,status in rows}==names and all(status=='PASSED' for name,status in rows)
for filename in ['authored-run-v2.log','authored-replay-v2.log']:
 body=(E/filename).read_text();assert 'Ran 9 tests' in body and body.rstrip().endswith('OK')
 assert len(set(re.findall(r'^(test_\S+) \(__main__',body,re.M)))==9
payload=json.loads((E/'dependency-pins.json').read_text());assert len(payload['distributions'])==14
for row in payload['distributions']:assert row['url'].startswith('https://files.pythonhosted.org/') and row['url'].endswith('.whl') and re.fullmatch('[0-9a-f]{64}',row['archive_hashes']['sha256'])
for row in payload['files']:assert h(Path(payload['root'])/row['path'])==row['sha256']
for path,digest in json.loads((E/'source-pins.json').read_text()).items():assert h(Path(runtime['source'])/path)==digest

out=D/'dedup-deduplify177-complete-source-tree-audit-20261015.json';assert not out.exists();out.write_text(json.dumps(dict(status='verified30_blobs8_trees_original7_authored9_native_evidence',repository=pin['repository'],commit=pin['commit'],tree=pin['tree'],blobs_verified=30,trees_verified=8,missing_blob_rejected=True,changed_blob_rejected=True,registry_sha256=h(registry_path),review_sha256=h(review_path),pin_sha256=h(pinpath),auditor_sha256=h(__file__),canonical_helper_sha256=h(helper),limits=['Fullarchive/Gitobject/source/control-artifact identity, not automated understanding, complete backend execution or quality/scale/performance proof.','Alloriginal7tests and9authoredcontrols twice withactualcoredependencies onMacARM/Python3.12. Not oldCI/fullsetup/publishing, userdatasetpurge, mutationrace/quality/scale/performance proof.']),indent=2)+'\n');print('30blobs,8trees,original7/authored9/dependency evidence verified')
