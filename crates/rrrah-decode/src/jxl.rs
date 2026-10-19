//! Selected composited JPEG XL frames with native precision and rational timing.
use crate::{
    DecodeRequest,
    raster::{MAX_RASTER_BYTES, RasterDecodeError},
};
use rrrah_core::{DecodedRaster, RasterColorSpace, RasterPixels};
use std::{
    io::{Cursor, Read},
    sync::Arc,
};
pub(crate) fn has_magic(bytes: &[u8]) -> bool {
    bytes.starts_with(&[0xff, 0x0a]) || bytes.starts_with(b"\0\0\0\x0cJXL \r\n\x87\n")
}
fn invalid(message: &'static str) -> RasterDecodeError {
    image::ImageError::IoError(std::io::Error::new(std::io::ErrorKind::InvalidData, message)).into()
}
fn codec_error(error: Box<dyn std::error::Error + Send + Sync>) -> RasterDecodeError {
    match error.downcast::<crate::DecodeError>() {
        Ok(error) => (*error).into(),
        Err(error) => image::ImageError::Decoding(image::error::DecodingError::new(
            image::error::ImageFormatHint::Unknown,
            error,
        ))
        .into(),
    }
}
fn samples<T: jxl_oxide::FrameBufferSample + Default + Copy>(
    render: &jxl_oxide::Render,
    opaque: T,
    request: &DecodeRequest,
) -> Result<Vec<T>, RasterDecodeError> {
    let mut stream = render.stream();
    let channels = stream.channels() as usize;
    if !(1..=4).contains(&channels) {
        return Err(invalid("unsupported JPEG XL pixel layout"));
    }
    let width = stream.width() as usize;
    let height = stream.height() as usize;
    // One source row fits within the separately admitted output-equivalent scratch.
    let mut row = vec![T::default(); width * channels];
    let mut rgba = Vec::new();
    rgba.try_reserve_exact(width * height * 4)
        .map_err(|_| RasterDecodeError::OutputTooLarge)?;
    for _ in 0..height {
        request.check_cancelled()?;
        if stream.write_to_buffer(&mut row) != row.len() {
            return Err(invalid("incomplete JPEG XL pixel stream"));
        }
        for pixel in row.chunks_exact(channels) {
            match channels {
                1 => rgba.extend_from_slice(&[pixel[0], pixel[0], pixel[0], opaque]),
                2 => rgba.extend_from_slice(&[pixel[0], pixel[0], pixel[0], pixel[1]]),
                3 => rgba.extend_from_slice(&[pixel[0], pixel[1], pixel[2], opaque]),
                _ => rgba.extend_from_slice(pixel),
            }
        }
    }
    Ok(rgba)
}
fn timing(
    ticks: u32,
    numerator: u32,
    denominator: u32,
    plays: u32,
) -> Result<rrrah_core::RasterAnimationTiming, RasterDecodeError> {
    if numerator == 0 || denominator == 0 {
        return Err(invalid("invalid JPEG XL tick rate"));
    }
    let mut n = u128::from(ticks) * u128::from(denominator) * 1000;
    let mut d = u128::from(numerator);
    let (mut a, mut b) = (n, d);
    while b != 0 {
        (a, b) = (b, a % b);
    }
    n /= a;
    d /= a;
    let n = u32::try_from(n).map_err(|_| invalid("JPEG XL duration exceeds rational range"))?;
    let d = u32::try_from(d).map_err(|_| invalid("JPEG XL duration exceeds rational range"))?;
    Ok(rrrah_core::RasterAnimationTiming::new(n, d, plays)?)
}
pub(crate) fn decode(bytes: &[u8], request: &DecodeRequest) -> Result<DecodedRaster, RasterDecodeError> {
    request.check_cancelled()?;
    // Header-only initialization cannot allocate tracked codec groups. ICC byte
    // buffers are admitted separately; parser metadata is outside this tracker.
    let tracker = jxl_oxide::AllocTracker::with_limit(0);
    // Declared before codec state: buffers must drop before their working credit,
    // including every early return after admission.
    let _working;
    // Must outlive codec state, including every partial initialization attempt.
    let mut header_credit = request
        .memory_budget
        .as_ref()
        .map(|root| root.try_reserve(0))
        .transpose()
        .map_err(crate::DecodeError::Memory)?;
    let mut icc_credit = Vec::new();
    let mut icc_admitted = 0u64;
    let mut uninit = jxl_oxide::JxlImage::builder()
        .alloc_tracker(tracker.clone())
        .build_uninit();
    let mut reader = Cursor::new(bytes);
    let mut buffer = [0u8; 4096];
    let mut valid = 0;
    let mut image = loop {
        request.check_cancelled()?;
        let count = reader
            .read(&mut buffer[valid..])
            .map_err(image::ImageError::IoError)?;
        if count == 0 {
            return Err(invalid("incomplete JPEG XL header"));
        }
        valid += count;
        let consumed = uninit
            .feed_bytes_with_header_admission(&buffer[..valid], |bytes| {
                request.check_cancelled()?;
                if let Some(credit) = &mut header_credit {
                    credit.ensure_bytes(bytes).map_err(crate::DecodeError::Memory)?;
                }
                Ok(())
            })
            .map_err(codec_error)?;
        buffer.copy_within(consumed..valid, 0);
        valid -= consumed;
        match uninit
            .try_init_header_only_with_icc_admission(|bytes| {
                request.check_cancelled()?;
                if bytes > icc_admitted {
                    if let Some(root) = &request.memory_budget {
                        icc_credit.push(
                            root.try_reserve(bytes - icc_admitted)
                                .map_err(crate::DecodeError::Memory)?,
                        );
                    }
                    icc_admitted = bytes;
                }
                Ok(())
            })
            .map_err(codec_error)?
        {
            jxl_oxide::InitializeResult::Initialized(image) => break image,
            jxl_oxide::InitializeResult::NeedMoreData(next) => uninit = next,
        }
    };
    request.check_cancelled()?;
    if image.pixel_format().has_black() {
        image.request_color_encoding(jxl_oxide::EnumColourEncoding::srgb(
            jxl_oxide::RenderingIntent::Relative,
        ));
    }
    let (width, height) = (image.width(), image.height());
    let metadata = &image.image_header().metadata;
    let depths = std::iter::once(metadata.bit_depth).chain(
        metadata
            .ec_info
            .iter()
            .filter(|e| matches!(e.ty, jxl_oxide::ExtraChannelType::Alpha { .. }))
            .map(|e| e.bit_depth),
    );
    let mut sample_bytes = 1u64;
    for depth in depths {
        let size = match depth {
            jxl_oxide::image::BitDepth::FloatSample { .. } => 4,
            _ if depth.bits_per_sample() > 16 => 4,
            _ if depth.bits_per_sample() > 8 => 2,
            _ => 1,
        };
        sample_bytes = sample_bytes.max(size);
    }
    // Validate dimensions before multiplying untrusted header sizes.
    if width > 65536 || height > 65536 {
        return Err(RasterDecodeError::OutputTooLarge);
    }
    // Feeding all animation headers also retains compressed frame groups in the
    // codec. Their valid total cannot exceed this source span. Admit that copy
    // separately from the caller-owned source and decoded-grid working bound.
    let encoded_history = if metadata.animation.is_some() {
        bytes.len() as u64
    } else {
        0
    };
    let output_bytes = u64::from(width) * u64::from(height) * 4 * sample_bytes;
    if output_bytes > MAX_RASTER_BYTES {
        return Err(RasterDecodeError::OutputTooLarge);
    }
    let output_reservation = request
        .memory_budget
        .as_ref()
        .map(|b| b.try_reserve(output_bytes))
        .transpose()
        .map_err(crate::DecodeError::Memory)?;
    let codec_bytes = (65536 + u64::from(width) * u64::from(height) * 64)
        .checked_add(encoded_history)
        .ok_or(RasterDecodeError::OutputTooLarge)?
        .min(MAX_RASTER_BYTES);
    _working = request
        .memory_budget
        .as_ref()
        .map(|b| b.try_reserve(codec_bytes + output_bytes))
        .transpose()
        .map_err(crate::DecodeError::Memory)?;
    tracker.expand_limit(codec_bytes as usize);
    request.check_cancelled()?;
    image.resume_frame_loading().map_err(codec_error)?;
    // Deferred initial codestream Vec has now been consumed and dropped.
    drop(header_credit);
    while !image.is_loading_done() {
        request.check_cancelled()?;
        if valid > 0 {
            let consumed = image.feed_bytes(&buffer[..valid]).map_err(codec_error)?;
            buffer.copy_within(consumed..valid, 0);
            valid -= consumed;
            if image.is_loading_done() {
                break;
            }
        }
        let count = reader
            .read(&mut buffer[valid..])
            .map_err(image::ImageError::IoError)?;
        if count == 0 {
            return Err(invalid("incomplete JPEG XL frames"));
        }
        valid += count;
    }
    image.finalize().map_err(codec_error)?;
    request.check_cancelled()?;
    let count = image.num_loaded_keyframes();
    if request.image_index >= count {
        return Err(crate::DecodeError::UnsupportedImageIndex {
            index: request.image_index,
        }
        .into());
    }
    let render = image.render_frame(request.image_index).map_err(codec_error)?;
    request.check_cancelled()?;
    let animation = image
        .image_header()
        .metadata
        .animation
        .as_ref()
        .map(|a| timing(render.duration(), a.tps_numerator, a.tps_denominator, a.num_loops))
        .transpose()?;
    let mut profile_reservation = None;
    let color = RasterColorSpace::Icc(
        image
            .rendered_icc_with_admission(|bytes| {
                request.check_cancelled()?;
                if let Some(root) = &request.memory_budget {
                    profile_reservation = Some(root.try_reserve(bytes).map_err(crate::DecodeError::Memory)?);
                }
                Ok(())
            })
            .map_err(codec_error)?,
    );
    macro_rules! pixels {
        ($ty:ty,$opaque:expr,$variant:ident) => {{
            let values = samples::<$ty>(&render, $opaque, request)?;
            RasterPixels::$variant(if let Some(reservation) = output_reservation {
                reservation
                    .try_adopt(values)
                    .map_err(crate::DecodeError::Memory)?
                    .into()
            } else {
                Arc::new(values).into()
            })
        }};
    }
    let pixels = match sample_bytes {
        2 => pixels!(u16, u16::MAX, Rgba16),
        4 => pixels!(f32, 1.0, Rgba32Float),
        _ => pixels!(u8, u8::MAX, Rgba8),
    };
    request.check_cancelled()?;
    let frame = DecodedRaster::new(width, height, pixels, color)?
        .with_animation_timing(animation)
        .with_image_selection(request.image_index, count)?;
    match profile_reservation {
        Some(credit) => frame
            .with_color_profile_reservation(credit)
            .map_err(|e| crate::DecodeError::Memory(e).into()),
        None => Ok(frame),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn header_initialization_does_not_allocate_codec_frame_groups_before_admission() {
        for bytes in [
            include_bytes!("../../../tests/fixtures/raster/alpha-container-0.jxl").as_slice(),
            include_bytes!("../../../tests/fixtures/raster/alpha-container-1.jxl").as_slice(),
            include_bytes!("../../../tests/fixtures/raster/animation-rgb.jxl").as_slice(),
            include_bytes!("../../../tests/fixtures/raster/animation-finite16.jxl").as_slice(),
            include_bytes!("../../../tests/fixtures/raster/animation-linear-float.jxl").as_slice(),
            include_bytes!("../../../tests/fixtures/raster/animation-scale-600.jxl").as_slice(),
        ] {
            for mode in 0..3 {
                let tracker = jxl_oxide::AllocTracker::with_limit(0);
                let mut uninit = jxl_oxide::JxlImage::builder()
                    .alloc_tracker(tracker.clone())
                    .build_uninit();
                uninit.feed_bytes(bytes).unwrap();
                let jxl_oxide::InitializeResult::Initialized(mut image) =
                    uninit.try_init_header_only().unwrap()
                else {
                    panic!("complete header needed")
                };
                assert_eq!(image.num_loaded_frames(), 0);
                assert!(!image.is_loading_done());
                let root = rrrah_core::MemoryBudget::new(1024 * 1024);
                let credit = root.try_reserve(root.limit()).unwrap();
                tracker.expand_limit(root.limit() as usize);
                match mode {
                    0 => {
                        image.resume_frame_loading().unwrap();
                        image.resume_frame_loading().unwrap();
                    }
                    1 => {
                        image.feed_bytes(&[]).unwrap();
                    }
                    _ => {
                        image.finalize().unwrap();
                    }
                }
                assert!(image.is_loading_done());
                let count = image.num_loaded_keyframes();
                assert!(count > 0);
                assert!(image.render_frame(count - 1).is_ok());
                drop(image);
                drop(credit);
                assert_eq!(root.used(), 0);
            }
        }
        // Existing callers retain the eager API; zero codec storage still
        // refuses a buffered payload rather than silently ignoring its limit.
        let tracker = jxl_oxide::AllocTracker::with_limit(0);
        let mut eager = jxl_oxide::JxlImage::builder()
            .alloc_tracker(tracker)
            .build_uninit();
        eager
            .feed_bytes(include_bytes!(
                "../../../tests/fixtures/raster/alpha-container-1.jxl"
            ))
            .unwrap();
        assert!(eager.try_init().is_err());
    }
    #[test]
    fn long_animation_history_is_admitted_and_native_selections_survive_tight_parent_budget() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../tests/fixtures/raster/animation-scale-600.jxl");
        let root = rrrah_core::MemoryBudget::new(256 * 1024);
        for index in [0, 300, 599] {
            let mut request = DecodeRequest::new(&path);
            request.image_index = index;
            request.memory_budget = Some(root.clone());
            let raster = crate::decode_raster(&request).unwrap();
            assert_eq!((raster.image_index(), raster.image_count()), (index, 600));
            let RasterPixels::Rgba16(values) = raster.pixels() else {
                panic!()
            };
            let expected = std::fs::read(format!("{}.{index}.rgba16le", path.display())).unwrap();
            assert_eq!(
                values.iter().flat_map(|v| v.to_le_bytes()).collect::<Vec<_>>(),
                expected
            );
            drop(raster);
            assert_eq!(root.used(), 0);
            let peak = root.peak();
            let short = rrrah_core::MemoryBudget::new(peak - 1);
            request.memory_budget = Some(short.clone());
            assert!(matches!(
                crate::decode_raster(&request),
                Err(RasterDecodeError::Source(crate::DecodeError::Memory(_)))
            ));
            assert_eq!(short.used(), 0);
            let exact = rrrah_core::MemoryBudget::new(peak);
            request.memory_budget = Some(exact.clone());
            let retry = crate::decode_raster(&request).unwrap();
            assert_eq!(retry.image_count(), 600);
            drop(retry);
            assert_eq!(exact.used(), 0);
        }
        eprintln!(
            "JXL_HISTORY frames=600 selections=3 root_peak={} limit={} final=0",
            root.peak(),
            root.limit()
        );
    }
    #[test]
    fn embedded_icc_working_storage_is_admitted_before_initialization_and_retries() {
        let bytes = include_bytes!("../../../tests/fixtures/raster/animation-embedded-icc.jxl");
        let profile = include_bytes!("../../../tests/fixtures/raster/linear-identity-rgb.icc");
        let mut events = Vec::new();
        let mut uninit = jxl_oxide::JxlImage::builder()
            .alloc_tracker(jxl_oxide::AllocTracker::with_limit(0))
            .build_uninit();
        let mut header_requests = Vec::new();
        uninit
            .feed_bytes_with_header_admission(bytes, |n| {
                header_requests.push(n);
                Ok(())
            })
            .unwrap();
        let jxl_oxide::InitializeResult::Initialized(image) = uninit
            .try_init_header_only_with_icc_admission(|n| {
                events.push(n);
                Ok(())
            })
            .unwrap()
        else {
            panic!()
        };
        assert_eq!(image.num_loaded_frames(), 0);
        assert_eq!(image.original_icc().unwrap(), profile);
        drop(image);
        assert!(events.len() > 2);
        assert!(events[2..].iter().all(|&n| n == events[1]));
        assert_eq!(events[1] - events[0], 2 * profile.len() as u64);
        let (first, high) = (events[0], events[1]);
        // Seven-byte feeds repeatedly retry the entropy/header parser. Only
        // increases of the high-water reservation may consume additional credit.
        let root = rrrah_core::MemoryBudget::new(high);
        let mut credit = Vec::new();
        let mut admitted = 0;
        let mut calls = 0;
        let mut uninit = jxl_oxide::JxlImage::builder()
            .alloc_tracker(jxl_oxide::AllocTracker::with_limit(0))
            .build_uninit();
        let mut initialized = None;
        let mut pending = Vec::new();
        for chunk in bytes.chunks(7) {
            pending.extend_from_slice(chunk);
            let consumed = uninit.feed_bytes(&pending).unwrap();
            pending.drain(..consumed);
            match uninit
                .try_init_header_only_with_icc_admission(|n| {
                    calls += 1;
                    if n > admitted {
                        credit.push(root.try_reserve(n - admitted)?);
                        admitted = n;
                    }
                    Ok(())
                })
                .unwrap()
            {
                jxl_oxide::InitializeResult::NeedMoreData(next) => uninit = next,
                jxl_oxide::InitializeResult::Initialized(image) => {
                    initialized = Some(image);
                    break;
                }
            }
        }
        let image = initialized.unwrap();
        assert!(calls > 2);
        assert_eq!(image.num_loaded_frames(), 0);
        assert_eq!(image.original_icc().unwrap(), profile);
        assert_eq!(root.used(), high);
        assert_eq!(root.peak(), high);
        drop(image);
        drop(credit);
        assert_eq!(root.used(), 0);
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../tests/fixtures/raster/animation-embedded-icc.jxl");
        let source_bytes = bytes.len() as u64;
        // Container parsing can emit multiple codestream parts from one feed.
        // Include their measured capacity/reallocation high water, not file len.
        let held_source_and_header = source_bytes + header_requests.into_iter().max().unwrap();
        for (limit, requested, used) in [
            (held_source_and_header + first - 1, first, held_source_and_header),
            (
                held_source_and_header + high - 1,
                high - first,
                held_source_and_header + first,
            ),
        ] {
            let root = rrrah_core::MemoryBudget::new(limit);
            let mut request = DecodeRequest::new(&path);
            request.memory_budget = Some(root.clone());
            let result = crate::decode_raster(&request);
            assert!(
                matches!(result,Err(RasterDecodeError::Source(crate::DecodeError::Memory(rrrah_core::BufferError::Capacity{requested:r,used:u,..}))) if r==requested && u==used)
            );
            assert_eq!(root.used(), 0);
            assert_eq!(root.peak(), used);
        }
        let root = rrrah_core::MemoryBudget::new(1024 * 1024);
        let mut request = DecodeRequest::new(&path);
        request.image_index = 2;
        request.memory_budget = Some(root.clone());
        drop(crate::decode_raster(&request).unwrap());
        let peak = root.peak();
        assert_eq!(root.used(), 0);
        for limit in [peak - 1, peak] {
            let root = rrrah_core::MemoryBudget::new(limit);
            request.memory_budget = Some(root.clone());
            let result = crate::decode_raster(&request);
            if limit < peak {
                assert!(matches!(
                    result,
                    Err(RasterDecodeError::Source(crate::DecodeError::Memory(_)))
                ));
            } else {
                assert!(result.is_ok());
            }
            drop(result);
            assert_eq!(root.used(), 0);
        }
        eprintln!(
            "JXL_ICC_ADMISSION first={first} working={high} header_calls={calls} public_peak={peak} final=0"
        );
    }
    #[test]
    fn truncated_embedded_icc_releases_preinitialization_credit() {
        let bytes = include_bytes!("../../../tests/fixtures/raster/animation-embedded-icc.jxl");
        let mut header_failures = 0;
        for end in 1..bytes.len() {
            let root = rrrah_core::MemoryBudget::new(1024 * 1024);
            let mut request = DecodeRequest::new("truncated-icc.jxl");
            request.memory_budget = Some(root.clone());
            let result = decode(&bytes[..end], &request);
            if result.is_err() && root.peak() > 0 && root.peak() < 65536 {
                header_failures += 1;
            }
            drop(result);
            assert_eq!(root.used(), 0, "prefix {end}");
        }
        assert!(
            header_failures > 0,
            "must exercise errors after ICC admission but before frame admission"
        );
        eprintln!(
            "JXL_ICC_TRUNCATION prefixes={} admitted_header_failures={header_failures} final=0",
            bytes.len() - 1
        );
    }
    #[test]
    fn embedded_icc_animation_preserves_independent_profile_and_native_samples() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../tests/fixtures/raster/animation-embedded-icc.jxl");
        let bytes = std::fs::read(&path).unwrap();
        let image = jxl_oxide::JxlImage::builder().read(Cursor::new(&bytes)).unwrap();
        assert!(image.image_header().metadata.colour_encoding.want_icc());
        let profile = include_bytes!("../../../tests/fixtures/raster/linear-identity-rgb.icc");
        assert_eq!(image.original_icc().unwrap(), profile);
        // jxl-oxide may describe a recognized profile with synthesized ICC bytes.
        let rendered_profile = image.rendered_icc();
        drop(image);
        let root = rrrah_core::MemoryBudget::new(1024 * 1024);
        for index in 0..3 {
            let mut request = DecodeRequest::new(&path);
            request.image_index = index;
            request.memory_budget = Some(root.clone());
            let frame = crate::decode_raster(&request).unwrap();
            let RasterPixels::Rgba16(values) = frame.pixels() else {
                panic!("native precision lost")
            };
            let reference = std::fs::read(format!("{}.{index}.rgba16le", path.display())).unwrap();
            let expected: Vec<u16> = reference
                .chunks_exact(2)
                .map(|b| u16::from_le_bytes(b.try_into().unwrap()))
                .collect();
            assert_eq!(values.as_ref(), expected);
            assert_eq!(
                frame.color_space(),
                &RasterColorSpace::Icc(rendered_profile.clone())
            );
            assert_eq!((frame.image_index(), frame.image_count()), (index, 3));
            let prepared = crate::prepare_raster_for_display_with_budget(&frame, Some(&root)).unwrap();
            assert_eq!(frame.animation_timing(), prepared.animation_timing());
            drop((prepared, frame));
            assert_eq!(root.used(), 0);
        }
    }
    #[test]
    fn float_animation_preserves_independent_bits_and_signed_linear_display_range() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../tests/fixtures/raster/animation-linear-float.jxl");
        let root = rrrah_core::MemoryBudget::new(1024 * 1024);
        for index in 0..3 {
            let mut request = DecodeRequest::new(&path);
            request.image_index = index;
            request.memory_budget = Some(root.clone());
            let frame = crate::decode_raster(&request).unwrap();
            let RasterPixels::Rgba32Float(values) = frame.pixels() else {
                panic!("float precision lost")
            };
            let bytes = std::fs::read(format!("{}.{index}.rgba32fle", path.display())).unwrap();
            let expected: Vec<u32> = bytes
                .chunks_exact(4)
                .map(|b| u32::from_le_bytes(b.try_into().unwrap()))
                .collect();
            assert_eq!(values.iter().map(|v| v.to_bits()).collect::<Vec<_>>(), expected);
            let RasterColorSpace::Icc(profile) = frame.color_space() else {
                panic!()
            };
            assert_eq!(
                profile.as_slice(),
                include_bytes!("../../../tests/fixtures/raster/animation-linear-float.icc")
            );
            let ready = crate::prepare_raster_for_display_with_budget(&frame, Some(&root)).unwrap();
            let RasterPixels::Rgba32Float(prepared) = ready.pixels() else {
                panic!()
            };
            assert!(prepared.chunks_exact(4).any(|p| p[..3].iter().any(|v| *v > 8.)));
            assert!(prepared.chunks_exact(4).any(|p| p[..3].iter().any(|v| *v < -1.)));
            for (a, b) in values.chunks_exact(4).zip(prepared.chunks_exact(4)) {
                assert_eq!(a[3].to_bits(), b[3].to_bits());
            }
            let reference = std::fs::read(format!("{}.{index}.linear-rgb64le", path.display())).unwrap();
            let mut maximum = 0f64;
            for pixel in 0..15 {
                for channel in 0..3 {
                    let offset = (pixel * 3 + channel) * 8;
                    let expected = f64::from_le_bytes(reference[offset..offset + 8].try_into().unwrap());
                    let error = (f64::from(prepared[pixel * 4 + channel]) - expected).abs();
                    maximum = maximum.max(error);
                    assert!(
                        error <= 2e-5,
                        "frame {index} pixel {pixel} channel {channel}: {error}"
                    );
                }
            }
            eprintln!("JXL_FLOAT_LINEAR frame={index} lcms_max_error={maximum} bound=0.00002");
            assert_eq!(frame.animation_timing(), ready.animation_timing());
            drop((ready, frame));
            assert_eq!(root.used(), 0);
        }
    }
    #[test]
    fn finite_rgba16_animation_matches_libjxl_samples_and_fractional_timing() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../tests/fixtures/raster/animation-finite16.jxl");
        let budget = rrrah_core::MemoryBudget::new(1024 * 1024);
        for (index, (n, d)) in [(1001, 8), (7007, 24), (11011, 24)].into_iter().enumerate() {
            let mut request = DecodeRequest::new(&path);
            request.image_index = index;
            request.memory_budget = Some(budget.clone());
            let raster = crate::decode_raster(&request).unwrap();
            let RasterPixels::Rgba16(pixels) = raster.pixels() else {
                panic!("16bit animation was reduced")
            };
            let bytes = std::fs::read(format!("{}.{index}.rgba16le", path.display())).unwrap();
            let expected: Vec<u16> = bytes
                .chunks_exact(2)
                .map(|b| u16::from_le_bytes([b[0], b[1]]))
                .collect();
            assert_eq!(pixels.as_ref(), expected);
            let timing = raster.animation_timing().unwrap();
            assert_eq!(
                (
                    timing.delay_ms_numerator(),
                    timing.delay_ms_denominator(),
                    timing.num_plays()
                ),
                (n, d, 2)
            );
            assert_eq!((raster.image_index(), raster.image_count()), (index, 3));
            let prepared = crate::prepare_raster_for_display_with_budget(&raster, Some(&budget)).unwrap();
            assert_eq!(prepared.animation_timing(), Some(timing));
            drop((prepared, raster));
            assert_eq!(budget.used(), 0);
        }
    }
    #[test]
    fn selected_animation_frames_match_independent_oracles_and_release_owners() {
        let root_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/raster");
        for name in ["animation-rgb.jxl", "animation-alpha.jxl"] {
            for (index, delay) in [40, 90, 130].into_iter().enumerate() {
                let budget = rrrah_core::MemoryBudget::new(1024 * 1024);
                let mut request = DecodeRequest::new(root_path.join(name));
                request.image_index = index;
                request.memory_budget = Some(budget.clone());
                let frame = crate::decode_raster(&request).unwrap();
                assert_eq!((frame.image_index(), frame.image_count()), (index, 3));
                let timing = frame.animation_timing().unwrap();
                assert_eq!(timing.delay(), std::time::Duration::from_millis(delay));
                assert_eq!(timing.num_plays(), 0);
                assert!(matches!(frame.color_space(), RasterColorSpace::Icc(_)));
                let RasterPixels::Rgba8(actual) = frame.pixels() else {
                    panic!()
                };
                let expected = std::fs::read(root_path.join(format!("{name}.{index}.rgba"))).unwrap();
                let mut normalized = actual.as_ref().to_vec();
                for pixel in normalized.chunks_exact_mut(4) {
                    if pixel[3] == 0 {
                        pixel[..3].fill(0);
                    }
                }
                assert_eq!(normalized, expected);
                let prepared = crate::prepare_raster_for_display_with_budget(&frame, Some(&budget)).unwrap();
                assert_eq!(prepared.animation_timing(), frame.animation_timing());
                assert_eq!((prepared.image_index(), prepared.image_count()), (index, 3));
                drop((prepared, frame));
                assert_eq!(budget.used(), 0);
            }
        }
    }
    #[test]
    fn animation_invalid_index_and_truncation_leave_same_root_retryable() {
        let bytes = include_bytes!("../../../tests/fixtures/raster/animation-alpha.jxl");
        let budget = rrrah_core::MemoryBudget::new(1024 * 1024);
        let mut request = DecodeRequest::new("animation.jxl");
        request.memory_budget = Some(budget.clone());
        request.image_index = 3;
        assert!(matches!(
            decode(bytes, &request),
            Err(RasterDecodeError::Source(
                crate::DecodeError::UnsupportedImageIndex { index: 3 }
            ))
        ));
        assert_eq!(budget.used(), 0);
        request.image_index = 2;
        let mut refused = 0;
        for end in 1..bytes.len() {
            let result = decode(&bytes[..end], &request);
            if result.is_err() {
                refused += 1;
            }
            drop(result);
            assert_eq!(budget.used(), 0, "prefix {end}");
        }
        assert!(refused > 0);
        let frame = decode(bytes, &request).unwrap();
        assert_eq!(frame.image_count(), 3);
        drop(frame);
        assert_eq!(budget.used(), 0);
    }
    #[test]
    fn rational_jxl_ticks_reduce_without_overflow_or_silent_rounding() {
        let value = timing(3, 24000, 1001, 2).unwrap();
        assert_eq!(
            (
                value.delay_ms_numerator(),
                value.delay_ms_denominator(),
                value.num_plays()
            ),
            (1001, 8, 2)
        );
        assert!(timing(1, 0, 1, 0).is_err());
        assert!(timing(u32::MAX, 1, u32::MAX, 0).is_err());
        assert_eq!(timing(0, 100, 1, 0).unwrap().delay_ms_numerator(), 0);
    }
    #[test]
    fn truncated_stream_errors_release_admitted_output_and_working_credit() {
        let bytes = include_bytes!("../../../tests/fixtures/raster/alpha-container-1.jxl");
        let mut admitted_failures = 0;
        for end in 1..bytes.len() {
            let budget = rrrah_core::MemoryBudget::new(1024 * 1024);
            let mut request = DecodeRequest::new("truncated.jxl");
            request.memory_budget = Some(budget.clone());
            let result = decode(&bytes[..end], &request);
            if result.is_err() && budget.peak() > 65536 {
                admitted_failures += 1;
            }
            drop(result);
            assert_eq!(budget.used(), 0, "truncated at {end}");
        }
        assert!(
            admitted_failures > 0,
            "must exercise errors after working admission"
        );
    }
    #[test]
    fn buffered_header_growth_is_admitted_with_reallocation_overlap() {
        fn initialize(
            root: &rrrah_core::MemoryBudget,
        ) -> Result<(jxl_oxide::JxlImage, rrrah_core::Reservation, Vec<u64>), RasterDecodeError> {
            let bytes = include_bytes!("../../../tests/fixtures/raster/animation-large-icc.jxl");
            let mut credit = root.try_reserve(0).map_err(crate::DecodeError::Memory)?;
            let mut requests = Vec::new();
            let mut uninit = jxl_oxide::JxlImage::builder()
                .alloc_tracker(jxl_oxide::AllocTracker::with_limit(0))
                .build_uninit();
            let mut pending = Vec::new();
            for chunk in bytes.chunks(4096) {
                pending.extend_from_slice(chunk);
                let consumed = uninit
                    .feed_bytes_with_header_admission(&pending, |n| {
                        requests.push(n);
                        credit.ensure_bytes(n).map_err(crate::DecodeError::Memory)?;
                        Ok(())
                    })
                    .map_err(codec_error)?;
                pending.drain(..consumed);
                match uninit.try_init_header_only().map_err(codec_error)? {
                    jxl_oxide::InitializeResult::NeedMoreData(next) => uninit = next,
                    jxl_oxide::InitializeResult::Initialized(image) => return Ok((image, credit, requests)),
                }
            }
            Err(invalid("header fixture incomplete"))
        }
        // This root deliberately covers only the encoded header Vec; the ICC
        // working callback is tested separately. No frame groups are loaded.
        let root = rrrah_core::MemoryBudget::new(1024 * 1024);
        let (image, credit, requests) = initialize(&root).unwrap();
        assert_eq!(requests, [4043, 12182, 24417]);
        assert_eq!(root.used(), 24417);
        assert_eq!(root.peak(), 24417);
        assert_eq!(image.num_loaded_frames(), 0);
        assert_eq!(image.original_icc().unwrap().len(), 8712);
        drop(image);
        drop(credit);
        assert_eq!(root.used(), 0);
        for (limit, peak) in [(4042, 0), (24416, 12182)] {
            let root = rrrah_core::MemoryBudget::new(limit);
            assert!(matches!(
                initialize(&root),
                Err(RasterDecodeError::Source(crate::DecodeError::Memory(_)))
            ));
            assert_eq!(root.peak(), peak);
            assert_eq!(root.used(), 0);
        }
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../tests/fixtures/raster/animation-large-icc.jxl");
        let root = rrrah_core::MemoryBudget::new(1024 * 1024);
        let mut request = DecodeRequest::new(path);
        request.image_index = 2;
        request.memory_budget = Some(root.clone());
        let frame = crate::decode_raster(&request).unwrap();
        assert_eq!(root.used(), frame.capacity_bytes());
        drop(frame);
        assert_eq!(root.used(), 0);
        let peak = root.peak();
        for limit in [peak - 1, peak] {
            let root = rrrah_core::MemoryBudget::new(limit);
            request.memory_budget = Some(root.clone());
            let result = crate::decode_raster(&request);
            if limit < peak {
                assert!(matches!(
                    result,
                    Err(RasterDecodeError::Source(crate::DecodeError::Memory(_)))
                ));
            } else {
                assert!(result.is_ok());
            }
            drop(result);
            assert_eq!(root.used(), 0);
        }
        eprintln!("JXL_HEADER_BUFFER growth={requests:?} overlap_peak=24417 public_peak={peak} final=0");
    }
    #[test]
    fn icc_reconstruction_cancellation_preserves_typed_error_and_same_root_retry() {
        use std::sync::atomic::{AtomicU64, Ordering};
        let bytes = include_bytes!("../../../tests/fixtures/raster/animation-embedded-icc.jxl");
        let owner = Arc::new(AtomicU64::new(1));
        let root = rrrah_core::MemoryBudget::new(1024 * 1024);
        let mut request = DecodeRequest::new("icc.jxl");
        request.cancellation = Some(crate::GenerationToken::new(owner.clone(), 1));
        let mut credits = Vec::new();
        let mut admitted = 0;
        let mut calls = 0;
        let mut uninit = jxl_oxide::JxlImage::builder()
            .alloc_tracker(jxl_oxide::AllocTracker::with_limit(0))
            .build_uninit();
        uninit.feed_bytes(bytes).unwrap();
        let result = uninit.try_init_header_only_with_icc_admission(|n| {
            calls += 1;
            // Small ICC: two admission calls, reconstruction output check,
            // then a tag-command checkpoint after the ICC header is built.
            if calls == 4 {
                owner.store(2, Ordering::Release);
            }
            request.check_cancelled()?;
            if n > admitted {
                credits.push(root.try_reserve(n - admitted)?);
                admitted = n;
            }
            Ok(())
        });
        let error = match result {
            Err(e) => e,
            Ok(_) => panic!("reconstruction ignored cancellation"),
        };
        assert!(matches!(
            codec_error(error),
            RasterDecodeError::Source(crate::DecodeError::Cancelled)
        ));
        assert_eq!(calls, 4);
        assert_eq!(admitted, 1445);
        drop(credits);
        assert_eq!(root.used(), 0);
        request.cancellation = Some(crate::GenerationToken::new(owner, 2));
        request.memory_budget = Some(root.clone());
        for index in 0..3 {
            request.image_index = index;
            let frame = decode(bytes, &request).unwrap();
            assert_eq!((frame.image_index(), frame.image_count()), (index, 3));
            drop(frame);
            assert_eq!(root.used(), 0);
        }
        eprintln!("JXL_ICC_RECONSTRUCT_CANCEL callbacks={calls} admitted={admitted} retry_frames=3 final=0");
    }
    #[test]
    fn large_icc_entropy_cancellation_releases_credit_and_preserves_retry() {
        use std::sync::atomic::{AtomicU64, Ordering};
        let bytes = include_bytes!("../../../tests/fixtures/raster/animation-large-icc.jxl");
        let owner = Arc::new(AtomicU64::new(7));
        let root = rrrah_core::MemoryBudget::new(1024 * 1024);
        let mut request = DecodeRequest::new("large-icc.jxl");
        request.cancellation = Some(crate::GenerationToken::new(owner.clone(), 7));
        let mut credit = Vec::new();
        let mut admitted = 0;
        let mut calls = 0;
        let mut uninit = jxl_oxide::JxlImage::builder()
            .alloc_tracker(jxl_oxide::AllocTracker::with_limit(0))
            .build_uninit();
        uninit.feed_bytes(bytes).unwrap();
        let result = uninit.try_init_header_only_with_icc_admission(|n| {
            calls += 1;
            // First two calls admit buffers. The third must occur within the
            // entropy loop, before the complete encoded profile is read.
            if calls == 3 {
                owner.store(8, Ordering::Release);
            }
            request.check_cancelled()?;
            if n > admitted {
                credit.push(root.try_reserve(n - admitted)?);
                admitted = n;
            }
            Ok(())
        });
        let error = match result {
            Err(e) => e,
            Ok(_) => panic!("ICC entropy loop did not observe cancellation"),
        };
        assert!(matches!(
            codec_error(error),
            RasterDecodeError::Source(crate::DecodeError::Cancelled)
        ));
        assert_eq!(calls, 3);
        assert!(admitted > 8192);
        assert_eq!(root.peak(), admitted);
        drop(credit);
        assert_eq!(root.used(), 0);
        request.cancellation = Some(crate::GenerationToken::new(owner, 8));
        request.memory_budget = Some(root.clone());
        for index in 0..3 {
            request.image_index = index;
            let frame = decode(bytes, &request).unwrap();
            let RasterPixels::Rgba16(values) = frame.pixels() else {
                panic!()
            };
            let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(format!(
                "../../tests/fixtures/raster/animation-large-icc.jxl.{index}.rgba16le"
            ));
            let reference = std::fs::read(path).unwrap();
            let expected: Vec<u16> = reference
                .chunks_exact(2)
                .map(|b| u16::from_le_bytes(b.try_into().unwrap()))
                .collect();
            assert_eq!(values.as_ref(), expected);
            assert_eq!((frame.image_index(), frame.image_count()), (index, 3));
            assert_eq!(root.used(), frame.capacity_bytes());
            drop(frame);
            assert_eq!(root.used(), 0);
        }
        eprintln!("JXL_ICC_CANCEL callbacks={calls} admitted={admitted} retry_frames=3 final=0");
    }
    #[test]
    fn rendered_profile_admission_covers_synthesized_and_unrecognized_icc_paths() {
        let bytes = include_bytes!("../../../tests/fixtures/raster/animation-embedded-icc.jxl");
        let mut image = jxl_oxide::JxlImage::builder().read(Cursor::new(bytes)).unwrap();
        let mut table_profile =
            include_bytes!("../../../tests/fixtures/raster/linear-identity-rgb.icc").to_vec();
        let offset = table_profile.len();
        table_profile.extend_from_slice(b"curv\0\0\0\0\0\0\0\x03\0\0\x20\0\xff\xff\0\0");
        let length = table_profile.len() as u32;
        table_profile[..4].copy_from_slice(&length.to_be_bytes());
        let tags = u32::from_be_bytes(table_profile[128..132].try_into().unwrap()) as usize;
        for entry in 0..tags {
            let at = 132 + entry * 12;
            if [b"rTRC", b"gTRC", b"bTRC"].contains(&table_profile[at..at + 4].try_into().unwrap()) {
                table_profile[at + 4..at + 8].copy_from_slice(&(offset as u32).to_be_bytes());
                table_profile[at + 8..at + 12].copy_from_slice(&18u32.to_be_bytes());
            }
        }
        for opaque in [false, true] {
            if opaque {
                image.request_icc(&table_profile).unwrap();
            }
            let expected = image.rendered_icc();
            if opaque {
                assert_eq!(expected, table_profile, "must exercise retained raw ICC branch");
            }
            for limit in [expected.len() as u64 - 1, expected.len() as u64] {
                let root = rrrah_core::MemoryBudget::new(limit);
                let mut credit = None;
                let mut calls = 0;
                let result = image.rendered_icc_with_admission(|n| {
                    calls += 1;
                    assert_eq!(n, expected.len() as u64);
                    credit = Some(root.try_reserve(n).map_err(crate::DecodeError::Memory)?);
                    Ok(())
                });
                assert_eq!(calls, 1);
                if limit < expected.len() as u64 {
                    assert!(matches!(
                        codec_error(result.unwrap_err()),
                        RasterDecodeError::Source(crate::DecodeError::Memory(_))
                    ));
                    assert_eq!(root.peak(), 0);
                } else {
                    let output = result.unwrap();
                    assert_eq!(output, expected);
                    assert_eq!(output.capacity(), output.len());
                    assert_eq!(root.used(), output.capacity() as u64);
                    drop(output);
                }
                drop(credit);
                assert_eq!(root.used(), 0);
            }
        }
    }
    #[test]
    fn output_admission_precedes_pixel_decode_and_tracks_shared_owners() {
        for bytes in [
            include_bytes!("../../../tests/fixtures/raster/precision16.jxl").as_slice(),
            include_bytes!("../../../tests/fixtures/raster/alpha-container-1.jxl").as_slice(),
        ] {
            let reference = decode(bytes, &DecodeRequest::new("image.jxl")).unwrap();
            let weight = reference.pixel_capacity_bytes();
            let budget = rrrah_core::MemoryBudget::new(weight - 1);
            let mut request = DecodeRequest::new("image.jxl");
            request.memory_budget = Some(budget.clone());
            assert!(matches!(
                decode(bytes, &request),
                Err(RasterDecodeError::Source(crate::DecodeError::Memory(_)))
            ));
            assert_eq!(budget.peak(), 0);
            let working = 65536 + u64::from(reference.width()) * u64::from(reference.height()) * 64 + weight;
            let budget = rrrah_core::MemoryBudget::new(weight);
            request.memory_budget = Some(budget.clone());
            assert!(matches!(
                decode(bytes, &request),
                Err(RasterDecodeError::Source(crate::DecodeError::Memory(_)))
            ));
            assert_eq!(budget.used(), 0);
            let profile = reference.color_profile_capacity_bytes();
            let budget = rrrah_core::MemoryBudget::new(weight + working + profile - 1);
            request.memory_budget = Some(budget.clone());
            assert!(
                matches!(decode(bytes, &request), Err(RasterDecodeError::Source(crate::DecodeError::Memory(rrrah_core::BufferError::Capacity { requested, used, .. }))) if requested == profile && used == weight + working)
            );
            assert_eq!(budget.used(), 0);
            let budget = rrrah_core::MemoryBudget::new(weight + working + profile);
            request.memory_budget = Some(budget.clone());
            let frame = decode(bytes, &request).unwrap();
            assert_eq!(budget.used(), weight + profile);
            let retained = frame.clone();
            drop(frame);
            assert_eq!(budget.used(), weight + profile);
            drop(retained);
            assert_eq!(budget.used(), 0);
        }
    }
    #[test]
    fn sixteen_bit_samples_remain_distinct() {
        let bytes = include_bytes!("../../../tests/fixtures/raster/precision16.jxl");
        let frame = decode(bytes, &DecodeRequest::new("precision16.jxl")).unwrap();
        let RasterPixels::Rgba16(pixels) = frame.pixels() else {
            panic!("lost precision")
        };
        assert_eq!(
            pixels.as_slice(),
            &[32768, 32768, 32768, 65535, 32769, 32769, 32769, 65535]
        );
    }
    #[test]
    fn icc_profile_reaches_display_and_alpha_is_retained() {
        let bytes = include_bytes!("../../../tests/fixtures/raster/alpha-container-1.jxl");
        let frame = decode(bytes, &DecodeRequest::new("renamed.cr3")).unwrap();
        assert!(matches!(frame.color_space(), RasterColorSpace::Icc(_)));
        let prepared = crate::prepare_raster_for_display(&frame).unwrap();
        let RasterPixels::Rgba32Float(pixels) = prepared.pixels() else {
            panic!()
        };
        assert!((pixels[7] - 128.0 / 255.0).abs() < 1e-7);
    }
}
