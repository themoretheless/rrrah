use squeez::economy::calibrate::{select_profile,profile_to_config,write_config,BenchmarkAnalysis};
fn main(){
 let empty=select_profile(&BenchmarkAnalysis{avg_reduction_pct:75.0,quality_pass_count:0,total_scenarios:0}); assert_eq!(empty.max_lines,120);
 let a=select_profile(&BenchmarkAnalysis{avg_reduction_pct:75.0,quality_pass_count:10,total_scenarios:10});
 let b=select_profile(&BenchmarkAnalysis{avg_reduction_pct:60.0,quality_pass_count:0,total_scenarios:10});assert_eq!(b.max_lines,200);
 let base=std::path::PathBuf::from(std::env::var_os("SQUEEZ_DIR").unwrap());std::fs::write(base.join("config.ini"),"persona = off\n").unwrap();
 write_config(&a).unwrap();let old=std::fs::read(base.join("config.ini")).unwrap();write_config(&b).unwrap();
 let merged=std::fs::read_to_string(base.join("config.ini")).unwrap(); assert!(merged.contains("read_max_lines = 300"));assert!(merged.contains("grep_max_results = 100"));assert!(merged.contains("persona = off"));assert_eq!(std::fs::read(base.join("config.ini.bak")).unwrap(),old);
 let fresh=profile_to_config(&b);assert!(!fresh.contains("read_max_lines"));
 println!("{{\"zero_scenario_high_average_selects_aggressive\":true,\"zero_quality_passes_selects_balanced\":true,\"balanced_after_aggressive_keeps_read_limit\":300,\"balanced_after_aggressive_keeps_grep_limit\":100,\"fresh_balanced_omits_limits\":true,\"user_key_and_backup_preserved\":true}}");
}
