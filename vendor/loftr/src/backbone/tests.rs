// Modified by rrrah (2026-10-10): strip-based 3x3 convolution and lazy debug statistics.
use super::*;
use tch::{Device, Kind};

#[test]
fn strip_cancellation_never_returns_partial_convolution() -> Result<(), LoftrError> {
    use std::cell::Cell;
    tch::set_num_threads(2);
    let _inference = tch::no_grad_guard();
    let vs = nn::VarStore::new(Device::Cpu);
    let layer = conv3x3(&vs.root(), 3, 8, 1);
    let input = Tensor::randn([1, 3, 83, 19], (Kind::Float, Device::Cpu));
    let count = Cell::new(0);
    let reference = layer.forward(&input, &|| {
        count.set(count.get() + 1);
        false
    })?;
    let total = count.get();
    assert!(total >= 9);
    for stop in 1..=total {
        let count = Cell::new(0);
        let result = layer.forward(&input, &|| {
            count.set(count.get() + 1);
            count.get() == stop
        });
        assert!(matches!(result, Err(LoftrError::Cancelled)));
        assert_eq!(count.get(), stop);
    }
    assert!(reference.equal(&layer.forward(&input, &|| false)?));
    Ok(())
}

#[test]
fn resnet_fpn_8_2_matches_expected_output_shapes() -> Result<(), LoftrError> {
    let vs = nn::VarStore::new(Device::Cpu);
    let backbone = ResNetFpn8_2::new(&vs.root(), &LoftrConfig::outdoor().resnetfpn)?;
    let input = Tensor::randn([1, 1, 240, 320], (Kind::Float, Device::Cpu));
    let (coarse, fine) = backbone.forward_t(&input, false)?;
    assert_eq!(coarse.size(), vec![1, 256, 30, 40]);
    assert_eq!(fine.size(), vec![1, 128, 120, 160]);
    Ok(())
}

#[test]
fn build_backbone_supports_outdoor_config() -> Result<(), LoftrError> {
    let vs = nn::VarStore::new(Device::Cpu);
    let backbone = build_backbone(&vs.root(), &LoftrConfig::outdoor())?;
    let input = Tensor::randn([1, 1, 120, 160], (Kind::Float, Device::Cpu));
    let (coarse, fine) = backbone.forward_t(&input, false)?;
    assert_eq!(coarse.size(), vec![1, 256, 15, 20]);
    assert_eq!(fine.size(), vec![1, 128, 60, 80]);
    Ok(())
}

#[test]
fn build_backbone_rejects_unsupported_resolution() {
    let vs = nn::VarStore::new(Device::Cpu);
    let mut config = LoftrConfig::outdoor();
    config.resolution = (16, 4);
    match build_backbone(&vs.root(), &config) {
        Ok(_) => panic!("unsupported resolution should fail"),
        Err(err) => assert!(format!("{err}").contains("only (8, 2) is ported")),
    }
}

#[test]
fn tiled_convolution_matches_full_convolution_at_seams_and_borders() -> Result<(), LoftrError> {
    tch::set_num_threads(2);
    let _inference = tch::no_grad_guard();
    let vs = nn::VarStore::new(Device::Cpu);
    for (case, (batch, channels, height, width, stride)) in [
        (1, 3, 77, 81, 1),
        (2, 7, 78, 80, 2),
        (1, 128, 83, 97, 1),
        (1, 128, 137, 96, 2),
        (1, 3, 1, 1, 1),
        (1, 3, 2, 3, 2),
    ]
    .into_iter()
    .enumerate()
    {
        let layer = conv3x3(&(&vs.root() / case.to_string()), channels, 16, stride);
        let input = Tensor::randn([batch, channels, height, width], (Kind::Float, Device::Cpu));
        let expected = input.apply(&layer.conv);
        let actual = layer.forward(&input, &|| false)?;
        assert_eq!(actual.size(), expected.size());
        let difference = (&actual - &expected).abs().max().double_value(&[]);
        assert!(
            actual.allclose(&expected, 1e-5, 1e-5, false),
            "case {case}: max error {difference}"
        );
    }
    Ok(())
}

#[test]
fn strip_padding_matches_global_border_reference_with_bounded_copy() -> Result<(), LoftrError> {
    let _inference = tch::no_grad_guard();
    for (height, width, stride) in [(137, 96, 2), (83, 97, 1), (1, 1, 1), (2, 3, 2)] {
        let input = Tensor::arange(2 * 3 * height * width, (Kind::Float, Device::Cpu))
            .reshape([2, 3, height, width]);
        let global = input.f_constant_pad_nd([1, 1, 1, 1])?;
        let output_height = (height - 1) / stride + 1;
        let mut first = 0;
        while first < output_height {
            let rows = (output_height - first).min(32);
            let expected = global.f_narrow(2, first * stride, (rows - 1) * stride + 3)?;
            let actual = padded_input_strip(&input, first, rows, stride)?;
            assert!(actual.equal(&expected));
            assert_eq!(actual.size(), [2, 3, (rows - 1) * stride + 3, width + 2]);
            assert!(actual.numel() <= (2 * 3 * (31 * stride + 3) * (width + 2)) as usize);
            if height > 80 {
                assert!(actual.numel() < global.numel());
            }
            first += rows;
        }
    }
    Ok(())
}
