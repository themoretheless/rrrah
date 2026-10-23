use rrrah_dedup::exact::{ContentSnapshot, SnapshotError};
use std::{cell::Cell, fs, io::Write, path::PathBuf};

fn main() {
    let directory = PathBuf::from(std::env::args_os().nth(1).unwrap());
    fs::create_dir(&directory).unwrap();
    let path = directory.join("owned.bin");
    fs::write(&path, b"one").unwrap();
    let calls = Cell::new(0);
    let baseline = ContentSnapshot::read(&path, 1024, || {
        calls.set(calls.get() + 1);
        false
    }).unwrap();
    assert_eq!(baseline.byte_count(), 3);
    let checkpoints = calls.get();
    assert!(checkpoints > 2);
    for stop in 2..=checkpoints {
        fs::write(&path, b"one").unwrap();
        let seen = Cell::new(0);
        let mutated = Cell::new(false);
        let result = ContentSnapshot::read(&path, 1024, || {
            seen.set(seen.get() + 1);
            if seen.get() == stop {
                fs::OpenOptions::new().append(true).open(&path).unwrap()
                    .write_all(b"-appended").unwrap();
                mutated.set(true);
            }
            false
        });
        assert!(mutated.get(), "checkpoint {stop} not reached");
        assert!(matches!(result, Err(SnapshotError::Changed)), "checkpoint {stop}: {result:?}");
        let retry = ContentSnapshot::read(&path, 1024, || false).unwrap();
        assert_eq!(retry.byte_count(), 12);
        retry.verify(|| false).unwrap();
    }
    fs::remove_file(&path).unwrap();
    fs::remove_dir(&directory).unwrap();
    println!("{{\"baseline_checkpoints\":{checkpoints},\"growth_refusals\":{},\"fresh_retries_verified\":{}}}", checkpoints - 1, checkpoints - 1);
}
