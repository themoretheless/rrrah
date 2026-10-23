use squeez::{config::Config,context::{factsheet,summarize::{self,SummaryFormat}},json_util};
fn main(){
 let negative=factsheet::extract("delta -1234567");let positive=factsheet::extract("delta +1234567");assert_eq!(negative,positive);assert_eq!(negative,vec!["1234567"]);
 let versions=(0..512).map(|i|format!("v0.1.{i} ")).collect::<String>();let late=factsheet::extract(&format!("{versions}a1347daf9"));let early=factsheet::extract(&format!("a1347daf9 {versions}"));assert!(late.is_empty());assert_eq!(early,vec!["a1347daf9"]);
 let mut minus=vec!["delta -1234567".to_string()];minus.extend((0..80).map(|_|"ordinary filler text".to_string()));let mut plus=minus.clone();plus[0]="delta +1234567".into();
 let prose_equal=summarize::apply_with_format(minus.clone(),"owned",SummaryFormat::Prose)==summarize::apply_with_format(plus.clone(),"owned",SummaryFormat::Prose);
 let structured_equal=summarize::apply_with_format(minus,"owned",SummaryFormat::Structured)==summarize::apply_with_format(plus,"owned",SummaryFormat::Structured);assert!(prose_equal&&structured_equal);
 let huge=format!(r#"{{"payload":"{}"}}"#,"x".repeat(60000));let triggered=summarize::should_apply(&[huge.clone()],&Config::default());assert!(triggered);let summary=summarize::apply(vec![huge.clone()],"owned");assert_eq!(summary.last(),Some(&huge));let bytes=summary.join("\n").len();assert!(bytes>huge.len());
 println!("{{\"scope\":\"Original public factsheet and summary APIs, finite synthetic strings; no complete wrapper or filesystem identity claim\",\"signed_integer_facts\":{},\"both_summary_formats_erase_sign_difference\":true,\"late_sha_after_512_candidates\":{},\"early_sha_before_512_candidates\":{},\"long_line\":{{\"input_bytes\":{},\"byte_trigger\":{},\"summary_bytes\":{},\"input_preserved_verbatim_as_tail\":true,\"byte_reduction\":false}}}}",json_util::str_array(&negative),json_util::str_array(&late),json_util::str_array(&early),huge.len(),triggered,bytes);
}
