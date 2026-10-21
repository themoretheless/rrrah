#[path = "../src/imaging.rs"]
mod imaging;
use image::{DynamicImage, ImageFormat, RgbImage};
use std::io::Cursor;
fn png(w:u32,h:u32)->Vec<u8>{let mut b=Cursor::new(Vec::new());DynamicImage::ImageRgb8(RgbImage::new(w,h)).write_to(&mut b,ImageFormat::Png).unwrap();b.into_inner()}
#[test]
fn review_thumbnail_must_not_upscale_as_documented(){
 let source=png(2,1);let output=imaging::thumbnail(&source,20,20,imaging::OutFormat::Png).unwrap();
 let info=imaging::image_info(&output).unwrap();println!("2x1 within20x20 became {}x{}",info.width,info.height);
 assert_eq!((info.width,info.height),(2,1),"documented never-upscale contract");
}
#[test]
fn review_info_requires_pixel_payload(){
 let source=png(2,1);let header=&source[..41];
 let dimensions=image::ImageReader::new(Cursor::new(header)).with_guessed_format().unwrap().into_dimensions().unwrap();
 assert_eq!(dimensions,(2,1));assert!(imaging::image_info(header).is_err());
 println!("header-only dimension oracle succeeds; image_info requires full decode");
}
