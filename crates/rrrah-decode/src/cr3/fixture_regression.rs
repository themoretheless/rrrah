//! Opt-in regression contracts for owner-supplied EOS R8 fixtures.
//!
//! The fixture and oracle bytes stay outside the repository. Run with:
//!
//! ```text
//! RRRAH_CR3_REGRESSION_DIR=/path/to/fixtures \
//!   cargo test -p rrrah-decode --lib cr3::fixture_regression -- --ignored
//! ```
//!
//! The directory must contain the staged file names listed in
//! [`EOS_R8_FIXTURES`]. Input fingerprints prevent a same-sized but unrelated
//! private file from being mistaken for a passing regression fixture.

use std::{
    env, fs,
    path::{Path, PathBuf},
};

use rrrah_core::{CfaColor, Orientation};

use super::native;

const FIXTURE_DIR_ENV: &str = "RRRAH_CR3_REGRESSION_DIR";
const SENSOR_WIDTH: usize = 6_188;
const SENSOR_HEIGHT: usize = 4_120;
const SENSOR_DIMENSIONS_U32: (u32, u32) = (6_188, 4_120);
const PIXEL_COUNT: usize = SENSOR_WIDTH * SENSOR_HEIGHT;
const PIXEL_ORACLE_CONTEXT: &str = "rrrah.native-cr3.pixel-oracle.v1";

#[derive(Clone, Copy)]
struct FixtureContract {
    source_file: &'static str,
    source_len: usize,
    source_blake3: &'static str,
    raw_offset: u64,
    raw_size: u32,
    raw_declared_payload_size: u32,
    ctmd_offset: u64,
    ctmd_size: u32,
    plane_lengths: [usize; 4],
    plane_ff03_tails: [u8; 4],
    plane_blake3: [&'static str; 4],
    white_balance_ratio: [u16; 4],
    oracle_file: &'static str,
    oracle_blake3: &'static str,
    pixel_blake3_le_u16: &'static str,
}

const EOS_R8_FIXTURES: [FixtureContract; 2] = [
    FixtureContract {
        source_file: "rrrah-eos-r8-9043.cr3",
        source_len: 22_382_226,
        source_blake3: "ddd7452512a75d7a2d1fa3914ea63e348178821cc616f5b5928671ba3dc399bf",
        raw_offset: 2_992_128,
        raw_size: 19_269_248,
        raw_declared_payload_size: 19_269_136,
        ctmd_offset: 22_261_760,
        ctmd_size: 120_466,
        plane_lengths: [4_631_608, 4_978_552, 4_977_664, 4_681_312],
        plane_ff03_tails: [0, 6, 6, 2],
        plane_blake3: [
            "e785b01b9a80c1a53c936e994ddef0d94d361c7b4a96c401bdfea7ee783453a6",
            "05cec9217cc963e699277e4e626ea9559c590fb50c0979551d878869d25e9e04",
            "67cb0af9e88cb29c90eae100e6e165cb491214627e2f23c043c3b9a601139f72",
            "3915154c210368920c1967a691b348b8d7386167c31df4e25f139455248354ed",
        ],
        white_balance_ratio: [1_678, 1_024, 1_659, 1_024],
        oracle_file: "rrrah-eos-r8-9043-u16le.bin",
        oracle_blake3: "93e1edb11bcc962c1689c84709f3ac0a3b0aa5b8ab19f9116e12798316d875bd",
        pixel_blake3_le_u16: "b294672c768d88768bec364cdb396078f8bccefb0d629a1bfc279a968e65cbe7",
    },
    FixtureContract {
        source_file: "rrrah-eos-r8-9074.cr3",
        source_len: 21_368_466,
        source_blake3: "8b015e4c6c82b5c644722344aa4fb25bc7fc1f8c926fbba4e482f06805fc981e",
        raw_offset: 2_845_696,
        raw_size: 18_402_048,
        raw_declared_payload_size: 18_401_936,
        ctmd_offset: 21_248_000,
        ctmd_size: 120_466,
        plane_lengths: [4_453_096, 4_742_944, 4_740_960, 4_464_936],
        plane_ff03_tails: [4, 5, 2, 0],
        plane_blake3: [
            "d1500186c4421ae8547e36074f32f6c947d8b96a95ac31f4d6c15b864e5ccdb2",
            "d61a1fc1052fdbe109e48b940e02f857fa19112ecba894738ee3b7ec1b0bbe03",
            "1bae195abf10666a3acbdb96552d663178e394de3c9327fdae518c3b1e80bb60",
            "cd976c202d7de52e786684bc7bac16ec998be4d819fc114be0e051fc258a9648",
        ],
        white_balance_ratio: [1_691, 1_024, 1_641, 1_024],
        oracle_file: "rrrah-eos-r8-9074-u16le.bin",
        oracle_blake3: "ef677bae0d39f0164e943aaa81c61c064151b5503bdead9400574ae2def9db62",
        pixel_blake3_le_u16: "2a4e3c6eb525c485873158cb35645942295a9d6b57aa1c99d14f0d2cc8873d8f",
    },
];

#[test]
#[ignore = "requires owner-supplied private EOS R8 fixtures"]
fn eos_r8_full_native_decode_matches_pixel_oracles() {
    let fixture_directory = fixture_directory();
    for contract in EOS_R8_FIXTURES {
        assert_fixture_contract(&fixture_directory, contract);
    }
}

fn assert_fixture_contract(directory: &Path, contract: FixtureContract) {
    let bytes = read_required(directory, contract.source_file);
    assert_eq!(
        bytes.len(),
        contract.source_len,
        "{} length",
        contract.source_file
    );
    assert_blake3(&bytes, contract.source_blake3, contract.source_file);

    let frame = native::parse(&bytes).unwrap_or_else(|error| {
        panic!("parse exact fixture {}: {error}", contract.source_file);
    });
    assert_eq!(
        frame.file_len,
        u64::try_from(contract.source_len).expect("fixture length fits u64")
    );
    assert_container_contract(&frame, contract);
    assert_configuration_contract(&frame);
    assert_plane_contract(&frame, contract);
    assert_metadata_contract(&frame);
    assert_ctmd_contract(&frame, contract);
    assert_oracle_contract(directory, contract);
}

fn assert_container_contract(frame: &native::NativeFrame<'_>, contract: FixtureContract) {
    assert_eq!(frame.raw_track_id, Some(3));
    assert_eq!(frame.raw_track_index, 2);
    assert_eq!(frame.raw_description_index, 0);
    assert_eq!(frame.raw_sample_location.sample_index, 0);
    assert_eq!(frame.raw_sample_location.chunk_index, 0);
    assert_eq!(frame.raw_sample_location.description_index, 1);
    assert_eq!(frame.raw_sample_location.offset, contract.raw_offset);
    assert_eq!(frame.raw_sample_location.size, contract.raw_size);
    assert_eq!(
        frame.raw_declared_payload_size,
        contract.raw_declared_payload_size
    );
    assert_eq!(
        usize::try_from(frame.raw_sample_location.size).expect("sample size fits usize"),
        super::crx::CRX_SAMPLE_HEADER_LEN + contract.plane_lengths.iter().sum::<usize>()
    );
}

fn assert_configuration_contract(frame: &native::NativeFrame<'_>) {
    let compression = frame.config.compression;
    assert_eq!(compression.sample_precision, 15);
    assert_eq!(compression.version, 0x0100);
    assert_eq!(
        (compression.image_width, compression.image_height),
        SENSOR_DIMENSIONS_U32
    );
    assert_eq!(
        (compression.tile_width, compression.tile_height),
        SENSOR_DIMENSIONS_U32
    );
    assert_eq!(compression.n_bits, 14);
    assert_eq!(compression.plane_count, 4);
    assert_eq!(
        usize::try_from(compression.sample_header_size).expect("header size fits usize"),
        super::crx::CRX_SAMPLE_HEADER_LEN
    );
    assert_eq!(compression.format_tail, [0, 0]);
    assert_eq!(compression.raw_plane_configs, [[1, 1, 0, 0]; 4]);

    let geometry = frame
        .config
        .image_description
        .eos_r8_sensor_geometry()
        .expect("exact EOS R8 sensor geometry");
    assert_eq!(
        (
            geometry.active_area.x,
            geometry.active_area.y,
            geometry.active_area.width,
            geometry.active_area.height
        ),
        (156, 96, 6_022, 4_020)
    );
    assert_eq!(
        (
            geometry.crop_area.x,
            geometry.crop_area.y,
            geometry.crop_area.width,
            geometry.crop_area.height
        ),
        (168, 108, 6_000, 4_000)
    );
}

fn assert_plane_contract(frame: &native::NativeFrame<'_>, contract: FixtureContract) {
    assert_eq!(
        frame.planes.each_ref().map(|plane| plane.data.len()),
        contract.plane_lengths
    );
    assert_eq!(
        frame.planes.each_ref().map(|plane| plane.plane_index),
        [0, 1, 2, 3]
    );
    assert_eq!(
        frame.planes.each_ref().map(|plane| plane.quantization_parameter),
        [4; 4]
    );
    assert_eq!(
        frame.planes.each_ref().map(|plane| plane.empirical_ff03_tail),
        contract.plane_ff03_tails
    );
    assert_eq!(
        frame.planes.each_ref().map(|plane| plane.raw_ff02_descriptor),
        [[0x08, 0, 0, 0], [0x18, 0, 0, 0], [0x28, 0, 0, 0], [0x38, 0, 0, 0]]
    );
    for (plane, expected_hash) in frame.planes.iter().zip(contract.plane_blake3) {
        assert_blake3(
            plane.data,
            expected_hash,
            &format!("{} plane {}", contract.source_file, plane.plane_index),
        );
    }
}

fn assert_metadata_contract(frame: &native::NativeFrame<'_>) {
    assert_eq!(frame.metadata.recorded_make, "Canon");
    assert_eq!(frame.metadata.recorded_model, "Canon EOS R8");
    assert_eq!(frame.metadata.orientation, Orientation::Normal);
    assert_eq!(frame.metadata.profile.bits_per_sample, 14);
    assert_eq!(
        frame.metadata.profile.cfa,
        [CfaColor::Red, CfaColor::Green, CfaColor::Green, CfaColor::Blue]
    );
    assert_eq!(
        frame.metadata.profile.black_level.map(f32::to_bits),
        [512.0_f32.to_bits(); 4]
    );
    assert_eq!(
        frame.metadata.profile.white_level.to_bits(),
        12_735.0_f32.to_bits()
    );
    assert_eq!(
        frame
            .metadata
            .profile
            .xyz_to_camera
            .map(|row| row.map(f32::to_bits)),
        [
            [
                0.9539_f32.to_bits(),
                (-0.2795_f32).to_bits(),
                (-0.1224_f32).to_bits()
            ],
            [
                (-0.4175_f32).to_bits(),
                1.1998_f32.to_bits(),
                0.2458_f32.to_bits()
            ],
            [
                (-0.0465_f32).to_bits(),
                0.1755_f32.to_bits(),
                0.6048_f32.to_bits()
            ],
            [0.0_f32.to_bits(); 3],
        ]
    );
}

fn assert_ctmd_contract(frame: &native::NativeFrame<'_>, contract: FixtureContract) {
    assert_eq!(frame.ctmd_track_id, Some(4));
    assert_eq!(frame.ctmd_sample_location.sample_index, 0);
    assert_eq!(frame.ctmd_sample_location.chunk_index, 0);
    assert_eq!(frame.ctmd_sample_location.description_index, 1);
    assert_eq!(frame.ctmd_sample_location.offset, contract.ctmd_offset);
    assert_eq!(frame.ctmd_sample_location.size, contract.ctmd_size);
    assert_eq!(
        [
            frame.as_shot_white_balance.red_numerator,
            frame.as_shot_white_balance.red_denominator,
            frame.as_shot_white_balance.blue_numerator,
            frame.as_shot_white_balance.blue_denominator,
        ],
        contract.white_balance_ratio
    );
}

fn assert_oracle_contract(directory: &Path, contract: FixtureContract) {
    let oracle = read_required(directory, contract.oracle_file);
    assert_eq!(oracle.len(), PIXEL_COUNT * size_of::<u16>());
    assert_blake3(&oracle, contract.oracle_blake3, contract.oracle_file);
    assert_eq!(
        pixel_oracle_digest(&oracle),
        contract.pixel_blake3_le_u16,
        "{} trusted pixel digest",
        contract.oracle_file
    );
    let decoded = crate::decode_file(directory.join(contract.source_file))
        .unwrap_or_else(|error| panic!("production decode {}: {error}", contract.source_file));
    let mosaic = &decoded.mosaic.pixels;
    assert_eq!(
        (
            decoded.mosaic.metadata.width,
            decoded.mosaic.metadata.height,
            decoded.mosaic.metadata.bits_per_sample,
            decoded.mosaic.metadata.orientation,
        ),
        (
            u32::try_from(SENSOR_WIDTH).expect("sensor width fits u32"),
            u32::try_from(SENSOR_HEIGHT).expect("sensor height fits u32"),
            14,
            Orientation::Normal,
        )
    );
    assert_eq!(
        decoded
            .timings
            .native
            .expect("native timing breakdown")
            .worker_count,
        4
    );
    assert_eq!(mosaic.len(), PIXEL_COUNT);
    for (index, (actual, expected)) in mosaic
        .iter()
        .copied()
        .zip(
            oracle
                .chunks_exact(2)
                .map(|bytes| u16::from_le_bytes([bytes[0], bytes[1]])),
        )
        .enumerate()
    {
        assert_eq!(
            actual, expected,
            "{} full mosaic sample {index}",
            contract.source_file
        );
    }
    assert_eq!(
        decoded_pixel_digest(mosaic),
        contract.pixel_blake3_le_u16,
        "{} decoded pixel digest",
        contract.source_file
    );
}

fn fixture_directory() -> PathBuf {
    env::var_os(FIXTURE_DIR_ENV).map_or_else(
        || panic!("{FIXTURE_DIR_ENV} must point to the private fixture directory"),
        PathBuf::from,
    )
}

fn read_required(directory: &Path, file_name: &str) -> Vec<u8> {
    let path = directory.join(file_name);
    fs::read(&path).unwrap_or_else(|error| panic!("read fixture {}: {error}", path.display()))
}

fn assert_blake3(bytes: &[u8], expected: &str, label: &str) {
    assert_eq!(blake3::hash(bytes).to_hex().as_str(), expected, "{label} BLAKE3");
}

fn pixel_oracle_digest(le_u16_bytes: &[u8]) -> String {
    let mut hasher = blake3::Hasher::new_derive_key(PIXEL_ORACLE_CONTEXT);
    hasher.update(le_u16_bytes);
    hasher.finalize().to_hex().to_string()
}

fn decoded_pixel_digest(samples: &[u16]) -> String {
    let mut hasher = blake3::Hasher::new_derive_key(PIXEL_ORACLE_CONTEXT);
    for sample in samples {
        hasher.update(&sample.to_le_bytes());
    }
    hasher.finalize().to_hex().to_string()
}
