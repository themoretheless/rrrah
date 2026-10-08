use rrrah_dedup::{
    gradient::{GradientDescriptor, GradientFeature, GradientMatchPolicy, match_gradients},
    local::LocalError,
};
use std::cell::Cell;

#[test]
fn empty_gradient_inputs_honor_terminal_cancellation_and_retry() {
    let mut descriptor = [0.; 128];
    descriptor[0] = 1.;
    let feature = GradientFeature {
        position: [12., 34.],
        descriptor: GradientDescriptor(descriptor),
    };
    let policy = GradientMatchPolicy {
        max_comparisons: 0,
        max_squared_distance: 0.5,
        squared_ratio: 0.64,
    };
    for (left, right) in [
        (&[][..], &[][..]),
        (&[feature][..], &[][..]),
        (&[][..], &[feature][..]),
    ] {
        // The last callback occurs after validation, immediately before the empty result.
        let terminal = 2 + left.len() + right.len();
        let calls = Cell::new(0);
        assert!(matches!(
            match_gradients(left, right, policy, || {
                calls.set(calls.get() + 1);
                calls.get() == terminal
            }),
            Err(LocalError::Cancelled)
        ));
        assert_eq!(calls.get(), terminal);
        assert!(match_gradients(left, right, policy, || false).unwrap().is_empty());
    }
}
