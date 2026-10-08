//! Manual sustained qualification with delayed real file I/O through the codec.
use rrrah_cache::{
    CacheLimits, ImageSwapCache, ImageSwapConfig, MemoryBudget, SwapPayload, SwapPayloadError,
};
use rrrah_core::{DecodedRaster, RasterColorSpace, RasterPixels};
use std::{
    io::{Read, Write},
    sync::Arc,
    time::{Duration, Instant},
};

fn physical_usage(directory: &std::path::Path) -> (u64, usize) {
    let mut usage = (0, 0);
    for entry in std::fs::read_dir(directory).unwrap() {
        let path = entry.unwrap().path();
        let metadata = match std::fs::metadata(&path) {
            Ok(value) => value,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
            Err(error) => panic!("swap metadata: {error}"),
        };
        if metadata.is_dir() {
            let child = physical_usage(&path);
            usage.0 += child.0;
            usage.1 += child.1;
        } else if metadata.is_file() {
            usage.0 += metadata.len();
            usage.1 += 1;
        }
    }
    usage
}

struct SlowRaster(DecodedRaster);
struct SlowIo<'a, T>(&'a mut T);
impl<T: Write> Write for SlowIo<'_, T> {
    fn write(&mut self, data: &[u8]) -> std::io::Result<usize> {
        std::thread::sleep(Duration::from_millis(2));
        self.0.write(&data[..data.len().min(65536)])
    }
    fn flush(&mut self) -> std::io::Result<()> {
        self.0.flush()
    }
}
impl<T: Read> Read for SlowIo<'_, T> {
    fn read(&mut self, data: &mut [u8]) -> std::io::Result<usize> {
        std::thread::sleep(Duration::from_millis(2));
        let length = data.len().min(65536);
        self.0.read(&mut data[..length])
    }
}
impl SwapPayload for SlowRaster {
    fn capacity_bytes(&self) -> u64 {
        self.0.capacity_bytes()
    }
    fn payload_len(&self) -> std::io::Result<u64> {
        self.0.payload_len()
    }
    fn write_payload(&self, writer: &mut impl Write) -> std::io::Result<()> {
        self.0.write_payload(&mut SlowIo(writer))
    }
    fn read_payload(
        reader: &mut impl Read,
        bytes: u64,
        budget: &MemoryBudget,
    ) -> Result<Self, SwapPayloadError> {
        DecodedRaster::read_payload(&mut SlowIo(reader), bytes, budget).map(Self)
    }
}

#[test]
#[ignore = "manual sustained delayed-I/O pressure qualification; default 60 seconds"]
fn sustained_large_keyspace_delayed_swap_pressure_and_retry() {
    sustained_swap_with_limits(8 * 1024 * 1024, 16, Duration::from_secs(2));
}

#[test]
#[ignore = "manual sustained independent byte-cap qualification"]
fn sustained_byte_cap_evicts_without_count_or_ttl_pressure() {
    // Each encoded raster is 262144 pixel bytes plus a 64-byte header.
    sustained_swap_with_limits(3 * (262144 + 64), 100, Duration::from_secs(3600));
}

fn sustained_swap_with_limits(disk_max_bytes: u64, disk_max_objects: usize, ttl: Duration) {
    let seconds = std::env::var("RRRAH_STRESS_SECONDS")
        .map(|s| s.parse::<u64>().unwrap())
        .unwrap_or(60);
    assert!(seconds >= 10, "qualification requires at least ten seconds");
    let directory = tempfile::tempdir().unwrap();
    let root = MemoryBudget::new(2 * 1024 * 1024);
    let queue = MemoryBudget::new(1024 * 1024);
    let swap: ImageSwapCache<u64, SlowRaster> = ImageSwapCache::new_with_budgets(
        directory.path(),
        ImageSwapConfig {
            limits: CacheLimits {
                max_bytes: disk_max_bytes,
                max_entries: Some(disk_max_objects),
                ttl: Some(ttl),
            },
            queue_bytes: 1024 * 1024,
            queue_count: 4,
            restore_bytes: 512 * 1024,
        },
        queue.clone(),
        root.clone(),
    )
    .unwrap();
    let start = Instant::now();
    let mut cycles = 0u64;
    let mut refusals = 0;
    let mut disk_peak_bytes = 0;
    let mut disk_peak_objects = 0;
    while start.elapsed() < Duration::from_secs(seconds) {
        let key = cycles % 100_000;
        let value = (cycles % 251) as u8;
        let raster = DecodedRaster::new(
            256,
            256,
            RasterPixels::Rgba8(Arc::new(vec![value; 256 * 256 * 4]).into()),
            RasterColorSpace::Srgb,
        )
        .unwrap()
        .try_manage_pixels(&root)
        .unwrap();
        swap.enqueue(key, SlowRaster(raster));
        swap.wait_idle().unwrap();
        assert_eq!(root.used(), 0);
        assert_eq!(queue.used(), 0);
        let usage = swap.disk_usage().unwrap();
        assert!(usage.bytes <= disk_max_bytes);
        assert!(usage.objects <= disk_max_objects);
        disk_peak_bytes = disk_peak_bytes.max(usage.bytes);
        disk_peak_objects = disk_peak_objects.max(usage.objects);
        let physical = physical_usage(directory.path());
        assert!(
            physical.0 <= disk_max_bytes,
            "physical swap files exceed byte cap"
        );
        assert!(
            physical.1 <= disk_max_objects,
            "physical swap files exceed object cap"
        );

        let held = root.try_reserve(root.limit()).unwrap();
        assert!(
            swap.try_get(&key, || false).is_err(),
            "memory pressure must refuse restore"
        );
        refusals += 1;
        drop(held);
        let restored = swap
            .try_get(&key, || false)
            .unwrap()
            .expect("pressure lost completed entry");
        let RasterPixels::Rgba8(values) = restored.0.pixels() else {
            panic!()
        };
        assert!(values.iter().all(|&v| v == value));
        assert_eq!((restored.0.width(), restored.0.height()), (256, 256));
        assert_eq!(restored.0.color_space(), &RasterColorSpace::Srgb);
        drop(restored);
        assert_eq!(root.used(), 0);
        assert!(root.peak() <= root.limit());
        assert!(queue.peak() <= queue.limit());
        cycles += 1;
    }
    swap.wait_idle().unwrap();
    let stats = swap.stats();
    assert!(cycles > 10);
    assert_eq!(stats.errors, 0);
    assert_eq!(stats.writes, cycles);
    assert_eq!(stats.reads, cycles);
    assert_eq!(stats.queued_bytes, 0);
    if disk_max_objects == 100 {
        assert_eq!(disk_peak_bytes, disk_max_bytes);
        assert_eq!(disk_peak_objects, 3);
    }
    drop(swap);
    assert_eq!(root.used(), 0);
    assert_eq!(queue.used(), 0);
    eprintln!(
        "sustained swap: seconds={} cycles={cycles} pressure_refusals={refusals} root_peak={} queue_peak={} disk_peak_bytes={disk_peak_bytes} disk_peak_objects={disk_peak_objects} final=0",
        start.elapsed().as_secs_f64(),
        root.peak(),
        queue.peak()
    );
}

#[test]
fn cancelled_delayed_restore_releases_partial_pixels_and_keeps_retryable_entry() {
    let directory = tempfile::tempdir().unwrap();
    let root = MemoryBudget::new(1024 * 1024);
    let queue = MemoryBudget::new(1024 * 1024);
    let swap: ImageSwapCache<u64, SlowRaster> = ImageSwapCache::new_with_budgets(
        directory.path(),
        ImageSwapConfig {
            limits: CacheLimits::bytes(1024 * 1024),
            queue_bytes: 1024 * 1024,
            queue_count: 2,
            restore_bytes: 512 * 1024,
        },
        queue.clone(),
        root.clone(),
    )
    .unwrap();
    for key in 0..16u64 {
        let value = key as u8;
        let raster = DecodedRaster::new(
            256,
            256,
            RasterPixels::Rgba8(Arc::new(vec![value; 256 * 256 * 4]).into()),
            RasterColorSpace::Srgb,
        )
        .unwrap()
        .try_manage_pixels(&root)
        .unwrap();
        swap.enqueue(key, SlowRaster(raster));
        swap.wait_idle().unwrap();
        let mut allocation_seen = false;
        assert!(
            swap.try_get(&key, || {
                allocation_seen |= root.used() > 0;
                allocation_seen
            })
            .unwrap()
            .is_none()
        );
        assert!(allocation_seen, "must cancel after pixel admission");
        assert_eq!(root.used(), 0);
        let restored = swap
            .try_get(&key, || false)
            .unwrap()
            .expect("cancelled read removed completed object");
        let RasterPixels::Rgba8(pixels) = restored.0.pixels() else {
            panic!()
        };
        assert!(pixels.iter().all(|&v| v == value));
        drop(restored);
        assert_eq!(root.used(), 0);
    }
    assert_eq!(swap.stats().errors, 0);
    assert_eq!(swap.stats().reads, 16);
    drop(swap);
    assert_eq!(queue.used(), 0);
    assert_eq!(root.used(), 0);
}

/// Pause a slow restore after pixel admission while it owns the restore lock.
/// A cancelled waiter must exit before that lock is released; the latest key
/// and the cancelled object's retry must then remain exact and budgeted.
#[test]
fn stale_slow_restore_and_cancelled_lock_waiter_release_credit_for_latest_key() {
    use std::sync::{
        atomic::{AtomicU64, Ordering},
        mpsc,
    };
    let directory = tempfile::tempdir().unwrap();
    let root = MemoryBudget::new(1024 * 1024);
    let queue = MemoryBudget::new(1024 * 1024);
    let swap: ImageSwapCache<u64, SlowRaster> = ImageSwapCache::new_with_budgets(
        directory.path(),
        ImageSwapConfig {
            limits: CacheLimits::bytes(1024 * 1024),
            queue_bytes: 1024 * 1024,
            queue_count: 2,
            restore_bytes: 512 * 1024,
        },
        queue.clone(),
        root.clone(),
    )
    .unwrap();
    for key in [1u64, 2] {
        let raster = DecodedRaster::new(
            256,
            256,
            RasterPixels::Rgba8(Arc::new(vec![key as u8; 256 * 256 * 4]).into()),
            RasterColorSpace::Srgb,
        )
        .unwrap()
        .try_manage_pixels(&root)
        .unwrap();
        swap.enqueue(key, SlowRaster(raster));
        swap.wait_idle().unwrap();
    }
    assert_eq!(root.used(), 0);
    for cycle in 0..8 {
        let generation = AtomicU64::new(0);
        let (entered_tx, entered_rx) = mpsc::channel();
        let (resume_tx, resume_rx) = mpsc::channel();
        std::thread::scope(|scope| {
            let root = &root;
            let swap = &swap;
            let generation = &generation;
            let old = scope.spawn(move || {
                let mut paused = false;
                swap.try_get(&1, || {
                    if !paused && root.used() > 0 {
                        paused = true;
                        entered_tx.send(()).unwrap();
                        // Timeout ensures a failing assertion cannot hang the test.
                        if resume_rx.recv_timeout(Duration::from_secs(5)).is_err() {
                            return true;
                        }
                    }
                    generation.load(Ordering::Acquire) != 0
                })
                .unwrap()
            });
            entered_rx.recv_timeout(Duration::from_secs(5)).unwrap();
            assert_eq!(root.used(), 256 * 256 * 4);
            let mut polls = 0;
            let waiter = swap
                .try_get(&2, || {
                    polls += 1;
                    polls >= 4
                })
                .unwrap();
            assert!(waiter.is_none(), "cancelled waiter published pixels: {cycle}");
            assert_eq!(polls, 4);
            assert_eq!(
                root.used(),
                256 * 256 * 4,
                "waiting cancellation allocated or discarded another owner's pixels"
            );
            generation.store(1, Ordering::Release);
            resume_tx.send(()).unwrap();
            assert!(old.join().unwrap().is_none());
        });
        assert_eq!(root.used(), 0);
        // The selected image and the stale object's later retry both remain intact.
        for key in [2u64, 1] {
            let restored = swap.try_get(&key, || false).unwrap().unwrap();
            let RasterPixels::Rgba8(pixels) = restored.0.pixels() else {
                panic!()
            };
            assert!(pixels.iter().all(|&sample| sample == key as u8));
            assert_eq!(root.used(), 256 * 256 * 4);
            drop(restored);
            assert_eq!(root.used(), 0);
        }
    }
    assert_eq!(swap.stats().writes, 2);
    assert_eq!(swap.stats().reads, 16);
    assert_eq!(swap.stats().errors, 0);
    assert_eq!(swap.stats().queued_bytes, 0);
    drop(swap);
    assert_eq!(queue.used(), 0);
    assert_eq!(root.used(), 0);
    println!(
        "slow restore contention: cycles=8 cancelled_waiters=8 stale_reads=8 successful_reads=16 root_peak={} final=0",
        root.peak()
    );
}

#[test]
fn idle_ttl_removes_delayed_large_blobs_and_physical_files() {
    let directory = tempfile::tempdir().unwrap();
    let root = MemoryBudget::new(1024 * 1024);
    let queue = MemoryBudget::new(1024 * 1024);
    let swap: ImageSwapCache<u64, SlowRaster> = ImageSwapCache::new_with_budgets(
        directory.path(),
        ImageSwapConfig {
            limits: CacheLimits {
                max_bytes: 8 * 1024 * 1024,
                max_entries: Some(100),
                ttl: Some(Duration::from_secs(2)),
            },
            queue_bytes: 1024 * 1024,
            queue_count: 2,
            restore_bytes: 512 * 1024,
        },
        queue.clone(),
        root.clone(),
    )
    .unwrap();
    for key in 0..8u64 {
        let raster = DecodedRaster::new(
            256,
            256,
            RasterPixels::Rgba8(Arc::new(vec![key as u8; 256 * 256 * 4]).into()),
            RasterColorSpace::Srgb,
        )
        .unwrap()
        .try_manage_pixels(&root)
        .unwrap();
        swap.enqueue(key, SlowRaster(raster));
        swap.wait_idle().unwrap();
    }
    assert_eq!(swap.stats().writes, 8);
    assert!(swap.disk_usage().unwrap().objects > 0);
    assert!(physical_usage(directory.path()).1 > 0);
    assert_eq!(root.used(), 0);
    assert_eq!(queue.used(), 0);
    let deadline = Instant::now() + Duration::from_secs(6);
    // No writes, restores or foreground prune call; the worker must reclaim TTL.
    while swap.disk_usage().unwrap().objects != 0 {
        assert!(
            Instant::now() < deadline,
            "idle TTL worker did not reclaim disk quota"
        );
        std::thread::sleep(Duration::from_millis(20));
    }
    // The quota reservation and temporary file have separate drop operations;
    // synchronize their completion before inspecting physical files.
    swap.wait_idle().unwrap();
    assert_eq!(swap.disk_usage().unwrap().bytes, 0);
    assert_eq!(physical_usage(directory.path()), (0, 0));
    assert_eq!(root.used(), 0);
    assert_eq!(queue.used(), 0);
}

#[test]
fn burst_overload_of_delayed_writer_is_bounded_and_rejected_keys_retry() {
    use rrrah_cache::SpillAdmission;
    for (queue_bytes, queue_count) in [(512 * 1024, 8), (2 * 1024 * 1024, 1)] {
        let directory = tempfile::tempdir().unwrap();
        let root = MemoryBudget::new(4 * 1024 * 1024);
        let queue = MemoryBudget::new(queue_bytes);
        let swap: ImageSwapCache<u64, SlowRaster> = ImageSwapCache::new_with_budgets(
            directory.path(),
            ImageSwapConfig {
                limits: CacheLimits::bytes(64 * 1024 * 1024),
                queue_bytes,
                queue_count,
                restore_bytes: 512 * 1024,
            },
            queue.clone(),
            root.clone(),
        )
        .unwrap();
        let make = |value| {
            SlowRaster(
                DecodedRaster::new(
                    256,
                    256,
                    RasterPixels::Rgba8(Arc::new(vec![value; 256 * 256 * 4]).into()),
                    RasterColorSpace::Srgb,
                )
                .unwrap()
                .try_manage_pixels(&root)
                .unwrap(),
            )
        };
        let mut accepted = 0u64;
        let mut rejected = 0u64;
        for batch in 0..16u64 {
            let mut keys = Vec::new();
            let mut retry = None;
            for offset in 0..64u64 {
                let key = batch * 64 + offset;
                match swap.try_enqueue(key, make((key % 251) as u8)) {
                    SpillAdmission::Queued => {
                        keys.push(key);
                        accepted += 1;
                    }
                    SpillAdmission::Full | SpillAdmission::Busy => {
                        rejected += 1;
                        retry.get_or_insert(key);
                    }
                    other => panic!("unexpected admission {other:?}"),
                }
                assert!(queue.used() <= queue_bytes);
                assert!(root.used() <= root.limit());
                if queue_count == 1 {
                    assert!(
                        root.used() <= 2 * 262144,
                        "waiting count retained too many payloads"
                    );
                }
            }
            swap.wait_idle().unwrap();
            assert_eq!(queue.used(), 0);
            assert_eq!(root.used(), 0);
            let retry = retry.expect("burst must exercise overload");
            assert_eq!(
                swap.try_enqueue(retry, make((retry % 251) as u8)),
                SpillAdmission::Queued
            );
            accepted += 1;
            keys.push(retry);
            swap.wait_idle().unwrap();
            for key in keys {
                let restored = swap
                    .try_get(&key, || false)
                    .unwrap()
                    .expect("accepted write disappeared");
                let RasterPixels::Rgba8(pixels) = restored.0.pixels() else {
                    panic!()
                };
                assert!(pixels.iter().all(|&v| v == (key % 251) as u8));
                drop(restored);
                assert_eq!(root.used(), 0);
            }
        }
        assert!(rejected > 0);
        assert_eq!(swap.stats().writes, accepted);
        assert_eq!(swap.stats().errors, 0);
        assert!(queue.peak() <= queue.limit());
        drop(swap);
        assert_eq!(root.used(), 0);
        assert_eq!(queue.used(), 0);
        eprintln!(
            "burst delayed swap: queue_bytes={queue_bytes} queue_count={queue_count} accepted={accepted} rejected={rejected} root_peak={} queue_peak={} final=0",
            root.peak(),
            queue.peak()
        );
    }
}

#[test]
#[ignore = "manual 100000 distinct-key real-file churn; small payloads isolate index retention"]
fn hundred_thousand_distinct_rasters_keep_swap_index_and_memory_bounded() {
    let directory = tempfile::tempdir().unwrap();
    let root = MemoryBudget::new(128 * 1024);
    let queue = MemoryBudget::new(4096);
    let swap: ImageSwapCache<u64, DecodedRaster> = ImageSwapCache::new_with_budgets(
        directory.path(),
        ImageSwapConfig {
            limits: CacheLimits {
                max_bytes: 4096,
                max_entries: Some(17),
                ttl: None,
            },
            queue_bytes: 4096,
            queue_count: 4,
            restore_bytes: 4096,
        },
        queue.clone(),
        root.clone(),
    )
    .unwrap();
    let start = Instant::now();
    let mut pressure_refusals = 0;
    for key in 0..100_000u64 {
        // Every key has its own four-byte identity, preventing stale-key aliases.
        let expected = (key as u32).to_le_bytes();
        let raster = DecodedRaster::new(
            1,
            1,
            RasterPixels::Rgba8(Arc::new(expected.to_vec()).into()),
            RasterColorSpace::Srgb,
        )
        .unwrap()
        .try_manage_pixels(&root)
        .unwrap();
        swap.enqueue(key, raster);
        swap.wait_idle().unwrap();
        assert_eq!(queue.used(), 0);
        assert_eq!(root.used(), 0);
        if key % 100 == 0 {
            let held = root.try_reserve(root.limit()).unwrap();
            assert!(swap.try_get(&key, || false).is_err());
            pressure_refusals += 1;
            drop(held);
        }
        let restored = swap.try_get(&key, || false).unwrap().expect("completed key lost");
        let RasterPixels::Rgba8(pixels) = restored.pixels() else {
            panic!()
        };
        assert_eq!(&pixels[..], &expected);
        drop(restored);
        let usage = swap.disk_usage().unwrap();
        assert!(usage.objects <= 17 && usage.bytes <= 4096);
        if key >= 17 {
            assert!(
                swap.try_get(&(key - 17), || false).unwrap().is_none(),
                "evicted key retained"
            );
        }
        if key % 1000 == 999 {
            let physical = physical_usage(directory.path());
            assert!(physical.0 <= 4096 && physical.1 <= 17);
        }
        assert_eq!(root.used(), 0);
    }
    let stats = swap.stats();
    assert_eq!(stats.errors, 0);
    assert_eq!(stats.writes, 100_000);
    assert_eq!(stats.reads, 100_000);
    assert_eq!(stats.queued_bytes, 0);
    drop(swap);
    assert_eq!(root.used(), 0);
    assert_eq!(queue.used(), 0);
    eprintln!(
        "100k swap: seconds={} writes=100000 reads=100000 pressure_refusals={pressure_refusals} root_peak={} queue_peak={} final=0",
        start.elapsed().as_secs_f64(),
        root.peak(),
        queue.peak()
    );
}
