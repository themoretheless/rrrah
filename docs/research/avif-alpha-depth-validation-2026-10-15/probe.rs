use std::io::Cursor;
use image::ImageDecoder;
fn main() {
 for name in ["primary8-alpha10.avif", "primary10-alpha8.avif"] {
  let bytes=std::fs::read(format!("docs/research/avif-alpha-depth-validation-2026-10-15/{name}")).unwrap();
  for limit in [0,256] {
   let error=match image::codecs::avif::AvifDecoder::new_with_pixel_limit(Cursor::new(&bytes),limit) { Ok(_)=>panic!("mixed depth accepted"),Err(error)=>error };
   assert!(error.to_string().contains("alpha bit depth differs"));
   println!("{name}: limit{limit} refused before conversion: {error}");
  }
 }
 for name in ["ravif-8bit-prem.avif","ravif-10bit-prem.avif"] {
  let bytes=std::fs::read(format!("tests/fixtures/raster/{name}")).unwrap();
  let decoder=image::codecs::avif::AvifDecoder::new(Cursor::new(&bytes)).unwrap();
  assert_eq!(decoder.dimensions(),(16,16));assert!(image::DynamicImage::from_decoder(decoder).is_ok());
  println!("{name}: same-depth retry accepted");
 }
}
