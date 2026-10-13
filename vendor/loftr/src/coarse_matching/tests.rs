// Modified by rrrah (2026-10-10): inference buffer reuse and autograd regression.
use super::*;
use crate::loftr_config::LoftrConfig;
use tch::Device;

#[test]
fn dual_softmax_identity_features_match_diagonal() -> Result<(), LoftrError> {
    let config = LoftrConfig::outdoor();
    let matcher = CoarseMatching::new(&MatchCoarseConfig {
        border_rm: 0,
        ..config.match_coarse
    });
    let feat_c0 = Tensor::eye(4, (Kind::Float, Device::Cpu)).view([1, 4, 4]);
    let feat_c1 = Tensor::eye(4, (Kind::Float, Device::Cpu)).view([1, 4, 4]);
    let out = matcher.forward(
        &feat_c0,
        &feat_c1,
        &CoarseMatchingData {
            hw0_i: (8, 8),
            hw1_i: (8, 8),
            hw0_c: (2, 2),
            hw1_c: (2, 2),
            scale0: None,
            scale1: None,
        },
        None,
        None,
    )?;
    assert_eq!(out.b_ids.size(), vec![4]);
    assert_eq!(out.i_ids.size(), vec![4]);
    assert_eq!(out.j_ids.size(), vec![4]);
    assert_eq!(out.mconf.size(), vec![4]);
    assert!(out.mconf.min().double_value(&[]) > 0.2);
    assert_eq!(out.mkpts0_c.size(), vec![4, 2]);
    assert_eq!(out.mkpts1_c.size(), vec![4, 2]);
    Ok(())
}

#[test]
fn border_mask_removes_all_matches_on_tiny_grid() -> Result<(), LoftrError> {
    let config = LoftrConfig::outdoor();
    let matcher = CoarseMatching::new(&config.match_coarse);
    let feat_c0 = Tensor::eye(4, (Kind::Float, Device::Cpu)).view([1, 4, 4]);
    let feat_c1 = Tensor::eye(4, (Kind::Float, Device::Cpu)).view([1, 4, 4]);
    let out = matcher.forward(
        &feat_c0,
        &feat_c1,
        &CoarseMatchingData {
            hw0_i: (8, 8),
            hw1_i: (8, 8),
            hw0_c: (2, 2),
            hw1_c: (2, 2),
            scale0: None,
            scale1: None,
        },
        None,
        None,
    )?;
    assert_eq!(out.b_ids.size(), vec![0]);
    assert_eq!(out.mconf.size(), vec![0]);
    Ok(())
}

#[test]
fn masks_remove_masked_tokens_from_matches() -> Result<(), LoftrError> {
    let config = LoftrConfig::outdoor();
    let matcher = CoarseMatching::new(&MatchCoarseConfig {
        border_rm: 0,
        ..config.match_coarse
    });
    let feat_c0 = Tensor::eye(4, (Kind::Float, Device::Cpu)).view([1, 4, 4]);
    let feat_c1 = Tensor::eye(4, (Kind::Float, Device::Cpu)).view([1, 4, 4]);
    let mask_c0 = Tensor::from_slice(&[1_i64, 1, 0, 0]).view([1, 4]);
    let mask_c1 = Tensor::from_slice(&[1_i64, 1, 0, 0]).view([1, 4]);
    let out = matcher.forward(
        &feat_c0,
        &feat_c1,
        &CoarseMatchingData {
            hw0_i: (8, 8),
            hw1_i: (8, 8),
            hw0_c: (2, 2),
            hw1_c: (2, 2),
            scale0: None,
            scale1: None,
        },
        Some(&mask_c0),
        Some(&mask_c1),
    )?;
    assert_eq!(out.b_ids.size(), vec![2]);
    assert_eq!(out.i_ids.int64_value(&[0]), 0);
    assert_eq!(out.i_ids.int64_value(&[1]), 1);
    assert_eq!(out.j_ids.int64_value(&[0]), 0);
    assert_eq!(out.j_ids.int64_value(&[1]), 1);
    Ok(())
}

#[test]
fn confidence_buffer_reuse_preserves_forward_and_autograd() -> Result<(), LoftrError> {
    tch::set_num_threads(2);
    let values = Tensor::randn([2, 7, 11], (Kind::Float, Device::Cpu));
    let saved = &values + 0.0;
    let expected = values.softmax(1, Kind::Float) * values.softmax(2, Kind::Float);
    let actual = dual_softmax_confidence(&values)?;
    assert!(actual.equal(&expected));
    assert!(values.equal(&saved), "input logits must remain unchanged");
    assert!(!actual.requires_grad());
    let left = (&values + 0.0).set_requires_grad(true);
    let right = (&values + 0.0).set_requires_grad(true);
    let actual = dual_softmax_confidence(&left)?;
    let expected = right.softmax(1, Kind::Float) * right.softmax(2, Kind::Float);
    assert!(actual.equal(&expected));
    actual.square().sum(Kind::Float).backward();
    expected.square().sum(Kind::Float).backward();
    assert!(
        left.grad().equal(&right.grad()),
        "autograd must retain untouched softmax outputs"
    );
    Ok(())
}

#[test]
fn blocked_confidence_matches_dense_across_block_boundaries() -> Result<(), LoftrError> {
    tch::set_num_threads(2);
    for rows in [0, 1, 127, 128, 129, 257] {
        for cols in [1, 7, 193] {
            let values = Tensor::randn([2, rows, cols], (Kind::Float, Device::Cpu));
            let saved = &values + 0.0;
            let expected = values.softmax(1, Kind::Float) * values.softmax(2, Kind::Float);
            let actual = dual_softmax_confidence(&values)?;
            assert!(actual.equal(&expected), "rows={rows} cols={cols}");
            assert!(values.equal(&saved));
        }
    }
    Ok(())
}

fn assert_same_coarse_output(a: &CoarseMatchingOutput, b: &CoarseMatchingOutput) {
    for (x, y) in [
        (&a.conf_matrix, &b.conf_matrix),
        (&a.b_ids, &b.b_ids),
        (&a.i_ids, &b.i_ids),
        (&a.j_ids, &b.j_ids),
        (&a.m_bids, &b.m_bids),
        (&a.mkpts0_c, &b.mkpts0_c),
        (&a.mkpts1_c, &b.mkpts1_c),
        (&a.mconf, &b.mconf),
    ] {
        assert!(x.equal(y));
    }
}

#[test]
fn coarse_cancellation_at_every_checkpoint_discards_output_and_retries() -> Result<(), LoftrError> {
    use std::cell::Cell;
    tch::set_num_threads(2);
    let mut config = LoftrConfig::outdoor().match_coarse;
    config.thr = 0.0;
    config.border_rm = 0;
    let module = CoarseMatching::new(&config);
    let left = Tensor::randn([2, 257, 256], (Kind::Float, Device::Cpu));
    let right = Tensor::randn([2, 11, 256], (Kind::Float, Device::Cpu));
    let mask0 = Tensor::ones([2, 257], (Kind::Bool, Device::Cpu));
    let mask1 = Tensor::ones([2, 11], (Kind::Bool, Device::Cpu));
    let data = CoarseMatchingData {
        hw0_i: (8, 2056),
        hw1_i: (8, 88),
        hw0_c: (1, 257),
        hw1_c: (1, 11),
        scale0: None,
        scale1: None,
    };
    for masked in [false, true] {
        let m0 = masked.then_some(&mask0);
        let m1 = masked.then_some(&mask1);
        let reference = module.forward(&left, &right, &data, m0, m1)?;
        let calls = Cell::new(0);
        let output = module.forward_cancellable(&left, &right, &data, m0, m1, &|| {
            calls.set(calls.get() + 1);
            false
        })?;
        assert_same_coarse_output(&reference, &output);
        assert!(calls.get() >= 12);
        for stop in 1..=calls.get() {
            let seen = Cell::new(0);
            let result = module.forward_cancellable(&left, &right, &data, m0, m1, &|| {
                seen.set(seen.get() + 1);
                seen.get() == stop
            });
            assert!(matches!(result, Err(LoftrError::Cancelled)));
            assert_eq!(seen.get(), stop, "no evaluations after cancellation");
            let retry = module.forward(&left, &right, &data, m0, m1)?;
            assert_same_coarse_output(&reference, &retry);
        }
    }
    Ok(())
}

#[test]
fn softmax_cancellation_covers_blocks_and_autograd_without_input_mutation() -> Result<(), LoftrError>
{
    use std::cell::Cell;
    tch::set_num_threads(2);
    for grad in [false, true] {
        let values =
            Tensor::randn([2, 257, 11], (Kind::Float, Device::Cpu)).set_requires_grad(grad);
        let saved = &values + 0.0;
        let expected = values.softmax(1, Kind::Float) * values.softmax(2, Kind::Float);
        let calls = Cell::new(0);
        let actual = dual_softmax_confidence_cancellable(&values, &|| {
            calls.set(calls.get() + 1);
            false
        })?;
        assert!(actual.equal(&expected));
        assert!(calls.get() >= if grad { 3 } else { 6 });
        for stop in 1..=calls.get() {
            let seen = Cell::new(0);
            let result = dual_softmax_confidence_cancellable(&values, &|| {
                seen.set(seen.get() + 1);
                seen.get() == stop
            });
            assert!(matches!(result, Err(LoftrError::Cancelled)));
            assert_eq!(seen.get(), stop);
            assert!(values.equal(&saved));
            assert!(dual_softmax_confidence(&values)?.equal(&expected));
        }
    }
    Ok(())
}

#[test]
fn blocked_selection_matches_dense_order_ties_borders_and_thresholds() -> Result<(), LoftrError> {
    tch::manual_seed(418);
    for (b, h, w, h1, w1) in [(1, 1, 1, 1, 1), (2, 10, 13, 5, 7), (2, 20, 13, 5, 7)] {
        let data = CoarseMatchingData {
            hw0_i: (h, w),
            hw1_i: (h1, w1),
            hw0_c: (h, w),
            hw1_c: (h1, w1),
            scale0: None,
            scale1: None,
        };
        for confidence in [
            Tensor::rand([b, h * w, h1 * w1], (Kind::Float, Device::Cpu)),
            Tensor::ones([b, h * w, h1 * w1], (Kind::Float, Device::Cpu)),
            Tensor::zeros([b, h * w, h1 * w1], (Kind::Float, Device::Cpu)),
        ] {
            for border in [-1, 0, 1, 8] {
                for threshold in [-0.1, 0., 0.5, 1.] {
                    let dense = confidence_threshold_mask(&confidence, &data, threshold, border)
                        .logical_and(&confidence.eq_tensor(&confidence.max_dim(2, true).0))
                        .logical_and(&confidence.eq_tensor(&confidence.max_dim(1, true).0))
                        .nonzero();
                    let actual = coarse_match_indices_blocked(
                        &confidence,
                        &data,
                        threshold,
                        border,
                        &|| false,
                    )?;
                    assert!(
                        actual.equal(&dense),
                        "b{b}, rows{}, border{border}, threshold{threshold}",
                        h * w
                    );
                }
            }
        }
    }
    Ok(())
}

#[test]
fn blocked_selection_cancels_between_batches_and_blocks_without_mutation() -> Result<(), LoftrError>
{
    use std::cell::Cell;
    let confidence = Tensor::ones([2, 260, 35], (Kind::Float, Device::Cpu));
    let original = confidence.copy();
    let data = CoarseMatchingData {
        hw0_i: (20, 13),
        hw1_i: (5, 7),
        hw0_c: (20, 13),
        hw1_c: (5, 7),
        scale0: None,
        scale1: None,
    };
    let count = Cell::new(0);
    let expected = coarse_match_indices_blocked(&confidence, &data, 0., 0, &|| {
        count.set(count.get() + 1);
        false
    })?;
    for stop in 1..=count.get() {
        let calls = Cell::new(0);
        let result = coarse_match_indices_blocked(&confidence, &data, 0., 0, &|| {
            calls.set(calls.get() + 1);
            calls.get() == stop
        });
        assert!(matches!(result, Err(LoftrError::Cancelled)));
        assert_eq!(calls.get(), stop);
        assert!(confidence.equal(&original));
    }
    assert!(coarse_match_indices_blocked(&confidence, &data, 0., 0, &|| false)?.equal(&expected));
    Ok(())
}
