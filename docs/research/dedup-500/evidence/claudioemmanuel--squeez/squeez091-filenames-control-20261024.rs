use squeez::{commands::{fs::FsHandler,text_proc::TextProcHandler,Handler},config::Config,json_util};
fn main(){
 let cfg=Config::default();let names=vec!["/owned/a:0.rs".into(),"/owned/report0%".into(),"/owned/space name.rs".into()];let files=FsHandler.compress("ls -1 /owned",names.clone(),&cfg);assert!(!files.contains(&names[1]));assert!(files.contains(&names[0])&&files.contains(&names[2]));
 let matches=(0..5).map(|i|format!("/owned/a:{i}.rs:1:unique match {i}")).collect::<Vec<_>>();let grouped=TextProcHandler.compress("rg -n unique /owned",matches,&cfg);assert_eq!(grouped,vec!["/owned/a: 5 matches [squeez: collapsed]"]);
 let ordinary=(0..5).map(|i|format!("/owned/file{i}.rs:1:unique match {i}")).collect::<Vec<_>>();assert_eq!(TextProcHandler.compress("rg -n unique /owned",ordinary.clone(),&cfg),ordinary);
 println!("{{\"scope\":\"Unmodified public handler APIs, synthetic listing and grep-result strings; filenames valid on POSIX, no real rg/ls invocation\",\"listing_input\":{},\"listing_output\":{},\"five_distinct_colon_paths_grouped_as\":{},\"ordinary_distinct_path_control_passed\":true}}",json_util::str_array(&names),json_util::str_array(&files),json_util::str_array(&grouped));
}
