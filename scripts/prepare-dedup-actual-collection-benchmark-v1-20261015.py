#!/usr/bin/env python3
from pathlib import Path
import json,hashlib,shutil
R=Path(__file__).resolve().parents[1];D=R/'docs/research';sha=lambda p:hashlib.sha256(Path(p).read_bytes()).hexdigest()
P=D/'dedup-selective-collection-full-v2-prepared-20261015.json';d=json.loads(P.read_text());B=Path(d['candidate']);F=Path('/private/tmp/rrrah-actual-collection-benchmark-v1-20261015');assert not F.exists()
M=R/d['baseline_manifest'];m=json.loads(M.read_text());expected={r['path']:r['sha256']for r in m['source_files']};expected.update({r['path']:r['candidate_sha256']for r in d['changed']})
for name,h in expected.items():assert sha(B/name)==h
shutil.copytree(B,F);p=F/'crates/rrrah-dedup/src/collection.rs';s=p.read_text();needle='        // Preserve dependency verification order once visual decoding has activated.';assert s.count(needle)==1
s=s.replace(needle,'''        #[cfg(test)]
        if !BENCH_SELECTIVE_PREVERIFY.with(Cell::get) {
            self.verify(cancel).map_err(|e| (true, e))?;
            other.verify(cancel).map_err(|e| (false, e))?;
            return self.identical(other, cancel);
        }
'''+needle,1)
bench=D/'dedup-actual-collection-benchmark-v1-20261015.rs';b=bench.read_text();needle='        let n: usize';assert b.count(needle)==1;b=b.replace(needle,'''        let variant=std::env::var("DEDUP_BENCH_VARIANT").unwrap();
        assert!(["baseline","candidate"].contains(&variant.as_str()));
        BENCH_SELECTIVE_PREVERIFY.with(|flag|flag.set(variant=="candidate"));
'''+needle,1)
s+='\n#[cfg(test)]\nstd::thread_local! { static BENCH_SELECTIVE_PREVERIFY: Cell<bool> = const { Cell::new(true) }; }\n'+b;p.write_text(s)
changed=[]
for row in m['source_files']:
 h=sha(F/row['path'])
 if h!=row['sha256']:changed.append(dict(path=row['path'],baseline_sha256=row['sha256'],candidate_sha256=h))
assert {r['path']for r in changed}=={'crates/rrrah-dedup/src/exact.rs','crates/rrrah-dedup/src/collection.rs'}
O=D/'dedup-actual-collection-benchmark-v1-prepared-20261015.json';assert not O.exists();O.write_text(json.dumps(dict(status='prepared_not_compiled_not_executed',source=str(F),baseline_manifest=str(M),baseline_manifest_sha256=sha(M),integration_candidate_manifest_sha256=sha(P),benchmark_source_sha256=sha(bench),changed=changed,unchanged_files=len(m['source_files'])-2,protocol=['Single binary two variants selected by threadlocal flag set once before timed loop','Both variants incur same test-only flag lookup; production build excludes flag completely','Baseline executes original leftverify/rightverify/identical order','Candidate fuses only before visual dependency activation','Actual scan_guarded_exact_collection captures all files, owns complete report and invokes original postpair/postloop/publication guards','Provider is synthetic original-byte oracle; no visual algorithm performance or quality claim','All originalnine families retained; negative modifications at payload offset54 keep dependency header valid'],required_before_dispatch='Finish and audit full baseline; run complete candidate library/integration gates; opt3 benchmark only after other owned heavy builds stop. Check completepair outputs, provider counts and memory credit release.'),indent=2)+'\n');print('Same-binary actualengine benchmark prepared;2changed3376unchangedfiles')
