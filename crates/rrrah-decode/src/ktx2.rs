//! KTX2 ordinary RGB/RGBA surfaces, retaining native precision and transfer.
use crate::{
    DecodeRequest,
    raster::{MAX_RASTER_BYTES, RasterDecodeError},
};
use rrrah_core::{DecodedRaster, RasterColorSpace, RasterPixels};
use std::{io::Read, ops::Range, sync::Arc};
fn bad(s: &'static str) -> RasterDecodeError {
    RasterDecodeError::InvalidKtx2(s)
}
pub(crate) fn has_magic(b: &[u8]) -> bool {
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
fn metadata(b: &[u8]) -> Result<(bool, bool), RasterDecodeError> {
    let (mut at, mut flipx, mut flipy) = (0, false, false);
    let mut keys = std::collections::HashSet::new();
    while at < b.len() {
        let n = word(b, at)? as usize;
        at += 4;
        let end = at.checked_add(n).ok_or_else(|| bad("metadata overflow"))?;
        let entry = b.get(at..end).ok_or_else(|| bad("truncated metadata"))?;
        let split = entry
            .iter()
            .position(|v| *v == 0)
            .ok_or_else(|| bad("unterminated metadata key"))?;
        let key = std::str::from_utf8(&entry[..split]).map_err(|_| bad("metadata UTF8"))?;
        if key.is_empty() || !keys.insert(key) {
            return Err(bad("empty or duplicate metadata key"));
        }
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
    Ok((flipx, flipy))
}
pub(crate) fn decode(b: &[u8], request: &DecodeRequest) -> Result<DecodedRaster, RasterDecodeError> {
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
    if u64::from(width) * u64::from(height) * ((f.channels + 4) * f.size) as u64 > MAX_RASTER_BYTES {
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
    let kvd = range(b, word(b, 56)?.into(), word(b, 60)?.into())?;
    if kvd.len() > 4 * 1024 * 1024 || (!kvd.is_empty() && kvd.start % 4 != 0) {
        return Err(bad("metadata alignment or limit"));
    }
    if long(b, 64)? != 0 || long(b, 72)? != 0 {
        return Err(bad("unexpected supercompression global data"));
    }
    let (flipx, flipy) = metadata(&b[kvd.clone()])?;
    let mut ranges = vec![0..index_end, dfd];
    if !kvd.is_empty() {
        ranges.push(kvd);
    }
    let mut data_ranges = Vec::new();
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
                _ => texel,
            };
            if r.start % alignment != 0 {
                return Err(bad("mip alignment"));
            }
        }
        ranges.push(r.clone());
        data_ranges.push((r, expected as usize));
    }
    ranges.sort_by_key(|r| r.start);
    for pair in ranges.windows(2) {
        if pair[0].end > pair[1].start || b[pair[0].end..pair[1].start].iter().any(|v| *v != 0) {
            return Err(bad("overlapping ranges or nonzero padding"));
        }
    }
    if ranges.last().unwrap().end != b.len() {
        return Err(bad("trailing data"));
    }
    let mut selected = None;
    for (level, (r, expected)) in data_ranges.into_iter().enumerate() {
        request.check_cancelled()?;
        if level != request.image_index {
            continue;
        }
        let data = if compression == 0 {
            b[r].to_vec()
        } else {
            let source = &b[r];
            let mut decoder = flate2::read::ZlibDecoder::new(source);
            let mut out = Vec::with_capacity(expected);
            let mut chunk = [0u8; 65536];
            loop {
                request.check_cancelled()?;
                let n = decoder
                    .read(&mut chunk)
                    .map_err(|_| bad("DEFLATE stream invalid"))?;
                if n == 0 {
                    break;
                }
                if out.len() + n > expected {
                    return Err(bad("DEFLATE exceeds declared size"));
                }
                out.extend_from_slice(&chunk[..n]);
            }
            if out.len() != expected || decoder.total_in() != source.len() as u64 {
                return Err(bad("DEFLATE size or trailing stream data"));
            }
            out
        };
        if level == request.image_index {
            selected = Some(data);
        }
    }
    let primary = selected.ok_or(crate::DecodeError::UnsupportedImageIndex {
        index: request.image_index,
    })?;
    let width = (width >> request.image_index).max(1);
    let height = (height >> request.image_index).max(1);
    let count = width as usize * height as usize;
    let pixel_at = |index: usize| {
        let (x, y) = (index % width as usize, index / width as usize);
        let sx = if flipx { width as usize - 1 - x } else { x };
        let sy = if flipy { height as usize - 1 - y } else { y };
        (sy * width as usize + sx) * f.channels * f.size
    };
    let pixels = match f.size {
        1 => {
            let mut out = Vec::with_capacity(count * 4);
            for index in 0..count {
                if index % width as usize == 0 {
                    request.check_cancelled()?;
                }
                let at = pixel_at(index);
                let p = &primary[at..at + f.channels];
                out.extend([
                    p[if f.bgr { 2 } else { 0 }],
                    p[1],
                    p[if f.bgr { 0 } else { 2 }],
                    if f.channels == 4 { p[3] } else { 255 },
                ]);
            }
            RasterPixels::Rgba8(Arc::new(out).into())
        }
        2 => {
            let mut out = Vec::with_capacity(count * 4);
            for index in 0..count {
                if index % width as usize == 0 {
                    request.check_cancelled()?;
                }
                let at = pixel_at(index);
                for c in 0..4 {
                    out.push(if c == 3 && f.channels == 3 {
                        65535
                    } else {
                        u16::from_le_bytes(primary[at + c * 2..at + c * 2 + 2].try_into().unwrap())
                    });
                }
            }
            RasterPixels::Rgba16(Arc::new(out).into())
        }
        _ => {
            let mut out = Vec::with_capacity(count * 4);
            for index in 0..count {
                if index % width as usize == 0 {
                    request.check_cancelled()?;
                }
                let at = pixel_at(index);
                for c in 0..4 {
                    out.push(if c == 3 && f.channels == 3 {
                        1.0
                    } else {
                        f32::from_le_bytes(primary[at + c * 4..at + c * 4 + 4].try_into().unwrap())
                    });
                }
            }
            RasterPixels::Rgba32Float(Arc::new(out).into())
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
    fn all_stored_mips_match_native_sample_contracts() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/raster");
        let manifest = std::fs::read_to_string(root.join("ktx2-mip-manifest.tsv")).unwrap();
        for line in manifest.lines().filter(|s| !s.starts_with('#') && !s.is_empty()) {
            let c: Vec<_> = line.split('\t').collect();
            let mut request = DecodeRequest::new(root.join(c[0]));
            request.image_index = c[1].parse().unwrap();
            let image = crate::decode_raster(&request).unwrap();
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
            assert_eq!(
                actual,
                std::fs::read(root.join(c[6])).unwrap(),
                "{} mip {}",
                c[0],
                c[1]
            );
            let prepared = crate::prepare_raster_for_display(&image).unwrap();
            assert_eq!(
                (prepared.image_index(), prepared.image_count()),
                (request.image_index, 4)
            );
            request.image_index = 4;
            assert!(matches!(
                crate::decode_raster(&request),
                Err(RasterDecodeError::Source(
                    crate::DecodeError::UnsupportedImageIndex { index: 4 }
                ))
            ));
        }
    }
    #[test]
    fn selected_mip_keeps_pixels_transfer_and_selection() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/raster");
        let mut request = DecodeRequest::new(root.join("mips.ktx2"));
        request.image_index = 1;
        let image = crate::decode_raster(&request).unwrap();
        assert_eq!((image.width(), image.height()), (1, 1));
        assert_eq!((image.image_index(), image.image_count()), (1, 2));
        assert_eq!(image.color_space(), &RasterColorSpace::Srgb);
        let RasterPixels::Rgba8(v) = image.pixels() else {
            panic!()
        };
        assert_eq!(&v[..], &[20, 40, 60, 255]);
        let prepared = crate::prepare_raster_for_display(&image).unwrap();
        assert_eq!((prepared.image_index(), prepared.image_count()), (1, 2));
        request.image_index = usize::MAX;
        assert!(matches!(
            crate::decode_raster(&request),
            Err(RasterDecodeError::Source(
                crate::DecodeError::UnsupportedImageIndex { .. }
            ))
        ));
        request.image_index = 0;
        let bytes = std::fs::read(&request.path).unwrap();
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
        let prepared = crate::prepare_raster_for_display(&frame).unwrap();
        let RasterPixels::Rgba32Float(p) = prepared.pixels() else {
            panic!()
        };
        assert!((p[7] - 128.0 / 255.0).abs() < 1e-7);
    }
}
