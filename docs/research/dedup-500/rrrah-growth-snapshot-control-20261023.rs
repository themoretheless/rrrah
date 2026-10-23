use rrrah_dedup::exact::{ContentSnapshot, SnapshotError, Options, scan, scan_verified};
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
    assert!(matches!(ContentSnapshot::read(&path, 1024, || true), Err(SnapshotError::Cancelled)));
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
    let other = directory.join("other.bin");
    let paths = [path.clone(), other.clone()];
    let mut api_checks = Vec::new();
    for verified in [false, true] {
        let run = |cancel: &dyn Fn() -> bool| {
            if verified { scan_verified(&paths, &Options::default(), cancel) }
            else { scan(&paths, &Options::default(), cancel) }
        };
        fs::write(&path, b"one").unwrap();
        fs::write(&other, b"one").unwrap();
        let observed = Cell::new(0);
        let positive = run(&|| { observed.set(observed.get() + 1); false });
        assert!(positive.complete());
        assert_eq!(positive.groups.len(), 1);
        let cancelled = run(&|| true);
        assert!(cancelled.cancelled && cancelled.groups.is_empty());
        let total = observed.get();
        for victim in &paths {
        for stop in 1..=total {
            fs::write(&path, b"one").unwrap();
            fs::write(&other, b"one").unwrap();
            let seen = Cell::new(0);
            let mutated = Cell::new(false);
            let report = run(&|| {
                seen.set(seen.get() + 1);
                if seen.get() == stop {
                    fs::OpenOptions::new().append(true).open(victim).unwrap()
                        .write_all(b"-appended").unwrap();
                    mutated.set(true);
                }
                false
            });
            assert!(mutated.get(), "verified={verified} checkpoint={stop}");
            assert!(!report.cancelled);
            assert!(report.groups.is_empty(), "verified={verified} checkpoint={stop}: {report:?}");
            let retry = run(&|| false);
            assert!(retry.complete() && retry.groups.is_empty(), "{retry:?}");
        }
        }
        api_checks.push(total);
    }
    fs::remove_file(&path).unwrap();
    fs::remove_file(&other).unwrap();
    fs::remove_dir(&directory).unwrap();
    println!("{{\"baseline_checkpoints\":{checkpoints},\"growth_refusals\":{},\"fresh_retries_verified\":{},\"scan_mutation_checkpoints\":{},\"verified_scan_mutation_checkpoints\":{},\"scan_mutation_cases\":{},\"verified_scan_mutation_cases\":{},\"prelatched_snapshot_cancelled\":true,\"prelatched_scan_cancellations\":2}}", checkpoints - 1, checkpoints - 1, api_checks[0], api_checks[1], api_checks[0] * 2, api_checks[1] * 2);
}
