use squeez::{config::Config,context::cache::{SessionContext,BurnEntry},economy::burn_rate::calls_remaining};
fn main(){
 let c=Config::default(); let mut s=SessionContext::default();
 for i in 0..3 {s.burn_window.push(BurnEntry{call_n:i,tokens:1000,ts:0});}
 s.tokens_bash=1_000_000; s.real_ctx_tokens=40_000;
 let normal=calls_remaining(&s,&c); assert_eq!(normal,Some(160));
 s.tokens_bash=u64::MAX; s.tokens_read=1;
 let overflow=std::panic::catch_unwind(||calls_remaining(&s,&c)).is_err(); assert!(overflow);
 println!("{{\"measured_context_control_remaining\":160,\"unused_counter_sum_panics_in_debug\":{}}}",overflow);
}
