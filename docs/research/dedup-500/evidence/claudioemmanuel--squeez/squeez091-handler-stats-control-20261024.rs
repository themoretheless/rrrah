use squeez::economy::{handler_stats::HandlerStats,efficiency};
fn main(){
 let dir=std::path::PathBuf::from(std::env::args_os().nth(1).unwrap());
 let mut initial=HandlerStats::default(); initial.record("git",100,10);initial.save(&dir);
 let mut a=HandlerStats::load(&dir);let mut b=HandlerStats::load(&dir);
 a.record("git",100,10);a.save(&dir);b.record("cargo",200,20);b.save(&dir);
 let final_stats=HandlerStats::load(&dir);let rows=final_stats.rows();
 assert_eq!(rows.iter().find(|r|r.name=="git").unwrap().calls,1);
 assert_eq!(rows.iter().find(|r|r.name=="cargo").unwrap().calls,1);
 let mut extreme=HandlerStats::default(); extreme.record("large",u64::MAX,0);let bp=extreme.rows()[0].savings_bp;assert_eq!(bp,1);
 let mut ordinary=HandlerStats::default();ordinary.record("small",1000,0);assert_eq!(ordinary.rows()[0].savings_bp,10000);
 let panic=std::panic::catch_unwind(||efficiency::compute(u64::MAX,0,0,0,0,0,200000)).is_err();assert!(panic);
 println!("{{\"stale_snapshot_loses_git_increment\":true,\"both_saves_sequential\":true,\"full_savings_extreme_basis_points\":{},\"ordinary_full_savings_basis_points\":10000,\"efficiency_extreme_panics_debug\":{}}}",bp,panic);
}
