fn main() {
 let root=rrrah_core::MemoryBudget::new(1024*1024);
 for name in ["avif-mixed-primary8-alpha10.avif","avif-mixed-primary10-alpha8.avif"] {
  let mut request=rrrah_decode::DecodeRequest::new(format!("tests/fixtures/raster/{name}"));
  request.memory_budget=Some(root.clone());
  let error=rrrah_decode::decode_raster(&request).unwrap_err();
  assert!(error.to_string().contains("alpha bit depth differs"));
  assert_eq!(root.used(),0);
  println!("{name}: public decode refused; root=0");
 }
 for name in ["ravif-8bit-prem.avif","ravif-10bit-prem.avif"] {
  let mut request=rrrah_decode::DecodeRequest::new(format!("tests/fixtures/raster/{name}"));
  request.memory_budget=Some(root.clone());
  let raster=rrrah_decode::decode_raster(&request).unwrap();
  assert!(root.used()>0);drop(raster);assert_eq!(root.used(),0);
  println!("{name}: public retry accepted; root=0");
 }
}
