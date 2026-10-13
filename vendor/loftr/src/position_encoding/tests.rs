use super::*;

#[test]
fn create_position_encoding_has_expected_shape() -> Result<(), LoftrError> {
    let pe = create_position_encoding(256, (32, 48), false, Device::Cpu)?;
    assert_eq!(pe.size(), vec![1, 256, 32, 48]);
    Ok(())
}

#[test]
fn forward_expands_internal_encoding_when_input_is_larger() -> Result<(), LoftrError> {
    let mut module = PositionEncodingSine::new(256, (8, 8), false, Device::Cpu)?;
    let input = Tensor::zeros([1, 256, 16, 12], (Kind::Float, Device::Cpu));
    let out = module.forward(&input)?;
    assert_eq!(out.size(), vec![1, 256, 16, 12]);
    assert!(module.pe.size()[2] >= 16);
    assert!(module.pe.size()[3] >= 12);
    Ok(())
}

#[test]
fn temp_bug_fix_changes_encoding_values() -> Result<(), LoftrError> {
    let fixed = create_position_encoding(256, (4, 4), true, Device::Cpu)?;
    let legacy = create_position_encoding(256, (4, 4), false, Device::Cpu)?;
    let diff = (&fixed - &legacy).abs().sum(Kind::Float).double_value(&[]);
    assert!(diff > 0.0);
    Ok(())
}

#[test]
fn forward_rejects_wrong_channel_count() {
    let mut module = match PositionEncodingSine::new(256, (8, 8), false, Device::Cpu) {
        Ok(module) => module,
        Err(err) => panic!("module construction failed unexpectedly: {err}"),
    };
    let input = Tensor::zeros([1, 128, 8, 8], (Kind::Float, Device::Cpu));
    match module.forward(&input) {
        Ok(_) => panic!("channel mismatch should fail"),
        Err(err) => assert!(format!("{err}").contains("d_model mismatch")),
    }
}

#[test]
fn alternating_aspect_ratios_retain_only_the_current_extent_and_preserve_values()
-> Result<(), LoftrError> {
    for fixed in [false, true] {
        let mut reused = PositionEncodingSine::new(8, (8, 8), fixed, Device::Cpu)?;
        let mut prior = None;
        for (height, width) in [(24, 2), (2, 24), (4, 4), (24, 2), (2, 24)] {
            let input = Tensor::zeros([1, 8, height, width], (Kind::Float, Device::Cpu));
            let mut fresh = PositionEncodingSine::new(8, (8, 8), fixed, Device::Cpu)?;
            let expected = fresh.forward(&input)?;
            let output = reused.forward(&input)?;
            assert_eq!(reused.pe.size(), vec![1, 8, height.max(8), width.max(8)]);
            assert_eq!((&output - &expected).abs().max().double_value(&[]), 0.);
            if let Some((old, saved)) = prior.take() {
                let difference: Tensor = old - saved;
                assert_eq!(difference.abs().max().double_value(&[]), 0.);
            }
            prior = Some((output.shallow_clone(), output.copy()));
        }
        let before = reused.pe.size();
        assert!(
            reused
                .forward(&Tensor::zeros([1, 4, 64, 64], (Kind::Float, Device::Cpu)))
                .is_err()
        );
        assert_eq!(reused.pe.size(), before);
    }
    Ok(())
}

#[test]
fn small_floor_matches_original_floor_for_reused_inputs() -> Result<(), LoftrError> {
    for fixed in [false, true] {
        let mut original = PositionEncodingSine::new(256, (256, 256), fixed, Device::Cpu)?;
        let mut candidate = PositionEncodingSine::new(256, (1, 1), fixed, Device::Cpu)?;
        for (height, width) in [
            (80, 80),
            (48, 80),
            (80, 48),
            (1, 1),
            (257, 2),
            (2, 257),
            (80, 80),
        ] {
            let input = Tensor::randn([1, 256, height, width], (Kind::Float, Device::Cpu));
            let expected = original.forward(&input)?;
            let actual = candidate.forward(&input)?;
            let delta = (&actual - &expected).abs().max().double_value(&[]);
            assert_eq!(delta, 0., "fixed={fixed}, shape={height}x{width}");
            assert_eq!(candidate.pe.size(), vec![1, 256, height, width]);
        }
    }
    Ok(())
}
