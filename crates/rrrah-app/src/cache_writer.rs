//! Bounded write-back persistence for foreground RAW decodes.
//!
//! Decoded pixels are reference counted by `DecodedMosaic`, so enqueueing a
//! write clones metadata and an `Arc<Vec<u16>>`, not the full sensor buffer.
//! The one-slot queue is latest-wins: rapid navigation retains at most one
//! active write and one pending mosaic, and stale jobs are normally discarded
//! during the quiet-period debounce before they touch disk.

use crossbeam_channel::{Receiver, Sender, bounded};
use rrrah_cache::{CacheKey, DiskMosaicCache};
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
}

/// A single asynchronous disk-cache writer with a one-job pending queue.
#[derive(Debug)]
pub struct CacheWriter {
    tx: Sender<CacheWriteJob>,
    /// The producer may remove an obsolete queued job before publishing the
    /// newest generation. The worker is the only other receiver.
    pending: Receiver<CacheWriteJob>,
    generation: Arc<AtomicU64>,
}

impl CacheWriter {
    pub fn spawn(cache: DiskMosaicCache, generation: Arc<AtomicU64>) -> std::io::Result<Self> {
        Self::spawn_with_store(generation, WRITE_DEBOUNCE, move |key, mosaic| {
            if let Err(error) = cache.store(key, mosaic) {
                log::warn!("decoded RAW is usable but async cache write {key} failed: {error}");
            }
        })
    }

    fn spawn_with_store<F>(generation: Arc<AtomicU64>, debounce: Duration, store: F) -> std::io::Result<Self>
    where
        F: Fn(CacheKey, &DecodedMosaic) + Send + 'static,
    {
        let (tx, jobs) = bounded::<CacheWriteJob>(1);
        let pending = jobs.clone();
        let worker_generation = Arc::clone(&generation);
        thread::Builder::new()
            .name("rrrah-cache-write".into())
            .spawn(move || {
                while let Ok(job) = jobs.recv() {
                    if !wait_for_current_generation(&worker_generation, job.generation, debounce) {
                        continue;
                    }
                    store(job.key, &job.mosaic);
                }
            })?;
        Ok(Self {
            tx,
            pending,
            generation,
        })
    }

    /// Replace pending persistence with the newest visible frame. An already
    /// active atomic store is allowed to finish, but it never holds the decode
    /// permit and therefore cannot make foreground wait for `fsync`.
    pub fn submit(&self, generation: u64, key: CacheKey, mosaic: DecodedMosaic) -> bool {
        if self.generation.load(Ordering::Acquire) != generation {
            return false;
        }
        while self.pending.try_recv().is_ok() {}
        self.tx
            .try_send(CacheWriteJob {
                generation,
                key,
                mosaic,
            })
            .is_ok()
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
        assert!(Arc::ptr_eq(&original.pixels, &clone.pixels));
    }
}
