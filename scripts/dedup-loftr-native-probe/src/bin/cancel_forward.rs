//! Real-weight inference cancellation diagnostic over pinned native grayscale inputs.
use loftr::{LoftrConfig, LoftrError, LoftrModel};
use std::{cell::Cell, error::Error};
use tch::{Device, Kind, Tensor};

fn input(path: &str, width: i64, height: i64) -> Result<Tensor, Box<dyn Error>> {
    let bytes = std::fs::read(path)?;
    if width <= 0 || height <= 0 || width.checked_mul(height) != Some(bytes.len() as i64) {
        return Err("gray input size mismatch".into());
    }
    Ok(Tensor::f_from_slice(&bytes)?
        .f_to_kind(Kind::Float)?
        .f_div_scalar(255.)?
        .f_reshape([1, 1, height, width])?)
}

fn main() -> Result<(), Box<dyn Error>> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    if args.len() != 7 {
        return Err("usage: cancel_forward left.gray width height right.gray width height weights".into());
    }
    tch::set_num_threads(2);
    let left = input(&args[0], args[1].parse()?, args[2].parse()?)?;
    let right = input(&args[3], args[4].parse()?, args[5].parse()?)?;
    let mut model = LoftrModel::new(Device::Cpu, LoftrConfig::outdoor())?;
    model.load_weights(&args[6])?;
    let _inference = tch::no_grad_guard();
    let calls = Cell::new(0usize);
    let expected = model.forward_cancellable(&left, &right, &|| {
        calls.set(calls.get() + 1);
        false
    })?;
    let total = calls.get();
    if total < 65 || expected.confidence.numel() == 0 {
        return Err("real positive inference did not exercise expected stages".into());
    }
    let mut points = vec![1, 2, 16, 64, total / 2, total - 8, total - 5, total - 1, total];
    points.sort_unstable();
    points.dedup();
    for &stop in &points {
        let calls = Cell::new(0usize);
        let result = model.forward_cancellable(&left, &right, &|| {
            calls.set(calls.get() + 1);
            calls.get() == stop
        });
        if !matches!(result, Err(LoftrError::Cancelled)) || calls.get() != stop {
            return Err(format!("cancellation failed at checkpoint {stop}/{total}").into());
        }
    }
    let retry = model.forward(&left, &right)?;
    if !expected.keypoints0.equal(&retry.keypoints0)
        || !expected.keypoints1.equal(&retry.keypoints1)
        || !expected.confidence.equal(&retry.confidence)
        || !expected.batch_indexes.equal(&retry.batch_indexes)
    {
        return Err("retry after cancellation changed matches".into());
    }
    println!(
        "{}",
        serde_json::json!({"status":"ok", "checkpoints":total,
        "cancelled_at":points,"raw_proposals":expected.confidence.numel(),
        "retry_exact":true,"scope":"Cooperative checkpoints; individual libtorch kernels are not preempted."})
    );
    Ok(())
}
