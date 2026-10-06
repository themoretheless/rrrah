//! Warm local exact-scan cancellation measurement, excluding fixture creation.
use rrrah_dedup::exact::{Options, scan};
use std::{cell::Cell, fs::File, io::Write, time::Instant};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    for mib in [1u64, 8, 64, 256] {
        let directory = tempfile::tempdir()?;
        let bytes = mib * 1024 * 1024;
        let buffer = [0x5au8; 64 * 1024];
        for name in ["a", "b"] {
            let mut file = File::create(directory.path().join(name))?;
            for _ in 0..bytes / buffer.len() as u64 {
                file.write_all(&buffer)?;
            }
        }
        for run in 0..5 {
            let calls = Cell::new(0usize);
            let cancelled_at = Cell::new(None);
            let start = Instant::now();
            let report = scan(&[directory.path().to_path_buf()], &Options::default(), || {
                calls.set(calls.get() + 1);
                if calls.get() >= 25 {
                    if cancelled_at.get().is_none() {
                        cancelled_at.set(Some(Instant::now()));
                    }
                    true
                } else {
                    false
                }
            });
            let returned_at = Instant::now();
            assert!(report.cancelled);
            assert!(!report.complete());
            assert!(report.groups.is_empty());
            assert!(report.bytes_read > 64 * 1024);
            assert!(report.bytes_read < 2 * bytes);
            let cancellation_ns = returned_at
                .duration_since(cancelled_at.get().expect("cancellation observed"))
                .as_nanos();
            let total_ns = returned_at.duration_since(start).as_nanos();
            println!(
                "{{\"bytes\":{bytes},\"run\":{run},\"cancellation_ns\":{cancellation_ns},\"total_ns\":{total_ns},\"bytes_read\":{},\"callbacks\":{}}}",
                report.bytes_read,
                calls.get()
            );
        }
        let retry = scan(&[directory.path().to_path_buf()], &Options::default(), || false);
        assert!(retry.complete());
        assert_eq!(retry.groups.len(), 1);
        assert_eq!(retry.groups[0].paths.len(), 2);
        assert_eq!(retry.groups[0].bytes, bytes);
    }
    #[cfg(target_os = "linux")]
    for line in std::fs::read_to_string("/proc/self/status")?.lines() {
        if line.starts_with("VmHWM:") || line.starts_with("VmRSS:") {
            eprintln!("process_memory {line}");
        }
    }
    Ok(())
}
