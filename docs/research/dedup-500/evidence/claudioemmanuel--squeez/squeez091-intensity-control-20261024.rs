use squeez::config::Config;
use squeez::context::intensity::{scale,derive_with,Intensity};
fn main(){
 let mut c=Config::default(); c.max_lines=1; c.git_diff_max_lines=1; c.dedup_min=0;
 let s=scale(&c,Intensity::Ultra); assert_eq!(s.max_lines,20); assert_eq!(s.git_diff_max_lines,20); assert_eq!(s.dedup_min,2);
 c.context_window_tokens=u64::MAX; c.ultra_trigger_pct=0.65;
 let level=derive_with(u64::MAX/2,&c,0); assert_eq!(level,Intensity::Ultra);
 println!("{{\"requested_max_lines\":1,\"scaled_max_lines\":{},\"scaled_dedup_min\":{},\"extreme_budget_half_usage\":\"{}\"}}",s.max_lines,s.dedup_min,level.as_str());
}
