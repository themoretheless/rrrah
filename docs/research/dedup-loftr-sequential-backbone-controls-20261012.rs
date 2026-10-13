
#[test]
fn sequential_backbone_matches_stacked_features_exactly() -> Result<(), LoftrError> {
    tch::set_num_threads(2);
    tch::manual_seed(173);
    let _inference = tch::no_grad_guard();
    let model = LoFTRModel::new(Device::Cpu, LoftrConfig::outdoor())?;
    for (batch, height, width) in [(1, 64, 64), (1, 64, 80), (1, 96, 128), (2, 64, 80)] {
        let left = Tensor::rand([batch, 1, height, width], (Kind::Float, Device::Cpu));
        let right = Tensor::rand([batch, 1, height, width], (Kind::Float, Device::Cpu));
        let stacked = model.backbone_features(&left, &right, batch, (height, width), (height, width), &|| false)?;
        let (coarse0, fine0) = model.backbone.forward_cancellable(&left, false, &|| false)?;
        let (coarse1, fine1) = model.backbone.forward_cancellable(&right, false, &|| false)?;
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
