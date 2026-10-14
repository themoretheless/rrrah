#!/usr/bin/env python3
from pathlib import Path
import hashlib,json,re,shutil,difflib
R=Path(__file__).resolve().parents[1];D=R/'docs/research';P=D/'dedup-gradient-masked-lanes-managed-20261014.json';d=json.loads(P.read_text());sha=lambda p:hashlib.sha256(Path(p).read_bytes()).hexdigest()
assert d['status']=='passed_pending_audit' and d['build_returncode']==d['test_returncode']==0
assert (d['passed'],d['failed'],d['ignored'])==(59,0,3)
assert all(sha(p)==h for p,h in d['pins'].items()) and all(sha(p)==h for p,h in d['dependency_pins'].items())
source=Path(d['source']);assert all(sha(source/p)==h for p,h in d['source_sha256'].items())
assert sha(d['original_manifest'])==d['original_manifest_sha256'];m=json.loads(Path(d['original_manifest']).read_text());baseline=Path(m['source'])
assert all(sha(baseline/p)==h for p,h in m['source_sha256'].items())
changed={p for p,h in d['source_sha256'].items()if sha(baseline/p)!=h};assert changed=={'crates/rrrah-dedup/src/gradient.rs'}
F=Path('/private/tmp/rrrah-gradient-masked-lanes-managed-20261014')
listed=re.findall(r'^(.+): test$',(F/'inventory.log').read_text(),re.M)
executed=re.findall(r'^test (.+) \.\.\. (ok|ignored(?:,.*)?)$',(F/'tests.log').read_text(),re.M)
assert len(listed)==62 and len(set(listed))==62 and len(executed)==62 and sorted(listed)==sorted(n for n,_ in executed)
assert sum(status=='ok'for _,status in executed)==59 and sum(status.startswith('ignored')for _,status in executed)==3
retained=D/'dedup-gradient-masked-lanes-managed-20261014';retained.mkdir(exist_ok=False);pins={}
for p in [F/'build.log',F/'inventory.log',F/'tests.log',source/'crates/rrrah-dedup/src/gradient.rs']:
 target=retained/p.name;shutil.copyfile(p,target);assert sha(p)==sha(target);pins[str(target.relative_to(R))]=sha(target)
old=(baseline/'crates/rrrah-dedup/src/gradient.rs').read_text();new=(source/'crates/rrrah-dedup/src/gradient.rs').read_text()
patch=retained/'gradient.patch';patch.write_text(''.join(difflib.unified_diff(old.splitlines(keepends=True),new.splitlines(keepends=True),fromfile='V7/gradient.rs',tofile='isolated-masked/gradient.rs')));pins[str(patch.relative_to(R))]=sha(patch)
O=D/'dedup-gradient-masked-lanes-managed-audit-20261014.json';assert not O.exists();O.write_text(json.dumps(dict(status='passed_bounded_raster_library_gate',report=str(P.relative_to(R)),report_sha256=sha(P),auditor_sha256=sha(__file__),tests_passed=59,tests_ignored=3,inventory_exact=True,changed_source_files=sorted(changed),retained_artifacts=pins,scope=d['scope'],pending='Actual public managed baseline-only/cache/linear headroom benchmark; native complete-operation/corpus/platform/default qualification remains separate. Production checkout gradient.rs unchanged.'),indent=2)+'\n');print(O)
