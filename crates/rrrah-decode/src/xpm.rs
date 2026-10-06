use crate::{
    DecodeRequest,
    raster::{MAX_RASTER_BYTES, RasterDecodeError},
};
use rrrah_core::{DecodedRaster, RasterColorSpace, RasterPixels};
use std::{collections::HashMap, sync::Arc};
fn invalid(s: &'static str) -> RasterDecodeError {
    RasterDecodeError::InvalidXpm(s)
}
fn color(s: &str) -> Result<[u16; 4], RasterDecodeError> {
    if s.eq_ignore_ascii_case("None") {
        return Ok([0; 4]);
    }
    if let Some(hex) = s.strip_prefix('#') {
        if !matches!(hex.len(), 3 | 6 | 9 | 12) {
            return Err(invalid("invalid hex color"));
        }
        let n = hex.len() / 3;
        let max = (1u32 << (n * 4)) - 1;
        let mut rgb = [0, 0, 0, 65535];
        for i in 0..3 {
            let value = u32::from_str_radix(&hex[i * n..(i + 1) * n], 16)
                .map_err(|_| invalid("invalid hex component"))?;
            rgb[i] = ((value * 65535 + max / 2) / max) as u16;
        }
        return Ok(rgb);
    }
    let rgb = match s.to_ascii_lowercase().as_str() {
        "black" => [0, 0, 0],
        "white" => [255, 255, 255],
        "red" => [255, 0, 0],
        "green" => [0, 255, 0],
        "blue" => [0, 0, 255],
        "yellow" => [255, 255, 0],
        "cyan" => [0, 255, 255],
        "magenta" => [255, 0, 255],
        _ => return Err(invalid("unsupported named color")),
    };
    Ok([rgb[0] * 257, rgb[1] * 257, rgb[2] * 257, 65535])
}
fn strings(text: &str) -> Result<Vec<String>, RasterDecodeError> {
    if let Some(rest) = text
        .strip_prefix("! XPM2\n")
        .or_else(|| text.strip_prefix("! XPM2\r\n"))
    {
        return Ok(rest.lines().map(str::to_owned).collect());
    }
    if !text.starts_with("/* XPM */") {
        return Err(invalid("missing XPM signature"));
    }
    let mut values = Vec::new();
    let b = text.as_bytes();
    let mut i = 0;
    while i < b.len() {
        if b[i..].starts_with(b"/*") {
            let end = text[i + 2..]
                .find("*/")
                .ok_or_else(|| invalid("unterminated comment"))?;
            i += end + 4;
            continue;
        }
        if b[i] != b'"' {
            i += 1;
            continue;
        }
        i += 1;
        let mut value = Vec::new();
        loop {
            let c = *b.get(i).ok_or_else(|| invalid("unterminated string"))?;
            i += 1;
            if c == b'"' {
                break;
            }
            if c == b'\\' {
                let c = *b.get(i).ok_or_else(|| invalid("unterminated escape"))?;
                i += 1;
                if !matches!(c, b'\\' | b'"') {
                    return Err(invalid("unsupported C escape"));
                }
                value.push(c);
            } else {
                value.push(c);
            }
        }
        values.push(String::from_utf8(value).map_err(|_| invalid("invalid string"))?);
    }
    Ok(values)
}
pub(crate) fn decode(bytes: &[u8], request: &DecodeRequest) -> Result<DecodedRaster, RasterDecodeError> {
    if bytes.len() > 64 * 1024 * 1024 {
        return Err(RasterDecodeError::OutputTooLarge);
    }
    let text = std::str::from_utf8(bytes).map_err(|_| invalid("non-UTF8 source"))?;
    if !text.is_ascii() {
        return Err(invalid("non-ASCII source"));
    }
    let lines = strings(text)?;
    let header = lines.first().ok_or_else(|| invalid("missing values"))?;
    let mut words = header.split_ascii_whitespace();
    let mut number = || {
        words
            .next()
            .and_then(|v| v.parse::<u32>().ok())
            .filter(|v| *v > 0)
            .ok_or_else(|| invalid("invalid values"))
    };
    let width = number()?;
    let height = number()?;
    let colors = number()? as usize;
    let cpp = number()? as usize;
    if u64::from(width) * u64::from(height) > MAX_RASTER_BYTES / 4 || colors > 65536 || cpp > 64 {
        return Err(RasterDecodeError::OutputTooLarge);
    }
    if lines.len() < 1 + colors + height as usize {
        return Err(invalid("truncated palette or rows"));
    }
    let mut wide = false;
    let mut palette = HashMap::with_capacity(colors);
    for line in &lines[1..1 + colors] {
        let key = line.get(..cpp).ok_or_else(|| invalid("short palette key"))?;
        let mut parts = line[cpp..].split_ascii_whitespace();
        let mut value = None;
        while let Some(part) = parts.next() {
            if part == "c" {
                value = parts.next();
                break;
            }
        }
        let value = value.ok_or_else(|| invalid("missing color visual"))?;
        wide |= value.starts_with('#') && value.len() > 7;
        let rgba = color(value)?;
        if palette.insert(key, rgba).is_some() {
            return Err(invalid("duplicate palette key"));
        }
    }
    if u64::from(width) * u64::from(height) > MAX_RASTER_BYTES / 8 {
        return Err(RasterDecodeError::OutputTooLarge);
    }
    let mut pixels = Vec::new();
    pixels
        .try_reserve_exact(width as usize * height as usize * 4)
        .map_err(|_| RasterDecodeError::OutputTooLarge)?;
    for row in &lines[1 + colors..1 + colors + height as usize] {
        request.check_cancelled()?;
        if row.len() != width as usize * cpp {
            return Err(invalid("row length mismatch"));
        }
        for key in row.as_bytes().chunks_exact(cpp) {
            let key = std::str::from_utf8(key).unwrap();
            pixels.extend(*palette.get(key).ok_or_else(|| invalid("unknown pixel key"))?);
        }
    }
    Ok(DecodedRaster::new(
        width,
        height,
        if wide {
            RasterPixels::Rgba16(Arc::new(pixels).into())
        } else {
            RasterPixels::Rgba8(
                Arc::new(pixels.into_iter().map(|v| (v / 257) as u8).collect::<Vec<_>>()).into(),
            )
        },
        RasterColorSpace::AssumedSrgb,
    )?)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn xpm2_multichar_transparency() {
        let f = decode(
            b"! XPM2\n2 1 2 2\naa c #123\n.. c None\naa..\n",
            &DecodeRequest::new("unused"),
        )
        .unwrap();
        let RasterPixels::Rgba8(p) = f.pixels() else {
            panic!()
        };
        assert_eq!(&p[..], [17, 34, 51, 255, 0, 0, 0, 0]);
    }
    #[test]
    fn malformed_palette_rows_and_colors() {
        for b in [
            "! XPM2\n1 1 1 1\na c #xyz\na\n",
            "! XPM2\n1 1 1 1\na c red\nb\n",
            "! XPM2\n1 1 2 1\na c red\na c blue\na\n",
        ] {
            assert!(decode(b.as_bytes(), &DecodeRequest::new("unused")).is_err());
        }
    }
    #[test]
    fn sixteen_bit_palette_preserves_adjacent_samples_and_transparency() {
        let f = decode(
            b"! XPM2\n3 1 3 1\na c #800080008000\nb c #800180008000\n. c None\nab.\n",
            &DecodeRequest::new("unused"),
        )
        .unwrap();
        let RasterPixels::Rgba16(p) = f.pixels() else {
            panic!()
        };
        assert_eq!(
            &p[..],
            [32768, 32768, 32768, 65535, 32769, 32768, 32768, 65535, 0, 0, 0, 0]
        );
        let prepared = crate::prepare_raster_for_display(&f).unwrap();
        let RasterPixels::Rgba32Float(linear) = prepared.pixels() else {
            panic!()
        };
        assert!(linear[4] > linear[0]);
        assert_eq!(linear[11], 0.0);
    }
    #[test]
    fn twelve_bit_palette_scales_to_full_sixteen_bit_range() {
        assert_eq!(color("#fff800001").unwrap(), [65535, 32776, 16, 65535]);
    }
}
