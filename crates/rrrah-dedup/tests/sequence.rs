use rrrah_dedup::{
    linear::LinearRgbaView,
    sequence::{Duration, Frame, Playback, Sequence, SequenceError},
};

fn frame(samples: &[f32], duration: Option<Duration>) -> Frame<'_> {
    Frame {
        pixels: LinearRgbaView::new(1, 1, samples, 1, || false).unwrap(),
        duration,
    }
}

#[test]
fn first_frame_match_does_not_hide_changed_last_frame() {
    let a = [0.0, 0.0, 0.0, 1.0];
    let b = [1.0, 1.0, 1.0, 1.0];
    let left = Sequence::new(
        2,
        Playback::Pages,
        [(0, frame(&a, None)), (1, frame(&a, None))],
        2,
        || false,
    )
    .unwrap();
    let right = Sequence::new(
        2,
        Playback::Pages,
        [(0, frame(&a, None)), (1, frame(&b, None))],
        2,
        || false,
    )
    .unwrap();
    assert!(!left.same_sequence(&right, || false).unwrap());
}

#[test]
fn complete_coverage_is_required() {
    let a = [0.0; 4];
    for ordinals in [vec![0], vec![0, 0], vec![1, 0], vec![0, 1, 2]] {
        assert_eq!(
            Sequence::new(
                2,
                Playback::Pages,
                ordinals.into_iter().map(|i| (i, frame(&a, None))),
                2,
                || false
            )
            .unwrap_err(),
            SequenceError::Coverage
        );
    }
    assert_eq!(
        Sequence::new(2, Playback::Pages, [], 1, || false).unwrap_err(),
        SequenceError::Coverage
    );
}

#[test]
fn admission_cancel_at_every_checkpoint_returns_no_partial_sequence_and_retry_succeeds() {
    let samples = [0.25, 0.5, 0.75, 1.0];
    for checkpoint in 1..=5 {
        let calls = std::cell::Cell::new(0);
        assert_eq!(
            Sequence::new(
                4,
                Playback::Pages,
                (0..4).map(|i| (i, frame(&samples, None))),
                4,
                || {
                    calls.set(calls.get() + 1);
                    calls.get() == checkpoint
                },
            )
            .unwrap_err(),
            SequenceError::Pixels(rrrah_dedup::pixels::PixelError::Cancelled)
        );
    }
    let build = || {
        Sequence::new(
            4,
            Playback::Pages,
            (0..4).map(|i| (i, frame(&samples, None))),
            4,
            || false,
        )
        .unwrap()
    };
    assert!(build().same_sequence(&build(), || false).unwrap());
}

#[test]
fn animation_requires_timing_and_compares_repetitions() {
    let a = [0.0; 4];
    let playback = Playback::Animation { repetitions: None };
    assert_eq!(
        Sequence::new(1, playback, [(0, frame(&a, None))], 1, || false).unwrap_err(),
        SequenceError::Timing
    );
    let make = |duration, playback| {
        Sequence::new(1, playback, [(0, frame(&a, Some(duration)))], 1, || false).unwrap()
    };
    let left = make(Duration::new(1, 10).unwrap(), playback);
    let same = make(Duration::new(100, 1000).unwrap(), playback);
    let slow = make(Duration::new(2, 10).unwrap(), playback);
    let once = make(
        Duration::new(1, 10).unwrap(),
        Playback::Animation { repetitions: Some(1) },
    );
    assert!(left.same_sequence(&same, || false).unwrap());
    assert!(!left.same_sequence(&slow, || false).unwrap());
    assert!(!left.same_sequence(&once, || false).unwrap());
    assert!(left.same_sequence(&same, || true).is_err());
    assert_eq!(Duration::new(1, 0), Err(SequenceError::Timing));
    assert_eq!(Duration::new(0, 100), Duration::new(0, 1));
}

#[test]
fn equivalent_timelines_ignore_frame_splits_but_keep_time_and_pixels() {
    fn make(items: Vec<(&[f32], u64, u64)>) -> Sequence<'_> {
        let count = items.len();
        Sequence::new(
            count,
            Playback::Animation { repetitions: None },
            items
                .into_iter()
                .enumerate()
                .map(|(i, (p, n, d))| (i, frame(p, Some(Duration::new(n, d).unwrap())))),
            count,
            || false,
        )
        .unwrap()
    }
    let a = [0.0, 0.0, 0.0, 1.0];
    let b = [1.0, 0.0, 0.0, 1.0];
    let left = make(vec![(&a, 1, 3), (&b, 1, 2)]);
    let split = make(vec![(&b, 0, 1), (&a, 1, 6), (&a, 1, 6), (&b, 1, 4), (&b, 1, 4)]);
    assert!(!left.same_sequence(&split, || false).unwrap());
    assert!(left.same_timeline(&split, || false).unwrap());
    assert!(split.same_timeline(&left, || false).unwrap());
    assert!(
        !left
            .same_timeline(&make(vec![(&a, 1, 3), (&b, 1, 3)]), || false)
            .unwrap()
    );
    assert!(
        !left
            .same_timeline(&make(vec![(&b, 1, 3), (&a, 1, 2)]), || false)
            .unwrap()
    );
    assert!(left.same_timeline(&split, || true).is_err());
}

#[test]
fn rational_extremes_fail_explicitly_without_wrapping() {
    let a = [0.0; 4];
    let make = |duration| {
        Sequence::new(
            1,
            Playback::Animation { repetitions: None },
            [(0, frame(&a, Some(duration)))],
            1,
            || false,
        )
        .unwrap()
    };
    let large = make(Duration::new(u64::MAX, 1).unwrap());
    assert!(large.same_timeline(&large, || false).unwrap());
    let x = make(Duration::new(1, u64::MAX).unwrap());
    let y = make(Duration::new(1, u64::MAX - 1).unwrap());
    assert_eq!(x.same_timeline(&y, || false), Err(SequenceError::Timing));
    assert_eq!(y.same_timeline(&x, || false), Err(SequenceError::Timing));
}

#[test]
fn timeline_comparison_matches_independent_discrete_presentation_oracle() {
    fn make<'a>(items: &[(usize, u64)], samples: &'a [[f32; 4]]) -> Sequence<'a> {
        Sequence::new(
            items.len(),
            Playback::Animation { repetitions: Some(3) },
            items.iter().enumerate().map(|(i, &(color, ticks))| {
                (i, frame(&samples[color], Some(Duration::new(ticks, 6).unwrap())))
            }),
            items.len(),
            || false,
        )
        .unwrap()
    }
    fn oracle(items: &[(usize, u64)]) -> Vec<usize> {
        items
            .iter()
            .flat_map(|&(color, ticks)| std::iter::repeat_n(color, usize::try_from(ticks).unwrap()))
            .collect()
    }
    let samples = [[0., 0., 0., 1.], [1., 0., 0., 1.], [0., 1., 0., 1.]];
    let mut state = 7351_u64;
    let mut random = || {
        state = state.wrapping_mul(6_364_136_223_846_793_005).wrapping_add(1);
        state >> 32
    };
    for _ in 0..300 {
        let items: Vec<_> = (0..8)
            .map(|_| (usize::try_from(random() % 3).unwrap(), random() % 5))
            .collect();
        let split: Vec<_> = items
            .iter()
            .flat_map(|&(color, ticks)| [(color, ticks / 2), (color, ticks - ticks / 2)])
            .collect();
        let mut changed = split.clone();
        let index = usize::try_from(random() % 16).unwrap();
        changed[index].0 = (changed[index].0 + 1) % 3;
        let left = make(&items, &samples);
        for right in [&split, &changed] {
            let expected = oracle(&items) == oracle(right);
            assert_eq!(
                left.same_timeline(&make(right, &samples), || false).unwrap(),
                expected
            );
            assert_eq!(
                make(right, &samples).same_timeline(&left, || false).unwrap(),
                expected
            );
        }
    }
}
