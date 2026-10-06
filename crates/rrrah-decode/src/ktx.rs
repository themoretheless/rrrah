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
pub(crate) fn has_magic(bytes: &[u8]) -> bool {
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
pub(crate) fn decode(bytes: &[u8], request: &DecodeRequest) -> Result<DecodedRaster, RasterDecodeError> {
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
    let mut keys = std::collections::HashSet::new();
    while meta.at < meta.b.len() {
        let n = meta.word()? as usize;
        let kv = meta.take(n)?;
        let split = kv
            .iter()
            .position(|v| *v == 0)
            .ok_or_else(|| bad("unterminated metadata key"))?;
        let key = std::str::from_utf8(&kv[..split]).map_err(|_| bad("metadata key UTF8"))?;
        if !keys.insert(key) {
            return Err(bad("duplicate metadata key"));
        }
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
            (0x1401, 1, 3, 0x8051)
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
    if compressed && channels != if codec == 0 { 3 } else { 4 } {
        return Err(bad("compressed base format mismatch"));
    }
    if (width as u64) * (height as u64) * 4 * (size as u64) > MAX_RASTER_BYTES {
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
    let (data, width, height) = selected.ok_or(crate::DecodeError::UnsupportedImageIndex {
        index: request.image_index,
    })?;
    let color = if srgb {
        RasterColorSpace::Srgb
    } else {
        RasterColorSpace::LinearSrgb
    };
    let destination = |x: usize, y: usize| {
        ((if flipy { height - 1 - y } else { y }) * width + if flipx { width - 1 - x } else { x }) * 4
    };
    let pixels = if compressed {
        let mut out = vec![0u8; width * height * 4];
        for (i, b) in data.chunks_exact(block).enumerate() {
            request.check_cancelled()?;
            let mut p = [0u32; 16];
            match codec {
                0 => texture2ddecoder::decode_bc1_block(b, &mut p),
                1 => texture2ddecoder::decode_bc1a_block(b, &mut p),
                2 => texture2ddecoder::decode_bc2_block(b, &mut p),
                3 => texture2ddecoder::decode_bc3_block(b, &mut p),
                _ => unreachable!(),
            };
            let left = (i % width.div_ceil(4)) * 4;
            let top = (i / width.div_ceil(4)) * 4;
            for y in 0..4 {
                for x in 0..4 {
                    if left + x < width && top + y < height {
                        let [b, g, r, a] = p[y * 4 + x].to_le_bytes();
                        let at = destination(left + x, top + y);
                        out[at..at + 4].copy_from_slice(&[r, g, b, a]);
                    }
                }
            }
        }
        RasterPixels::Rgba8(Arc::new(out).into())
    } else {
        let stride = (width * channels * size).div_ceil(4) * 4;
        macro_rules! unpack {
            ($t:ty,$read:expr,$opaque:expr,$variant:ident) => {{
                let mut out = vec![$opaque; width * height * 4];
                for y in 0..height {
                    request.check_cancelled()?;
                    for x in 0..width {
                        let at = destination(x, y);
                        for c in 0..channels {
                            let src = y * stride + (x * channels + c) * size;
                            out[at + c] = $read(src);
                        }
                    }
                }
                RasterPixels::$variant(Arc::new(out).into())
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
    #[test]
    fn selected_mips_preserve_native_precision_orientation_and_transfer() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/raster");
        let manifest = std::fs::read_to_string(root.join("ktx-mip-manifest.tsv")).unwrap();
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
    fn every_chain_prefix_is_rejected_even_for_a_complete_first_mip() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/raster");
        let manifest = std::fs::read_to_string(root.join("ktx-mip-manifest.tsv")).unwrap();
        let mut names = std::collections::BTreeSet::new();
        for line in manifest.lines().filter(|s| !s.starts_with('#') && !s.is_empty()) {
            names.insert(line.split('\t').next().unwrap());
        }
        for name in names {
            let bytes = std::fs::read(root.join(name)).unwrap();
            let request = DecodeRequest::new(name);
            for end in 0..bytes.len() {
                assert!(decode(&bytes[..end], &request).is_err(), "{name}: prefix {end}");
            }
            assert!(decode(&bytes, &request).is_ok());
        }
    }
    #[test]
    fn selected_mip_routes_under_a_sensor_extension() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/raster");
        let path = std::env::temp_dir().join(format!("rrrah-ktx-mip-{}.cr3", std::process::id()));
        std::fs::copy(root.join("ktx-mips-4ch-16-1.ktx"), &path).unwrap();
        let mut request = DecodeRequest::new(&path);
        request.image_index = 2;
        let result = crate::decode_image(&request);
        std::fs::remove_file(path).unwrap();
        let crate::DecodedImage::Raster(image) = result.unwrap() else {
            panic!()
        };
        assert_eq!((image.width(), image.height()), (2, 1));
        assert_eq!((image.image_index(), image.image_count()), (2, 4));
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
