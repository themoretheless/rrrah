"""Verify selected native index/cluster/score controls without asserting whole repository coverage."""
from pathlib import Path
import hashlib,json,subprocess
R=Path(__file__).resolve().parents[1];D=R/'docs/research/dedup-500';W=R/'.dedup-work/dupe60-20261016';h=lambda p:hashlib.sha256(Path(p).read_bytes()).hexdigest()
p=D/'dupe60-selected-index-review-20261016.json';d=json.loads(p.read_text());assert not d['whole_repo_complete'] and d['original_tests']==15 and d['additional_controls']==8 and d['runs']==2
for name,sha in d['evidence_pins'].items():assert h(R/name)==sha
assert d['producer_sha256']==h(R/'scripts/qualify-dupe60-selected-index-20261016.py') and d['harness_source']==(W/'index-harness.rs').read_text()
f=json.loads((D/'dupe60-fetched-source-pins-20261016.json').read_text());assert len(f['files'])==97 and len(d['fully_read_files'])==12
for x in f['files']:
 b=(W/'source'/x['path']).read_bytes();assert len(b)==x['bytes'] and hashlib.sha1(b'blob '+str(len(b)).encode()+b'\0'+b).hexdigest()==x['git_blob'] and hashlib.sha256(b).hexdigest()==x['sha256']
inv=subprocess.check_output([str(W/'index-tests-locked-hash'),'--list'],text=True);assert inv==(W/'index-locked-inventory.log').read_text() and '23 tests, 0 benchmarks' in inv
names=[s for s in inv.splitlines() if s.endswith(': test')];assert len(names)==23 and sum(s.startswith('controls::') for s in names)==8
assert {m:sum(s.startswith(m+'::') for s in names) for m in ['fingerprint','index','cluster','score']}==dict(fingerprint=5,index=3,cluster=4,score=3)
for i in [1,2]:assert '23 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out' in (W/f'index-locked-run{i}.log').read_text()
for i in [1,2]:assert not (W/f'index-locked-build{i}.log').read_text()
out=D/'dupe60-selected-index-audit-20261016.json';assert not out.exists();out.write_text(json.dumps(dict(status='verified_selected_whole_module_native_evidence',whole_repo_complete=False,review=str(p.relative_to(R)),review_sha256=h(p),producer_sha256=d['producer_sha256'],auditor_sha256=h(__file__),source_blobs=97,fully_read_files=d['fully_read_files'],original_tests=15,additional_controls=8,passed_per_run=23,runs=2,observations=d['observations'],dependencies=d['dependencies'],adapter_scope=d['adapter_scope'],limits=d['limits']),indent=2)+'\n');print('97sourcepins reverified;23nativecontrols twice;12/97filesread; nofullcountincrease')
