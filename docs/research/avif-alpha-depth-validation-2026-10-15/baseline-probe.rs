use std::io::Cursor;
use image::ImageDecoder;
fn main() {
 for name in ["primary8-alpha10.avif", "primary10-alpha8.avif"] {
  let bytes=std::fs::read(format!("docs/research/avif-alpha-depth-validation-2026-10-15/{name}")).unwrap();
  let decoder=image::codecs::avif::AvifDecoder::new(Cursor::new(&bytes));
  match decoder {
   Err(error)=>println!("{name}: constructor refused: {error}"),
   Ok(decoder)=>{
    println!("{name}: constructor ACCEPTED {:?}, {:?}",decoder.dimensions(),decoder.color_type());
    match std::panic::catch_unwind(std::panic::AssertUnwindSafe(||image::DynamicImage::from_decoder(decoder))) {
     Ok(Ok(frame))=>println!("{name}: conversion ACCEPTED {} bytes",frame.as_bytes().len()),
     Ok(Err(error))=>println!("{name}: conversion refused: {error}"),
     Err(_)=>println!("{name}: conversion PANICKED"),
    }
   }
  }
 }
}
