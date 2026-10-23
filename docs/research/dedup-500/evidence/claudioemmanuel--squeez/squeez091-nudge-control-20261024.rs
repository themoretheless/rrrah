use squeez::{config::Config,context::cache::{FileAccess,SessionContext},economy::{nudge::evaluate,enterprise::{detect_from,EnterpriseMode}}};
fn main(){
 let cfg=Config::default();let mut c=SessionContext::default();
 let a=evaluate(&mut c,"cat sleeping_notes.txt",&[],FileAccess::Read,&[],&cfg);assert!(a.is_empty());
 let b=evaluate(&mut c,"cat sleeping_notes.txt",&[],FileAccess::Read,&[],&cfg);assert!(b.iter().any(|s|s.contains("POLLING DETECTED")));
 let mut direct=SessionContext::default();let mut wrapped=SessionContext::default();let mut dh=0;let mut wh=0;
 for _ in 0..4 {dh+=evaluate(&mut direct,"cargo test",&[],FileAccess::Read,&[],&cfg).len();wh+=evaluate(&mut wrapped,"env cargo test",&[],FileAccess::Read,&[],&cfg).len();}
 assert_eq!(dh,1);assert_eq!(wh,0);
 let mut f=SessionContext::default();let paths=vec!["/owned/file.rs".to_string();5];let hints=evaluate(&mut f,"cmd",&paths,FileAccess::Write,&[],&cfg);assert_eq!(hints.len(),1);assert!(hints[0].contains("modified ×5"));
 assert_eq!(detect_from(&[("OTEL_EXPORTER_OTLP_ENDPOINT","")]),EnterpriseMode::Otel);
 assert_eq!(detect_from(&[("AWS_BEDROCK_REGION","us-east-1"),("ANTHROPIC_VERTEX_PROJECT_ID","p")]),EnterpriseMode::Vertex);
 println!("{}", r#"{"filename_substring_false_polling_hint": true, "direct_command_hint_count": 1, "env_wrapped_same_command_hint_count": 0, "duplicate_paths_one_call_count_as_five_writes": true, "empty_otel_endpoint_selects_otel": true, "vertex_project_precedes_bedrock_region": true}"#);
}
