//! KTX2 ordinary RGB/RGBA surfaces, retaining native precision and transfer.
use crate::deflate::INFLATE_BYTES;
use crate::{
    DecodeRequest,
    raster::{MAX_RASTER_BYTES, RasterDecodeError},
};
use rrrah_core::{DecodedRaster, RasterColorSpace, RasterPixels};
use std::{io::Read, ops::Range, sync::Arc};
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
    RasterDecodeError::InvalidKtx2(s)
}
pub fn has_magic(b: &[u8]) -> bool {
    b.starts_with(b"\xabKTX 20\xbb\r\n\x1a\n")
}
fn word(b: &[u8], at: usize) -> Result<u32, RasterDecodeError> {
    Ok(u32::from_le_bytes(
        b.get(at..at + 4)
            .ok_or_else(|| bad("truncated word"))?
            .try_into()
            .unwrap(),
    ))
}
fn long(b: &[u8], at: usize) -> Result<u64, RasterDecodeError> {
    Ok(u64::from_le_bytes(
        b.get(at..at + 8)
            .ok_or_else(|| bad("truncated offset"))?
            .try_into()
            .unwrap(),
    ))
}
fn range(b: &[u8], at: u64, n: u64) -> Result<Range<usize>, RasterDecodeError> {
    let end = at.checked_add(n).ok_or_else(|| bad("offset overflow"))?;
    if end > b.len() as u64 {
        return Err(bad("range outside container"));
    }
    Ok(at as usize..end as usize)
}
#[derive(Clone, Copy)]
struct Format {
    channels: usize,
    size: usize,
    srgb: bool,
    float: bool,
    bgr: bool,
}
fn format(vk: u32) -> Result<Format, RasterDecodeError> {
    let (channels, size, srgb, float, bgr) = match vk {
        9 => (1, 1, false, false, false),
        16 => (2, 1, false, false, false),
        70 => (1, 2, false, false, false),
        77 => (2, 2, false, false, false),
        100 => (1, 4, false, true, false),
        103 => (2, 4, false, true, false),
        76 => (1, 2, false, true, false),
        83 => (2, 2, false, true, false),
        90 => (3, 2, false, true, false),
        97 => (4, 2, false, true, false),
        23 => (3, 1, false, false, false),
        29 => (3, 1, true, false, false),
        30 => (3, 1, false, false, true),
        36 => (3, 1, true, false, true),
        37 => (4, 1, false, false, false),
        43 => (4, 1, true, false, false),
        44 => (4, 1, false, false, true),
        50 => (4, 1, true, false, true),
        84 => (3, 2, false, false, false),
        91 => (4, 2, false, false, false),
        106 => (3, 4, false, true, false),
        109 => (4, 4, false, true, false),
        _ => return Err(bad("Vulkan storage format unsupported")),
    };
    Ok(Format {
        channels,
        size,
        srgb,
        float,
        bgr,
    })
}
fn descriptor(d: &[u8], f: Format) -> Result<RasterColorSpace, RasterDecodeError> {
    let expected = 28 + 16 * f.channels;
    if d.len() != expected || word(d, 0)? as usize != expected || word(d, 4)? != 0 {
        return Err(bad("DFD length, vendor or descriptor type"));
    }
    if word(d, 8)? != (2 | (((expected - 4) as u32) << 16)) || d[12] != 1 || d[15] != 0 {
        return Err(bad("DFD version, color model or associated alpha"));
    }
    if d[16..20] != [0; 4] || d[20] != (f.size * f.channels) as u8 || d[21..28] != [0; 7] {
        return Err(bad(
            "DFD block geometry or plane storage disagrees with Vulkan format",
        ));
    }
    if d[14] != if f.srgb { 2 } else { 1 } {
        return Err(bad("DFD transfer disagrees with Vulkan format"));
    }
    for index in 0..f.channels {
        let at = 28 + index * 16;
        let channel = if index == 3 {
            15
        } else if f.bgr {
            2 - index as u8
        } else {
            index as u8
        };
        let qualifier = if f.float {
            0xc0
        } else if f.srgb && index == 3 {
            0x10
        } else {
            0
        };
        if u16::from_le_bytes(d[at..at + 2].try_into().unwrap()) as usize != index * f.size * 8
            || d[at + 2] as usize != f.size * 8 - 1
            || d[at + 3] != (channel | qualifier)
            || d[at + 4..at + 8] != [0; 4]
        {
            return Err(bad("DFD sample layout disagrees with Vulkan format"));
        }
        let (lower, upper) = if f.float {
            (0xbf800000, 0x3f800000)
        } else {
            (0, (1u32 << (f.size * 8)) - 1)
        };
        if word(d, at + 8)? != lower || word(d, at + 12)? != upper {
            return Err(bad("DFD sample range unsupported"));
        }
    }
    Ok(match (d[13], d[14]) {
        (1, 1) => RasterColorSpace::LinearSrgb,
        (1, 2) => RasterColorSpace::Srgb,
        (_, 1) => RasterColorSpace::LinearRgbUnspecified,
        _ => RasterColorSpace::Unspecified,
    })
}
fn metadata(b: &[u8], request: &DecodeRequest) -> Result<(bool, bool), RasterDecodeError> {
    let (mut at, mut flipx, mut flipy) = (0, false, false);
    let mut count = 0usize;
    while at < b.len() {
        request.check_cancelled()?;
        let n = word(b, at)? as usize;
        at += 4;
        let end = at.checked_add(n).ok_or_else(|| bad("metadata overflow"))?;
        let entry = b.get(at..end).ok_or_else(|| bad("truncated metadata"))?;
        let split = entry
            .iter()
            .position(|v| *v == 0)
            .ok_or_else(|| bad("unterminated metadata key"))?;
        let key = std::str::from_utf8(&entry[..split]).map_err(|_| bad("metadata UTF8"))?;
        if key.is_empty() {
            return Err(bad("empty or duplicate metadata key"));
        }
        count += 1;
        let value = entry[split + 1..]
            .strip_suffix(&[0])
            .unwrap_or(&entry[split + 1..]);
        match key {
            "KTXorientation" => match value {
                b"rd" => (),
                b"ru" => flipy = true,
                b"ld" => flipx = true,
                b"lu" => {
                    flipx = true;
                    flipy = true;
                }
                _ => return Err(bad("orientation unsupported")),
            },
            "KTXswizzle" | "KTXanimData" => return Err(bad("swizzle or animation semantics pending")),
            _ => (),
        }
        let padded = end
            .checked_add((4 - n % 4) % 4)
            .ok_or_else(|| bad("metadata padding overflow"))?;
        if b.get(end..padded)
            .ok_or_else(|| bad("truncated metadata padding"))?
            .iter()
            .any(|v| *v != 0)
        {
            return Err(bad("nonzero metadata padding"));
        }
        at = padded;
    }
    if count > 1 {
        let mut keys = buffer(count, (0usize, 0usize), request)?;
        let entries = keys.get_mut().unwrap();
        let mut at = 0;
        for entry in entries.iter_mut() {
            request.check_cancelled()?;
            let n = word(b, at)? as usize;
            at += 4;
            let len = b[at..at + n].iter().position(|v| *v == 0).unwrap();
            *entry = (at, len);
            at += n + (4 - n % 4) % 4;
        }
        entries.sort_unstable_by(|a, c| b[a.0..a.0 + a.1].cmp(&b[c.0..c.0 + c.1]));
        request.check_cancelled()?;
        for pair in entries.windows(2) {
            let (a, c) = (pair[0], pair[1]);
            if b[a.0..a.0 + a.1] == b[c.0..c.0 + c.1] {
                return Err(bad("empty or duplicate metadata key"));
            }
        }
    }
    Ok((flipx, flipy))
}
pub fn decode(b: &[u8], request: &DecodeRequest) -> Result<DecodedRaster, RasterDecodeError> {
    request.check_cancelled()?;
    if b.len() < 104 || !has_magic(b) {
        return Err(bad("header or signature"));
    }
    let f = format(word(b, 12)?)?;
    if word(b, 16)? as usize != f.size {
        return Err(bad("typeSize disagrees with Vulkan format"));
    }
    let (width, height) = (word(b, 20)?, word(b, 24)?);
    if word(b, 28)? != 0 || word(b, 32)? != 0 || word(b, 36)? != 1 {
        return Err(bad("volume, array or cube selection pending"));
    }
    // Uncompressed KTX2 permits zero to request runtime mip generation.
    // A zero declaration still stores one selectable base level.
    let levels = word(b, 40)?.max(1) as usize;
    if width == 0
        || height == 0
        || width > 65536
        || height > 65536
        || levels > (u32::BITS - width.max(height).leading_zeros()) as usize
    {
        return Err(bad("dimensions or mip count"));
    }
    let output_size = if f.float { 4 } else { f.size };
    if u64::from(width) * u64::from(height) * (f.channels * f.size + 4 * output_size) as u64
        > MAX_RASTER_BYTES
    {
        return Err(RasterDecodeError::OutputTooLarge);
    }
    let compression = word(b, 44)?;
    if compression != 0 && compression != 3 {
        return Err(bad("supercompression scheme unsupported"));
    }
    let index_end = 80 + levels * 24;
    if index_end > b.len() {
        return Err(bad("truncated level index"));
    }
    let dfd = range(b, word(b, 48)?.into(), word(b, 52)?.into())?;
    if dfd.start % 4 != 0 || dfd.len() > 4096 {
        return Err(bad("DFD alignment or limit"));
    }
    let color = descriptor(&b[dfd.clone()], f)?;
    // Only absent primaries may use an explicit import assumption. Known
    // non-sRGB DFD primaries must not be relabelled as linear sRGB.
    let color = if b[dfd.start + 13] == 0 {
        request.qualify_linear_color(color)
    } else {
        color
    };
    let kvd = range(b, word(b, 56)?.into(), word(b, 60)?.into())?;
    if kvd.len() > 4 * 1024 * 1024 || (!kvd.is_empty() && kvd.start % 4 != 0) {
        return Err(bad("metadata alignment or limit"));
    }
    if long(b, 64)? != 0 || long(b, 72)? != 0 {
        return Err(bad("unexpected supercompression global data"));
    }
    let (flipx, flipy) = metadata(&b[kvd.clone()], request)?;
    // Dimension validation limits mip levels to at most 17 (65536 base edge).
    let mut ranges: [Range<usize>; 20] = std::array::from_fn(|_| 0..0);
    ranges[0] = 0..index_end;
    ranges[1] = dfd;
    let mut range_count = 2;
    if !kvd.is_empty() {
        ranges[range_count] = kvd;
        range_count += 1;
    }
    let mut data_ranges: [(Range<usize>, usize); 17] = std::array::from_fn(|_| (0..0, 0));
    for level in 0..levels {
        let at = 80 + level * 24;
        let r = range(b, long(b, at)?, long(b, at + 8)?)?;
        let expected = u64::from((width >> level).max(1))
            * u64::from((height >> level).max(1))
            * (f.channels * f.size) as u64;
        if r.is_empty() || long(b, at + 16)? != expected || (compression == 0 && r.len() as u64 != expected) {
            return Err(bad("mip payload size mismatch"));
        }
        if compression == 0 {
            let texel = f.channels * f.size;
            let alignment = match texel {
                3 | 6 => 12,
                _ => texel.max(4),
            };
            if r.start % alignment != 0 {
                return Err(bad("mip alignment"));
            }
        }
        ranges[range_count] = r.clone();
        range_count += 1;
        data_ranges[level] = (r, expected as usize);
    }
    let ranges = &mut ranges[..range_count];
    ranges.sort_unstable_by_key(|r| r.start);
    for pair in ranges.windows(2) {
        if pair[0].end > pair[1].start || b[pair[0].end..pair[1].start].iter().any(|v| *v != 0) {
            return Err(bad("overlapping ranges or nonzero padding"));
        }
    }
    if ranges.last().unwrap().end != b.len() {
        return Err(bad("trailing data"));
    }
    let (r, expected) =
        data_ranges[..levels]
            .get(request.image_index)
            .ok_or(crate::DecodeError::UnsupportedImageIndex {
                index: request.image_index,
            })?;
    let source = &b[r.clone()];
    // Uncompressed storage is borrowed; only DEFLATE requires expanded scratch.
    let expanded = if compression == 0 {
        None
    } else {
        let _decoder_credit = request
            .memory_budget
            .as_ref()
            .map(|budget| budget.try_reserve(INFLATE_BYTES))
            .transpose()
            .map_err(crate::DecodeError::Memory)?;
        let mut out = buffer(*expected, 0u8, request)?;
        let target = out.get_mut().unwrap();
        let mut decoder = flate2::bufread::ZlibDecoder::new(source);
        let mut chunk = [0u8; 65536];
        let mut written = 0;
        loop {
            request.check_cancelled()?;
            let n = decoder
                .read(&mut chunk)
                .map_err(|_| bad("DEFLATE stream invalid"))?;
            if n == 0 {
                break;
            }
            if written + n > *expected {
                return Err(bad("DEFLATE exceeds declared size"));
            }
            target[written..written + n].copy_from_slice(&chunk[..n]);
            written += n;
        }
        if written != *expected || decoder.total_in() != source.len() as u64 {
            return Err(bad("DEFLATE size or trailing stream data"));
        }
        Some(out)
    };
    let primary = expanded.as_ref().map_or(source, |v| v.as_slice());
    let width = (width >> request.image_index).max(1);
    let height = (height >> request.image_index).max(1);
    let count = width as usize * height as usize;
    let pixel_at = |index: usize| {
        let (x, y) = (index % width as usize, index / width as usize);
        let sx = if flipx { width as usize - 1 - x } else { x };
        let sy = if flipy { height as usize - 1 - y } else { y };
        (sy * width as usize + sx) * f.channels * f.size
    };
    let pixels = if f.float && f.size == 2 {
        let mut storage = buffer(count * 4, 0f32, request)?;
        let out = storage.get_mut().unwrap();
        for index in 0..count {
            if index % width as usize == 0 {
                request.check_cancelled()?;
            }
            let at = pixel_at(index);
            for c in 0..f.channels {
                let bits = u16::from_le_bytes(primary[at + c * 2..at + c * 2 + 2].try_into().unwrap());
                out[index * 4 + c] = rrrah_scientific::sample::half_to_f32(bits);
            }
            if f.channels < 4 {
                out[index * 4 + 3] = 1.0;
            }
        }
        RasterPixels::Rgba32Float(storage)
    } else {
        match f.size {
            1 => {
                let mut storage = buffer(count * 4, 0u8, request)?;
                let out = storage.get_mut().unwrap();
                for index in 0..count {
                    if index % width as usize == 0 {
                        request.check_cancelled()?;
                    }
                    let at = pixel_at(index);
                    let p = &primary[at..at + f.channels];
                    out[index * 4..index * 4 + 4].copy_from_slice(&[
                        p[if f.bgr { 2 } else { 0 }],
                        p.get(1).copied().unwrap_or(0),
                        p.get(if f.bgr { 0 } else { 2 }).copied().unwrap_or(0),
                        if f.channels == 4 { p[3] } else { 255 },
                    ]);
                }
                RasterPixels::Rgba8(storage)
            }
            2 => {
                let mut storage = buffer(count * 4, 0u16, request)?;
                let out = storage.get_mut().unwrap();
                for index in 0..count {
                    if index % width as usize == 0 {
                        request.check_cancelled()?;
                    }
                    let at = pixel_at(index);
                    for c in 0..4 {
                        out[index * 4 + c] = if c >= f.channels {
                            if c == 3 { 65535 } else { 0 }
                        } else {
                            u16::from_le_bytes(primary[at + c * 2..at + c * 2 + 2].try_into().unwrap())
                        };
                    }
                }
                RasterPixels::Rgba16(storage)
            }
            _ => {
                let mut storage = buffer(count * 4, 0f32, request)?;
                let out = storage.get_mut().unwrap();
                for index in 0..count {
                    if index % width as usize == 0 {
                        request.check_cancelled()?;
                    }
                    let at = pixel_at(index);
                    for c in 0..4 {
                        out[index * 4 + c] = if c >= f.channels {
                            if c == 3 { 1.0 } else { 0.0 }
                        } else {
                            f32::from_le_bytes(primary[at + c * 4..at + c * 4 + 4].try_into().unwrap())
                        };
                    }
                }
                RasterPixels::Rgba32Float(storage)
            }
        }
    };
    request.check_cancelled()?;
    Ok(
        DecodedRaster::new(width, height, pixels, color)?
            .with_image_selection(request.image_index, levels)?,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn pinned_inflate_allocation_fits_reserved_credit() {
        use std::sync::atomic::{AtomicUsize, Ordering};
        static REQUESTED: AtomicUsize = AtomicUsize::new(0);
        extern "C" fn probe(_: *mut std::ffi::c_void, n: u32, size: u32) -> *mut std::ffi::c_void {
            REQUESTED.store(n as usize * size as usize, Ordering::SeqCst);
            std::ptr::null_mut()
        }
        extern "C" fn free(_: *mut std::ffi::c_void, _: *mut std::ffi::c_void) {}
        let mut stream = zlib_rs::c_api::z_stream {
            zalloc: Some(probe),
            zfree: Some(free),
            ..Default::default()
        };
        assert_eq!(
            zlib_rs::inflate::init(&mut stream, zlib_rs::InflateConfig::default()),
            zlib_rs::ReturnCode::MemError
        );
        let bytes = REQUESTED.load(Ordering::SeqCst);
        assert!(
            bytes > 32768 && bytes as u64 <= INFLATE_BYTES,
            "backend requested {bytes}"
        );
        eprintln!("KTX2_INFLATE_BACKEND requested={bytes} admitted={INFLATE_BYTES}");
    }
    fn metadata_records(keys: &[String]) -> Vec<u8> {
        let mut bytes = Vec::new();
        for key in keys {
            let n = key.len() + 2;
            bytes.extend((n as u32).to_le_bytes());
            bytes.extend(key.as_bytes());
            bytes.extend([0, 42]);
            bytes.resize(bytes.len().next_multiple_of(4), 0);
        }
        bytes
    }
    #[test]
    fn largest_permitted_level_directory_fits_fixed_tables() {
        let original = include_bytes!("../../../tests/fixtures/raster/ktx2-mips-29-0-rd.ktx2");
        let descriptor_start = word(original, 48).unwrap() as usize;
        let descriptor_len = word(original, 52).unwrap() as usize;
        let mut source = original[..80].to_vec();
        source.resize(80 + 17 * 24, 0);
        source[20..24].copy_from_slice(&65536u32.to_le_bytes());
        source[24..28].copy_from_slice(&1u32.to_le_bytes());
        source[40..44].copy_from_slice(&17u32.to_le_bytes());
        let dfd_start = source.len() as u32;
        source[48..52].copy_from_slice(&dfd_start.to_le_bytes());
        source.extend(&original[descriptor_start..descriptor_start + descriptor_len]);
        let kvd = metadata_records(&["unknown".to_string()]);
        let kvd_start = source.len() as u32;
        source[56..60].copy_from_slice(&kvd_start.to_le_bytes());
        source[60..64].copy_from_slice(&(kvd.len() as u32).to_le_bytes());
        source.extend(kvd);
        for level in 0..17 {
            let length = (65536usize >> level) * 3;
            source.resize(source.len().next_multiple_of(12), 0);
            let offset = source.len() as u64;
            let at = 80 + level * 24;
            source[at..at + 8].copy_from_slice(&offset.to_le_bytes());
            source[at + 8..at + 16].copy_from_slice(&(length as u64).to_le_bytes());
            source[at + 16..at + 24].copy_from_slice(&(length as u64).to_le_bytes());
            for _ in 0..length / 3 {
                source.extend([1, 2, 3]);
            }
        }
        let budget = rrrah_core::MemoryBudget::new(4);
        let mut request = DecodeRequest::new("17-levels.ktx2");
        request.image_index = 16;
        request.memory_budget = Some(budget.clone());
        let frame = decode(&source, &request).unwrap();
        assert_eq!((frame.width(), frame.height(), frame.image_count()), (1, 1, 17));
        let RasterPixels::Rgba8(p) = frame.pixels() else {
            panic!()
        };
        assert_eq!(&p[..], &[1, 2, 3, 255]);
        drop(frame);
        assert_eq!((budget.used(), budget.peak()), (0, 4));
    }
    #[test]
    fn metadata_scratch_is_admitted_and_released_on_success_and_duplicate() {
        let keys: Vec<_> = (0..128).rev().map(|i| format!("unknown-{i:03}")).collect();
        let bytes = metadata_records(&keys);
        let scratch = (keys.len() * std::mem::size_of::<(usize, usize)>()) as u64;
        let budget = rrrah_core::MemoryBudget::new(scratch);
        let mut request = DecodeRequest::new("metadata.ktx2");
        request.memory_budget = Some(budget.clone());
        assert_eq!(metadata(&bytes, &request).unwrap(), (false, false));
        assert_eq!((budget.used(), budget.peak()), (0, scratch));
        let short = rrrah_core::MemoryBudget::new(scratch - 1);
        request.memory_budget = Some(short.clone());
        assert!(matches!(
            metadata(&bytes, &request),
            Err(RasterDecodeError::Source(crate::DecodeError::Memory(_)))
        ));
        assert_eq!(short.used(), 0);
        let mut keys = keys;
        keys.push(keys[0].clone());
        let duplicates = rrrah_core::MemoryBudget::new(scratch + 16);
        request.memory_budget = Some(duplicates.clone());
        assert!(matches!(
            metadata(&metadata_records(&keys), &request),
            Err(RasterDecodeError::InvalidKtx2("empty or duplicate metadata key"))
        ));
        assert_eq!(duplicates.used(), 0);
        for keys in [vec![], vec!["single".to_string()]] {
            let zero = rrrah_core::MemoryBudget::new(0);
            request.memory_budget = Some(zero.clone());
            assert_eq!(
                metadata(&metadata_records(&keys), &request).unwrap(),
                (false, false)
            );
            assert_eq!((zero.used(), zero.peak()), (0, 0));
        }
    }
    #[test]
    fn linear_assumption_only_qualifies_absent_dfd_primaries() {
        let original = include_bytes!("../../../tests/fixtures/raster/precision-32-0.ktx2");
        let dfd = word(original, 48).unwrap() as usize;
        let strict = DecodeRequest::new("primaries.ktx2");
        let mut explicit = strict.clone();
        explicit.assume_untagged_linear_srgb = true;
        for primaries in [0, 1, 2, 9, 255] {
            let mut bytes = original.to_vec();
            bytes[dfd + 13] = primaries;
            let baseline = decode(&bytes, &strict).unwrap();
            let frame = decode(&bytes, &explicit).unwrap();
            let expected = if primaries == 0 || primaries == 1 {
                RasterColorSpace::LinearSrgb
            } else {
                RasterColorSpace::LinearRgbUnspecified
            };
            assert_eq!(frame.color_space(), &expected);
            assert_eq!(
                baseline.color_space() == &RasterColorSpace::LinearSrgb,
                primaries == 1
            );
            let (RasterPixels::Rgba32Float(a), RasterPixels::Rgba32Float(b)) =
                (baseline.pixels(), frame.pixels())
            else {
                panic!("float precision lost")
            };
            assert!(a.iter().zip(b.iter()).all(|(a, b)| a.to_bits() == b.to_bits()));
        }
    }
    #[test]
    fn official_khronos_producer_files_preserve_all_mips_and_exact_ownership() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/raster");
        let mut count = 0;
        for line in include_str!("../../../tests/fixtures/raster/ktx2-khronos-producer-manifest.tsv")
            .lines()
            .filter(|line| !line.starts_with('#'))
        {
            let fields: Vec<_> = line.split('\t').collect();
            let source = std::fs::read(root.join(fields[0])).unwrap();
            let oracle = std::fs::read(root.join(fields[5])).unwrap();
            let size = oracle.len() as u64;
            let budget = rrrah_core::MemoryBudget::new(size);
            let mut request = DecodeRequest::new(fields[0]);
            request.image_index = fields[1].parse().unwrap();
            request.memory_budget = Some(budget.clone());
            let image = decode(&source, &request).unwrap();
            assert_eq!(
                (image.width(), image.height()),
                (fields[2].parse().unwrap(), fields[3].parse().unwrap())
            );
            assert_eq!(
                (image.image_index(), image.image_count()),
                (request.image_index, fields[4].parse().unwrap())
            );
            assert_eq!(image.color_space(), &RasterColorSpace::Srgb);
            let RasterPixels::Rgba8(values) = image.pixels() else {
                panic!()
            };
            assert!(values.is_managed());
            assert_eq!(values.as_slice(), oracle);
            let retained = image.clone();
            drop(image);
            assert_eq!((budget.used(), budget.peak()), (size, size));
            drop(retained);
            assert_eq!(budget.used(), 0);
            let short = rrrah_core::MemoryBudget::new(size - 1);
            request.memory_budget = Some(short.clone());
            assert!(matches!(
                decode(&source, &request),
                Err(RasterDecodeError::Source(crate::DecodeError::Memory(_)))
            ));
            assert_eq!((short.used(), short.peak()), (0, 0));
            if request.image_index == 0 {
                let zero = rrrah_core::MemoryBudget::new(0);
                request.memory_budget = Some(zero.clone());
                for end in 0..source.len() {
                    assert!(
                        matches!(
                            decode(&source[..end], &request),
                            Err(RasterDecodeError::InvalidKtx2(_))
                        ),
                        "{} prefix{end}",
                        fields[0]
                    );
                    assert_eq!((zero.used(), zero.peak()), (0, 0));
                }
            }
            count += 1;
        }
        assert_eq!(count, 8);
    }
    #[test]
    fn official_khronos_descriptors_match_all_supported_vulkan_storage_formats() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/raster");
        let mut count = 0;
        for line in include_str!("../../../tests/fixtures/raster/ktx2-khronos-dfd-manifest.tsv")
            .lines()
            .filter(|line| !line.starts_with('#'))
        {
            let fields: Vec<_> = line.split('\t').collect();
            let vk = fields[0].parse().unwrap();
            let oracle = std::fs::read(root.join(fields[1])).unwrap();
            assert_eq!(oracle.len(), fields[2].parse::<usize>().unwrap());
            let f = format(vk).unwrap();
            assert_eq!(
                descriptor(&oracle, f).unwrap(),
                if f.srgb {
                    RasterColorSpace::Srgb
                } else {
                    RasterColorSpace::LinearSrgb
                }
            );
            if matches!(vk, 76 | 83 | 90 | 97 | 9 | 16 | 70 | 77 | 100 | 103) {
                let family = if matches!(vk, 76 | 83 | 90 | 97) {
                    "half"
                } else {
                    "rg"
                };
                for compression in [0, 3] {
                    for orientation in ["rd", "ru", "ld", "lu"] {
                        let bytes = std::fs::read(
                            root.join(format!("ktx2-{family}-{vk}-{compression}-{orientation}.ktx2")),
                        )
                        .unwrap();
                        let offset = word(&bytes, 48).unwrap() as usize;
                        let len = word(&bytes, 52).unwrap() as usize;
                        assert_eq!(&bytes[offset..offset + len], oracle.as_slice());
                    }
                }
            }
            count += 1;
        }
        assert_eq!(count, 22);
    }
    #[test]
    fn half_dfd_and_storage_mismatches_refuse_before_output_admission() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/raster");
        let mut cases = 0;
        for vk in [76, 83, 90, 97] {
            let source = std::fs::read(root.join(format!("ktx2-half-{vk}-0-rd.ktx2"))).unwrap();
            let dfd = word(&source, 48).unwrap() as usize;
            for mutation in 0..8 {
                let mut invalid = source.clone();
                match mutation {
                    0 => invalid[16..20].copy_from_slice(&4u32.to_le_bytes()),
                    1 => invalid[dfd + 28..dfd + 30].copy_from_slice(&16u16.to_le_bytes()),
                    2 => invalid[dfd + 30] = 31,
                    3 => invalid[dfd + 31] &= 15,
                    4 => invalid[dfd + 14] = 2,
                    5 => invalid[dfd + 20] += 2,
                    6 => invalid[dfd + 40..dfd + 44].copy_from_slice(&65535u32.to_le_bytes()),
                    7 => {
                        let offset = long(&source, 80).unwrap();
                        invalid[80..88].copy_from_slice(&(offset + 2).to_le_bytes());
                    }
                    _ => unreachable!(),
                }
                let budget = rrrah_core::MemoryBudget::new(0);
                let mut request = DecodeRequest::new("bad-half.ktx2");
                request.memory_budget = Some(budget.clone());
                assert!(
                    matches!(decode(&invalid, &request), Err(RasterDecodeError::InvalidKtx2(_))),
                    "vk={vk} mutation={mutation}"
                );
                assert_eq!((budget.used(), budget.peak()), (0, 0));
                cases += 1;
            }
        }
        assert_eq!(cases, 32);
    }
    #[test]
    fn all_stored_mips_match_native_sample_contracts() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/raster");
        let manifest = std::fs::read_to_string(root.join("ktx2-mip-manifest.tsv")).unwrap()
            + &std::fs::read_to_string(root.join("ktx2-half-manifest.tsv")).unwrap()
            + &std::fs::read_to_string(root.join("ktx2-rg-manifest.tsv")).unwrap();
        let mut cases = 0;
        for line in manifest.lines().filter(|s| !s.starts_with('#') && !s.is_empty()) {
            let c: Vec<_> = line.split('\t').collect();
            let mut request = DecodeRequest::new(root.join(c[0]));
            request.image_index = c[1].parse().unwrap();
            let bytes = std::fs::read(&request.path).unwrap();
            let oracle = std::fs::read(root.join(c[6])).unwrap();
            let size = oracle.len() as u64;
            let scratch = if word(&bytes, 44).unwrap() == 0 {
                0
            } else {
                long(&bytes, 80 + request.image_index * 24 + 16).unwrap()
            };
            let peak = if scratch == 0 {
                size
            } else {
                scratch + size.max(INFLATE_BYTES)
            };
            let budget = rrrah_core::MemoryBudget::new(peak);
            request.memory_budget = Some(budget.clone());
            let image = decode(&bytes, &request).unwrap();
            assert_eq!(
                (image.width(), image.height()),
                (c[2].parse().unwrap(), c[3].parse().unwrap())
            );
            assert_eq!(
                (image.image_index(), image.image_count()),
                (request.image_index, 4)
            );
            assert_eq!(
                image.color_space(),
                &if c[5] == "srgb" {
                    RasterColorSpace::Srgb
                } else {
                    RasterColorSpace::LinearSrgb
                }
            );
            let actual: Vec<u8> = match image.pixels() {
                RasterPixels::Rgba8(v) => v.to_vec(),
                RasterPixels::Rgba16(v) => v.iter().flat_map(|x| x.to_le_bytes()).collect(),
                RasterPixels::Rgba32Float(v) => v.iter().flat_map(|x| x.to_le_bytes()).collect(),
            };
            assert_eq!(actual, oracle, "{} mip {}", c[0], c[1]);
            assert_eq!((budget.used(), budget.peak()), (size, peak));
            let shared = image.clone();
            drop(image);
            assert_eq!(budget.used(), size);
            drop(shared);
            assert_eq!(budget.used(), 0);
            let short = rrrah_core::MemoryBudget::new(peak - 1);
            request.memory_budget = Some(short.clone());
            assert!(matches!(
                decode(&bytes, &request),
                Err(RasterDecodeError::Source(crate::DecodeError::Memory(_)))
            ));
            assert_eq!(short.used(), 0);
            cases += 1;
            request.image_index = 4;
            assert!(matches!(
                decode(&bytes, &request),
                Err(RasterDecodeError::Source(
                    crate::DecodeError::UnsupportedImageIndex { index: 4 }
                ))
            ));
        }
        assert_eq!(cases, 416);
    }
    #[test]
    fn corrupt_deflate_releases_admitted_scratch() {
        let mut bytes = include_bytes!("../../../tests/fixtures/raster/ktx2-mips-109-3-rd.ktx2").to_vec();
        let offset = long(&bytes, 80).unwrap() as usize;
        bytes[offset] = 0;
        let scratch = long(&bytes, 96).unwrap();
        let budget = rrrah_core::MemoryBudget::new(scratch + INFLATE_BYTES);
        let mut request = DecodeRequest::new("corrupt.ktx2");
        request.memory_budget = Some(budget.clone());
        assert!(matches!(
            decode(&bytes, &request),
            Err(RasterDecodeError::InvalidKtx2(_))
        ));
        assert_eq!((budget.used(), budget.peak()), (0, scratch + INFLATE_BYTES));
    }
    #[test]
    fn selected_mip_keeps_pixels_transfer_and_selection() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/raster");
        let mut request = DecodeRequest::new(root.join("mips.ktx2"));
        let bytes = std::fs::read(&request.path).unwrap();
        request.image_index = 1;
        let image = decode(&bytes, &request).unwrap();
        assert_eq!((image.width(), image.height()), (1, 1));
        assert_eq!((image.image_index(), image.image_count()), (1, 2));
        assert_eq!(image.color_space(), &RasterColorSpace::Srgb);
        let RasterPixels::Rgba8(v) = image.pixels() else {
            panic!()
        };
        assert_eq!(&v[..], &[20, 40, 60, 255]);
        request.image_index = usize::MAX;
        assert!(matches!(
            decode(&bytes, &request),
            Err(RasterDecodeError::Source(
                crate::DecodeError::UnsupportedImageIndex { .. }
            ))
        ));
        request.image_index = 0;
        for end in 0..bytes.len() {
            assert!(decode(&bytes[..end], &request).is_err(), "prefix {end}");
        }
    }
    #[test]
    fn only_selected_deflate_payload_is_inflated_but_all_ranges_are_checked() {
        let mut bytes = include_bytes!("../../../tests/fixtures/raster/ktx2-mips-109-3-rd.ktx2").to_vec();
        let offset = long(&bytes, 80 + 24).unwrap() as usize;
        bytes[offset] = 0;
        let mut request = DecodeRequest::new("synthetic.ktx2");
        assert!(decode(&bytes, &request).is_ok());
        request.image_index = 1;
        assert!(decode(&bytes, &request).is_err());
        request.image_index = 0;
        let length = bytes.len() as u64;
        bytes[80 + 24..80 + 24 + 8].copy_from_slice(&length.to_le_bytes());
        assert!(decode(&bytes, &request).is_err());
    }
    #[test]
    fn sixteen_bit_samples_survive_plain_and_deflate() {
        for bytes in [
            include_bytes!("../../../tests/fixtures/raster/precision-16-0.ktx2").as_slice(),
            include_bytes!("../../../tests/fixtures/raster/precision-16-3.ktx2").as_slice(),
        ] {
            let frame = decode(bytes, &DecodeRequest::new("x.ktx2")).unwrap();
            let RasterPixels::Rgba16(p) = frame.pixels() else {
                panic!("precision lost")
            };
            assert_eq!(p.as_slice(), &[32768, 1, 65534, 65535, 32769, 2, 65535, 65535]);
            assert_eq!(frame.color_space(), &RasterColorSpace::LinearSrgb);
        }
    }
    #[test]
    fn float_hdr_survives_plain_and_deflate() {
        for bytes in [
            include_bytes!("../../../tests/fixtures/raster/precision-32-0.ktx2").as_slice(),
            include_bytes!("../../../tests/fixtures/raster/precision-32-3.ktx2").as_slice(),
        ] {
            let frame = decode(bytes, &DecodeRequest::new("x.ktx2")).unwrap();
            let RasterPixels::Rgba32Float(p) = frame.pixels() else {
                panic!("float lost")
            };
            assert_eq!(p.as_slice(), &[0.5, 0.1, 0.3, 1.0, 2.0, 0.2, 0.4, 1.0]);
        }
    }
    #[test]
    fn descriptor_mismatch_overlap_and_corrupt_deflate_are_rejected() {
        let bytes = include_bytes!("../../../tests/fixtures/raster/rgba8-rd-0.ktx2");
        let dfd = word(bytes, 48).unwrap() as usize;
        for at in [dfd + 12, dfd + 14, dfd + 15, dfd + 20, dfd + 31] {
            let mut invalid = bytes.to_vec();
            invalid[at] ^= 1;
            assert!(
                decode(&invalid, &DecodeRequest::new("x.ktx2")).is_err(),
                "byte {at}"
            );
        }
        let mut invalid = bytes.to_vec();
        invalid[80..88].copy_from_slice(&104u64.to_le_bytes());
        assert!(decode(&invalid, &DecodeRequest::new("x.ktx2")).is_err());
        let mut invalid = include_bytes!("../../../tests/fixtures/raster/rgba8-rd-3.ktx2").to_vec();
        let end = invalid.len() - 1;
        invalid[end] ^= 1;
        assert!(decode(&invalid, &DecodeRequest::new("x.ktx2")).is_err());
    }
    #[test]
    fn implicit_base_level_is_valid_for_uncompressed_storage() {
        let mut bytes = include_bytes!("../../../tests/fixtures/raster/rgba8-rd-0.ktx2").to_vec();
        bytes[40..44].copy_from_slice(&0u32.to_le_bytes());
        let frame = decode(&bytes, &DecodeRequest::new("implicit.ktx2")).unwrap();
        assert_eq!((frame.width(), frame.height()), (3, 2));
    }
    #[test]
    fn srgb_alpha_reaches_linear_display() {
        let frame = decode(
            include_bytes!("../../../tests/fixtures/raster/rgba8-ru-3.ktx2"),
            &DecodeRequest::new("x.ktx2"),
        )
        .unwrap();
        let RasterPixels::Rgba8(p) = frame.pixels() else {
            panic!()
        };
        assert_eq!(p[7], 128);
    }
}
