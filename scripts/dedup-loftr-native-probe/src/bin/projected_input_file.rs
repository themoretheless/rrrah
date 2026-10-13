//! Independent preparation diagnostic; inference is performed in a separate process.
use rrrah_dedup::{
    animated::AnimationBudget, model_input::ModelInputPolicy,
    model_input_file::prepare_gray_model_file_with_projection, model_projection::ModelInputProjection,
};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    if args.len() != 2 {
        return Err("usage: projected_input_file source output.gray".into());
    }
    let mut request = rrrah_decode::DecodeRequest::new(&args[0]);
    request.assume_untagged_linear_srgb = true;
    let budget = rrrah_core::MemoryBudget::new(512 * 1024 * 1024);
    let prepared = prepare_gray_model_file_with_projection(
        &request,
        AnimationBudget {
            max_frames: 1,
            max_pixels: 1000000,
            max_file_bytes: 16000000,
        },
        ModelInputPolicy {
            max_side: 320,
            max_source_pixels: 1000000,
            max_output_pixels: 102400,
            max_pixel_reads: 4000000,
        },
        ModelInputProjection::CompositeReinhardV1 {
            exposure: 1.,
            background: [0.; 3],
        },
        &budget,
        || false,
    )?;
    std::fs::write(&args[1], &*prepared.input().pixels)?;
    prepared.verify_source(|| false)?;
    let dimensions = prepared.input().dimensions;
    let original = prepared.original_dimensions();
    drop(prepared);
    assert_eq!(budget.used(), 0);
    println!(
        "{}",
        serde_json::json!({"dimensions":dimensions,"original":original,"managed_used":budget.used(),"managed_peak":budget.peak()})
    );
    Ok(())
}
