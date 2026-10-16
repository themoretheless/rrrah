//! PVR v3 2D mip selection, ordinary normalized/float channels and PVRTC1.
use crate::{
    DecodeRequest,
    bounded_io::read_bounded,
    raster::{MAX_RASTER_BYTES, RasterDecodeError},
};
use rrrah_core::{DecodedRaster, RasterColorSpace, RasterPixels};
use std::sync::Arc;
// texture2ddecoder 0.1.2 PVRTCTexelInfo contains two 4-byte colors,
// 32 i8 weights and one u32 flag (44 bytes); conservatively admit 64.
const PVRTC_BLOCK_CREDIT: usize = 64;
fn pvrtc_table_credit(width: usize, height: usize, two: bool) -> u64 {
    (width.div_ceil(if two { 8 } else { 4 }) * height.div_ceil(4) * PVRTC_BLOCK_CREDIT) as u64
}
fn buffer<T: Copy>(
    len: usize,
    value: T,
    request: &DecodeRequest,
) -> Result<rrrah_core::PixelBuffer<T>, RasterDecodeError> {
    Ok(match &request.memory_budget {
        Some(budget) => budget
            .try_buffer(len, value)
            .map_err(crate::DecodeError::Memory)?
            .freeze()
            .into(),
        None => {
            let mut values = Vec::new();
            values
                .try_reserve_exact(len)
                .map_err(|_| RasterDecodeError::OutputTooLarge)?;
            values.resize(len, value);
            Arc::new(values).into()
        }
    })
}
fn bad(s: &'static str) -> RasterDecodeError {
    RasterDecodeError::InvalidPvr(s)
}
pub fn has_magic(b: &[u8]) -> bool {
    matches!(b.get(..4), Some(b"PVR\x03") | Some(b"\x03RVP"))
}
#[derive(Debug, Clone)]
pub struct PvrMetadata {
    pub fourcc: [u8; 4],
    pub key: u32,
    pub data: rrrah_core::PixelBuffer<u8>,
}
#[derive(Debug, Clone)]
pub struct PvrImage {
    pub raster: DecodedRaster,
    pub metadata: rrrah_core::PixelBuffer<PvrMetadata>,
    pub pixel_format: u64,
    pub mip_index: usize,
}
pub fn decode_pvr(request: &DecodeRequest) -> Result<PvrImage, RasterDecodeError> {
    request.check_cancelled()?;
    decode(&read_bounded(request)?, request)
}
fn rgba_channels<T: Copy>(source: &[T], names: &[u8], zero: T, one: T) -> [T; 4] {
    let mut out = [zero, zero, zero, one];
    for (&name, &value) in names.iter().zip(source) {
        match name {
            b'r' => out[0] = value,
            b'g' => out[1] = value,
            b'b' => out[2] = value,
            b'a' => out[3] = value,
            b'l' | b'i' => out[..3].fill(value),
            _ => {}
        }
    }
    out
}
fn unassociate(pixel: &mut [f32; 4]) -> Result<(), RasterDecodeError> {
    if pixel[3] == 0. {
        if pixel[..3].iter().any(|v| *v != 0.) {
            return Err(bad("zero-alpha emission requires associated-alpha rendering"));
        }
    } else {
        let alpha = pixel[3];
        for value in &mut pixel[..3] {
            *value /= alpha;
            if !value.is_finite() {
                return Err(bad("alpha unassociation overflow"));
            }
        }
    }
    Ok(())
}
pub fn decode(b: &[u8], request: &DecodeRequest) -> Result<PvrImage, RasterDecodeError> {
    request.check_cancelled()?;
    if b.len() < 52 || !has_magic(b) {
        return Err(bad("missing v3 header"));
    }
    let be = b[0] == 3;
    let word = |at: usize| {
        let a = b[at..at + 4].try_into().unwrap();
        if be {
            u32::from_be_bytes(a)
        } else {
            u32::from_le_bytes(a)
        }
    };
    let flags = word(4);
    if flags & !2 != 0 {
        return Err(bad("unknown flags"));
    }
    let a = b[8..16].try_into().unwrap();
    let format = if be {
        u64::from_be_bytes(a)
    } else {
        u64::from_le_bytes(a)
    };
    let color = match word(16) {
        0 => RasterColorSpace::LinearSrgb,
        1 => RasterColorSpace::Srgb,
        _ => return Err(bad("unknown color space")),
    };
    let associated = flags == 2 || matches!(format, 8 | 10);
    if associated && color == RasterColorSpace::Srgb {
        return Err(bad("sRGB premultiplication requires exporter qualification"));
    }
    let mut channel_type = word(20);
    let height = word(24) as usize;
    let width = word(28) as usize;
    if width == 0 || height == 0 || width > 65536 || height > 65536 {
        return Err(bad("invalid dimensions"));
    }
    if width as u64 * height as u64 * 16 > MAX_RASTER_BYTES {
        return Err(RasterDecodeError::OutputTooLarge);
    }
    if word(32) != 1 || word(36) != 1 || word(40) != 1 {
        return Err(bad("array, volume and cubemap selection pending"));
    }
    let levels = word(44) as usize;
    if levels == 0 || levels > (usize::BITS - width.max(height).leading_zeros()) as usize {
        return Err(bad("invalid mip count"));
    }
    let meta_len = word(48) as usize;
    if meta_len > 4 * 1024 * 1024 {
        return Err(bad("oversized metadata"));
    }
    let start = 52usize
        .checked_add(meta_len)
        .filter(|v| *v <= b.len())
        .ok_or_else(|| bad("truncated metadata"))?;
    let mut records = 0usize;
    let mut scan = 52;
    while scan < start {
        request.check_cancelled()?;
        if start - scan < 12 {
            return Err(bad("truncated metadata record"));
        }
        let len = word(scan + 8) as usize;
        scan = scan
            .checked_add(12)
            .and_then(|at| at.checked_add(len))
            .filter(|end| *end <= start)
            .ok_or_else(|| bad("metadata exceeds declared block"))?;
        records += 1;
    }
    let mut metadata_credit = request
        .memory_budget
        .as_ref()
        .map(|budget| budget.try_reserve((records * std::mem::size_of::<PvrMetadata>()) as u64))
        .transpose()
        .map_err(crate::DecodeError::Memory)?;
    let mut metadata = Vec::new();
    metadata
        .try_reserve_exact(records)
        .map_err(|_| RasterDecodeError::OutputTooLarge)?;
    if let Some(credit) = &mut metadata_credit {
        credit
            .ensure_bytes((metadata.capacity() * std::mem::size_of::<PvrMetadata>()) as u64)
            .map_err(crate::DecodeError::Memory)?;
    }
    let mut at = 52;
    let mut flips = [false; 2];
    let mut orientation_seen = false;
    let mut types_seen = false;
    let descriptor = format.to_le_bytes();
    let names = &descriptor[..4];
    let bits = &descriptor[4..];
    let channels = names.iter().position(|v| *v == 0).unwrap_or(4);
    while at < start {
        request.check_cancelled()?;
        if start - at < 12 {
            return Err(bad("truncated metadata record"));
        }
        let fourcc = word(at).to_le_bytes();
        let key = word(at + 4);
        let len = word(at + 8) as usize;
        at += 12;
        let end = at
            .checked_add(len)
            .filter(|v| *v <= start)
            .ok_or_else(|| bad("metadata exceeds declared block"))?;
        let data = &b[at..end];
        at = end;
        if &fourcc[..3] == b"PVR" {
            if fourcc[3] != 3 {
                return Err(bad("unknown reserved metadata identifier"));
            }
            match key {
                0 => {
                    if data.len() % 16 != 0 {
                        return Err(bad("invalid atlas metadata"));
                    }
                }
                3 => {
                    if data.len() != 3 || orientation_seen {
                        return Err(bad("invalid/duplicate orientation"));
                    }
                    orientation_seen = true;
                    flips = [data[0] != 0, data[1] != 0];
                }
                4 => {
                    if data.len() != 12 || data.iter().any(|v| *v != 0) {
                        return Err(bad("texture border interpretation pending"));
                    }
                }
                5 => {}
                6 => {
                    if data.len() != 4 || types_seen || channels == 0 || format >> 32 == 0 {
                        return Err(bad("invalid channel type override"));
                    }
                    types_seen = true;
                    channel_type = u32::from(data[0]);
                    if data[..channels].iter().any(|v| u32::from(*v) != channel_type) {
                        return Err(bad("mixed channel types unsupported"));
                    }
                }
                _ => return Err(bad("metadata changes interpretation or is unsupported")),
            }
        }
        let mut stored = buffer(data.len(), 0u8, request)?;
        for (out, source) in stored
            .get_mut()
            .unwrap()
            .chunks_mut(65536)
            .zip(data.chunks(65536))
        {
            request.check_cancelled()?;
            out.copy_from_slice(source);
        }
        metadata.push(PvrMetadata {
            fourcc,
            key,
            data: stored,
        });
    }
    let metadata: rrrah_core::PixelBuffer<PvrMetadata> = match metadata_credit {
        Some(credit) => credit
            .try_adopt(metadata)
            .map_err(crate::DecodeError::Memory)?
            .into(),
        None => Arc::new(metadata).into(),
    };
    let compressed = format >> 32 == 0;
    let component_bytes = if compressed {
        if be
            || if matches!(format, 25 | 26) {
                channel_type > 13
            } else {
                !matches!(format, 0..=3 | 6..=11) || channel_type != 0
            }
        {
            return Err(bad("unsupported compressed codec/type/byte order"));
        }
        if format <= 3 && (!width.is_power_of_two() || !height.is_power_of_two()) {
            return Err(bad("PVRTC1 dimensions must be powers of two"));
        }
        0
    } else {
        if channels == 0
            || names[channels..].iter().any(|v| *v != 0)
            || bits[channels..].iter().any(|v| *v != 0)
        {
            return Err(bad("invalid channel descriptor"));
        }
        for (i, name) in names[..channels].iter().enumerate() {
            if !matches!(name, b'r' | b'g' | b'b' | b'a' | b'l' | b'i' | b'x')
                || *name != b'x' && names[..i].contains(name)
            {
                return Err(bad("invalid/duplicate channel name"));
            }
        }
        if names[..channels].iter().all(|v| matches!(v, b'a' | b'x'))
            || names[..channels].iter().any(|v| matches!(v, b'l' | b'i'))
                && names[..channels].iter().any(|v| matches!(v, b'r' | b'g' | b'b'))
        {
            return Err(bad("ambiguous channel semantics"));
        }
        let bytes = match channel_type {
            0 => 1,
            4 => 2,
            12 => 4,
            _ => return Err(bad("unsupported channel type")),
        };
        if bits[..channels].iter().any(|v| usize::from(*v) != bytes * 8) {
            return Err(bad("packed/mixed channel widths unsupported"));
        }
        bytes
    };
    let length = |w: usize, h: usize| -> usize {
        if compressed {
            if matches!(format, 6..=11 | 25 | 26) {
                w.div_ceil(4) * h.div_ceil(4) * if matches!(format, 6 | 7 | 25) { 8 } else { 16 }
            } else if format < 2 {
                w.max(16) * h.max(8) / 4
            } else {
                w.max(8) * h.max(8) / 2
            }
        } else {
            w * h * channels * component_bytes
        }
    };
    let mut at = start;
    let mut selected = None;
    for mip in 0..levels {
        let w = (width >> mip).max(1);
        let h = (height >> mip).max(1);
        let len = length(w, h);
        let end = at
            .checked_add(len)
            .filter(|v| *v <= b.len())
            .ok_or_else(|| bad("truncated mip data"))?;
        if request.image_index == mip {
            selected = Some((&b[at..end], w, h));
        }
        at = end;
    }
    if at != b.len() {
        return Err(bad("trailing texture data"));
    }
    let (data, w, h) = selected.ok_or(crate::DecodeError::UnsupportedImageIndex {
        index: request.image_index,
    })?;
    let names = &names[..channels];
    let position = |x: usize, y: usize| {
        let x = if flips[0] { w - 1 - x } else { x };
        let y = if flips[1] { h - 1 - y } else { y };
        y * w + x
    };
    let pixels = if compressed && matches!(format, 25 | 26) {
        // PowerVR maps signed byte/short/integer/float type labels to SNORM EAC.
        // Storage precision is intrinsic11-bit, independent of the label width.
        let signed = matches!(channel_type, 1 | 3 | 5 | 7 | 9 | 11 | 12);
        let dual = format == 26;
        let denominator = if signed { 1023f32 } else { 2047f32 };
        let mut storage = buffer(w * h * 4, 0f32, request)?;
        let values = storage.get_mut().unwrap();
        for (i, block) in data.chunks_exact(if dual { 16 } else { 8 }).enumerate() {
            request.check_cancelled()?;
            let red = crate::eac::decode(block[..8].try_into().unwrap(), signed);
            let green = if dual {
                crate::eac::decode(block[8..].try_into().unwrap(), signed)
            } else {
                [0; 16]
            };
            let left = i % w.div_ceil(4) * 4;
            let top = i / w.div_ceil(4) * 4;
            for y in 0..4 {
                for x in 0..4 {
                    if left + x < w && top + y < h {
                        let at = position(left + x, top + y) * 4;
                        let pixel = y * 4 + x;
                        values[at..at + 4].copy_from_slice(&[
                            f32::from(red[pixel]) / denominator,
                            f32::from(green[pixel]) / denominator,
                            0.,
                            1.,
                        ]);
                    }
                }
            }
        }
        RasterPixels::Rgba32Float(storage)
    } else if compressed && matches!(format, 6..=11) {
        if format == 6 {
            for block in data.chunks_exact(8) {
                request.check_cancelled()?;
                crate::etc1::validate(block).map_err(bad)?;
            }
        }
        let block_size = if matches!(format, 6 | 7) { 8 } else { 16 };
        let mut storage = if associated {
            RasterPixels::Rgba32Float(buffer(w * h * 4, 0f32, request)?)
        } else {
            RasterPixels::Rgba8(buffer(w * h * 4, 0u8, request)?)
        };
        let (mut bytes, mut floats) = match &mut storage {
            RasterPixels::Rgba8(v) => (Some(v.get_mut().unwrap()), None),
            RasterPixels::Rgba32Float(v) => (None, Some(v.get_mut().unwrap())),
            _ => unreachable!(),
        };
        for (i, block) in data.chunks_exact(block_size).enumerate() {
            request.check_cancelled()?;
            let decoded = if format == 6 {
                let mut colors = [0u32; 16];
                crate::etc1::decode(block, &mut colors);
                colors.map(|color| {
                    let [blue, green, red, alpha] = color.to_le_bytes();
                    [red, green, blue, alpha]
                })
            } else {
                crate::dds::block(
                    block,
                    match format {
                        7 => 1,
                        8 | 9 => 2,
                        10 | 11 => 3,
                        _ => unreachable!(),
                    },
                )
            };
            let left = (i % w.div_ceil(4)) * 4;
            let top = (i / w.div_ceil(4)) * 4;
            for y in 0..4 {
                for x in 0..4 {
                    if left + x < w && top + y < h {
                        let at = position(left + x, top + y) * 4;
                        let color = decoded[y * 4 + x];
                        if let Some(values) = bytes.as_deref_mut() {
                            values[at..at + 4].copy_from_slice(&color);
                        } else {
                            let mut pixel = color.map(|v| f32::from(v) / 255.);
                            unassociate(&mut pixel)?;
                            floats.as_deref_mut().unwrap()[at..at + 4].copy_from_slice(&pixel);
                        }
                    }
                }
            }
        }
        storage
    } else if compressed {
        let two = format < 2;
        let pw = w.max(if two { 16 } else { 8 });
        let ph = h.max(8);
        let mut decoded = buffer(pw * ph, 0u32, request)?;
        {
            let _table_credit = request
                .memory_budget
                .as_ref()
                .map(|budget| budget.try_reserve(pvrtc_table_credit(pw, ph, two)))
                .transpose()
                .map_err(crate::DecodeError::Memory)?;
            texture2ddecoder::decode_pvrtc(data, pw, ph, decoded.get_mut().unwrap(), two).map_err(bad)?;
        }
        request.check_cancelled()?;
        let mut storage = buffer(w * h * 4, 0u8, request)?;
        let values = storage.get_mut().unwrap();
        for y in 0..h {
            request.check_cancelled()?;
            for x in 0..w {
                let [blue, green, red, alpha] = decoded[y * pw + x].to_le_bytes();
                let p = position(x, y) * 4;
                values[p..p + 4].copy_from_slice(&[
                    red,
                    green,
                    blue,
                    if format % 2 == 0 { 255 } else { alpha },
                ]);
            }
        }
        RasterPixels::Rgba8(storage)
    } else if component_bytes == 1 {
        let mut storage = buffer(w * h * 4, 0u8, request)?;
        let values = storage.get_mut().unwrap();
        for (i, v) in data.chunks_exact(channels).enumerate() {
            if i % 4096 == 0 {
                request.check_cancelled()?;
            }
            let p = position(i % w, i / w) * 4;
            values[p..p + 4].copy_from_slice(&rgba_channels(v, names, 0, 255));
        }
        RasterPixels::Rgba8(storage)
    } else if component_bytes == 2 {
        let mut storage = buffer(w * h * 4, 0u16, request)?;
        let values = storage.get_mut().unwrap();
        for (i, v) in data.chunks_exact(channels * 2).enumerate() {
            if i % 4096 == 0 {
                request.check_cancelled()?;
            }
            let mut c = [0u16; 4];
            for (c, v) in c.iter_mut().zip(v.chunks_exact(2)) {
                let a = v.try_into().unwrap();
                *c = if be {
                    u16::from_be_bytes(a)
                } else {
                    u16::from_le_bytes(a)
                };
            }
            let p = position(i % w, i / w) * 4;
            values[p..p + 4].copy_from_slice(&rgba_channels(&c, names, 0, 65535));
        }
        RasterPixels::Rgba16(storage)
    } else {
        let mut storage = buffer(w * h * 4, 0f32, request)?;
        let values = storage.get_mut().unwrap();
        for (i, v) in data.chunks_exact(channels * 4).enumerate() {
            if i % 4096 == 0 {
                request.check_cancelled()?;
            }
            let mut c = [0f32; 4];
            for (c, v) in c.iter_mut().zip(v.chunks_exact(4)) {
                let a = v.try_into().unwrap();
                *c = if be {
                    f32::from_be_bytes(a)
                } else {
                    f32::from_le_bytes(a)
                };
            }
            let pixel = rgba_channels(&c, names, 0., 1.);
            if pixel.iter().any(|v| !v.is_finite()) || !(0. ..=1.).contains(&pixel[3]) {
                return Err(bad("invalid floating color/alpha"));
            }
            let p = position(i % w, i / w) * 4;
            values[p..p + 4].copy_from_slice(&pixel);
        }
        RasterPixels::Rgba32Float(storage)
    };
    let pixels = if associated && !(compressed && matches!(format, 6..=11 | 25 | 26)) {
        let mut straight = buffer(w * h * 4, 0f32, request)?;
        let output = straight.get_mut().unwrap();
        for i in 0..w * h {
            if i % 4096 == 0 {
                request.check_cancelled()?;
            }
            let mut pixel: [f32; 4] = match &pixels {
                RasterPixels::Rgba8(v) => std::array::from_fn(|c| f32::from(v[i * 4 + c]) / 255.),
                RasterPixels::Rgba16(v) => std::array::from_fn(|c| f32::from(v[i * 4 + c]) / 65535.),
                RasterPixels::Rgba32Float(v) => v[i * 4..i * 4 + 4].try_into().unwrap(),
            };
            unassociate(&mut pixel)?;
            output[i * 4..i * 4 + 4].copy_from_slice(&pixel);
        }
        RasterPixels::Rgba32Float(straight)
    } else {
        pixels
    };
    request.check_cancelled()?;
    let raster = DecodedRaster::new(w as u32, h as u32, pixels, color)?
        .with_image_selection(request.image_index, levels)?;
    Ok(PvrImage {
        raster,
        metadata,
        pixel_format: format,
        mip_index: request.image_index,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    fn metadata_budget(source: &[u8]) -> u64 {
        let be = source.starts_with(b"\x03RVP");
        let word = |at: usize| {
            let bytes = source[at..at + 4].try_into().unwrap();
            (if be {
                u32::from_be_bytes(bytes)
            } else {
                u32::from_le_bytes(bytes)
            }) as usize
        };
        let end = 52 + word(48);
        let mut at = 52;
        let mut bytes = 0;
        while at < end {
            let len = word(at + 8);
            bytes += std::mem::size_of::<PvrMetadata>() + len;
            at += 12 + len;
        }
        bytes as u64
    }

    #[test]
    fn metadata_list_and_block_clones_retain_their_own_credit() {
        let source = include_bytes!("../../../tests/fixtures/raster/pvr-rgba8.pvr");
        let meta = metadata_budget(source);
        let budget = rrrah_core::MemoryBudget::new(meta + 60);
        let mut request = DecodeRequest::new("metadata.pvr");
        request.memory_budget = Some(budget.clone());
        let image = decode(source, &request).unwrap();
        let list = image.metadata.clone();
        drop(image);
        assert_eq!(budget.used(), meta);
        let block = list.iter().find(|m| m.fourcc == *b"CC0!").unwrap().data.clone();
        drop(list);
        assert_eq!(budget.used(), block.len() as u64);
        assert_eq!(&block[..], b"CC0 custom metadata");
        drop(block);
        assert_eq!(budget.used(), 0);
    }
    #[test]
    fn managed_native_and_pvrtc_exact_short_and_shared_ownership() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/raster");
        let manifest = std::fs::read_to_string(root.join("pvr-manifest.tsv")).unwrap()
            + &std::fs::read_to_string(root.join("pvr-etc1-manifest.tsv")).unwrap()
            + &std::fs::read_to_string(root.join("pvr-bc-manifest.tsv")).unwrap()
            + &std::fs::read_to_string(root.join("pvr-associated-bc-manifest.tsv")).unwrap()
            + &std::fs::read_to_string(root.join("pvr-eac-manifest.tsv")).unwrap();
        let mut cases = 0;
        for line in manifest.lines().filter(|v| !v.starts_with('#') && !v.is_empty()) {
            let f: Vec<_> = line.split('\t').collect();
            let source = std::fs::read(root.join(f[0])).unwrap();
            let oracle = std::fs::read(root.join(f[6])).unwrap();
            let width: usize = f[2].parse().unwrap();
            let height: usize = f[3].parse().unwrap();
            let be = source.starts_with(b"\x03RVP");
            let format = if be {
                u64::from_be_bytes(source[8..16].try_into().unwrap())
            } else {
                u64::from_le_bytes(source[8..16].try_into().unwrap())
            };
            let scratch = if format <= 3 {
                width.max(if format < 2 { 16 } else { 8 }) * height.max(8) * 4
            } else {
                0
            };
            let size = oracle.len() as u64;
            let flags = if be {
                u32::from_be_bytes(source[4..8].try_into().unwrap())
            } else {
                u32::from_le_bytes(source[4..8].try_into().unwrap())
            };
            let native_size = (width
                * height
                * 4
                * if format >> 32 == 0 {
                    1
                } else {
                    ((format >> 32) & 255) as usize / 8
                }) as u64;
            let backend = if format <= 3 {
                pvrtc_table_credit(
                    width.max(if format < 2 { 16 } else { 8 }),
                    height.max(8),
                    format < 2,
                )
            } else {
                0
            };
            let associated = flags == 2 || matches!(format, 8 | 10);
            let peak = if matches!(format, 25 | 26) || associated && matches!(format, 6..=11) {
                size
            } else {
                (scratch as u64 + backend.max(native_size)).max(if associated {
                    native_size + size
                } else {
                    native_size
                })
            };
            let retained = size + metadata_budget(&source);
            let peak = peak + metadata_budget(&source);
            let budget = rrrah_core::MemoryBudget::new(peak);
            let mut request = DecodeRequest::new(f[0]);
            request.image_index = f[1].parse().unwrap();
            request.memory_budget = Some(budget.clone());
            let image = decode(&source, &request).unwrap();
            let actual: Vec<u8> = match image.raster.pixels() {
                RasterPixels::Rgba8(v) => v.to_vec(),
                RasterPixels::Rgba16(v) => v.iter().flat_map(|v| v.to_le_bytes()).collect(),
                RasterPixels::Rgba32Float(v) => v.iter().flat_map(|v| v.to_le_bytes()).collect(),
            };
            assert_eq!(actual, oracle, "{} mip {}", f[0], f[1]);
            assert_eq!((budget.used(), budget.peak()), (retained, peak));
            let shared = image.clone();
            drop(image);
            assert_eq!(budget.used(), retained);
            drop(shared);
            assert_eq!(budget.used(), 0);
            let short = rrrah_core::MemoryBudget::new(peak - 1);
            request.memory_budget = Some(short.clone());
            assert!(matches!(
                decode(&source, &request),
                Err(RasterDecodeError::Source(crate::DecodeError::Memory(_)))
            ));
            assert_eq!(short.used(), 0);
            cases += 1;
        }
        assert_eq!(cases, 666);
    }
    #[test]
    fn native_sample_and_powervr_pvrtc_oracles_match() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/raster");
        let manifest = std::fs::read_to_string(root.join("pvr-manifest.tsv")).unwrap()
            + &std::fs::read_to_string(root.join("pvr-etc1-manifest.tsv")).unwrap()
            + &std::fs::read_to_string(root.join("pvr-bc-manifest.tsv")).unwrap()
            + &std::fs::read_to_string(root.join("pvr-associated-bc-manifest.tsv")).unwrap()
            + &std::fs::read_to_string(root.join("pvr-eac-manifest.tsv")).unwrap();
        for line in manifest.lines().filter(|s| !s.starts_with('#') && !s.is_empty()) {
            let c: Vec<_> = line.split('\t').collect();
            let mut request = DecodeRequest::new(root.join(c[0]));
            request.image_index = c[1].parse().unwrap();
            let image = decode_pvr(&request).unwrap();
            let raster = &image.raster;
            assert_eq!(
                (raster.width(), raster.height()),
                (c[2].parse().unwrap(), c[3].parse().unwrap())
            );
            assert_eq!(raster.image_index(), request.image_index);
            assert_eq!(image.mip_index, request.image_index);
            assert_eq!(
                raster.color_space(),
                if c[5] == "0" {
                    &RasterColorSpace::LinearSrgb
                } else {
                    &RasterColorSpace::Srgb
                }
            );
            assert!(
                image
                    .metadata
                    .iter()
                    .any(|m| m.fourcc == *b"CC0!" && m.key == 42 && &m.data[..] == b"CC0 custom metadata")
            );
            let oracle = std::fs::read(root.join(c[6])).unwrap();
            let mut actual = Vec::new();
            match raster.pixels() {
                RasterPixels::Rgba8(v) => actual.extend_from_slice(v),
                RasterPixels::Rgba16(v) => {
                    for x in v.iter() {
                        actual.extend_from_slice(&x.to_le_bytes());
                    }
                }
                RasterPixels::Rgba32Float(v) => {
                    for x in v.iter() {
                        actual.extend_from_slice(&x.to_le_bytes());
                    }
                }
            }
            assert_eq!(actual, oracle, "{} mip {}", c[0], c[1]);
        }
    }
    #[test]
    fn every_fixture_prefix_is_rejected_before_partial_mip_output() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/raster");
        let manifest = std::fs::read_to_string(root.join("pvr-manifest.tsv")).unwrap()
            + &std::fs::read_to_string(root.join("pvr-etc1-manifest.tsv")).unwrap()
            + &std::fs::read_to_string(root.join("pvr-bc-manifest.tsv")).unwrap()
            + &std::fs::read_to_string(root.join("pvr-associated-bc-manifest.tsv")).unwrap()
            + &std::fs::read_to_string(root.join("pvr-eac-manifest.tsv")).unwrap();
        let mut sources = std::collections::BTreeSet::new();
        for line in manifest.lines().filter(|s| !s.starts_with('#') && !s.is_empty()) {
            sources.insert(line.split('\t').next().unwrap());
        }
        for source in sources {
            let bytes = std::fs::read(root.join(source)).unwrap();
            let request = DecodeRequest::new(source);
            for end in 0..bytes.len() {
                assert!(decode(&bytes[..end], &request).is_err(), "{source}: prefix {end}");
            }
            assert!(decode(&bytes, &request).is_ok(), "{source}: complete source");
        }
    }
    #[test]
    fn cancellation_precedes_io_and_header_validation() {
        let mut request = DecodeRequest::new("missing-cancelled.pvr");
        request.cancellation = Some(crate::GenerationToken::new(
            Arc::new(std::sync::atomic::AtomicU64::new(2)),
            1,
        ));
        assert!(matches!(
            decode_pvr(&request),
            Err(RasterDecodeError::Source(crate::DecodeError::Cancelled))
        ));
        assert!(matches!(
            decode(&[], &request),
            Err(RasterDecodeError::Source(crate::DecodeError::Cancelled))
        ));
    }
    #[test]
    fn bounds_mips_flags_channels_and_metadata_fail_explicitly() {
        let original = include_bytes!("../../../tests/fixtures/raster/pvr-rgba8.pvr");
        let request = DecodeRequest::new("synthetic.pvr");
        for end in [0, 51, original.len() - 1] {
            assert!(decode(&original[..end], &request).is_err());
        }
        for (at, value) in [
            (4, 1),
            (16, 2),
            (20, 2),
            (24, 0),
            (28, 65537),
            (32, 2),
            (36, 2),
            (40, 6),
            (44, 0),
            (44, 4),
            (48, u32::MAX),
        ] {
            let mut b = original.to_vec();
            b[at..at + 4].copy_from_slice(&value.to_le_bytes());
            assert!(decode(&b, &request).is_err(), "offset {at} value {value}");
        }
        let mut b = original.to_vec();
        b.extend([0]);
        assert!(decode(&b, &request).is_err());
        let mut b = original.to_vec();
        b[8..12].copy_from_slice(b"rrba");
        assert!(decode(&b, &request).is_err());
        let mut b = original.to_vec();
        b[12] = 4;
        assert!(decode(&b, &request).is_err());
        for (at, value) in [
            (56, 1),
            (56, 4),
            (56, 6),
            (56, 7),
            (56, 8),
            (56, 90),
            (60, 2),
            (60, u32::MAX),
        ] {
            let mut b = original.to_vec();
            b[at..at + 4].copy_from_slice(&value.to_le_bytes());
            assert!(decode(&b, &request).is_err());
        }
        let mut request = request;
        request.image_index = 3;
        assert!(decode(original, &request).is_err());
    }
    fn large_associated_bc_source() -> Vec<u8> {
        let mut source = b"PVR\x03".to_vec();
        source.extend(0u32.to_le_bytes());
        source.extend(8u64.to_le_bytes());
        for word in [0u32, 0, 1024, 1024, 1, 1, 1, 1, 0] {
            source.extend(word.to_le_bytes());
        }
        let block = [
            0x88, 0x88, 0x88, 0x88, 0x88, 0x88, 0x88, 0x88, 0, 0, 255, 255, 0xe4, 0xe4, 0xe4, 0xe4,
        ];
        for _ in 0..256 * 256 {
            source.extend(block);
        }
        source
    }
    #[test]
    fn eac_color_declaration_alpha_identity_and_invalid_type_are_explicit() {
        for original in [
            include_bytes!("../../../tests/fixtures/raster/pvr-eac-25-t0-00.pvr").as_slice(),
            include_bytes!("../../../tests/fixtures/raster/pvr-eac-26-t1-00.pvr").as_slice(),
        ] {
            let metadata = u32::from_le_bytes(original[48..52].try_into().unwrap()) as usize;
            let mut source = original[..52].to_vec();
            source[48..52].fill(0);
            source.extend_from_slice(&original[52 + metadata..]);
            let baseline = decode(&source, &DecodeRequest::new("eac.pvr")).unwrap();
            let RasterPixels::Rgba32Float(values) = baseline.raster.pixels() else {
                panic!()
            };
            let expected: Vec<_> = values.iter().map(|v| v.to_bits()).collect();
            drop(baseline);
            let budget = rrrah_core::MemoryBudget::new(9 * 5 * 16);
            let mut request = DecodeRequest::new("eac.pvr");
            request.memory_budget = Some(budget.clone());
            source[16..20].copy_from_slice(&1u32.to_le_bytes());
            let encoded = decode(&source, &request).unwrap();
            assert_eq!(encoded.raster.color_space(), &RasterColorSpace::Srgb);
            let RasterPixels::Rgba32Float(values) = encoded.raster.pixels() else {
                panic!()
            };
            assert_eq!(values.iter().map(|v| v.to_bits()).collect::<Vec<_>>(), expected);
            drop(encoded);
            source[16..20].fill(0);
            source[4..8].copy_from_slice(&2u32.to_le_bytes());
            let associated = decode(&source, &request).unwrap();
            let RasterPixels::Rgba32Float(values) = associated.raster.pixels() else {
                panic!()
            };
            assert_eq!(values.iter().map(|v| v.to_bits()).collect::<Vec<_>>(), expected);
            drop(associated);
            assert_eq!((budget.used(), budget.peak()), (0, 9 * 5 * 16));
            source[4..8].fill(0);
            source[20..24].copy_from_slice(&14u32.to_le_bytes());
            let zero = rrrah_core::MemoryBudget::new(0);
            request.memory_budget = Some(zero.clone());
            assert!(matches!(
                decode(&source, &request),
                Err(RasterDecodeError::InvalidPvr(
                    "unsupported compressed codec/type/byte order"
                ))
            ));
            assert_eq!((zero.used(), zero.peak()), (0, 0));
        }
    }
    #[test]
    fn every_orientation_byte_uses_boolean_semantics_and_retains_original_metadata() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/raster");
        let mut cases = 0;
        for (name, size, oracle_name) in [
            ("pvr-etc1-0-rd.pvr", 4, "pvr-etc1-0-rd.pvr.mip0.rgba8"),
            ("pvr-bc1-0-00.pvr", 4, "pvr-bc1-0-00.pvr.mip0.rgba8"),
            ("pvr-bc3-0-00.pvr", 4, "pvr-bc3-0-00.pvr.mip0.rgba8"),
            (
                "pvr-dxt2-linear-00.pvr",
                16,
                "pvr-dxt2-linear-00.pvr.mip0.rgba32f",
            ),
        ] {
            let original = std::fs::read(root.join(name)).unwrap();
            let oracle = std::fs::read(root.join(oracle_name)).unwrap();
            assert_eq!(&original[52..56], b"PVR\x03");
            assert_eq!(&original[56..60], &3u32.to_le_bytes());
            let budget = rrrah_core::MemoryBudget::new(metadata_budget(&original) + (9 * 5 * size) as u64);
            let mut request = DecodeRequest::new(name);
            request.memory_budget = Some(budget.clone());
            for axis in 0..3 {
                for value in 0..=255u8 {
                    let mut source = original.clone();
                    source[64 + axis] = value;
                    let image = decode(&source, &request).unwrap();
                    let actual: Vec<u8> = match image.raster.pixels() {
                        RasterPixels::Rgba8(values) => values.to_vec(),
                        RasterPixels::Rgba32Float(values) => {
                            values.iter().flat_map(|v| v.to_le_bytes()).collect()
                        }
                        _ => panic!(),
                    };
                    for y in 0..5 {
                        for x in 0..9 {
                            let sx = if axis == 0 && value != 0 { 8 - x } else { x };
                            let sy = if axis == 1 && value != 0 { 4 - y } else { y };
                            assert_eq!(
                                &actual[(y * 9 + x) * size..(y * 9 + x + 1) * size],
                                &oracle[(sy * 9 + sx) * size..(sy * 9 + sx + 1) * size],
                                "{name} axis{axis} value{value}"
                            );
                        }
                    }
                    let metadata = image
                        .metadata
                        .iter()
                        .find(|record| record.fourcc == *b"PVR\x03" && record.key == 3)
                        .unwrap();
                    assert_eq!(&metadata.data[..], &source[64..67]);
                    drop(image);
                    assert_eq!(budget.used(), 0);
                    cases += 1;
                }
            }
        }
        assert_eq!(cases, 3072);
    }
    #[test]
    fn associated_bc_direct_output_fits_one_float_plane_without_byte_staging() {
        let source = large_associated_bc_source();
        let expected = include_bytes!("../../../tests/fixtures/raster/pvr-dxt2-linear-00.pvr.mip0.rgba32f");
        let expected: Vec<u32> = expected[..64]
            .as_chunks::<4>()
            .0
            .iter()
            .map(|bytes| u32::from_le_bytes(*bytes))
            .collect();
        let bytes = 1024 * 1024 * 16;
        let budget = rrrah_core::MemoryBudget::new(bytes);
        let mut request = DecodeRequest::new("direct-1024.pvr");
        request.memory_budget = Some(budget.clone());
        let image = decode(&source, &request).unwrap();
        let RasterPixels::Rgba32Float(values) = image.raster.pixels() else {
            panic!()
        };
        for (i, pixel) in values.as_chunks::<4>().0.iter().enumerate() {
            assert_eq!(pixel.map(f32::to_bits), expected[(i % 4) * 4..(i % 4 + 1) * 4]);
        }
        assert_eq!((budget.used(), budget.peak()), (bytes, bytes));
        let retained = image.raster.clone();
        drop(image);
        assert!(matches!(
            decode(&source, &request),
            Err(RasterDecodeError::Source(crate::DecodeError::Memory(_)))
        ));
        assert_eq!(budget.used(), bytes);
        drop(retained);
        assert_eq!(budget.used(), 0);
        let short = rrrah_core::MemoryBudget::new(bytes - 1);
        request.memory_budget = Some(short.clone());
        assert!(matches!(
            decode(&source, &request),
            Err(RasterDecodeError::Source(crate::DecodeError::Memory(_)))
        ));
        assert_eq!((short.used(), short.peak()), (0, 0));
    }
    #[test]
    #[ignore = "manual paired direct versus staged associated PVR profile"]
    fn direct_and_staged_associated_output_paired_profile() {
        use std::{hint::black_box, time::Instant};
        let direct = large_associated_bc_source();
        let mut staged = direct.clone();
        staged[8..16].copy_from_slice(&9u64.to_le_bytes());
        let budget = rrrah_core::MemoryBudget::new(20 * 1024 * 1024);
        let mut request = DecodeRequest::new("profile.pvr");
        request.memory_budget = Some(budget.clone());
        // Reconstruct the former two-stage operation using the same owned BC
        // decoder, allocation API, float normalization and validation helper.
        let load = |candidate: bool| {
            let image = decode(black_box(if candidate { &direct } else { &staged }), &request).unwrap();
            if candidate {
                image.raster
            } else {
                let RasterPixels::Rgba8(native) = image.raster.pixels() else {
                    panic!()
                };
                let mut pixels = buffer(native.len(), 0f32, &request).unwrap();
                let output = pixels.get_mut().unwrap();
                for (i, bytes) in native.as_chunks::<4>().0.iter().enumerate() {
                    if i % 4096 == 0 {
                        request.check_cancelled().unwrap();
                    }
                    let mut pixel = bytes.map(|v| f32::from(v) / 255.);
                    unassociate(&mut pixel).unwrap();
                    output[i * 4..i * 4 + 4].copy_from_slice(&pixel);
                }
                DecodedRaster::new(
                    1024,
                    1024,
                    RasterPixels::Rgba32Float(pixels),
                    RasterColorSpace::LinearSrgb,
                )
                .unwrap()
            }
        };
        let reference = load(false);
        let RasterPixels::Rgba32Float(values) = reference.pixels() else {
            panic!()
        };
        let expected: Vec<_> = values.iter().map(|v| v.to_bits()).collect();
        drop(reference);
        let candidate = load(true);
        let RasterPixels::Rgba32Float(values) = candidate.pixels() else {
            panic!()
        };
        assert_eq!(expected, values.iter().map(|v| v.to_bits()).collect::<Vec<_>>());
        drop(candidate);
        assert_eq!(budget.used(), 0);
        let run = |candidate| {
            let start = Instant::now();
            for _ in 0..8 {
                black_box(load(candidate));
            }
            assert_eq!(budget.used(), 0);
            start.elapsed().as_nanos()
        };
        run(false);
        run(true);
        for pair in 0..12 {
            let (staged_ns, direct_ns) = if pair % 2 == 0 {
                (run(false), run(true))
            } else {
                let direct = run(true);
                (run(false), direct)
            };
            eprintln!(
                "PVR_DIRECT_AB pair={pair} decodes=8 pixels_per_decode=1048576 staged_ns={staged_ns} direct_ns={direct_ns}"
            );
        }
    }
    #[test]
    fn intrinsic_bc_association_is_applied_once_and_zero_alpha_policy_is_explicit() {
        for original in [
            include_bytes!("../../../tests/fixtures/raster/pvr-dxt2-linear-00.pvr").as_slice(),
            include_bytes!("../../../tests/fixtures/raster/pvr-dxt4-linear-00.pvr").as_slice(),
        ] {
            let mut bytes = original.to_vec();
            let request = DecodeRequest::new("associated.pvr");
            let base = decode(&bytes, &request).unwrap();
            let RasterPixels::Rgba32Float(expected) = base.raster.pixels() else {
                panic!()
            };
            assert!(expected.iter().any(|v| *v > 1.));
            bytes[4..8].copy_from_slice(&2u32.to_le_bytes());
            let flagged = decode(&bytes, &request).unwrap();
            let RasterPixels::Rgba32Float(actual) = flagged.raster.pixels() else {
                panic!()
            };
            assert_eq!(
                actual.iter().map(|v| v.to_bits()).collect::<Vec<_>>(),
                expected.iter().map(|v| v.to_bits()).collect::<Vec<_>>()
            );
            let metadata = u32::from_le_bytes(bytes[48..52].try_into().unwrap()) as usize;
            bytes.drain(52..52 + metadata);
            bytes[48..52].fill(0);
            bytes[52..60].fill(0);
            let budget = rrrah_core::MemoryBudget::new(900);
            let mut request = DecodeRequest::new("zero-alpha.pvr");
            request.memory_budget = Some(budget.clone());
            assert!(matches!(
                decode(&bytes, &request),
                Err(RasterDecodeError::InvalidPvr(
                    "zero-alpha emission requires associated-alpha rendering"
                ))
            ));
            assert_eq!(budget.used(), 0);
            bytes[52..].fill(0);
            let black = decode(&bytes, &request).unwrap();
            let RasterPixels::Rgba32Float(values) = black.raster.pixels() else {
                panic!()
            };
            assert!(values.iter().all(|v| *v == 0.));
            drop(black);
            assert_eq!(budget.used(), 0);
        }
    }
    #[test]
    fn srgb_bc_premultiplied_storage_is_not_silently_reinterpreted() {
        let original = include_bytes!("../../../tests/fixtures/raster/pvr-bc2-0-00.pvr");
        let metadata = u32::from_le_bytes(original[48..52].try_into().unwrap()) as usize;
        let mut source = original[..52].to_vec();
        source[48..52].fill(0);
        source.extend_from_slice(&original[52 + metadata..]);
        let budget = rrrah_core::MemoryBudget::new(0);
        let mut request = DecodeRequest::new("associated-bc.pvr");
        request.memory_budget = Some(budget.clone());
        for format in [8u64, 10] {
            source[8..16].copy_from_slice(&format.to_le_bytes());
            source[16..20].copy_from_slice(&1u32.to_le_bytes());
            assert!(matches!(
                decode(&source, &request),
                Err(RasterDecodeError::InvalidPvr(
                    "sRGB premultiplication requires exporter qualification"
                ))
            ));
            assert_eq!((budget.used(), budget.peak()), (0, 0));
        }
    }
    #[test]
    fn etc1_invalid_endpoints_refuse_before_pixel_admission() {
        let original = include_bytes!("../../../tests/fixtures/raster/pvr-etc1-0-rd.pvr");
        let metadata = u32::from_le_bytes(original[48..52].try_into().unwrap()) as usize;
        let mut source = original[..52].to_vec();
        source[48..52].fill(0);
        source.extend_from_slice(&original[52 + metadata..]);
        let budget = rrrah_core::MemoryBudget::new(0);
        let mut request = DecodeRequest::new("invalid-etc1.pvr");
        request.memory_budget = Some(budget.clone());
        for channel in 0..3 {
            for value in [4, 249] {
                let mut bytes = source.clone();
                bytes[52 + channel] = value;
                bytes[55] |= 2;
                assert!(matches!(
                    decode(&bytes, &request),
                    Err(RasterDecodeError::InvalidPvr("invalid ETC1 differential color"))
                ));
                assert_eq!((budget.used(), budget.peak()), (0, 0));
            }
        }
    }
    #[test]
    fn compressed_variable_types_do_not_bypass_codec_interpretation() {
        let mut b = vec![0u8; 52];
        b[..4].copy_from_slice(b"PVR\x03");
        b[8..16].copy_from_slice(&3u64.to_le_bytes());
        for (at, value) in [(24, 8u32), (28, 8), (32, 1), (36, 1), (40, 1), (44, 1), (48, 16)] {
            b[at..at + 4].copy_from_slice(&value.to_le_bytes());
        }
        b.extend_from_slice(b"PVR\x03");
        b.extend_from_slice(&6u32.to_le_bytes());
        b.extend_from_slice(&4u32.to_le_bytes());
        b.extend_from_slice(&[0, 4, 0, 0]);
        b.extend_from_slice(&[0; 32]);
        assert!(matches!(
            decode(&b, &DecodeRequest::new("variable-types.pvr")),
            Err(RasterDecodeError::InvalidPvr("invalid channel type override"))
        ));
    }
    #[test]
    fn associated_linear_hdr_is_unassociated_and_invalid_alpha_is_rejected() {
        let original = include_bytes!("../../../tests/fixtures/raster/pvr-rgba32f-le.pvr");
        let meta = metadata_budget(original);
        let budget = rrrah_core::MemoryBudget::new(480 + meta);
        let mut request = DecodeRequest::new("synthetic.pvr");
        request.memory_budget = Some(budget.clone());
        let mut b = original.to_vec();
        b[4..8].copy_from_slice(&2u32.to_le_bytes());
        let start = 52 + u32::from_le_bytes(b[48..52].try_into().unwrap()) as usize;
        for v in b[start..start + 5 * 3 * 16].chunks_exact_mut(16) {
            if f32::from_le_bytes(v[12..].try_into().unwrap()) == 0. {
                v[..12].fill(0);
            }
        }
        for (i, v) in [4f32, -0.5, 0.25, 0.25].into_iter().enumerate() {
            b[start + i * 4..start + i * 4 + 4].copy_from_slice(&v.to_le_bytes());
        }
        let image = decode(&b, &request).unwrap();
        let RasterPixels::Rgba32Float(v) = image.raster.pixels() else {
            panic!()
        };
        assert_eq!(&v[..4], &[16., -2., 1., 0.25]);
        assert_eq!((budget.used(), budget.peak()), (240 + meta, 480 + meta));
        drop(image);
        assert_eq!(budget.used(), 0);
        let short = rrrah_core::MemoryBudget::new(479 + meta);
        request.memory_budget = Some(short.clone());
        assert!(matches!(
            decode(&b, &request),
            Err(RasterDecodeError::Source(crate::DecodeError::Memory(_)))
        ));
        assert_eq!(short.used(), 0);
        request.memory_budget = Some(budget.clone());
        let mut invalid = b.clone();
        invalid[start + 12..start + 16].fill(0);
        assert!(decode(&invalid, &request).is_err());
        for alpha in [f32::NAN, -1., 2.] {
            let mut invalid = b.clone();
            invalid[start + 12..start + 16].copy_from_slice(&alpha.to_le_bytes());
            assert!(decode(&invalid, &request).is_err());
        }
        let mut invalid = b;
        invalid[16..20].copy_from_slice(&1u32.to_le_bytes());
        assert!(decode(&invalid, &request).is_err());
    }
}
