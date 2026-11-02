#[path = "../../rrrah-gpu/tests/common/mod.rs"]
mod common;
use rrrah_core::{MemoryBudget, RasterColorSpace, RasterPixels};

#[test]
#[ignore = "requires pinned external W3C attachments and FFmpeg RGB48 oracle; RRRAH_W3C_PNG_DIR"]
fn external_w3c_hdr_png_native_codes_and_explicit_policy() {
    let directory =
        std::path::PathBuf::from(std::env::var_os("RRRAH_W3C_PNG_DIR").expect("fixture directory"));
    for (name, transfer, full) in [
        ("PNG-HLG-FancyColorBars-16bit-CICP-FR.png", 18, true),
        ("PNG-HLG-FancyColorBars-16bit-CICP-NR.png", 18, false),
        ("PNG-PQ-BT.2111-ColorBars-16bit-CICP-FR.png", 16, true),
    ] {
        let root = MemoryBudget::new(128 * 1024 * 1024);
        let mut request = rrrah_decode::DecodeRequest::new(directory.join(name));
        request.memory_budget = Some(root.clone());
        let native = rrrah_decode::decode_raster(&request).unwrap();
        assert_eq!((native.width(), native.height()), (1920, 1080));
        assert_eq!(
            native.color_space(),
            &RasterColorSpace::Cicp {
                primaries: 9,
                transfer,
                matrix: 0,
                full_range: full
            }
        );
        let oracle = std::fs::read(directory.join(format!("{name}.rgb48le"))).unwrap();
        assert_eq!(oracle.len(), 1920 * 1080 * 6);
        let RasterPixels::Rgba16(pixels) = native.pixels() else {
            panic!()
        };
        for (index, (actual, expected)) in pixels.chunks_exact(4).zip(oracle.chunks_exact(6)).enumerate() {
            for channel in 0..3 {
                assert_eq!(
                    actual[channel],
                    u16::from_le_bytes([expected[channel * 2], expected[channel * 2 + 1]]),
                    "{name}: pixel={index} channel={channel}"
                );
            }
            assert_eq!(actual[3], 65535);
        }
        assert_eq!(root.used(), 1920 * 1080 * 8);
        drop(native);
        assert_eq!(root.used(), 0);
        if transfer == 16 {
            request.pq_reference_white_nits = Some(100.);
        } else {
            request.hlg_display = Some((100., 1000., 1.2));
        }
        let output = rrrah_decode::decode_raster(&request);
        {
            let frame = output.unwrap();
            assert_eq!(frame.color_space(), &RasterColorSpace::LinearSrgb);
            let RasterPixels::Rgba32Float(values) = frame.pixels() else {
                panic!()
            };
            assert!(values.iter().all(|v| v.is_finite()));
            assert!(values.chunks_exact(4).all(|p| p[3] == 1.));
            assert!(values.chunks_exact(4).any(|p| p[..3].iter().any(|v| *v > 1.)));
            assert_eq!(root.used(), 1920 * 1080 * 16);
            let alias = frame.clone();
            drop(frame);
            assert_eq!(root.used(), 1920 * 1080 * 16);
            drop(alias);
            assert_eq!(root.used(), 0);
        }
        eprintln!(
            "{name}: all 2073600 RGB16 pixels exact against FFmpeg; CICP retained; explicit display policy {}; final root=0",
            "accepted (independent linear oracle checked separately)"
        );
    }
}

#[test]
#[ignore = "requires pinned W3C PNGs, Decimal linear oracle and actual Metal; RRRAH_W3C_PNG_DIR"]
fn external_w3c_linear_hdr_reference_swap_and_metal() {
    let gpu = common::qualification_gpu().expect("required GPU adapter");
    eprintln!("External W3C PNG adapter: {}", gpu.adapter_name());
    let directory =
        std::path::PathBuf::from(std::env::var_os("RRRAH_W3C_PNG_DIR").expect("fixture directory"));
    for name in [
        "PNG-HLG-FancyColorBars-16bit-CICP-FR.png",
        "PNG-HLG-FancyColorBars-16bit-CICP-NR.png",
        "PNG-PQ-BT.2111-ColorBars-16bit-CICP-FR.png",
    ] {
        let root = MemoryBudget::new(128 * 1024 * 1024);
        let mut request = rrrah_decode::DecodeRequest::new(directory.join(name));
        request.memory_budget = Some(root.clone());
        if name.contains("HLG") {
            request.hlg_display = Some((100., 1000., 1.2));
        } else {
            request.pq_reference_white_nits = Some(100.);
        }
        let source = rrrah_decode::decode_raster(&request).unwrap();
        let bytes = std::fs::read(directory.join(format!("{name}.linear-rgba32le"))).unwrap();
        assert_eq!(bytes.len(), 1920 * 1080 * 16);
        let expected: Vec<f32> = bytes
            .chunks_exact(4)
            .map(|v| f32::from_le_bytes(v.try_into().unwrap()))
            .collect();
        drop(bytes);
        let RasterPixels::Rgba32Float(actual) = source.pixels() else {
            panic!()
        };
        let mut maximum = 0f64;
        for (index, (a, b)) in actual.iter().zip(&expected).enumerate() {
            let delta = f64::from((a - b).abs());
            maximum = maximum.max(delta);
            assert!(a.is_finite() && b.is_finite());
            assert!(
                delta <= f64::from(b.abs().max(1.)) * 0.00001,
                "{name}: sample={index}: {a} != {b}"
            );
        }
        let bits: Vec<_> = actual.iter().map(|v| v.to_bits()).collect();
        let golden = rrrah_core::DecodedRaster::new(
            1920,
            1080,
            RasterPixels::Rgba32Float(std::sync::Arc::new(expected).into()),
            RasterColorSpace::LinearSrgb,
        )
        .unwrap();
        // Cancel the viewer's 32-pixel fit margin for exact source-pixel centers.
        let fit = (1888f32 / 1920.).min(1048f32 / 1080.);
        let zoom = 1. / fit;
        assert_eq!((fit * zoom).to_bits(), 1f32.to_bits());
        let parameters = rrrah_gpu::ViewParameters {
            viewport: [1920., 1080.],
            zoom,
            ..Default::default()
        };
        let sdr = gpu.render_raster(&source, parameters, [1920, 1080]);
        let hdr = gpu.render_raster_half(&source, parameters, [1920, 1080]);
        let sdr_reference = gpu.render_raster(&golden, parameters, [1920, 1080]);
        assert!(
            sdr.pixels
                .iter()
                .zip(&sdr_reference.pixels)
                .all(|(a, b)| a.abs_diff(*b) <= 1)
        );
        let hdr_reference = gpu.render_raster_half(&golden, parameters, [1920, 1080]);
        let half = |v: u16| {
            let sign = if v & 0x8000 == 0 { 1. } else { -1. };
            let exponent = (v >> 10) & 31;
            assert_ne!(exponent, 31);
            sign * if exponent == 0 {
                f64::from(v & 1023) * 2f64.powi(-24)
            } else {
                f64::from(1024 + (v & 1023)) * 2f64.powi(i32::from(exponent) - 25)
            }
        };
        for (a, b) in hdr.iter().zip(&hdr_reference) {
            assert!((half(*a) - half(*b)).abs() <= half(*b).abs().max(0.01) * 0.002);
        }
        // Independent per-source-pixel HDR oracle: no second GPU rendering in
        // this comparison. This also rejects a skipped border or shifted sampling.
        let RasterPixels::Rgba32Float(golden_pixels) = golden.pixels() else {
            panic!()
        };
        assert_eq!(hdr.len(), 1920 * 1080 * 4);
        assert_eq!(golden_pixels.len(), hdr.len());
        for (index, (actual, expected)) in hdr.iter().zip(golden_pixels.iter()).enumerate() {
            let actual = half(*actual);
            let expected = f64::from(*expected);
            assert!(
                (actual - expected).abs() <= expected.abs().max(0.01) * 0.002,
                "{name}: 1:1 HDR channel {index}: {actual} != {expected}"
            );
        }
        // W3C sRGB encoding, independently evaluated in f64 from the
        // Decimal-derived source, with final normalized attachment clipping.
        // https://www.w3.org/Graphics/Color/srgb
        assert_eq!(sdr.pixels.len(), golden_pixels.len());
        let mut maximum_sdr_code_error = 0;
        for (index, (actual, expected)) in sdr.pixels.iter().zip(golden_pixels.iter()).enumerate() {
            let expected = if index % 4 == 3 {
                255
            } else {
                let linear = f64::from(*expected).clamp(0., 1.);
                let encoded = if linear <= 0.0031308 {
                    12.92 * linear
                } else {
                    1.055 * linear.powf(1. / 2.4) - 0.055
                };
                (encoded * 255.).round() as u8
            };
            let error = actual.abs_diff(expected);
            maximum_sdr_code_error = maximum_sdr_code_error.max(error);
            assert!(
                error <= 1,
                "{name}: 1:1 SDR channel {index}: {actual} != {expected}"
            );
        }
        let weight = source.capacity_bytes();
        assert_eq!(weight, 1920 * 1080 * 16);
        let directory = tempfile::tempdir().unwrap();
        let swap = rrrah_cache::RasterSwapCache::<u8>::new_with_budgets(
            directory.path(),
            rrrah_cache::ImageSwapConfig {
                limits: rrrah_cache::CacheLimits::bytes(weight + 1024 * 1024),
                queue_bytes: weight,
                queue_count: 1,
                restore_bytes: weight,
            },
            MemoryBudget::new(weight),
            root.clone(),
        )
        .unwrap();
        let mut ram = rrrah_cache::RasterRamCache::new(rrrah_cache::CacheLimits::bytes(weight));
        ram.enable_swap(swap);
        assert!(ram.insert_visible(1, source));
        let lease = ram.get_lease(&1).unwrap();
        assert!(!ram.set_limits_and_spill(rrrah_cache::CacheLimits::bytes(0)));
        drop(lease);
        ram.mark_visible(2);
        assert!(ram.set_limits_and_spill(rrrah_cache::CacheLimits::bytes(0)));
        let swap = ram.swap().unwrap();
        swap.wait_idle().unwrap();
        assert_eq!(root.used(), 0);
        let pressure = root.try_reserve(root.limit()).unwrap();
        assert!(swap.try_get(&1, || false).is_err());
        drop(pressure);
        assert!(swap.try_get(&1, || root.used() > 0).unwrap().is_none());
        assert_eq!(root.used(), 0);
        let restored = swap.try_get(&1, || false).unwrap().unwrap();
        let RasterPixels::Rgba32Float(restored_pixels) = restored.pixels() else {
            panic!()
        };
        assert!(restored_pixels.iter().zip(&bits).all(|(a, b)| a.to_bits() == *b));
        assert_eq!(
            sdr.pixels,
            gpu.render_raster(&restored, parameters, [1920, 1080]).pixels
        );
        assert_eq!(hdr, gpu.render_raster_half(&restored, parameters, [1920, 1080]));
        drop(restored);
        assert_eq!(root.used(), 0);
        assert_eq!(
            (swap.stats().reads, swap.stats().writes, swap.stats().errors),
            (1, 1, 0)
        );
        eprintln!(
            "{name}: all2073600 linear pixels match Decimal (max absolute {maximum}); 1920x1080 1:1 Metal output+direct Decimal SDR/HDR pixel oracles (SDR max error {maximum_sdr_code_error}): reference SDR<=1code/HDR<=0.2%; native bits+Metal exact after RAM/swap pressure/cancel/retry; root=0"
        );
    }
}
