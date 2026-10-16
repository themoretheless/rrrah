//! Selected composited APNG frames with explicit timing and repeat metadata.
use crate::animation::over;
use crate::{
    DecodeRequest,
    bounded_io::read_bounded,
    raster::{MAX_RASTER_BYTES, RasterDecodeError},
};
use image::{DynamicImage, ImageDecoder, Limits, codecs::png::PngDecoder};
use rrrah_core::{DecodedRaster, RasterColorSpace, RasterPixels};
use std::io::Cursor;
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
pub fn has_animation(b: &[u8]) -> bool {
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
pub fn decode(b: &[u8], request: &DecodeRequest) -> Result<ApngImage, RasterDecodeError> {
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
    let mut idat_closed = false;
    let mut frame_uses_idat = false;
    let mut frame_has_data = false;
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
        let kind = &h[4..];
        if idat_seen && kind != b"IDAT" {
            idat_closed = true;
        }
        if kind == b"IDAT" {
            if idat_closed || (controls > 0 && !frame_uses_idat) {
                return Err(bad("IDAT outside default image"));
            }
            idat_seen = true;
            if controls > 0 {
                frame_has_data = true;
            }
        }
        if kind == b"fdAT" && (!idat_seen || frame_uses_idat) {
            return Err(bad("fdAT outside animation frame"));
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
            if kind == b"fcTL" {
                if controls > 0 && !frame_has_data {
                    return Err(bad("animation frame without data"));
                }
                frame_uses_idat = !idat_seen;
                frame_has_data = false;
                if frame_uses_idat && (b[at + 12..at + 20] != b[16..24] || b[at + 20..at + 28] != [0; 8]) {
                    return Err(bad("default frame must cover canvas"));
                }
                controls += 1;
            } else {
                frame_has_data = true;
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
    if controls > 0 && !frame_has_data {
        return Err(bad("animation frame without data"));
    }
    let (count, num_plays) = animation.ok_or_else(|| bad("missing animation control"))?;
    if count == 0 || count > 4096 || controls != count as usize {
        return Err(bad("invalid/excessive frame count"));
    }
    // PNG validates legal depth/color combinations and expands packed samples.
    let declaration = crate::png_color::declaration(b)?;
    let mut limits = Limits::default();
    limits.max_alloc = Some(MAX_RASTER_BYTES);
    let mut decoder = PngDecoder::with_limits(Cursor::new(b), limits)?;
    let (w, h) = decoder.dimensions();
    if w == 0 || h == 0 || w > 65536 || h > 65536 {
        return Err(bad("canvas dimensions"));
    }
    if u64::from(w) * u64::from(h) * 16 > MAX_RASTER_BYTES {
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
    frames.try_reserve_exact(count as usize).map_err(|_| RasterDecodeError::OutputTooLarge)?;
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
    let fallback = rrrah_core::MemoryBudget::new(MAX_RASTER_BYTES);
    let budget = request.memory_budget.as_ref().unwrap_or(&fallback);
    let mut canvas = budget
        .try_buffer(w as usize * h as usize * 4, 0f32)
        .map_err(crate::DecodeError::Memory)?;
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
        // Admit the reconstructed encoded frame before allocating its exact capacity.
        let encoded_len = frame
            .data
            .iter()
            .try_fold(8usize + 25 + shared.len() + 12, |n, p| {
                n.checked_add(p.len()).and_then(|v| v.checked_add(12))
            })
            .ok_or(RasterDecodeError::OutputTooLarge)?;
        let encoded_credit = budget
            .try_reserve(encoded_len as u64)
            .map_err(crate::DecodeError::Memory)?;
        let mut standalone = Vec::new();
        standalone
            .try_reserve_exact(encoded_len)
            .map_err(|_| RasterDecodeError::OutputTooLarge)?;
        standalone.extend_from_slice(MAGIC);
        append_chunk(&mut standalone, b"IHDR", &ihdr);
        standalone.extend_from_slice(&shared);
        for piece in &frame.data {
            append_chunk(&mut standalone, b"IDAT", piece);
        }
        append_chunk(&mut standalone, b"IEND", &[]);
        let standalone = encoded_credit
            .try_adopt(standalone)
            .map_err(crate::DecodeError::Memory)?;
        let mut limits = Limits::default();
        limits.max_alloc = Some(MAX_RASTER_BYTES);
        // set_limits after construction cannot update png::Reader's internal
        // allocation limits; install the limit before header/row setup.
        let source = PngDecoder::with_limits(Cursor::new(&*standalone), limits)?;
        // Decoder output and RGBA conversion can coexist. Retain both admissions
        // through conversion; PNG inflater/private row scratch is governed by codec limits.
        let decoded_credit = budget
            .try_reserve(source.total_bytes())
            .map_err(crate::DecodeError::Memory)?;
        let sixteen = matches!(
            source.color_type(),
            image::ColorType::L16
                | image::ColorType::La16
                | image::ColorType::Rgb16
                | image::ColorType::Rgba16
        );
        let rgba_credit = budget
            .try_reserve(u64::from(frame.width) * u64::from(frame.height) * if sixteen { 8 } else { 4 })
            .map_err(crate::DecodeError::Memory)?;
        let decoded = DynamicImage::from_decoder(source)?;
        let pixels = if sixteen {
            let raw = rgba_credit
                .try_adopt(decoded.into_rgba16().into_raw())
                .map_err(crate::DecodeError::Memory)?;
            RasterPixels::Rgba16(raw.into())
        } else {
            let raw = rgba_credit
                .try_adopt(decoded.into_rgba8().into_raw())
                .map_err(crate::DecodeError::Memory)?;
            RasterPixels::Rgba8(raw.into())
        };
        drop(decoded_credit);
        drop(standalone);
        let surface = DecodedRaster::new(frame.width, frame.height, pixels, color.clone())?;
        let prepared =
            crate::prepare_raster_for_display_with_budget_and_cancel(&surface, Some(budget), || {
                request.check_cancelled().is_err()
            })?;
        let RasterPixels::Rgba32Float(raw) = prepared.pixels() else {
            unreachable!()
        };
        let previous = if frame.dispose == 2 && index != request.image_index {
            Some(save_previous_rectangle(&canvas, w, frame, budget, request)?)
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
            2 => {
                let previous = previous.unwrap();
                if previous.len() == canvas.len() {
                    // Full-canvas PREVIOUS can transfer ownership instead of
                    // copying every sample back into the existing allocation.
                    canvas = previous;
                } else {
                    restore_previous_rectangle(&mut canvas, w, frame, &previous, request)?;
                }
            }
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
        RasterPixels::Rgba32Float(pixels.freeze().into()),
        RasterColorSpace::LinearSrgb,
    )?
    .with_pixel_aspect(declaration.pixel_aspect)?
    .with_animation_timing(Some(rrrah_core::RasterAnimationTiming::new(delay_ms_numerator, delay_ms_denominator, num_plays)?))
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
// PREVIOUS restores only the region this frame can modify. Preserve the
// shared canvas allocation instead of copying/replacing the whole image.
fn save_previous_rectangle(
    canvas: &[f32],
    width: u32,
    frame: &Control<'_>,
    budget: &rrrah_core::MemoryBudget,
    request: &DecodeRequest,
) -> Result<rrrah_core::MutableBuffer<f32>, RasterDecodeError> {
    request.check_cancelled()?;
    let stride = frame.width as usize * 4;
    let mut saved = budget
        .try_buffer(stride * frame.height as usize, 0f32)
        .map_err(crate::DecodeError::Memory)?;
    for y in 0..frame.height as usize {
        request.check_cancelled()?;
        let start = ((y + frame.y as usize) * width as usize + frame.x as usize) * 4;
        saved[y * stride..(y + 1) * stride].copy_from_slice(&canvas[start..start + stride]);
    }
    Ok(saved)
}
fn restore_previous_rectangle(
    canvas: &mut [f32],
    width: u32,
    frame: &Control<'_>,
    saved: &[f32],
    request: &DecodeRequest,
) -> Result<(), RasterDecodeError> {
    let stride = frame.width as usize * 4;
    for y in 0..frame.height as usize {
        request.check_cancelled()?;
        let start = ((y + frame.y as usize) * width as usize + frame.x as usize) * 4;
        canvas[start..start + stride].copy_from_slice(&saved[y * stride..(y + 1) * stride]);
    }
    Ok(())
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
    use std::sync::Arc;
    #[test]
    fn full_canvas_previous_restores_before_partial_over_frame() {
        use std::io::Write;
        let mut bytes = MAGIC.to_vec();
        append_chunk(&mut bytes, b"IHDR", &[0, 0, 0, 2, 0, 0, 0, 2, 8, 6, 0, 0, 0]);
        append_chunk(&mut bytes, b"sRGB", &[0]);
        append_chunk(&mut bytes, b"acTL", &[0, 0, 0, 3, 0, 0, 0, 0]);
        let mut sequence = 0u32;
        for (index, (size, pixel, dispose, blend)) in [
            (2u32, [255u8, 255, 255, 255], 0, 0),
            (2, [0, 255, 0, 255], 2, 0),
            (1, [0, 0, 255, 128], 0, 1),
        ]
        .into_iter()
        .enumerate()
        {
            let mut control = Vec::new();
            for v in [sequence, size, size, 0, 0] {
                control.extend_from_slice(&v.to_be_bytes());
            }
            sequence += 1;
            control.extend_from_slice(&[0, 1, 0, 10, dispose, blend]);
            append_chunk(&mut bytes, b"fcTL", &control);
            let mut z = flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::default());
            for _ in 0..size {
                z.write_all(&[0]).unwrap();
                for _ in 0..size {
                    z.write_all(&pixel).unwrap();
                }
            }
            let data = z.finish().unwrap();
            if index == 0 {
                append_chunk(&mut bytes, b"IDAT", &data);
            } else {
                let mut data_with_sequence = sequence.to_be_bytes().to_vec();
                sequence += 1;
                data_with_sequence.extend_from_slice(&data);
                append_chunk(&mut bytes, b"fdAT", &data_with_sequence);
            }
        }
        append_chunk(&mut bytes, b"IEND", &[]);
        let root = rrrah_core::MemoryBudget::new(4096);
        let mut request = DecodeRequest::new("full-previous.apng");
        request.image_index = 2;
        request.memory_budget = Some(root.clone());
        let result = decode(&bytes, &request).unwrap();
        let RasterPixels::Rgba32Float(values) = result.raster.pixels() else {
            panic!()
        };
        for (value, expected) in values[..4].iter().zip([127. / 255., 127. / 255., 1., 1.]) {
            assert!((value - expected).abs() < 1e-7);
        }
        assert!(values[4..].iter().all(|v| *v == 1.));
        assert_eq!(root.used(), 64);
        drop(result);
        assert_eq!(root.used(), 0);
    }
    #[test]
    #[ignore = "manual paired12MP APNG PREVIOUS storage benchmark"]
    fn paired_previous_rectangle_storage() {
        let length = 4000 * 3000 * 4;
        let old_root = rrrah_core::MemoryBudget::new(512 * 1024 * 1024);
        let patch_root = rrrah_core::MemoryBudget::new(512 * 1024 * 1024);
        let mut old_canvas = old_root.try_buffer(length, 0f32).unwrap();
        let mut patch_canvas = patch_root.try_buffer(length, 0f32).unwrap();
        for (index, (old, new)) in old_canvas.iter_mut().zip(patch_canvas.iter_mut()).enumerate() {
            let value = (index % 1021) as f32 / 1024.;
            *old = value;
            *new = value;
        }
        let frame = Control {
            width: 64,
            height: 64,
            x: 1973,
            y: 1417,
            delay_num: 1,
            delay_den: 10,
            dispose: 2,
            blend: 1,
            data: Vec::new(),
        };
        let request = DecodeRequest::new("benchmark.apng");
        let mutate = |canvas: &mut [f32]| {
            for y in 0..64 {
                let at = ((y + 1417) * 4000 + 1973) * 4;
                canvas[at..at + 64 * 4].fill(-1.);
            }
        };
        let mut pairs = Vec::new();
        for pair in 0..12 {
            let mut times = [0.; 2];
            for mode in if pair % 2 == 0 { [0, 1] } else { [1, 0] } {
                let start = std::time::Instant::now();
                if mode == 0 {
                    let mut saved = old_root.try_buffer(length, 0f32).unwrap();
                    saved.copy_from_slice(&old_canvas);
                    mutate(&mut old_canvas);
                    old_canvas = saved;
                } else {
                    let saved =
                        save_previous_rectangle(&patch_canvas, 4000, &frame, &patch_root, &request).unwrap();
                    mutate(&mut patch_canvas);
                    restore_previous_rectangle(&mut patch_canvas, 4000, &frame, &saved, &request).unwrap();
                    drop(saved);
                }
                times[mode] = start.elapsed().as_secs_f64() * 1000.;
            }
            assert_eq!((old_root.used(), patch_root.used()), (192_000_000, 192_000_000));
            if pair >= 3 {
                pairs.push(times);
            }
        }
        for (index, (old, new)) in old_canvas.iter().zip(patch_canvas.iter()).enumerate() {
            let expected = (index % 1021) as f32 / 1024.;
            assert_eq!(old.to_bits(), expected.to_bits());
            assert_eq!(new.to_bits(), expected.to_bits());
        }
        let old_peak = old_root.peak();
        let patch_peak = patch_root.peak();
        drop(old_canvas);
        drop(patch_canvas);
        assert_eq!((old_root.used(), patch_root.used()), (0, 0));
        println!(
            "APNG PREVIOUS paired12MP64x64: pairs_ms={pairs:?}; old_peak={old_peak}; patch_peak={patch_peak}; all48000000 components exact; final=0"
        );
    }
    #[test]
    fn previous_rectangle_preserves_surroundings_with_only_patch_credit() {
        let root = rrrah_core::MemoryBudget::new(4 * 3 * 4 * 4 + 2 * 2 * 4 * 4);
        let mut canvas = root.try_buffer(4 * 3 * 4, 0f32).unwrap();
        for (i, v) in canvas.iter_mut().enumerate() {
            *v = i as f32;
        }
        let frame = Control {
            width: 2,
            height: 2,
            x: 1,
            y: 1,
            delay_num: 1,
            delay_den: 10,
            dispose: 2,
            blend: 0,
            data: Vec::new(),
        };
        let request = DecodeRequest::new("patch.apng");
        let saved = save_previous_rectangle(&canvas, 4, &frame, &root, &request).unwrap();
        assert_eq!(
            &*saved,
            &[
                20., 21., 22., 23., 24., 25., 26., 27., 36., 37., 38., 39., 40., 41., 42., 43.
            ]
        );
        assert_eq!(root.used(), 256);
        canvas[20..28].fill(-1.);
        canvas[36..44].fill(-2.);
        restore_previous_rectangle(&mut canvas, 4, &frame, &saved, &request).unwrap();
        for (i, v) in canvas.iter().enumerate() {
            assert_eq!(*v, i as f32);
        }
        drop(saved);
        assert_eq!(root.used(), 192);
        drop(canvas);
        assert_eq!(root.used(), 0);
    }
    #[test]
    fn packed_grayscale_and_palette_frames_preserve_samples_and_transparency() {
        let cases: [(&[u8], [[u8; 4]; 4]); 3] = [
            (
                include_bytes!("../../../tests/fixtures/raster/apng-1bit-packed.png"),
                [
                    [0, 0, 0, 255],
                    [255, 255, 255, 255],
                    [0, 0, 0, 255],
                    [255, 255, 255, 255],
                ],
            ),
            (
                include_bytes!("../../../tests/fixtures/raster/apng-2bit-packed.png"),
                [
                    [255, 0, 0, 0],
                    [0, 255, 0, 64],
                    [0, 0, 255, 128],
                    [255, 255, 255, 255],
                ],
            ),
            (
                include_bytes!("../../../tests/fixtures/raster/apng-4bit-packed.png"),
                [
                    [0, 0, 0, 255],
                    [85, 85, 85, 255],
                    [170, 170, 170, 255],
                    [255, 255, 255, 255],
                ],
            ),
        ];
        for (bytes, expected) in cases {
            let root = rrrah_core::MemoryBudget::new(4096);
            let mut request = DecodeRequest::new("packed.apng");
            request.memory_budget = Some(root.clone());
            for index in 0..2 {
                request.image_index = index;
                let image = decode(bytes, &request).unwrap();
                let RasterPixels::Rgba32Float(values) = image.raster.pixels() else {
                    panic!()
                };
                for x in 0..4 {
                    let samples = expected[if index == 0 { x } else { 3 - x }];
                    for channel in 0..4 {
                        let encoded = f64::from(samples[channel]) / 255.;
                        let wanted = if channel == 3 {
                            encoded
                        } else if encoded <= 0.04045 {
                            encoded / 12.92
                        } else {
                            ((encoded + 0.055) / 1.055).powf(2.4)
                        };
                        assert!((f64::from(values[x * 4 + channel]) - wanted).abs() < 2e-7);
                    }
                }
                assert_eq!(
                    (image.raster.image_index(), image.raster.image_count()),
                    (index, 2)
                );
                assert_eq!(root.used(), 64);
                drop(image);
                assert_eq!(root.used(), 0);
            }
        }
    }
    #[test]
    fn sixteen_bit_selected_frames_keep_low_bits_and_alpha() {
        let bytes = include_bytes!("../../../tests/fixtures/raster/apng-16bit-precision.png");
        let root = rrrah_core::MemoryBudget::new(4096);
        let mut request = DecodeRequest::new("precision.apng");
        request.memory_budget = Some(root.clone());
        for (index, samples) in [[257u16, 12345, 32769, 65534], [258, 12346, 32770, 65533]]
            .into_iter()
            .enumerate()
        {
            request.image_index = index;
            let image = decode(bytes, &request).unwrap();
            let RasterPixels::Rgba32Float(values) = image.raster.pixels() else {
                panic!()
            };
            for channel in 0..4 {
                let encoded = f64::from(samples[channel]) / 65535.;
                let expected = if channel == 3 {
                    encoded
                } else if encoded <= 0.04045 {
                    encoded / 12.92
                } else {
                    ((encoded + 0.055) / 1.055).powf(2.4)
                };
                assert!(
                    (f64::from(values[channel]) - expected).abs() < 1e-7,
                    "frame {index} channel {channel}: {} != {expected}",
                    values[channel]
                );
            }
            assert_eq!(
                (image.raster.image_index(), image.raster.image_count()),
                (index, 2)
            );
            assert_eq!(root.used(), 16);
            drop(image);
            assert_eq!(root.used(), 0);
        }
    }
    #[test]
    fn canvas_credit_follows_last_output_reference_and_refusal_releases_scratch() {
        let bytes = include_bytes!("../../../tests/fixtures/raster/apng-compose-0-0.apng");
        let budget = rrrah_core::MemoryBudget::new(1024 * 1024);
        let mut request = DecodeRequest::new("synthetic.apng");
        request.memory_budget = Some(budget.clone());
        request.image_index = 2;
        let image = decode(bytes, &request).unwrap();
        assert_eq!(budget.used(), 5 * 3 * 4 * 4);
        let retained = image.raster.clone();
        drop(image);
        assert_eq!(budget.used(), 240);
        drop(retained);
        assert_eq!(budget.used(), 0);
        let tiny = rrrah_core::MemoryBudget::new(239);
        request.memory_budget = Some(tiny.clone());
        assert!(decode(bytes, &request).is_err());
        assert_eq!(tiny.used(), 0);
        let canvas_only = rrrah_core::MemoryBudget::new(240);
        request.memory_budget = Some(canvas_only.clone());
        assert!(decode(bytes, &request).is_err());
        assert_eq!(canvas_only.used(), 0);
        // Sweep boundaries through encoded reconstruction, decoder output,
        // RGBA adoption, linear preparation and PREVIOUS admission.
        let mut refused = 0;
        let mut admitted = 0;
        for limit in (240..=2048).step_by(16) {
            let root = rrrah_core::MemoryBudget::new(limit);
            request.memory_budget = Some(root.clone());
            match decode(bytes, &request) {
                Ok(image) => {
                    admitted += 1;
                    assert_eq!(root.used(), 240);
                    drop(image);
                }
                Err(_) => refused += 1,
            }
            assert_eq!(root.used(), 0, "limit {limit}");
            assert!(root.peak() <= limit);
        }
        assert!(refused > 0 && admitted > 0);
    }
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
            let prepared = crate::prepare_raster_for_display(&image.raster).unwrap();
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
    fn chunk_ownership_and_empty_frames_refuse_before_pixel_admission() {
        let bytes = include_bytes!("../../../tests/fixtures/raster/apng-compose-0-0.apng");
        let mut chunks = Vec::new();
        let mut at = 8;
        while at < bytes.len() {
            let n = u32::from_be_bytes(bytes[at..at + 4].try_into().unwrap()) as usize;
            chunks.push((bytes[at + 4..at + 8].to_vec(), bytes[at + 8..at + 8 + n].to_vec()));
            at += n + 12;
        }
        let first_idat = chunks.iter().position(|(k, _)| k == b"IDAT").unwrap();
        let controls: Vec<_> = chunks
            .iter()
            .enumerate()
            .filter(|(_, (k, _))| k == b"fcTL")
            .map(|(i, _)| i)
            .collect();
        // A separate static preview is legal: move the first fcTL after IDAT
        // and supply its animation data in fdAT, then renumber animation chunks.
        let mut preview = chunks.clone();
        let control = preview.remove(controls[0]);
        let idat = preview.iter().position(|(k, _)| k == b"IDAT").unwrap();
        let payload = preview[idat].1.clone();
        preview.insert(idat + 1, control);
        let mut fdat = vec![0; 4];
        fdat.extend_from_slice(&payload);
        preview.insert(idat + 2, (b"fdAT".to_vec(), fdat));
        let mut sequence = 0u32;
        let mut valid = MAGIC.to_vec();
        for (kind, mut data) in preview {
            if kind == b"fcTL" || kind == b"fdAT" {
                data[..4].copy_from_slice(&sequence.to_be_bytes());
                sequence += 1;
            }
            // Multiple consecutive IDAT pieces must remain accepted.
            if kind == b"IDAT" {
                let mid = data.len() / 2;
                append_chunk(&mut valid, b"IDAT", &data[..mid]);
                append_chunk(&mut valid, b"IDAT", &data[mid..]);
            } else {
                append_chunk(&mut valid, kind.as_slice().try_into().unwrap(), &data);
            }
        }
        let mut request = DecodeRequest::new("preview.apng");
        request.image_index = 3;
        let actual = decode(&valid, &request).unwrap();
        let expected = decode(bytes, &request).unwrap();
        let (RasterPixels::Rgba32Float(a), RasterPixels::Rgba32Float(b)) =
            (actual.raster.pixels(), expected.raster.pixels())
        else {
            panic!()
        };
        assert_eq!(a, b);
        assert_eq!(actual.raster.image_count(), 4);
        let mut cases = Vec::new();
        let mut wrong = chunks.clone();
        let extra = wrong[first_idat].clone();
        wrong.insert(controls[1] + 1, extra);
        cases.push(wrong);
        let mut wrong = chunks.clone();
        wrong.swap(first_idat, controls[1]);
        cases.push(wrong);
        let mut wrong = chunks.clone();
        wrong[controls[0]].1[4..8].copy_from_slice(&1u32.to_be_bytes());
        cases.push(wrong);
        let mut wrong = chunks.clone();
        wrong.truncate(controls[3] + 1);
        wrong.push((b"IEND".to_vec(), Vec::new()));
        cases.push(wrong);
        for chunks in cases {
            let mut bytes = MAGIC.to_vec();
            for (kind, data) in chunks {
                append_chunk(&mut bytes, kind.as_slice().try_into().unwrap(), &data);
            }
            let root = rrrah_core::MemoryBudget::new(4096);
            let mut request = DecodeRequest::new("invalid.apng");
            request.image_index = 3;
            request.memory_budget = Some(root.clone());
            assert!(matches!(
                decode(&bytes, &request),
                Err(RasterDecodeError::InvalidApng(_))
            ));
            assert_eq!((root.used(), root.peak()), (0, 0));
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
