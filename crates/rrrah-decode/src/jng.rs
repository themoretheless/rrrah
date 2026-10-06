//! Standalone JNG JPEG color with PNG or JPEG straight alpha.
use crate::{
    DecodeRequest,
    raster::{MAX_RASTER_BYTES, RasterDecodeError},
};
use image::{
    DynamicImage, ImageDecoder, Limits,
    codecs::{jpeg::JpegDecoder, png::PngDecoder},
};
use rrrah_core::{DecodedRaster, RasterColorSpace, RasterPixels};
use std::{
    io::{Cursor, Write},
    sync::Arc,
};
const MAGIC: &[u8] = b"\x8bJNG\r\n\x1a\n";
pub(crate) fn has_magic(b: &[u8]) -> bool {
    b.starts_with(MAGIC)
}
fn bad(s: &'static str) -> RasterDecodeError {
    RasterDecodeError::InvalidJng(s)
}
fn chunk(out: &mut Vec<u8>, kind: &[u8], data: &[u8]) {
    out.extend_from_slice(&(data.len() as u32).to_be_bytes());
    let start = out.len();
    out.extend_from_slice(kind);
    out.extend_from_slice(data);
    let mut crc = flate2::Crc::new();
    crc.update(&out[start..]);
    out.extend_from_slice(&crc.sum().to_be_bytes());
}
fn png(w: u32, h: u32, depth: u8, color_type: u8, interlace: u8, parts: &[&[u8]]) -> Vec<u8> {
    let mut out = b"\x89PNG\r\n\x1a\n".to_vec();
    let mut hdr = w.to_be_bytes().to_vec();
    hdr.extend_from_slice(&h.to_be_bytes());
    hdr.extend_from_slice(&[depth, color_type, 0, 0, interlace]);
    chunk(&mut out, b"IHDR", &hdr);
    for part in parts {
        chunk(&mut out, b"IDAT", part);
    }
    chunk(&mut out, b"IEND", &[]);
    out
}
fn validate_jpeg_header(b: &[u8], components: u8, interlace: u8) -> Result<(), RasterDecodeError> {
    if !b.starts_with(&[255, 216]) {
        return Err(bad("JPEG signature"));
    }
    let mut at = 2usize;
    while at < b.len() {
        if b[at] != 255 {
            return Err(bad("JPEG marker"));
        }
        while b.get(at) == Some(&255) {
            at += 1;
        }
        let marker = *b.get(at).ok_or_else(|| bad("JPEG marker"))?;
        at += 1;
        if marker == 0xda || marker == 0xd9 {
            return Err(bad("missing JPEG frame header"));
        }
        let len = b
            .get(at..at + 2)
            .map(|v| u16::from_be_bytes(v.try_into().unwrap()) as usize)
            .ok_or_else(|| bad("JPEG segment"))?;
        let end = at
            .checked_add(len)
            .filter(|&v| len >= 2 && v <= b.len())
            .ok_or_else(|| bad("JPEG segment bounds"))?;
        if matches!(marker, 0xc0 | 0xc1 | 0xc2) {
            let data = &b[at + 2..end];
            if data.len() != 6 + 3 * components as usize
                || data[0] != 8
                || data[5] != components
                || (marker == 0xc2) != (interlace == 8)
            {
                return Err(bad("JPEG source channels/depth/interlace"));
            }
            return Ok(());
        }
        at = end;
    }
    Err(bad("missing JPEG frame header"))
}
pub(crate) fn decode(b: &[u8], request: &DecodeRequest) -> Result<DecodedRaster, RasterDecodeError> {
    request.check_cancelled()?;
    if !has_magic(b) {
        return Err(bad("signature"));
    }
    let mut at = 8usize;
    let mut hdr = None;
    let mut jpeg = Vec::new();
    let mut alpha = Vec::new();
    let mut png_alpha = Vec::new();
    let mut color_chunks = Vec::new();
    let mut ended = false;
    let mut count = 0;
    while at < b.len() {
        request.check_cancelled()?;
        count += 1;
        if count > 65536 {
            return Err(bad("too many chunks"));
        }
        let h = b
            .get(at..at.saturating_add(8))
            .ok_or_else(|| bad("chunk header"))?;
        let n = u32::from_be_bytes(h[..4].try_into().unwrap()) as usize;
        let end = at
            .checked_add(n)
            .and_then(|v| v.checked_add(12))
            .filter(|&v| v <= b.len())
            .ok_or_else(|| bad("chunk bounds"))?;
        let mut crc = flate2::Crc::new();
        for part in b[at + 4..end - 4].chunks(65536) {
            request.check_cancelled()?;
            crc.update(part);
        }
        if crc.sum() != u32::from_be_bytes(b[end - 4..end].try_into().unwrap()) {
            return Err(bad("chunk CRC"));
        }
        let kind = &h[4..8];
        let data = &b[at + 8..end - 4];
        if hdr.is_none() && kind != b"JHDR" {
            return Err(bad("JHDR must be first"));
        }
        match kind {
            b"JHDR" => {
                if hdr.is_some() || n != 16 {
                    return Err(bad("JHDR size/duplicate"));
                }
                hdr = Some(data);
            }
            b"JDAT" => jpeg.push(data),
            b"IDAT" => png_alpha.push(data),
            b"JDAA" => alpha.push(data),
            b"sRGB" | b"iCCP" | b"gAMA" | b"cHRM" => {
                if !jpeg.is_empty() || !alpha.is_empty() || !png_alpha.is_empty() {
                    return Err(bad("color chunk after pixels"));
                }
                color_chunks.push((kind, data));
            }
            b"IEND" => {
                if n != 0 || end != b.len() {
                    return Err(bad("IEND/trailing data"));
                }
                ended = true;
            }
            _ if kind[0] & 32 == 0 => return Err(bad("unsupported critical chunk")),
            _ => (),
        }
        at = end;
    }
    let hdr = hdr.ok_or_else(|| bad("missing JHDR"))?;
    let (w, h) = (
        u32::from_be_bytes(hdr[..4].try_into().unwrap()),
        u32::from_be_bytes(hdr[4..8].try_into().unwrap()),
    );
    let has_alpha = matches!(hdr[8], 12 | 14);
    if !ended
        || jpeg.is_empty()
        || w == 0
        || h == 0
        || w > 65536
        || h > 65536
        || u64::from(w) * u64::from(h) * 32 > MAX_RASTER_BYTES
    {
        return Err(bad("image bounds/data"));
    }
    if !matches!(hdr[8], 8 | 10 | 12 | 14) || hdr[9] != 8 || hdr[10] != 8 || !matches!(hdr[11], 0 | 8) {
        return Err(bad("unsupported JPEG color/depth/storage"));
    }
    if !has_alpha {
        if hdr[12..] != [0, 0, 0, 0] || !alpha.is_empty() || !png_alpha.is_empty() {
            return Err(bad("unexpected alpha"));
        }
    } else if hdr[13] == 0 {
        if !matches!(hdr[12], 1 | 2 | 4 | 8 | 16)
            || hdr[14] != 0
            || hdr[15] > 1
            || png_alpha.is_empty()
            || !alpha.is_empty()
        {
            return Err(bad("PNG alpha parameters"));
        }
    } else if hdr[13] == 8 {
        if hdr[12] != 8
            || hdr[14] != 0
            || !matches!(hdr[15], 0 | 8)
            || alpha.is_empty()
            || !png_alpha.is_empty()
        {
            return Err(bad("JPEG alpha parameters"));
        }
    } else {
        return Err(bad("unsupported alpha compression"));
    }
    let limits = || {
        let mut l = Limits::default();
        l.max_alloc = Some(MAX_RASTER_BYTES);
        l.max_image_width = Some(65536);
        l.max_image_height = Some(65536);
        l
    };
    let jpeg = jpeg.concat();
    validate_jpeg_header(&jpeg, if matches!(hdr[8], 8 | 12) { 1 } else { 3 }, hdr[11])?;
    let mut decoder = JpegDecoder::new(Cursor::new(jpeg))?;
    decoder.set_limits(limits())?;
    if decoder.dimensions() != (w, h)
        || decoder.color_type()
            != if matches!(hdr[8], 8 | 12) {
                image::ColorType::L8
            } else {
                image::ColorType::Rgb8
            }
    {
        return Err(bad("JPEG dimensions/channels disagree"));
    }
    let rgb = DynamicImage::from_decoder(decoder)?.into_rgb8().into_raw();
    let alpha_image = if !has_alpha {
        None
    } else if hdr[13] == 0 {
        let mut d = PngDecoder::new(Cursor::new(png(w, h, hdr[12], 0, hdr[15], &png_alpha)))?;
        d.set_limits(limits())?;
        Some(DynamicImage::from_decoder(d)?)
    } else {
        let alpha = alpha.concat();
        validate_jpeg_header(&alpha, 1, hdr[15])?;
        let mut d = JpegDecoder::new(Cursor::new(alpha))?;
        d.set_limits(limits())?;
        if d.dimensions() != (w, h) || d.color_type() != image::ColorType::L8 {
            return Err(bad("JPEG alpha dimensions/channels"));
        }
        Some(DynamicImage::from_decoder(d)?)
    };
    // Color declarations belong to the outer container, independently of alpha.
    let mut z = flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::fast());
    let gray = matches!(hdr[8], 8 | 12);
    z.write_all(if gray { &[0, 0][..] } else { &[0, 0, 0, 0][..] })
        .map_err(|_| bad("color carrier compression"))?;
    let compressed = z.finish().map_err(|_| bad("color carrier compression"))?;
    let carrier = png(1, 1, 8, if gray { 0 } else { 2 }, 0, &[&compressed]);
    let mut metadata = carrier[..33].to_vec();
    for (kind, data) in color_chunks {
        chunk(&mut metadata, kind, data);
    }
    metadata.extend_from_slice(&carrier[33..]);
    let declaration = crate::png_color::declaration(&metadata)?;
    let mut d = PngDecoder::new(Cursor::new(metadata))?;
    let icc = d.icc_profile()?;
    let color = if declaration.override_icc {
        declaration.color.unwrap_or(RasterColorSpace::Unspecified)
    } else {
        icc.map(RasterColorSpace::Icc)
            .or(declaration.color)
            .unwrap_or(RasterColorSpace::Unspecified)
    };
    request.check_cancelled()?;
    let pixels = if has_alpha && hdr[12] == 16 {
        let a = alpha_image.unwrap().into_luma16().into_raw();
        let mut out = Vec::with_capacity(w as usize * h as usize * 4);
        for (i, (rgb, a)) in rgb.chunks_exact(3).zip(a).enumerate() {
            if i % 65536 == 0 {
                request.check_cancelled()?;
            }
            out.extend_from_slice(&[
                u16::from(rgb[0]) * 257,
                u16::from(rgb[1]) * 257,
                u16::from(rgb[2]) * 257,
                a,
            ]);
        }
        RasterPixels::Rgba16(Arc::new(out).into())
    } else {
        let a = alpha_image.map(|v| v.into_luma8().into_raw());
        let mut out = Vec::with_capacity(w as usize * h as usize * 4);
        for (i, rgb) in rgb.chunks_exact(3).enumerate() {
            if i % 65536 == 0 {
                request.check_cancelled()?;
            }
            out.extend_from_slice(rgb);
            out.push(a.as_ref().map_or(255, |v| v[i]));
        }
        RasterPixels::Rgba8(Arc::new(out).into())
    };
    Ok(DecodedRaster::new(w, h, pixels, color)?)
}
#[cfg(test)]
mod tests {
    use super::*;
    fn root() -> std::path::PathBuf {
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/raster")
    }
    #[test]
    fn independent_jpeg_and_alpha_samples() {
        let root = root();
        for line in std::fs::read_to_string(root.join("jng-manifest.tsv"))
            .unwrap()
            .lines()
            .filter(|s| !s.starts_with('#'))
        {
            let c: Vec<_> = line.split('\t').collect();
            let request = DecodeRequest::new(root.join(c[0]));
            let raster = crate::decode_raster(&request).unwrap();
            assert_eq!((raster.width(), raster.height()), (8, 3));
            assert_eq!(raster.color_space(), &RasterColorSpace::Srgb);
            let actual: Vec<u16> = match raster.pixels() {
                RasterPixels::Rgba8(v) => v.iter().map(|&v| u16::from(v) * 257).collect(),
                RasterPixels::Rgba16(v) => v.as_ref().to_vec(),
                _ => panic!(),
            };
            let expected = std::fs::read(root.join(c[5])).unwrap();
            for (i, (&a, b)) in actual.iter().zip(expected.chunks_exact(2)).enumerate() {
                let e = u16::from_le_bytes(b.try_into().unwrap());
                let tol = if i % 4 == 3 && c[4] == "False" { 0 } else { 514 };
                assert!(a.abs_diff(e) <= tol, "{} sample {i}: {a}/{e}", c[0]);
            }
            crate::prepare_raster_for_display(&raster).unwrap();
        }
    }
    #[test]
    fn outer_color_declarations_and_header_fields_are_checked() {
        let path = root().join("jng-RGB-16-0.jng");
        let request = DecodeRequest::new(&path);
        let original = std::fs::read(&path).unwrap();
        // Removing the outer sRGB declaration produces an explicit assumed import.
        let srgb = original.windows(4).position(|v| v == b"sRGB").unwrap() - 4;
        let mut plain = original.clone();
        plain.drain(srgb..srgb + 13);
        assert_eq!(
            decode(&plain, &request).unwrap().color_space(),
            &RasterColorSpace::AssumedSrgb
        );
        let profiled_png = std::fs::read(root().join("pattern.profiled.png")).unwrap();
        let mut at = 8;
        let iccp = loop {
            let n = u32::from_be_bytes(profiled_png[at..at + 4].try_into().unwrap()) as usize;
            let end = at + n + 12;
            if &profiled_png[at + 4..at + 8] == b"iCCP" {
                break &profiled_png[at..end];
            }
            at = end;
        };
        let mut profiled = original.clone();
        profiled.splice(srgb..srgb + 13, iccp.iter().copied());
        let raster = decode(&profiled, &request).unwrap();
        assert!(matches!(raster.color_space(), RasterColorSpace::Icc(_)));
        let prepared = crate::prepare_raster_for_display(&raster).unwrap();
        let RasterPixels::Rgba32Float(values) = prepared.pixels() else {
            panic!()
        };
        assert_eq!(values[3], 32768. / 65535.);
        // Valid-CRC header corruption must still reject before decoding pixels.
        for (offset, value) in [
            (8, 0u8),
            (9, 12),
            (10, 0),
            (11, 1),
            (12, 3),
            (13, 7),
            (14, 1),
            (15, 2),
        ] {
            let mut altered = original.clone();
            altered[16 + offset] = value;
            let mut checksum = flate2::Crc::new();
            checksum.update(&altered[12..32]);
            altered[32..36].copy_from_slice(&checksum.sum().to_be_bytes());
            assert!(decode(&altered, &request).is_err(), "header field {offset}");
        }
        let mut selected = request.clone();
        selected.image_index = 1;
        assert!(matches!(
            crate::decode_raster(&selected),
            Err(RasterDecodeError::Source(
                crate::DecodeError::UnsupportedImageIndex { .. }
            ))
        ));
        assert!(crate::is_supported_image_path(&path));
    }
    #[test]
    fn truncated_crc_header_and_index_reject() {
        let path = root().join("jng-RGB-16-0.jng");
        let request = DecodeRequest::new(&path);
        let b = std::fs::read(path).unwrap();
        for n in 0..b.len() {
            assert!(decode(&b[..n], &request).is_err(), "prefix {n}");
        }
        let mut corrupt = b.clone();
        corrupt[30] ^= 1;
        assert!(decode(&corrupt, &request).is_err());
        let mut trailing = b;
        trailing.push(0);
        assert!(decode(&trailing, &request).is_err());
    }
}
