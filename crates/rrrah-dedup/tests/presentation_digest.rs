#![cfg(feature = "decode")]
use rrrah_core::MemoryBudget;
use rrrah_decode::DecodeRequest;
use rrrah_dedup::{
    animated::{AnimationBudget, AnimationKind, decode_animation},
    pages::{PageKind, decode_pages},
};
const LIMITS: AnimationBudget = AnimationBudget {
    max_frames: 10,
    max_pixels: 100,
    max_file_bytes: 100_000,
};

fn is_cancelled(error: &rrrah_dedup::animated::AnimationError) -> bool {
    use rrrah_dedup::{
        animated::AnimationError, decode::FileError, pixels::PixelError, raster::AdapterError,
        sequence::SequenceError,
    };
    matches!(
        error,
        AnimationError::Sequence(SequenceError::Pixels(PixelError::Cancelled))
            | AnimationError::Decode(FileError::Normalize(AdapterError::Pixels(PixelError::Cancelled)))
    )
}

#[test]
fn timeline_keys_preserve_cross_encoding_splits_zero_prefix_and_labelled_negatives() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/timeline");
    let budget = MemoryBudget::new(4 * 1024 * 1024);
    let animations = [
        ("base.gif", AnimationKind::Gif, 0),
        ("base.webp", AnimationKind::Webp, 0),
        ("split.apng", AnimationKind::Apng, 0),
        ("zero-prefix.apng", AnimationKind::Apng, 0),
        ("changed-time.apng", AnimationKind::Apng, 1),
        ("changed-pixels.apng", AnimationKind::Apng, 2),
    ]
    .into_iter()
    .map(|(name, kind, label)| {
        (
            decode_animation(
                &DecodeRequest::new(root.join(name)),
                kind,
                LIMITS,
                &budget,
                || false,
            )
            .unwrap(),
            label,
        )
    })
    .collect::<Vec<_>>();
    let keys = animations
        .iter()
        .map(|(animation, _)| animation.timeline_digest(|| false).unwrap())
        .collect::<Vec<_>>();
    for a in 0..animations.len() {
        for b in a + 1..animations.len() {
            let labelled_equal = animations[a].1 == animations[b].1;
            assert_eq!(keys[a] == keys[b], labelled_equal);
            assert_eq!(
                animations[a].0.same_timeline(&animations[b].0, || false).unwrap(),
                labelled_equal
            );
        }
    }
    let calls = std::cell::Cell::new(0);
    let key = animations[2]
        .0
        .timeline_digest(|| {
            calls.set(calls.get() + 1);
            false
        })
        .unwrap();
    for checkpoint in 1..=calls.get() {
        let count = std::cell::Cell::new(0);
        assert!(is_cancelled(
            &animations[2]
                .0
                .timeline_digest(|| {
                    count.set(count.get() + 1);
                    count.get() == checkpoint
                })
                .unwrap_err()
        ));
    }
    assert_eq!(animations[2].0.timeline_digest(|| false).unwrap(), key);
    drop(animations);
    assert_eq!(budget.used(), 0);
}

#[test]
fn page_keys_cover_every_page_across_compression_and_byte_order() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/tiff");
    let budget = MemoryBudget::new(4 * 1024 * 1024);
    let pages = [
        "classic-little-none-same.tif",
        "bigtiff-big-lzw-same.tif",
        "bigtiff-big-lzw-changed.tif",
    ]
    .map(|name| {
        decode_pages(
            &DecodeRequest::new(root.join(name)),
            PageKind::Tiff,
            LIMITS,
            &budget,
            || false,
        )
        .unwrap()
    });
    let keys = pages.each_ref().map(|value| value.page_digest(|| false).unwrap());
    assert_eq!(keys[0], keys[1]);
    assert_ne!(keys[0], keys[2]);
    assert!(pages[0].same_pages(&pages[1], || false).unwrap());
    assert!(!pages[0].same_pages(&pages[2], || false).unwrap());
    let calls = std::cell::Cell::new(0);
    let key = pages[1]
        .page_digest(|| {
            calls.set(calls.get() + 1);
            false
        })
        .unwrap();
    for checkpoint in 1..=calls.get() {
        let count = std::cell::Cell::new(0);
        assert!(is_cancelled(
            &pages[1]
                .page_digest(|| {
                    count.set(count.get() + 1);
                    count.get() == checkpoint
                })
                .unwrap_err()
        ));
    }
    assert_eq!(pages[1].page_digest(|| false).unwrap(), key);
    drop(pages);
    assert_eq!(budget.used(), 0);
}
