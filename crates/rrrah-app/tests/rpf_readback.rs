//! Synthetic RPF channel/alpha qualification through the real display and swap path.
#[path = "../../rrrah-gpu/tests/common/mod.rs"]
mod common;
fn source() -> Vec<u8> {
    let mut b = vec![0; 744];
    for (at, value) in [
        (2, 1u16),
        (10, 1),
        (20, 3),
        (22, 1),
        (26, 0xfffd),
        (658, 32),
        (662, 32),
    ] {
        b[at..at + 2].copy_from_slice(&value.to_be_bytes());
    }
    let program = b"3ds max : ( )";
    b[400..400 + program.len()].copy_from_slice(program);
    b[740..744].copy_from_slice(&744u32.to_be_bytes());
    for values in [[0.125f32, 1.0], [0.25, 2.0], [0.5, 4.0], [0.25, 0.5]] {
        b.extend(8u16.to_be_bytes());
        for value in values {
            b.extend(value.to_bits().to_be_bytes());
        }
    }
    b
}
#[test]
#[ignore = "requires actual GPU adapter"]
fn hdr_rpf_alpha_ram_and_swap_match_explicit_reference_on_gpu() {
    use rrrah_core::{DecodedRaster, MemoryBudget, RasterColorSpace, RasterPixels};
    let gpu = common::qualification_gpu().expect("actual GPU required");
    eprintln!("RPF adapter: {}", gpu.adapter_name());
    for (mode, samples) in [
        (
            rrrah_decode::RlaAlphaMode::Straight,
            [0.125, 0.25, 0.5, 0.25, 1.0, 2.0, 4.0, 0.5],
        ),
        (
            rrrah_decode::RlaAlphaMode::Premultiplied,
            [0.5, 1.0, 2.0, 0.25, 2.0, 4.0, 8.0, 0.5],
        ),
    ] {
        let directory = tempfile::tempdir().unwrap();
        let root = MemoryBudget::new(16 * 1024 * 1024);
        let bytes = source();
        let path = directory.path().join("explicit.rpf");
        std::fs::write(&path, &bytes).unwrap();
        let request = rrrah_decode::DecodeRequest::new(&path);
        let (native, aspect) = rrrah_decode::decode_rpf_file_raster_with_budget(
            &request,
            &root,
            rrrah_decode::RpfDecodeLimits {
                max_node_names: 0,
                max_node_name_bytes: 0,
                max_row_layers: 0,
                max_total_layers: 0,
                max_output_bytes: 776,
            },
            &rrrah_decode::RpfRasterInterpretation {
                rgb: [0, 1, 2],
                matte: Some(0),
                alpha_mode: mode,
                color_space: RasterColorSpace::LinearSrgb,
            },
        )
        .unwrap();
        assert_eq!(aspect, None);
        assert_eq!(root.used(), 32);
        let mut reference = root.try_buffer(8, 0.0f32).unwrap();
        reference[..].copy_from_slice(&samples);
        let golden = DecodedRaster::new(
            2,
            1,
            RasterPixels::Rgba32Float(reference.freeze().into()),
            RasterColorSpace::LinearSrgb,
        )
        .unwrap();
        let display = rrrah_decode::prepare_raster_for_display_with_budget(&native, Some(&root)).unwrap();
        let reference_display =
            rrrah_decode::prepare_raster_for_display_with_budget(&golden, Some(&root)).unwrap();
        let parameters = rrrah_gpu::ViewParameters {
            viewport: [128., 64.],
            zoom: 32.,
            ..Default::default()
        };
        let expected = gpu
            .render_raster(&reference_display, parameters, [128, 64])
            .pixels;
        assert_eq!(
            gpu.render_raster(&display, parameters, [128, 64]).pixels,
            expected
        );
        let mut ram =
            rrrah_cache::RasterRamCache::new(rrrah_cache::CacheLimits::bytes(display.capacity_bytes()));
        assert!(ram.insert(1u8, display.clone()));
        let lease = ram.get_lease(&1).unwrap();
        assert!(ram.set_limits(rrrah_cache::CacheLimits::bytes(0)).is_none());
        assert_eq!(gpu.render_raster(&lease, parameters, [128, 64]).pixels, expected);
        drop(lease);
        drop(ram);
        let weight = native.capacity_bytes();
        let swap = rrrah_cache::ImageSwapCache::<u8, DecodedRaster>::new_with_budgets(
            directory.path(),
            rrrah_cache::ImageSwapConfig {
                limits: rrrah_cache::CacheLimits::bytes(4096),
                queue_bytes: weight,
                queue_count: 1,
                restore_bytes: weight,
            },
            MemoryBudget::new(weight),
            root.clone(),
        )
        .unwrap();
        assert_eq!(
            swap.try_enqueue(1, native.clone()),
            rrrah_cache::SpillAdmission::Queued
        );
        swap.wait_idle().unwrap();
        drop(display);
        drop(native);
        let pressure = root.try_reserve(root.available_bytes()).unwrap();
        assert!(swap.try_get(&1, || false).is_err());
        drop(pressure);
        let restored = swap.try_get(&1, || false).unwrap().unwrap();
        let RasterPixels::Rgba32Float(p) = restored.pixels() else {
            panic!("float expected")
        };
        assert_eq!(&p[..], samples);
        let display = rrrah_decode::prepare_raster_for_display_with_budget(&restored, Some(&root)).unwrap();
        assert_eq!(
            gpu.render_raster(&display, parameters, [128, 64]).pixels,
            expected
        );
        drop(display);
        drop(restored);
        drop(swap);
        drop(reference_display);
        drop(golden);
        assert_eq!(root.used(), 0);
    }
}

#[test]
#[ignore = "requires actual GPU adapter"]
fn rpf_non_square_pixels_match_physical_rectangle_on_gpu() {
    use rrrah_core::{DecodedRaster, MemoryBudget, RasterColorSpace, RasterPixels};
    let gpu = common::qualification_gpu().expect("actual GPU required");
    let root = MemoryBudget::new(4096);
    let mut bytes = source();
    bytes[572..580].copy_from_slice(b"4.00000\0");
    for channel in 0..4 {
        for pixel in 0..2 {
            let at = 744 + channel * 10 + 2 + pixel * 4;
            bytes[at..at + 4].copy_from_slice(&1.0f32.to_bits().to_be_bytes());
        }
    }
    let file = rrrah_decode::inspect_rpf_file(&bytes, 0, 0, || false).unwrap();
    let image = file.decode_with_budget(&root, 0, 0, 776, || false).unwrap();
    let aspect = image.producer_pixel_aspect().unwrap().unwrap() as f32;
    assert_eq!(aspect, 2.0);
    let native = image
        .to_raster_with_interpretation(
            [0, 1, 2],
            Some(0),
            rrrah_decode::RlaAlphaMode::Straight,
            RasterColorSpace::LinearSrgb,
            &root,
            || false,
        )
        .unwrap();
    let golden = DecodedRaster::new(
        4,
        1,
        RasterPixels::Rgba32Float(root.try_buffer(16, 1.0f32).unwrap().freeze().into()),
        RasterColorSpace::LinearSrgb,
    )
    .unwrap();
    let view = rrrah_gpu::ViewParameters {
        viewport: [128., 64.],
        ..Default::default()
    };
    let expected = gpu.render_raster(&golden, view, [128, 64]).pixels;
    assert_eq!(
        gpu.render_raster_with_aspect(&native, view, [128, 64], aspect)
            .pixels,
        expected
    );
    assert_ne!(gpu.render_raster(&native, view, [128, 64]).pixels, expected);
    let native = native.with_pixel_aspect(Some(aspect)).unwrap();
    assert_eq!(gpu.render_raster(&native, view, [128, 64]).pixels, expected);
    let mut payload = Vec::new();
    rrrah_cache::write_raster_payload(&mut payload, &native).unwrap();
    let restored = rrrah_cache::read_raster_payload_with_length(
        &mut payload.as_slice(), payload.len() as u64, &root).unwrap();
    assert_eq!(restored.pixel_aspect(), Some(aspect));
    assert_eq!(gpu.render_raster(&restored, view, [128, 64]).pixels, expected);
    drop(restored);
    drop(golden);
    drop(native);
    drop(image);
    assert_eq!(root.used(), 0);
}
