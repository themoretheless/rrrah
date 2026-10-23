use squeez::{json_util,strategies::{toon,smart_filter}};
fn doc(value:&str)->String {
 let row=format!(r#"{{"repeated_value_field_for_qualification":{value},"repeated_label_field_for_qualification":"stable"}}"#);
 format!("[{row},{row},{row}]")
}
fn main(){
 let cases=[(r#""true""#,"true"),(r#""42""#,"42"),(r#""null""#,"null"),(r#""[1,2]""#,"[1,2]"),(r#""a\bb""#,r#""ab""#),(r#""\uD83D\uDE00""#,r#""""#)];
 println!("{{\"scope\":\"Unchanged public strategies with synthetic JSON and diagnostic lines; no full wrapper or TOON decoder qualification\",\"encoding_collisions\":[");
 for (i,(a,b)) in cases.into_iter().enumerate(){
  let left=doc(a);let right=doc(b);let x=toon::try_to_toon(&left).unwrap();let y=toon::try_to_toon(&right).unwrap();assert_eq!(x,y);
  println!("{}{{\"input_json\":{},\"encoded_output\":{},\"outputs_equal\":true}}",if i==0{""}else{","},json_util::str_array(&[left,right]),json_util::str_array(&[x]));
 }
 let diagnostics=["ERROR: unsuccessful tests 0%","[assert] x => error","ERROR: cannot open input"];
 let filtered=diagnostics.map(|line|smart_filter::apply(vec![line.to_owned()]));
 assert!(filtered[0].is_empty());assert!(filtered[1].is_empty());assert_eq!(filtered[2],vec![diagnostics[2]]);
 println!("],\"diagnostic_filter\":{{\"inputs\":{},\"output_lengths\":[{},{},{}]}}}}",json_util::str_array(&diagnostics.map(str::to_owned)),filtered[0].len(),filtered[1].len(),filtered[2].len());
}
