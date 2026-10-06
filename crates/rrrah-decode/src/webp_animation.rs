//! Selected WebP animation presentations with linear-light composition.
use crate::{
    DecodeRequest,
    bounded_io::read_bounded,
    raster::{MAX_RASTER_BYTES, RasterDecodeError},
};
use image::{DynamicImage, ImageDecoder, Limits, codecs::webp::WebPDecoder, metadata::Orientation};
use rrrah_core::{DecodedRaster, RasterColorSpace, RasterPixels};
use std::{io::Cursor, sync::Arc};
#[derive(Debug, Clone)]
pub struct WebpImage {
    pub raster: DecodedRaster,
    pub source_color_space: RasterColorSpace,
    pub delay_ms: u32,
    /// Zero means infinite; positive values are total plays.
    pub num_plays: u16,
}
fn bad(s: &'static str) -> RasterDecodeError {
    RasterDecodeError::InvalidWebpAnimation(s)
}
fn u24(b: &[u8]) -> u32 {
    u32::from(b[0]) | u32::from(b[1]) << 8 | u32::from(b[2]) << 16
}
pub(crate) fn has_animation(b: &[u8]) -> bool {
    b.starts_with(b"RIFF") && b.get(8..16) == Some(b"WEBPVP8X") && b.get(20).is_some_and(|v| v & 2 != 0)
}
fn chunks<'a>(b: &'a [u8], request: &DecodeRequest) -> Result<Vec<(&'a [u8], &'a [u8])>, RasterDecodeError> {
    let mut at = 0usize;
    let mut out = Vec::new();
    while at < b.len() {
        request.check_cancelled()?;
        let h = b
            .get(at..at.saturating_add(8))
            .ok_or_else(|| bad("truncated chunk"))?;
        let n = u32::from_le_bytes(h[4..8].try_into().unwrap()) as usize;
        let end = at
            .checked_add(8)
            .and_then(|v| v.checked_add(n))
            .filter(|&v| v <= b.len())
            .ok_or_else(|| bad("chunk range"))?;
        out.push((&h[..4], &b[at + 8..end]));
        at = end
            .checked_add(n & 1)
            .filter(|&v| v <= b.len())
            .ok_or_else(|| bad("chunk padding"))?;
        if n & 1 != 0 && b[end] != 0 {
            return Err(bad("nonzero padding"));
        }
        if out.len() > 65536 {
            return Err(bad("too many chunks"));
        }
    }
    Ok(out)
}
struct Frame<'a> {
    x: u32,
    y: u32,
    w: u32,
    h: u32,
    delay: u32,
    flags: u8,
    data: &'a [u8],
}
pub fn decode_webp_animation(request: &DecodeRequest) -> Result<WebpImage, RasterDecodeError> {
    request.check_cancelled()?;
    decode(&read_bounded(request)?, request)
}
pub(crate) fn decode(b: &[u8], request: &DecodeRequest) -> Result<WebpImage, RasterDecodeError> {
    request.check_cancelled()?;
    if !has_animation(b)
        || b.len() < 12
        || u32::from_le_bytes(b[4..8].try_into().unwrap()) as usize != b.len() - 8
    {
        return Err(bad("RIFF header/size"));
    }
    let cs = chunks(&b[12..], request)?;
    let first = cs.first().ok_or_else(|| bad("missing VP8X"))?;
    if first.0 != b"VP8X" || first.1.len() != 10 {
        return Err(bad("VP8X header"));
    }
    let (w, h) = (u24(&first.1[4..7]) + 1, u24(&first.1[7..10]) + 1);
    if u64::from(w) * u64::from(h) * 16 * 4 > MAX_RASTER_BYTES {
        return Err(RasterDecodeError::OutputTooLarge);
    }
    let mut anim = None;
    let mut frames = Vec::new();
    let mut icc = None;
    for &(kind, data) in &cs[1..] {
        match kind {
            b"VP8X" => return Err(bad("duplicate VP8X")),
            b"ICCP" => {
                if icc.is_some() || anim.is_some() {
                    return Err(bad("ICC order/duplicate"));
                }
                icc = Some(data);
            }
            b"ANIM" => {
                if anim.is_some() || data.len() != 6 || !frames.is_empty() {
                    return Err(bad("ANIM order/size"));
                }
                anim = Some(data);
            }
            b"ANMF" => {
                if anim.is_none() || data.len() < 16 || frames.len() >= 4096 {
                    return Err(bad("ANMF order/size/count"));
                }
                let f = Frame {
                    x: u24(&data[..3]) * 2,
                    y: u24(&data[3..6]) * 2,
                    w: u24(&data[6..9]) + 1,
                    h: u24(&data[9..12]) + 1,
                    delay: u24(&data[12..15]),
                    flags: data[15],
                    data: &data[16..],
                };
                if f.x + f.w > w || f.y + f.h > h {
                    return Err(bad("frame geometry"));
                }
                let nested = chunks(f.data, request)?;
                if !matches!(
                    nested.as_slice(),
                    [(b"VP8L", _)] | [(b"VP8 ", _)] | [(b"ALPH", _), (b"VP8 ", _)]
                ) {
                    return Err(bad("frame image chunks"));
                }
                frames.push(f);
            }
            b"VP8 " | b"VP8L" | b"ALPH" => return Err(bad("still payload in animation")),
            _ => (),
        }
    }
    let anim = anim.ok_or_else(|| bad("missing ANIM"))?;
    if frames.is_empty() || (first.1[0] & 32 != 0) != icc.is_some() {
        return Err(bad("frame count/ICC flag"));
    }
    if request.image_index >= frames.len() {
        return Err(crate::DecodeError::UnsupportedImageIndex {
            index: request.image_index,
        }
        .into());
    }
    let mut exif = cs.iter().filter(|c| c.0 == b"EXIF");
    if let Some((_, data)) = exif.next() {
        if exif.next().is_some() {
            return Err(bad("duplicate EXIF"));
        }
        if Orientation::from_exif_chunk(data).unwrap_or(Orientation::NoTransforms)
            != Orientation::NoTransforms
        {
            return Err(bad("animated EXIF orientation unsupported"));
        }
    }
    let color = icc.map_or(RasterColorSpace::Srgb, |v| RasterColorSpace::Icc(v.to_vec()));
    let background = DecodedRaster::new(
        1,
        1,
        RasterPixels::Rgba8(Arc::new(vec![anim[2], anim[1], anim[0], anim[3]]).into()),
        color.clone(),
    )?;
    let prepared = crate::prepare_raster_for_display(&background)?;
    let RasterPixels::Rgba32Float(background) = prepared.pixels() else {
        unreachable!()
    };
    let mut canvas = background.repeat((w as usize) * (h as usize));
    let start = (0..=request.image_index)
        .rev()
        .find(|&i| {
            let f = &frames[i];
            f.x == 0 && f.y == 0 && f.w == w && f.h == h && f.flags & 2 != 0
        })
        .unwrap_or(0);
    for (i, f) in frames
        .iter()
        .enumerate()
        .skip(start)
        .take(request.image_index - start + 1)
    {
        request.check_cancelled()?;
        // ALPH + VP8 needs an extended standalone container. VP8L also
        // accepts this wrapper; opaque VP8 must not advertise a missing ALPH chunk.
        let mut standalone = b"RIFF".to_vec();
        standalone.extend_from_slice(&((f.data.len() + 22) as u32).to_le_bytes());
        standalone.extend_from_slice(b"WEBPVP8X");
        standalone.extend_from_slice(&10u32.to_le_bytes());
        let has_alpha = f.data.starts_with(b"ALPH")
            || (f.data.starts_with(b"VP8L") && f.data.get(12).is_some_and(|v| v & 16 != 0));
        standalone.extend_from_slice(&[if has_alpha { 16 } else { 0 }, 0, 0, 0]);
        standalone.extend_from_slice(&(f.w - 1).to_le_bytes()[..3]);
        standalone.extend_from_slice(&(f.h - 1).to_le_bytes()[..3]);
        standalone.extend_from_slice(f.data);
        let mut decoder = WebPDecoder::new(Cursor::new(standalone))?;
        if decoder.dimensions() != (f.w, f.h) {
            return Err(bad("frame bitstream dimensions"));
        }
        let mut limits = Limits::default();
        limits.max_alloc = Some(MAX_RASTER_BYTES);
        decoder.set_limits(limits)?;
        let raw = DynamicImage::from_decoder(decoder)?.into_rgba8().into_raw();
        let source = DecodedRaster::new(f.w, f.h, RasterPixels::Rgba8(Arc::new(raw).into()), color.clone())?;
        let prepared = crate::prepare_raster_for_display(&source)?;
        let RasterPixels::Rgba32Float(raw) = prepared.pixels() else {
            unreachable!()
        };
        for y in 0..f.h as usize {
            request.check_cancelled()?;
            for x in 0..f.w as usize {
                let src = (y * f.w as usize + x) * 4;
                let dst = ((y + f.y as usize) * w as usize + x + f.x as usize) * 4;
                if f.flags & 2 != 0 {
                    canvas[dst..dst + 4].copy_from_slice(&raw[src..src + 4]);
                } else {
                    crate::animation::over(&mut canvas[dst..dst + 4], &raw[src..src + 4]);
                }
            }
        }
        if i == request.image_index {
            break;
        }
        if f.flags & 1 != 0 {
            for y in 0..f.h as usize {
                let at = ((y + f.y as usize) * w as usize + f.x as usize) * 4;
                for p in canvas[at..at + f.w as usize * 4].chunks_exact_mut(4) {
                    p.copy_from_slice(background);
                }
            }
        }
    }
    let raster = DecodedRaster::new(
        w,
        h,
        RasterPixels::Rgba32Float(Arc::new(canvas).into()),
        RasterColorSpace::LinearSrgb,
    )?
    .with_image_selection(request.image_index, frames.len())?;
    Ok(WebpImage {
        raster,
        source_color_space: color,
        delay_ms: frames[request.image_index].delay,
        num_plays: u16::from_le_bytes(anim[4..6].try_into().unwrap()),
    })
}
#[cfg(test)]
mod tests {
    use super::*;
    fn root() -> std::path::PathBuf {
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/raster")
    }
    #[test]
    fn selected_linear_presentations_timing_and_disposal() {
        let root = root();
        for line in std::fs::read_to_string(root.join("webp-animation-manifest.tsv"))
            .unwrap()
            .lines()
            .filter(|s| !s.starts_with('#'))
        {
            let c: Vec<_> = line.split('\t').collect();
            let mut request = DecodeRequest::new(root.join(c[0]));
            request.image_index = c[1].parse().unwrap();
            let image = decode_webp_animation(&request).unwrap();
            assert_eq!(image.delay_ms, c[4].parse::<u32>().unwrap());
            assert_eq!(image.num_plays, c[5].parse::<u16>().unwrap());
            assert_eq!((image.raster.width(), image.raster.height()), (5, 3));
            assert_eq!(
                (image.raster.image_index(), image.raster.image_count()),
                (request.image_index, 4)
            );
            let RasterPixels::Rgba32Float(pixels) = image.raster.pixels() else {
                panic!()
            };
            let bytes = std::fs::read(root.join(c[6])).unwrap();
            for (&actual, b) in pixels.iter().zip(bytes.chunks_exact(4)) {
                assert!(
                    (actual - f32::from_le_bytes(b.try_into().unwrap())).abs() < 2e-7,
                    "{} frame {}",
                    c[0],
                    c[1]
                );
            }
            assert_eq!(crate::decode_raster(&request).unwrap().image_count(), 4);
        }
    }
    fn chunk(kind: &[u8; 4], data: &[u8]) -> Vec<u8> {
        let mut b = kind.to_vec();
        b.extend_from_slice(&(data.len() as u32).to_le_bytes());
        b.extend_from_slice(data);
        if data.len() % 2 != 0 {
            b.push(0);
        }
        b
    }
    fn repair_size(b: &mut [u8]) {
        let size = (b.len() - 8) as u32;
        b[4..8].copy_from_slice(&size.to_le_bytes());
    }
    #[test]
    fn icc_color_and_animated_orientation_are_explicit() {
        let root = root();
        let request = DecodeRequest::new(root.join("webp-animation-0.webp"));
        let original = std::fs::read(&request.path).unwrap();
        let png = std::fs::read(root.join("pattern.profiled.png")).unwrap();
        let mut decoder = image::codecs::png::PngDecoder::new(Cursor::new(png)).unwrap();
        let icc = decoder.icc_profile().unwrap().unwrap();
        let mut b = original.clone();
        b[20] |= 32;
        b.splice(30..30, chunk(b"ICCP", &icc));
        repair_size(&mut b);
        let image = decode(&b, &request).unwrap();
        assert_eq!(image.source_color_space, RasterColorSpace::Icc(icc));
        let RasterPixels::Rgba32Float(pixels) = image.raster.pixels() else {
            panic!()
        };
        assert!((pixels[0] - 1.).abs() < 1e-3);
        assert_eq!(pixels[3], 1.);
        let mut invalid = original.clone();
        invalid[20] |= 32;
        assert!(decode(&invalid, &request).is_err());
        let exif = b"II\x2a\0\x08\0\0\0\x01\0\x12\x01\x03\0\x01\0\0\0\x06\0\0\0\0\0\0\0";
        let mut oriented = original;
        oriented[20] |= 8;
        oriented.extend_from_slice(&chunk(b"EXIF", exif));
        repair_size(&mut oriented);
        assert!(matches!(
            decode(&oriented, &request),
            Err(RasterDecodeError::InvalidWebpAnimation(_))
        ));
    }
    #[test]
    fn truncated_ranges_and_lazy_source_restart() {
        let path = root().join("webp-animation-0.webp");
        let mut b = std::fs::read(&path).unwrap();
        let mut request = DecodeRequest::new(path);
        for n in 0..b.len() {
            assert!(decode(&b[..n], &request).is_err(), "prefix {n}");
        }
        request.image_index = 4;
        assert!(decode(&b, &request).is_err());
        request.image_index = 0;
        let mut earlier = b.clone();
        let cs = chunks(&earlier[12..], &request).unwrap();
        let f = cs.iter().find(|c| c.0 == b"ANMF").unwrap().1;
        let at = f.as_ptr() as usize - earlier.as_ptr() as usize + 24;
        earlier[at] = 0;
        request.image_index = 3;
        assert!(decode(&earlier, &request).is_ok());
        request.image_index = 0;
        assert!(decode(&earlier, &request).is_err());
        let cs = chunks(&b[12..], &request).unwrap();
        let f = cs.iter().filter(|c| c.0 == b"ANMF").last().unwrap().1;
        let offset = f.as_ptr() as usize - b.as_ptr() as usize + 24;
        b[offset] = 0;
        assert!(decode(&b, &request).is_ok());
        request.image_index = 3;
        assert!(decode(&b, &request).is_err());
    }
}
