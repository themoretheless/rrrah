// Modified by rrrah (2026-10-10): strip-based 3x3 convolution and lazy debug statistics.
use super::*;

#[test]
fn cancellation_at_every_inference_checkpoint_discards_output_and_allows_retry()
-> Result<(), LoftrError> {
    use std::cell::Cell;
    tch::set_num_threads(2);
    let _inference = tch::no_grad_guard();
    let mut model = LoFTRModel::new(Device::Cpu, LoftrConfig::outdoor())?;
    // Unequal shapes exercise both independent backbone evaluations; 80 rows
    // produce multiple strips in the fine-resolution convolution maps.
    let left = Tensor::rand([1, 1, 80, 80], (Kind::Float, Device::Cpu));
    let right = Tensor::rand([1, 1, 64, 80], (Kind::Float, Device::Cpu));
    let calls = Cell::new(0);
    let expected = model.forward_cancellable(&left, &right, &|| {
        calls.set(calls.get() + 1);
        false
    })?;
    let total = calls.get();
    assert!(
        total > 60,
        "backbone and transformer checkpoints must be exercised"
    );
    for stop in 1..=total {
        let calls = Cell::new(0);
        let result = model.forward_cancellable(&left, &right, &|| {
            calls.set(calls.get() + 1);
            calls.get() == stop
        });
        assert!(
            matches!(result, Err(LoftrError::Cancelled)),
            "checkpoint {stop}/{total}"
        );
        assert_eq!(calls.get(), stop, "no work checkpoints after cancellation");
    }
    let retry = model.forward(&left, &right)?;
    assert!(expected.keypoints0.equal(&retry.keypoints0));
    assert!(expected.keypoints1.equal(&retry.keypoints1));
    assert!(expected.confidence.equal(&retry.confidence));
    assert!(expected.batch_indexes.equal(&retry.batch_indexes));
    // Entry cancellation takes precedence over input normalization.
    let invalid = Tensor::zeros([0], (Kind::Float, Device::Cpu));
    assert!(matches!(
        model.forward_cancellable(&invalid, &invalid, &|| true),
        Err(LoftrError::Cancelled)
    ));
    Ok(())
}

#[test]
fn model_variable_names_match_kornia_prefixes() -> Result<(), LoftrError> {
    let model = LoFTRModel::new(Device::Cpu, LoftrConfig::outdoor())?;
    let vars = model.var_store().variables();
    for name in [
        "backbone.conv1.weight",
        "backbone.layer1.0.conv1.weight",
        "fine_preprocess.down_proj.weight",
        "loftr_coarse.layers.0.q_proj.weight",
        "loftr_fine.layers.0.q_proj.weight",
    ] {
        assert!(vars.contains_key(name), "missing variable `{name}`");
    }
    Ok(())
}

#[test]
fn model_forward_smoke_returns_consistent_output_shapes() -> Result<(), LoftrError> {
    let mut model = LoFTRModel::new(Device::Cpu, LoftrConfig::outdoor())?;
    let image0 = Tensor::rand([1, 1, 128, 128], (Kind::Float, Device::Cpu));
    let image1 = Tensor::rand([1, 1, 128, 128], (Kind::Float, Device::Cpu));
    let out = model.forward(&image0, &image1)?;
    assert_eq!(out.keypoints0.size().len(), 2);
    assert_eq!(out.keypoints1.size().len(), 2);
    assert_eq!(out.confidence.size().len(), 1);
    assert_eq!(out.batch_indexes.size().len(), 1);
    assert_eq!(out.keypoints0.size()[0], out.keypoints1.size()[0]);
    assert_eq!(out.keypoints0.size()[0], out.confidence.size()[0]);
    assert_eq!(out.keypoints0.size()[0], out.batch_indexes.size()[0]);
    assert_eq!(out.keypoints0.size()[1], 2);
    assert_eq!(out.keypoints1.size()[1], 2);
    Ok(())
}

#[test]
fn regression_no_eager_debug_preserves_outputs_and_public_debug() -> Result<(), LoftrError> {
    tch::set_num_threads(2);
    tch::manual_seed(173);
    let mut model = LoFTRModel::new(Device::Cpu, LoftrConfig::outdoor())?;
    let image0 = Tensor::rand([1, 1, 64, 64], (Kind::Float, Device::Cpu));
    let image1 = Tensor::rand([1, 1, 64, 64], (Kind::Float, Device::Cpu));
    let _inference = tch::no_grad_guard();
    let plain = model.forward_impl(&image0, &image1, false, &|| false)?;
    let debug = model.forward_impl(&image0, &image1, true, &|| false)?;
    assert!(plain.debug.is_none());
    assert!(debug.debug.is_some());
    assert!(plain.matches.keypoints0.equal(&debug.matches.keypoints0));
    assert!(plain.matches.keypoints1.equal(&debug.matches.keypoints1));
    assert!(plain.matches.confidence.equal(&debug.matches.confidence));
    assert!(
        plain
            .matches
            .batch_indexes
            .equal(&debug.matches.batch_indexes)
    );
    let public = model.forward_debug(&image0, &image1)?;
    assert_eq!(public.image0.shape, vec![1, 1, 64, 64]);
    Ok(())
}

#[test]
fn sequential_backbone_matches_stacked_features_exactly() -> Result<(), LoftrError> {
    tch::set_num_threads(2);
    tch::manual_seed(173);
    let _inference = tch::no_grad_guard();
    let model = LoFTRModel::new(Device::Cpu, LoftrConfig::outdoor())?;
    for (batch, height, width) in [(1, 64, 64), (1, 64, 80), (1, 96, 128), (2, 64, 80)] {
        let left = Tensor::rand([batch, 1, height, width], (Kind::Float, Device::Cpu));
        let right = Tensor::rand([batch, 1, height, width], (Kind::Float, Device::Cpu));
        let joined = Tensor::cat(&[left.shallow_clone(), right.shallow_clone()], 0);
        let (oracle_coarse, oracle_fine) = model.backbone.forward_cancellable(&joined, false, &|| false)?;
        let oc = oracle_coarse.split(batch, 0); let of = oracle_fine.split(batch, 0);
        let stacked = BackboneFeatures { coarse0: oc[0].shallow_clone(), coarse1: oc[1].shallow_clone(), fine0: of[0].shallow_clone(), fine1: of[1].shallow_clone() };
        let sequential = model.backbone_features(&left, &right, &|| false)?;
        let BackboneFeatures {coarse0, coarse1, fine0, fine1} = sequential;
        for (label, actual, expected) in [
            ("coarse0", coarse0, stacked.coarse0),
            ("coarse1", coarse1, stacked.coarse1),
            ("fine0", fine0, stacked.fine0),
            ("fine1", fine1, stacked.fine1),
        ] {
            let difference = (&actual - &expected).abs().max().double_value(&[]);
            eprintln!("sequential_backbone batch={batch} height={height} width={width} output={label} equal={} max_abs={difference}", actual.equal(&expected));
            assert!(actual.equal(&expected), "batch={batch}, {height}x{width}, {label}, max_abs={difference}");
        }
    }
    Ok(())
}
