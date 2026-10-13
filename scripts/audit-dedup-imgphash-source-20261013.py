#!/usr/bin/env python3
"""Verify all eight pinned upstream blobs and twelve unchanged-library controls."""
import hashlib
import json
import re
import shutil
from pathlib import Path
R=Path(__file__).resolve().parents[1]
D=R/'docs/research'
E=D/'dedup-500/evidence/A5M1--imgphash'
T=Path('/Users/themoretheless/.codex/tmp')
S=T/'rrrah-imgphash-full-source-20261013'
H=T/'rrrah-imgphash-controls-20261013'
sha=lambda p:hashlib.sha256(Path(p).read_bytes()).hexdigest()
commit=json.loads((E/'commit-20261013.json').read_text())
tree=json.loads((E/'tree.json').read_text())
assert commit['sha']=='89061afa98da7ec11b885afe786aed6e56f62f3c' and commit['tree']['sha']==tree['sha'] and not tree['truncated']
rows=[]
for entry in tree['tree']:
    if entry['type']!='blob':continue
    p=S/entry['path'];data=p.read_bytes()
    assert hashlib.sha1(b'blob '+str(len(data)).encode()+b'\0'+data).hexdigest()==entry['sha']
    rows.append(dict(path=entry['path'],git_blob=entry['sha'],sha256=sha(p),bytes=len(data),lines=len(data.splitlines()),fully_read=True))
assert len(rows)==8
assert (S/'src/lib.rs').read_bytes()==(E/'02-lib.rs').read_bytes()
upstream=T/'rrrah-imgphash-upstream-online-20261013.log'
assert re.findall(r'test result: ok\. (\d+) passed; (\d+) failed; (\d+) ignored;',upstream.read_text())==[('0','0','0'),('0','0','0')]
log=T/'rrrah-imgphash-controls-20261013.log';text=log.read_text()
assert re.findall(r'test result: ok\. (\d+) passed; (\d+) failed; (\d+) ignored;',text)==[('12','0','0'),('0','0','0')]
controls=re.findall(r'^test controls::(\w+) \.\.\. ok$',text,re.M);assert len(controls)==12
binary=Path(re.search(r'Running unittests src/lib.rs \(([^)]+)\)',text).group(1))
dest=T/'rrrah-imgphash-qualified-20261013';assert not dest.exists();dest.mkdir()
output=dest/'controls';shutil.copy2(binary,output);assert sha(output)==sha(binary)
pins={str(p):sha(p) for p in [E/'tree.json',E/'commit-20261013.json',S/'Cargo.lock',H/'Cargo.toml',H/'Cargo.lock',H/'src/lib.rs',upstream,log,output,Path(__file__)]}
result=dict(status='verified_full8_blob_source_read_and12_selected_controls',repository='A5M1/imgphash',commit=commit['sha'],source=str(S),files=rows,whole_repo_complete=True,whole_implementation_read=True,license='MIT; full LICENSE read, no code adopted',upstream_tests_run=True,upstream_actual_tests=0,selected_controls_passed=12,controls=controls,pins=pins,
    findings=dict(runtime=['invalid hex equals empty at distance0','unequal lengths ignore nonmatching trailing byte','Unicode parser panic','flat parser skips invalid chars','empty MultiHash violates reflexivity','MultiHash equality violates transitivity and symmetry','empty invalid segment produces NaN score','multiple segments reuse one counterpart (directional behavior)','remove_max_haar_ll zeros all details and LL; distinct images both zero, retained-details control differs','zero highfreq factor panics','valid average_hash64-bit positive control differs for inverse gradient'],
                  static_only=['C wrappers lack buffer length and accept raw pointers through safe public Rust functions; no hostile pointer calls executed','header average_hash mentions RGBA/RGB but implementation reads RGB24 only; README correctly specifies RGB24','no admission/cancellation contract in algorithms; unbounded dimensions/bit widths not exhaustively executed']),
    scope='All8 repository blobs text read, Git-object/commit-tree checked. Upstream cargo test executed0 tests. External12-control harness links unchanged upstream library; no C FFI execution, broad corpus quality/performance/unsafe-memory proof, or all500 completion.')
path=D/'dedup-500/imgphash-full-source-audit-20261013.json';assert not path.exists();path.write_text(json.dumps(result,indent=2)+'\n');print(result['status'])
