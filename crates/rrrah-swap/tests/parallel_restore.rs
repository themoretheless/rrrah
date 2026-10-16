use rrrah_memory::MemoryBudget;
use rrrah_swap::{PARALLEL_HASH_SCRATCH_BYTES, SwapError, SwapLimits, SwapStore};
use std::{
    io::{Read, Seek, SeekFrom, Write},
    sync::{Mutex, mpsc},
    time::Duration,
};

// Exercise the process-wide slot deterministically within this test binary.
static SERIAL: Mutex<()> = Mutex::new(());
const BLOCK: usize = 64 * 1024;
fn fixture() -> (tempfile::TempDir, SwapStore, rrrah_swap::SwapHandle, Vec<u8>) {
    let directory = tempfile::tempdir().unwrap();
    let source: Vec<_> = (0..8 * 1024 * 1024 + 97).map(|i| u8::try_from(i % 251).unwrap()).collect();
    let store = SwapStore::new(
        directory.path(),
        SwapLimits {
            max_bytes: source.len() as u64,
            max_objects: Some(1),
        },
    )
    .unwrap();
    let handle = store
        .write_chunks(source.len() as u64, [&source[..]], || false)
        .unwrap();
    (directory, store, handle, source)
}
fn drain(reader: &mut dyn Read) -> std::io::Result<Vec<u8>> {
    let mut result = Vec::new();
    let mut prefix = [0; 17];
    reader.read_exact(&mut prefix)?;
    result.extend(prefix);
    let mut part = [0; 16384];
    loop {
        let count = reader.read(&mut part)?;
        if count == 0 {
            break;
        }
        result.extend_from_slice(&part[..count]);
    }
    Ok(result)
}
fn parallel_available() -> bool {
    std::thread::available_parallelism().is_ok_and(|n| n.get() >= 2)
}

#[test]
fn exact_stream_and_memory_pressure_fallback() {
    let _serial = SERIAL.lock().unwrap();
    let (_directory, store, handle, source) = fixture();
    for limit in [0, PARALLEL_HASH_SCRATCH_BYTES as u64, 32 * 1024 * 1024] {
        let budget = MemoryBudget::new(limit);
        let actual = store
            .read_stream_with_scratch(&handle, || false, drain, &budget)
            .unwrap();
        assert_eq!(actual, source);
        assert_eq!(budget.used(), 0);
        if limit <= PARALLEL_HASH_SCRATCH_BYTES as u64 {
            assert_eq!(budget.peak(), 0);
        } else if parallel_available() {
            assert_eq!(budget.peak(), PARALLEL_HASH_SCRATCH_BYTES as u64);
        }
    }
}

#[test]
fn cancellation_during_batch_is_latched_and_releases_scratch() {
    let _serial = SERIAL.lock().unwrap();
    if !parallel_available() {
        return;
    }
    let (_directory, store, handle, source) = fixture();
    let budget = MemoryBudget::new(32 * 1024 * 1024);
    let mut batch_polls = 0;
    let result = store.read_stream_with_scratch(
        &handle,
        || {
            if budget.used() != 0 {
                batch_polls += 1;
            }
            batch_polls == 4
        },
        |reader| {
            // Even a codec swallowing the I/O cancellation cannot publish a value.
            let _ = drain(reader);
            Ok(())
        },
        &budget,
    );
    assert!(matches!(result, Err(SwapError::Cancelled)));
    assert_eq!(batch_polls, 4);
    assert_eq!(budget.used(), 0);
    assert_eq!(
        store
            .read_stream_with_scratch(&handle, || false, drain, &budget)
            .unwrap(),
        source
    );
    assert_eq!(budget.used(), 0);
}

#[test]
fn concurrent_stores_share_one_scratch_slot_and_do_not_wait() {
    let _serial = SERIAL.lock().unwrap();
    if !parallel_available() {
        return;
    }
    let (_directory, store, handle, source) = fixture();
    let (_other_directory, other, other_handle, other_source) = fixture();
    let first_budget = MemoryBudget::new(32 * 1024 * 1024);
    let second_budget = MemoryBudget::new(32 * 1024 * 1024);
    let (ready_tx, ready_rx) = mpsc::channel();
    let (release_tx, release_rx) = mpsc::channel();
    std::thread::scope(|scope| {
        let first_store = &store;
        let first_handle = &handle;
        let first_scratch = &first_budget;
        let first = scope.spawn(move || {
            first_store
                .read_stream_with_scratch(
                    first_handle,
                    || false,
                    |reader| {
                        let mut prefix = vec![0; BLOCK + 1];
                        reader.read_exact(&mut prefix)?;
                        assert_eq!(first_scratch.used(), PARALLEL_HASH_SCRATCH_BYTES as u64);
                        ready_tx.send(()).unwrap();
                        release_rx.recv_timeout(Duration::from_secs(10)).unwrap();
                        reader.read_to_end(&mut prefix)?;
                        Ok(prefix)
                    },
                    first_scratch,
                )
                .unwrap()
        });
        ready_rx.recv_timeout(Duration::from_secs(10)).unwrap();
        assert_eq!(
            other
                .read_stream_with_scratch(&other_handle, || false, drain, &second_budget)
                .unwrap(),
            other_source
        );
        assert_eq!(second_budget.peak(), 0);
        release_tx.send(()).unwrap();
        assert_eq!(first.join().unwrap(), source);
    });
    assert_eq!(first_budget.used(), 0);
    assert_eq!(second_budget.used(), 0);
    assert_eq!(
        other
            .read_stream_with_scratch(&other_handle, || false, drain, &second_budget)
            .unwrap(),
        other_source
    );
    assert_eq!(second_budget.peak(), PARALLEL_HASH_SCRATCH_BYTES as u64);
}

#[test]
fn parallel_integrity_rejects_partial_corrupt_trailing_and_truncated_objects() {
    let _serial = SERIAL.lock().unwrap();
    for damage in ["partial", "corrupt", "trailing", "truncated"] {
        let (directory, store, handle, _source) = fixture();
        let budget = MemoryBudget::new(32 * 1024 * 1024);
        let private = std::fs::read_dir(directory.path())
            .unwrap()
            .next()
            .unwrap()
            .unwrap()
            .path();
        let blob = std::fs::read_dir(private)
            .unwrap()
            .next()
            .unwrap()
            .unwrap()
            .path();
        let mut file = std::fs::OpenOptions::new().write(true).open(blob).unwrap();
        match damage {
            "corrupt" => {
                file.seek(SeekFrom::Start((BLOCK * 3) as u64)).unwrap();
                file.write_all(&[255]).unwrap();
            }
            "trailing" => {
                file.seek(SeekFrom::End(0)).unwrap();
                file.write_all(&[255]).unwrap();
            }
            "truncated" => file.set_len(handle.byte_len() - 1).unwrap(),
            _ => {}
        }
        drop(file);
        let result = store.read_stream_with_scratch(
            &handle,
            || false,
            |reader| {
                if damage == "partial" {
                    let mut prefix = vec![0; BLOCK + 1];
                    reader.read_exact(&mut prefix)?;
                    Ok(prefix)
                } else {
                    drain(reader)
                }
            },
            &budget,
        );
        match damage {
            "corrupt" => assert!(matches!(result, Err(SwapError::Corrupt))),
            "truncated" => assert!(matches!(result, Err(SwapError::Io(_)))),
            _ => assert!(matches!(result, Err(SwapError::Length))),
        }
        assert_eq!(budget.used(), 0);
    }
}
