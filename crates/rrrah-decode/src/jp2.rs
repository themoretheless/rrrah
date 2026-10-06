//! JPEG 2000 with bounded header preflight and native component precision.
use crate::{
    DecodeRequest,
    raster::{MAX_RASTER_BYTES, RasterDecodeError},
};
use rrrah_core::{DecodedRaster, RasterColorSpace, RasterPixels};
use std::sync::Arc;
const MAGIC: &[u8] = b"\0\0\0\x0cjP  \r\n\x87\n";
fn invalid(reason: &str) -> RasterDecodeError {
    RasterDecodeError::InvalidJp2(reason.into())
}
pub(crate) fn has_magic(bytes: &[u8]) -> bool {
    bytes.starts_with(MAGIC) || bytes.starts_with(&[0xff, 0x4f, 0xff, 0x51])
}
fn u16be(b: &[u8], at: usize) -> Result<u16, RasterDecodeError> {
    Ok(u16::from_be_bytes(
        b.get(at..at + 2)
            .ok_or_else(|| invalid("truncated header"))?
            .try_into()
            .unwrap(),
    ))
}
fn u32be(b: &[u8], at: usize) -> Result<u32, RasterDecodeError> {
    Ok(u32::from_be_bytes(
        b.get(at..at + 4)
            .ok_or_else(|| invalid("truncated header"))?
            .try_into()
            .unwrap(),
    ))
}
fn boxes<'a>(
    mut b: &'a [u8],
    mut visit: impl FnMut(&'a [u8], &'a [u8]) -> Result<(), RasterDecodeError>,
) -> Result<(), RasterDecodeError> {
    let mut count = 0;
    while !b.is_empty() {
        count += 1;
        if count > 65536 {
            return Err(invalid("too many boxes"));
        }
        let short = u32be(b, 0)?;
        let kind = b.get(4..8).ok_or_else(|| invalid("truncated box"))?;
        let (len, header) = match short {
            0 => (b.len(), 8),
            1 => {
                let x = u64::from_be_bytes(
                    b.get(8..16)
                        .ok_or_else(|| invalid("truncated extended box"))?
                        .try_into()
                        .unwrap(),
                );
                (usize::try_from(x).map_err(|_| invalid("box overflow"))?, 16)
            }
            n => (n as usize, 8),
        };
        if len < header || len > b.len() {
            return Err(invalid("invalid box size"));
        }
        visit(kind, &b[header..len])?;
        b = &b[len..];
    }
    Ok(())
}
struct Header<'a> {
    stream: &'a [u8],
    color: RasterColorSpace,
    width: u32,
    height: u32,
    channels: usize,
    wide: bool,
}
fn header(bytes: &[u8]) -> Result<Header<'_>, RasterDecodeError> {
    let mut stream = None;
    let mut color = RasterColorSpace::Unspecified;
    let mut color_seen = false;
    if bytes.starts_with(MAGIC) {
        boxes(bytes, |kind, payload| {
            if kind == b"jp2c" {
                if stream.replace(payload).is_some() {
                    return Err(invalid("multiple codestreams"));
                }
            }
            if kind == b"jp2h" {
                boxes(payload, |kind, payload| {
                    if kind == b"colr" {
                        if color_seen {
                            return Err(invalid("multiple color specifications unsupported"));
                        }
                        color_seen = true;
                        match payload.first() {
                            Some(1) => {
                                color = match u32be(payload, 3)? {
                                    16 => RasterColorSpace::Srgb,
                                    17 => RasterColorSpace::AssumedSrgb,
                                    _ => return Err(invalid("color space unsupported")),
                                };
                            }
                            Some(2) => {
                                let profile = payload.get(3..).ok_or_else(|| invalid("truncated ICC"))?;
                                if profile.len() > 4 * 1024 * 1024 {
                                    return Err(invalid("ICC too large"));
                                }
                                color = RasterColorSpace::Icc(profile.to_vec());
                            }
                            _ => return Err(invalid("color specification method unsupported")),
                        }
                    }
                    if kind == b"pclr" || kind == b"cmap" {
                        return Err(invalid("palette mapping unsupported"));
                    }
                    if kind == b"cdef" {
                        let count = usize::from(u16be(payload, 0)?);
                        if payload.len() != 2 + count * 6 {
                            return Err(invalid("invalid channel definitions"));
                        }
                        for i in 0..count {
                            if u16be(payload, 4 + i * 6)? > 1 {
                                return Err(invalid("associated alpha unsupported"));
                            }
                        }
                    }
                    Ok(())
                })?;
            }
            Ok(())
        })?;
    } else {
        stream = Some(bytes);
    }
    let stream = stream.ok_or_else(|| invalid("missing codestream"))?;
    if !stream.starts_with(&[0xff, 0x4f, 0xff, 0x51]) {
        return Err(invalid("missing SIZ marker"));
    }
    let size = usize::from(u16be(stream, 4)?);
    let channels = usize::from(u16be(stream, 40)?);
    if !(1..=4).contains(&channels) || size != 38 + 3 * channels {
        return Err(invalid("invalid component count or SIZ size"));
    }
    let width = u32be(stream, 8)?
        .checked_sub(u32be(stream, 16)?)
        .ok_or_else(|| invalid("invalid canvas origin"))?;
    let height = u32be(stream, 12)?
        .checked_sub(u32be(stream, 20)?)
        .ok_or_else(|| invalid("invalid canvas origin"))?;
    if width == 0 || height == 0 || width > 65536 || height > 65536 {
        return Err(invalid("invalid or excessive dimensions"));
    }
    let mut wide = false;
    for i in 0..channels {
        let spec = stream
            .get(42 + i * 3..45 + i * 3)
            .ok_or_else(|| invalid("truncated component header"))?;
        let precision = (spec[0] & 127) + 1;
        if precision > 16 || spec[0] & 128 != 0 || spec[1] != 1 || spec[2] != 1 {
            return Err(invalid(
                "signed, high precision or subsampled component unsupported",
            ));
        }
        wide |= precision > 8;
    }
    // Bound decoded i32 planes, RGBA16 intermediate and RGBA8 output.
    if u64::from(width) * u64::from(height) * ((channels as u64) * 4 + 12) > MAX_RASTER_BYTES {
        return Err(RasterDecodeError::OutputTooLarge);
    }
    let tile_width = u32be(stream, 24)?;
    let tile_height = u32be(stream, 28)?;
    if tile_width == 0 || tile_height == 0 || tile_width > 65536 || tile_height > 65536 {
        return Err(invalid("invalid or excessive tile dimensions"));
    }
    if u64::from(u32be(stream, 8)?.div_ceil(tile_width)) * u64::from(u32be(stream, 12)?.div_ceil(tile_height))
        > 65536
    {
        return Err(invalid("too many tiles"));
    }
    Ok(Header {
        stream,
        color,
        width,
        height,
        channels,
        wide,
    })
}
pub(crate) fn decode(bytes: &[u8], request: &DecodeRequest) -> Result<DecodedRaster, RasterDecodeError> {
    request.check_cancelled()?;
    let h = header(bytes)?;
    let _ = h.stream;
    let image = jpeg2k::Image::from_bytes_with(bytes, jpeg2k::DecodeParameters::new().strict(true))
        .map_err(|e| invalid(&e.to_string()))?;
    request.check_cancelled()?;
    let components = image.components();
    let pixels = (h.width as usize) * (h.height as usize);
    if components.len() != h.channels
        || components.iter().any(|c| {
            c.width() != h.width
                || c.height() != h.height
                || c.data().len() != pixels
                || c.is_signed()
                || c.precision() == 0
                || c.precision() > 16
        })
    {
        return Err(invalid("decoded component mismatch"));
    }
    if !matches!(
        image.color_space(),
        jpeg2k::ColorSpace::Unknown
            | jpeg2k::ColorSpace::Unspecified
            | jpeg2k::ColorSpace::SRGB
            | jpeg2k::ColorSpace::Gray
    ) {
        return Err(invalid("decoded color space unsupported"));
    }
    let gray = matches!(h.channels, 1 | 2);
    let alpha = if h.channels == 2 || h.channels == 4 {
        Some(h.channels - 1)
    } else {
        None
    };
    if let Some(a) = alpha {
        if !components[a].is_alpha() {
            return Err(invalid("extra channel lacks alpha declaration"));
        }
    }
    if components
        .iter()
        .enumerate()
        .any(|(i, c)| c.is_alpha() && Some(i) != alpha)
    {
        return Err(invalid("alpha channel order unsupported"));
    }
    let max = if h.wide { 65535u64 } else { 255 };
    let sample = |c: usize, p: usize| -> Result<u16, RasterDecodeError> {
        let component = &components[c];
        let value = component.data()[p];
        let original_max = (1u64 << component.precision()) - 1;
        if value < 0 || value as u64 > original_max {
            return Err(invalid("component sample outside precision"));
        }
        Ok(((value as u64 * max + original_max / 2) / original_max) as u16)
    };
    let mut output = Vec::with_capacity(pixels * 4);
    for p in 0..pixels {
        if p % 65536 == 0 {
            request.check_cancelled()?;
        }
        for c in 0..3 {
            output.push(sample(if gray { 0 } else { c }, p)?);
        }
        output.push(if let Some(a) = alpha {
            sample(a, p)?
        } else {
            max as u16
        });
    }
    let pixels = if h.wide {
        RasterPixels::Rgba16(Arc::new(output).into())
    } else {
        RasterPixels::Rgba8(Arc::new(output.into_iter().map(|x| x as u8).collect::<Vec<_>>()).into())
    };
    Ok(DecodedRaster::new(h.width, h.height, pixels, h.color)?)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn retains_adjacent_sixteen_bit_values() {
        let bytes = include_bytes!("../../../tests/fixtures/raster/precision16.jp2");
        let frame = decode(bytes, &DecodeRequest::new("precision16.jp2")).unwrap();
        let RasterPixels::Rgba16(p) = frame.pixels() else {
            panic!("precision lost")
        };
        assert_eq!(
            p.as_slice(),
            &[32768, 32768, 32768, 65535, 32769, 32769, 32769, 65535]
        );
    }
    #[test]
    fn preflight_rejects_excessive_canvas_before_codec() {
        let bytes = include_bytes!("../../../tests/fixtures/raster/pattern-rgb.j2k");
        let mut bytes = bytes.to_vec();
        bytes[8..12].copy_from_slice(&65537u32.to_be_bytes());
        assert!(header(&bytes).is_err());
        bytes[8..12].copy_from_slice(&65536u32.to_be_bytes());
        bytes[12..16].copy_from_slice(&65536u32.to_be_bytes());
        assert!(matches!(header(&bytes), Err(RasterDecodeError::OutputTooLarge)));
    }
    #[test]
    fn truncated_codestream_is_not_progressive_success() {
        let bytes = include_bytes!("../../../tests/fixtures/raster/pattern-rgb.j2k");
        assert!(decode(&bytes[..bytes.len() - 10], &DecodeRequest::new("truncated.j2k")).is_err());
    }
    #[test]
    fn malformed_box_lengths_return_errors() {
        for len in [1u32, 7, 65535, u32::MAX] {
            let mut bytes = MAGIC.to_vec();
            bytes.extend(len.to_be_bytes());
            bytes.extend(b"jp2c");
            assert!(header(&bytes).is_err());
        }
    }
    #[test]
    fn rgb_icc_reaches_display_without_modifying_alpha() {
        let bytes = include_bytes!("../../../tests/fixtures/raster/pattern-rgba-icc.jp2");
        let frame = decode(bytes, &DecodeRequest::new("image.jp2")).unwrap();
        assert!(matches!(frame.color_space(), RasterColorSpace::Icc(_)));
        let prepared = crate::prepare_raster_for_display(&frame).unwrap();
        let RasterPixels::Rgba32Float(p) = prepared.pixels() else {
            panic!()
        };
        assert!((p[7] - 128.0 / 255.0).abs() < 1e-7);
    }
    #[test]
    fn codestream_magic_overrides_camera_extension() {
        let path = std::env::temp_dir().join(format!("rrrah-jp2-magic-{}.cr3", std::process::id()));
        std::fs::write(
            &path,
            include_bytes!("../../../tests/fixtures/raster/pattern-rgb.j2k"),
        )
        .unwrap();
        let result = crate::decode_image_file(&path);
        std::fs::remove_file(path).unwrap();
        assert!(matches!(result, Ok(crate::DecodedImage::Raster(_))));
    }
}
