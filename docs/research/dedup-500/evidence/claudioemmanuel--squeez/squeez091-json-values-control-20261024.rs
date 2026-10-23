use squeez::json_util;
fn main(){
 let surrogate=json_util::parse_value(r#""\uD83D\uDE00""#).unwrap();let text=surrogate.as_str().unwrap();assert_eq!(text,"\u{fffd}\u{fffd}");
 let exact=9007199254740993u64;let rounded=json_util::parse_value(&exact.to_string()).unwrap().as_u64().unwrap();assert_ne!(exact,rounded);
 let integer=json_util::extract_u64(&format!("{{\"id\":{exact}}}"),"id").unwrap();assert_eq!(integer,exact);
 assert!(json_util::parse_value("true trailing").is_some());
 let unknown=json_util::parse_value(r#""a\q""#).unwrap();assert_eq!(unknown.as_str(),Some("aq"));
 println!(r#"{{"scope":"Original public recursive JSON parser and exact integer extractor, finite scalar inputs","valid_surrogate_pair_expected_codepoint":128512,"actual_codepoints":[65533,65533],"integer_input":{exact},"recursive_parser_as_u64":{rounded},"flat_integer_extractor":{integer},"trailing_non_json_accepted":true,"unknown_string_escape_accepted":true,"important_scope":"SessionContext uses integer extractors, so recursive f64 rounding is not evidence that its hash arrays lose precision."}}"#);
}
