#![cfg(feature = "decode")]
use rrrah_core::{DecodedRaster, RasterColorSpace, RasterPixels};
#[test]
fn independent_lcms_camera_render_grid_preserves_profiled_linear_samples() {
    check_grid(
        include_bytes!("fixtures/icc-render/samples.rgba16le"),
        include_bytes!("fixtures/icc-render/lcms.rgba32le"),
    );
}
#[test]
fn independent_lcms_boundary_samples_preserve_black_channel_endpoints() {
    check_grid(
        include_bytes!("fixtures/icc-render/boundary.rgba16le"),
        include_bytes!("fixtures/icc-render/boundary-lcms.rgba32le"),
    );
}
fn check_grid(input_bytes: &[u8], expected_bytes: &[u8]) {
    let input = input_bytes
        .as_chunks::<2>()
        .0
        .iter()
        .map(|b| u16::from_le_bytes([b[0], b[1]]))
        .collect::<Vec<_>>();
    let expected = expected_bytes
        .as_chunks::<4>()
        .0
        .iter()
        .map(|b| f32::from_le_bytes(*b))
        .collect::<Vec<_>>();
    let frame = DecodedRaster::new(
        32,
        32,
        RasterPixels::Rgba16(std::sync::Arc::new(input).into()),
        RasterColorSpace::Icc(include_bytes!("fixtures/icc-render/profile.icc").to_vec()),
    )
    .unwrap();
    let converted = rrrah_decode::prepare_raster_for_display(&frame).unwrap();
    let RasterPixels::Rgba32Float(actual) = converted.pixels() else {
        panic!("precision lost");
    };
    assert_eq!(actual.len(), expected.len());
    for (i, (a, b)) in actual.iter().zip(&expected).enumerate() {
        if i % 4 == 3 {
            assert_eq!(a.to_bits(), 1.0f32.to_bits());
        } else {
            assert!((a - b).abs() < 0.0005, "sample {i}: {a} versus independent {b}");
        }
    }
}

#[test]
fn camera_gamma_profile_preserves_exact_black_and_unfitted_alpha() {
    let input = vec![0, 0, 0, 0, 0, 0, 0, 32768, 0, 0, 0, 65535];
    let frame = DecodedRaster::new(
        3,
        1,
        RasterPixels::Rgba16(std::sync::Arc::new(input).into()),
        RasterColorSpace::Icc(include_bytes!("fixtures/icc-render/profile.icc").to_vec()),
    )
    .unwrap();
    let converted = rrrah_decode::prepare_raster_for_display(&frame).unwrap();
    let RasterPixels::Rgba32Float(actual) = converted.pixels() else {
        panic!("precision lost");
    };
    for (pixel, alpha) in actual
        .as_chunks::<4>()
        .0
        .iter()
        .zip([0.0f32, 32768.0 / 65535.0, 1.0])
    {
        assert!(
            pixel[..3].iter().all(|v| *v == 0.0),
            "ICC black must stay black: {pixel:?}"
        );
        assert_eq!(pixel[3].to_bits(), alpha.to_bits());
    }
}
