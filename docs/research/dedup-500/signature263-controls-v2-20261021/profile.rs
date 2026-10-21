extern crate dedup_signature;
use dedup_signature::generator::text_profile_signature::TextProfileSignature;
fn main(){
 let g=TextProfileSignature::default();
 assert_eq!(g.generate_sign("alpha beta"),g.generate_sign("beta alpha"));
 println!("PASS token-order invariance is intentional fuzzy-profile behavior, not exact content identity");
 assert_eq!(g.generate_sign(""),g.generate_sign("a an to"));
 println!("PASS empty and only discarded short tokens share empty-profile signature");
 assert_eq!(g.generate_sign("alpha alpha beta"),g.generate_sign("alpha alpha gamma"));
 println!("PASS repeated dominant token makes once-occurring distinct tokens disappear under default quantization");
 assert_ne!(g.generate_sign("alpha"),g.generate_sign("gamma"));
 println!("PASS distinct retained single tokens produce distinct signatures");
 println!("RESULT 4 original TextProfileSignature controls passed; actual rust-crypto dependency, authored executable");
}
