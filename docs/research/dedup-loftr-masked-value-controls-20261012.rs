
#[test]
fn masked_values_remain_unchanged_with_live_aliases() -> Result<(), LoftrError> {
    tch::set_num_threads(2);
    let _inference = tch::no_grad_guard();
    let q = Tensor::ones([1, 2, 2, 4], (Kind::Float, Device::Cpu));
    let k = Tensor::ones([1, 3, 2, 4], (Kind::Float, Device::Cpu));
    let v = Tensor::arange_start(1, 25, (Kind::Float, Device::Cpu)).view([1, 3, 2, 4]);
    let saved = &v + 0.0;
    let alias = v.shallow_clone();
    let mask = Tensor::from_slice(&[1_i64, 0, 1]).view([1, 3]);
    let out = LinearAttention::default().forward(&q, &k, &v, None, Some(&mask))?;
    assert!(out.isfinite().all().int64_value(&[]) != 0);
    assert!(v.equal(&saved), "masked forward mutated borrowed values");
    assert!(alias.equal(&saved), "masked forward mutated a caller alias");
    assert!(q.eq(1.0).all().int64_value(&[]) != 0);
    assert!(k.eq(1.0).all().int64_value(&[]) != 0);
    Ok(())
}

#[test]
fn masked_call_does_not_change_subsequent_unmasked_result() -> Result<(), LoftrError> {
    tch::set_num_threads(2);
    let _inference = tch::no_grad_guard();
    let q = Tensor::ones([1, 2, 2, 4], (Kind::Float, Device::Cpu));
    let k = Tensor::ones([1, 3, 2, 4], (Kind::Float, Device::Cpu));
    let v = Tensor::arange_start(1, 25, (Kind::Float, Device::Cpu)).view([1, 3, 2, 4]);
    let module = LinearAttention::default();
    let expected = module.forward(&q, &k, &v, None, None)?;
    for entries in [[1_i64, 0, 1], [0, 1, 0], [1, 1, 1], [0, 0, 0]] {
        let mask = Tensor::from_slice(&entries).view([1, 3]);
        let _ = module.forward(&q, &k, &v, None, Some(&mask))?;
        let actual = module.forward(&q, &k, &v, None, None)?;
        assert!(actual.equal(&expected), "a masked call changed reusable inputs");
    }
    Ok(())
}

#[test]
fn masked_leaf_value_gradients_are_supported_and_inputs_preserved() -> Result<(), LoftrError> {
    tch::set_num_threads(2);
    let q = Tensor::ones([1, 2, 2, 4], (Kind::Float, Device::Cpu)).set_requires_grad(true);
    let k = Tensor::ones([1, 3, 2, 4], (Kind::Float, Device::Cpu)).set_requires_grad(true);
    let v = Tensor::arange_start(1, 25, (Kind::Float, Device::Cpu))
        .view([1, 3, 2, 4]).set_requires_grad(true);
    let saved = &v + 0.0;
    let mask = Tensor::from_slice(&[1_i64, 0, 1]).view([1, 3]);
    let out = LinearAttention::default().forward(&q, &k, &v, None, Some(&mask))?;
    out.sum(Kind::Float).backward();
    assert!(v.equal(&saved));
    for gradient in [q.grad(), k.grad(), v.grad()] {
        assert!(gradient.defined());
        assert!(gradient.isfinite().all().int64_value(&[]) != 0);
    }
    assert!(v.grad().get(0).get(1).eq(0.0).all().int64_value(&[]) != 0);
    assert!(v.grad().get(0).get(0).gt(0.0).all().int64_value(&[]) != 0);
    assert!(v.grad().get(0).get(2).gt(0.0).all().int64_value(&[]) != 0);
    Ok(())
}
