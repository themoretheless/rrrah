use squeez::{commands::compress_output::compute_rewrite,config::Config,context::cache::SessionContext};
use std::path::Path;
fn payload(path:&str)->String {
    let body=(0..12).map(|i|format!("line {i} of {path}")).collect::<Vec<_>>().join("\\n");
    format!(r#"{{"tool_name":"Read","file_path":"{path}","tool_result":{{"content":"{body}"}}}}"#)
}
fn check(dir:&Path, raise:bool)->bool {
    let cfg=Config::default();let text=payload("/owned/target.rs");
    assert!(compute_rewrite(&text,"Read",dir,&cfg).is_none());
    if raise {
        let mut c=SessionContext::load(dir);c.dedup_floor_call=c.call_counter;c.save(dir);
        assert!(compute_rewrite(&text,"Read",dir,&cfg).is_none());
    }
    for i in 0..25 {let _=compute_rewrite(&payload(&format!("/owned/noise{i}.rs")),"Read",dir,&cfg);}
    compute_rewrite(&text,"Read",dir,&cfg).is_some()
}
fn main(){
    let base=std::path::PathBuf::from(std::env::args_os().nth(1).unwrap());
    let baseline=check(&base.join("baseline"),false);let renewed=check(&base.join("renewed"),true);
    assert!(baseline);assert!(!renewed);
    println!(r#"{{"scope":"Unchanged compute_rewrite pipeline in owned directories, synthetic Read payloads, manual floor transition matching cache field change; no real host hook","noise_calls":25,"baseline_session_long_reread_collapsed":true,"post_floor_reissued_reread_collapsed":false}}"#);
}
