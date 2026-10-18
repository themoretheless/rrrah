//! KTX1 selected 2D mip with explicit storage, orientation and transfer.
use crate::{
    DecodeRequest,
    raster::{MAX_RASTER_BYTES, RasterDecodeError},
};
use rrrah_core::{DecodedRaster, RasterColorSpace, RasterPixels};
use std::sync::Arc;
const MAGIC: &[u8] = b"\xabKTX 11\xbb\r\n\x1a\n";
fn bad(s: &'static str) -> RasterDecodeError {
    RasterDecodeError::InvalidKtx(s)
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
pub fn has_magic(bytes: &[u8]) -> bool {
    bytes.starts_with(MAGIC)
}
struct Input<'a> {
    b: &'a [u8],
    at: usize,
    be: bool,
}
impl<'a> Input<'a> {
    fn take(&mut self, n: usize) -> Result<&'a [u8], RasterDecodeError> {
        let end = self.at.checked_add(n).ok_or_else(|| bad("size overflow"))?;
        let b = self
            .b
            .get(self.at..end)
            .ok_or_else(|| bad("truncated header, metadata or mip"))?;
        self.at = end;
        Ok(b)
    }
    fn word(&mut self) -> Result<u32, RasterDecodeError> {
        let be = self.be;
        let b = self.take(4)?.try_into().unwrap();
        Ok(if be {
            u32::from_be_bytes(b)
        } else {
            u32::from_le_bytes(b)
        })
    }
}
pub fn decode(bytes: &[u8], request: &DecodeRequest) -> Result<DecodedRaster, RasterDecodeError> {
    request.check_cancelled()?;
    if !has_magic(bytes) {
        return Err(bad("signature"));
    }
    let be = match bytes.get(12..16) {
        Some([4, 3, 2, 1]) => true,
        Some([1, 2, 3, 4]) => false,
        _ => return Err(bad("endianness marker")),
    };
    let mut r = Input { b: bytes, at: 16, be };
    let ty = r.word()?;
    let size = r.word()? as usize;
    let format = r.word()?;
    let internal = r.word()?;
    let base = r.word()?;
    let width = r.word()? as usize;
    let height = r.word()? as usize;
    if r.word()? != 0 || r.word()? != 0 || r.word()? != 1 {
        return Err(bad("volume, array or cubemap selection pending"));
    }
    let levels = r.word()?.max(1) as usize;
    let metadata = r.word()? as usize;
    if width == 0
        || height == 0
        || width > 65536
        || height > 65536
        || levels > usize::BITS as usize
        || levels > (usize::BITS - width.max(height).leading_zeros()) as usize
    {
        return Err(bad("dimensions or mip count"));
    }
    if metadata > 4 * 1024 * 1024 {
        return Err(bad("metadata too large"));
    }
    let mut meta = Input {
        b: r.take(metadata)?,
        at: 0,
        be,
    };
    let (mut flipx, mut flipy) = (false, false);
    let metadata_bytes = meta.b;
    let mut key_count = 0usize;
    while meta.at < meta.b.len() {
        request.check_cancelled()?;
        let n = meta.word()? as usize;
        let kv = meta.take(n)?;
        let split = kv
            .iter()
            .position(|v| *v == 0)
            .ok_or_else(|| bad("unterminated metadata key"))?;
        let key = std::str::from_utf8(&kv[..split]).map_err(|_| bad("metadata key UTF8"))?;
        key_count += 1;
        if key == "KTXorientation" {
            let value = &kv[split + 1..];
            let value = value.strip_suffix(&[0]).unwrap_or(value);
            match value {
                b"S=r,T=d" => (),
                b"S=r,T=u" => flipy = true,
                b"S=l,T=d" => flipx = true,
                b"S=l,T=u" => {
                    flipx = true;
                    flipy = true;
                }
                _ => return Err(bad("orientation unsupported")),
            }
        }
        if key == "KTXswizzle" || key == "KTXpremultipliedAlpha" {
            return Err(bad("swizzle or associated alpha pending"));
        }
        if meta.take((4 - n % 4) % 4)?.iter().any(|v| *v != 0) {
            return Err(bad("metadata padding"));
        }
    }
    let channels = match base {
        0x1903 => 1usize,
        0x8227 => 2,
        0x1907 => 3usize,
        0x1908 => 4,
        _ => return Err(bad("base color format unsupported")),
    };
    let compressed = ty == 0;
    let (block, codec, srgb) = if compressed {
        if format != 0 || size != 1 {
            return Err(bad("compressed header"));
        }
        match internal {
            0x9270 => (8, 5, false),
            0x9271 => (8, 6, false),
            0x9272 => (16, 7, false),
            0x9273 => (16, 8, false),
            0x8d64 => (8, 4, false),
            0x83f0 => (8, 0, false),
            0x83f1 => (8, 1, false),
            0x83f2 => (16, 2, false),
            0x83f3 => (16, 3, false),
            0x8c4c => (8, 0, true),
            0x8c4d => (8, 1, true),
            0x8c4e => (16, 2, true),
            0x8c4f => (16, 3, true),
            _ => return Err(bad("compressed storage pending")),
        }
    } else {
        if format != base {
            return Err(bad("format and base mismatch"));
        }
        let valid = match (ty, size, channels, internal) {
            (0x140b, 2, 1, 0x822d)
            | (0x140b, 2, 2, 0x822f)
            | (0x140b, 2, 3, 0x881b)
            | (0x140b, 2, 4, 0x881a)
            | (0x1401, 1, 1, 0x8229)
            | (0x1401, 1, 2, 0x822b)
            | (0x1403, 2, 1, 0x822a)
            | (0x1403, 2, 2, 0x822c)
            | (0x1406, 4, 1, 0x822e)
            | (0x1406, 4, 2, 0x8230)
            | (0x1401, 1, 3, 0x8051)
            | (0x1401, 1, 4, 0x8058)
            | (0x1403, 2, 3, 0x8054)
            | (0x1403, 2, 4, 0x805b)
            | (0x1406, 4, 3, 0x8815)
            | (0x1406, 4, 4, 0x8814) => Some(false),
            (0x1401, 1, 3, 0x8c41) | (0x1401, 1, 4, 0x8c43) => Some(true),
            _ => None,
        };
        (
            0,
            0,
            valid.ok_or_else(|| bad("sample type or internal format unsupported"))?,
        )
    };
    if compressed
        && channels
            != match codec {
                0 | 4 => 3,
                5 | 6 => 1,
                7 | 8 => 2,
                _ => 4,
            }
    {
        return Err(bad("compressed base format mismatch"));
    }
    let output_size = if (compressed && codec >= 5) || ty == 0x140b {
        4
    } else {
        size
    };
    if (width as u64) * (height as u64) * 4 * (output_size as u64) > MAX_RASTER_BYTES {
        return Err(RasterDecodeError::OutputTooLarge);
    }
    let mut selected = None;
    for level in 0..levels {
        request.check_cancelled()?;
        let w = (width >> level).max(1);
        let h = (height >> level).max(1);
        let expected = if compressed {
            w.div_ceil(4) * h.div_ceil(4) * block
        } else {
            (w * channels * size).div_ceil(4) * 4 * h
        };
        let n = r.word()? as usize;
        if n != expected {
            return Err(bad("mip byte size mismatch"));
        }
        let data = r.take(n)?;
        if level == request.image_index {
            selected = Some((data, w, h));
        }
        if r.take((4 - n % 4) % 4)?.iter().any(|v| *v != 0) {
            return Err(bad("mip padding"));
        }
    }
    if r.at != bytes.len() {
        return Err(bad("trailing mip data"));
    }
    // Borrow keys from the source; only their offsets need temporary storage.
    // Sorting bounds duplicate detection without a growing hash table.
    if key_count > 1 {
        let mut keys = buffer(key_count, (0usize, 0usize), request)?;
        let entries = keys.get_mut().unwrap();
        let mut meta = Input {
            b: metadata_bytes,
            at: 0,
            be,
        };
        for entry in entries.iter_mut() {
            request.check_cancelled()?;
            let n = meta.word()? as usize;
            let start = meta.at;
            let kv = meta.take(n)?;
            let len = kv.iter().position(|v| *v == 0).unwrap();
            *entry = (start, len);
            meta.take((4 - n % 4) % 4)?;
        }
        entries.sort_unstable_by(|a, b| metadata_bytes[a.0..a.0 + a.1].cmp(&metadata_bytes[b.0..b.0 + b.1]));
        request.check_cancelled()?;
        for pair in entries.windows(2) {
            let (a, b) = (pair[0], pair[1]);
            if metadata_bytes[a.0..a.0 + a.1] == metadata_bytes[b.0..b.0 + b.1] {
                return Err(bad("duplicate metadata key"));
            }
        }
    }
    let (data, width, height) = selected.ok_or(crate::DecodeError::UnsupportedImageIndex {
        index: request.image_index,
    })?;
    let color = if channels < 3 {
        request.qualify_linear_color(RasterColorSpace::LinearRgbUnspecified)
    } else if srgb {
        RasterColorSpace::Srgb
    } else {
        RasterColorSpace::LinearSrgb
    };
    let destination = |x: usize, y: usize| {
        ((if flipy { height - 1 - y } else { y }) * width + if flipx { width - 1 - x } else { x }) * 4
    };
    if compressed && codec == 4 {
        for block in data.chunks_exact(8) {
            request.check_cancelled()?;
            crate::etc1::validate(block).map_err(bad)?;
        }
    }
    let pixels = if compressed && codec >= 5 {
        let signed = matches!(codec, 6 | 8);
        let dual = matches!(codec, 7 | 8);
        let denominator = if signed { 1023.0 } else { 2047.0 };
        let mut pixels = buffer(width * height * 4, 0f32, request)?;
        let out = pixels.get_mut().unwrap();
        for (i, b) in data.chunks_exact(block).enumerate() {
            request.check_cancelled()?;
            let red = crate::eac::decode(b[..8].try_into().unwrap(), signed);
            let green = if dual {
                crate::eac::decode(b[8..].try_into().unwrap(), signed)
            } else {
                [0; 16]
            };
            let left = i % width.div_ceil(4) * 4;
            let top = i / width.div_ceil(4) * 4;
            for y in 0..4 {
                for x in 0..4 {
                    if left + x < width && top + y < height {
                        let at = destination(left + x, top + y);
                        let sample = y * 4 + x;
                        out[at..at + 4].copy_from_slice(&[
                            f32::from(red[sample]) / denominator,
                            f32::from(green[sample]) / denominator,
                            0.0,
                            1.0,
                        ]);
                    }
                }
            }
        }
        RasterPixels::Rgba32Float(pixels)
    } else if compressed {
        let mut pixels = buffer(width * height * 4, 0u8, request)?;
        let out = pixels.get_mut().unwrap();
        for (i, b) in data.chunks_exact(block).enumerate() {
            request.check_cancelled()?;
            let p = match codec {
                0 | 1 => {
                    let mut colors = crate::dds::block(b, 1);
                    if codec == 0 {
                        for color in &mut colors {
                            color[3] = 255;
                        }
                    }
                    colors
                }
                2 | 3 => crate::dds::block(b, codec),
                4 => {
                    let mut colors = [0u32; 16];
                    crate::etc1::decode(b, &mut colors);
                    colors.map(|color| {
                        let [blue, green, red, alpha] = color.to_le_bytes();
                        [red, green, blue, alpha]
                    })
                }
                _ => unreachable!(),
            };
            let left = (i % width.div_ceil(4)) * 4;
            let top = (i / width.div_ceil(4)) * 4;
            for y in 0..4 {
                for x in 0..4 {
                    if left + x < width && top + y < height {
                        let at = destination(left + x, top + y);
                        out[at..at + 4].copy_from_slice(&p[y * 4 + x]);
                    }
                }
            }
        }
        RasterPixels::Rgba8(pixels)
    } else {
        let stride = (width * channels * size).div_ceil(4) * 4;
        macro_rules! unpack {
            ($t:ty,$read:expr,$opaque:expr,$variant:ident) => {{
                let mut pixels = buffer::<$t>(width * height * 4, $opaque, request)?;
                let out = pixels.get_mut().unwrap();
                for y in 0..height {
                    request.check_cancelled()?;
                    for x in 0..width {
                        let at = destination(x, y);
                        for c in 0..channels {
                            let src = y * stride + (x * channels + c) * size;
                            out[at + c] = $read(src);
                        }
                        if channels < 3 {
                            out[at + channels..at + 3].fill(Default::default());
                        }
                    }
                }
                RasterPixels::$variant(pixels)
            }};
        }
        match ty {
            0x1401 => unpack!(u8, |i| data[i], 255, Rgba8),
            0x1403 => unpack!(
                u16,
                |i| {
                    let b = data[i..i + 2].try_into().unwrap();
                    if be {
                        u16::from_be_bytes(b)
                    } else {
                        u16::from_le_bytes(b)
                    }
                },
                65535u16,
                Rgba16
            ),
            0x140b => unpack!(
                f32,
                |i| {
                    let b = data[i..i + 2].try_into().unwrap();
                    rrrah_scientific::sample::half_to_f32(if be {
                        u16::from_be_bytes(b)
                    } else {
                        u16::from_le_bytes(b)
                    })
                },
                1.0f32,
                Rgba32Float
            ),
            0x1406 => unpack!(
                f32,
                |i| {
                    let b = data[i..i + 4].try_into().unwrap();
                    if be {
                        f32::from_be_bytes(b)
                    } else {
                        f32::from_le_bytes(b)
                    }
                },
                1.0f32,
                Rgba32Float
            ),
            _ => unreachable!(),
        }
    };
    Ok(DecodedRaster::new(width as u32, height as u32, pixels, color)?
        .with_image_selection(request.image_index, levels)?)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn metadata_source(keys: &[String]) -> Vec<u8> {
        let original = include_bytes!("../../../tests/fixtures/raster/rgba8-0-rd.ktx");
        assert_eq!(&original[12..16], &[1, 2, 3, 4]);
        let old = u32::from_le_bytes(original[60..64].try_into().unwrap()) as usize;
        let mut records = Vec::new();
        for key in keys {
            let n = key.len() + 2;
            records.extend((n as u32).to_le_bytes());
            records.extend(key.as_bytes());
            records.extend([0, 42]);
            records.resize(records.len().next_multiple_of(4), 0);
        }
        let mut source = original[..64].to_vec();
        source[60..64].copy_from_slice(&(records.len() as u32).to_le_bytes());
        source.extend(records);
        source.extend(&original[64 + old..]);
        source
    }

    #[test]
    fn metadata_offsets_are_admitted_released_and_duplicates_refused() {
        let keys: Vec<_> = (0..128).rev().map(|i| format!("unknown-{i:03}")).collect();
        let source = metadata_source(&keys);
        let scratch = (keys.len() * std::mem::size_of::<(usize, usize)>()) as u64;
        let budget = rrrah_core::MemoryBudget::new(scratch);
        let mut request = DecodeRequest::new("metadata.ktx");
        request.memory_budget = Some(budget.clone());
        let frame = decode(&source, &request).unwrap();
        assert_eq!((budget.used(), budget.peak()), (24, scratch));
        drop(frame);
        assert_eq!(budget.used(), 0);
        let short = rrrah_core::MemoryBudget::new(scratch - 1);
        request.memory_budget = Some(short.clone());
        assert!(matches!(
            decode(&source, &request),
            Err(RasterDecodeError::Source(crate::DecodeError::Memory(_)))
        ));
        assert_eq!(short.used(), 0);
        let mut duplicates = keys;
        duplicates.push(duplicates[0].clone());
        let duplicate_budget = rrrah_core::MemoryBudget::new(scratch + 16);
        request.memory_budget = Some(duplicate_budget.clone());
        assert!(matches!(
            decode(&metadata_source(&duplicates), &request),
            Err(RasterDecodeError::InvalidKtx("duplicate metadata key"))
        ));
        assert_eq!(duplicate_budget.used(), 0);
        // With no keys or a single key there is no index table to allocate.
        for keys in [vec![], vec!["unknown".to_string()]] {
            let small = rrrah_core::MemoryBudget::new(24);
            request.memory_budget = Some(small.clone());
            drop(decode(&metadata_source(&keys), &request).unwrap());
            assert_eq!((small.used(), small.peak()), (0, 24));
        }
    }

    #[test]
    fn bc2_bc3_reversed_endpoints_keep_four_color_palette() {
        for internal in [0x83f2u32, 0x83f3] {
            let mut source = MAGIC.to_vec();
            for word in [0x04030201, 0, 1, 0, internal, 0x1908, 4, 4, 0, 0, 1, 1, 0] {
                source.extend(word.to_le_bytes());
            }
            source.extend(16u32.to_le_bytes());
            let alpha = if internal == 0x83f2 {
                [255u8; 8]
            } else {
                [255, 0, 0, 0, 0, 0, 0, 0]
            };
            source.extend(alpha);
            source.extend([0, 0, 255, 255, 0xee, 0xee, 0xee, 0xee]);
            let budget = rrrah_core::MemoryBudget::new(64);
            let mut request = DecodeRequest::new("reversed-bc.ktx");
            request.memory_budget = Some(budget.clone());
            let frame = decode(&source, &request).unwrap();
            let RasterPixels::Rgba8(pixels) = frame.pixels() else {
                panic!()
            };
            for (i, actual) in pixels.as_chunks::<4>().0.iter().enumerate() {
                let color = if i % 2 == 0 { 85 } else { 170 };
                assert_eq!(*actual, [color, color, color, 255]);
            }
            drop(frame);
            assert_eq!(budget.used(), 0);
        }
    }
    #[test]
    fn invalid_etc1_endpoint_refuses_before_zero_output_budget() {
        let mut bytes = include_bytes!("../../../tests/fixtures/raster/ktx-etc1-le-rd.ktx").to_vec();
        let metadata = u32::from_le_bytes(bytes[60..64].try_into().unwrap()) as usize;
        let first_block = 64 + metadata + 4;
        bytes[first_block] = 4; // Five-bit base zero plus differential -4.
        bytes[first_block + 3] |= 2;
        let budget = rrrah_core::MemoryBudget::new(0);
        let mut request = DecodeRequest::new("invalid-etc1.ktx");
        request.memory_budget = Some(budget.clone());
        assert!(matches!(
            decode(&bytes, &request),
            Err(RasterDecodeError::InvalidKtx("invalid ETC1 differential color"))
        ));
        assert_eq!((budget.used(), budget.peak()), (0, 0));
    }
    #[test]
    fn every_binary16_pattern_matches_independent_numpy_expansion_in_both_endians() {
        let oracle = include_bytes!("../../../tests/fixtures/raster/mrc-half-all.r32f");
        for be in [false, true] {
            let word = |value: u32| {
                if be {
                    value.to_be_bytes()
                } else {
                    value.to_le_bytes()
                }
            };
            let mut source = MAGIC.to_vec();
            for value in [
                0x04030201, 0x140b, 2, 0x1903, 0x822d, 0x1903, 256, 256, 0, 0, 1, 1, 0, 131072,
            ] {
                source.extend(word(value));
            }
            for bits in 0..=u16::MAX {
                source.extend(if be {
                    bits.to_be_bytes()
                } else {
                    bits.to_le_bytes()
                });
            }
            let root = rrrah_core::MemoryBudget::new(65536 * 16);
            let mut request = DecodeRequest::new("all-half.ktx");
            request.assume_untagged_linear_srgb = true;
            request.memory_budget = Some(root.clone());
            let decoded = decode(&source, &request).unwrap();
            let RasterPixels::Rgba32Float(values) = decoded.pixels() else {
                panic!()
            };
            for (index, rgba) in values.chunks_exact(4).enumerate() {
                let expected = u32::from_le_bytes(oracle[index * 4..index * 4 + 4].try_into().unwrap());
                assert_eq!(rgba[0].to_bits(), expected, "half{index:04x} be={be}");
                assert_eq!(
                    [rgba[1].to_bits(), rgba[2].to_bits(), rgba[3].to_bits()],
                    [0, 0, 1f32.to_bits()]
                );
            }
            // Native bits include nonfinite values, which must never be admitted as display colors.
            assert!(matches!(
                decoded.to_linear_srgb(),
                Err(rrrah_core::RasterError::NonFiniteSample)
            ));
            assert_eq!(root.used(), 65536 * 16);
            drop(decoded);
            assert_eq!((root.used(), root.peak()), (0, 65536 * 16));
        }
    }
    #[test]
    fn eac_requires_explicit_linear_color_and_matching_base_format() {
        let source = include_bytes!("../../../tests/fixtures/raster/ktx-eac-9273-le-00.ktx");
        let native = decode(source, &DecodeRequest::new("data.ktx")).unwrap();
        assert_eq!(native.color_space(), &RasterColorSpace::LinearRgbUnspecified);
        assert!(native.to_linear_srgb().is_err());
        let mut request = DecodeRequest::new("data.ktx");
        request.assume_untagged_srgb = true;
        assert_eq!(
            decode(source, &request).unwrap().color_space(),
            &RasterColorSpace::LinearRgbUnspecified
        );
        request.assume_untagged_linear_srgb = true;
        let qualified = decode(source, &request).unwrap();
        assert_eq!(qualified.color_space(), &RasterColorSpace::LinearSrgb);
        let (RasterPixels::Rgba32Float(a), RasterPixels::Rgba32Float(b)) =
            (native.pixels(), qualified.pixels())
        else {
            panic!()
        };
        assert_eq!(a.as_slice(), b.as_slice());
        let mut invalid = source.to_vec();
        invalid[32..36].copy_from_slice(&0x1908u32.to_le_bytes());
        let zero = rrrah_core::MemoryBudget::new(0);
        request.memory_budget = Some(zero.clone());
        assert!(matches!(
            decode(&invalid, &request),
            Err(RasterDecodeError::InvalidKtx("compressed base format mismatch"))
        ));
        assert_eq!((zero.used(), zero.peak()), (0, 0));
    }
    #[test]
    fn selected_mips_preserve_native_precision_orientation_and_transfer() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/raster");
        let manifest = std::fs::read_to_string(root.join("ktx-mip-manifest.tsv")).unwrap()
            + &std::fs::read_to_string(root.join("ktx-etc1-manifest.tsv")).unwrap()
            + &std::fs::read_to_string(root.join("ktx-eac-manifest.tsv")).unwrap()
            + &std::fs::read_to_string(root.join("ktx-rg-manifest.tsv")).unwrap()
            + &std::fs::read_to_string(root.join("ktx-half-manifest.tsv")).unwrap();
        let mut cases = 0;
        for line in manifest.lines().filter(|s| !s.starts_with('#') && !s.is_empty()) {
            let c: Vec<_> = line.split('\t').collect();
            let mut request = DecodeRequest::new(root.join(c[0]));
            request.image_index = c[1].parse().unwrap();
            let bytes = std::fs::read(&request.path).unwrap();
            let oracle = std::fs::read(root.join(c[6])).unwrap();
            let size = oracle.len() as u64;
            let budget = rrrah_core::MemoryBudget::new(size);
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
                &if c[5] == "linear-unspecified" {
                    RasterColorSpace::LinearRgbUnspecified
                } else if c[5] == "srgb" {
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
            assert_eq!((budget.used(), budget.peak()), (size, size));
            let shared = image.clone();
            drop(image);
            assert_eq!(budget.used(), size);
            drop(shared);
            assert_eq!(budget.used(), 0);
            let short = rrrah_core::MemoryBudget::new(size - 1);
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
        assert_eq!(cases, 540);
    }
    #[test]
    fn every_chain_prefix_is_rejected_even_for_a_complete_first_mip() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/raster");
        let manifest = std::fs::read_to_string(root.join("ktx-mip-manifest.tsv")).unwrap()
            + &std::fs::read_to_string(root.join("ktx-etc1-manifest.tsv")).unwrap()
            + &std::fs::read_to_string(root.join("ktx-eac-manifest.tsv")).unwrap()
            + &std::fs::read_to_string(root.join("ktx-rg-manifest.tsv")).unwrap()
            + &std::fs::read_to_string(root.join("ktx-half-manifest.tsv")).unwrap();
        let mut names = std::collections::BTreeSet::new();
        for line in manifest.lines().filter(|s| !s.starts_with('#') && !s.is_empty()) {
            names.insert(line.split('\t').next().unwrap());
        }
        for name in names {
            let bytes = std::fs::read(root.join(name)).unwrap();
            let mut request = DecodeRequest::new(name);
            let budget = rrrah_core::MemoryBudget::new(0);
            request.memory_budget = Some(budget.clone());
            for end in 0..bytes.len() {
                assert!(
                    matches!(
                        decode(&bytes[..end], &request),
                        Err(RasterDecodeError::InvalidKtx(_))
                    ),
                    "{name}: prefix {end}"
                );
                assert_eq!((budget.used(), budget.peak()), (0, 0));
            }
            assert!(matches!(
                decode(&bytes, &request),
                Err(RasterDecodeError::Source(crate::DecodeError::Memory(_)))
            ));
        }
    }
    #[test]
    fn sixteen_bit_and_float_samples_survive_both_byte_orders() {
        for bytes in [
            include_bytes!("../../../tests/fixtures/raster/precision-16-0.ktx").as_slice(),
            include_bytes!("../../../tests/fixtures/raster/precision-16-1.ktx").as_slice(),
        ] {
            let f = decode(bytes, &DecodeRequest::new("x.ktx")).unwrap();
            let RasterPixels::Rgba16(p) = f.pixels() else {
                panic!()
            };
            assert_eq!(p.as_slice(), [32768, 1, 65534, 65535, 32769, 2, 65535, 65535]);
        }
        for bytes in [
            include_bytes!("../../../tests/fixtures/raster/precision-32-0.ktx").as_slice(),
            include_bytes!("../../../tests/fixtures/raster/precision-32-1.ktx").as_slice(),
        ] {
            let f = decode(bytes, &DecodeRequest::new("x.ktx")).unwrap();
            let RasterPixels::Rgba32Float(p) = f.pixels() else {
                panic!()
            };
            assert_eq!(p.as_slice(), [0.5, 0.1, 0.3, 1.0, 2.0, 0.2, 0.4, 1.0]);
        }
    }
    #[test]
    fn malformed_mip_size_array_and_trailing_data_are_rejected() {
        let source = include_bytes!("../../../tests/fixtures/raster/rgb8-padded.ktx");
        let mut array = source.to_vec();
        array[48..52].copy_from_slice(&1u32.to_le_bytes());
        assert!(decode(&array, &DecodeRequest::new("x.ktx")).is_err());
        let mut levels = source.to_vec();
        levels[56..60].copy_from_slice(&2u32.to_le_bytes());
        assert!(decode(&levels, &DecodeRequest::new("x.ktx")).is_err());
        let mut bytes = source.to_vec();
        bytes.push(0);
        assert!(decode(&bytes, &DecodeRequest::new("x.ktx")).is_err());
        let mut bytes = source.to_vec();
        let metadata = u32::from_le_bytes(bytes[60..64].try_into().unwrap()) as usize;
        bytes[64 + metadata..68 + metadata].copy_from_slice(&4u32.to_le_bytes());
        assert!(decode(&bytes, &DecodeRequest::new("x.ktx")).is_err());
    }
    #[test]
    fn srgb_and_linear_transfers_remain_distinct() {
        let source = include_bytes!("../../../tests/fixtures/raster/rgba8-0-rd.ktx");
        let f = decode(source, &DecodeRequest::new("x.ktx")).unwrap();
        assert!(matches!(f.color_space(), RasterColorSpace::Srgb));
        let mut bytes = source.to_vec();
        bytes[28..32].copy_from_slice(&0x8058u32.to_le_bytes());
        let f = decode(&bytes, &DecodeRequest::new("x.ktx")).unwrap();
        assert!(matches!(f.color_space(), RasterColorSpace::LinearSrgb));
    }
}
