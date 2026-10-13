#!/usr/bin/env python3
import hashlib,json,subprocess,re
from pathlib import Path
R=Path(__file__).resolve().parents[1];D=R/'docs/research/dedup-500';T=Path('/Users/themoretheless/.codex/tmp');S=T/'rrrah-fclones-full-source-20261012'
def sha(p):return hashlib.sha256(Path(p).read_bytes()).hexdigest()
def main():
 h=T/'rrrah-fclones-report-controls-20261012';assert not h.exists();(h/'src').mkdir(parents=True)
 (h/'Cargo.toml').write_text('[package]\nname="rrrah-fclones-report-controls"\nversion="0.0.0"\nedition="2021"\n[dependencies]\nfclones={path="'+str(S/'fclones')+'"}\nfallible-iterator="0.2"\n')
 (h/'src/lib.rs').write_text('''#[cfg(test)] mod controls {
use fclones::report::{TextReportReader,ReportReader};
use std::io::{BufReader,Cursor};
const HEADER: &str="# Report by fclones 0.35.0\\n# Timestamp: 2021-08-27 12:11:23.456 +0000\\n# Command: fclones group .\\n# Base dir: /tmp\\n# Total: 8 B (8 B) in 2 files in 1 groups\\n# Redundant: 4 B (4 B) in 1 files\\n# Missing: 0 B (0 B) in 0 files\\n";
fn groups(body: &str) -> Box<fclones::report::GroupIterator> {
 let mut reader=Box::new(TextReportReader::new(BufReader::new(Cursor::new(format!("{HEADER}{body}").into_bytes()))));
 reader.read_header().unwrap();reader.read_groups().unwrap()
}
#[test] fn oversized_length_panics_instead_of_invalid_data() {
 assert!(std::panic::catch_unwind(||groups("aa, 18446744073709551616 B (large) * 1:\\n    /a\\n").next()).is_err());
}
#[test] fn blank_line_silently_ends_group_stream() {
 let mut iterator=groups("aa, 4 B (4 B) * 1:\\n    /a\\n\\nbb, 4 B (4 B) * 1:\\n    /b\\n");
 assert_eq!(iterator.next().unwrap().unwrap().files.len(),1);
 assert!(iterator.next().unwrap().is_none());
 // A caller following FallibleIterator's terminal None does not see the second group.
}
#[test] fn leading_and_trailing_path_spaces_are_trimmed() {
 let mut iterator=groups("aa, 4 B (4 B) * 1:\\n     /a \\n");
 let group=iterator.next().unwrap().unwrap();
 assert_eq!(group.files[0].to_path_buf(),std::path::PathBuf::from("/a"));
 assert_ne!(group.files[0].to_path_buf(),std::path::PathBuf::from(" /a "));
}
#[test] fn malformed_header_returns_typed_error_then_stops() {
 let mut iterator=groups("malformed\\n");
 assert_eq!(iterator.next().unwrap_err().kind(),std::io::ErrorKind::InvalidData);
 assert!(iterator.next().unwrap().is_none());
}
}
''')
 log=T/'rrrah-fclones-report-controls-tests-20261012.log';cmd=['cargo','test','--offline','--manifest-path',str(h/'Cargo.toml'),'--target-dir',str(T/'rrrah-fclones-target-20261012'),'--','--test-threads=1'];p=subprocess.run(cmd,cwd='/tmp',capture_output=True,text=True);log.write_text(p.stdout+p.stderr);assert p.returncode==0,log
 names=re.findall(r'^test (\S+) \.\.\. ok$',p.stdout,re.M);assert len(names)==4
 original=T/'rrrah-fclones-report-tests-20261012.log';old=original.read_text();oldnames=re.findall(r'^test (\S+) \.\.\. ok$',old,re.M);assert len(oldnames)==11 and '11 passed; 0 failed; 0 ignored' in old
 out=D/'fclones-report-review-20261012.json';assert not out.exists();out.write_text(json.dumps({'status':'complete_report_source_review_three_public_counterexamples_and_typed_error_control','commit':'a74f90d293e05856d19a4c0ac2b29b46ef16cf23','fully_read_additional_files':{'fclones/src/report.rs':939},'original_tests_passed':11,'additional_controls_passed':4,'tests':oldnames+names,'pins':{str(p):sha(p) for p in [Path(__file__),log,original,h/'Cargo.toml',h/'Cargo.lock',h/'src/lib.rs',S/'fclones/src/report.rs',T/'rrrah-fclones-upstream-executables-20261012/lib']},'observations':['Oversized u64 file length in a syntactically matching group header panics instead of returning InvalidData; panic deliberately caught.','A blank line after first group returns terminal None before a later valid group.','Path strings with leading/trailing spaces are trimmed to a different path.','Malformed ordinary group header returns InvalidData and subsequent None.','JSON reader deserializes full report into Vec; text reader uses unbounded read_line. Source observations only, no peak memory claim.'],'scope':'Complete939-line report module;11 original upstream tests and4 actual public report API controls using pinned dependency source. No CLI/dedupe/removal/link/move execution, no adoption or full42tree/full500/platform/total-memory completion.'},indent=2)+'\n');print('11 original report tests and4 public controls passed')
if __name__=='__main__':main()
