
#[test]
fn full_attention_key_mask_without_query_mask_excludes_hidden_values() -> Result<(), LoftrError> {
    tch::set_num_threads(2);
    let q = Tensor::ones([1, 2, 1, 1], (Kind::Float, Device::Cpu));
    let k = Tensor::zeros([1, 3, 1, 1], (Kind::Float, Device::Cpu));
    let v = Tensor::from_slice(&[2_f32, 10.0, 20.0]).view([1, 3, 1, 1]);
    let mask = Tensor::from_slice(&[1_i64, 0, 1]).view([1, 3]);
    let out = FullAttention::default().forward(&q, &k, &v, None, Some(&mask))?;
    assert!((out.double_value(&[0, 0, 0, 0]) - 11.0).abs() < 1e-6);
    assert!((out.double_value(&[0, 1, 0, 0]) - 11.0).abs() < 1e-6);
    Ok(())
}

#[test]
fn full_attention_query_mask_independently_zeroes_hidden_rows() -> Result<(), LoftrError> {
    tch::set_num_threads(2);
    let q = Tensor::ones([1, 2, 1, 1], (Kind::Float, Device::Cpu));
    let k = Tensor::zeros([1, 3, 1, 1], (Kind::Float, Device::Cpu));
    let v = Tensor::from_slice(&[2_f32, 10.0, 20.0]).view([1, 3, 1, 1]);
    let query = Tensor::from_slice(&[1_i64, 0]).view([1, 2]);
    let keys = Tensor::ones([1, 3], (Kind::Bool, Device::Cpu));
    for key_mask in [None, Some(&keys)] {
        let out = FullAttention::default().forward(&q, &k, &v, Some(&query), key_mask)?;
        assert!(out.isfinite().all().int64_value(&[]) != 0);
        assert!((out.double_value(&[0, 0, 0, 0]) - 32.0 / 3.0).abs() < 1e-5);
        assert_eq!(out.double_value(&[0, 1, 0, 0]), 0.0);
    }
    Ok(())
}

#[test]
fn full_attention_empty_key_support_is_finite_zero_with_finite_gradients() -> Result<(), LoftrError> {
    tch::set_num_threads(2);
    let q = Tensor::ones([2, 2, 1, 1], (Kind::Float, Device::Cpu)).set_requires_grad(true);
    let k = Tensor::zeros([2, 2, 1, 1], (Kind::Float, Device::Cpu)).set_requires_grad(true);
    let v = Tensor::from_slice(&[3_f32, 7.0, 5.0, 11.0]).view([2, 2, 1, 1]).set_requires_grad(true);
    let saved = &v + 0.0;
    let query = Tensor::from_slice(&[1_i64, 0, 1, 1]).view([2, 2]);
    let keys = Tensor::from_slice(&[0_i64, 0, 1, 0]).view([2, 2]);
    let out = FullAttention::default().forward(&q, &k, &v, Some(&query), Some(&keys))?;
    assert!(out.isfinite().all().int64_value(&[]) != 0);
    assert!(out.get(0).eq(0.0).all().int64_value(&[]) != 0);
    assert!(out.get(1).eq(5.0).all().int64_value(&[]) != 0);
    out.sum(Kind::Float).backward();
    assert!(v.equal(&saved));
    for gradient in [q.grad(), k.grad(), v.grad()] {
        assert!(gradient.defined());
        assert!(gradient.isfinite().all().int64_value(&[]) != 0);
        assert!(gradient.get(0).eq(0.0).all().int64_value(&[]) != 0);
    }
    assert_eq!(v.grad().double_value(&[1, 0, 0, 0]), 2.0);
    assert_eq!(v.grad().double_value(&[1, 1, 0, 0]), 0.0);
    // A key-only mask with no available key must follow the same zero policy.
    let out = FullAttention::default().forward(&q, &k, &v, None, Some(&keys))?;
    assert!(out.isfinite().all().int64_value(&[]) != 0);
    assert!(out.get(0).eq(0.0).all().int64_value(&[]) != 0);
    Ok(())
}

#[test]
fn full_attention_single_invalid_mask_shape_is_reported() {
    let q = Tensor::ones([1, 2, 1, 1], (Kind::Float, Device::Cpu));
    let k = Tensor::ones([1, 3, 1, 1], (Kind::Float, Device::Cpu));
    let v = Tensor::ones([1, 3, 1, 1], (Kind::Float, Device::Cpu));
    let wrong_query = Tensor::ones([1, 3], (Kind::Bool, Device::Cpu));
    let wrong_keys = Tensor::ones([1, 2], (Kind::Bool, Device::Cpu));
    assert!(matches!(FullAttention::default().forward(&q, &k, &v, Some(&wrong_query), None), Err(LoftrError::InvalidConfig(_))));
    assert!(matches!(FullAttention::default().forward(&q, &k, &v, None, Some(&wrong_keys)), Err(LoftrError::InvalidConfig(_))));
}
