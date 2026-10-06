use crate::{
    DecodeRequest,
    raster::{MAX_RASTER_BYTES, RasterDecodeError},
};
use rrrah_core::{DecodedRaster, RasterColorSpace, RasterPixels};
use std::sync::Arc;
fn invalid(s: &'static str) -> RasterDecodeError {
    RasterDecodeError::InvalidXbm(s)
}
fn integer(s: &str) -> Option<u32> {
    let s = s.trim().trim_end_matches(['u', 'U', 'l', 'L']);
    if let Some(s) = s.strip_prefix("0x").or_else(|| s.strip_prefix("0X")) {
        u32::from_str_radix(s, 16).ok()
    } else if s.starts_with('0') && s.len() > 1 {
        u32::from_str_radix(s, 8).ok()
    } else {
        s.parse().ok()
    }
}
pub(crate) fn decode(bytes: &[u8], request: &DecodeRequest) -> Result<DecodedRaster, RasterDecodeError> {
    if bytes.len() > 64 * 1024 * 1024 {
        return Err(RasterDecodeError::OutputTooLarge);
    }
    let source = std::str::from_utf8(bytes).map_err(|_| invalid("non-UTF8 source"))?;
    let mut text = String::with_capacity(source.len());
    let mut rest = source;
    while let Some(at) = rest.find("/*") {
        text.push_str(&rest[..at]);
        text.push(' ');
        rest = &rest[at + 2..];
        let end = rest.find("*/").ok_or_else(|| invalid("unterminated comment"))?;
        rest = &rest[end + 2..];
    }
    text.push_str(rest);
    let mut width = None;
    let mut height = None;
    let mut prefix = None;
    for line in text.lines() {
        let mut words = line.split_ascii_whitespace();
        if words.next() != Some("#define") {
            continue;
        }
        let name = words.next().ok_or_else(|| invalid("missing define name"))?;
        let target = if let Some(p) = name.strip_suffix("_width") {
            prefix = Some(p);
            &mut width
        } else if name.ends_with("_height") {
            &mut height
        } else {
            continue;
        };
        let value = integer(words.next().ok_or_else(|| invalid("missing dimension"))?)
            .filter(|v| *v > 0)
            .ok_or_else(|| invalid("invalid dimension"))?;
        if target.replace(value).is_some() {
            return Err(invalid("duplicate dimension"));
        }
    }
    let width = width.ok_or_else(|| invalid("missing width"))?;
    let height = height.ok_or_else(|| invalid("missing height"))?;
    if u64::from(width) * u64::from(height) > MAX_RASTER_BYTES / 4 {
        return Err(RasterDecodeError::OutputTooLarge);
    }
    let start = text.find('{').ok_or_else(|| invalid("missing initializer"))?;
    let end = text[start + 1..]
        .find('}')
        .ok_or_else(|| invalid("unterminated initializer"))?
        + start
        + 1;
    let header = &text[..start];
    if !header.contains(&format!("{}_bits", prefix.unwrap())) {
        return Err(invalid("mismatched bitmap name"));
    }
    let short = header.split_ascii_whitespace().any(|v| v == "short");
    let unit = if short { 16 } else { 8 };
    let stride = width.div_ceil(unit) as usize;
    let mut values = Vec::new();
    for token in text[start + 1..end].split(',') {
        if token.trim().is_empty() {
            continue;
        }
        let value = integer(token)
            .filter(|v| *v < if short { 65536 } else { 256 })
            .ok_or_else(|| invalid("invalid initializer value"))?;
        if values.len() >= stride * height as usize {
            return Err(invalid("excess bitmap data"));
        }
        values.push(value);
    }
    if values.len() != stride * height as usize {
        return Err(invalid("truncated bitmap data"));
    }
    let mut pixels = Vec::new();
    pixels
        .try_reserve_exact(width as usize * height as usize * 4)
        .map_err(|_| RasterDecodeError::OutputTooLarge)?;
    for row in values.chunks_exact(stride) {
        request.check_cancelled()?;
        for x in 0..width as usize {
            let v = if row[x / unit as usize] & (1 << (x % unit as usize)) != 0 {
                0
            } else {
                255
            };
            pixels.extend([v, v, v, 255]);
        }
    }
    Ok(DecodedRaster::new(
        width,
        height,
        RasterPixels::Rgba8(Arc::new(pixels).into()),
        RasterColorSpace::Srgb,
    )?)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn x10_short_padding_and_lsb_order() {
        let f=decode(b"#define test_width 17\n#define test_height 2\nstatic short test_bits[]={0x0001,0x0001,0x0002,0};",&DecodeRequest::new("unused")).unwrap();
        let RasterPixels::Rgba8(p) = f.pixels() else {
            panic!()
        };
        assert_eq!(p[0], 0);
        assert_eq!(p[4], 255);
        assert_eq!(p[16 * 4], 0);
        assert_eq!(p[17 * 4], 255);
        assert_eq!(p[18 * 4], 0);
    }
    #[test]
    fn malformed_and_oversized_source() {
        for source in [
            "#define x_width 1\n#define x_height 1\nstatic char x_bits[]={256};",
            "#define x_width 1\n#define x_height 1\nstatic char x_bits[]={};",
            "/*unfinished",
            "#define x_width 4294967295\n#define x_height 4294967295\n",
        ] {
            assert!(decode(source.as_bytes(), &DecodeRequest::new("unused")).is_err());
        }
    }
}
