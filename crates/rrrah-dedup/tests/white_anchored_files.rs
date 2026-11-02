#![cfg(feature = "decode")]
use rrrah_core::MemoryBudget;
use rrrah_decode::DecodeRequest;
use rrrah_dedup::{
    animated::AnimationBudget,
    geometry::Transform,
    warp::{PhotometricPolicy, WarpPolicy},
    white_anchored_file::verify_white_anchored_files as verify,
};
use std::{cell::Cell, fs};

const ID: Transform = Transform {
    a: 1.0,
    b: 0.0,
    translation: [0.0; 2],
};
const DECODE: AnimationBudget = AnimationBudget {
    max_frames: 1,
    max_pixels: 20,
    max_file_bytes: 1024,
};
const FIT: PhotometricPolicy = PhotometricPolicy {
    residual: WarpPolicy {
        tolerance: 1e-6,
        max_source_pixels: 100,
    },
    minimum_samples: 8,
    minimum_variance: 1e-5,
    minimum_gain: 0.9,
    maximum_gain: 1.1,
    maximum_offset: 0.1,
};
fn bmp() -> Vec<u8> {
    let mut b = vec![0_u8; 118];
    b[..2].copy_from_slice(b"BM");
    for (offset, value) in [(2, 118_u32), (10, 54), (14, 40), (18, 5), (22, 4), (34, 64)] {
        b[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
    }
    b[26..28].copy_from_slice(&1_u16.to_le_bytes());
    b[28..30].copy_from_slice(&24_u16.to_le_bytes());
    for y in 0..4 {
        for x in 0..5 {
            let at = 54 + y * 16 + x * 3;
            b[at..at + 3].copy_from_slice(&[
                20 + (x * 30) as u8,
                30 + (y * 40) as u8,
                15 + ((x + y) * 20) as u8,
            ]);
        }
    }
    b
}

#[test]
fn files_cancel_at_every_checkpoint_release_memory_and_retry() {
    let dir = tempfile::tempdir().unwrap();
    let a = dir.path().join("a.bmp");
    let b = dir.path().join("b.bmp");
    fs::write(&a, bmp()).unwrap();
    fs::write(&b, bmp()).unwrap();
    let (a, b) = (DecodeRequest::new(a), DecodeRequest::new(b));
    let memory = MemoryBudget::new(1024 * 1024);
    let calls = Cell::new(0);
    let baseline = verify(&a, &b, ID, DECODE, FIT, &memory, || {
        calls.set(calls.get() + 1);
        false
    })
    .unwrap();
    assert_eq!(baseline.projected.evidence.forward.pixels.matched_pixels, 20);
    assert_eq!(memory.used(), 0);
    for stop in 1..=calls.get() {
        let seen = Cell::new(0);
        assert!(
            verify(&a, &b, ID, DECODE, FIT, &memory, || {
                seen.set(seen.get() + 1);
                seen.get() == stop
            })
            .is_err(),
            "checkpoint {stop}"
        );
        assert_eq!(memory.used(), 0);
    }
    assert_eq!(
        verify(&a, &b, ID, DECODE, FIT, &memory, || false).unwrap(),
        baseline
    );
    assert!(verify(&a, &b, ID, DECODE, FIT, &MemoryBudget::new(0), || false).is_err());
}

#[test]
fn final_callback_source_edit_cannot_publish_stale_evidence() {
    let dir = tempfile::tempdir().unwrap();
    let a = dir.path().join("a.bmp");
    let b = dir.path().join("b.bmp");
    fs::write(&a, bmp()).unwrap();
    fs::write(&b, bmp()).unwrap();
    let (left, right) = (DecodeRequest::new(&a), DecodeRequest::new(&b));
    let memory = MemoryBudget::new(1024 * 1024);
    let calls = Cell::new(0);
    verify(&left, &right, ID, DECODE, FIT, &memory, || {
        calls.set(calls.get() + 1);
        false
    })
    .unwrap();
    let stop = calls.get();
    for changed_path in [&a, &b] {
        fs::write(&a, bmp()).unwrap();
        fs::write(&b, bmp()).unwrap();
        let seen = Cell::new(0);
        let result = verify(&left, &right, ID, DECODE, FIT, &memory, || {
            seen.set(seen.get() + 1);
            if seen.get() == stop {
                let mut changed = bmp();
                changed[54] ^= 63;
                fs::write(changed_path, changed).unwrap();
            }
            false
        });
        assert!(
            result.is_err(),
            "published stale result after final callback edit of {changed_path:?}"
        );
        assert_eq!(memory.used(), 0);
    }
}
