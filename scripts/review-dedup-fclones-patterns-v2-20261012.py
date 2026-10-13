#!/usr/bin/env python3
import hashlib,json,subprocess,shutil,re
from pathlib import Path
R=Path(__file__).resolve().parents[1];D=R/'docs/research/dedup-500';T=Path('/Users/themoretheless/.codex/tmp');S=T/'rrrah-fclones-full-source-20261012'
def sha(p):return hashlib.sha256(Path(p).read_bytes()).hexdigest()
def main():
 h=T/'rrrah-fclones-pattern-controls-v2-20261012';assert not h.exists();(h/'src').mkdir(parents=True)
 for f in ['regex.rs','pattern.rs']:shutil.copy2(S/'fclones/src'/f,h/'src'/f)
 (h/'Cargo.toml').write_text('[package]\nname="rrrah-fclones-pattern-controls"\nversion="0.0.0"\nedition="2021"\n[dependencies]\nnom="7"\nregex="1.5"\n')
 (h/'src/lib.rs').write_text('''mod path { pub const PATH_ESCAPE_CHAR: &str = "\\\\"; }
mod regex;
mod pattern;
#[cfg(test)] mod controls {
 use super::pattern::Pattern;
 #[test] fn raw_alternation_is_not_fully_anchored() {
  let p=Pattern::regex("foo|bar").unwrap();
  assert!(p.matches("foo-tail")); assert!(p.matches("head-bar"));
  let grouped=Pattern::regex("(?:foo|bar)").unwrap();
  assert!(!grouped.matches("foo-tail")); assert!(!grouped.matches("head-bar"));
  assert!(grouped.matches("foo")); assert!(grouped.matches("bar"));
 }
 #[test] fn escaped_optional_prefix_rejects_valid_ancestor() {
  let p=Pattern::regex(r"a\\.*/child").unwrap();
  assert!(p.matches("a./child"));
  assert!(!p.matches_partially("a./"));
 }
 #[test] fn unicode_optional_prefix_rejects_valid_ancestor() {
  let p=Pattern::regex("ąę?/child").unwrap();
  assert!(p.matches("ą/child"));
  assert!(!p.matches_partially("ą/"));
 }
 #[test] fn advertised_negative_extglob_is_rejected() {
  assert!(Pattern::glob("!(foo|bar)").is_err());
 }
}
''')
 log=T/'rrrah-fclones-pattern-controls-tests-v2-20261012.log';cmd=['cargo','test','--offline','--manifest-path',str(h/'Cargo.toml'),'--target-dir',str(T/'rrrah-fclones-target-20261012'),'--','--test-threads=1'];p=subprocess.run(cmd,cwd='/tmp',capture_output=True,text=True);log.write_text(p.stdout+p.stderr);assert p.returncode==0,log
 names=re.findall(r'^test (\S+) \.\.\. ok$',p.stdout,re.M);assert len(names)==34 and '34 passed; 0 failed; 0 ignored' in p.stdout
 pins={str(p):sha(p) for p in [Path(__file__),log,h/'Cargo.toml',h/'Cargo.lock',h/'src/lib.rs',h/'src/regex.rs',h/'src/pattern.rs',S/'fclones/src/regex.rs',S/'fclones/src/pattern.rs',S/'fclones/src/lib.rs']}
 out=D/'fclones-pattern-regex-review-v2-20261012.json';assert not out.exists();out.write_text(json.dumps({'status':'two_complete_files_and_four_runtime_counterexamples','commit':'a74f90d293e05856d19a4c0ac2b29b46ef16cf23','fully_read_additional_files':{f:len((S/f).read_text().splitlines()) for f in ['fclones/src/regex.rs','fclones/src/pattern.rs','fclones/src/lib.rs']},'original_tests_passed':30,'counterexample_controls_passed':4,'tests':names,'pins':pins,'observations':['Raw ungrouped regex alternatives are not fully anchored: foo|bar matches foo-tail and head-bar. Grouped control rejects both.','Fixed-prefix extraction retains escape state after escaped punctuation, so a\\.*/child accepts a./child but rejects valid ancestor a./.','Byte count used for Unicode optional prefix truncation: ąę?/child accepts ą/child but rejects valid ancestor ą/.','Advertised negative extended glob !(foo|bar) translates to unsupported look-around and returns an error.'],'scope':'Original private regex and pattern source copied byte-for-byte into isolated macOS ARM64 harness with the exact Unix PATH_ESCAPE_CHAR constant. All30 original tests and4 counterexamples pass. Whole CLI traversal/filter consequences not executed; no rrrah implementation adopted, full42-tree/full500 not complete.'},indent=2)+'\n');print('30 original tests and4 counterexamples passed')
if __name__=='__main__':main()
