#!/usr/bin/env python3
"""Record four complete text reads; leave remaining source and binary assets explicit."""
import hashlib,json
from pathlib import Path
R=Path(__file__).resolve().parents[1];D=R/'docs/research/dedup-500';S=Path('/Users/themoretheless/.codex/tmp/rrrah-skiff-source-20261012')
def sha(p):return hashlib.sha256(Path(p).read_bytes()).hexdigest()
parent=D/'skiff-full-file-prefix-097-agent-review-20261012.json';p=json.loads(parent.read_text());assert all(sha(f)==v for f,v in p['pins'].items())
module=D/'skiff-exact-module-source-runtime-review-20261012.json';m=json.loads(module.read_text())
new=['README.zh-CN.md','index.html','src/assets/skiff-logo.svg','src/App.css'];read=sorted(set(p['full_files_read']+new))
assert len(read)==101 and all(sha(S/n)==m['archive_inventory'][n]['sha256'] for n in read)
out=D/'skiff-full-file-prefix-101-doc-style-review-20261012.json';assert not out.exists()
out.write_text(json.dumps(dict(status='verified_complete_text_prefix_not_full_repository',repository=m['repository'],commit=m['commit'],fully_read_file_count=len(read),total_files=129,full_files_read=read,new_full_files_read=new,remaining_files_not_fully_reviewed=sorted(set(m['archive_inventory'])-set(read)),findings=['Chinese README repeats size/hash/byte-confirmation algorithm and HOME-only configuration claim; earlier complete settings source review found outside-HOME configured paths accepted. Documentation claim is not a validated boundary.','README lists destructive CLI/package operations; none executed for this review.','index.html contains startup shell/CSS and module entry only; SVG contains four drawing paths. No additional duplicate matcher or source/resource admission.','App.css contains theme/layout/markdown/transition styles and reduced-motion handling for world transitions. No duplicate identity, keeper selection or backend cancellation logic. No rendered UI/animation qualification inferred.'],new_runtime_tests_run=False,implementation_adopted=False,pins={str(f):sha(f) for f in [parent,module,Path(__file__)]+[S/n for n in read]},scope='Four complete text reads, no truncated outputs counted. Cumulative101/129; structured locks/translations, binary assets and remaining source require separate review. Not full repository/all500, app runtime, platform or new correctness qualification.'),indent=2)+'\n')
print(len(read),'fully read;',129-len(read),'remaining')
