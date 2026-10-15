"""Independently verify selected native fingerprint evidence; no full repository claim."""
from pathlib import Path
import hashlib,json,re,subprocess
R=Path(__file__).resolve().parents[1];D=R/'docs/research/dedup-500';W=R/'.dedup-work/dupe60-20261016'
h=lambda p:hashlib.sha256(p.read_bytes()).hexdigest()
p=D/'dupe60-selected-fingerprint-review-20261016.json';d=json.loads(p.read_text());assert not d['whole_repo_complete']
for path,sha in d['evidence_pins'].items():assert h(R/path)==sha
f=json.loads((D/'dupe60-fetched-source-pins-20261016.json').read_text());assert len(f['files'])==97 and f['verified_trees']==18
for x in f['files']:
 b=(W/'source'/x['path']).read_bytes();assert len(b)==x['bytes'] and hashlib.sha1(b'blob '+str(len(b)).encode()+b'\0'+b).hexdigest()==x['git_blob'] and hashlib.sha256(b).hexdigest()==x['sha256']
 if 'content'in x:assert b.decode()==x['content']
s=(W/'source/src/config.rs').read_text();assert re.search(r'pub const KGRAM: usize = 10;',s) and re.search(r'pub const WINDOW: usize = 8;',s)
original=(W/'source/src/fingerprint.rs').read_text();assert original.count('#[test]')==5
inv=subprocess.check_output([str(W/'fingerprint-tests'),'--list'],text=True);assert inv==(W/'fingerprint-inventory.log').read_text() and '8 tests, 0 benchmarks' in inv
for i in [1,2]:assert '8 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out' in (W/f'fingerprint-run{i}.log').read_text()
out=D/'dupe60-selected-fingerprint-audit-20261016.json';assert not out.exists();out.write_text(json.dumps(dict(status='verified_selected_native_evidence',whole_repo_complete=False,review=str(p.relative_to(R)),review_sha256=h(p),auditor_sha256=h(Path(__file__)),source_blobs=97,source_trees=18,fully_read_files=d['fully_read_files'],selected_original_tests=5,additional_controls=3,passed_per_run=8,runs=2,oracle_queries_per_run=246025,limits=d['limits']),indent=2)+'\n');print('verified97blobpins and selected8nativecontrols twice; fullreview incomplete')
