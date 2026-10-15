#!/usr/bin/env python3
from pathlib import Path
import json,hashlib,shutil
R=Path(__file__).resolve().parents[1];D=R/'docs/research';sha=lambda p:hashlib.sha256(Path(p).read_bytes()).hexdigest()
M=D/'dedup-committed-build-source-415443172-v2-20261015.json';m=json.loads(M.read_text());B=Path(m['source']);F=Path('/private/tmp/rrrah-selective-collection-full-v2-20261015');assert not F.exists()
for row in m['source_files']:assert sha(B/row['path'])==row['sha256']
shutil.copytree(B,F)
archive=D/'dedup-collection-fused-preverify-v4-source-20261015.rs';s=archive.read_text();base=(B/'crates/rrrah-dedup/src/exact.rs').read_text();assert s.startswith(base+'\n');extra=s[len(base):].split('#[cfg(test)]\nmod fused_preverify_benchmark')[0]
for name in ['compare_verified_observed_candidate','original_verified_pair','selective_verified_pair']:
 extra=extra.replace('#[cfg(test)]\nfn '+name,'#[cfg(any(feature = "decode", test))]\n'+('pub(crate) 'if name=='selective_verified_pair'else'')+'fn '+name)
(F/'crates/rrrah-dedup/src/exact.rs').write_text(base+extra)
p=F/'crates/rrrah-dedup/src/collection.rs';s=p.read_text();needle='    fn activate(\n';pos=s.index(needle)
s=s[:pos]+'''    fn verified_identical(&self, other: &Self, cancel: &dyn Fn() -> bool) -> Result<bool, (bool, SnapshotError)> {
        self.verify(cancel).map_err(|e| (true, e))?;
        other.verify(cancel).map_err(|e| (false, e))?;
        self.identical(other, cancel)
    }
'''+s[pos:]
start=s.index('impl Observation for ExactObservation');pos=s.index(needle,start)
s=s[:pos]+'''    fn verified_identical(&self, other: &Self, cancel: &dyn Fn() -> bool) -> Result<bool, (bool, SnapshotError)> {
        // Preserve dependency verification order once visual decoding has activated.
        if self.visual.is_none() && other.visual.is_none() {
            crate::exact::selective_verified_pair(&self.original, &other.original, cancel)
        } else {
            self.verify(cancel).map_err(|e| (true, e))?;
            other.verify(cancel).map_err(|e| (false, e))?;
            self.identical(other, cancel)
        }
    }
'''+s[pos:]
old='''            verify_file(&snapshots, &requests, i, &cancelled)?;
            verify_file(&snapshots, &requests, j, &cancelled)?;
            let (left, l)''';assert s.count(old)==1;s=s.replace(old,'            let (left, l)',1)
old='.identical(&snapshots.values[j], &cancelled)';assert s.count(old)==1;s=s.replace(old,'.verified_identical(&snapshots.values[j], &cancelled)',1);p.write_text(s)
changed=[]
for row in m['source_files']:
 h=sha(F/row['path'])
 if h!=row['sha256']:changed.append(dict(path=row['path'],baseline_sha256=row['sha256'],candidate_sha256=h))
assert {r['path']for r in changed}=={'crates/rrrah-dedup/src/exact.rs','crates/rrrah-dedup/src/collection.rs'}
O=D/'dedup-selective-collection-full-v2-prepared-20261015.json';assert not O.exists();O.write_text(json.dumps(dict(status='prepared_not_compiled_not_adopted',baseline_manifest=str(M.relative_to(R)),baseline_manifest_sha256=sha(M),candidate=str(F),candidate_archive_sha256=sha(archive),changed=changed,unchanged_files=len(m['source_files'])-len(changed),guard_invariants=['Default nonexact observation retains original leftverify/rightverify/identical order','Exact fusion only before both visual dependency snapshots activate','Activated visual dependency guards preserve prior order','All postpair/postloop/callbackfree publication guards unchanged','No pair schedule, evidence factory, model loading or budget changes'],scope='Immutable full workspace candidate for actual collection integration testing. Preparation and source accounting only; not a passing compile/runtime/performance or current mutable Root claim.'),indent=2)+'\n');print('Prepared candidate; exactly2files changed;3376baseline files unchanged')
