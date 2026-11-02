//! Read-only in-memory selected-frame equality probe, not whole-file identity.
use rrrah_core::MemoryBudget;
use rrrah_decode::DecodeRequest;
use rrrah_dedup::{animated::AnimationBudget, decode::decode_selected_frame_bounded};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    if !(2..=4).contains(&args.len()) {
        return Err("usage: selected_frame_compare LEFT RIGHT [LEFT_INDEX] [RIGHT_INDEX]".into());
    }
    let index = |at: usize| -> Result<usize, Box<dyn std::error::Error>> {
        args.get(at)
            .map_or(Ok(0), |v| Ok(v.to_str().ok_or("index must be UTF-8")?.parse()?))
    };
    let mut left = DecodeRequest::new(&args[0]);
    let mut right = DecodeRequest::new(&args[1]);
    left.image_index = index(2)?;
    right.image_index = index(3)?;
    let budget = MemoryBudget::new(128 * 1024 * 1024);
    let limits = AnimationBudget {
        max_frames: 256,
        max_pixels: 1_000_000,
        max_file_bytes: 16 * 1024 * 1024,
    };
    let a = decode_selected_frame_bounded(&left, limits, &budget, || false)?;
    let b = decode_selected_frame_bounded(&right, limits, &budget, || false)?;
    let equal = a.same_selected_frame(&b, || false)?;
    let digest_equal = a.selected_frame_digest(|| false)? == b.selected_frame_digest(|| false)?;
    if equal && !digest_equal {
        return Err("equal frames have inconsistent digests".into());
    }
    println!(
        "left_dimensions={:?} right_dimensions={:?} left_index={} left_count={} right_index={} right_count={} selected_frame_equal={} digest_equal={}",
        a.view(|| false)?.dimensions(),
        b.view(|| false)?.dimensions(),
        a.image_index(),
        a.image_count(),
        b.image_index(),
        b.image_count(),
        equal,
        digest_equal
    );
    drop((a, b));
    println!("managed_used={} managed_peak={}", budget.used(), budget.peak());
    if budget.used() != 0 {
        return Err("retained managed memory".into());
    }
    Ok(())
}
