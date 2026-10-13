#!/usr/bin/env python3
"""Apply only the independently qualified native method and regression test."""
import hashlib,json
from pathlib import Path
R=Path(__file__).resolve().parents[1];D=R/'docs/research';native=R/'vendor/loftr/src'
def sha(p):return hashlib.sha256(Path(p).read_bytes()).hexdigest()
p=D/'dedup-loftr-sequential-current-units-20261012.json';r=json.loads(p.read_text());assert r['status']=='current_native_units_passed_not_adopted' and r['passed']==56 and r['returncode']==0
assert all(sha(p)==h for p,h in r['pins'].items());before=r['root_native_before'];assert all(sha(native/n)==h for n,h in before.items())
audit=D/'dedup-loftr-sequential-backbone-complete-audit-20261012.json';a=json.loads(audit.read_text());assert a['entire_evidence_identical'] and a['rss_delta_vs_original_bytes']<0
out=D/'dedup-loftr-sequential-backbone-adoption-20261012.json';assert not out.exists()
source=Path(r['source'])/'src';changed=['loftr_model.rs','loftr_model/tests.rs'];assert sorted(n for n,h in before.items() if sha(source/n)!=h)==sorted(changed)
for n in changed:(native/n).write_bytes((source/n).read_bytes())
after={n:sha(native/n) for n in before};assert all(after[n]==sha(source/n) for n in before)
out.write_text(json.dumps(dict(status='adopted_exact_56_unit_qualified_native_source',production_delta=['loftr_model.rs'],test_delta=['loftr_model/tests.rs'],before=before,after=after,native_units_passed=56,whole_case_evidence_identical=True,whole_case_rss_bytes=a['rss_bytes'],whole_case_baseline_rss_bytes=a['original_uninstrumented_rss_bytes'],pins={str(p):sha(p) for p in [p,audit,Path(__file__)]},scope='Root native source bytes exactly equal the complete current-native candidate that passed56 units. One full real-weight worst-negative caller case used older frozen remaining native/caller/decoder composition and exact whole JSON parity, with RSS reduction615858176 bytes. Does not establish current whole-stack corpus outcomes, universal memory bounds, gradients, other devices/platforms or full458/full500 completion.'),indent=2)+'\n')
print('Adopted tested native source; 56 units; fixed-case RSS saving',-a['rss_delta_vs_original_bytes'])
