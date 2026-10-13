#!/usr/bin/env python3
import hashlib,json,subprocess,re
from pathlib import Path
R=Path(__file__).resolve().parents[1];D=R/'docs/research/dedup-500';T=Path('/Users/themoretheless/.codex/tmp');S=T/'rrrah-fclones-full-source-20261012'
def sha(p):return hashlib.sha256(Path(p).read_bytes()).hexdigest()
def main():
 h=T/'rrrah-fclones-dedupe-plan-controls-20261012';assert not h.exists();(h/'src').mkdir(parents=True)
 (h/'Cargo.toml').write_text('[package]\nname="rrrah-fclones-dedupe-plan-controls"\nversion="0.0.0"\nedition="2021"\n[dependencies]\nfclones={path="'+str(S/'fclones')+'"}\ntempfile="3"\n')
 (h/'src/lib.rs').write_text('''#[cfg(test)] mod controls {
use fclones::{dedupe,log_script,DedupeOp,DedupeConfig,FileGroup,FileHash,FileLen,Path};
use fclones::log::StdLog;
#[test] fn generated_removal_plan_trusts_report_hash_without_content_recheck() {
 let root=tempfile::tempdir().unwrap();let a=root.path().join("a");let b=root.path().join("b");
 std::fs::write(&a,b"aaaa").unwrap();std::fs::write(&b,b"bbbb").unwrap();
 let group=FileGroup {file_len:FileLen(4),file_hash:FileHash::from(0_u128),files:vec![Path::from(&a),Path::from(&b)]};
 let config=DedupeConfig::default();let mut logger=StdLog::new();logger.no_progress=true;
 let mut output=Vec::new();let plan=dedupe(vec![group],DedupeOp::Remove,&config,&logger);
 let result=log_script(plan,&mut output).unwrap();
 assert_eq!(result.processed_count,1);assert_eq!(result.reclaimed_space,FileLen(4));
 assert!(String::from_utf8(output).unwrap().starts_with("rm "));
 assert_eq!(std::fs::read(a).unwrap(),b"aaaa");assert_eq!(std::fs::read(b).unwrap(),b"bbbb");
 // Deliberately do not call run_script or execute generated commands.
}
}
''')
 log=T/'rrrah-fclones-dedupe-plan-controls-tests-20261012.log';cmd=['cargo','test','--offline','--manifest-path',str(h/'Cargo.toml'),'--target-dir',str(T/'rrrah-fclones-target-20261012'),'--','--test-threads=1'];p=subprocess.run(cmd,cwd='/tmp',capture_output=True,text=True);log.write_text(p.stdout+p.stderr);assert p.returncode==0,log
 assert '1 passed; 0 failed; 0 ignored' in p.stdout;original=T/'rrrah-fclones-dedupe-tests-20261012.log';old=original.read_text();assert '20 passed; 0 failed; 0 ignored' in old
 out=D/'fclones-dedupe-review-20261012.json';assert not out.exists();out.write_text(json.dumps({'status':'complete1633line_dedupe_source_twenty_original_tests_and_one_public_plan_control','commit':'a74f90d293e05856d19a4c0ac2b29b46ef16cf23','fully_read_additional_files':{'fclones/src/dedupe.rs':1633},'original_tests_passed':20,'public_plan_controls_passed':1,'tests':re.findall(r'^test (\S+) \.\.\. ok$',old+p.stdout,re.M),'pins':{str(p):sha(p) for p in [Path(__file__),log,original,h/'Cargo.toml',h/'Cargo.lock',h/'src/lib.rs',S/'fclones/src/dedupe.rs',S/'fclones/src/lock.rs',T/'rrrah-fclones-upstream-executables-20261012/lib']},'observations':['Public dedupe+log_script plans removal of one unequal same-size file when supplied a synthetic equal report hash and default API config. Both file contents verified unchanged; no generated commands executed. CLI normally supplies report timestamp, so this control is scoped to public API default trust boundary.','Partition checks regular-file status, size and optional modification cutoff, but does not rehash or byte-compare before producing commands.','FsCommand execute assigns maybe_lock result to underscore; returned guard is not retained through the following operation. Move additionally discards lock acquisition error. Source inference, no concurrent lock counterexample.','Move checks target existence before an overwriting rename/copy; this is not atomic no-clobber. Source only.','safe_remove uses a random path without exclusive creation; rollback/cleanup failures log warnings. Source only.','run_script logs individual failures and continues; output contains successes rather than an all-or-nothing transaction.','log_script uses unbounded channel and reorder queue; no hard memory bound established.'],'scope':'Full1633-line source read. Twenty original upstream tests executed only in new isolated temporary fixture working directory. One public dry-plan counterexample, no generated operation execution. No user source files modified, implementation adopted, physical concurrency/Windows/full42tree/full500/resource-cap qualification.'},indent=2)+'\n');print('20 original dedupe tests and1 public dry-plan control passed')
if __name__=='__main__':main()
