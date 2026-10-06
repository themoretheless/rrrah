//! Selected composited APNG frames with explicit timing and repeat metadata.
use crate::animation::over;
use crate::{
    DecodeRequest,
    bounded_io::read_bounded,
    raster::{MAX_RASTER_BYTES, RasterDecodeError},
};
use image::{DynamicImage, ImageDecoder, Limits, codecs::png::PngDecoder};
use rrrah_core::{DecodedRaster, RasterColorSpace, RasterPixels};
use std::{io::Cursor, sync::Arc};
const MAGIC: &[u8] = b"\x89PNG\r\n\x1a\n";
fn bad(s: &'static str) -> RasterDecodeError {
    RasterDecodeError::InvalidApng(s)
}
/// Animation presentation metadata. Zero `num_plays` means infinite repetition.
#[derive(Debug, Clone)]
pub struct ApngImage {
    pub raster: DecodedRaster,
    pub source_color_space: RasterColorSpace,
    pub delay_ms_numerator: u32,
    pub delay_ms_denominator: u32,
    pub num_plays: u32,
}
pub(crate) fn has_animation(b: &[u8]) -> bool {
    if !b.starts_with(MAGIC) {
        return false;
    }
    let mut at = 8usize;
    while let Some(h) = b.get(at..at.saturating_add(8)) {
        let n = u32::from_be_bytes(h[..4].try_into().unwrap()) as usize;
        if &h[4..] == b"acTL" {
            return true;
        }
        if &h[4..] == b"IEND" {
            return false;
        }
        let Some(end) = at.checked_add(n).and_then(|v| v.checked_add(12)) else {
            return false;
        };
        at = end;
    }
    false
}
pub fn decode_apng(request: &DecodeRequest) -> Result<ApngImage, RasterDecodeError> {
    request.check_cancelled()?;
    decode(&read_bounded(request)?, request)
}
pub(crate) fn decode(b: &[u8], request: &DecodeRequest) -> Result<ApngImage, RasterDecodeError> {
    request.check_cancelled()?;
    if !b.starts_with(MAGIC) {
        return Err(bad("PNG signature"));
    }
    let mut at = 8usize;
    let mut animation = None;
    let mut controls = 0usize;
    let mut sequence = 0u32;
    let mut idat_seen = false;
    let mut ended = false;
    while at < b.len() {
        request.check_cancelled()?;
        let h = b.get(at..at + 8).ok_or_else(|| bad("truncated chunk header"))?;
        let n = u32::from_be_bytes(h[..4].try_into().unwrap()) as usize;
        let end = at
            .checked_add(n)
            .and_then(|v| v.checked_add(12))
            .filter(|v| *v <= b.len())
            .ok_or_else(|| bad("truncated chunk"))?;
        let crc = u32::from_be_bytes(b[end - 4..end].try_into().unwrap());
        let mut checksum = flate2::Crc::new();
        for piece in b[at + 4..end - 4].chunks(65536) {
            request.check_cancelled()?;
            checksum.update(piece);
        }
        if checksum.sum() != crc {
            return Err(bad("chunk CRC"));
        }
        if &h[4..] == b"acTL" {
            if n != 8 || animation.is_some() || idat_seen {
                return Err(bad("invalid/duplicate animation control"));
            }
            animation = Some((
                u32::from_be_bytes(b[at + 8..at + 12].try_into().unwrap()),
                u32::from_be_bytes(b[at + 12..at + 16].try_into().unwrap()),
            ));
        }
        if &h[4..] == b"IDAT" {
            idat_seen = true;
        }
        if matches!(&h[4..], b"fcTL" | b"fdAT") {
            if animation.is_none()
                || (&h[4..] == b"fcTL" && n != 26)
                || (&h[4..] == b"fdAT" && (n < 4 || controls == 0))
            {
                return Err(bad("invalid frame control/data"));
            }
            let declared = u32::from_be_bytes(b[at + 8..at + 12].try_into().unwrap());
            if declared != sequence {
                return Err(bad("frame sequence mismatch"));
            }
            sequence = sequence
                .checked_add(1)
                .ok_or_else(|| bad("frame sequence overflow"))?;
            if &h[4..] == b"fcTL" {
                controls += 1;
            }
        }
        if &h[4..] == b"IEND" {
            if n != 0 || end != b.len() {
                return Err(bad("invalid end/trailing data"));
            }
            ended = true;
        }
        at = end;
    }
    if !ended {
        return Err(bad("missing end"));
    }
    let (count, num_plays) = animation.ok_or_else(|| bad("missing animation control"))?;
    if count == 0 || count > 4096 || controls != count as usize {
        return Err(bad("invalid/excessive frame count"));
    }
    if b.get(24) != Some(&8) {
        return Err(bad("only 8-bit animation samples are qualified"));
    }
    let declaration = crate::png_color::declaration(b)?;
    let mut decoder = PngDecoder::new(Cursor::new(b))?;
    let (w, h) = decoder.dimensions();
    if w == 0 || h == 0 || w > 65536 || h > 65536 {
        return Err(bad("canvas dimensions"));
    }
    if u64::from(w) * u64::from(h) * 16 * 4 > MAX_RASTER_BYTES {
        return Err(RasterDecodeError::OutputTooLarge);
    }
    let mut limits = Limits::default();
    limits.max_alloc = Some(MAX_RASTER_BYTES);
    decoder.set_limits(limits)?;
    if decoder.orientation()? != image::metadata::Orientation::NoTransforms {
        return Err(bad("animated EXIF orientation requires qualification"));
    }
    let profile = decoder.icc_profile()?;
    let color = if declaration.override_icc {
        declaration.color.unwrap_or(RasterColorSpace::Unspecified)
    } else {
        profile
            .map(RasterColorSpace::Icc)
            .or(declaration.color)
            .unwrap_or(RasterColorSpace::Unspecified)
    };
    // Only animation payloads needed for presentation are inflated.
    drop(decoder);
    let mut frames: Vec<Control<'_>> = Vec::new();
    let mut shared = Vec::new();
    let mut at = 8;
    while at < b.len() {
        let n = u32::from_be_bytes(b[at..at + 4].try_into().unwrap()) as usize;
        let kind = &b[at + 4..at + 8];
        let data = &b[at + 8..at + 8 + n];
        if kind == b"fcTL" {
            let word = |i| u32::from_be_bytes(data[i..i + 4].try_into().unwrap());
            let control = Control {
                width: word(4),
                height: word(8),
                x: word(12),
                y: word(16),
                delay_num: u16::from_be_bytes(data[20..22].try_into().unwrap()),
                delay_den: u16::from_be_bytes(data[22..24].try_into().unwrap()),
                dispose: data[24],
                blend: data[25],
                data: Vec::new(),
            };
            if control.width == 0
                || control.height == 0
                || control.x.checked_add(control.width).is_none_or(|v| v > w)
                || control.y.checked_add(control.height).is_none_or(|v| v > h)
                || control.dispose > 2
                || control.blend > 1
            {
                return Err(bad("frame rectangle/disposal/blend"));
            }
            frames.push(control);
        } else if matches!(kind, b"IDAT" | b"fdAT") {
            if let Some(frame) = frames.last_mut() {
                frame.data.push(if kind == b"fdAT" { &data[4..] } else { data });
            }
        } else if matches!(kind, b"PLTE" | b"tRNS") {
            shared.extend_from_slice(&b[at..at + n + 12]);
        }
        at += n + 12;
    }
    if request.image_index >= frames.len() {
        return Err(crate::DecodeError::UnsupportedImageIndex {
            index: request.image_index,
        }
        .into());
    }
    // A full SOURCE frame resets the canvas; PREVIOUS must retain older state unless it is selected.
    let start = (0..=request.image_index)
        .rev()
        .find(|&i| {
            let f = &frames[i];
            f.blend == 0
                && f.x == 0
                && f.y == 0
                && f.width == w
                && f.height == h
                && (f.dispose != 2 || i == request.image_index)
        })
        .unwrap_or(0);
    let mut canvas = vec![0f32; w as usize * h as usize * 4];
    let mut selected = None;
    for (index, frame) in frames
        .iter()
        .enumerate()
        .skip(start)
        .take(request.image_index - start + 1)
    {
        request.check_cancelled()?;
        let mut ihdr = b[16..29].to_vec();
        ihdr[..4].copy_from_slice(&frame.width.to_be_bytes());
        ihdr[4..8].copy_from_slice(&frame.height.to_be_bytes());
        let mut standalone = MAGIC.to_vec();
        append_chunk(&mut standalone, b"IHDR", &ihdr);
        standalone.extend_from_slice(&shared);
        for piece in &frame.data {
            append_chunk(&mut standalone, b"IDAT", piece);
        }
        append_chunk(&mut standalone, b"IEND", &[]);
        let mut source = PngDecoder::new(Cursor::new(standalone))?;
        let mut limits = Limits::default();
        limits.max_alloc = Some(MAX_RASTER_BYTES);
        source.set_limits(limits)?;
        let raw = DynamicImage::from_decoder(source)?.into_rgba8().into_raw();
        let surface = DecodedRaster::new(
            frame.width,
            frame.height,
            RasterPixels::Rgba8(Arc::new(raw).into()),
            color.clone(),
        )?;
        let prepared = crate::prepare_raster_for_display(&surface)?;
        let RasterPixels::Rgba32Float(raw) = prepared.pixels() else {
            unreachable!()
        };
        let previous = if frame.dispose == 2 && index != request.image_index {
            Some(canvas.clone())
        } else {
            None
        };
        for y in 0..frame.height as usize {
            request.check_cancelled()?;
            for x in 0..frame.width as usize {
                let src = (y * frame.width as usize + x) * 4;
                let dst = ((y + frame.y as usize) * w as usize + x + frame.x as usize) * 4;
                if frame.blend == 0 {
                    canvas[dst..dst + 4].copy_from_slice(&raw[src..src + 4]);
                } else {
                    over(&mut canvas[dst..dst + 4], &raw[src..src + 4]);
                }
            }
        }
        if index == request.image_index {
            selected = Some((
                canvas,
                u32::from(frame.delay_num) * 1000,
                u32::from(if frame.delay_den == 0 {
                    100
                } else {
                    frame.delay_den
                }),
            ));
            break;
        }
        match frame.dispose {
            1 => {
                for y in 0..frame.height as usize {
                    let at = ((y + frame.y as usize) * w as usize + frame.x as usize) * 4;
                    canvas[at..at + frame.width as usize * 4].fill(0.);
                }
            }
            2 => canvas = previous.unwrap(),
            _ => (),
        }
    }
    let (pixels, delay_ms_numerator, delay_ms_denominator) =
        selected.ok_or(crate::DecodeError::UnsupportedImageIndex {
            index: request.image_index,
        })?;
    request.check_cancelled()?;
    let raster = DecodedRaster::new(
        w,
        h,
        RasterPixels::Rgba32Float(Arc::new(pixels).into()),
        RasterColorSpace::LinearSrgb,
    )?
    .with_image_selection(request.image_index, count as usize)?;
    Ok(ApngImage {
        raster,
        source_color_space: color,
        delay_ms_numerator,
        delay_ms_denominator,
        num_plays,
    })
}
struct Control<'a> {
    width: u32,
    height: u32,
    x: u32,
    y: u32,
    delay_num: u16,
    delay_den: u16,
    dispose: u8,
    blend: u8,
    data: Vec<&'a [u8]>,
}
fn append_chunk(out: &mut Vec<u8>, kind: &[u8; 4], data: &[u8]) {
    out.extend_from_slice(&(data.len() as u32).to_be_bytes());
    let start = out.len();
    out.extend_from_slice(kind);
    out.extend_from_slice(data);
    out.extend_from_slice(&crate::png_color::crc32(&out[start..]).to_be_bytes());
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn linear_light_composition_timing_and_loop_oracles() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/raster");
        let manifest = std::fs::read_to_string(root.join("apng-manifest.tsv")).unwrap();
        for line in manifest.lines().filter(|s| !s.starts_with('#') && !s.is_empty()) {
            let c: Vec<_> = line.split('\t').collect();
            let mut request = DecodeRequest::new(root.join(c[0]));
            request.image_index = c[1].parse().unwrap();
            let image = decode_apng(&request).unwrap();
            assert_eq!((image.raster.width(), image.raster.height()), (5, 3));
            assert_eq!(
                (image.raster.image_index(), image.raster.image_count()),
                (request.image_index, 4)
            );
            assert_eq!(image.num_plays, c[6].parse::<u32>().unwrap());
            let numerator: u64 = c[4].parse().unwrap();
            let denominator: u64 = c[5].parse().unwrap();
            assert_eq!(
                u64::from(image.delay_ms_numerator) * denominator,
                numerator * u64::from(image.delay_ms_denominator)
            );
            assert_eq!(image.raster.color_space(), &RasterColorSpace::LinearSrgb);
            assert_eq!(image.source_color_space, RasterColorSpace::Srgb);
            let RasterPixels::Rgba32Float(values) = image.raster.pixels() else {
                panic!()
            };
            let expected = std::fs::read(root.join(c[7])).unwrap();
            assert_eq!(expected.len(), values.len() * 4);
            for (value, bits) in values.iter().zip(expected.chunks_exact(4)) {
                let wanted = f32::from_le_bytes(bits.try_into().unwrap());
                assert!(
                    (value - wanted).abs() < 2e-7,
                    "{} frame {}: {value} != {wanted}",
                    c[0],
                    c[1]
                );
            }
            let routed = crate::decode_raster(&request).unwrap();
            assert_eq!(routed.image_count(), 4);
            let prepared = crate::prepare_raster_for_display(&routed).unwrap();
            assert_eq!(
                (prepared.image_index(), prepared.image_count()),
                (request.image_index, 4)
            );
            request.image_index = 4;
            assert!(matches!(
                decode_apng(&request),
                Err(RasterDecodeError::Source(
                    crate::DecodeError::UnsupportedImageIndex { index: 4 }
                ))
            ));
        }
    }
    #[test]
    fn truncation_crc_trailing_and_cancellation_are_explicit() {
        let bytes = include_bytes!("../../../tests/fixtures/raster/apng-compose-0-0.apng");
        let request = DecodeRequest::new("synthetic.apng");
        for end in 0..bytes.len() {
            assert!(decode(&bytes[..end], &request).is_err(), "prefix {end}");
        }
        let mut invalid = bytes.to_vec();
        invalid[bytes.len() - 1] ^= 1;
        assert!(matches!(
            decode(&invalid, &request),
            Err(RasterDecodeError::InvalidApng("chunk CRC"))
        ));
        let mut invalid = bytes.to_vec();
        invalid.push(0);
        assert!(decode(&invalid, &request).is_err());
        let mut request = request;
        request.cancellation = Some(crate::GenerationToken::new(
            Arc::new(std::sync::atomic::AtomicU64::new(2)),
            1,
        ));
        assert!(matches!(
            decode_apng(&request),
            Err(RasterDecodeError::Source(crate::DecodeError::Cancelled))
        ));
    }
}
#[cfg(test)]
mod malformed_controls {
    use super::*;
    fn altered(kind: &[u8; 4], offset: usize, values: &[u8]) -> Vec<u8> {
        let mut b = include_bytes!("../../../tests/fixtures/raster/apng-compose-0-0.apng").to_vec();
        let mut at = 8;
        loop {
            let n = u32::from_be_bytes(b[at..at + 4].try_into().unwrap()) as usize;
            if &b[at + 4..at + 8] == kind {
                b[at + 8 + offset..at + 8 + offset + values.len()].copy_from_slice(values);
                let crc = crate::png_color::crc32(&b[at + 4..at + 8 + n]);
                b[at + 8 + n..at + 12 + n].copy_from_slice(&crc.to_be_bytes());
                return b;
            }
            at += n + 12;
        }
    }
    #[test]
    fn valid_crc_does_not_hide_invalid_animation_controls() {
        let request = DecodeRequest::new("synthetic.apng");
        for b in [
            altered(b"acTL", 0, &0u32.to_be_bytes()),
            altered(b"acTL", 0, &3u32.to_be_bytes()),
            altered(b"acTL", 0, &4097u32.to_be_bytes()),
            altered(b"fcTL", 0, &1u32.to_be_bytes()),
            altered(b"fcTL", 4, &6u32.to_be_bytes()),
            altered(b"fcTL", 24, &[3]),
            altered(b"fcTL", 25, &[2]),
            altered(b"IHDR", 8, &[16]),
        ] {
            assert!(decode(&b, &request).is_err());
        }
    }
}
#[cfg(test)]
mod lazy_tests {
    use super::*;
    use std::io::Write;
    fn two_frames(bad_first: bool, bad_last: bool) -> Vec<u8> {
        let mut out = MAGIC.to_vec();
        append_chunk(&mut out, b"IHDR", &[0, 0, 0, 1, 0, 0, 0, 1, 8, 6, 0, 0, 0]);
        append_chunk(&mut out, b"sRGB", &[0]);
        append_chunk(&mut out, b"acTL", &[0, 0, 0, 2, 0, 0, 0, 0]);
        for i in 0..2u32 {
            let mut control = Vec::new();
            for v in [i, 1, 1, 0, 0] {
                control.extend_from_slice(&v.to_be_bytes());
            }
            control.extend_from_slice(&[0, 1, 0, 60, 0, 0]);
            append_chunk(&mut out, b"fcTL", &control);
            let payload = if (i == 0 && bad_first) || (i == 1 && bad_last) {
                vec![0, 1, 2]
            } else {
                let mut z = flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::default());
                z.write_all(if i == 0 {
                    &[0, 255, 0, 0, 255]
                } else {
                    &[0, 0, 255, 0, 255]
                })
                .unwrap();
                z.finish().unwrap()
            };
            if i == 0 {
                append_chunk(&mut out, b"IDAT", &payload);
            } else {
                let mut data = 2u32.to_be_bytes().to_vec();
                data.extend(payload);
                append_chunk(&mut out, b"fdAT", &data);
            }
        }
        append_chunk(&mut out, b"IEND", &[]);
        out
    }
    #[test]
    fn unrelated_frame_pixels_are_not_decoded_but_chunk_crc_still_is() {
        let mut request = DecodeRequest::new("synthetic.apng");
        let bytes = two_frames(false, true);
        let image = decode(&bytes, &request).unwrap();
        let RasterPixels::Rgba32Float(values) = image.raster.pixels() else {
            panic!()
        };
        assert_eq!(&values[..], &[1., 0., 0., 1.]);
        request.image_index = 1;
        assert!(decode(&bytes, &request).is_err());
        let bytes = two_frames(true, false);
        let image = decode(&bytes, &request).unwrap();
        let RasterPixels::Rgba32Float(values) = image.raster.pixels() else {
            panic!()
        };
        assert_eq!(&values[..], &[0., 1., 0., 1.]);
        request.image_index = 0;
        assert!(decode(&bytes, &request).is_err());
        request.image_index = 1;
        let mut corrupt = bytes;
        let last = corrupt.len() - 1;
        corrupt[last] ^= 1;
        assert!(decode(&corrupt, &request).is_err());
    }
}
