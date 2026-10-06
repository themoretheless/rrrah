//! PVR v3 2D mip selection, ordinary normalized/float channels and PVRTC1.
use crate::{
    DecodeRequest,
    bounded_io::read_bounded,
    raster::{MAX_RASTER_BYTES, RasterDecodeError},
};
use rrrah_core::{DecodedRaster, RasterColorSpace, RasterPixels};
use std::sync::Arc;
fn bad(s: &'static str) -> RasterDecodeError {
    RasterDecodeError::InvalidPvr(s)
}
pub(crate) fn has_magic(b: &[u8]) -> bool {
    matches!(b.get(..4), Some(b"PVR\x03") | Some(b"\x03RVP"))
}
#[derive(Debug, Clone)]
pub struct PvrMetadata {
    pub fourcc: [u8; 4],
    pub key: u32,
    pub data: Arc<Vec<u8>>,
}
#[derive(Debug, Clone)]
pub struct PvrImage {
    pub raster: DecodedRaster,
    pub metadata: Vec<PvrMetadata>,
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
pub(crate) fn decode(b: &[u8], request: &DecodeRequest) -> Result<PvrImage, RasterDecodeError> {
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
    if flags == 2 && color == RasterColorSpace::Srgb {
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
    let mut metadata = Vec::new();
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
        metadata.push(PvrMetadata {
            fourcc,
            key,
            data: Arc::new(data.to_vec()),
        });
    }
    let compressed = format >> 32 == 0;
    let component_bytes = if compressed {
        if format > 3 || channel_type != 0 || be {
            return Err(bad("unsupported compressed codec/type/byte order"));
        }
        if !width.is_power_of_two() || !height.is_power_of_two() {
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
            if format < 2 {
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
    let pixels = if compressed {
        let two = format < 2;
        let pw = w.max(if two { 16 } else { 8 });
        let ph = h.max(8);
        let mut decoded = vec![0u32; pw * ph];
        texture2ddecoder::decode_pvrtc(data, pw, ph, &mut decoded, two).map_err(bad)?;
        request.check_cancelled()?;
        let mut values = vec![0u8; w * h * 4];
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
        RasterPixels::Rgba8(Arc::new(values).into())
    } else if component_bytes == 1 {
        let mut values = vec![0u8; w * h * 4];
        for (i, v) in data.chunks_exact(channels).enumerate() {
            if i % 4096 == 0 {
                request.check_cancelled()?;
            }
            let p = position(i % w, i / w) * 4;
            values[p..p + 4].copy_from_slice(&rgba_channels(v, names, 0, 255));
        }
        RasterPixels::Rgba8(Arc::new(values).into())
    } else if component_bytes == 2 {
        let mut values = vec![0u16; w * h * 4];
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
        RasterPixels::Rgba16(Arc::new(values).into())
    } else {
        let mut values = vec![0f32; w * h * 4];
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
        RasterPixels::Rgba32Float(Arc::new(values).into())
    };
    let pixels = if flags == 2 {
        let mut straight = Vec::with_capacity(w * h * 4);
        for i in 0..w * h {
            if i % 4096 == 0 {
                request.check_cancelled()?;
            }
            let mut pixel: [f32; 4] = match &pixels {
                RasterPixels::Rgba8(v) => std::array::from_fn(|c| f32::from(v[i * 4 + c]) / 255.),
                RasterPixels::Rgba16(v) => std::array::from_fn(|c| f32::from(v[i * 4 + c]) / 65535.),
                RasterPixels::Rgba32Float(v) => v[i * 4..i * 4 + 4].try_into().unwrap(),
            };
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
            straight.extend(pixel);
        }
        RasterPixels::Rgba32Float(Arc::new(straight).into())
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
    #[test]
    fn native_sample_and_powervr_pvrtc_oracles_match() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/raster");
        let manifest = std::fs::read_to_string(root.join("pvr-manifest.tsv")).unwrap();
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
            let generic = crate::decode_raster(&request).unwrap();
            assert_eq!(generic.image_count(), raster.image_count());
            let prepared = crate::prepare_raster_for_display(raster).unwrap();
            assert_eq!(
                (prepared.image_index(), prepared.image_count()),
                (raster.image_index(), raster.image_count())
            );
            if c[7] != "-" {
                let oracle = std::fs::read(root.join(c[7])).unwrap();
                let RasterPixels::Rgba32Float(values) = prepared.pixels() else {
                    panic!()
                };
                for (pixel, expected) in values.chunks_exact(4).zip(oracle.chunks_exact(12)) {
                    for channel in 0..3 {
                        let expected =
                            f32::from_le_bytes(expected[channel * 4..channel * 4 + 4].try_into().unwrap());
                        let composed =
                            pixel[channel] * pixel[3] + [0.25, 0.5, 0.75][channel] * (1. - pixel[3]);
                        assert!((composed - expected).abs() < 2e-7);
                    }
                }
            }
        }
    }
    #[test]
    fn every_fixture_prefix_is_rejected_before_partial_mip_output() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/raster");
        let manifest = std::fs::read_to_string(root.join("pvr-manifest.tsv")).unwrap();
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
        let request = DecodeRequest::new("synthetic.pvr");
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
    #[test]
    fn selected_mip_routes_under_raw_suffix_and_is_bounded() {
        let path = std::env::temp_dir().join(format!("rrrah-pvr-router-{}.cr3", std::process::id()));
        std::fs::write(
            &path,
            include_bytes!("../../../tests/fixtures/raster/pvr-rgba16-be.pvr"),
        )
        .unwrap();
        let mut request = DecodeRequest::new(&path);
        request.image_index = 1;
        let result = crate::decode_image(&request);
        std::fs::remove_file(path).unwrap();
        let crate::DecodedImage::Raster(raster) = result.unwrap() else {
            panic!()
        };
        assert_eq!((raster.width(), raster.height()), (2, 1));
        assert_eq!((raster.image_index(), raster.image_count()), (1, 3));
        let prepared = crate::prepare_raster_for_display(&raster).unwrap();
        assert_eq!((prepared.image_index(), prepared.image_count()), (1, 3));
    }
}
