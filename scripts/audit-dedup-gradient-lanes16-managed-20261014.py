#!/usr/bin/env python3
from pathlib import Path
import hashlib,json,re,shutil,difflib
R=Path(__file__).resolve().parents[1];D=R/'docs/research';P=D/'dedup-gradient-lanes16-managed-20261014.json';d=json.loads(P.read_text());sha=lambda p:hashlib.sha256(Path(p).read_bytes()).hexdigest()
assert d['status']=='passed_pending_audit'and d['build_returncode']==d['test_returncode']==0 and (d['passed'],d['failed'],d['ignored'])==(62,0,3)
assert all(sha(p)==h for p,h in d['pins'].items())and all(sha(p)==h for p,h in d['dependency_pins'].items())
source=Path(d['source']);baseline=Path(d['baseline_source']);assert all(sha(source/p)==h for p,h in d['source_sha256'].items())and all(sha(baseline/p)==h for p,h in d['baseline_source_sha256'].items())
changed={p for p,h in d['source_sha256'].items()if d['baseline_source_sha256'].get(p)!=h};assert changed=={'crates/rrrah-dedup/src/gradient.rs','crates/rrrah-dedup/src/gradient_matching_tests.rs'}
old=(baseline/'crates/rrrah-dedup/src/gradient.rs').read_text();new=(source/'crates/rrrah-dedup/src/gradient.rs').read_text()
def second(text):
    a=text.index('fn match_gradients_selected_with_scratch<');b=text.index('\n/// Describe a rotated',a);text=text[a:b];return text[text.index('    if let Some(radius) = distinct_radius_squared {'):]
assert ''.join(second(old).split())==''.join(second(new).split())and 'right.chunks(4)'in second(new)
a=new.index('fn match_gradients_selected_with_scratch<');b=new.index('    if let Some(radius) = distinct_radius_squared {',a);assert new[a:b].count('right.chunks(16)')==1 and new[a:b].count('distances_sixteen(&a.descriptor, chunk)')==1
F=Path('/private/tmp/rrrah-gradient-lanes16-managed-20261014');listed=re.findall(r'^(.+): test$',(F/'inventory.log').read_text(),re.M);executed=re.findall(r'^test (.+) \.\.\. (ok|ignored(?:,.*)?)$',(F/'tests.log').read_text(),re.M)
assert len(listed)==len(set(listed))==len(executed)==65 and sorted(listed)==sorted(n for n,_ in executed)
assert sum(s=='ok'for _,s in executed)==62 and sum(s.startswith('ignored')for _,s in executed)==3
assert ('gradient::matching_tests::sixteen_descriptor_lanes_preserve_scalar_bits_and_every_tail','ok')in executed
retained=D/'dedup-gradient-lanes16-managed-20261014';retained.mkdir(exist_ok=False);pins={}
for p in [F/'build.log',F/'inventory.log',F/'tests.log',source/'crates/rrrah-dedup/src/gradient.rs',source/'crates/rrrah-dedup/src/gradient_matching_tests.rs']:
    copy=retained/p.name;shutil.copyfile(p,copy);assert sha(p)==sha(copy);pins[str(copy.relative_to(R))]=sha(copy)
patch=retained/'gradient.patch';patch.write_text(''.join(difflib.unified_diff(old.splitlines(keepends=True),new.splitlines(keepends=True),fromfile='root-baseline/gradient.rs',tofile='isolated-firstpass16/gradient.rs')));pins[str(patch.relative_to(R))]=sha(patch)
O=D/'dedup-gradient-lanes16-managed-audit-20261014.json';assert not O.exists();O.write_text(json.dumps(dict(status='verified_isolated_actual_layout_firstpass16_raster_gate',report=str(P.relative_to(R)),report_sha256=sha(P),auditor_sha256=sha(__file__),passed=62,failed=0,manual_ignored=3,inventory_exact=True,first_pass_lanes=16,second_pass_lanes=4,second_pass_implementation_unchanged=True,changed_source_files=sorted(changed),retained_artifacts=pins,scope=d['scope'],remaining='No-optional-feature candidate unit gate, nearest-table/cache/cancellation parity and actual public API CPU comparisons on large natural/small/cache/fallback/runner paths. Primitive descriptor-only layout gains are not adoption evidence for actual1040byte features.'),indent=2)+'\n');print(O)
