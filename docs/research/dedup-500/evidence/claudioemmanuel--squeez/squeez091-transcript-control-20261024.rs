use squeez::context::transcript::{last_context_tokens_in,last_context_tokens,measure_agent_usage_in,detect_window};
fn main(){
 let root=std::path::PathBuf::from(std::env::args_os().nth(1).unwrap());
 let compact=r#"{"type":"assistant","message":{"usage":{"input_tokens":10}}}"#;
 let spaced=r#"{"type": "assistant", "message": {"usage": {"input_tokens":10}}}"#;
 assert_eq!(last_context_tokens_in(compact),Some(10));assert_eq!(last_context_tokens_in(spaced),None);
 let side=r#"{"type":"assistant","isSidechain": true,"message":{"usage":{"input_tokens":99}}}"#;
 assert_eq!(last_context_tokens_in(&format!("{compact}\n{side}")),Some(99));
 let user=r#"{"type":"user","usage":{"input_tokens":77}}"#;
 assert_eq!(measure_agent_usage_in(user).unwrap().input,77);
 let repeated=format!("{compact}\n{compact}");assert_eq!(measure_agent_usage_in(&repeated).unwrap().input,20);
 let huge=format!("{{\"type\":\"assistant\",\"message\":{{\"usage\":{{\"input_tokens\":10}},\"content\":\"{}\"}}}}", "a".repeat(600000));
 let p=root.join("large-record.jsonl");std::fs::write(&p,&huge).unwrap();assert_eq!(last_context_tokens_in(&huge),Some(10));assert_eq!(last_context_tokens(&p),None);
 let identity1=r#"{"type":"attachment","identity":{"modelId":"x[1m]"}}"#;
 let identity2=r#"{"type":"attachment","identity":{"modelId":"x"}}"#;
 let padding=format!("{{\"type\":\"user\",\"content\":\"{}\"}}", "a".repeat(600000));
 let text=format!("{identity1}\n{padding}\n{identity2}\n{padding}\n{compact}\n");
 let q=root.join("middle-switch.jsonl");std::fs::write(&q,text).unwrap();assert_eq!(detect_window(&q),Some(1000000));
 println!("{{\"compact_usage\":10,\"spaced_equivalent_usage\":null,\"spaced_sidechain_not_skipped\":true,\"nonassistant_usage_counted\":77,\"repeated_records_total\":20,\"large_record_in_memory_usage\":10,\"large_record_file_usage\":null,\"middle_switch_to_standard_returns_old_window\":1000000}}");
}
