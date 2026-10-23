use squeez::context::cache::SessionContext;
fn main() {
    let mode=std::env::args().nth(1).unwrap();
    let dir=std::path::PathBuf::from(std::env::args_os().nth(2).unwrap());
    let mut c=SessionContext::default();
    if mode=="initial" { c.session_file="previous-valid-state".into();c.call_counter=17; }
    else { c.session_file="replacement".repeat(8192);c.call_counter=99; }
    c.save(&dir);
}
