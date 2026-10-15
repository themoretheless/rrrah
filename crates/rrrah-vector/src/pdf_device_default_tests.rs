#[test]
fn default_cmyk_nested_scopes_cover_devices_meshes_images_and_alternates() {
    let budget = rrrah_core::MemoryBudget::new(2 * 1024 * 1024);
    let mut request = crate::DecodeRequest::new("device-default-cmyk-contexts.pdf");
    request.memory_budget = Some(budget.clone());
    let image = crate::pdf::decode(
        include_bytes!("../../../tests/fixtures/pdf/device-default-cmyk-contexts.pdf"),
        &request,
    )
    .unwrap();
    assert_eq!((image.width(), image.height()), (144, 8));
    let rrrah_core::RasterPixels::Rgba8(pixels) = image.pixels() else {
        panic!("RGBA8 expected")
    };
    for y in 0..8 {
        for x in 0..144 {
            let expected = if x < 72 {
                [255, 0, 0, 255]
            } else {
                [0, 0, 255, 255]
            };
            assert_eq!(
                &pixels[(y * 144 + x) * 4..(y * 144 + x) * 4 + 4],
                &expected,
                "pixel {x},{y}; form {}, cell {}",
                x / 72,
                (x % 72) / 8
            );
        }
    }
    drop(image);
    assert_eq!(budget.used(), 0);
}
#[test]
fn identical_shading_dictionaries_keep_distinct_stream_payloads() {
    let budget = rrrah_core::MemoryBudget::new(1024 * 1024);
    let mut request = crate::DecodeRequest::new("mesh-equal-dictionaries-distinct-streams.pdf");
    request.memory_budget = Some(budget.clone());
    let image = crate::pdf::decode(
        include_bytes!("../../../tests/fixtures/pdf/mesh-equal-dictionaries-distinct-streams.pdf"),
        &request,
    )
    .unwrap();
    assert_eq!((image.width(), image.height()), (16, 8));
    let rrrah_core::RasterPixels::Rgba8(pixels) = image.pixels() else {
        panic!("RGBA8 expected")
    };
    for y in 0..8 {
        for x in 0..16 {
            let expected = if x < 8 { [255, 0, 0, 255] } else { [0, 0, 255, 255] };
            assert_eq!(
                &pixels[(y * 16 + x) * 4..(y * 16 + x) * 4 + 4],
                &expected,
                "pixel {x},{y}"
            );
        }
    }
    drop(image);
    assert_eq!(budget.used(), 0);
}
#[test]
fn equal_stream_headers_have_content_sensitive_cache_keys() {
    use hayro::hayro_interpret::CacheKey;
    use hayro::hayro_syntax::object::{Name, Object};
    let pdf = hayro::hayro_syntax::Pdf::new(
        include_bytes!("../../../tests/fixtures/pdf/mesh-equal-dictionaries-distinct-streams.pdf").to_vec(),
    )
    .unwrap();
    let page = &pdf.pages()[0];
    let Object::Stream(red) = page.resources().get_shading(&Name::new_unescaped(b"R")).unwrap() else {
        panic!("red stream")
    };
    let Object::Stream(blue) = page.resources().get_shading(&Name::new_unescaped(b"B")).unwrap() else {
        panic!("blue stream")
    };
    assert_eq!(red.dict().cache_key(), blue.dict().cache_key());
    assert_ne!(red.raw_data(), blue.raw_data());
    assert_ne!(red.cache_key(), blue.cache_key());
    assert_eq!(red.clone().cache_key(), red.cache_key());
}

#[test]
fn default_gray_and_rgb_isolate_device_fills_and_shared_images() {
    for bytes in [
        include_bytes!("../../../tests/fixtures/pdf/device-default-gray-contexts.pdf").as_slice(),
        include_bytes!("../../../tests/fixtures/pdf/device-default-rgb-contexts.pdf").as_slice(),
    ] {
        let budget = rrrah_core::MemoryBudget::new(1024 * 1024);
        let mut request = crate::DecodeRequest::new("device-default.pdf");
        request.memory_budget = Some(budget.clone());
        let image = crate::pdf::decode(bytes, &request).unwrap();
        assert_eq!((image.width(), image.height()), (32, 8));
        let rrrah_core::RasterPixels::Rgba8(pixels) = image.pixels() else {
            panic!("RGBA8 expected")
        };
        for y in 0..8 {
            for x in 0..32 {
                let expected = if x < 16 {
                    [255, 0, 0, 255]
                } else {
                    [0, 0, 255, 255]
                };
                assert_eq!(&pixels[(y * 32 + x) * 4..(y * 32 + x) * 4 + 4], &expected);
            }
        }
        drop(image);
        assert_eq!(budget.used(), 0);
    }
}

#[test]
fn implicit_initial_gray_remains_black_with_default_gray_resource() {
    let image = crate::pdf::decode(
        include_bytes!("../../../tests/fixtures/pdf/device-default-gray-implicit.pdf"),
        &crate::DecodeRequest::new("device-default-gray-implicit.pdf"),
    )
    .unwrap();
    assert_eq!((image.width(), image.height()), (8, 8));
    let rrrah_core::RasterPixels::Rgba8(pixels) = image.pixels() else {
        panic!("RGBA8 expected")
    };
    assert!(pixels.chunks_exact(4).all(|pixel| pixel == [0, 0, 0, 255]));
}

#[test]
fn shared_soft_mask_group_preserves_mask_kind_and_transfer() {
    let image = crate::pdf::decode(
        include_bytes!("../../../tests/fixtures/pdf/soft-mask-shared-group.pdf"),
        &crate::DecodeRequest::new("soft-mask-shared-group.pdf"),
    )
    .unwrap();
    let rrrah_core::RasterPixels::Rgba8(pixels) = image.pixels() else {
        panic!("RGBA8 expected")
    };
    assert_eq!((image.width(), image.height()), (24, 8));
    for y in 0..8 {
        for x in 0..24 {
            let pixel = &pixels[(y * 24 + x) * 4..(y * 24 + x) * 4 + 4];
            assert_eq!(&pixel[..3], &[0, 0, 0]);
            if x < 8 {
                // PDF device-space blue luminosity rounds to 28/255.
                assert_eq!(pixel[3], 28);
            } else {
                assert_eq!(pixel[3], 255, "mask variant at {x},{y}");
            }
        }
    }
}

#[test]
fn device_rgb_soft_mask_luminosity_matches_independent_primary_bands() {
    let budget = rrrah_core::MemoryBudget::new(1024 * 1024);
    let mut request = crate::DecodeRequest::new("soft-mask-device-rgb-bands.pdf");
    request.memory_budget = Some(budget.clone());
    let image = crate::pdf::decode(
        include_bytes!("../../../tests/fixtures/pdf/soft-mask-device-rgb-bands.pdf"),
        &request,
    )
    .unwrap();
    let rrrah_core::RasterPixels::Rgba8(pixels) = image.pixels() else {
        panic!("RGBA8 expected")
    };
    for row in pixels.chunks_exact(24 * 4) {
        for (x, pixel) in row.chunks_exact(4).enumerate() {
            assert_eq!(pixel, &[0, 0, 0, [77, 150, 28][x / 8]]);
        }
    }
    drop(image);
    assert_eq!(budget.used(), 0);
}

#[test]
fn shared_soft_mask_group_isolates_inherited_color_resources() {
    let budget = rrrah_core::MemoryBudget::new(1024 * 1024);
    let mut request = crate::DecodeRequest::new("soft-mask-inherited-profile.pdf");
    request.memory_budget = Some(budget.clone());
    let image = crate::pdf::decode(
        include_bytes!("../../../tests/fixtures/pdf/soft-mask-inherited-profile.pdf"),
        &request,
    )
    .unwrap();
    assert_eq!((image.width(), image.height()), (16, 8));
    let rrrah_core::RasterPixels::Rgba8(pixels) = image.pixels() else {
        panic!("RGBA8 expected")
    };
    // Context isolation only: calibrated luminosity remains separately unqualified.
    // The current calibrated renderer yields red 54 and blue 18; never reuse red.
    for y in 0..8 {
        for x in 0..16 {
            assert_eq!(
                &pixels[(y * 16 + x) * 4..(y * 16 + x) * 4 + 4],
                &[0, 0, 0, if x < 8 { 54 } else { 18 }],
                "scope at {x},{y}"
            );
        }
    }
    drop(image);
    assert_eq!(budget.used(), 0);
}

#[test]
fn alpha_soft_mask_without_group_color_space_preserves_coverage() {
    let budget = rrrah_core::MemoryBudget::new(1024 * 1024);
    let mut request = crate::DecodeRequest::new("soft-mask-alpha-without-group-cs.pdf");
    request.memory_budget = Some(budget.clone());
    let image = crate::pdf::decode(
        include_bytes!("../../../tests/fixtures/pdf/soft-mask-alpha-without-group-cs.pdf"),
        &request,
    )
    .unwrap();
    assert_eq!((image.width(), image.height()), (16, 8));
    let rrrah_core::RasterPixels::Rgba8(pixels) = image.pixels() else {
        panic!("RGBA8 expected")
    };
    for row in pixels.chunks_exact(16 * 4) {
        for (x, pixel) in row.chunks_exact(4).enumerate() {
            assert_eq!(pixel, &[0, 0, 0, if x < 8 { 255 } else { 0 }]);
        }
    }
    drop(image);
    assert_eq!(budget.used(), 0);
}
