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
