#[derive(Debug)] enum DecodeError { Memory(rrrah_core::BufferError) }
// Diagnostic integration of current AVIF metadata/conversion with the independent
// image/dav1d backend. This does not replace the public decode/cache/Metal gates.
mod raster { #[derive(Debug)] pub enum RasterDecodeError { InvalidAvif(&'static str), Source(crate::DecodeError) } }
struct DecodeRequest;
impl DecodeRequest { fn check_cancelled(&self)->Result<(),raster::RasterDecodeError>{Ok(())} }
#[path="/Users/themoretheless/Documents/Sources/rrrah/crates/rrrah-decode/src/avif_color.rs"]
mod avif_color;
fn main() {
    let root=std::path::PathBuf::from(std::env::args_os().nth(1).unwrap());
    let request=DecodeRequest;
    for depth in [8u8,10] {
        for prem in [false,true] {
            let name=format!("ravif-{depth}bit-{}.avif",if prem {"prem"}else{"straight"});
            let data=std::fs::read(root.join(&name)).unwrap();
            let ctx=mp4parse::read_avif(&mut std::io::Cursor::new(&data),mp4parse::ParseStrictness::Normal).unwrap();
            assert_eq!(ctx.premultiplied_alpha,prem);
            let metadata=avif_color::properties_with_budget(&data,&||Ok(()),None).unwrap();
            assert_eq!(metadata.premultiplied_alpha,prem);
            assert_eq!(metadata.bit_depth,Some(depth));
            let decoded=image::load_from_memory_with_format(&data,image::ImageFormat::Avif).unwrap();
            assert_eq!((decoded.width(),decoded.height()),(16,16));
            if depth==8 {
                let source=decoded.into_rgba8().into_raw(); let mut actual=source.clone();
                if prem { avif_color::unassociate_u8(&mut actual,&request).unwrap(); }
                for (a,s) in actual.chunks_exact(4).zip(source.chunks_exact(4)) {
                    assert_eq!(a[3],s[3]);
                    for c in 0..3 { let expected=if !prem {s[c]}else if s[3]==0 {0}else {
                        (f64::from(s[c])*255.0/f64::from(s[3])).round().min(255.0) as u8
                    }; assert_eq!(a[c],expected); }
                }
                if prem {assert_ne!(source,actual);}
                std::fs::write(root.join(format!("{name}.coded-rgba")),&source).unwrap();
                std::fs::write(root.join(format!("{name}.rgba")),&actual).unwrap();
            } else {
                let source=decoded.into_rgba16().into_raw(); let mut actual=source.clone();
                if prem {
                    avif_color::unassociate_u16(&mut actual,depth,&request).unwrap();
                    let mut max_double_rounding_delta = 0u16;
                    for (a,s) in actual.chunks_exact(4).zip(source.chunks_exact(4)) {
                        let alpha=if s[3]==65535 {1023}else{s[3]>>6};
                        assert_eq!(a[3],(f64::from(alpha)*65535.0/1023.0).round() as u16);
                        for c in 0..3 {let expected=if alpha==0 {0}else {
                            (f64::from(s[c]>>6)*65535.0/f64::from(alpha)).round().min(65535.0) as u16
                        };assert_eq!(a[c],expected);
                        let expanded = (f64::from(s[c]>>6)*65535.0/1023.0).round();
                        let expanded_alpha = (f64::from(alpha)*65535.0/1023.0).round();
                        let old = if expanded_alpha == 0.0 {0}else{
                            (expanded*65535.0/expanded_alpha).round().min(65535.0) as u16
                        };
                        max_double_rounding_delta = max_double_rounding_delta.max(a[c].abs_diff(old));
                        }
                    }
                    assert_ne!(source,actual);
                    assert!(max_double_rounding_delta > 0,"fixture must distinguish division before expansion");
                    println!("10-bit prem control: maximum avoided double-rounding delta={max_double_rounding_delta}");
                }
                let mut normalized = actual;
                if !prem {
                    for pixel in normalized.chunks_exact_mut(4) {
                        for (c,value) in pixel.iter_mut().enumerate() {
                            if c==3 && *value==65535 {continue;}
                            *value=(f64::from(*value>>6)*65535.0/1023.0).round() as u16;
                        }
                    }
                }
                let raw:Vec<u8>=source.iter().flat_map(|v|v.to_le_bytes()).collect();
                let reference:Vec<u8>=normalized.iter().flat_map(|v|v.to_le_bytes()).collect();
                std::fs::write(root.join(format!("{name}.coded-rgba16le")),raw).unwrap();
                std::fs::write(root.join(format!("{name}.rgba16le")),reference).unwrap();
            }
            println!("{name}: independent parser prem={prem}; depth={depth}; 256 pixels; metadata/decode passed; prem rational conversion checked={prem}; color={:?}",metadata.color);
        }
    }
}
