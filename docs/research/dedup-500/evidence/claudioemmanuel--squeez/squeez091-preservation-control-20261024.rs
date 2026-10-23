use squeez::economy::preservation::{extract_anchors,info_preservation,is_compression_risk};
fn main(){
 let a="/owned/first report.rs:7"; let b="/other/second report.rs:7";
 let path_score=info_preservation(a,b); assert_eq!(path_score,1.0);
 let absent_score=info_preservation("/owned/README",""); assert_eq!(absent_score,1.0);
 let verdict="1 passed; 999 passed"; let v=extract_anchors(verdict); assert!(v.test_verdicts.contains("1 passed")); assert!(!v.test_verdicts.contains("999 passed"));
 let verdict_score=info_preservation(verdict,"1 passed"); assert_eq!(verdict_score,1.0);
 assert!(!is_compression_risk(99.0,path_score));
 assert_eq!(info_preservation("src/main.rs:7","src/other.rs:7"),0.0);
 println!("{{\"different_space_paths_score\":{},\"dropped_extensionless_path_score\":{},\"dropped_second_verdict_score\":{},\"ordinary_distinct_paths_score\":0}}",path_score,absent_score,verdict_score);
}
