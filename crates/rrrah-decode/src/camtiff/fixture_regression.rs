//! Opt-in regression tests for owner-supplied camera RAW fixtures.
//!
//! Camera files are not committed to the repository (see
//! `tests/fixtures/README.md`); redistribution terms vary and a synthetic
//! TIFF is not a valid camera regression fixture. Each test below is gated
//! on an environment variable holding the path to a real camera file:
//!
//! ```text
//! RRRAH_CR2_FIXTURE=/licensed/path/sample.CR2 \
//!   cargo test -p rrrah-decode --lib camtiff::fixture_regression
//! ```
//!
//! When the variable is absent the test reports a skip and passes; it never
//! invents fixture bytes. Unlike the DNG/CR3 regression suites, these tests
//! assert only the internal consistency of the decode — routing, geometry,
//! CFA, levels and pixel count — because arbitrary owner-supplied files have
//! no licensed pixel oracle in the manifest. Exact-oracle contracts belong
//! in per-camera fixtures staged through `tests/fixtures/SHA256SUMS`.

use std::{
    env,
    path::{Path, PathBuf},
};

use rrrah_core::{CfaColor, Photometric};

use super::CameraFormat;
use crate::{DecodeRequest, NativeRawDecoder, RawDecoder};

macro_rules! camera_fixture_test {
    ($name:ident, $format:expr, $format_name:literal, $env_var:literal) => {
        #[test]
        fn $name() {
            let Some(path) = env::var_os($env_var).map(PathBuf::from) else {
                eprintln!(
                    "{} is not set; skipping {} fixture regression",
                    $env_var, $format_name
                );
                return;
            };
            assert_camera_fixture(&path, $format);
        }
    };
}

camera_fixture_test!(
    configured_cr2_fixture_decodes_sensor_samples,
    CameraFormat::Cr2,
    "CR2",
    "RRRAH_CR2_FIXTURE"
);
camera_fixture_test!(
    configured_nef_fixture_decodes_sensor_samples,
    CameraFormat::Nef,
    "NEF",
    "RRRAH_NEF_FIXTURE"
);
camera_fixture_test!(
    configured_arw_fixture_decodes_sensor_samples,
    CameraFormat::Arw,
    "ARW",
    "RRRAH_ARW_FIXTURE"
);
camera_fixture_test!(
    configured_orf_fixture_decodes_sensor_samples,
    CameraFormat::Orf,
    "ORF",
    "RRRAH_ORF_FIXTURE"
);
camera_fixture_test!(
    configured_pef_fixture_decodes_sensor_samples,
    CameraFormat::Pef,
    "PEF",
    "RRRAH_PEF_FIXTURE"
);
camera_fixture_test!(
    configured_rw2_fixture_decodes_sensor_samples,
    CameraFormat::Rw2,
    "RW2",
    "RRRAH_RW2_FIXTURE"
);
camera_fixture_test!(
    configured_raf_fixture_decodes_sensor_samples,
    CameraFormat::Raf,
    "RAF",
    "RRRAH_RAF_FIXTURE"
);

/// Decodes `path` through the production router and asserts invariants that
/// must hold for any real camera file handled by the `format` backend,
/// without assuming camera-specific dimensions or levels.
fn assert_camera_fixture(path: &Path, format: CameraFormat) {
    let request = DecodeRequest::new(path);
    assert_eq!(
        NativeRawDecoder
            .mosaic_recipe(&request)
            .unwrap_or_else(|error| panic!("recipe for {}: {error}", path.display())),
        format.recipe(),
        "routing {} must land on the {format:?} backend (check the extension)",
        path.display()
    );
    let output = crate::decode_file(path)
        .unwrap_or_else(|error| panic!("decode {} through {format:?}: {error}", path.display()));
    let metadata = &output.mosaic.metadata;
    let pixels = &output.mosaic.pixels;

    // Geometry and pixel count must agree exactly.
    assert!(metadata.width > 0 && metadata.height > 0, "nonzero dimensions");
    let expected = usize::try_from(metadata.width)
        .ok()
        .and_then(|width| {
            usize::try_from(metadata.height)
                .ok()
                .and_then(|height| width.checked_mul(height))
        })
        .expect("fixture dimensions must fit usize");
    assert_eq!(
        pixels.len(),
        expected,
        "pixel count must match the declared {}x{} geometry",
        metadata.width,
        metadata.height
    );
    assert!(
        (1..=16).contains(&metadata.bits_per_sample),
        "bits per sample {} out of range",
        metadata.bits_per_sample
    );
    assert_eq!(
        metadata.components_per_pixel, 1,
        "camera backends output a single CFA plane"
    );
    assert!(
        matches!(metadata.photometric, Photometric::Cfa),
        "camera backends must report CFA photometric"
    );

    // Every camera backend enforces a 2x2 RGB Bayer mosaic.
    let cfa = metadata
        .cfa
        .as_ref()
        .expect("camera decoders must report a CFA pattern");
    assert_eq!((cfa.width, cfa.height), (2, 2), "only 2x2 mosaics are supported");
    assert!(
        cfa.cells
            .iter()
            .all(|cell| matches!(cell, CfaColor::Red | CfaColor::Green | CfaColor::Blue)),
        "only RGB Bayer cells are supported, got {:?}",
        cfa.cells
    );

    // Levels and white balance must be finite, positive, and ordered.
    assert!(
        metadata
            .black_level
            .values
            .iter()
            .all(|value| value.is_finite() && *value >= 0.0),
        "black level must be finite and non-negative"
    );
    assert!(
        metadata
            .white_level
            .0
            .iter()
            .all(|value| value.is_finite() && *value > 0.0),
        "white level must be finite and positive"
    );
    let max_black = metadata
        .black_level
        .values
        .iter()
        .copied()
        .fold(0.0_f32, f32::max);
    let min_white = metadata
        .white_level
        .0
        .iter()
        .copied()
        .fold(f32::INFINITY, f32::min);
    assert!(
        min_white > max_black,
        "white level ({min_white}) must exceed black level ({max_black})"
    );
    assert!(
        metadata
            .white_balance
            .iter()
            .all(|value| value.is_finite() && *value > 0.0),
        "white balance gains must be finite and positive"
    );

    // A real exposure is never a constant frame.
    let (min, max) = pixels.iter().fold((u16::MAX, u16::MIN), |(low, high), &sample| {
        (low.min(sample), high.max(sample))
    });
    assert!(min < max, "decoded mosaic is constant; not a real exposure");
}
