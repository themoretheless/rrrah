#![cfg(feature = "decode")]
use rrrah_core::MemoryBudget;
use rrrah_decode::DecodeRequest;
use rrrah_dedup::animated::{AnimationBudget, AnimationKind, decode_animation};
fn root() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/raster")
}
const LIMITS: AnimationBudget = AnimationBudget {
    max_frames: 10,
    max_pixels: 100,
    max_file_bytes: 100_000,
};

#[test]
fn complete_existing_composition_fixtures_decode_and_compare() {
    for (name, kind) in [
        ("gif-animation-disposal-1.gif", AnimationKind::Gif),
        ("apng-compose-0-0.apng", AnimationKind::Apng),
        ("webp-animation-0.webp", AnimationKind::Webp),
    ] {
        let budget = MemoryBudget::new(4 * 1024 * 1024);
        let request = DecodeRequest::new(root().join(name));
        let a = decode_animation(&request, kind, LIMITS, &budget, || false).unwrap();
        let b = decode_animation(&request, kind, LIMITS, &budget, || false).unwrap();
        assert!(a.frame_count() > 1);
        assert!(a.same_animation(&b, || false).unwrap());
        assert!(
            decode_animation(
                &request,
                kind,
                AnimationBudget {
                    max_frames: 1,
                    ..LIMITS
                },
                &budget,
                || false
            )
            .is_err()
        );
        assert!(a.same_animation(&b, || true).is_err());
    }
}

#[test]
fn changed_gif_timing_prevents_whole_animation_equality() {
    let original = root().join("gif-animation-disposal-1.gif");
    let mut bytes = std::fs::read(&original).unwrap();
    let control = bytes.windows(3).position(|v| v == [0x21, 0xf9, 4]).unwrap();
    bytes[control + 4] += 1;
    let dir = tempfile::tempdir().unwrap();
    let changed = dir.path().join("changed.gif");
    std::fs::write(&changed, bytes).unwrap();
    let budget = MemoryBudget::new(4 * 1024 * 1024);
    let a = decode_animation(
        &DecodeRequest::new(original),
        AnimationKind::Gif,
        LIMITS,
        &budget,
        || false,
    )
    .unwrap();
    let b = decode_animation(
        &DecodeRequest::new(changed),
        AnimationKind::Gif,
        LIMITS,
        &budget,
        || false,
    )
    .unwrap();
    assert!(!a.same_animation(&b, || false).unwrap());
}

#[test]
fn independent_cross_container_timeline_splits_and_negatives() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/timeline");
    let budget = MemoryBudget::new(4 * 1024 * 1024);
    let load = |name, kind| {
        decode_animation(
            &DecodeRequest::new(root.join(name)),
            kind,
            LIMITS,
            &budget,
            || false,
        )
        .unwrap()
    };
    let split = load("split.apng", AnimationKind::Apng);
    assert_eq!(split.frame_count(), 4);
    for (name, kind) in [
        ("base.gif", AnimationKind::Gif),
        ("base.png", AnimationKind::Apng),
        ("base.webp", AnimationKind::Webp),
    ] {
        let base = load(name, kind);
        assert_eq!(base.frame_count(), 2);
        assert!(!base.same_animation(&split, || false).unwrap());
        assert!(base.same_timeline(&split, || false).unwrap(), "{name}");
        assert!(split.same_timeline(&base, || false).unwrap(), "{name}");
        for negative in ["changed-time.apng", "changed-pixels.apng"] {
            assert!(
                !base
                    .same_timeline(&load(negative, AnimationKind::Apng), || false)
                    .unwrap(),
                "{name} vs {negative}"
            );
        }
    }
}

#[test]
fn gif_disposal_modes_match_independently_composited_full_canvas_apng() {
    let oracle_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/disposal");
    let budget = MemoryBudget::new(4 * 1024 * 1024);
    let mut animations = Vec::new();
    for mode in 1..=3 {
        let gif = decode_animation(
            &DecodeRequest::new(root().join(format!("gif-animation-disposal-{mode}.gif"))),
            AnimationKind::Gif,
            LIMITS,
            &budget,
            || false,
        )
        .unwrap();
        let oracle = decode_animation(
            &DecodeRequest::new(oracle_root.join(format!("disposal-{mode}.apng"))),
            AnimationKind::Apng,
            LIMITS,
            &budget,
            || false,
        )
        .unwrap();
        assert_eq!(gif.frame_count(), 4);
        assert!(gif.same_animation(&oracle, || false).unwrap(), "disposal {mode}");
        assert!(gif.same_timeline(&oracle, || false).unwrap(), "disposal {mode}");
        assert_eq!(
            gif.timeline_digest(|| false).unwrap(),
            oracle.timeline_digest(|| false).unwrap()
        );
        animations.push(gif);
    }
    for left in 0..3 {
        for right in left + 1..3 {
            assert!(
                !animations[left]
                    .same_timeline(&animations[right], || false)
                    .unwrap(),
                "disposals {} and {}",
                left + 1,
                right + 1
            );
        }
    }
    drop(animations);
    assert_eq!(budget.used(), 0);
}

#[test]
fn every_apng_truncation_and_crc_damage_refuses_without_leaking_then_retries() {
    use rrrah_dedup::animated::AnimationError;
    let original =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/disposal/disposal-3.apng");
    let bytes = std::fs::read(original).unwrap();
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("retry.apng");
    let request = DecodeRequest::new(&path);
    let budget = MemoryBudget::new(4 * 1024 * 1024);
    for length in 0..bytes.len() {
        std::fs::write(&path, &bytes[..length]).unwrap();
        let result = decode_animation(&request, AnimationKind::Apng, LIMITS, &budget, || false);
        assert!(
            matches!(result, Err(AnimationError::Decode(_))),
            "prefix {length}: {result:?}"
        );
        assert_eq!(budget.used(), 0, "prefix {length}");
    }
    let mut offset = 8;
    while offset < bytes.len() {
        let length = u32::from_be_bytes(bytes[offset..offset + 4].try_into().unwrap()) as usize;
        let end = offset + length + 12;
        let mut damaged = bytes.clone();
        damaged[end - 1] ^= 1;
        std::fs::write(&path, damaged).unwrap();
        let result = decode_animation(&request, AnimationKind::Apng, LIMITS, &budget, || false);
        assert!(
            matches!(result, Err(AnimationError::Decode(_))),
            "chunk at {offset}: {result:?}"
        );
        assert_eq!(budget.used(), 0, "chunk at {offset}");
        offset = end;
    }
    let mut trailing = bytes.clone();
    trailing.push(0);
    std::fs::write(&path, trailing).unwrap();
    assert!(decode_animation(&request, AnimationKind::Apng, LIMITS, &budget, || false).is_err());
    assert_eq!(budget.used(), 0);
    std::fs::write(&path, bytes).unwrap();
    let recovered = decode_animation(&request, AnimationKind::Apng, LIMITS, &budget, || false).unwrap();
    assert_eq!(recovered.frame_count(), 4);
    drop(recovered);
    assert_eq!(budget.used(), 0);
}

#[test]
fn every_gif_and_webp_truncation_refuses_releases_memory_and_recovers() {
    use rrrah_dedup::animated::AnimationError;
    for (name, kind) in [
        ("gif-animation-disposal-3.gif", AnimationKind::Gif),
        ("webp-animation-0.webp", AnimationKind::Webp),
    ] {
        let bytes = std::fs::read(root().join(name)).unwrap();
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join(name);
        let request = DecodeRequest::new(&path);
        let budget = MemoryBudget::new(4 * 1024 * 1024);
        for length in 0..bytes.len() {
            std::fs::write(&path, &bytes[..length]).unwrap();
            let result = decode_animation(&request, kind, LIMITS, &budget, || false);
            assert!(
                matches!(result, Err(AnimationError::Decode(_))),
                "{name} prefix {length}: {result:?}"
            );
            assert_eq!(budget.used(), 0, "{name} prefix {length}");
        }
        let mut trailing = bytes.clone();
        trailing.push(0);
        std::fs::write(&path, trailing).unwrap();
        assert!(decode_animation(&request, kind, LIMITS, &budget, || false).is_err());
        assert_eq!(budget.used(), 0);
        std::fs::write(&path, bytes).unwrap();
        let recovered = decode_animation(&request, kind, LIMITS, &budget, || false).unwrap();
        assert!(recovered.frame_count() > 1);
        drop(recovered);
        assert_eq!(budget.used(), 0);
    }
}

#[test]
fn every_observed_animation_cancellation_checkpoint_releases_and_allows_retry() {
    use std::cell::Cell;
    for (name, kind) in [
        ("gif-animation-disposal-3.gif", AnimationKind::Gif),
        ("apng-compose-0-0.apng", AnimationKind::Apng),
        ("webp-animation-0.webp", AnimationKind::Webp),
    ] {
        let request = DecodeRequest::new(root().join(name));
        let budget = MemoryBudget::new(4 * 1024 * 1024);
        let observations = Cell::new(0usize);
        let baseline = decode_animation(&request, kind, LIMITS, &budget, || {
            observations.set(observations.get() + 1);
            false
        })
        .unwrap();
        let expected_key = baseline.timeline_digest(|| false).unwrap();
        let checkpoint_count = observations.get();
        assert!(checkpoint_count > 1, "{name}");
        drop(baseline);
        assert_eq!(budget.used(), 0);
        for checkpoint in 1..=checkpoint_count {
            let calls = Cell::new(0usize);
            let latched = Cell::new(false);
            let result = decode_animation(&request, kind, LIMITS, &budget, || {
                calls.set(calls.get() + 1);
                if calls.get() >= checkpoint {
                    latched.set(true);
                }
                latched.get()
            });
            assert!(latched.get(), "{name} checkpoint {checkpoint} not reached");
            assert!(result.is_err(), "{name} checkpoint {checkpoint} admitted output");
            assert_eq!(budget.used(), 0, "{name} checkpoint {checkpoint}");
        }
        let retry = decode_animation(&request, kind, LIMITS, &budget, || false).unwrap();
        assert_eq!(retry.timeline_digest(|| false).unwrap(), expected_key, "{name}");
        drop(retry);
        assert_eq!(budget.used(), 0);
    }
}
