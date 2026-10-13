//! Native decoded JPEG-to-model fixture diagnostic, without Python pixel preparation.
use rrrah_dedup::{
    animated::AnimationBudget, model_input::ModelInputPolicy, model_input_file::prepare_gray_model_file,
};
use std::{error::Error, io::Write};
fn main() -> Result<(), Box<dyn Error>> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    if args.len() != 3 {
        return Err("usage: model_input_file_probe source max_side output.gray".into());
    }
    let request = rrrah_decode::DecodeRequest::new(&args[0]);
    let budget = rrrah_core::MemoryBudget::new(512 * 1024 * 1024);
    let input = prepare_gray_model_file(
        &request,
        AnimationBudget {
            max_frames: 1,
            max_pixels: 12800000,
            max_file_bytes: 5242880,
        },
        ModelInputPolicy {
            max_side: args[1].parse()?,
            max_source_pixels: 12800000,
            max_output_pixels: 409600,
            max_pixel_reads: 51200000,
        },
        &budget,
        || false,
    )?;
    let mut file = std::fs::OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(&args[2])?;
    file.write_all(&input.input().pixels)?;
    file.flush()?;
    input.verify_source(|| false)?;
    let dimensions = input.input().dimensions;
    let original = input.input().original;
    drop(input);
    assert_eq!(budget.used(), 0);
    println!(
        "{{\"dimensions\":{dimensions:?},\"original\":{original:?},\"memory_used\":0,\"memory_peak\":{}}}",
        budget.peak()
    );
    Ok(())
}
