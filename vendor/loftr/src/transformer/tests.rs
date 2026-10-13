// Modified by rrrah (2026-10-10): cooperative inference cancellation.
use super::*;
use crate::loftr_config::{AttentionType, LoftrConfig};
use tch::{Device, Kind, nn};

#[test]
fn coarse_and_fine_transformer_cancellation_between_each_evaluation() -> Result<(), LoftrError> {
    use std::cell::Cell;
    tch::set_num_threads(2);
    let _inference = tch::no_grad_guard();
    let config = LoftrConfig::outdoor();
    let fine = crate::loftr_config::TransformerConfig {
        d_model: config.fine.d_model,
        d_ffn: config.fine.d_ffn,
        nhead: config.fine.nhead,
        layers: config.fine.layers.clone(),
        attention: config.fine.attention,
        temp_bug_fix: false,
    };
    for config in [&config.coarse, &fine] {
        let vs = nn::VarStore::new(Device::Cpu);
        let transformer = LocalFeatureTransformer::new(&vs.root(), config)?;
        let left = Tensor::randn([2, 12, config.d_model], (Kind::Float, Device::Cpu));
        let right = Tensor::randn([2, 15, config.d_model], (Kind::Float, Device::Cpu));
        let calls = Cell::new(0);
        let expected = transformer.forward_cancellable(&left, &right, None, None, &|| {
            calls.set(calls.get() + 1);
            false
        })?;
        let total = calls.get();
        assert_eq!(total, 2 + 2 * config.layers.len());
        for stop in 1..=total {
            let calls = Cell::new(0);
            let result = transformer.forward_cancellable(&left, &right, None, None, &|| {
                calls.set(calls.get() + 1);
                calls.get() == stop
            });
            assert!(matches!(result, Err(LoftrError::Cancelled)));
            assert_eq!(calls.get(), stop);
        }
        let retry = transformer.forward(&left, &right, None, None)?;
        assert!(expected.0.equal(&retry.0));
        assert!(expected.1.equal(&retry.1));
    }
    Ok(())
}

#[test]
fn encoder_layer_preserves_shape() -> Result<(), LoftrError> {
    let vs = nn::VarStore::new(Device::Cpu);
    let layer = LoFTREncoderLayer::new(&vs.root(), 256, 8, AttentionType::Linear)?;
    let x = Tensor::randn([2, 12, 256], (Kind::Float, Device::Cpu));
    let source = Tensor::randn([2, 15, 256], (Kind::Float, Device::Cpu));
    let out = layer.forward(&x, &source, None, None)?;
    assert_eq!(out.size(), vec![2, 12, 256]);
    Ok(())
}

#[test]
fn transformer_preserves_feature_shapes() -> Result<(), LoftrError> {
    let vs = nn::VarStore::new(Device::Cpu);
    let config = LoftrConfig::outdoor().coarse;
    let transformer = LocalFeatureTransformer::new(&vs.root(), &config)?;
    let feat0 = Tensor::randn([1, 24, 256], (Kind::Float, Device::Cpu));
    let feat1 = Tensor::randn([1, 30, 256], (Kind::Float, Device::Cpu));
    let (out0, out1) = transformer.forward(&feat0, &feat1, None, None)?;
    assert_eq!(out0.size(), vec![1, 24, 256]);
    assert_eq!(out1.size(), vec![1, 30, 256]);
    Ok(())
}

#[test]
fn transformer_rejects_wrong_feature_dim() {
    let vs = nn::VarStore::new(Device::Cpu);
    let config = LoftrConfig::outdoor().fine;
    let transformer_config = TransformerConfig {
        d_model: config.d_model,
        d_ffn: config.d_ffn,
        nhead: config.nhead,
        layers: config.layers,
        attention: config.attention,
        temp_bug_fix: false,
    };
    let transformer = match LocalFeatureTransformer::new(&vs.root(), &transformer_config) {
        Ok(transformer) => transformer,
        Err(err) => panic!("transformer construction failed unexpectedly: {err}"),
    };
    let feat0 = Tensor::randn([1, 12, 64], (Kind::Float, Device::Cpu));
    let feat1 = Tensor::randn([1, 12, 128], (Kind::Float, Device::Cpu));
    match transformer.forward(&feat0, &feat1, None, None) {
        Ok(_) => panic!("wrong feature dim should fail"),
        Err(err) => assert!(format!("{err}").contains("expected feature dim")),
    }
}

#[test]
fn transformer_accepts_masks_and_preserves_feature_shapes() -> Result<(), LoftrError> {
    let vs = nn::VarStore::new(Device::Cpu);
    let config = LoftrConfig::outdoor().coarse;
    let transformer = LocalFeatureTransformer::new(&vs.root(), &config)?;
    let feat0 = Tensor::randn([1, 8, 256], (Kind::Float, Device::Cpu));
    let feat1 = Tensor::randn([1, 8, 256], (Kind::Float, Device::Cpu));
    let mask0 = Tensor::from_slice(&[1_i64, 1, 1, 1, 0, 0, 0, 0]).view([1, 8]);
    let mask1 = Tensor::from_slice(&[1_i64, 1, 1, 1, 1, 1, 0, 0]).view([1, 8]);
    let (out0, out1) = transformer.forward(&feat0, &feat1, Some(&mask0), Some(&mask1))?;
    assert_eq!(out0.size(), vec![1, 8, 256]);
    assert_eq!(out1.size(), vec![1, 8, 256]);
    Ok(())
}
