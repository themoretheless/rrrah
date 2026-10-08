//! Pinned test-only upstream QOI encoder/decoder fixtures.
use rrrah_core::{MemoryBudget, RasterColorSpace, RasterPixels};
use rrrah_decode::{DecodeRequest, decode_raster};
use std::path::PathBuf;

#[test]
fn upstream_qoi_corpus_preserves_every_sample_flag_and_alpha() {
    let directory = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/qoi");
    let manifest: serde_json::Value =
        serde_json::from_slice(&std::fs::read(directory.join("manifest.json")).unwrap()).unwrap();
    assert_eq!(manifest["fixtures"].as_array().unwrap().len(), 8);
    for fixture in manifest["fixtures"].as_array().unwrap() {
        let name = fixture["source"].as_str().unwrap();
        let budget = MemoryBudget::new(1024 * 1024);
        let mut request = DecodeRequest::new(directory.join(name));
        request.memory_budget = Some(budget.clone());
        let raster = decode_raster(&request).unwrap();
        assert_eq!(u64::from(raster.width()), fixture["width"].as_u64().unwrap());
        assert_eq!(u64::from(raster.height()), fixture["height"].as_u64().unwrap());
        let color = if fixture["flag"] == 0 {
            RasterColorSpace::Srgb
        } else {
            RasterColorSpace::LinearSrgb
        };
        assert_eq!(raster.color_space(), &color, "{name}");
        let RasterPixels::Rgba8(pixels) = raster.pixels() else {
            panic!("{name}: wrong precision")
        };
        let expected = std::fs::read(directory.join(fixture["oracle"].as_str().unwrap())).unwrap();
        assert_eq!(&pixels[..], expected, "{name}");
        drop(raster);
        assert_eq!(budget.used(), 0, "{name}: retained credit");
    }
}

#[test]
fn malformed_qoi_rejects_truncation_run_overflow_and_trailing_data_without_credit() {
    let source = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/qoi/all-opcodes-0.qoi");
    let bytes = std::fs::read(source).unwrap();
    let path = std::env::temp_dir().join(format!(
        "rrrah-qoi-malformed-{}-{}.qoi",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let budget = MemoryBudget::new(1024 * 1024);
    let mut request = DecodeRequest::new(&path);
    request.memory_budget = Some(budget.clone());
    for length in 0..bytes.len() {
        std::fs::write(&path, &bytes[..length]).unwrap();
        assert!(decode_raster(&request).is_err(), "accepted prefix {length}");
        assert_eq!(budget.used(), 0);
    }
    let mut overflow = b"qoif".to_vec();
    overflow.extend(1u32.to_be_bytes());
    overflow.extend(1u32.to_be_bytes());
    overflow.extend([4, 0, 0xfd]);
    overflow.extend([0, 0, 0, 0, 0, 0, 0, 1]);
    std::fs::write(&path, overflow).unwrap();
    assert!(
        decode_raster(&request).is_err(),
        "accepted 62-pixel run in 1-pixel image"
    );
    assert_eq!(budget.used(), 0);
    for (offset, replacement) in [
        (4, vec![0, 0, 0, 0]),
        (8, vec![0, 0, 0, 0]),
        (12, vec![2]),
        (13, vec![2]),
        (4, vec![255; 8]),
    ] {
        let mut invalid = bytes.clone();
        invalid[offset..offset + replacement.len()].copy_from_slice(&replacement);
        std::fs::write(&path, invalid).unwrap();
        assert!(
            decode_raster(&request).is_err(),
            "accepted invalid header at {offset}"
        );
        assert_eq!(budget.used(), 0);
    }
    let mut marker_damage = bytes.clone();
    *marker_damage.last_mut().unwrap() = 2;
    std::fs::write(&path, marker_damage).unwrap();
    assert!(decode_raster(&request).is_err());
    assert_eq!(budget.used(), 0);
    let mut trailing = bytes;
    trailing.push(0);
    std::fs::write(&path, trailing).unwrap();
    assert!(
        decode_raster(&request).is_err(),
        "accepted bytes after end marker"
    );
    assert_eq!(budget.used(), 0);
    std::fs::remove_file(path).unwrap();
}
