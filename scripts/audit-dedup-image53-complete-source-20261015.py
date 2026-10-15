"""Verify8 blobs/2trees, real-backend15controls twice and six scheduled IPC losses."""
from pathlib import Path
import ast,hashlib,json,zipfile,csv,base64
R=Path(__file__).resolve().parents[1];D=R/'docs/research';h=lambda p:hashlib.sha256(Path(p).read_bytes()).hexdigest();helper=R/'scripts/audit-dedup-five-complete-source-trees-20261015.py';ns={'Path':Path,'hashlib':hashlib};exec(compile(ast.Module(body=[n for n in ast.parse(helper.read_text()).body if isinstance(n,ast.FunctionDef)],type_ignores=[]),'canonical','exec'),ns)
regpath=D/'dedup-500/reviews/053.json';reg=json.loads(regpath.read_text());rp=R/reg['complete_source_review'];review=json.loads(rp.read_text());pinpath=D/'image53-commit-tree-pin-20261015.json';pin=json.loads(pinpath.read_text());base=D/'dedup-500/evidence/chinalu--imagedups';tree=json.loads((base/'tree.json').read_text());assert not tree['truncated'];built=ns['reconstruct'](tree['tree']);assert len(built)==2 and built['']==pin['tree']==review['tree'];assert reg['commit']==pin['commit']==review['commit'];assert reg['full_repository_read'] and review['whole_repository_complete'];assert pin['blobs']==review['blobs'] and h(pinpath)==review['pin_sha256'];entries=[e for e in tree['tree'] if e['type']=='blob'];assert len(entries)==8;assert {str(p.relative_to(base/'full-source')) for p in (base/'full-source').rglob('*') if p.is_file()}=={e['path'] for e in entries}
for e in entries:
 p=base/'full-source'/e['path'];b=next(b for b in pin['blobs'] if b['path']==e['path']);raw=p.read_bytes();assert e['mode']==b['mode'] and len(raw)==e['size']==b['bytes'];assert ns['git_object']('blob',raw)==e['sha']==b['git_blob'] and h(p)==b['sha256'];assert h(Path('/private/tmp/rrrah-image53-native-20261015')/e['path'])==b['sha256']
missing=False
try:missing=ns['reconstruct'](tree['tree'][:-1])['']!=pin['tree']
except AssertionError:missing=True
assert missing;assert ns['git_object']('blob',(base/'full-source/README.md').read_bytes()+b'!')!=next(e['sha'] for e in entries if e['path']=='README.md')
cp=R/review['controls'];assert h(cp)==review['controls_sha256'];runtime=json.loads(cp.read_text());assert runtime['cases']==15 and runtime['repeats']==2;assert runtime['source_sha256']==h(base/'full-source/src/imagedups.py')
for a in runtime['artifacts']:assert h(R/a['path'])==a['sha256']
for name in ['run.log','replay.log']:
 lines=(cp.parent/name).read_text().splitlines();passed=[l[5:] for l in lines if l.startswith('PASS ')];assert passed==runtime['case_names'] and len(passed)==15;row=json.loads(lines[-1]);assert row['source_sha256']==runtime['source_sha256'] and row['wheel_sha256']==runtime['wheel_sha256'];assert row['observations']['repeated_root']==dict(input_files=1,remaining_files=0);assert row['observations']['flat256']==dict(distinct_gray_pixels=256,hashes=1);assert row['observations']['stale_deletion']==dict(new_distinct_hash='0f0f0f0f0f0f0f0f',changed_file_deleted=True);assert len(row['observations']['lost_update'])==3
 for race in row['observations']['lost_update']:assert race==dict(processes=2,read_snapshot_lengths=[0,0],published_lengths=[1,1],final_members=1)
deps=json.loads((cp.parent/'dependency-pins.json').read_text());assert len(deps['wheels'])==3 and len(deps['installed'])==3
for w in deps['wheels']:
 p=Path(w['path']);assert h(p)==w['sha256']==runtime['wheel_sha256'][str(p)] and p.stat().st_size==w['bytes'];z=zipfile.ZipFile(p);assert z.testzip() is None;record=next(n for n in z.namelist() if n.endswith('.dist-info/RECORD'));count=0
 for name,digest,size in csv.reader(z.read(record).decode().splitlines()):
  if not digest:continue
  algorithm,want=digest.split('=',1);raw=z.read(name);assert algorithm=='sha256' and base64.urlsafe_b64encode(hashlib.sha256(raw).digest()).decode().rstrip('=')==want and len(raw)==int(size);count+=1
 assert count==w['record_entries_verified']
for d in deps['installed']:
 for p in d['files']:assert h(p['path'])==p['sha256']
assert 'PermissionError: [Errno 1] Operation not permitted' in (cp.parent/'sandbox-ipc-preflight.log').read_text();assert 'no such file' in (cp.parent/'wrong-cwd-preflight.log').read_text().lower() or "can't open file" in (cp.parent/'wrong-cwd-preflight.log').read_text()
crossp=R/review['root_crosswalk'];assert h(crossp)==review['root_crosswalk_sha256'];cross=json.loads(crossp.read_text());assert cross['current_source_matches_all'] and len(cross['rows'])==9
for p in cross['current_source_pins']:assert h(R/p['path'])==p['current_sha256']==p['qualified_sha256'] and p['matches']
for p in cross['qualified_evidence']:assert h(R/p['path'])==p['sha256']
out=D/'dedup-image53-complete-source-tree-audit-20261015.json';assert not out.exists();out.write_text(json.dumps(dict(status='verified8_blobs2_trees15_native_controls_twice6_scheduled_lost_updates',repository=pin['repository'],commit=pin['commit'],tree=pin['tree'],blobs_verified=8,trees_verified=2,missing_blob_rejected=True,changed_blob_rejected=True,registry_sha256=h(regpath),review_sha256=h(rp),pin_sha256=h(pinpath),auditor_sha256=h(__file__),limits=['Native diagnosticcohort/explicitscheduler/CPUcount1 andownedFS only, not completehistoricalpackage/defaultwholePool/corpus/platform/resource/performance/adoption proof.','Rootcrosswalk verifiesboundedexistingevidence/currentmoduleidentity, notnewruntimegate/fullcollection/frame/destructivepipeline qualification.']),indent=2)+'\n');print('8blobs2trees15controls twice6nativeIPC lostupdates verified')
