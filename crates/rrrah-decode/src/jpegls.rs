//! Bounded CharLS decoding with retained sample precision and explicit layout.
use crate::{
    DecodeRequest,
    bounded_io::read_bounded,
    raster::{MAX_RASTER_BYTES, RasterDecodeError},
};
use rrrah_core::{DecodedRaster, RasterColorSpace, RasterPixels};
use std::sync::Arc;
#[derive(Debug, Clone)]
pub struct JpegLsImage {
    pub raster: DecodedRaster,
    pub bits_per_sample: u8,
    pub maximum_sample_value: u16,
    pub near_lossless: u8,
    /// 0 planar; 1 line-interleaved source; 2 sample-interleaved source.
    pub interleave_mode: u8,
}
fn bad(s: &'static str) -> RasterDecodeError {
    RasterDecodeError::InvalidJpegLs(s)
}
fn segments<'a>(b: &'a [u8], request: &DecodeRequest) -> Result<Vec<(u8, &'a [u8])>, RasterDecodeError> {
    if !b.starts_with(&[255, 216]) {
        return Err(bad("SOI"));
    }
    let mut at = 2usize;
    let mut out = Vec::new();
    let mut entropy = false;
    let mut seen_scan = false;
    loop {
        request.check_cancelled()?;
        if entropy {
            loop {
                if at % 65536 == 0 {
                    request.check_cancelled()?;
                }
                let v = *b.get(at).ok_or_else(|| bad("missing EOI"))?;
                if v == 255 {
                    let next = *b.get(at + 1).ok_or_else(|| bad("truncated entropy marker"))?;
                    if next >= 128 {
                        break;
                    }
                    at += 2;
                } else {
                    at += 1;
                }
            }
        }
        if b.get(at) != Some(&255) {
            return Err(bad("marker prefix"));
        }
        while b.get(at) == Some(&255) {
            at += 1;
        }
        let marker = *b.get(at).ok_or_else(|| bad("truncated marker"))?;
        at += 1;
        if marker == 0xd9 {
            if at != b.len() || !seen_scan {
                return Err(bad("EOI/trailing data"));
            }
            return Ok(out);
        }
        if (0xd0..=0xd7).contains(&marker) && entropy {
            continue;
        }
        if marker == 0xd8 {
            return Err(bad("duplicate SOI"));
        }
        entropy = false;
        let n = b
            .get(at..at + 2)
            .map(|v| u16::from_be_bytes(v.try_into().unwrap()) as usize)
            .ok_or_else(|| bad("segment length"))?;
        let end = at
            .checked_add(n)
            .filter(|&v| n >= 2 && v <= b.len())
            .ok_or_else(|| bad("segment bounds"))?;
        out.push((marker, &b[at + 2..end]));
        if out.len() > 65536 {
            return Err(bad("too many markers"));
        }
        if marker == 0xda {
            entropy = true;
            seen_scan = true;
        }
        at = end;
    }
}
pub(crate) fn has_magic(b: &[u8]) -> bool {
    if !b.starts_with(&[255, 216]) {
        return false;
    }
    let mut at = 2usize;
    while b.get(at) == Some(&255) {
        while b.get(at) == Some(&255) {
            at += 1;
        }
        let Some(&marker) = b.get(at) else {
            return false;
        };
        at += 1;
        if marker == 0xf7 {
            return true;
        }
        let Some(n) = b
            .get(at..at + 2)
            .map(|v| u16::from_be_bytes(v.try_into().unwrap()) as usize)
        else {
            return false;
        };
        if n < 2 {
            return false;
        }
        let Some(end) = at.checked_add(n) else {
            return false;
        };
        at = end;
    }
    false
}
pub fn decode_jpegls(request: &DecodeRequest) -> Result<JpegLsImage, RasterDecodeError> {
    request.check_cancelled()?;
    decode(&read_bounded(request)?, request)
}
pub(crate) fn decode(b: &[u8], request: &DecodeRequest) -> Result<JpegLsImage, RasterDecodeError> {
    request.check_cancelled()?;
    if request.image_index != 0 {
        return Err(crate::DecodeError::UnsupportedImageIndex {
            index: request.image_index,
        }
        .into());
    }
    if !has_magic(b) || !b.ends_with(&[255, 217]) {
        return Err(bad("JPEG-LS SOF/EOI"));
    }
    let segments = segments(b, request)?;
    let info = charls::CharLS::default().get_frame_info(b)?;
    if info.width == 0
        || info.height == 0
        || info.width > 65536
        || info.height > 65536
        || !matches!(info.component_count, 1 | 3)
        || !(2..=16).contains(&info.bits_per_sample)
    {
        return Err(bad("dimensions/channels/depth"));
    }
    let pixels = u64::from(info.width) * u64::from(info.height);
    if pixels * 32 > MAX_RASTER_BYTES {
        return Err(RasterDecodeError::OutputTooLarge);
    }
    let bits = info.bits_per_sample as u8;
    let mut max = (1u32 << bits) - 1;
    let mut color = RasterColorSpace::AssumedSrgb;
    let mut profile_parts = Vec::new();
    let mut total = None;
    let mut scanned = false;
    for &(marker, data) in &segments {
        if marker == 0xda {
            scanned = true;
        }
        request.check_cancelled()?;
        if marker == 0xf8 && data.first() == Some(&1) {
            if scanned {
                return Err(bad("changing preset parameters between scans"));
            }
            if data.len() != 11 {
                return Err(bad("preset coding parameters"));
            }
            max = u32::from(u16::from_be_bytes(data[1..3].try_into().unwrap()));
        }
        if marker == 0xe8 && data.starts_with(b"SPIFF\0") {
            if data.len() != 30
                || data[9] != info.component_count as u8
                || data[19] != bits
                || data[20] != 6
                || u32::from_be_bytes(data[10..14].try_into().unwrap()) != info.height
                || u32::from_be_bytes(data[14..18].try_into().unwrap()) != info.width
            {
                return Err(bad("SPIFF header"));
            }
            color = match data[18] {
                2 => RasterColorSpace::Unspecified,
                8 if info.component_count == 1 => RasterColorSpace::AssumedSrgb,
                10 if info.component_count == 3 => RasterColorSpace::AssumedSrgb,
                _ => return Err(bad("unsupported SPIFF color interpretation")),
            };
        }
        if marker == 0xe2 && data.starts_with(b"ICC_PROFILE\0") {
            if data.len() < 14
                || data[12] == 0
                || data[13] == 0
                || data[12] > data[13]
                || total.is_some_and(|n| n != data[13])
            {
                return Err(bad("ICC sequence"));
            }
            total = Some(data[13]);
            profile_parts.push((data[12], &data[14..]));
        }
    }
    if max == 0 || max >= (1u32 << bits) {
        return Err(bad("maximum sample value"));
    }
    if let Some(count) = total {
        profile_parts.sort_by_key(|v| v.0);
        if profile_parts.len() != count as usize
            || profile_parts
                .iter()
                .enumerate()
                .any(|(i, p)| p.0 as usize != i + 1)
        {
            return Err(bad("ICC sequence"));
        }
        color = RasterColorSpace::Icc(profile_parts.iter().flat_map(|p| p.1.iter().copied()).collect());
    }
    let sof: Vec<_> = segments.iter().filter(|v| v.0 == 0xf7).collect();
    if sof.len() != 1 || sof[0].1.len() != 6 + 3 * info.component_count as usize {
        return Err(bad("SOF component layout"));
    }
    let ids: Vec<_> = sof[0].1[6..].chunks_exact(3).map(|v| v[0]).collect();
    let scans: Vec<_> = segments.iter().filter(|v| v.0 == 0xda).collect();
    let sos = scans.first().ok_or_else(|| bad("missing SOS"))?.1;
    let count = *sos.first().ok_or_else(|| bad("SOS"))? as usize;
    if sos.len() != 1 + 2 * count + 3 {
        return Err(bad("SOS size"));
    }
    let near = sos[1 + 2 * count];
    let ilv = sos[2 + 2 * count];
    if ilv > 2
        || u32::from(near) > max
        || scans.len()
            != if ilv == 0 {
                info.component_count as usize
            } else {
                1
            }
    {
        return Err(bad("SOS layout/near/scans"));
    }
    for (index, scan) in scans.iter().enumerate() {
        let data = scan.1;
        let n = *data.first().ok_or_else(|| bad("SOS"))? as usize;
        if data.len() != 1 + 2 * n + 3
            || data[1 + 2 * n] != near
            || data[2 + 2 * n] != ilv
            || data[3 + 2 * n] != 0
        {
            return Err(bad("mixed scan parameters or point transform"));
        }
        let selectors: Vec<_> = data[1..1 + 2 * n].chunks_exact(2).map(|v| v[0]).collect();
        if (ilv == 0 && (n != 1 || selectors[0] != ids[index])) || (ilv != 0 && selectors != ids) {
            return Err(bad("scan component order"));
        }
    }
    request.check_cancelled()?;
    let native = charls::CharLS::default().decode(b)?;
    request.check_cancelled()?;
    let sample_bytes = if bits <= 8 { 1usize } else { 2 };
    let components = info.component_count as usize;
    let pixels = pixels as usize;
    if native.len() != pixels * components * sample_bytes {
        return Err(bad("decoded sample count"));
    }
    let value = |pixel: usize, component: usize| -> Result<u32, RasterDecodeError> {
        let index = if ilv == 0 {
            component * pixels + pixel
        } else {
            pixel * components + component
        };
        let at = index * sample_bytes;
        let v = if sample_bytes == 1 {
            u32::from(native[at])
        } else {
            u32::from(u16::from_ne_bytes(native[at..at + 2].try_into().unwrap()))
        };
        if v > max {
            return Err(bad("decoded sample exceeds MAXVAL"));
        }
        Ok(v)
    };
    let raster_pixels = if bits <= 8 {
        let mut out = Vec::with_capacity(pixels * 4);
        for p in 0..pixels {
            if p % 65536 == 0 {
                request.check_cancelled()?;
            }
            for c in 0..3 {
                let v = value(p, if components == 1 { 0 } else { c })?;
                out.push(((v * 255 + max / 2) / max) as u8);
            }
            out.push(255);
        }
        RasterPixels::Rgba8(Arc::new(out).into())
    } else {
        let mut out = Vec::with_capacity(pixels * 4);
        for p in 0..pixels {
            if p % 65536 == 0 {
                request.check_cancelled()?;
            }
            for c in 0..3 {
                let v = value(p, if components == 1 { 0 } else { c })?;
                out.push(((v * 65535 + max / 2) / max) as u16);
            }
            out.push(65535);
        }
        RasterPixels::Rgba16(Arc::new(out).into())
    };
    Ok(JpegLsImage {
        raster: DecodedRaster::new(info.width, info.height, raster_pixels, color)?,
        bits_per_sample: bits,
        maximum_sample_value: max as u16,
        near_lossless: near,
        interleave_mode: ilv,
    })
}
#[cfg(test)]
mod tests {
    use super::*;
    fn root() -> std::path::PathBuf {
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/raster")
    }
    #[test]
    fn independent_native_samples_and_layouts() {
        let root = root();
        for row in std::fs::read_to_string(root.join("jpegls-manifest.tsv"))
            .unwrap()
            .lines()
            .filter(|v| !v.starts_with('#'))
        {
            let c: Vec<_> = row.split('\t').collect();
            let request = DecodeRequest::new(root.join(c[0]));
            let image = decode_jpegls(&request).unwrap();
            assert_eq!((image.raster.width(), image.raster.height()), (9, 5));
            assert_eq!(image.bits_per_sample, c[3].parse::<u8>().unwrap());
            assert_eq!(image.interleave_mode, c[5].parse::<u8>().unwrap());
            assert_eq!(image.near_lossless, c[6].parse::<u8>().unwrap());
            let values: Vec<u16> = match image.raster.pixels() {
                RasterPixels::Rgba8(v) => v.iter().map(|&v| u16::from(v) * 257).collect(),
                RasterPixels::Rgba16(v) => v.as_ref().to_vec(),
                _ => panic!(),
            };
            let bytes = std::fs::read(root.join(c[8])).unwrap();
            let expected: Vec<_> = bytes
                .chunks_exact(2)
                .map(|b| u16::from_le_bytes(b.try_into().unwrap()))
                .collect();
            assert_eq!(values, expected, "{}", c[0]);
            crate::prepare_raster_for_display(&image.raster).unwrap();
            assert_eq!(crate::decode_raster(&request).unwrap().width(), 9);
        }
    }
    #[test]
    fn fragmented_icc_and_cancelled_requests_are_explicit() {
        use image::ImageDecoder;
        let root = root();
        let mut request = DecodeRequest::new(root.join("jls-ffmpeg-8-3-1-0.jls"));
        let original = std::fs::read(&request.path).unwrap();
        let png = std::fs::read(root.join("pattern.profiled.png")).unwrap();
        let mut decoder = image::codecs::png::PngDecoder::new(std::io::Cursor::new(png)).unwrap();
        let icc = decoder.icc_profile().unwrap().unwrap();
        let mut app = Vec::new();
        let half = icc.len() / 2;
        for (index, data) in [(2, &icc[half..]), (1, &icc[..half])] {
            let mut payload = b"ICC_PROFILE\0".to_vec();
            payload.extend_from_slice(&[index, 2]);
            payload.extend_from_slice(data);
            app.extend_from_slice(&[255, 226]);
            app.extend_from_slice(&((payload.len() + 2) as u16).to_be_bytes());
            app.extend_from_slice(&payload);
        }
        let mut profiled = original.clone();
        profiled.splice(2..2, app);
        let image = decode(&profiled, &request).unwrap();
        assert_eq!(image.raster.color_space(), &RasterColorSpace::Icc(icc));
        crate::prepare_raster_for_display(&image.raster).unwrap();
        let mut duplicate = profiled.clone();
        duplicate[2 + 4 + 12] = 1;
        assert!(decode(&duplicate, &request).is_err());
        let generation = std::sync::Arc::new(std::sync::atomic::AtomicU64::new(2));
        request.cancellation = Some(crate::GenerationToken::new(generation, 1));
        assert!(decode_jpegls(&request).is_err());
    }
    #[test]
    fn truncated_and_oversized_sources_reject() {
        let path = root().join("jls-ffmpeg-16-1-0-0.jls");
        let request = DecodeRequest::new(&path);
        let b = std::fs::read(path).unwrap();
        for n in 0..b.len() {
            assert!(decode(&b[..n], &request).is_err(), "prefix {n}");
        }
        let mut oversized = b.clone();
        let at = oversized.windows(2).position(|v| v == [255, 247]).unwrap();
        oversized[at + 5..at + 9].fill(255);
        assert!(decode(&oversized, &request).is_err());
        let mut concatenated = b.clone();
        concatenated.extend_from_slice(&b);
        assert!(decode(&concatenated, &request).is_err());
        let mut trailing = b;
        trailing.push(0);
        assert!(decode(&trailing, &request).is_err());
    }
}
