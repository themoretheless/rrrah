#![cfg(feature = "decode")]
use rrrah_core::MemoryBudget;
use rrrah_decode::DecodeRequest;
use rrrah_dedup::{animated::AnimationBudget, decode::decode_selected_frame_bounded};

// Authored 2x2 DIB entries: bottom-up XOR rows followed by 32-bit padded AND rows.
fn dib(depth: u16, masked: bool) -> Vec<u8> {
    let mut b = vec![0u8; 64];
    for (offset, value) in [(0, 40u32), (4, 2), (8, 4), (20, 24)] {
        b[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
    }
    b[12..14].copy_from_slice(&1u16.to_le_bytes());
    b[14..16].copy_from_slice(&depth.to_le_bytes());
    for y in 0..2 {
        for x in 0..2 {
            let at = 40 + y * 8 + x * usize::from(depth / 8);
            if depth == 32 {
                b[at..at + 4].copy_from_slice(&[0, 0, 255, 128]);
            } else {
                b[at..at + 3].copy_from_slice(&[255, 0, 0]);
            }
        }
    }
    if masked {
        b[56] = 0x80;
    } // bottom-left display pixel is transparent.
    b
}
fn icon(masked: bool) -> Vec<u8> {
    let mut b = vec![0u8; 38];
    b[2..4].copy_from_slice(&1u16.to_le_bytes());
    b[4..6].copy_from_slice(&2u16.to_le_bytes());
    for (index, depth) in [(0, 32u16), (1, 24)] {
        let payload = dib(depth, masked && depth == 24);
        let offset = b.len();
        let at = 6 + index * 16;
        b[at] = 2;
        b[at + 1] = 2;
        b[at + 4..at + 6].copy_from_slice(&1u16.to_le_bytes());
        b[at + 6..at + 8].copy_from_slice(&depth.to_le_bytes());
        b[at + 8..at + 12].copy_from_slice(&(payload.len() as u32).to_le_bytes());
        b[at + 12..at + 16].copy_from_slice(&(offset as u32).to_le_bytes());
        b.extend(payload);
    }
    b
}
const LIMITS: AnimationBudget = AnimationBudget {
    max_frames: 2,
    max_pixels: 4,
    max_file_bytes: 1024,
};

#[test]
fn dib_icon_depth_rank_alpha_and_and_mask_survive_normalization() {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("authored.ico");
    for masked in [false, true] {
        std::fs::write(&path, icon(masked)).unwrap();
        for index in 0..2 {
            let mut request = DecodeRequest::new(&path);
            request.image_index = index;
            let memory = MemoryBudget::new(4 * 1024 * 1024);
            let frame = decode_selected_frame_bounded(&request, LIMITS, &memory, || false).unwrap();
            assert_eq!((frame.image_index(), frame.image_count()), (index, 2));
            let view = frame.view(|| false).unwrap();
            assert_eq!(view.dimensions(), (2, 2));
            for y in 0..2 {
                for x in 0..2 {
                    let pixel = view.rgba(x, y).unwrap();
                    let alpha = if index == 0 {
                        128.0 / 255.0
                    } else if masked && x == 0 && y == 1 {
                        0.0
                    } else {
                        1.0
                    };
                    assert!(
                        (pixel[3] - alpha).abs() < 1e-7,
                        "index={index}, masked={masked}, pixel={pixel:?}"
                    );
                    if alpha > 0.0 {
                        assert_eq!(
                            &pixel[..3],
                            if index == 0 {
                                &[1.0, 0.0, 0.0]
                            } else {
                                &[0.0, 0.0, 1.0]
                            }
                        );
                    }
                }
            }
            drop(frame);
            assert_eq!(memory.used(), 0);
        }
    }
}

#[test]
fn dib_icon_limits_and_invalid_frame_refuse_without_retained_memory() {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("authored.ico");
    std::fs::write(&path, icon(false)).unwrap();
    let mut request = DecodeRequest::new(&path);
    for (limits, bytes, index) in [
        (LIMITS, 0u64, 0),
        (
            AnimationBudget {
                max_pixels: 3,
                ..LIMITS
            },
            4 * 1024 * 1024,
            0,
        ),
        (
            AnimationBudget {
                max_frames: 1,
                ..LIMITS
            },
            4 * 1024 * 1024,
            0,
        ),
        (LIMITS, 4 * 1024 * 1024, 2),
    ] {
        request.image_index = index;
        let memory = MemoryBudget::new(bytes);
        let result = decode_selected_frame_bounded(&request, limits, &memory, || false);
        assert!(result.is_err(), "limits={limits:?}, index={index}");
        drop(result);
        assert_eq!(memory.used(), 0);
    }
}

#[test]
fn icon_directory_count_is_refused_before_managed_pixel_allocation() {
    let root = tempfile::tempdir().unwrap();
    for kind in [1u16, 2] {
        let mut bytes = icon(false);
        bytes[2..4].copy_from_slice(&kind.to_le_bytes());
        // For CUR these directory words are valid hotspots instead of depth.
        if kind == 2 {
            for at in [6usize, 22] {
                bytes[at + 4..at + 8].fill(0);
            }
        }
        let path = root
            .path()
            .join(if kind == 1 { "frame.ico" } else { "frame.cur" });
        std::fs::write(&path, bytes).unwrap();
        let budget = MemoryBudget::new(0);
        let error = decode_selected_frame_bounded(
            &DecodeRequest::new(path),
            AnimationBudget {
                max_frames: 1,
                ..LIMITS
            },
            &budget,
            || false,
        )
        .unwrap_err();
        assert!(matches!(
            error,
            rrrah_dedup::decode::CachedError::FrameLimit {
                observed: 2,
                limit: 1
            }
        ));
        assert_eq!((budget.used(), budget.peak()), (0, 0));
    }
}

#[test]
fn persisted_icon_fingerprints_cannot_bypass_stricter_frame_admission() {
    use rrrah_dedup::{
        cache::FingerprintCache,
        decode::{CachedError, FingerprintPolicy, fingerprint_file},
    };
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("authored.ico");
    let disk = root.path().join("cache.bin");
    std::fs::write(&path, icon(false)).unwrap();
    let budget = MemoryBudget::new(4 * 1024 * 1024);
    let policy = FingerprintPolicy {
        recipe: [0; 32],
        max_file_bytes: 1024,
        max_pixels: 4,
        max_frames: 2,
        max_cache_entries: 8,
    };
    let mut cache = FingerprintCache::default();
    let mut expected = Vec::new();
    for index in 0..2 {
        let mut request = DecodeRequest::new(&path);
        request.image_index = index;
        let cold = fingerprint_file(&request, policy, &budget, &mut cache, || false).unwrap();
        assert!(!cold.cache_hit, "selected index {index} reused another index");
        let warm = fingerprint_file(&request, policy, &budget, &mut cache, || false).unwrap();
        assert!(warm.cache_hit);
        assert_eq!(cold.fingerprint, warm.fingerprint);
        expected.push(cold.fingerprint);
        assert_eq!(budget.used(), 0);
    }
    cache.save_atomic(&disk, || false).unwrap();
    let mut loaded = FingerprintCache::load_file(&disk, 8, || false).unwrap();
    for index in [1usize, 0] {
        let mut request = DecodeRequest::new(&path);
        request.image_index = index;
        let error = fingerprint_file(
            &request,
            FingerprintPolicy {
                max_frames: 1,
                ..policy
            },
            &budget,
            &mut loaded,
            || false,
        )
        .unwrap_err();
        assert!(matches!(
            error,
            CachedError::FrameLimit {
                observed: 2,
                limit: 1
            }
        ));
        assert_eq!(budget.used(), 0);
        let warm = fingerprint_file(&request, policy, &budget, &mut loaded, || false).unwrap();
        assert!(warm.cache_hit, "refusal discarded a valid cached index {index}");
        assert_eq!(warm.fingerprint, expected[index]);
        assert_eq!(budget.used(), 0);
    }
}
