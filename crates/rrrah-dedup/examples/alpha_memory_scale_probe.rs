//! Independent straight-alpha cross-encoding fixture qualification.
use rrrah_core::MemoryBudget;
use rrrah_decode::DecodeRequest;
use rrrah_dedup::decode::decode_selected_frame;
use std::{path::PathBuf, time::Instant};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let folder = PathBuf::from(std::env::args_os().nth(1).ok_or("fixture folder required")?);
    for side in [1024u64, 2048, 4096] {
        let pixels = side * side;
        let budget = MemoryBudget::new(1024 * 1024 * 1024);
        let baseline = decode_selected_frame(
            &DecodeRequest::new(folder.join(format!("{side}-base.png"))),
            pixels,
            &budget,
            || false,
        )?;
        for (name, equal) in [("base.tiff", true), ("hidden.png", true), ("changed.png", false)] {
            let mut request = DecodeRequest::new(folder.join(format!("{side}-{name}")));
            if name.ends_with("tiff") {
                request.assume_untagged_srgb = true;
            }
            for bytes in [0, 1] {
                let refused = MemoryBudget::new(bytes);
                assert!(decode_selected_frame(&request, pixels, &refused, || false).is_err());
                assert_eq!(refused.used(), 0);
            }
            let start = Instant::now();
            let decoded = decode_selected_frame(&request, pixels, &budget, || false)?;
            let decode_ns = start.elapsed().as_nanos();
            assert_eq!(
                baseline.same_selected_frame(&decoded, || false)?,
                equal,
                "{side}-{name}"
            );
            drop(decoded);
            println!(
                "{{\"side\":{side},\"variant\":\"{name}\",\"equal\":{equal},\"decode_ns\":{decode_ns}}}"
            );
        }
        drop(baseline);
        assert_eq!(budget.used(), 0);
    }
    #[cfg(target_os = "linux")]
    for line in std::fs::read_to_string("/proc/self/status")?.lines() {
        if line.starts_with("VmHWM:") || line.starts_with("VmRSS:") {
            eprintln!("process_memory {line}");
        }
    }
    Ok(())
}
