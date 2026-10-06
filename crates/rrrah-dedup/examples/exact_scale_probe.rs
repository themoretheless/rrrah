//! Warm local-file exact scan probe; generated fixtures use a fixed 64 KiB buffer.
use rrrah_dedup::exact::{Options, scan};
use std::{
    fs::File,
    io::{Seek, SeekFrom, Write},
    time::Instant,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    for mib in [1u64, 8, 64, 256] {
        let directory = tempfile::tempdir()?;
        let bytes = mib * 1024 * 1024;
        let buffer = [0x5au8; 64 * 1024];
        let a = directory.path().join("a");
        let b = directory.path().join("b");
        let different = directory.path().join("different-middle");
        for path in [&a, &b, &different] {
            let mut file = File::create(path)?;
            for _ in 0..bytes / buffer.len() as u64 {
                file.write_all(&buffer)?;
            }
        }
        let mut file = File::options().write(true).open(&different)?;
        file.seek(SeekFrom::Start(bytes / 2))?;
        file.write_all(&[0x59])?;
        drop(file);
        for run in 0..4 {
            let start = Instant::now();
            let report = scan(&[directory.path().to_path_buf()], &Options::default(), || false);
            let elapsed_ns = start.elapsed().as_nanos();
            assert!(report.complete(), "{:?}", report.issues);
            assert_eq!(report.files_observed, 3);
            assert_eq!(report.groups.len(), 1);
            assert_eq!(report.groups[0].paths, vec![a.clone(), b.clone()]);
            assert_eq!(report.groups[0].bytes, bytes);
            if run != 0 {
                println!(
                    "{{\"bytes\":{bytes},\"run\":{run},\"elapsed_ns\":{elapsed_ns},\"bytes_read\":{}}}",
                    report.bytes_read
                );
            }
        }
    }
    #[cfg(target_os = "linux")]
    for line in std::fs::read_to_string("/proc/self/status")?.lines() {
        if line.starts_with("VmHWM:") || line.starts_with("VmRSS:") {
            eprintln!("process_memory {line}");
        }
    }
    Ok(())
}
