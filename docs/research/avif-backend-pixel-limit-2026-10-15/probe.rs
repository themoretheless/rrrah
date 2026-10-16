use image::ImageDecoder;
use std::io::Cursor;
fn main() {
    for name in ["ravif-8bit-prem.avif", "ravif-10bit-prem.avif"] {
        let data = std::fs::read(format!("tests/fixtures/raster/{name}")).unwrap();
        let error = match image::codecs::avif::AvifDecoder::new_with_pixel_limit(Cursor::new(&data), 255) {
            Ok(_) => panic!("oversized coded frame accepted"),
            Err(error) => error,
        };
        let decoder = image::codecs::avif::AvifDecoder::new_with_pixel_limit(Cursor::new(&data), 256).unwrap();
        assert_eq!(decoder.dimensions(), (16,16));
        let bounded = image::DynamicImage::from_decoder(decoder).unwrap();
        let ordinary = image::DynamicImage::from_decoder(image::codecs::avif::AvifDecoder::new(Cursor::new(&data)).unwrap()).unwrap();
        assert_eq!(bounded.as_bytes(), ordinary.as_bytes());
        println!("{name}: limit255 refused={error}; limit256 accepted; raw pixels match default exactly");
    }
    let data=std::fs::read("docs/research/avif-backend-pixel-limit-2026-10-15/small-primary-large-alpha.avif").unwrap();
    assert!(image::codecs::avif::AvifDecoder::new_with_pixel_limit(Cursor::new(&data),32).is_err());
    let decoder=image::codecs::avif::AvifDecoder::new_with_pixel_limit(Cursor::new(&data),256).unwrap();
    assert_eq!(decoder.dimensions(),(8,4));
    println!("alpha-only oversized control: primary8x4 passes32; alpha16x16 refuses32; constructor accepts256; no malformed-frame rendering attempted");
}
