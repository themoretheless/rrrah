use squeez::session::{self,CurrentSession};
fn main(){
 let mode=std::env::args().nth(1).unwrap();let dir=std::path::PathBuf::from(std::env::args_os().nth(2).unwrap());
 if mode=="keys" {
  assert!(session::claim_nudge(&dir,"event:a"));assert!(!session::claim_nudge(&dir,"event/a"));assert!(!session::claim_nudge(&dir,"event:a"));
  session::reset_nudge_claims(&dir);assert!(session::claim_nudge(&dir,"event/a"));
  println!(r#"{{"scope":"Original public advisory API, owned directory and distinct synthetic keys","distinct_punctuation_keys_collide":true,"repeat_refused":true,"reset_releases_claim":true}}"#);return;
 }
 if let Some(id)=mode.strip_prefix("worker") {
  let id=id.parse::<u64>().unwrap();let c=CurrentSession{session_file:format!("writer{id}-{}","x".repeat(2048+id as usize*3000)),total_calls:id,..Default::default()};
  for _ in 0..100 {c.save(&dir);}return;
 }
 let c=CurrentSession{session_file:if mode=="initial"{"previous-valid-state".into()}else{"replacement".repeat(8192)},total_calls:if mode=="initial"{17}else{99},..Default::default()};c.save(&dir);
}
