//! Authored RGB-only reproduction: non-isolated knockout parent and two
//! isolated child groups. No external artwork or color-profile ambiguity.
use rrrah_decode::{DecodeRequest, decode_raster};

fn center_on_white(fixture: &str) -> [u32; 3] {
    sample_on_white(fixture, 16, 8)
}

fn sample_on_white(fixture: &str, x: usize, y: usize) -> [u32; 3] {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures/raster")
        .join(fixture);
    let image = decode_raster(&DecodeRequest::new(path)).unwrap();
    assert_eq!((image.width(), image.height()), (32, 16));
    let rrrah_core::RasterPixels::Rgba8(pixels) = image.pixels() else {
        panic!("expected RGBA8");
    };
    let offset = (y * 32 + x) * 4;
    let p = &pixels[offset..offset + 4];
    // Blue child at half opacity replaces the red child shape. Composite
    // premultiplied native samples on white, as in the independent oracle.
    let white = |c: u8| (u32::from(c) * u32::from(p[3]) + 255 * (255 - u32::from(p[3])) + 127) / 255;
    [white(p[0]), white(p[1]), white(p[2])]
}

#[test]
fn ordinary_group_preserves_previous_child_under_half_opacity() {
    let pixel = center_on_white("pdf-normal-overlap.pdf");
    assert!(pixel[0].abs_diff(127) <= 1);
    assert_eq!(pixel[1], 0);
    assert!(pixel[2].abs_diff(128) <= 1);
}

#[test]
#[ignore = "pending native non-isolated knockout group implementation"]
fn nonisolated_knockout_replaces_previous_child_shape() {
    let pixel = center_on_white("pdf-knockout-overlap.pdf");
    assert!(pixel[0].abs_diff(127) <= 1);
    assert!(pixel[1].abs_diff(127) <= 1);
    assert_eq!(pixel[2], 255);
}

#[test]
#[ignore = "pending opacity-independent knockout shape tracking"]
fn transparent_child_still_knocks_out_previous_shape() {
    assert_eq!(center_on_white("pdf-knockout-transparent-child.pdf"), [255; 3]);
}

#[test]
#[ignore = "pending non-isolated group backdrop preservation"]
fn knockout_restores_colored_initial_backdrop() {
    let pixel = center_on_white("pdf-knockout-colored-backdrop.pdf");
    for channel in pixel {
        assert!(channel.abs_diff(127) <= 1);
    }
}

#[test]
#[ignore = "Poppler edge diagnostic pending normative coverage analysis and renderer integration"]
fn child_isolation_changes_fractional_knockout_edge() {
    let nonisolated = sample_on_white("pdf-knockout-fractional-nonisolated.pdf", 8, 8);
    let isolated = sample_on_white("pdf-knockout-fractional-isolated.pdf", 8, 8);
    // Independent oracle: nonisolated [244,244,255], isolated [244,234,245].
    // Oracle diagnostic only: this difference is not yet a normative AA
    // requirement. Keep ignored until the group coverage computation is proved.
    assert!(nonisolated[1] >= isolated[1] + 5);
    assert!(nonisolated[2] >= isolated[2] + 5);
}
