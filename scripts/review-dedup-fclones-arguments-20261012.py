#!/usr/bin/env python3
import json,hashlib,subprocess,shutil,re
from pathlib import Path
R=Path(__file__).resolve().parents[1];D=R/'docs/research/dedup-500';T=Path('/Users/themoretheless/.codex/tmp');S=T/'rrrah-fclones-full-source-20261012'
def sha(p):return hashlib.sha256(Path(p).read_bytes()).hexdigest()
def main():
 h=T/'rrrah-fclones-argument-controls-20261012';assert not h.exists();(h/'src').mkdir(parents=True);shutil.copy2(S/'fclones/src/arg.rs',h/'src/arg.rs')
 (h/'Cargo.toml').write_text('[package]\nname="rrrah-fclones-argument-controls"\nversion="0.0.0"\nedition="2021"\n[dependencies]\nitertools="0.11"\nserde={version="1.0",features=["derive"]}\nstfu8="0.2"\n')
 (h/'src/lib.rs').write_text('''mod arg;
#[cfg(test)] mod controls {
 use super::arg::{Arg,quote,split};
 use std::ffi::OsString;
 #[test] fn unicode_dollar_quoted_text_panics_at_byte_slice() {
  assert!(std::panic::catch_unwind(||split("$'é'")).is_err());
  assert_eq!(split("$'abc'").unwrap(),vec![Arg::from("abc")]);
 }
 #[test] fn empty_argument_is_lost_on_quote_split_roundtrip() {
  let quoted=quote(OsString::new());assert_eq!(quoted,"");
  assert_eq!(split(&quoted).unwrap(),vec![]);
  assert_eq!(split("''").unwrap(),vec![Arg::from("")]);
 }
 #[cfg(unix)] #[test] fn invalid_utf8_roundtrip_preserves_bytes() {
  use std::os::unix::ffi::OsStringExt;
  let value=OsString::from_vec(vec![b'x',0xff,b' ',b'\\'']);
  let quoted=quote(value.clone());assert_eq!(split(&quoted).unwrap(),vec![Arg::from(value)]);
 }
}
''')
 log=T/'rrrah-fclones-argument-controls-tests-20261012.log';cmd=['cargo','test','--offline','--manifest-path',str(h/'Cargo.toml'),'--target-dir',str(T/'rrrah-fclones-target-20261012'),'--','--test-threads=1'];p=subprocess.run(cmd,cwd='/tmp',capture_output=True,text=True);log.write_text(p.stdout+p.stderr);assert p.returncode==0,log
 names=re.findall(r'^test (\S+) \.\.\. ok$',p.stdout,re.M);assert len(names)==14 and '14 passed; 0 failed; 0 ignored' in p.stdout;assert (h/'src/arg.rs').read_bytes()==(S/'fclones/src/arg.rs').read_bytes()
 out=D/'fclones-arguments-review-20261012.json';assert not out.exists();out.write_text(json.dumps({'status':'complete_arg_source_review_two_counterexamples_and_one_lossless_control','commit':'a74f90d293e05856d19a4c0ac2b29b46ef16cf23','fully_read_additional_files':{'fclones/src/arg.rs':474},'original_tests_passed':11,'additional_controls_passed':3,'tests':names,'pins':{str(p):sha(p) for p in [Path(__file__),log,h/'Cargo.toml',h/'Cargo.lock',h/'src/lib.rs',h/'src/arg.rs',S/'fclones/src/arg.rs']},'observations':['The dollar-quote parser advances pos per character, then slices UTF8 bytes: split of Unicode dollar-quoted é panics; ASCII control works.','quote of empty OsString returns empty text; split then loses the argument. Explicit empty single quotes control retains it.','Unix invalid UTF8, whitespace and apostrophe roundtrip preserves exact bytes through STFU8.','SPECIAL_CHARS contains Unicode small tilde but not ASCII tilde; quote of ASCII tilde can allow shell expansion. Source observation only, no shell execution.'],'scope':'Entire474-line arg.rs read, byte-identical isolated module with compatible offline-resolved dependencies. Eleven original tests and three controls passed; panic deliberately caught. No CLI/script/transform execution, Windows behavior, implementation adoption, whole42tree/full500 completion or general shell correctness claim.'},indent=2)+'\n');print('11 original tests and3 argument controls passed')
if __name__=='__main__':main()
