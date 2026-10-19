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
    image::ImageError::Decoding(image::error::DecodingError::new(
        image::error::ImageFormatHint::Unknown,
        error,
    ))
    .into()
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
    // Header-only initialization cannot allocate tracked codec groups. Parser,
    // ICC metadata and the buffered encoded header remain outside this tracker.
    let tracker = jxl_oxide::AllocTracker::with_limit(0);
    // Declared before codec state: buffers must drop before their working credit,
    // including every early return after admission.
    let _working;
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
        let consumed = uninit.feed_bytes(&buffer[..valid]).map_err(codec_error)?;
        buffer.copy_within(consumed..valid, 0);
        valid -= consumed;
        match uninit.try_init_header_only().map_err(codec_error)? {
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
    let color = RasterColorSpace::Icc(image.rendered_icc());
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
    Ok(DecodedRaster::new(width, height, pixels, color)?
        .with_animation_timing(animation)
        .with_image_selection(request.image_index, count)?)
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
            let budget = rrrah_core::MemoryBudget::new(weight + working);
            request.memory_budget = Some(budget.clone());
            let frame = decode(bytes, &request).unwrap();
            assert_eq!(budget.used(), weight);
            let retained = frame.clone();
            drop(frame);
            assert_eq!(budget.used(), weight);
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
