//! Bounded write-back persistence for foreground RAW decodes.
//!
//! Decoded pixels are reference counted by `DecodedMosaic`, so enqueueing a
//! write clones metadata and a shared pixel buffer, not the full sensor buffer.
//! The one-slot queue is latest-wins: rapid navigation retains at most one
//! active write and one pending mosaic, and stale jobs are normally discarded
//! during the quiet-period debounce before they touch disk.

use crate::cache_telemetry::CacheTelemetry;
use crossbeam_channel::{Receiver, Sender, bounded};
use rrrah_cache::{CacheKey, DiskMosaicCache, MemoryBudget, Reservation};
use rrrah_core::DecodedMosaic;
use std::{
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
    thread,
    time::{Duration, Instant},
};

const WRITE_DEBOUNCE: Duration = Duration::from_millis(150);
const GENERATION_POLL: Duration = Duration::from_millis(10);

#[derive(Debug)]
struct CacheWriteJob {
    generation: u64,
    key: CacheKey,
    mosaic: DecodedMosaic,
    _reservation: Reservation,
}

/// A single asynchronous disk-cache writer with a one-job pending queue.
#[derive(Debug)]
pub struct CacheWriter {
    tx: Sender<CacheWriteJob>,
    /// The producer may remove an obsolete queued job before publishing the
    /// newest generation. The worker is the only other receiver.
    pending: Receiver<CacheWriteJob>,
    generation: Arc<AtomicU64>,
    telemetry: Option<Arc<CacheTelemetry>>,
    budget: MemoryBudget,
}

impl CacheWriter {
    #[cfg(test)]
    pub fn spawn(
        cache: DiskMosaicCache,
        generation: Arc<AtomicU64>,
        telemetry: Arc<CacheTelemetry>,
        bytes: u64,
    ) -> std::io::Result<Self> {
        Self::spawn_with_budget(cache, generation, telemetry, MemoryBudget::new(bytes))
    }
    pub fn spawn_with_budget(
        cache: DiskMosaicCache,
        generation: Arc<AtomicU64>,
        telemetry: Arc<CacheTelemetry>,
        budget: MemoryBudget,
    ) -> std::io::Result<Self> {
        let store_telemetry = Arc::clone(&telemetry);
        let store_generation = Arc::clone(&generation);
        Self::spawn_with_store_and_telemetry(
            generation,
            WRITE_DEBOUNCE,
            Some(telemetry),
            budget,
            move |expected, key, mosaic| match cache.store_with_cancel(key, mosaic, || {
                store_generation.load(Ordering::Acquire) != expected
            }) {
                Ok(_) => {
                    match cache.usage() {
                        Ok(usage) => store_telemetry.update_disk_usage(usage),
                        Err(_) => store_telemetry.record_disk_scan_error(),
                    }
                    Some(true)
                }
                Err(rrrah_cache::CacheError::Cancelled) => None,
                Err(error) => {
                    log::warn!("decoded RAW is usable but async cache write {key} failed: {error}");
                    Some(false)
                }
            },
        )
    }

    #[cfg(test)]
    fn spawn_with_store<F>(generation: Arc<AtomicU64>, debounce: Duration, store: F) -> std::io::Result<Self>
    where
        F: Fn(CacheKey, &DecodedMosaic) + Send + 'static,
    {
        Self::spawn_with_store_and_telemetry(
            generation,
            debounce,
            None,
            MemoryBudget::new(128 * 1024 * 1024),
            move |_, key, mosaic| {
                store(key, mosaic);
                Some(true)
            },
        )
    }

    fn spawn_with_store_and_telemetry<F>(
        generation: Arc<AtomicU64>,
        debounce: Duration,
        telemetry: Option<Arc<CacheTelemetry>>,
        budget: MemoryBudget,
        store: F,
    ) -> std::io::Result<Self>
    where
        F: Fn(u64, CacheKey, &DecodedMosaic) -> Option<bool> + Send + 'static,
    {
        let (tx, jobs) = bounded::<CacheWriteJob>(1);
        let pending = jobs.clone();
        let worker_generation = Arc::clone(&generation);
        let worker_telemetry = telemetry.clone();
        thread::Builder::new()
            .name("rrrah-cache-write".into())
            .spawn(move || {
                while let Ok(job) = jobs.recv() {
                    let bytes = u64::try_from(job.mosaic.byte_len()).unwrap_or(u64::MAX);
                    if let Some(telemetry) = &worker_telemetry {
                        telemetry.writer_dequeued(bytes);
                    }
                    if !wait_for_current_generation(&worker_generation, job.generation, debounce) {
                        if let Some(telemetry) = &worker_telemetry {
                            telemetry.writer_cancelled();
                        }
                        continue;
                    }
                    if let Some(telemetry) = &worker_telemetry {
                        telemetry.writer_started();
                    }
                    let started = Instant::now();
                    let success = store(job.generation, job.key, &job.mosaic);
                    if let Some(telemetry) = &worker_telemetry {
                        match success {
                            Some(success) => telemetry.writer_finished(bytes, started.elapsed(), success),
                            None => telemetry.writer_cancelled(),
                        }
                    }
                }
            })?;
        Ok(Self {
            tx,
            pending,
            generation,
            telemetry,
            budget,
        })
    }

    /// Replace pending persistence with the newest visible frame. An already
    /// active store checks generation cancellation; it never holds the decode
    /// permit and therefore cannot make foreground wait for `fsync`.
    pub fn submit(&self, generation: u64, key: CacheKey, mosaic: DecodedMosaic) -> bool {
        if self.generation.load(Ordering::Acquire) != generation {
            return false;
        }
        while self.pending.try_recv().is_ok() {
            if let Some(telemetry) = &self.telemetry {
                telemetry.writer_superseded();
            }
        }
        let capacity = mosaic.pixels.capacity_bytes();
        let Ok(reservation) = self.budget.try_reserve(capacity) else {
            return false;
        };
        let bytes = u64::try_from(mosaic.byte_len()).unwrap_or(u64::MAX);
        if let Some(telemetry) = &self.telemetry {
            telemetry.writer_queued(bytes);
        }
        let submitted = self
            .tx
            .try_send(CacheWriteJob {
                generation,
                key,
                mosaic,
                _reservation: reservation,
            })
            .is_ok();
        if !submitted {
            if let Some(telemetry) = &self.telemetry {
                telemetry.writer_superseded();
            }
        }
        submitted
    }
}

fn wait_for_current_generation(generation: &AtomicU64, expected: u64, debounce: Duration) -> bool {
    let deadline = Instant::now() + debounce;
    loop {
        if generation.load(Ordering::Acquire) != expected {
            return false;
        }
        let Some(remaining) = deadline.checked_duration_since(Instant::now()) else {
            return true;
        };
        thread::sleep(remaining.min(GENERATION_POLL));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crossbeam_channel::bounded;
    use rrrah_cache::SourceFingerprint;
    use rrrah_core::{CfaColor, CfaPattern, LevelGrid, Orientation, Photometric, RawMetadata, WhiteLevel};

    fn key(tag: u8) -> CacheKey {
        CacheKey::for_mosaic(
            &SourceFingerprint {
                file_size: u64::from(tag),
                modified_ns: u128::from(tag),
                sampled_blake3: [tag; 32],
            },
            0,
        )
    }

    fn mosaic(tag: u16) -> DecodedMosaic {
        let metadata = RawMetadata {
            make: "Test".into(),
            model: "CacheWriter".into(),
            width: 2,
            height: 2,
            components_per_pixel: 1,
            bits_per_sample: 14,
            photometric: Photometric::Cfa,
            cfa: Some(CfaPattern {
                width: 2,
                height: 2,
                cells: vec![CfaColor::Red, CfaColor::Green, CfaColor::Green, CfaColor::Blue],
            }),
            black_level: LevelGrid {
                width: 1,
                height: 1,
                components: 1,
                values: vec![0.0],
            },
            white_level: WhiteLevel(vec![16_383.0]),
            white_balance: [1.0; 4],
            xyz_to_camera: [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0], [0.0; 3]],
            active_area: None,
            crop_area: None,
            orientation: Orientation::Normal,
        };
        DecodedMosaic::new(metadata, Arc::new(vec![tag; 4])).unwrap()
    }

    fn recv_test<T>(receiver: &Receiver<T>) -> T {
        receiver
            .recv_timeout(Duration::from_secs(2))
            .expect("cache writer test timed out")
    }

    #[test]
    fn queued_write_is_latest_wins_while_active_store_finishes() {
        let generation = Arc::new(AtomicU64::new(0));
        let (started_tx, started_rx) = bounded(2);
        let (release_tx, release_rx) = bounded(0);
        let (stored_tx, stored_rx) = bounded(2);
        let writer =
            CacheWriter::spawn_with_store(Arc::clone(&generation), Duration::ZERO, move |_key, mosaic| {
                let tag = mosaic.pixels[0];
                started_tx.send(tag).unwrap();
                if tag == 0 {
                    release_rx.recv().unwrap();
                }
                stored_tx.send(tag).unwrap();
            })
            .unwrap();

        assert!(writer.submit(0, key(0), mosaic(0)));
        assert_eq!(recv_test(&started_rx), 0);
        generation.store(1, Ordering::Release);
        assert!(writer.submit(1, key(1), mosaic(1)));
        generation.store(2, Ordering::Release);
        assert!(writer.submit(2, key(2), mosaic(2)));
        release_tx.send(()).unwrap();

        assert_eq!(recv_test(&stored_rx), 0);
        assert_eq!(recv_test(&started_rx), 2);
        assert_eq!(recv_test(&stored_rx), 2);
        assert!(started_rx.try_recv().is_err());
    }

    #[test]
    fn debounce_discards_generation_superseded_before_store() {
        let generation = Arc::new(AtomicU64::new(0));
        let (stored_tx, stored_rx) = bounded(2);
        let writer = CacheWriter::spawn_with_store(
            Arc::clone(&generation),
            Duration::from_millis(60),
            move |_key, mosaic| stored_tx.send(mosaic.pixels[0]).unwrap(),
        )
        .unwrap();

        assert!(writer.submit(0, key(0), mosaic(0)));
        let deadline = Instant::now() + Duration::from_secs(1);
        while !writer.pending.is_empty() && Instant::now() < deadline {
            thread::yield_now();
        }
        assert!(writer.pending.is_empty(), "worker did not dequeue first job");

        generation.store(1, Ordering::Release);
        assert!(writer.submit(1, key(1), mosaic(1)));
        assert_eq!(recv_test(&stored_rx), 1);
        assert!(stored_rx.try_recv().is_err());
    }

    #[test]
    fn enqueue_reuses_reference_counted_sensor_pixels() {
        let original = mosaic(7);
        let clone = original.clone();
        assert!(original.pixels.ptr_eq(&clone.pixels));
    }
    #[test]
    fn writer_budget_covers_active_jobs_and_rejects_without_pixel_copy() {
        let generation = Arc::new(AtomicU64::new(1));
        let root = MemoryBudget::new(8);
        let budget = root.child(16);
        let (entered_tx, entered_rx) = std::sync::mpsc::channel();
        let (release_tx, release_rx) = std::sync::mpsc::channel();
        let release_rx = std::sync::Mutex::new(release_rx);
        let writer = CacheWriter::spawn_with_store_and_telemetry(
            generation,
            Duration::ZERO,
            None,
            budget.clone(),
            move |_, _, _| {
                entered_tx.send(()).unwrap();
                release_rx.lock().unwrap().recv().unwrap();
                Some(true)
            },
        )
        .unwrap();
        assert!(writer.submit(1, key(1), mosaic(1)));
        entered_rx.recv_timeout(Duration::from_secs(2)).unwrap();
        assert_eq!(budget.used(), 8);
        assert_eq!(root.used(), 8);
        assert!(root.child(8).try_reserve(1).is_err());
        assert!(!writer.submit(1, key(2), mosaic(2)));
        assert_eq!(budget.used(), 8);
        release_tx.send(()).unwrap();
        drop(writer);
        let deadline = Instant::now() + Duration::from_secs(2);
        // Child and ancestor counters are released in sequence; await both.
        while (budget.used() != 0 || root.used() != 0) && Instant::now() < deadline {
            std::thread::yield_now();
        }
        assert_eq!(budget.used(), 0);
        assert_eq!(root.used(), 0);
    }
    #[test]
    fn generation_change_cancels_real_store_waiting_on_disk_lock() {
        let root = tempfile::tempdir().unwrap();
        let cache = DiskMosaicCache::new(root.path());
        let lock = std::fs::OpenOptions::new()
            .create(true)
            .truncate(false)
            .write(true)
            .open(root.path().join(".rrrah-cache-write.lock"))
            .unwrap();
        lock.lock().unwrap();
        let generation = Arc::new(AtomicU64::new(1));
        let telemetry = Arc::new(CacheTelemetry::new(true, cache.max_bytes()));
        let writer = CacheWriter::spawn(cache.clone(), generation.clone(), telemetry.clone(), 8).unwrap();
        assert!(writer.submit(1, key(1), mosaic(1)));
        let deadline = Instant::now() + Duration::from_secs(2);
        while telemetry.snapshot().writer_phase != crate::cache_telemetry::WriterPhase::Writing
            && Instant::now() < deadline
        {
            std::thread::sleep(Duration::from_millis(1));
        }
        assert_eq!(
            telemetry.snapshot().writer_phase,
            crate::cache_telemetry::WriterPhase::Writing
        );
        generation.store(2, Ordering::Release);
        let deadline = Instant::now() + Duration::from_secs(2);
        while writer.budget.used() != 0 && Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(1));
        }
        assert_eq!(writer.budget.used(), 0);
        assert!(!cache.contains(key(1)));
        let snapshot = telemetry.snapshot();
        assert_eq!(snapshot.writes_superseded, 1);
        assert_eq!(snapshot.write_failures, 0);
        drop(lock);
        assert!(writer.submit(2, key(2), mosaic(2)));
        let deadline = Instant::now() + Duration::from_secs(2);
        while telemetry.snapshot().writes_committed == 0 && Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(1));
        }
        assert_eq!(telemetry.snapshot().writes_committed, 1);
        assert!(cache.contains(key(2)));
    }
}
