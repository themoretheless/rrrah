//! Opt-in regression contracts for redistributable DNG fixtures.
//!
//! Download the matching CC0 files from raw.pixls.us, stage them under the
//! names below, and run:
//!
//! ```text
//! RRRAH_DNG_REGRESSION_DIR=/path/to/fixtures \
//!   cargo test -p rrrah-decode --lib dng::fixture_regression -- --ignored
//! ```

use std::{env, fs, path::PathBuf};

use super::{Compression, parse};

const FIXTURE_DIR_ENV: &str = "RRRAH_DNG_REGRESSION_DIR";

#[derive(Debug, Clone, Copy)]
struct FixtureContract {
    file: &'static str,
    source_len: usize,
    dimensions: (u32, u32),
    stored_bits: u8,
    compression: Compression,
    pixel_blake3_le_u16: &'static str,
}

const FIXTURES: [FixtureContract; 3] = [
    FixtureContract {
        file: "rrrah-leica-m8-uncompressed.dng",
        source_len: 10_575_296,
        dimensions: (3_920, 2_638),
        stored_bits: 8,
        compression: Compression::Uncompressed,
        pixel_blake3_le_u16: "e90b617fe1e75edcd85a3510b5ca7f3016ce3bee267a1fae66c751aa1b24c117",
    },
    FixtureContract {
        file: "rrrah-canon-a410-packed10.dng",
        source_len: 4_219_200,
        dimensions: (2_144, 1_560),
        stored_bits: 10,
        compression: Compression::Uncompressed,
        pixel_blake3_le_u16: "0981694d0816951e7973a76550ffd81f77f4b90ecda6d5593e2a3b53e8e84eac",
    },
    FixtureContract {
        file: "rrrah-canon-ljpeg.dng",
        source_len: 1_538_165,
        dimensions: (1_920, 818),
        stored_bits: 14,
        compression: Compression::LosslessJpeg,
        pixel_blake3_le_u16: "8d646e9ec3324c8e1b7643795322fd702e6b3482b2bdfb7781398df89333dbb8",
    },
];

#[test]
#[ignore = "requires separately downloaded CC0 DNG fixtures"]
fn real_dng_pixels_match_exact_oracles() {
    let directory = fixture_directory();
    for contract in FIXTURES {
        let path = directory.join(contract.file);
        let bytes = fs::read(&path).unwrap_or_else(|error| panic!("read {}: {error}", path.display()));
        assert_eq!(
            bytes.len(),
            contract.source_len,
            "{} source length",
            contract.file
        );
        let image = parse(&bytes)
            .unwrap_or_else(|error| panic!("parse {} through native DNG: {error}", contract.file));
        assert_eq!((image.width, image.height), contract.dimensions);
        assert_eq!(image.stored_bits_per_sample, contract.stored_bits);
        assert_eq!(image.compression, contract.compression);
        let decoded = image
            .decode_u16(&|| false)
            .unwrap_or_else(|error| panic!("decode {} through native DNG: {error}", contract.file));
        assert_eq!(
            pixel_digest(&decoded.pixels),
            contract.pixel_blake3_le_u16,
            "{} exact LE-u16 mosaic",
            contract.file
        );
    }
}

#[test]
#[ignore = "requires separately downloaded non-lossless DNG fixture"]
fn lossy_jpeg_dng_is_rejected_instead_of_misdecoded() {
    let path = fixture_directory().join("rrrah-blackmagic-ljpeg.dng");
    let bytes = fs::read(&path).unwrap_or_else(|error| panic!("read {}: {error}", path.display()));
    let image = parse(&bytes).unwrap_or_else(|error| panic!("parse {}: {error}", path.display()));
    let error = image
        .decode_u16(&|| false)
        .expect_err("DQT-bearing JPEG must not enter the SOF3 lossless path");
    assert!(
        error.to_string().contains("0xdb"),
        "expected explicit DQT marker rejection, got: {error}"
    );
}

fn fixture_directory() -> PathBuf {
    env::var_os(FIXTURE_DIR_ENV).map_or_else(
        || panic!("{FIXTURE_DIR_ENV} must point to the staged fixture directory"),
        PathBuf::from,
    )
}

fn pixel_digest(pixels: &[u16]) -> String {
    const SAMPLES_PER_CHUNK: usize = 8_192;
    let mut hasher = blake3::Hasher::new();
    let mut encoded = [0_u8; SAMPLES_PER_CHUNK * 2];
    for chunk in pixels.chunks(SAMPLES_PER_CHUNK) {
        let bytes = &mut encoded[..chunk.len() * 2];
        for (sample, destination) in chunk.iter().zip(bytes.chunks_exact_mut(2)) {
            destination.copy_from_slice(&sample.to_le_bytes());
        }
        hasher.update(bytes);
    }
    hasher.finalize().to_hex().to_string()
}
