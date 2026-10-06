//! PNG layer composition in MNG-VLC and global-palette MNG-LC subsets. Other control modes
//! are refused explicitly; no frame is silently treated as a complete animation.
use crate::{DecodeError, DecodeRequest, RasterDecodeError};
use rrrah_core::DecodedRaster;
const MAGIC: &[u8] = b"\x8aMNG\r\n\x1a\n";
pub(crate) fn has_magic(bytes: &[u8]) -> bool {
    bytes.starts_with(MAGIC)
}
fn bad(message: &'static str) -> RasterDecodeError {
    RasterDecodeError::InvalidMng(message)
}
#[derive(Debug)]
struct Layer {
    bytes: std::ops::Range<usize>,
    colors: [Option<std::ops::Range<usize>>; 4],
    local_color: bool,
    dimensions: [u32; 2],
    sixteen: bool,
    alpha: bool,
    palette: Option<std::ops::Range<usize>>,
    transparency: Option<std::ops::Range<usize>>,
    empty_palette: Option<std::ops::Range<usize>>,
    local_transparency: bool,
    intrapixel: bool,
}
fn color_slot(kind: &[u8]) -> Option<usize> {
    match kind {
        b"cHRM" => Some(0),
        b"gAMA" => Some(1),
        b"iCCP" => Some(2),
        b"sRGB" => Some(3),
        _ => None,
    }
}
#[derive(Debug)]
pub struct MngImage {
    pub raster: DecodedRaster,
    /// One frame per tick; zero describes a still image.
    pub ticks_per_second: u32,
}
pub fn decode_mng(request: &DecodeRequest) -> Result<MngImage, RasterDecodeError> {
    let source = crate::bounded_io::read_managed(request)?;
    decode(&source, request)
}
pub(crate) fn decode(bytes: &[u8], request: &DecodeRequest) -> Result<MngImage, RasterDecodeError> {
    request.check_cancelled()?;
    if !has_magic(bytes) {
        return Err(bad("signature"));
    }
    let mut at = 8usize;
    let mut header = None;
    let mut frames = Vec::new();
    let mut frame_start = None;
    let mut idat = false;
    let mut globals: [Option<std::ops::Range<usize>>; 4] = Default::default();
    let mut local_color = false;
    let mut frame_colors = globals.clone();
    let mut frame_dimensions = [0; 2];
    let mut sixteen = false;
    let mut alpha = false;
    let mut simple = false;
    let mut global_palette = None;
    let mut global_transparency = None;
    let mut palette = None;
    let mut transparency = None;
    let mut empty_palette = None;
    let mut local_transparency = false;
    let mut intrapixel = false;
    let mut ended = false;
    for _ in 0..65536 {
        request.check_cancelled()?;
        let h = bytes
            .get(at..at.checked_add(8).ok_or_else(|| bad("chunk overflow"))?)
            .ok_or_else(|| bad("truncated chunk"))?;
        let n = u32::from_be_bytes(h[..4].try_into().unwrap()) as usize;
        let end = at
            .checked_add(n)
            .and_then(|n| n.checked_add(12))
            .filter(|&end| end <= bytes.len())
            .ok_or_else(|| bad("chunk bounds"))?;
        let kind = &h[4..8];
        if !kind.iter().all(u8::is_ascii_alphabetic) || kind[2].is_ascii_lowercase() {
            return Err(bad("chunk type"));
        }
        let data = &bytes[at + 8..end - 4];
        let mut crc = flate2::Crc::new();
        for piece in bytes[at + 4..end - 4].chunks(65536) {
            request.check_cancelled()?;
            crc.update(piece);
        }
        if crc.sum() != u32::from_be_bytes(bytes[end - 4..end].try_into().unwrap()) {
            return Err(bad("chunk CRC"));
        }
        if header.is_none() && kind != b"MHDR" {
            return Err(bad("MHDR must be first"));
        }
        match kind {
            b"MHDR" if header.is_none() && data.len() == 28 => {
                let w = u32::from_be_bytes(data[..4].try_into().unwrap());
                let height = u32::from_be_bytes(data[4..8].try_into().unwrap());
                let ticks = u32::from_be_bytes(data[8..12].try_into().unwrap());
                let profile = u32::from_be_bytes(data[24..28].try_into().unwrap());
                if w == 0
                    || height == 0
                    || w > 32768
                    || height > 32768
                    || u64::from(w) * u64::from(height) * 8 > crate::raster::MAX_RASTER_BYTES
                {
                    return Err(bad("canvas limits"));
                }
                if profile & 1 == 0 || profile & 0x8000_fe34 != 0 || profile & 64 == 0 && profile & 384 != 0 {
                    return Err(bad("unsupported MNG profile or transparency"));
                }
                header = Some((w, height, ticks, profile & 64 == 0 || profile & 128 != 0));
                simple = profile & 2 != 0;
            }
            b"IHDR" if frame_start.is_none() && data.len() == 13 => {
                let (w, height, _, _) = header.unwrap();
                let lw = u32::from_be_bytes(data[..4].try_into().unwrap());
                let lh = u32::from_be_bytes(data[4..8].try_into().unwrap());
                if lw == 0 || lh == 0 || lw > w || lh > height || !matches!(data[9], 0 | 2 | 3 | 4 | 6) {
                    return Err(bad("opaque layer dimensions/color type"));
                }
                frame_dimensions = [lw, lh];
                sixteen = data[8] == 16;
                alpha = matches!(data[9], 4 | 6);
                intrapixel = data[11] == 64;
                if intrapixel && (!simple || !matches!(data[9], 2 | 6) || !matches!(data[8], 8 | 16)) {
                    return Err(bad("intrapixel filter profile/color type"));
                }
                palette = global_palette.clone();
                transparency = global_transparency.clone();
                empty_palette = None;
                local_transparency = false;
                frame_colors = globals.clone();
                local_color = false;
                frame_start = Some(at);
                idat = false;
            }
            b"IDAT" if frame_start.is_some() => {
                idat = true;
            }
            b"IEND" if frame_start.is_some() && data.is_empty() && idat => {
                if frames.len() >= 4096 {
                    return Err(bad("too many frames"));
                }
                frames.push(Layer {
                    bytes: frame_start.take().unwrap()..end,
                    colors: frame_colors.clone(),
                    local_color,
                    dimensions: frame_dimensions,
                    sixteen,
                    alpha,
                    palette: palette.clone(),
                    transparency: transparency.clone(),
                    empty_palette: empty_palette.clone(),
                    local_transparency,
                    intrapixel,
                });
            }
            b"MEND" if frame_start.is_none() && data.is_empty() && !frames.is_empty() => {
                if end != bytes.len() {
                    return Err(bad("trailing bytes"));
                }
                ended = true;
                break;
            }
            _ if frame_start.is_none() && color_slot(kind).is_some() => {
                let slot = color_slot(kind).unwrap();
                if !data.is_empty() {
                    let valid = match kind {
                        b"sRGB" => data.len() == 1 && data[0] <= 3,
                        b"gAMA" => data.len() == 4 && data != [0; 4],
                        b"cHRM" => data.len() == 32,
                        b"iCCP" => data.len() >= 3 && data.len() <= 16 * 1024 * 1024,
                        _ => false,
                    };
                    if !valid {
                        return Err(bad("global color chunk"));
                    }
                    if kind == b"sRGB" {
                        globals[0] = None;
                        globals[1] = None;
                    }
                    if kind == b"gAMA" || kind == b"cHRM" {
                        globals[3] = None;
                    }
                    globals[slot] = Some(at..end);
                } else {
                    globals[slot] = None;
                }
            }
            _ if frame_start.is_some() && !idat && color_slot(kind).is_some() => {
                if data.is_empty() {
                    return Err(bad("empty embedded color chunk"));
                }
                local_color = true;
            }
            b"PLTE" if frame_start.is_none() && simple => {
                if data.len() > 768 || data.len() % 3 != 0 {
                    return Err(bad("global palette length"));
                }
                global_palette = if data.is_empty() { None } else { Some(at..end) };
            }
            b"tRNS" if frame_start.is_none() && simple => {
                if data.len() > 256 {
                    return Err(bad("global transparency length"));
                }
                global_transparency = if data.is_empty() { None } else { Some(at..end) };
            }
            b"PLTE" if frame_start.is_some() && !idat => {
                if data.is_empty() {
                    if !simple || palette.is_none() || empty_palette.is_some() {
                        return Err(bad("missing global palette"));
                    }
                    empty_palette = Some(at..end);
                    alpha |= transparency.is_some();
                }
            }
            b"tRNS" if frame_start.is_some() && !idat => {
                alpha = true;
                local_transparency = true;
            }
            b"tEXt" | b"iTXt" | b"zTXt" => {}
            b"cICP" | b"sBIT" | b"pHYs" | b"eXIf" if frame_start.is_some() && !idat => {
                if kind == b"cICP" {
                    local_color = true;
                }
            }
            _ => return Err(bad("unsupported or misplaced MNG/PNG chunk")),
        }
        at = end;
    }
    if !ended {
        return Err(bad("missing MEND or chunk limit"));
    }
    let (width, height, ticks, transparent_background) = header.unwrap();
    let count = if ticks == 0 { 1 } else { frames.len() };
    if request.image_index >= count {
        return Err(DecodeError::UnsupportedImageIndex {
            index: request.image_index,
        }
        .into());
    }
    // A zero-tick stream has one final composition. A full-canvas selected
    // layer replaces predecessors; a partial layer retains their uncovered area.
    let layer = if ticks == 0 {
        frames.len() - 1
    } else {
        request.image_index
    };
    if !transparent_background {
        if frames[0].dimensions != [width, height] {
            return Err(bad("initial partial layer requires a background"));
        }
        if layer > 0 && frames[0].alpha {
            // Validate the initial presentation even when a later full-canvas
            // opaque layer would otherwise bypass all its ancestors.
            drop(compose_linear(
                bytes,
                &frames[..1],
                [width, height],
                request,
                false,
            )?);
        }
    }
    let selected = &frames[layer];
    let raster = if selected.dimensions == [width, height] && !selected.alpha {
        decode_layer(bytes, selected, request)?
    } else {
        let anchor = (0..=layer)
            .rev()
            .find(|&i| frames[i].dimensions == [width, height] && !frames[i].alpha);
        let layers = &frames[anchor.unwrap_or(0)..=layer];
        if layers.iter().any(|f| f.alpha) || anchor.is_none() {
            compose_linear(bytes, layers, [width, height], request, transparent_background)?
        } else if layers.iter().any(|f| f.sixteen) {
            compose::<u16>(bytes, layers, [width, height], request)?
        } else {
            compose::<u8>(bytes, layers, [width, height], request)?
        }
    }
    .with_image_selection(request.image_index, count)?;
    Ok(MngImage {
        raster,
        ticks_per_second: ticks,
    })
}

fn decode_layer(
    bytes: &[u8],
    selected: &Layer,
    request: &DecodeRequest,
) -> Result<DecodedRaster, RasterDecodeError> {
    let inherited: Vec<_> = if selected.local_color {
        Vec::new()
    } else {
        selected.colors.iter().flatten().collect()
    };
    let palette_extra = selected.empty_palette.as_ref().map_or(0, |_| {
        selected.palette.as_ref().unwrap().len() - 12
            + if selected.local_transparency {
                0
            } else {
                selected.transparency.as_ref().map_or(0, |r| r.len())
            }
    });
    let extra: usize = inherited.iter().map(|range| range.len()).sum::<usize>() + palette_extra;
    let len = selected
        .bytes
        .len()
        .checked_add(8)
        .and_then(|n| n.checked_add(extra))
        .ok_or_else(|| bad("PNG source size"))?;
    let _source_credit = request
        .memory_budget
        .as_ref()
        .map(|budget| budget.try_reserve(len as u64))
        .transpose()
        .map_err(DecodeError::Memory)?;
    let mut png = Vec::new();
    png.try_reserve_exact(len)
        .map_err(|e| DecodeError::Memory(rrrah_core::BufferError::Allocate(e)))?;
    png.extend_from_slice(b"\x89PNG\r\n\x1a\n");
    let header_end = selected.bytes.start + 25;
    png.extend_from_slice(&bytes[selected.bytes.start..header_end]);
    for range in inherited {
        png.extend_from_slice(&bytes[range.clone()]);
    }
    if let Some(empty) = &selected.empty_palette {
        png.extend_from_slice(&bytes[header_end..empty.start]);
        png.extend_from_slice(&bytes[selected.palette.clone().unwrap()]);
        if !selected.local_transparency {
            if let Some(trns) = &selected.transparency {
                png.extend_from_slice(&bytes[trns.clone()]);
            }
        }
        png.extend_from_slice(&bytes[empty.end..selected.bytes.end]);
    } else {
        png.extend_from_slice(&bytes[header_end..selected.bytes.end]);
    }
    let mut inner = request.clone();
    inner.image_index = 0;
    if selected.intrapixel {
        // Ordinary PNG unfiltering recovers the differenced samples first.
        png[27] = 0;
        let crc = crate::png_color::crc32(&png[12..29]);
        png[29..33].copy_from_slice(&crc.to_be_bytes());
        let mut at = 33;
        while at < png.len() {
            let n = u32::from_be_bytes(png[at..at + 4].try_into().unwrap()) as usize;
            if &png[at + 4..at + 8] == b"tRNS" && n == 6 {
                let key = &mut png[at + 8..at + 14];
                let r = u16::from_be_bytes(key[..2].try_into().unwrap());
                let g = u16::from_be_bytes(key[2..4].try_into().unwrap());
                let b = u16::from_be_bytes(key[4..].try_into().unwrap());
                let mask = if selected.sixteen { u16::MAX } else { 255 };
                key[..2].copy_from_slice(&(r.wrapping_sub(g) & mask).to_be_bytes());
                key[4..].copy_from_slice(&(b.wrapping_sub(g) & mask).to_be_bytes());
                let crc = crate::png_color::crc32(&png[at + 4..at + 8 + n]);
                png[at + 8 + n..at + 12 + n].copy_from_slice(&crc.to_be_bytes());
            }
            at += n + 12;
        }
    }
    let mut decoded = crate::raster::decode_raster_bytes(png, &inner)?;
    if selected.intrapixel {
        if let Some(p) = decoded.rgba8_mut() {
            for (index, pixel) in p.chunks_exact_mut(4).enumerate() {
                if index % 4096 == 0 {
                    request.check_cancelled()?;
                }
                pixel[0] = pixel[0].wrapping_add(pixel[1]);
                pixel[2] = pixel[2].wrapping_add(pixel[1]);
            }
        } else if let Some(p) = decoded.rgba16_mut() {
            for (index, pixel) in p.chunks_exact_mut(4).enumerate() {
                if index % 4096 == 0 {
                    request.check_cancelled()?;
                }
                pixel[0] = pixel[0].wrapping_add(pixel[1]);
                pixel[2] = pixel[2].wrapping_add(pixel[1]);
            }
        } else {
            return Err(bad("exclusive intrapixel samples required"));
        }
    }
    Ok(decoded)
}

fn compose_linear(
    bytes: &[u8],
    layers: &[Layer],
    dimensions: [u32; 2],
    request: &DecodeRequest,
    transparent_background: bool,
) -> Result<DecodedRaster, RasterDecodeError> {
    let count = dimensions[0] as usize * dimensions[1] as usize * 4;
    if count as u64 * 4 > crate::raster::MAX_RASTER_BYTES {
        return Err(RasterDecodeError::OutputTooLarge);
    }
    let credit = request
        .memory_budget
        .as_ref()
        .map(|b| b.try_reserve(count as u64 * 4))
        .transpose()
        .map_err(DecodeError::Memory)?;
    let mut canvas = Vec::new();
    canvas
        .try_reserve_exact(count)
        .map_err(|e| DecodeError::Memory(rrrah_core::BufferError::Allocate(e)))?;
    canvas.resize(count, 0f32);
    for (layer_index, layer) in layers.iter().enumerate() {
        request.check_cancelled()?;
        let decoded = decode_layer(bytes, layer, request)?;
        let prepared = crate::prepare_raster_for_display_with_budget_and_cancel(
            &decoded,
            request.memory_budget.as_ref(),
            || request.check_cancelled().is_err(),
        )?;
        let rrrah_core::RasterPixels::Rgba32Float(samples) = prepared.pixels() else {
            return Err(bad("linear layer expected"));
        };
        for row in 0..layer.dimensions[1] as usize {
            request.check_cancelled()?;
            for column in 0..layer.dimensions[0] as usize {
                if column % 4096 == 0 {
                    request.check_cancelled()?;
                }
                let source = (row * layer.dimensions[0] as usize + column) * 4;
                let destination = (row * dimensions[0] as usize + column) * 4;
                crate::animation::over(
                    &mut canvas[destination..destination + 4],
                    &samples[source..source + 4],
                );
            }
        }
        // Without background transparency, the initial composition must already
        // cover the canvas. A later opaque layer cannot repair an invalid start.
        if layer_index == 0 && !transparent_background && canvas.chunks_exact(4).any(|p| p[3] != 1.) {
            return Err(bad("initial partial layer requires a background"));
        }
    }
    if !transparent_background && canvas.chunks_exact(4).any(|p| p[3] != 1.) {
        return Err(bad("initial partial layer requires a background"));
    }
    let pixels = match credit {
        Some(c) => c.try_adopt(canvas).map_err(DecodeError::Memory)?.into(),
        None => std::sync::Arc::new(canvas).into(),
    };
    Ok(DecodedRaster::new(
        dimensions[0],
        dimensions[1],
        rrrah_core::RasterPixels::Rgba32Float(pixels),
        rrrah_core::RasterColorSpace::LinearSrgb,
    )?)
}

trait IntegerSample: Copy + Default {
    fn from8(value: u8) -> Self;
    fn from16(value: u16) -> Result<Self, RasterDecodeError>;
    fn freeze(
        values: Vec<Self>,
        credit: Option<rrrah_core::Reservation>,
    ) -> Result<rrrah_core::RasterPixels, RasterDecodeError>;
}
impl IntegerSample for u8 {
    fn from8(value: u8) -> Self {
        value
    }
    fn from16(value: u16) -> Result<Self, RasterDecodeError> {
        u8::try_from(value).map_err(|_| bad("layer precision admission"))
    }
    fn freeze(
        values: Vec<Self>,
        credit: Option<rrrah_core::Reservation>,
    ) -> Result<rrrah_core::RasterPixels, RasterDecodeError> {
        Ok(rrrah_core::RasterPixels::Rgba8(match credit {
            Some(c) => c.try_adopt(values).map_err(DecodeError::Memory)?.into(),
            None => std::sync::Arc::new(values).into(),
        }))
    }
}
impl IntegerSample for u16 {
    fn from8(value: u8) -> Self {
        u16::from(value) * 257
    }
    fn from16(value: u16) -> Result<Self, RasterDecodeError> {
        Ok(value)
    }
    fn freeze(
        values: Vec<Self>,
        credit: Option<rrrah_core::Reservation>,
    ) -> Result<rrrah_core::RasterPixels, RasterDecodeError> {
        Ok(rrrah_core::RasterPixels::Rgba16(match credit {
            Some(c) => c.try_adopt(values).map_err(DecodeError::Memory)?.into(),
            None => std::sync::Arc::new(values).into(),
        }))
    }
}
fn compose<T: IntegerSample>(
    bytes: &[u8],
    layers: &[Layer],
    dimensions: [u32; 2],
    request: &DecodeRequest,
) -> Result<DecodedRaster, RasterDecodeError> {
    let count = dimensions[0] as usize * dimensions[1] as usize * 4;
    let credit = request
        .memory_budget
        .as_ref()
        .map(|b| b.try_reserve(count as u64 * std::mem::size_of::<T>() as u64))
        .transpose()
        .map_err(DecodeError::Memory)?;
    let mut canvas = Vec::new();
    canvas
        .try_reserve_exact(count)
        .map_err(|error| DecodeError::Memory(rrrah_core::BufferError::Allocate(error)))?;
    canvas.resize(count, T::default());
    let mut color = None;
    let mut profile_credit = None;
    for layer in layers {
        request.check_cancelled()?;
        let decoded = decode_layer(bytes, layer, request)?;
        if matches!(
            decoded.color_space(),
            rrrah_core::RasterColorSpace::Unspecified | rrrah_core::RasterColorSpace::LinearRgbUnspecified
        ) {
            return Err(bad("unqualified layer color cannot be composed"));
        }
        if let Some(space) = &color {
            if decoded.color_space() != space {
                return Err(bad("mixed layer color spaces require linear composition"));
            }
        } else {
            if let rrrah_core::RasterColorSpace::Icc(profile) = decoded.color_space() {
                profile_credit = request
                    .memory_budget
                    .as_ref()
                    .map(|b| b.try_reserve(profile.len() as u64))
                    .transpose()
                    .map_err(DecodeError::Memory)?;
            }
            color = Some(decoded.color_space().clone());
        }
        for row in 0..layer.dimensions[1] as usize {
            request.check_cancelled()?;
            let source = row * layer.dimensions[0] as usize * 4;
            let destination = row * dimensions[0] as usize * 4;
            let length = layer.dimensions[0] as usize * 4;
            for offset in 0..length {
                if offset % 16384 == 0 {
                    request.check_cancelled()?;
                }
                canvas[destination + offset] = match decoded.pixels() {
                    rrrah_core::RasterPixels::Rgba8(p) => T::from8(p[source + offset]),
                    rrrah_core::RasterPixels::Rgba16(p) => T::from16(p[source + offset])?,
                    _ => return Err(bad("opaque integer layer expected")),
                };
            }
        }
    }
    let mut output = DecodedRaster::new(
        dimensions[0],
        dimensions[1],
        T::freeze(canvas, credit)?,
        color.unwrap(),
    )?;
    if let Some(credit) = profile_credit {
        output = output
            .with_color_profile_reservation(credit)
            .map_err(DecodeError::Memory)?;
    }
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;
    use rrrah_core::RasterPixels;
    fn alpha_stream(ticks: u32, layers: &[[u8; 4]]) -> Vec<u8> {
        use std::io::Write;
        let mut out = b"\x8aMNG\r\n\x1a\n".to_vec();
        let header: Vec<_> = [2u32, 1, ticks, 0, 0, 0, 457]
            .into_iter()
            .flat_map(u32::to_be_bytes)
            .collect();
        out.extend(chunk(b"MHDR", &header));
        for pixel in layers {
            let mut ihdr = Vec::new();
            ihdr.extend(1u32.to_be_bytes());
            ihdr.extend(1u32.to_be_bytes());
            ihdr.extend([8, 6, 0, 0, 0]);
            out.extend(chunk(b"IHDR", &ihdr));
            out.extend(chunk(b"sRGB", &[0]));
            let mut z = flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::default());
            z.write_all(&[0]).unwrap();
            z.write_all(pixel).unwrap();
            out.extend(chunk(b"IDAT", &z.finish().unwrap()));
            out.extend(chunk(b"IEND", &[]));
        }
        out.extend(chunk(b"MEND", &[]));
        out
    }

    #[test]
    fn transparent_partial_layers_blend_in_linear_light_and_keep_last_owner_credit() {
        let bytes = alpha_stream(10, &[[255, 0, 0, 255], [0, 255, 0, 128], [0, 0, 255, 0]]);
        let budget = rrrah_core::MemoryBudget::new(4096);
        let mut request = DecodeRequest::new("alpha.mng");
        request.image_index = 2;
        request.memory_budget = Some(budget.clone());
        let output = decode(&bytes, &request).unwrap().raster;
        let RasterPixels::Rgba32Float(samples) = output.pixels() else {
            panic!("linear canvas expected")
        };
        // Endpoint sRGB primaries are also linear endpoints: independent source-over result.
        let a = 128f32 / 255.;
        for (actual, expected) in samples.iter().zip([1. - a, a, 0., 1., 0., 0., 0., 0.]) {
            assert!((actual - expected).abs() < 2e-7, "{actual} != {expected}");
        }
        assert_eq!(output.image_count(), 3);
        assert_eq!(budget.used(), 32);
        let last = output.clone();
        drop(output);
        assert_eq!(budget.used(), 32);
        drop(last);
        assert_eq!(budget.used(), 0);
        request.image_index = 0;
        request.memory_budget = Some(rrrah_core::MemoryBudget::new(31));
        assert!(decode(&bytes, &request).is_err());
        assert_eq!(request.memory_budget.as_ref().unwrap().peak(), 0);
        let still = decode(
            &alpha_stream(0, &[[255, 0, 0, 128]]),
            &DecodeRequest::new("still.mng"),
        )
        .unwrap();
        assert_eq!(still.raster.image_count(), 1);
        let RasterPixels::Rgba32Float(p) = still.raster.pixels() else {
            panic!()
        };
        assert_eq!(p[3], a);
        assert_eq!(&p[4..], &[0., 0., 0., 0.]);
    }

    #[test]
    fn later_opaque_layer_cannot_repair_forbidden_initial_transparency() {
        let mut bytes = alpha_stream(0, &[[33, 81, 129, 128], [192, 64, 96, 255]]);
        bytes[16..20].copy_from_slice(&1u32.to_be_bytes());
        bytes[40..44].copy_from_slice(&329u32.to_be_bytes());
        let crc = crate::png_color::crc32(&bytes[12..44]);
        bytes[44..48].copy_from_slice(&crc.to_be_bytes());
        let budget = rrrah_core::MemoryBudget::new(4096);
        let mut request = DecodeRequest::new("invalid-background.mng");
        request.memory_budget = Some(budget.clone());
        assert!(matches!(
            decode(&bytes, &request),
            Err(RasterDecodeError::InvalidMng(
                "initial partial layer requires a background"
            ))
        ));
        assert_eq!(budget.used(), 0);
    }
    const RGB: &[u8] = include_bytes!("../../../tests/fixtures/mng/rgb8-two-frames.mng");
    #[test]
    fn intrapixel_filter_requires_simple_profile_and_releases_failed_admission() {
        let source = include_bytes!("../../../tests/fixtures/mng/intrapixel-rgb16-trns.mng");
        let mut bytes = source.to_vec();
        bytes[40..44].copy_from_slice(&457u32.to_be_bytes());
        let crc = crate::png_color::crc32(&bytes[12..44]);
        bytes[44..48].copy_from_slice(&crc.to_be_bytes());
        assert!(matches!(
            decode(&bytes, &DecodeRequest::new("vlc.mng")),
            Err(RasterDecodeError::InvalidMng(
                "intrapixel filter profile/color type"
            ))
        ));
        for limit in [0, 31, 32, 64, 128] {
            let budget = rrrah_core::MemoryBudget::new(limit);
            let mut request = DecodeRequest::new("limited.mng");
            request.memory_budget = Some(budget.clone());
            assert!(decode(source, &request).is_err());
            assert_eq!(budget.used(), 0);
            assert!(budget.peak() <= limit);
        }
    }
    #[test]
    fn palette_snapshots_and_transparency_nullification_do_not_recolor_earlier_layers() {
        let source = include_bytes!("../../../tests/fixtures/mng/palette8-global.mng");
        let mut at = 48;
        while &source[at + 4..at + 8] != b"IHDR" {
            at += u32::from_be_bytes(source[at..at + 4].try_into().unwrap()) as usize + 12;
        }
        let body = &source[at..source.len() - 12];
        let mut stream = source[..at].to_vec();
        stream[24..28].copy_from_slice(&10u32.to_be_bytes());
        let crc = crate::png_color::crc32(&stream[12..44]);
        stream[44..48].copy_from_slice(&crc.to_be_bytes());
        stream.extend_from_slice(body);
        stream.extend(chunk(b"PLTE", &[255, 255, 255, 0, 0, 0]));
        stream.extend(chunk(b"tRNS", &[128, 0]));
        stream.extend_from_slice(body);
        stream.extend(chunk(b"tRNS", &[]));
        stream.extend_from_slice(body);
        stream.extend(chunk(b"MEND", &[]));
        let budget = rrrah_core::MemoryBudget::new(4096);
        let mut request = DecodeRequest::new("palette-changes.mng");
        request.memory_budget = Some(budget.clone());
        fn linear(v: f64) -> f64 {
            let v = v / 255.;
            if v <= 0.04045 {
                v / 12.92
            } else {
                ((v + 0.055) / 1.055).powf(2.4)
            }
        }
        let a = 128. / 255.;
        let colored = [linear(192.), linear(64.), linear(96.)];
        for index in 0..3 {
            request.image_index = index;
            let frame = decode(&stream, &request).unwrap().raster;
            assert_eq!(frame.image_count(), 3);
            let prepared = crate::prepare_raster_for_display_with_budget(&frame, Some(&budget)).unwrap();
            let RasterPixels::Rgba32Float(p) = prepared.pixels() else {
                panic!()
            };
            let expected = match index {
                0 => [0., 0., 0., 0., colored[0], colored[1], colored[2], a],
                1 => [1., 1., 1., a, colored[0], colored[1], colored[2], a],
                _ => [1., 1., 1., 1., 0., 0., 0., 1.],
            };
            for (actual, expected) in p.iter().zip(expected) {
                assert!((*actual as f64 - expected).abs() < 2e-7, "frame {index}");
            }
            drop((prepared, frame));
            assert_eq!(budget.used(), 0);
        }
    }
    #[test]
    fn global_palette_requires_lc_profile_and_live_palette() {
        let source = include_bytes!("../../../tests/fixtures/mng/palette8-global.mng");
        let budget = rrrah_core::MemoryBudget::new(4096);
        let mut request = DecodeRequest::new("global.mng");
        request.memory_budget = Some(budget.clone());
        let mut vlc = source.to_vec();
        vlc[40..44].copy_from_slice(&457u32.to_be_bytes());
        let crc = crate::png_color::crc32(&vlc[12..44]);
        vlc[44..48].copy_from_slice(&crc.to_be_bytes());
        assert!(decode(&vlc, &request).is_err());
        let mut at = 48;
        while &source[at + 4..at + 8] != b"IHDR" {
            at += u32::from_be_bytes(source[at..at + 4].try_into().unwrap()) as usize + 12;
        }
        let mut cleared = source.to_vec();
        cleared.splice(at..at, chunk(b"PLTE", &[]));
        assert!(matches!(
            decode(&cleared, &request),
            Err(RasterDecodeError::InvalidMng("missing global palette"))
        ));
        let palette_end = 48 + u32::from_be_bytes(source[48..52].try_into().unwrap()) as usize + 12;
        let mut missing = source.to_vec();
        missing.drain(48..palette_end);
        assert!(decode(&missing, &request).is_err());
        assert_eq!(budget.used(), 0);
    }
    #[test]
    fn ffmpeg_qualified_alpha_sources_match_independent_linear_reference() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/mng");
        let manifest: serde_json::Value =
            serde_json::from_slice(&std::fs::read(root.join("alpha-manifest.json")).unwrap()).unwrap();
        for case in manifest["cases"].as_array().unwrap() {
            let file = case["file"].as_str().unwrap();
            for (index, expected) in case["linear_rgba"].as_array().unwrap().iter().enumerate() {
                let budget = rrrah_core::MemoryBudget::new(4096);
                let mut request = DecodeRequest::new(root.join(file));
                request.image_index = index;
                request.memory_budget = Some(budget.clone());
                let output = decode_mng(&request).unwrap();
                let RasterPixels::Rgba32Float(p) = output.raster.pixels() else {
                    panic!("linear float canvas required")
                };
                for (actual, expected) in p.iter().zip(expected.as_array().unwrap()) {
                    assert!(
                        (*actual as f64 - expected.as_f64().unwrap()).abs() < 2e-7,
                        "{file} frame {index}"
                    );
                }
                drop(output);
                assert_eq!(budget.used(), 0);
            }
        }
    }
    #[test]
    fn authored_integer_frames_keep_selection_precision_and_managed_ownership() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/mng");
        let manifest: serde_json::Value =
            serde_json::from_slice(&std::fs::read(root.join("manifest.json")).unwrap()).unwrap();
        for case in manifest["cases"].as_array().unwrap() {
            for (index, expected) in case["rgba"].as_array().unwrap().iter().enumerate() {
                let budget = rrrah_core::MemoryBudget::new(4096);
                let mut request = DecodeRequest::new(root.join(case["file"].as_str().unwrap()));
                request.image_index = index;
                request.memory_budget = Some(budget.clone());
                let decoded = decode_mng(&request).unwrap();
                assert_eq!(decoded.raster.image_index(), index);
                assert_eq!(
                    decoded.raster.image_count(),
                    case["rgba"].as_array().unwrap().len()
                );
                let values: Vec<u64> = match decoded.raster.pixels() {
                    RasterPixels::Rgba8(p) => {
                        assert!(p.is_managed());
                        p.iter().map(|&v| u64::from(v)).collect()
                    }
                    RasterPixels::Rgba16(p) => {
                        assert!(p.is_managed());
                        p.iter().map(|&v| u64::from(v)).collect()
                    }
                    _ => panic!("integer precision lost"),
                };
                assert_eq!(
                    values,
                    expected
                        .as_array()
                        .unwrap()
                        .iter()
                        .map(|v| v.as_u64().unwrap())
                        .collect::<Vec<_>>()
                );
                let held = decoded.raster.clone();
                drop(decoded);
                assert_eq!(budget.used(), held.pixel_capacity_bytes());
                drop(held);
                assert_eq!(budget.used(), 0);
            }
        }
    }
    #[test]
    fn mng_magic_wins_over_raw_suffix_and_frames_reach_common_router() {
        let path = std::env::temp_dir().join(format!("rrrah-mng-router-{}.mrw", std::process::id()));
        std::fs::write(&path, RGB).unwrap();
        let mut request = DecodeRequest::new(&path);
        request.image_index = 1;
        let decoded = crate::decode_image(&request).unwrap();
        std::fs::remove_file(path).unwrap();
        let crate::DecodedImage::Raster(raster) = decoded else {
            panic!("MNG routed as RAW")
        };
        assert_eq!((raster.image_index(), raster.image_count()), (1, 2));
        assert_eq!(raster.color_space(), &rrrah_core::RasterColorSpace::Srgb);
        assert!(crate::is_supported_image_path(std::path::Path::new(
            "animation.MNG"
        )));
    }

    #[test]
    fn zero_tick_full_canvas_layers_form_one_final_still() {
        let mut bytes = RGB.to_vec();
        bytes[24..28].copy_from_slice(&0u32.to_be_bytes());
        let crc = crate::png_color::crc32(&bytes[12..44]);
        bytes[44..48].copy_from_slice(&crc.to_be_bytes());
        let mut request = DecodeRequest::new("still.mng");
        let still = decode(&bytes, &request).unwrap();
        assert_eq!(still.ticks_per_second, 0);
        assert_eq!((still.raster.image_index(), still.raster.image_count()), (0, 1));
        request.image_index = 1;
        let animated = decode(RGB, &request).unwrap();
        match (still.raster.pixels(), animated.raster.pixels()) {
            (RasterPixels::Rgba8(a), RasterPixels::Rgba8(b)) => assert_eq!(a, b),
            _ => panic!("precision changed"),
        }
        assert!(matches!(
            decode(&bytes, &request),
            Err(RasterDecodeError::Source(DecodeError::UnsupportedImageIndex {
                index: 1
            }))
        ));
    }

    fn chunk(kind: &[u8], data: &[u8]) -> Vec<u8> {
        let mut out = (data.len() as u32).to_be_bytes().to_vec();
        out.extend_from_slice(kind);
        out.extend_from_slice(data);
        out.extend_from_slice(&crate::png_color::crc32(&out[4..]).to_be_bytes());
        out
    }
    fn no_local_srgb() -> Vec<u8> {
        let mut out = RGB[..48].to_vec();
        let mut at = 48;
        while at < RGB.len() {
            let n = u32::from_be_bytes(RGB[at..at + 4].try_into().unwrap()) as usize;
            let end = at + n + 12;
            if &RGB[at + 4..at + 8] != b"sRGB" {
                out.extend_from_slice(&RGB[at..end]);
            }
            at = end;
        }
        out
    }
    fn global_stream(base: &[u8], declarations: &[u8]) -> Vec<u8> {
        [base[..48].to_vec(), declarations.to_vec(), base[48..].to_vec()].concat()
    }
    #[test]
    fn color_defaults_snapshot_nullification_precedence_and_local_override() {
        let source = no_local_srgb();
        let mut request = DecodeRequest::new("global.mng");
        let srgb = chunk(b"sRGB", &[0]);
        let gamma = chunk(b"gAMA", &100000u32.to_be_bytes());
        for (declarations, color) in [
            (srgb.clone(), rrrah_core::RasterColorSpace::Srgb),
            (
                [gamma.clone(), srgb.clone()].concat(),
                rrrah_core::RasterColorSpace::Srgb,
            ),
            (
                [srgb.clone(), gamma.clone()].concat(),
                rrrah_core::RasterColorSpace::Unspecified,
            ),
        ] {
            let bytes = global_stream(&source, &declarations);
            for index in 0..2 {
                request.image_index = index;
                assert_eq!(decode(&bytes, &request).unwrap().raster.color_space(), &color);
            }
        }
        let overridden = global_stream(RGB, &gamma);
        request.image_index = 0;
        assert_eq!(
            decode(&overridden, &request).unwrap().raster.color_space(),
            &rrrah_core::RasterColorSpace::Srgb
        );
        let mut bytes = global_stream(&source, &srgb);
        let mut at = 48 + srgb.len();
        loop {
            let n = u32::from_be_bytes(bytes[at..at + 4].try_into().unwrap()) as usize;
            let end = at + n + 12;
            if &bytes[at + 4..at + 8] == b"IEND" {
                bytes.splice(end..end, chunk(b"sRGB", &[]));
                break;
            }
            at = end;
        }
        request.image_index = 0;
        assert_eq!(
            decode(&bytes, &request).unwrap().raster.color_space(),
            &rrrah_core::RasterColorSpace::Srgb
        );
        request.image_index = 1;
        assert_eq!(
            decode(&bytes, &request).unwrap().raster.color_space(),
            &rrrah_core::RasterColorSpace::AssumedSrgb
        );
        request.image_index = 0;
        for invalid in [
            chunk(b"sRGB", &[4]),
            chunk(b"gAMA", &[0; 4]),
            chunk(b"cHRM", &[0; 31]),
        ] {
            assert!(decode(&global_stream(&source, &invalid), &request).is_err());
        }
    }

    #[test]
    fn inherited_icc_preserves_profile_and_last_owner_budget() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../tests/fixtures/raster/pattern.profiled.png");
        let original = crate::decode_raster_file(&path).unwrap();
        let png = std::fs::read(path).unwrap();
        let mut at = 8;
        let mut icc = None;
        while at < png.len() {
            let n = u32::from_be_bytes(png[at..at + 4].try_into().unwrap()) as usize;
            let end = at + n + 12;
            if &png[at + 4..at + 8] == b"iCCP" {
                icc = Some(png[at..end].to_vec());
                break;
            }
            at = end;
        }
        let bytes = global_stream(&no_local_srgb(), &icc.unwrap());
        let budget = rrrah_core::MemoryBudget::new(32768);
        let mut request = DecodeRequest::new("icc-global.mng");
        request.memory_budget = Some(budget.clone());
        let output = decode(&bytes, &request).unwrap();
        assert!(matches!(
            output.raster.color_space(),
            rrrah_core::RasterColorSpace::Icc(_)
        ));
        assert_eq!(output.raster.color_space(), original.color_space());
        assert!(output.raster.capacity_bytes() > output.raster.pixel_capacity_bytes());
        let last = output.raster.clone();
        drop(output);
        assert_eq!(budget.used(), last.capacity_bytes());
        drop(last);
        assert_eq!(budget.used(), 0);
    }

    #[test]
    fn partial_composition_admits_canvas_first_and_rejects_missing_background() {
        let source = include_bytes!("../../../tests/fixtures/mng/rgb8-partial-two-frames.mng");
        let mut request = DecodeRequest::new("partial.mng");
        request.image_index = 1;
        for (limit, peak) in [(23, 0), (24, 24)] {
            let budget = rrrah_core::MemoryBudget::new(limit);
            request.memory_budget = Some(budget.clone());
            assert!(decode(source, &request).is_err());
            assert_eq!(budget.peak(), peak);
            assert_eq!(budget.used(), 0);
        }
        request.memory_budget = None;
        let mut no_base = source.to_vec();
        no_base[56..60].copy_from_slice(&2u32.to_be_bytes());
        no_base[60..64].copy_from_slice(&1u32.to_be_bytes());
        let crc = crate::png_color::crc32(&no_base[52..69]);
        no_base[69..73].copy_from_slice(&crc.to_be_bytes());
        assert!(matches!(
            decode(&no_base, &request),
            Err(RasterDecodeError::InvalidMng(
                "initial partial layer requires a background"
            ))
        ));
    }

    #[test]
    fn mixed_eight_and_sixteen_bit_layers_promote_without_quantization() {
        fn end_of_first_layer(bytes: &[u8]) -> usize {
            let mut at = 48;
            loop {
                let n = u32::from_be_bytes(bytes[at..at + 4].try_into().unwrap()) as usize;
                let end = at + n + 12;
                if &bytes[at + 4..at + 8] == b"IEND" {
                    return end;
                }
                at = end;
            }
        }
        let wide = include_bytes!("../../../tests/fixtures/mng/rgb16-partial-two-frames.mng");
        let narrow = include_bytes!("../../../tests/fixtures/mng/rgb8-partial-two-frames.mng");
        let mut mixed = wide.to_vec();
        mixed.splice(
            48..end_of_first_layer(wide),
            narrow[48..end_of_first_layer(narrow)].iter().copied(),
        );
        let mut request = DecodeRequest::new("mixed.mng");
        request.image_index = 1;
        let decoded = decode(&mixed, &request).unwrap();
        let RasterPixels::Rgba16(samples) = decoded.raster.pixels() else {
            panic!("precision lost")
        };
        let oracle = include_bytes!("../../../tests/fixtures/mng/rgb16-partial-two-frames.mng-frame-1.rgba");
        let expected: Vec<_> = oracle
            .chunks_exact(2)
            .map(|p| u16::from_le_bytes([p[0], p[1]]))
            .collect();
        assert_eq!(samples.as_slice(), expected);
    }

    #[test]
    fn truncated_crc_control_canvas_index_and_pressure_are_refused_without_retention() {
        let budget = rrrah_core::MemoryBudget::new(4096);
        let mut request = DecodeRequest::new("source.mng");
        request.memory_budget = Some(budget.clone());
        for end in 0..RGB.len() {
            assert!(decode(&RGB[..end], &request).is_err());
        }
        let mut corrupt = RGB.to_vec();
        corrupt[20] ^= 1;
        assert!(decode(&corrupt, &request).is_err());
        request.image_index = 2;
        assert!(decode(RGB, &request).is_err());
        request.image_index = 0;
        for (offset, value) in [(16, 0u32), (40, 5)] {
            let mut b = RGB.to_vec();
            b[offset..offset + 4].copy_from_slice(&value.to_be_bytes());
            let crc = crate::png_color::crc32(&b[12..44]);
            b[44..48].copy_from_slice(&crc.to_be_bytes());
            assert!(decode(&b, &request).is_err());
        }
        assert_eq!(budget.used(), 0);
        let tight = rrrah_core::MemoryBudget::new(1);
        request.memory_budget = Some(tight.clone());
        assert!(decode(RGB, &request).is_err());
        assert_eq!(tight.used(), 0);
    }
}
