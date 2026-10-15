#!/usr/bin/env python3
from pathlib import Path
import json,hashlib,shutil
R=Path(__file__).resolve().parents[1];D=R/'docs/research';sha=lambda p:hashlib.sha256(Path(p).read_bytes()).hexdigest();M=D/'dedup-committed-build-source-415443172-v2-20261015.json';m=json.loads(M.read_text());B=Path(m['source']);F=Path('/private/tmp/rrrah-ownerless-collection-full-v1-20261015');assert not F.exists()
for row in m['source_files']:assert sha(B/row['path'])==row['sha256']
shutil.copytree(B,F);p=F/'crates/rrrah-dedup/src/collection.rs';s=p.read_text();controls=D/'dedup-ownerless-guard-controls-v2-20261015.rs';c=controls.read_text();helper=c[c.index('fn owner_requests('):c.index('#[test]')];tests=c[c.index('#[test]'):c.index('#[test]\n#[ignore=')];needle='trait Observation: Sized {';assert s.count(needle)==1;s=s.replace(needle,helper+'\n'+needle,1)
needle='    let cancelled = || stopped(&requests.values, cancel, &latch);';assert s.count(needle)==1;s=s.replace(needle,'''    #[cfg(test)]
    let use_ownerless = BENCH_OWNERLESS.with(Cell::get);
    #[cfg(not(test))]
    let use_ownerless = true;
    let owners = if use_ownerless { owner_requests(&requests.values) } else { &requests.values };
    let cancelled = || stopped(owners, cancel, &latch);''',1)
needle='    let owner_cancelled = || owners_stopped(&requests.values, &latch);';assert s.count(needle)==1;s=s.replace(needle,'    let owner_cancelled = || owners_stopped(owners, &latch);',1)
s+='\n#[cfg(test)]\nmod ownerless_controls {\n use super::*;\n use std::sync::{Arc,atomic::{AtomicU64,Ordering}};\n'+tests+'\n}\n#[cfg(test)]\nstd::thread_local! { static BENCH_OWNERLESS: Cell<bool> = const { Cell::new(true) }; }\n'
H=D/'dedup-actual-collection-benchmark-v1-20261015.rs';bench=H.read_text();needle='        let n: usize';assert bench.count(needle)==1;bench=bench.replace(needle,'''        let variant=std::env::var("DEDUP_BENCH_VARIANT").unwrap();
        assert!(["baseline","candidate"].contains(&variant.as_str()));
        BENCH_OWNERLESS.with(|flag|flag.set(variant=="candidate"));
        let owner_kind=std::env::var("DEDUP_BENCH_OWNERS").unwrap_or_else(|_|"none".into());
        assert!(["none","all","mixed"].contains(&owner_kind.as_str()));
        let generation=std::sync::Arc::new(std::sync::atomic::AtomicU64::new(1));
'''+needle,1)
needle='            (i as u64,DecodeRequest::new(p))';assert bench.count(needle)==1;bench=bench.replace(needle,'''            let mut request=DecodeRequest::new(p);
            if owner_kind=="all"||(owner_kind=="mixed"&&i%2==0){request.cancellation=Some(rrrah_decode::GenerationToken::new(generation.clone(),1));}
            (i as u64,request)''',1);s+=bench;p.write_text(s)
changed=[]
for row in m['source_files']:
 h=sha(F/row['path'])
 if h!=row['sha256']:changed.append(dict(path=row['path'],baseline_sha256=row['sha256'],candidate_sha256=h))
assert len(changed)==1 and changed[0]['path']=='crates/rrrah-dedup/src/collection.rs';O=D/'dedup-ownerless-collection-full-v1-prepared-20261015.json';assert not O.exists();O.write_text(json.dumps(dict(status='prepared_not_compiled_not_adopted',producer_sha256=sha(__file__),source=str(F),baseline_manifest=str(M),baseline_manifest_sha256=sha(M),guard_controls_sha256=sha(controls),benchmark_harness_sha256=sha(H),changed=changed,unchanged_files=len(m['source_files'])-1,guard_invariants=['Presence checked once after requests sort; requests thereafter immutably borrowed','None ownerfields yield empty slice, Some fields preserve complete request slice','Original stopped and owners_stopped functions unchanged','Usercallback, sticky cancellation, final callbackfree publication checks retained','No sourceverification/compare/schedule/model/credit/admission changes','Test-only benchmark branch selected once, excluded from production; defaulttest selects candidate'],scope='Ownerpresence-only candidate on immutable415baseline, separate from fusedpreverification. All sourcefiles retained, fullcombined decoder/dedup lib/allintegration gate required. Samebinary actualengine benchmark prepared for all9datafamilies x3ownerkinds; no runtime/performance/Root adoption claim.'),indent=2)+'\n');print('Owner-only full candidate:1changed3377unchangedfiles')
