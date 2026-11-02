#![cfg(feature = "decode")]
use rrrah_core::MemoryBudget;
use rrrah_decode::DecodeRequest;
use rrrah_dedup::{animated::AnimationBudget, decode::decode_selected_frame_bounded};

#[test]
fn persisted_dicom_frames_do_not_bypass_stricter_container_limit() {
    use rrrah_dedup::{
        cache::FingerprintCache,
        decode::{CachedError, FingerprintPolicy, fingerprint_file},
    };
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("three-frames.dcm");
    std::fs::write(
        &path,
        include_bytes!("../../../tests/fixtures/raster/dicom-rgb-explicit-8-planar0-srgb.dcm"),
    )
    .unwrap();
    let memory = MemoryBudget::new(64 * 1024 * 1024);
    let policy = FingerprintPolicy {
        recipe: [0; 32],
        max_file_bytes: 16 * 1024 * 1024,
        max_pixels: 1_000_000,
        max_frames: 3,
        max_cache_entries: 8,
    };
    let mut cache = FingerprintCache::default();
    let mut expected = Vec::new();
    for index in 0..3 {
        let mut request = DecodeRequest::new(&path);
        request.image_index = index;
        request.assume_untagged_srgb = true;
        let cold = fingerprint_file(&request, policy, &memory, &mut cache, || false).unwrap();
        assert!(!cold.cache_hit);
        expected.push(cold.fingerprint);
        assert_eq!(memory.used(), 0);
    }
    let disk = root.path().join("cache.bin");
    cache.save_atomic(&disk, || false).unwrap();
    let mut loaded = FingerprintCache::load_file(&disk, 8, || false).unwrap();
    for index in [2usize, 0, 1] {
        let mut request = DecodeRequest::new(&path);
        request.image_index = index;
        request.assume_untagged_srgb = true;
        let error = fingerprint_file(
            &request,
            FingerprintPolicy {
                max_frames: 2,
                ..policy
            },
            &memory,
            &mut loaded,
            || false,
        )
        .unwrap_err();
        assert!(matches!(
            error,
            CachedError::FrameLimit {
                observed: 3,
                limit: 2
            }
        ));
        assert_eq!(memory.used(), 0);
        let warm = fingerprint_file(&request, policy, &memory, &mut loaded, || false).unwrap();
        assert!(warm.cache_hit, "refusal discarded valid frame {index}");
        assert_eq!(warm.fingerprint, expected[index]);
        assert_eq!(memory.used(), 0);
    }
}

#[test]
fn dicom_marker_wins_over_other_container_signatures_in_arbitrary_preamble() {
    let bytes = include_bytes!("../../../tests/fixtures/raster/dicom-rgb-explicit-8-planar0-srgb.dcm");
    assert_eq!(&bytes[128..132], b"DICM");
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("base.dcm");
    std::fs::write(&path, bytes).unwrap();
    let memory = MemoryBudget::new(64 * 1024 * 1024);
    let limits = AnimationBudget {
        max_frames: 256,
        max_pixels: 1_000_000,
        max_file_bytes: 16 * 1024 * 1024,
    };
    let mut request = DecodeRequest::new(&path);
    request.assume_untagged_srgb = true;
    let first = decode_selected_frame_bounded(&request, limits, &memory, || false).unwrap();
    let count = first.image_count();
    drop(first);
    for index in 0..count {
        request.image_index = index;
        let baseline = decode_selected_frame_bounded(&request, limits, &memory, || false).unwrap();
        for (name, prefix) in [
            ("misleading.tif", &[b'I', b'I', 42, 0, 255, 255][..]),
            ("misleading-big-endian.tif", &[b'M', b'M', 0, 42, 255, 255][..]),
            ("misleading-bigtiff-le.tif", &[b'I', b'I', 43, 0, 255, 255][..]),
            ("misleading-bigtiff-be.tif", &[b'M', b'M', 0, 43, 255, 255][..]),
            ("misleading.ico", &[0, 0, 1, 0, 255, 255][..]),
            ("misleading.cur", &[0, 0, 2, 0, 255, 255][..]),
            ("misleading.png", &b"\x89PNG\r\n\x1a\n"[..]),
            ("misleading.jpg", &[0xff, 0xd8, 0xff, 0xe0][..]),
            ("misleading.gif", &b"GIF89a"[..]),
            ("misleading.bmp", &b"BM"[..]),
            ("misleading.crw", &b"II\x1a\0\0\0HEAPCCDR"[..]),
            ("misleading.raf", &b"FUJIFILMCCD-RAW "[..]),
            ("misleading.mrw", &b"\0MRM"[..]),
        ] {
            let mut changed = bytes.to_vec();
            changed[..prefix.len()].copy_from_slice(prefix);
            let altered = root.path().join(name);
            std::fs::write(&altered, changed).unwrap();
            let mut other = DecodeRequest::new(altered);
            other.assume_untagged_srgb = true;
            other.image_index = index;
            let decoded = decode_selected_frame_bounded(&other, limits, &memory, || false)
                .unwrap_or_else(|error| panic!("{name} frame {index}: {error:?}"));
            assert_eq!((decoded.image_index(), decoded.image_count()), (index, count));
            assert!(baseline.same_selected_frame(&decoded, || false).unwrap());
            drop(decoded);
        }
        drop(baseline);
        assert_eq!(memory.used(), 0);
    }
}
