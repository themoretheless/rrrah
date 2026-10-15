"""Verify25 tracked blobs,8trees,visual records and preserved native UBSan failures."""
from pathlib import Path
import ast,hashlib,json
R=Path(__file__).resolve().parents[1];D=R/'docs/research'
h=lambda p:hashlib.sha256(Path(p).read_bytes()).hexdigest()
helper=R/'scripts/audit-dedup-five-complete-source-trees-20261015.py'
ns={'Path':Path,'hashlib':hashlib};parsed=ast.parse(helper.read_text())
exec(compile(ast.Module(body=[n for n in parsed.body if isinstance(n,ast.FunctionDef)],type_ignores=[]),'canonical-tree-functions','exec'),ns)
assert ns['git_object']('tree',b'')=='4b825dc642cb6eb9a060e54bf8d69288fbee4904'
registry_path=D/'dedup-500/reviews/384.json';registry=json.loads(registry_path.read_text());review_path=R/registry['complete_source_review'];review=json.loads(review_path.read_text());pinpath=D/'exper384-commit-tree-pin-20261015.json';pin=json.loads(pinpath.read_text())
assert registry['full_repository_read'] and review['whole_repository_complete']
assert registry['repository']==review['repository']==pin['repository']=='kemonomachi/experhash'
assert registry['commit']==review['commit']==pin['commit']
base=D/'dedup-500/evidence/kemonomachi--experhash';treepath=base/'tree.json';tree=json.loads(treepath.read_text());assert tree['truncated'] is False and tree['sha']==pin['tree']==review['tree']
built=ns['reconstruct'](tree['tree']);assert len(built)==8 and built['']==pin['tree']
entries=[e for e in tree['tree'] if e['type']=='blob'];assert len(entries)==25
assert {str(p.relative_to(base/'full-source')) for p in (base/'full-source').rglob('*') if p.is_file()}=={e['path'] for e in entries}
assert pin['blobs']==review['blobs'];blobs={b['path']:b for b in pin['blobs']};assert len(blobs)==25
for e in entries:
 p=base/'full-source'/e['path'];raw=p.read_bytes();b=blobs[e['path']]
 assert e['size']==b['bytes']==len(raw) and e['mode']==b['mode']
 assert ns['git_object']('blob',raw)==e['sha']==b['git_blob'] and h(p)==b['sha256']
rejected=False
try: rejected=ns['reconstruct'](tree['tree'][:-1])['']!=pin['tree']
except AssertionError: rejected=True
assert rejected
raw=(base/'full-source'/entries[0]['path']).read_bytes();changed=bytes([raw[0]^1])+raw[1:];assert ns['git_object']('blob',changed)!=entries[0]['sha']
controls=R/review['controls'];assert h(controls)==review['controls_sha256'];runtime=json.loads(controls.read_text());assert runtime['native_cases_per_run']==12 and runtime['native_passed_per_run']==9 and runtime['native_failed_per_run']==3 and runtime['repeats']==2
for artifact in runtime['artifacts']:assert h(R/artifact['path'])==artifact['sha256']
import re
E=D/'exper384-hash-adapter-controls-20261015'
assert h(runtime['binary'])==runtime['binary_sha256']
rows=json.loads((E/'verified-results.json').read_text());assert len(rows)==24
for repeat in ['run','replay']:
 part=[row for row in rows if row['repeat']==repeat]
 assert {(row['algorithm'],row['width']) for row in part}=={(a,w) for a in ['a','d','dd'] for w in [8,16,32,64]}
 assert sum(row['returncode']==0 for row in part)==9 and sum(row['returncode']!=0 for row in part)==3
 for row in part:
  assert h(row['log'])==row['log_sha256']
  if row['width']==64:assert row['returncode']!=0 and "shift exponent 63 is too large for 32-bit type 'int'" in Path(row['log']).read_text()
  else:assert row['returncode']==0
visual=json.loads((R/review['visuals']).read_text());assert len(visual['visuals'])==5
for row in visual['visuals']:assert row['visually_viewed'] and h(R/row['source'])==row['source_sha256'] and h(R/row['preview'])==row['preview_sha256']
distance=json.loads((R/review['distance_table']).read_text());assert h(R/distance['source'])==distance['source_sha256'] and distance['byte_pair_oracles']==65536
literal=(R/distance['source']).read_text().split('defp lookup_map() do',1)[1];values={int(a):int(b) for a,b in re.findall(r'^\s*(\d+)\s*=>\s*(\d+),?\s*$',literal,re.M)};assert len(values)==256
for a in range(256):
 for b in range(256):assert values[a^b]==bin(a^b).count('1')

out=D/'dedup-exper384-complete-source-tree-audit-20261015.json';assert not out.exists();out.write_text(json.dumps(dict(status='verified25_blobs8_trees_svg_records9_pass3_original_ubsan_fail_twice',repository=pin['repository'],commit=pin['commit'],tree=pin['tree'],blobs_verified=25,trees_verified=8,missing_blob_rejected=True,changed_blob_rejected=True,registry_sha256=h(registry_path),review_sha256=h(review_path),pin_sha256=h(pinpath),auditor_sha256=h(__file__),canonical_helper_sha256=h(helper),limits=['Fullarchive/Gitobject/source/control-artifact identity, not automated understanding, complete backend execution or quality/scale/performance proof.','Originalhashcode withpixelIOadapter only; width64originalfailures preserved. No actualElixir/ErlangEI/Magick++golden/runtime/quality/performance qualification.']),indent=2)+'\n');print('25blobs8trees,5visualrecords,9pass3UBSanfail eachrun verified')
