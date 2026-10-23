use squeez::context::cache::SessionContext;
fn main() {
    let cases=[
        ("empty_object", "{}"),
        ("legacy_no_shingles", r#"{"call_counter":7,"call_log_n":[1],"call_log_cmd":["legacy"],"call_log_hash":[123],"call_log_len":[10],"call_log_short":["00000000"]}"#),
        ("mismatched_arrays", r#"{"call_log_n":[1,2],"call_log_cmd":["one"],"call_log_hash":[123],"call_log_len":[10],"call_log_short":["00000000"]}"#),
        ("truncated_object", r#"{"call_counter":7"#),
    ];
    println!("{{\"scope\":\"Original public from_json, four finite synthetic inputs; parser acceptance is not JSON/schema validation\",\"cases\":[");
    for (i,(label,raw)) in cases.into_iter().enumerate() {
        let c=SessionContext::from_json(raw);
        println!("{}{{\"case\":\"{}\",\"call_counter\":{},\"call_log_entries\":{},\"shingle_entries\":{}}}",if i==0{""}else{","},label,c.call_counter,c.call_log.len(),c.call_log_shingles.len());
    }
    println!("]}}");
}
