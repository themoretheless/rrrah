//! Wavefront RLA indexed scanlines and independently encoded byte planes.
use crate::{
    DecodeRequest,
    raster::{MAX_RASTER_BYTES, RasterDecodeError},
};
use rrrah_core::{DecodedRaster, RasterColorSpace, RasterPixels};
fn bad(s: &'static str) -> RasterDecodeError {
    RasterDecodeError::InvalidRla(s)
}
fn plane(encoded: &[u8], width: usize) -> Result<(Vec<u8>, usize), RasterDecodeError> {
    let mut out = Vec::with_capacity(width);
    let mut at = 0;
    while out.len() < width {
        let control = *encoded.get(at).ok_or_else(|| bad("truncated byte plane"))? as i8;
        at += 1;
        let n = if control >= 0 {
            control as usize + 1
        } else {
            usize::from(control.unsigned_abs())
        };
        if n > width - out.len() {
            return Err(bad("cross-row byte run"));
        }
        if control >= 0 {
            let value = *encoded.get(at).ok_or_else(|| bad("truncated repeat"))?;
            at += 1;
            out.resize(out.len() + n, value);
        } else {
            let end = at + n;
            out.extend_from_slice(encoded.get(at..end).ok_or_else(|| bad("truncated literal"))?);
            at = end;
        }
    }
    Ok((out, at))
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RlaAlphaMode {
    Straight,
    Premultiplied,
}
/// Explicit producer color interpretation; float storage alone does not declare linear light.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RlaColorSpace {
    Srgb,
    LinearSrgb,
}
/// Float RLA records have producer-dependent byte order; never infer it from values.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RlaFloatByteOrder {
    Little,
    Big,
}
/// Decode float32 gray/RGB RLA with explicit producer byte order, alpha and color.
pub fn decode_rla_float_with_interpretation(
    request: &DecodeRequest,
    byte_order: RlaFloatByteOrder,
    alpha: RlaAlphaMode,
    color: RasterColorSpace,
) -> Result<DecodedRaster, RasterDecodeError> {
    request.check_cancelled()?;
    if request.image_index != 0 {
        return Err(crate::DecodeError::UnsupportedImageIndex {
            index: request.image_index,
        }
        .into());
    }
    decode_with_settings(
        &crate::bounded_io::read_bounded(request)?,
        request,
        Some(alpha),
        color,
        Some(byte_order),
    )
}
/// RLA does not reliably identify stored alpha association or color space.
/// Supply their interpretation from the producing pipeline.
pub fn decode_rla_with_interpretation(
    request: &DecodeRequest,
    alpha: RlaAlphaMode,
    color: RasterColorSpace,
) -> Result<DecodedRaster, RasterDecodeError> {
    request.check_cancelled()?;
    if request.image_index != 0 {
        return Err(crate::DecodeError::UnsupportedImageIndex {
            index: request.image_index,
        }
        .into());
    }
    decode_with_mode(
        &crate::bounded_io::read_bounded(request)?,
        request,
        Some(alpha),
        color,
    )
}
pub(crate) fn decode(b: &[u8], request: &DecodeRequest) -> Result<DecodedRaster, RasterDecodeError> {
    let color = if let Some(color) = request.rla_color_space {
        match color {
            RlaColorSpace::Srgb => RasterColorSpace::Srgb,
            RlaColorSpace::LinearSrgb => RasterColorSpace::LinearSrgb,
        }
    } else if request.assume_untagged_srgb {
        RasterColorSpace::AssumedSrgb
    } else {
        RasterColorSpace::Unspecified
    };
    decode_with_settings(
        b,
        request,
        request.rla_alpha_mode,
        color,
        request.rla_float_byte_order,
    )
}
fn decode_with_mode(
    b: &[u8],
    request: &DecodeRequest,
    mode: Option<RlaAlphaMode>,
    color_space: RasterColorSpace,
) -> Result<DecodedRaster, RasterDecodeError> {
    decode_with_settings(b, request, mode, color_space, None)
}
fn decode_with_settings(
    b: &[u8],
    request: &DecodeRequest,
    mode: Option<RlaAlphaMode>,
    color_space: RasterColorSpace,
    float_order: Option<RlaFloatByteOrder>,
) -> Result<DecodedRaster, RasterDecodeError> {
    request.check_cancelled()?;
    if b.len() < 740 {
        return Err(bad("truncated header"));
    }
    let u16at = |at| u16::from_be_bytes([b[at], b[at + 1]]);
    let i16at = |at| i16::from_be_bytes([b[at], b[at + 1]]);
    let u32at = |at| u32::from_be_bytes(b[at..at + 4].try_into().unwrap());
    if !matches!(u16at(26), 0 | 65534) || u32at(736) != 0 || u16at(612) != 0 || u16at(24) != 0 {
        return Err(bad(
            "unsupported revision, subimages, fields or auxiliary channels",
        ));
    }
    let (left, right, bottom, top) = (
        i32::from(i16at(0)),
        i32::from(i16at(2)),
        i32::from(i16at(4)),
        i32::from(i16at(6)),
    );
    let (active_left, active_right, active_bottom, active_top) = (
        i32::from(i16at(8)),
        i32::from(i16at(10)),
        i32::from(i16at(12)),
        i32::from(i16at(14)),
    );
    if left > right
        || bottom > top
        || active_left > active_right
        || active_bottom > active_top
        || active_left < left
        || active_right > right
        || active_bottom < bottom
        || active_top > top
    {
        return Err(bad("invalid full/active window bounds"));
    }
    let (w, h) = ((right - left + 1) as usize, (top - bottom + 1) as usize);
    let (aw, ah) = (
        (active_right - active_left + 1) as usize,
        (active_top - active_bottom + 1) as usize,
    );
    let x_offset = (active_left - left) as usize;
    let color = u16at(20) as usize;
    let matte = u16at(22) as usize;
    if !matches!(color, 1 | 3) || matte > 1 {
        return Err(bad("unsupported channel layout"));
    }
    if matte > 0 && mode.is_none() {
        return Err(RasterDecodeError::RlaInterpretationRequired);
    }
    let bits = if u16at(658) == 0 { 8 } else { u16at(658) };
    let alpha_bits = u16at(662);
    if u16at(18) == 4 || (matte > 0 && u16at(660) == 4) {
        let order = float_order.ok_or_else(|| bad("float byte order requires explicit interpretation"))?;
        if u16at(18) != 4 || bits != 32 || (matte > 0 && (u16at(660) != 4 || alpha_bits != 32)) {
            return Err(bad("mixed float/integer channel groups are unsupported"));
        }
        return decode_float_records(
            b,
            request,
            color_space,
            FloatSpec {
                width: w,
                height: h,
                active_width: aw,
                active_height: ah,
                x_offset,
                first_row: (top - active_bottom) as usize,
                color,
                matte,
                order,
                mode,
            },
        );
    }
    if !matches!(bits, 8 | 16)
        || u16at(18) > 1
        || (u16at(18) == 1 && bits != 16)
        || (matte > 0
            && (!matches!(alpha_bits, 8 | 16) || u16at(660) > 1 || (u16at(660) == 1 && alpha_bits != 16)))
    {
        return Err(bad("unsupported channel type or precision"));
    }
    if w as u64
        * h as u64
        * if mode == Some(RlaAlphaMode::Premultiplied) && matte > 0 {
            16
        } else {
            8
        }
        > MAX_RASTER_BYTES
    {
        return Err(RasterDecodeError::OutputTooLarge);
    }
    let directory = 740 + ah * 4;
    if b.len() < directory {
        return Err(bad("truncated offset table"));
    }
    let budget = request
        .memory_budget
        .clone()
        .unwrap_or_else(|| rrrah_core::MemoryBudget::new(MAX_RASTER_BYTES * 2));
    let mut out = budget
        .try_buffer(w * h * 4, 0u16)
        .map_err(crate::DecodeError::Memory)?;
    let mut ranges = Vec::with_capacity(ah);
    for row in 0..ah {
        let output_row = (top - active_bottom - row as i32) as usize;
        for pixel in
            out[(output_row * w + x_offset) * 4..(output_row * w + x_offset + aw) * 4].chunks_exact_mut(4)
        {
            pixel[3] = 65535;
        }
        request.check_cancelled()?;
        let start = u32at(740 + row * 4) as usize;
        if start < directory || start >= b.len() {
            return Err(bad("invalid row offset"));
        }
        let mut at = start;
        for channel in 0..color + matte {
            let precision = if channel < color { bits } else { alpha_bits };
            let header = b.get(at..at + 2).ok_or_else(|| bad("truncated record length"))?;
            let length = u16::from_be_bytes([header[0], header[1]]) as usize;
            at += 2;
            let end = at
                .checked_add(length)
                .ok_or_else(|| bad("record offset overflow"))?;
            let record = b.get(at..end).ok_or_else(|| bad("truncated record"))?;
            let _plane_credit = budget
                .try_reserve(aw as u64 * if precision == 16 { 2 } else { 1 })
                .map_err(crate::DecodeError::Memory)?;
            let (high, used) = plane(record, aw)?;
            let (low, total) = if precision == 16 {
                let (v, n) = plane(&record[used..], aw)?;
                (Some(v), used + n)
            } else {
                (None, used)
            };
            if total != length {
                return Err(bad("trailing channel bytes"));
            }
            for x in 0..aw {
                let value = low
                    .as_ref()
                    .map_or(u16::from(high[x]) * 257, |l| u16::from_be_bytes([high[x], l[x]]));
                let pixel = (output_row * w + x_offset + x) * 4;
                if channel >= color {
                    out[pixel + 3] = value;
                } else if color == 1 {
                    out[pixel..pixel + 3].fill(value);
                } else {
                    out[pixel + channel] = value;
                }
            }
            at = end;
        }
        ranges.push((start, at));
    }
    ranges.sort_unstable();
    let mut end = directory;
    for (start, next) in ranges {
        if start != end {
            return Err(bad("overlapping rows or unqualified gap"));
        }
        end = next;
    }
    if end != b.len() {
        return Err(bad("trailing image bytes"));
    }
    if matte > 0 && mode == Some(RlaAlphaMode::Premultiplied) {
        let mut float = budget
            .try_buffer(out.len(), 0.0f32)
            .map_err(crate::DecodeError::Memory)?;
        for (pixel, target) in out.chunks_exact(4).zip(float.chunks_exact_mut(4)) {
            request.check_cancelled()?;
            let alpha = f32::from(pixel[3]) / 65535.0;
            if alpha == 0.0 && pixel[..3].iter().any(|v| *v != 0) {
                return Err(bad("zero-alpha emission requires associated-alpha rendering"));
            }
            for (value, destination) in pixel[..3].iter().zip(&mut target[..3]) {
                *destination = if alpha > 0.0 {
                    (f32::from(*value) / 65535.0) / alpha
                } else {
                    0.0
                };
            }
            target[3] = alpha;
        }
        return Ok(DecodedRaster::new(
            w as u32,
            h as u32,
            RasterPixels::Rgba32Float(float.freeze().into()),
            color_space,
        )?);
    }
    let pixels = if bits == 8 && (matte == 0 || alpha_bits == 8) {
        let mut bytes = budget
            .try_buffer(out.len(), 0u8)
            .map_err(crate::DecodeError::Memory)?;
        for (destination, value) in bytes.iter_mut().zip(out.iter()) {
            *destination = (value / 257) as u8;
        }
        RasterPixels::Rgba8(bytes.freeze().into())
    } else {
        RasterPixels::Rgba16(out.freeze().into())
    };
    Ok(DecodedRaster::new(w as u32, h as u32, pixels, color_space)?)
}

struct FloatSpec {
    width: usize,
    height: usize,
    active_width: usize,
    active_height: usize,
    x_offset: usize,
    first_row: usize,
    color: usize,
    matte: usize,
    order: RlaFloatByteOrder,
    mode: Option<RlaAlphaMode>,
}
fn decode_float_records(
    b: &[u8],
    request: &DecodeRequest,
    color_space: RasterColorSpace,
    spec: FloatSpec,
) -> Result<DecodedRaster, RasterDecodeError> {
    let FloatSpec {
        width,
        height,
        active_width,
        active_height,
        x_offset,
        first_row,
        color,
        matte,
        order,
        mode,
    } = spec;
    if width as u64 * height as u64 * 16 > MAX_RASTER_BYTES {
        return Err(RasterDecodeError::OutputTooLarge);
    }
    let directory = 740 + active_height * 4;
    if b.len() < directory {
        return Err(bad("truncated offset table"));
    }
    let budget = request
        .memory_budget
        .clone()
        .unwrap_or_else(|| rrrah_core::MemoryBudget::new(MAX_RASTER_BYTES * 2));
    let mut output = budget
        .try_buffer(width * height * 4, 0.0f32)
        .map_err(crate::DecodeError::Memory)?;
    let mut ranges = Vec::with_capacity(active_height);
    for row in 0..active_height {
        request.check_cancelled()?;
        let output_row = first_row - row;
        let start = u32::from_be_bytes(b[740 + row * 4..744 + row * 4].try_into().unwrap()) as usize;
        if start < directory || start >= b.len() {
            return Err(bad("invalid row offset"));
        }
        let mut at = start;
        for pixel in output
            [(output_row * width + x_offset) * 4..(output_row * width + x_offset + active_width) * 4]
            .chunks_exact_mut(4)
        {
            pixel[3] = 1.0;
        }
        for channel in 0..color + matte {
            let header = b.get(at..at + 2).ok_or_else(|| bad("truncated record length"))?;
            let length = u16::from_be_bytes(header.try_into().unwrap()) as usize;
            at += 2;
            if length != active_width * 4 {
                return Err(bad("invalid float record length"));
            }
            let end = at
                .checked_add(length)
                .ok_or_else(|| bad("record offset overflow"))?;
            let record = b.get(at..end).ok_or_else(|| bad("truncated float record"))?;
            for (x, bytes) in record.chunks_exact(4).enumerate() {
                if x % 4096 == 0 {
                    request.check_cancelled()?;
                }
                let value = match order {
                    RlaFloatByteOrder::Little => f32::from_le_bytes(bytes.try_into().unwrap()),
                    RlaFloatByteOrder::Big => f32::from_be_bytes(bytes.try_into().unwrap()),
                };
                if !value.is_finite() {
                    return Err(bad("nonfinite float sample"));
                }
                let pixel = (output_row * width + x_offset + x) * 4;
                if channel >= color {
                    if !(0.0..=1.0).contains(&value) {
                        return Err(bad("float alpha outside unit range"));
                    }
                    output[pixel + 3] = value;
                } else if color == 1 {
                    output[pixel..pixel + 3].fill(value);
                } else {
                    output[pixel + channel] = value;
                }
            }
            at = end;
        }
        ranges.push((start, at));
    }
    ranges.sort_unstable();
    let mut end = directory;
    for (start, next) in ranges {
        if start != end {
            return Err(bad("overlapping rows or unqualified gap"));
        }
        end = next;
    }
    if end != b.len() {
        return Err(bad("trailing image bytes"));
    }
    if matte > 0 && mode == Some(RlaAlphaMode::Premultiplied) {
        for pixel in output.chunks_exact_mut(4) {
            request.check_cancelled()?;
            let alpha = pixel[3];
            if alpha == 0.0 && pixel[..3].iter().any(|value| *value != 0.0) {
                return Err(bad("zero-alpha emission requires associated-alpha rendering"));
            }
            for channel in &mut pixel[..3] {
                *channel = if alpha > 0.0 { *channel / alpha } else { 0.0 };
                if !channel.is_finite() {
                    return Err(bad("float unassociation overflow"));
                }
            }
        }
    }
    Ok(DecodedRaster::new(
        width as u32,
        height as u32,
        RasterPixels::Rgba32Float(output.freeze().into()),
        color_space,
    )?)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn explicit_float_byte_order_preserves_hdr_and_manages_public_output() {
        let path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../tests/fixtures/raster/rla-float32-le-hdr-oiio.rla");
        let source = std::fs::read(&path).unwrap();
        let budget = rrrah_core::MemoryBudget::new(4096);
        let mut request = DecodeRequest::new(path);
        request.memory_budget = Some(budget.clone());
        assert!(decode(&source, &request).is_err());
        let decoded = decode_rla_float_with_interpretation(
            &request,
            RlaFloatByteOrder::Little,
            RlaAlphaMode::Straight,
            RasterColorSpace::LinearSrgb,
        )
        .unwrap();
        let RasterPixels::Rgba32Float(samples) = decoded.pixels() else {
            panic!()
        };
        assert_eq!(samples.as_ref(), &[4.0, 2.0, -0.5, 1.0, 4.0, 2.0, -0.5, 1.0]);
        assert_eq!(budget.used(), decoded.pixel_capacity_bytes());
        drop(decoded);
        assert_eq!(budget.used(), 0);
        let mut swapped = source.clone();
        let start = u32::from_be_bytes(source[740..744].try_into().unwrap()) as usize;
        let mut at = start;
        for _ in 0..4 {
            let length = u16::from_be_bytes(source[at..at + 2].try_into().unwrap()) as usize;
            at += 2;
            for bytes in swapped[at..at + length].chunks_exact_mut(4) {
                bytes.reverse();
            }
            at += length;
        }
        let decoded = decode_with_settings(
            &swapped,
            &request,
            Some(RlaAlphaMode::Straight),
            RasterColorSpace::LinearSrgb,
            Some(RlaFloatByteOrder::Big),
        )
        .unwrap();
        let RasterPixels::Rgba32Float(samples) = decoded.pixels() else {
            panic!()
        };
        assert_eq!(samples.as_ref(), &[4.0, 2.0, -0.5, 1.0, 4.0, 2.0, -0.5, 1.0]);
        drop(decoded);
        assert_eq!(budget.used(), 0);
        // Four channel records, each with two raw f32 values and a two-byte length.
        let alpha_at = start + 3 * 10 + 2;
        let mut associated = source.clone();
        for bytes in associated[alpha_at..alpha_at + 8].chunks_exact_mut(4) {
            bytes.copy_from_slice(&0.5f32.to_le_bytes());
        }
        let decoded = decode_with_settings(
            &associated,
            &request,
            Some(RlaAlphaMode::Premultiplied),
            RasterColorSpace::LinearSrgb,
            Some(RlaFloatByteOrder::Little),
        )
        .unwrap();
        let RasterPixels::Rgba32Float(samples) = decoded.pixels() else {
            panic!()
        };
        assert_eq!(samples.as_ref(), &[8.0, 4.0, -1.0, 0.5, 8.0, 4.0, -1.0, 0.5]);
        drop(decoded);
        for (offset, value) in [
            (start + 2, f32::NAN),
            (alpha_at, 1.5),
            (alpha_at, -0.1),
            (alpha_at, 0.0),
        ] {
            let mut invalid = source.clone();
            invalid[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
            assert!(
                decode_with_settings(
                    &invalid,
                    &request,
                    Some(RlaAlphaMode::Premultiplied),
                    RasterColorSpace::LinearSrgb,
                    Some(RlaFloatByteOrder::Little)
                )
                .is_err()
            );
            assert_eq!(budget.used(), 0);
        }
        let short = rrrah_core::MemoryBudget::new(31);
        request.memory_budget = Some(short.clone());
        assert!(matches!(
            decode_with_settings(
                &source,
                &request,
                Some(RlaAlphaMode::Straight),
                RasterColorSpace::LinearSrgb,
                Some(RlaFloatByteOrder::Little)
            ),
            Err(RasterDecodeError::Source(crate::DecodeError::Memory(_)))
        ));
        assert_eq!(short.used(), 0);
    }
    #[test]
    fn offset_active_window_places_rows_in_transparent_full_canvas() {
        let original = include_bytes!("../../../tests/fixtures/raster/rla-8-c3-a0-mixed0.rla");
        let request = DecodeRequest::new("borrowed.rla");
        let original = decode(original, &request).unwrap();
        let RasterPixels::Rgba8(expected) = original.pixels() else {
            panic!()
        };
        let mut source = include_bytes!("../../../tests/fixtures/raster/rla-8-c3-a0-mixed0.rla").to_vec();
        // Active data covers x=0..139, y=0..2; full canvas starts at (-2,-1).
        for (offset, value) in [(0, -2i16), (2, 142), (4, -1), (6, 4)] {
            source[offset..offset + 2].copy_from_slice(&value.to_be_bytes());
        }
        let budget = rrrah_core::MemoryBudget::new(65536);
        let mut request = request;
        request.memory_budget = Some(budget.clone());
        let raster = decode(&source, &request).unwrap();
        assert_eq!((raster.width(), raster.height()), (145, 6));
        let RasterPixels::Rgba8(actual) = raster.pixels() else {
            panic!()
        };
        for y in 0..6usize {
            for x in 0..145usize {
                let pixel = &actual[(y * 145 + x) * 4..(y * 145 + x + 1) * 4];
                if (2..142).contains(&x) && (2..5).contains(&y) {
                    let offset = ((y - 2) * 140 + x - 2) * 4;
                    assert_eq!(pixel, &expected[offset..offset + 4]);
                } else {
                    assert_eq!(pixel, &[0, 0, 0, 0]);
                }
            }
        }
        let expected_canvas = actual.to_vec();
        drop(raster);
        assert_eq!(budget.used(), 0);
        // Absolute coordinates must not affect the normalized full-canvas pixels.
        // Exercise both signed extremes without overflowing the header coordinates.
        for (dx, dy) in [(-32766i32, -32767i32), (32625, 32763)] {
            let mut translated = source.clone();
            for offset in (0..16).step_by(2) {
                let coordinate = i16::from_be_bytes([source[offset], source[offset + 1]]);
                let delta = if offset % 8 < 4 { dx } else { dy };
                let shifted = i16::try_from(i32::from(coordinate) + delta).unwrap();
                translated[offset..offset + 2].copy_from_slice(&shifted.to_be_bytes());
            }
            let translated = decode(&translated, &request).unwrap();
            assert_eq!((translated.width(), translated.height()), (145, 6));
            let RasterPixels::Rgba8(pixels) = translated.pixels() else {
                panic!()
            };
            assert_eq!(pixels.as_ref(), expected_canvas.as_slice());
            drop(translated);
            assert_eq!(budget.used(), 0);
        }
        let short = rrrah_core::MemoryBudget::new(145 * 6 * 8 - 1);
        request.memory_budget = Some(short.clone());
        assert!(matches!(
            decode(&source, &request),
            Err(RasterDecodeError::Source(crate::DecodeError::Memory(_)))
        ));
        assert_eq!(short.used(), 0);
        request.memory_budget = Some(budget.clone());
        source[8..10].copy_from_slice(&(-3i16).to_be_bytes());
        assert!(decode(&source, &request).is_err());
        assert_eq!(budget.used(), 0);
    }
    #[test]
    fn explicit_rla_api_manages_pixels_and_refuses_short_output_budget() {
        for name in ["rla-8-c3-a8-mixed1.rla", "rla-16-c3-a8-mixed1.rla"] {
            let path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("../../tests/fixtures/raster")
                .join(name);
            let source = std::fs::read(&path).unwrap();
            let width = u16::from_be_bytes([source[2], source[3]]) as u64 + 1;
            let height = u16::from_be_bytes([source[6], source[7]]) as u64 + 1;
            let pixel_bytes = width * height * 8;
            let budget = rrrah_core::MemoryBudget::new(source.len() as u64 + pixel_bytes * 3);
            let mut request = DecodeRequest::new(&path);
            request.memory_budget = Some(budget.clone());
            let raster =
                decode_rla_with_interpretation(&request, RlaAlphaMode::Straight, RasterColorSpace::Srgb)
                    .unwrap();
            assert_eq!(budget.used(), raster.pixel_capacity_bytes());
            let alias = raster.clone();
            let weight = budget.used();
            drop(raster);
            assert_eq!(budget.used(), weight);
            drop(alias);
            assert_eq!(budget.used(), 0);
            let short = rrrah_core::MemoryBudget::new(source.len() as u64 + pixel_bytes - 1);
            request.memory_budget = Some(short.clone());
            assert!(matches!(
                decode_rla_with_interpretation(&request, RlaAlphaMode::Straight, RasterColorSpace::Srgb,),
                Err(RasterDecodeError::Source(crate::DecodeError::Memory(_)))
            ));
            assert_eq!(short.used(), 0);
        }
    }
    #[test]
    fn common_raster_route_requires_explicit_alpha_then_matches_direct_api() {
        let path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../tests/fixtures/raster/rla-8-c3-a8-mixed1.rla");
        let budget = rrrah_core::MemoryBudget::new(1024 * 1024);
        let mut request = DecodeRequest::new(path);
        request.memory_budget = Some(budget.clone());
        assert!(matches!(
            crate::decode_raster(&request),
            Err(RasterDecodeError::RlaInterpretationRequired)
        ));
        assert_eq!(budget.used(), 0);
        request.rla_alpha_mode = Some(RlaAlphaMode::Straight);
        let common = crate::decode_raster(&request).unwrap();
        assert_eq!(common.color_space(), &RasterColorSpace::Unspecified);
        let direct =
            decode_rla_with_interpretation(&request, RlaAlphaMode::Straight, RasterColorSpace::Unspecified)
                .unwrap();
        let (RasterPixels::Rgba8(a), RasterPixels::Rgba8(b)) = (common.pixels(), direct.pixels()) else {
            panic!()
        };
        assert_eq!(a.as_slice(), b.as_slice());
        drop((common, direct));
        assert_eq!(budget.used(), 0);
        request.assume_untagged_srgb = true;
        let raster = crate::decode_raster(&request).unwrap();
        assert_eq!(raster.color_space(), &RasterColorSpace::AssumedSrgb);
        let prepared = crate::prepare_raster_for_display_with_budget(&raster, Some(&budget)).unwrap();
        drop((raster, prepared));
        assert_eq!(budget.used(), 0);
    }
    #[test]
    fn premultiplied_zero_alpha_emission_is_explicitly_rejected() {
        let original = include_bytes!("../../../tests/fixtures/raster/rla-8-c3-a8-mixed1.rla");
        let mut b = original.to_vec();
        let mut at = u32::from_be_bytes(b[740..744].try_into().unwrap()) as usize;
        for _ in 0..3 {
            let length = u16::from_be_bytes(b[at..at + 2].try_into().unwrap()) as usize;
            at += 2 + length;
        }
        let len = u16::from_be_bytes(b[at..at + 2].try_into().unwrap()) as usize;
        // Same-sized literal/repeat record, zero all alpha values.
        b[at + 3..at + 3 + 128].fill(0);
        b[at + 2 + len - 1] = 0;
        let request = DecodeRequest::new("synthetic.rla");
        assert!(decode_with_mode(&b, &request, Some(RlaAlphaMode::Straight), RasterColorSpace::Srgb).is_ok());
        assert!(matches!(
            decode_with_mode(
                &b,
                &request,
                Some(RlaAlphaMode::Premultiplied),
                RasterColorSpace::LinearSrgb
            ),
            Err(RasterDecodeError::InvalidRla(_))
        ));
    }
    #[test]
    fn malformed_directory_headers_and_planes_are_rejected() {
        let b = include_bytes!("../../../tests/fixtures/raster/rla-16-c3-a8-mixed1.rla");
        let request = DecodeRequest::new("synthetic.rla");
        let decode = |b: &[u8], r: &DecodeRequest| {
            decode_with_mode(b, r, Some(RlaAlphaMode::Straight), RasterColorSpace::Unspecified)
        };
        for end in [0, 739, 751, b.len() - 1] {
            assert!(decode(&b[..end], &request).is_err());
        }
        for at in [2, 8, 18, 20, 22, 24, 27, 612, 658, 660, 662, 736, 740, 744, 748] {
            let mut v = b.to_vec();
            v[at] = 255;
            assert!(decode(&v, &request).is_err(), "field {at}");
        }
        let mut v = b.to_vec();
        let offset = v[740..744].to_vec();
        v[744..748].copy_from_slice(&offset);
        assert!(decode(&v, &request).is_err());
        let mut v = b.to_vec();
        v.push(0);
        assert!(decode(&v, &request).is_err());
        assert!(plane(&[127, 1], 1).is_err());
        assert!(plane(&[128, 1], 140).is_err());
        assert!(plane(&[0], 1).is_err());
    }
}
