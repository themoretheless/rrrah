use rrrah_memory::MemoryBudget;
use rrrah_swap::{SwapError, SwapLimits, SwapStore};
use std::sync::atomic::{AtomicBool, Ordering};

#[test]
fn partially_read_cancelled_restore_releases_ram_and_preserves_retryable_handle() {
    const SIZE: usize = 192 * 1024;
    let directory = tempfile::tempdir().unwrap();
    let store = SwapStore::new(
        directory.path(),
        SwapLimits {
            max_bytes: SIZE as u64,
            max_objects: Some(1),
        },
    )
    .unwrap();
    let payload: Vec<u8> = (0..SIZE).map(|index| (index % 251) as u8).collect();
    let handle = store
        .write_chunks(SIZE as u64, payload.chunks(4096), || false)
        .unwrap();
    let root = MemoryBudget::new(SIZE as u64);
    let output = root.child(SIZE as u64);
    let blocked = root.try_buffer(SIZE, 0u8).unwrap();
    assert!(matches!(
        store.restore(&handle, &output, || false),
        Err(SwapError::Memory(_))
    ));
    assert_eq!(output.used(), 0);
    drop(blocked);

    let cancelled = AtomicBool::new(false);
    let result = store.read_stream(
        &handle,
        || cancelled.load(Ordering::Acquire),
        |reader| {
            let mut destination = output.try_buffer(SIZE, 0u8).map_err(std::io::Error::other)?;
            reader.read_exact(&mut destination[..4096])?;
            assert_eq!(&destination[..4096], &payload[..4096]);
            assert_eq!(root.used(), SIZE as u64);
            assert_eq!(output.used(), SIZE as u64);
            cancelled.store(true, Ordering::Release);
            reader.read_exact(&mut destination[4096..])?;
            Ok(destination.freeze())
        },
    );
    assert!(matches!(result, Err(SwapError::Cancelled)));
    assert_eq!(root.used(), 0);
    assert_eq!(output.used(), 0);
    assert_eq!(store.usage().unwrap().objects, 1);
    assert_eq!(store.usage().unwrap().bytes, SIZE as u64);

    let restored = store.restore(&handle, &output, || false).unwrap();
    assert_eq!(&*restored, payload.as_slice());
    let last_owner = restored.clone();
    drop(restored);
    assert_eq!(root.used(), SIZE as u64);
    drop(last_owner);
    assert_eq!(root.used(), 0);
    assert_eq!(output.used(), 0);
    drop(handle);
    assert_eq!(store.usage().unwrap().objects, 0);
    assert_eq!(store.usage().unwrap().bytes, 0);
}
