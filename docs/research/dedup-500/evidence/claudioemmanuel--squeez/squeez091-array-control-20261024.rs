use squeez::{context::{cache::SessionContext,hash, redundancy},json_util};
fn main(){
    let long=(0..30).map(|i|format!("a word{i} stable tail token alpha beta gamma {i}")).collect::<Vec<_>>();
    let total=long.join("\n").len();let short=vec!["x".repeat(total/2),"y".repeat(total-total/2-1)];
    let mut ctx=SessionContext::default();redundancy::record(&mut ctx,"short-unrelated",&short);redundancy::record(&mut ctx,"long-actual",&long);
    assert!(ctx.call_log_shingles[0].is_empty());assert!(!ctx.call_log_shingles[1].is_empty());
    let mut query=long.clone();query[5]="a word5 STABLE TAIL token alpha beta gamma 5".into();
    let before=redundancy::check(&ctx,&query).unwrap();assert_eq!(before.call_n,2);
    let raw=ctx.to_json();std::fs::write(std::env::args_os().nth(1).unwrap(),&raw).unwrap();
    let loaded=SessionContext::from_json(&raw);let after=redundancy::check(&loaded,&query).unwrap();assert_eq!(after.call_n,1);assert_eq!(after.label,"short-unrelated");
    assert_eq!(hash::jaccard(&hash::shingle_minhash(&short.join("\n")),&hash::shingle_minhash(&query.join("\n"))),0.0);
    println!("{{\"scope\":\"Original record/check and SessionContext JSON roundtrip; synthetic two-line unrelated text and thirty-line candidate, equal byte lengths\",\"output_bytes\":{},\"before_call\":{},\"after_call\":{},\"before_label\":\"{}\",\"after_label\":\"{}\",\"shingle_arrays_before\":{},\"shingle_arrays_after\":{},\"unrelated_actual_shingle_similarity\":0.0,\"direct_bracket_array\":{},\"map_bracket_array\":{},\"empty_array_items_after_map\":{}}}",total,before.call_n,after.call_n,before.label,after.label,ctx.call_log_shingles.len(),loaded.call_log_shingles.len(),json_util::str_array(&json_util::extract_str_array(r#"{"a":["a]b","second"]}"#,"a")),json_util::str_array(&json_util::map_str_array(&json_util::extract_all(r#"{"a":["a]b","second"]}"#),"a")),json_util::str_array(&json_util::map_str_array(&json_util::extract_all(r#"{"a":["","alpha",""]}"#),"a")));
}
