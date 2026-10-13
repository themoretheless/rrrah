//! Isolated native inference diagnostic; no duplicate verdict or library budget guarantee.
use loftr::{LoftrConfig, LoftrModel};
use std::{error::Error, fs, path::Path};
use tch::{Device, Tensor};

fn image(path: &Path, width: i64, height: i64) -> Result<Tensor, Box<dyn Error>> {
    let count = width
        .checked_mul(height)
        .filter(|n| *n > 0 && *n <= 640 * 640)
        .ok_or("invalid diagnostic image dimensions")?;
    if fs::metadata(path)?.len() != u64::try_from(count)? {
        return Err("raw grayscale length mismatch".into());
    }
    let bytes = fs::read(path)?;
    let values: Vec<f32> = bytes.into_iter().map(|x| f32::from(x) / 255.0).collect();
    Ok(Tensor::f_from_slice(&values)?.f_reshape([1, 1, height, width])?)
}

fn main() -> Result<(), Box<dyn Error>> {
    let args: Vec<String> = std::env::args().collect();
    if args.len() != 8 {
        return Err("usage: probe weights source.gray source_width source_height target.gray target_width target_height".into());
    }
    tch::set_num_threads(2);
    tch::manual_seed(173);
    let left = image(Path::new(&args[2]), args[3].parse()?, args[4].parse()?)?;
    let right = image(Path::new(&args[5]), args[6].parse()?, args[7].parse()?)?;
    let mut model = LoftrModel::new(Device::Cpu, LoftrConfig::outdoor())?;
    model.load_weights(&args[1])?;
    let _inference = tch::no_grad_guard();
    let result = model.forward(&left, &right)?;
    let count = result.confidence.numel();
    if count > 5000 {
        return Err("diagnostic proposal cap exceeded; no truncation".into());
    }
    let points0 = Vec::<Vec<f32>>::try_from(&result.keypoints0)?;
    let points1 = Vec::<Vec<f32>>::try_from(&result.keypoints1)?;
    let confidence = Vec::<f32>::try_from(&result.confidence)?;
    if points0.len() != count || points1.len() != count {
        return Err("inconsistent native proposal shapes".into());
    }
    println!(
        "{}",
        serde_json::json!({"keypoints0":points0,"keypoints1":points1,"confidence":confidence})
    );
    Ok(())
}
