"""Verify all13 original blobs,4trees and selected-body control artifact identities."""
from pathlib import Path
import ast,hashlib,json
R=Path(__file__).resolve().parents[1];D=R/'docs/research';h=lambda p:hashlib.sha256(Path(p).read_bytes()).hexdigest();helper=R/'scripts/audit-dedup-five-complete-source-trees-20261015.py';ns={'Path':Path,'hashlib':hashlib};exec(compile(ast.Module(body=[n for n in ast.parse(helper.read_text()).body if isinstance(n,ast.FunctionDef)],type_ignores=[]),'canonical-tree','exec'),ns)
regpath=D/'dedup-500/reviews/380.json';reg=json.loads(regpath.read_text());reviewpath=R/reg['complete_source_review'];review=json.loads(reviewpath.read_text());pinpath=D/'scene380-commit-tree-pin-20261015.json';pin=json.loads(pinpath.read_text());base=D/'dedup-500/evidence/VieVie31--scene_detection';tree=json.loads((base/'tree.json').read_text());assert not tree['truncated'];built=ns['reconstruct'](tree['tree']);assert len(built)==4 and built['']==pin['tree']==review['tree'];assert reg['commit']==pin['commit']==review['commit'] and reg['full_repository_read'] and review['whole_repository_complete'];assert pin['blobs']==review['blobs'] and h(pinpath)==review['pin_sha256'];entries=[e for e in tree['tree'] if e['type']=='blob'];assert len(entries)==13;assert {str(p.relative_to(base/'full-source')) for p in (base/'full-source').rglob('*') if p.is_file()}=={e['path'] for e in entries}
for e in entries:
 p=base/'full-source'/e['path'];b=next(b for b in pin['blobs'] if b['path']==e['path']);raw=p.read_bytes();assert e['mode']==b['mode'] and len(raw)==e['size']==b['bytes'];assert ns['git_object']('blob',raw)==e['sha']==b['git_blob'] and h(p)==b['sha256']
missing_rejected=False
try:missing_rejected=ns['reconstruct'](tree['tree'][:-1])['']!=pin['tree']
except AssertionError:missing_rejected=True
assert missing_rejected
p=base/'full-source/README.md';assert ns['git_object']('blob',p.read_bytes()+b'!')!=next(e['sha'] for e in entries if e['path']=='README.md')
cp=R/review['controls'];assert h(cp)==review['controls_sha256'];runtime=json.loads(cp.read_text());assert runtime['cases']==12 and runtime['repeats']==2
for a in runtime['artifacts']:assert h(R/a['path'])==a['sha256']
for name,sha in runtime['source_sha256'].items():assert h(R/name)==sha
for name in ['run.log','replay.log']:
 lines=(cp.parent/name).read_text().splitlines();assert sum(x.startswith('PASS ') for x in lines)==12;row=json.loads(lines[-1]);assert row['case_names']==runtime['case_names'] and row['cases']==12
out=D/'dedup-scene380-complete-source-tree-audit-20261015.json';assert not out.exists();out.write_text(json.dumps(dict(status='verified13_blobs4_trees12_selected_controls_twice',blobs_verified=13,trees_verified=4,missing_blob_rejected=True,changed_blob_rejected=True,registry_sha256=h(regpath),review_sha256=h(reviewpath),pin_sha256=h(pinpath),auditor_sha256=h(__file__),limits=['Archive identity and selected unchanged function controls only, not full video backend/runtime/quality/performance proof.']),indent=2)+'\n');print('13blobs4trees12controls twice verified')
