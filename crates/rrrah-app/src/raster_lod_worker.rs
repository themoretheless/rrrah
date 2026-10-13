//! One background filter or verified swap-region restore, one replaceable pending request, one completed result.
use rrrah_core::{DecodedRaster, MemoryBudget};
use rrrah_decode::GenerationToken;
use std::sync::{
    Arc, Condvar, Mutex,
    atomic::{AtomicBool, AtomicU64, Ordering},
};

enum Source {
    Filter(DecodedRaster),
    SwapLod {
        source: rrrah_cache::SwappedRaster,
        scratch: MemoryBudget,
    },
    Persist {
        source: DecodedRaster,
        store: rrrah_swap::SwapStore,
    },
    Region {
        source: rrrah_cache::SwappedRaster,
        rect: [u32; 4],
        scratch: MemoryBudget,
    },
}
type WorkerError = Box<dyn std::error::Error + Send + Sync>;

struct Job {
    id: u64,
    generation: u64,
    source: Source,
    size: [u32; 2],
    budget: MemoryBudget,
    token: GenerationToken,
}
enum Output {
    Frame(DecodedRaster),
    Swapped(rrrah_cache::SwappedRaster),
}
pub(crate) struct Completed {
    pub id: u64,
    pub generation: u64,
    result: Result<Output, WorkerError>,
    token: GenerationToken,
}
impl Completed {
    pub fn into_frame(self) -> Result<DecodedRaster, WorkerError> {
        match self.result? {
            Output::Frame(frame) => Ok(frame),
            _ => Err(std::io::Error::other("expected raster worker output").into()),
        }
    }
    pub fn into_swapped(self) -> Result<rrrah_cache::SwappedRaster, WorkerError> {
        match self.result? {
            Output::Swapped(source) => Ok(source),
            _ => Err(std::io::Error::other("expected swap worker output").into()),
        }
    }
}
#[derive(Default)]
struct Slots {
    pending: Option<Job>,
    completed: Option<Completed>,
}
#[derive(Default)]
struct Shared {
    slots: Mutex<Slots>,
    ready: Condvar,
    revision: AtomicU64,
    stop: AtomicBool,
}
pub(crate) struct LodWorker {
    shared: Arc<Shared>,
    wake: Arc<dyn Fn() + Send + Sync>,
    thread: Option<std::thread::JoinHandle<()>>,
    #[cfg(test)]
    pub(crate) before_filter: Option<Arc<dyn Fn(u64) + Send + Sync>>,
}
impl Default for LodWorker {
    fn default() -> Self {
        Self {
            shared: Arc::new(Shared::default()),
            wake: Arc::new(|| {}),
            thread: None,
            #[cfg(test)]
            before_filter: None,
        }
    }
}
impl LodWorker {
    pub fn with_same_waker(&self) -> Self {
        let mut worker = Self::default();
        worker.wake = self.wake.clone();
        worker
    }
    pub fn set_waker(&mut self, wake: impl Fn() + Send + Sync + 'static) {
        assert!(
            self.thread.is_none(),
            "waker must be configured before first submission"
        );
        self.wake = Arc::new(wake);
    }
    pub fn submit(
        &mut self,
        source: DecodedRaster,
        size: [u32; 2],
        budget: MemoryBudget,
        generation: u64,
        token: GenerationToken,
    ) -> std::io::Result<u64> {
        self.submit_source(Source::Filter(source), size, budget, generation, token)
    }

    pub fn submit_swap(
        &mut self,
        source: DecodedRaster,
        store: rrrah_swap::SwapStore,
        index: MemoryBudget,
        generation: u64,
        token: GenerationToken,
    ) -> std::io::Result<u64> {
        let size = [source.width(), source.height()];
        self.submit_source(Source::Persist { source, store }, size, index, generation, token)
    }

    pub fn submit_swap_lod(
        &mut self,
        source: rrrah_cache::SwappedRaster,
        size: [u32; 2],
        budget: MemoryBudget,
        scratch: MemoryBudget,
        generation: u64,
        token: GenerationToken,
    ) -> std::io::Result<u64> {
        self.submit_source(
            Source::SwapLod { source, scratch },
            size,
            budget,
            generation,
            token,
        )
    }

    pub fn submit_region(
        &mut self,
        source: rrrah_cache::SwappedRaster,
        rect: [u32; 4],
        budget: MemoryBudget,
        scratch: MemoryBudget,
        generation: u64,
        token: GenerationToken,
    ) -> std::io::Result<u64> {
        self.submit_source(
            Source::Region {
                source,
                rect,
                scratch,
            },
            [rect[2], rect[3]],
            budget,
            generation,
            token,
        )
    }

    fn submit_source(
        &mut self,
        source: Source,
        size: [u32; 2],
        budget: MemoryBudget,
        generation: u64,
        token: GenerationToken,
    ) -> std::io::Result<u64> {
        if token.is_cancelled() {
            return Err(std::io::Error::new(std::io::ErrorKind::Interrupted,
                "obsolete raster worker request"));
        }
        if self.thread.is_none() {
            let shared = self.shared.clone();
            let wake = self.wake.clone();
            #[cfg(test)]
            let before_filter = self.before_filter.clone();
            self.thread = Some(std::thread::Builder::new().name("rrrah-hdr-lod".into()).spawn(
                move || loop {
                    let job = {
                        let mut slots = shared
                            .slots
                            .lock()
                            .unwrap_or_else(std::sync::PoisonError::into_inner);
                        while slots.pending.is_none() && !shared.stop.load(Ordering::Acquire) {
                            slots = shared
                                .ready
                                .wait(slots)
                                .unwrap_or_else(std::sync::PoisonError::into_inner);
                        }
                        if shared.stop.load(Ordering::Acquire) {
                            break;
                        }
                        slots.pending.take().unwrap()
                    };
                    #[cfg(test)]
                    if let Some(hook) = &before_filter {
                        hook(job.id);
                    }
                    let mut cancelled = || {
                        shared.stop.load(Ordering::Acquire)
                            || shared.revision.load(Ordering::Acquire) != job.id
                            || job.token.is_cancelled()
                    };
                    let result: Result<Output, WorkerError> = match &job.source {
                        Source::Filter(source) => source
                            .downsample_linear_with_budget_and_cancel(job.size, &job.budget, &mut cancelled)
                            .map(Output::Frame)
                            .map_err(|e| Box::new(e) as WorkerError),
                        Source::SwapLod { source, scratch } => source
                            .downsample_linear(job.size, &job.budget, scratch, &mut cancelled)
                            .map(Output::Frame)
                            .map_err(|e| Box::new(e) as WorkerError),
                        Source::Persist { source, store } => {
                            rrrah_cache::SwappedRaster::write(store, source, &job.budget, &mut cancelled)
                                .map(Output::Swapped)
                                .map_err(|e| Box::new(e) as WorkerError)
                        }
                        Source::Region {
                            source,
                            rect,
                            scratch,
                        } => source
                            .restore_region(*rect, &job.budget, scratch, &mut cancelled)
                            .map(|region| Output::Frame(region.frame))
                            .map_err(|e| Box::new(e) as WorkerError),
                    };
                    // Input ownership must end before wake-up, not at the next loop turn.
                    drop(job.source);
                    log::debug!(
                        "HDR level prepared: request={} generation={} size={:?} thread={:?}",
                        job.id,
                        job.generation,
                        job.size,
                        std::thread::current().name()
                    );
                    let published = {
                        let mut slots = shared
                            .slots
                            .lock()
                            .unwrap_or_else(std::sync::PoisonError::into_inner);
                        if cancelled() {
                            false
                        } else {
                            slots.completed = Some(Completed {
                                id: job.id,
                                generation: job.generation,
                                result,
                                token: job.token.clone(),
                            });
                            true
                        }
                    };
                    if published {
                        wake();
                    }
                },
            )?);
        }
        let mut slots = self
            .shared
            .slots
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        // Recheck after acquiring the queue lock: an old request must not
        // invalidate current work or discard its already prepared allocation.
        if token.is_cancelled() {
            return Err(std::io::Error::new(std::io::ErrorKind::Interrupted,
                "obsolete raster worker request"));
        }
        let id = self.shared.revision.fetch_add(1, Ordering::AcqRel) + 1;
        slots.completed = None;
        slots.pending = Some(Job {
            id,
            generation,
            source,
            size,
            budget,
            token,
        });
        self.shared.ready.notify_one();
        Ok(id)
    }
    pub fn try_take(&self) -> Option<Completed> {
        let completed = self
            .shared
            .slots
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .completed
            .take()?;
        // A generation can change after publication while the UI is asleep.
        // Drop its owned level here instead of handing it to the upload path.
        if completed.token.is_cancelled() {
            None
        } else {
            Some(completed)
        }
    }
    pub fn has_completed(&self) -> bool {
        self.shared
            .slots
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .completed
            .is_some()
    }
    pub fn cancel(&self) {
        let mut slots = self
            .shared
            .slots
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        self.shared.revision.fetch_add(1, Ordering::AcqRel);
        slots.pending = None;
        slots.completed = None;
    }
}
impl Drop for LodWorker {
    fn drop(&mut self) {
        self.shared.stop.store(true, Ordering::Release);
        self.cancel();
        self.shared.ready.notify_one();
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rrrah_core::{RasterColorSpace, RasterPixels};
    #[test]
    #[ignore = "actual Metal navigation while old swapped region worker is delayed"]
    fn navigation_discards_delayed_old_swap_and_presents_only_new_hdr_source() {
        let gpu = crate::navigation_metal_readback::qualification_gpu().expect("actual GPU required");
        let directory = tempfile::tempdir().unwrap();
        let store = rrrah_swap::SwapStore::new(
            directory.path(),
            rrrah_swap::SwapLimits {
                max_bytes: 200000,
                max_objects: Some(2),
            },
        )
        .unwrap();
        let cpu = MemoryBudget::new(2 * 64 * 64 * 16);
        let frame = |red| {
            DecodedRaster::new(
                64,
                64,
                RasterPixels::Rgba32Float(Arc::new([red, -0.5, 0.25, 1.].repeat(64 * 64)).into()),
                RasterColorSpace::LinearSrgb,
            )
            .unwrap()
            .try_manage_pixels(&cpu)
            .unwrap()
        };
        let old = frame(2.);
        let new = frame(8.);
        let view = rrrah_gpu::ViewParameters {
            viewport: [64.; 2],
            zoom: 2.,
            ..Default::default()
        };
        let expected = gpu.render_raster_half(&new, view, [64, 64]);
        let old_index = MemoryBudget::new(1024);
        let new_index = MemoryBudget::new(1024);
        let old_source = rrrah_cache::SwappedRaster::write(&store, &old, &old_index, || false).unwrap();
        let new_source = rrrah_cache::SwappedRaster::write(&store, &new, &new_index, || false).unwrap();
        drop(old);
        drop(new);
        assert_eq!(cpu.used(), 0);
        let output = MemoryBudget::new(200000);
        let scratch = output.child(65536);
        let generation = Arc::new(AtomicU64::new(7));
        let (entered_tx, entered_rx) = crossbeam_channel::bounded(1);
        let (resume_tx, resume_rx) = crossbeam_channel::bounded(1);
        let (wake_tx, wake_rx) = crossbeam_channel::bounded(1);
        let mut worker = LodWorker::default();
        worker.before_filter = Some(Arc::new(move |id| {
            if id == 1 {
                entered_tx.send(()).unwrap();
                resume_rx
                    .recv_timeout(std::time::Duration::from_secs(10))
                    .unwrap();
            }
        }));
        worker.set_waker(move || {
            let _ = wake_tx.try_send(());
        });
        worker
            .submit_region(
                old_source,
                [0, 0, 64, 64],
                output.clone(),
                scratch.clone(),
                7,
                GenerationToken::new(generation.clone(), 7),
            )
            .unwrap();
        entered_rx
            .recv_timeout(std::time::Duration::from_secs(10))
            .unwrap();
        assert_eq!(
            store.usage().unwrap().objects,
            2,
            "active old reader still owns its file"
        );
        generation.store(8, Ordering::Release);
        let latest = worker
            .submit_region(
                new_source.clone(),
                [0, 0, 64, 64],
                output.clone(),
                scratch.clone(),
                8,
                GenerationToken::new(generation, 8),
            )
            .unwrap();
        resume_tx.send(()).unwrap();
        wake_rx.recv_timeout(std::time::Duration::from_secs(10)).unwrap();
        let completed = worker.try_take().unwrap();
        assert_eq!((completed.id, completed.generation), (latest, 8));
        assert_eq!((old_index.used(), store.usage().unwrap().objects), (0, 1));
        let crop = completed.into_frame().unwrap();
        let texture = MemoryBudget::new(200000);
        let queued = MemoryBudget::new(200000);
        let mut renderer =
            rrrah_gpu::RasterRenderer::new_linear_hdr_with_budget(&gpu.device, texture.clone())
                .with_upload_queue_budget(queued.clone());
        renderer
            .upload_packed_region_with_cancel(&gpu.device, &gpu.queue, &crop, [64, 64], [0, 0], || false)
            .unwrap();
        renderer.update_view(&gpu.queue, view);
        assert_eq!(gpu.render_existing_raster_half(&renderer, [64, 64]), expected);
        assert!(
            wake_rx.try_recv().is_err(),
            "cancelled old source must not publish a wake/result"
        );
        drop(crop);
        drop(worker);
        drop(new_source);
        drop(renderer);
        assert_eq!(
            (
                cpu.used(),
                output.used(),
                scratch.used(),
                old_index.used(),
                new_index.used(),
                texture.used(),
                queued.used(),
                store.usage().unwrap().bytes
            ),
            (0, 0, 0, 0, 0, 0, 0, 0)
        );
        eprintln!(
            "delayed swap navigation: old file/index released before latest wake; only new HDR source renders; final credits=0"
        );
    }

    #[test]
    fn stale_persisted_source_releases_disk_index_and_input_before_consumption() {
        let directory = tempfile::tempdir().unwrap();
        let store = rrrah_swap::SwapStore::new(
            directory.path(),
            rrrah_swap::SwapLimits {
                max_bytes: 100000,
                max_objects: Some(1),
            },
        )
        .unwrap();
        let cpu = MemoryBudget::new(64 * 64 * 16);
        let source = DecodedRaster::new(
            64,
            64,
            RasterPixels::Rgba32Float(Arc::new([8., -0.5, 0.25, 1.].repeat(64 * 64)).into()),
            RasterColorSpace::LinearSrgb,
        )
        .unwrap()
        .try_manage_pixels(&cpu)
        .unwrap();
        let index = MemoryBudget::new(1024);
        let generation = Arc::new(AtomicU64::new(7));
        let (tx, rx) = crossbeam_channel::bounded(1);
        let mut worker = LodWorker::default();
        worker.set_waker(move || {
            let _ = tx.try_send(());
        });
        worker
            .submit_swap(
                source,
                store.clone(),
                index.clone(),
                7,
                GenerationToken::new(generation.clone(), 7),
            )
            .unwrap();
        rx.recv_timeout(std::time::Duration::from_secs(10)).unwrap();
        assert_eq!(cpu.used(), 0, "worker releases full pixels before wake");
        assert_eq!(store.usage().unwrap().objects, 1);
        assert_eq!(index.used(), 64);
        generation.store(8, Ordering::Release);
        assert!(worker.try_take().is_none());
        assert_eq!((index.used(), store.usage().unwrap().bytes), (0, 0));
        drop(worker);
    }

    #[test]
    fn swapped_regions_coalesce_drop_stale_output_and_retry_memory_pressure() {
        let directory = tempfile::tempdir().unwrap();
        let store = rrrah_swap::SwapStore::new(
            directory.path(),
            rrrah_swap::SwapLimits {
                max_bytes: 2_000_000,
                max_objects: Some(1),
            },
        )
        .unwrap();
        let cpu = MemoryBudget::new(256 * 256 * 16);
        let frame = DecodedRaster::new(
            256,
            256,
            RasterPixels::Rgba32Float(
                Arc::new(
                    (0..256 * 256)
                        .flat_map(|i| [i as f32, -0.5, 8., 1.])
                        .collect::<Vec<_>>(),
                )
                .into(),
            ),
            RasterColorSpace::LinearSrgb,
        )
        .unwrap()
        .try_manage_pixels(&cpu)
        .unwrap();
        let index = MemoryBudget::new(1024);
        let source = rrrah_cache::SwappedRaster::write(&store, &frame, &index, || false).unwrap();
        drop(frame);
        assert_eq!(cpu.used(), 0);
        let budget = MemoryBudget::new(100000);
        let scratch = budget.child(65536);
        let generation = Arc::new(AtomicU64::new(7));
        let token = || GenerationToken::new(generation.clone(), generation.load(Ordering::Acquire));
        let (started_tx, started_rx) = crossbeam_channel::bounded(1);
        let (resume_tx, resume_rx) = crossbeam_channel::bounded(1);
        let (wake_tx, wake_rx) = crossbeam_channel::bounded(1);
        let mut worker = LodWorker::default();
        worker.before_filter = Some(Arc::new(move |id| {
            if id == 1 {
                started_tx.send(()).unwrap();
                resume_rx
                    .recv_timeout(std::time::Duration::from_secs(10))
                    .unwrap();
            }
        }));
        worker.set_waker(move || {
            let _ = wake_tx.try_send(());
        });
        worker
            .submit_region(
                source.clone(),
                [0, 0, 16, 16],
                budget.clone(),
                scratch.clone(),
                7,
                token(),
            )
            .unwrap();
        started_rx
            .recv_timeout(std::time::Duration::from_secs(10))
            .unwrap();
        worker
            .submit_region(
                source.clone(),
                [100, 100, 8, 8],
                budget.clone(),
                scratch.clone(),
                7,
                token(),
            )
            .unwrap();
        let latest = worker
            .submit_region(
                source.clone(),
                [200, 201, 4, 4],
                budget.clone(),
                scratch.clone(),
                7,
                token(),
            )
            .unwrap();
        assert_eq!(budget.used(), 0);
        resume_tx.send(()).unwrap();
        wake_rx.recv_timeout(std::time::Duration::from_secs(10)).unwrap();
        let completed = worker.try_take().unwrap();
        assert_eq!(completed.id, latest);
        let crop = completed.into_frame().unwrap();
        let RasterPixels::Rgba32Float(pixels) = crop.pixels() else {
            panic!()
        };
        for row in 0..4 {
            for col in 0..4 {
                assert_eq!(
                    pixels[(row * 4 + col) * 4],
                    ((201 + row) * 256 + 200 + col) as f32
                );
            }
        }
        assert_eq!(budget.used(), 256);
        drop(crop);
        assert_eq!(budget.used(), 0);
        let pressure = budget.try_reserve(budget.available_bytes()).unwrap();
        worker
            .submit_region(
                source.clone(),
                [0, 0, 8, 8],
                budget.clone(),
                scratch.clone(),
                7,
                token(),
            )
            .unwrap();
        wake_rx.recv_timeout(std::time::Duration::from_secs(10)).unwrap();
        assert!(worker.try_take().unwrap().into_frame().is_err());
        drop(pressure);
        worker
            .submit_region(
                source.clone(),
                [0, 0, 8, 8],
                budget.clone(),
                scratch.clone(),
                7,
                token(),
            )
            .unwrap();
        wake_rx.recv_timeout(std::time::Duration::from_secs(10)).unwrap();
        assert_eq!(budget.used(), 1024);
        generation.store(8, Ordering::Release);
        assert!(worker.try_take().is_none());
        assert_eq!(budget.used(), 0);
        worker
            .submit_region(
                source.clone(),
                [1, 2, 8, 8],
                budget.clone(),
                scratch.clone(),
                8,
                token(),
            )
            .unwrap();
        wake_rx.recv_timeout(std::time::Duration::from_secs(10)).unwrap();
        assert!(worker.try_take().unwrap().into_frame().is_ok());
        drop(worker);
        drop(source);
        assert_eq!(
            (
                budget.used(),
                scratch.used(),
                index.used(),
                store.usage().unwrap().bytes
            ),
            (0, 0, 0, 0)
        );
    }

    #[test]
    fn generation_changed_after_publication_releases_output_before_consumption() {
        let source = DecodedRaster::new(
            128,
            128,
            RasterPixels::Rgba32Float(Arc::new([2., -0.5, 0.25, 0.5].repeat(128 * 128)).into()),
            RasterColorSpace::LinearSrgb,
        )
        .unwrap();
        let budget = MemoryBudget::new(4096);
        let generation = Arc::new(AtomicU64::new(7));
        let (wake_tx, wake_rx) = crossbeam_channel::bounded(1);
        let mut worker = LodWorker::default();
        assert!(!worker.has_completed());
        worker.set_waker(move || {
            let _ = wake_tx.try_send(());
        });
        worker
            .submit(
                source.clone(),
                [16, 16],
                budget.clone(),
                7,
                GenerationToken::new(generation.clone(), 7),
            )
            .unwrap();
        wake_rx.recv_timeout(std::time::Duration::from_secs(10)).unwrap();
        assert_eq!(budget.used(), 4096);
        assert!(worker.has_completed());
        generation.store(8, Ordering::Release);
        assert!(worker.try_take().is_none());
        assert!(!worker.has_completed());
        assert_eq!(budget.used(), 0);
        // The same worker and full budget remain usable by the current file.
        worker
            .submit(
                source,
                [16, 16],
                budget.clone(),
                8,
                GenerationToken::new(generation, 8),
            )
            .unwrap();
        wake_rx.recv_timeout(std::time::Duration::from_secs(10)).unwrap();
        let completed = worker.try_take().unwrap();
        assert_eq!(completed.generation, 8);
        assert_eq!(budget.used(), 4096);
        drop(completed);
        assert_eq!(budget.used(), 0);
    }
    #[test]
    fn stale_submission_preserves_current_completed_level_and_revision() {
        let source = DecodedRaster::new(16, 16,
            RasterPixels::Rgba32Float(Arc::new([4., -0.5, 0.25, 1.].repeat(256)).into()),
            RasterColorSpace::LinearSrgb).unwrap();
        let budget = MemoryBudget::new(1024);
        let generation = Arc::new(AtomicU64::new(8));
        let mut worker = LodWorker::default();
        let stale = || GenerationToken::new(generation.clone(), 7);
        let error = worker.submit(source.clone(), [8, 8], budget.clone(), 7, stale())
            .expect_err("already obsolete request must be refused");
        assert_eq!(error.kind(), std::io::ErrorKind::Interrupted);
        assert!(worker.thread.is_none());
        assert_eq!(worker.shared.revision.load(Ordering::Acquire), 0);
        let (wake_tx, wake_rx) = crossbeam_channel::bounded(1);
        worker.set_waker(move || { let _ = wake_tx.try_send(()); });
        let id = worker.submit(source.clone(), [8, 8], budget.clone(), 8,
            GenerationToken::new(generation.clone(), 8)).unwrap();
        wake_rx.recv_timeout(std::time::Duration::from_secs(10)).unwrap();
        assert!(worker.has_completed());
        assert_eq!(budget.used(), 1024);
        assert!(worker.submit(source, [4, 4], budget.clone(), 7, stale()).is_err());
        assert_eq!(worker.shared.revision.load(Ordering::Acquire), id);
        assert_eq!(budget.used(), 1024, "stale submission must retain ready current level");
        let completed = worker.try_take().unwrap();
        assert_eq!((completed.id, completed.generation), (id, 8));
        let frame = completed.into_frame().unwrap();
        assert_eq!((frame.width(), frame.height()), (8, 8));
        drop(frame); drop(worker);
        assert_eq!(budget.used(), 0);
    }

    #[test]
    fn pending_requests_coalesce_without_blocking_and_only_latest_result_owns_output() {
        let source = DecodedRaster::new(
            128,
            128,
            RasterPixels::Rgba32Float(Arc::new([2., -0.5, 0.25, 0.5].repeat(128 * 128)).into()),
            RasterColorSpace::LinearSrgb,
        )
        .unwrap();
        let budget = MemoryBudget::new(16 * 16 * 16);
        let generation = Arc::new(AtomicU64::new(7));
        let token = || GenerationToken::new(generation.clone(), 7);
        let (started_tx, started_rx) = crossbeam_channel::bounded(1);
        let (resume_tx, resume_rx) = crossbeam_channel::bounded(1);
        let (wake_tx, wake_rx) = crossbeam_channel::bounded(1);
        let mut worker = LodWorker::default();
        worker.before_filter = Some(Arc::new(move |id| {
            if id == 1 {
                started_tx.send(()).unwrap();
                resume_rx
                    .recv_timeout(std::time::Duration::from_secs(10))
                    .unwrap();
            }
        }));
        worker.set_waker(move || {
            let _ = wake_tx.try_send(());
        });
        assert_eq!(
            worker
                .submit(source.clone(), [16, 16], budget.clone(), 7, token())
                .unwrap(),
            1
        );
        started_rx
            .recv_timeout(std::time::Duration::from_secs(10))
            .unwrap();
        // These return while the worker is deliberately paused: no join/wait on submit.
        assert_eq!(
            worker
                .submit(source.clone(), [8, 8], budget.clone(), 7, token())
                .unwrap(),
            2
        );
        assert_eq!(
            worker
                .submit(source.clone(), [4, 4], budget.clone(), 7, token())
                .unwrap(),
            3
        );
        assert_eq!(
            worker.shared.slots.lock().unwrap().pending.as_ref().unwrap().id,
            3
        );
        assert_eq!(budget.used(), 0);
        resume_tx.send(()).unwrap();
        wake_rx.recv_timeout(std::time::Duration::from_secs(10)).unwrap();
        let result = worker.try_take().unwrap();
        assert_eq!((result.id, result.generation), (3, 7));
        let output = result.into_frame().unwrap();
        assert_eq!((output.width(), output.height()), (4, 4));
        assert_eq!(budget.used(), 256);
        drop(output);
        assert_eq!(budget.used(), 0);
        worker.submit(source, [8, 8], budget.clone(), 7, token()).unwrap();
        wake_rx.recv_timeout(std::time::Duration::from_secs(10)).unwrap();
        assert_eq!(budget.used(), 1024);
        worker.cancel();
        assert!(worker.try_take().is_none());
        assert_eq!(budget.used(), 0);
        drop(worker);
        assert_eq!(budget.used(), 0);
    }
}
