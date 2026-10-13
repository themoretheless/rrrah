//! One active geometry upload, one replaceable pending job, one completed result.
use std::sync::{Arc, Condvar, Mutex, atomic::{AtomicBool, AtomicU64, Ordering}};

enum UploadContext {
    Existing(rrrah_gpu::ModelUploadPreparer),
    New { device: wgpu::Device, format: wgpu::TextureFormat, budget: Option<rrrah_core::MemoryBudget> },
}
struct Job {
    id: u64,
    generation: u64,
    model: rrrah_decode::DecodedModel,
    context: UploadContext,
    token: rrrah_decode::GenerationToken,
}
pub(crate) struct Completed {
    pub id: u64,
    pub generation: u64,
    pub result: Result<rrrah_gpu::PreparedModelUpload, rrrah_gpu::ModelUploadError>,
    pub renderer: Option<rrrah_gpu::ModelRenderer>,
    token: rrrah_decode::GenerationToken,
}
#[derive(Default)]
struct Slots { pending: Option<Job>, completed: Option<Completed> }
#[derive(Default)]
struct Shared {
    slots: Mutex<Slots>,
    ready: Condvar,
    revision: AtomicU64,
    stop: AtomicBool,
}
pub(crate) struct ModelUploadWorker {
    shared: Arc<Shared>,
    wake: Arc<dyn Fn() + Send + Sync>,
    thread: Option<std::thread::JoinHandle<()>>,
    #[cfg(test)]
    before_prepare: Option<Arc<dyn Fn(u64) + Send + Sync>>,
}
impl Default for ModelUploadWorker {
    fn default() -> Self {
        Self {
            shared: Arc::new(Shared::default()), wake: Arc::new(|| {}), thread: None,
            #[cfg(test)]
            before_prepare: None,
        }
    }
}
impl ModelUploadWorker {
    pub fn set_waker(&mut self, wake: impl Fn() + Send + Sync + 'static) {
        assert!(self.thread.is_none());
        self.wake = Arc::new(wake);
    }
    pub fn submit(&mut self, model: rrrah_decode::DecodedModel,
        preparer: rrrah_gpu::ModelUploadPreparer, generation: u64,
        token: rrrah_decode::GenerationToken) -> std::io::Result<u64> {
        self.submit_context(model, UploadContext::Existing(preparer), generation, token)
    }
    pub fn submit_new(&mut self, model: rrrah_decode::DecodedModel, device: wgpu::Device,
        format: wgpu::TextureFormat, budget: Option<rrrah_core::MemoryBudget>, generation: u64,
        token: rrrah_decode::GenerationToken) -> std::io::Result<u64> {
        self.submit_context(model, UploadContext::New { device, format, budget }, generation, token)
    }
    fn submit_context(&mut self, model: rrrah_decode::DecodedModel, context: UploadContext,
        generation: u64, token: rrrah_decode::GenerationToken) -> std::io::Result<u64> {
        if token.is_cancelled() {
            return Err(std::io::Error::new(std::io::ErrorKind::Interrupted,
                "obsolete model upload request"));
        }
        if self.thread.is_none() {
            let shared = self.shared.clone();
            let wake = self.wake.clone();
            #[cfg(test)]
            let before_prepare = self.before_prepare.clone();
            self.thread = Some(std::thread::Builder::new().name("model-upload".into()).spawn(move || {
                loop {
                    let job = {
                        let mut slots = shared.slots.lock().unwrap();
                        while slots.pending.is_none() && !shared.stop.load(Ordering::Acquire) {
                            slots = shared.ready.wait(slots).unwrap();
                        }
                        if shared.stop.load(Ordering::Acquire) { break; }
                        slots.pending.take().unwrap()
                    };
                    #[cfg(test)]
                    if let Some(hook) = &before_prepare { hook(job.id); }
                    let cancelled = || shared.stop.load(Ordering::Acquire)
                        || shared.revision.load(Ordering::Acquire) != job.id
                        || job.token.is_cancelled();
                    // Skip stale jobs before potentially expensive shader compilation.
                    if cancelled() { continue; }
                    let (preparer, renderer) = match job.context {
                        UploadContext::Existing(preparer) => (preparer, None),
                        UploadContext::New { device, format, budget } => {
                            let renderer = match budget {
                                Some(budget) => rrrah_gpu::ModelRenderer::new_with_budget(&device, format, budget),
                                None => rrrah_gpu::ModelRenderer::new(&device, format),
                            };
                            (renderer.upload_preparer(), Some(renderer))
                        }
                    };
                    let result = preparer.prepare_with_cancel(job.model.triangles(), cancelled);
                    // Release source geometry before publishing/waking the UI.
                    drop(job.model);
                    let mut slots = shared.slots.lock().unwrap();
                    if shared.stop.load(Ordering::Acquire)
                        || shared.revision.load(Ordering::Acquire) != job.id
                        || job.token.is_cancelled() { continue; }
                    slots.completed = Some(Completed {
                        id: job.id, generation: job.generation, result, renderer, token: job.token,
                    });
                    drop(slots);
                    wake();
                }
            })?);
        }
        let mut slots = self.shared.slots.lock().unwrap();
        // Reject stale callers before replacing a valid queued/prepared upload.
        if token.is_cancelled() {
            return Err(std::io::Error::new(std::io::ErrorKind::Interrupted,
                "obsolete model upload request"));
        }
        let id = self.shared.revision.fetch_add(1, Ordering::AcqRel) + 1;
        slots.completed = None;
        slots.pending = Some(Job { id, generation, model, context, token });
        drop(slots);
        self.shared.ready.notify_one();
        Ok(id)
    }
    pub fn cancel(&self) {
        let mut slots = self.shared.slots.lock().unwrap();
        self.shared.revision.fetch_add(1, Ordering::AcqRel);
        slots.pending = None;
        slots.completed = None;
    }
    pub fn try_take(&self) -> Option<Completed> {
        let mut slots = self.shared.slots.lock().unwrap();
        let completed = slots.completed.take()?;
        if completed.id != self.shared.revision.load(Ordering::Acquire)
            || completed.token.is_cancelled() { return None; }
        Some(completed)
    }
}
impl Drop for ModelUploadWorker {
    fn drop(&mut self) {
        self.shared.stop.store(true, Ordering::Release);
        self.cancel();
        self.shared.ready.notify_one();
        if let Some(thread) = self.thread.take() { let _ = thread.join(); }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    #[ignore = "requires actual GPU for bounded model worker qualification"]
    fn latest_model_upload_wins_and_cancel_releases_worker_credit() {
        let gpu = crate::navigation_metal_readback::qualification_gpu().expect("actual GPU required");
        let budget = rrrah_core::MemoryBudget::new(72);
        let mut renderer = rrrah_gpu::ModelRenderer::new_with_budget(
            &gpu.device, crate::navigation_metal_readback::READBACK_FORMAT, budget.clone());
        let source_budget = rrrah_core::MemoryBudget::new(4096);
        let mut request = rrrah_decode::DecodeRequest::new(std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../tests/fixtures/models/triangle.stl"));
        request.memory_budget = Some(source_budget.clone());
        let model = rrrah_decode::decode_model(&request).unwrap();
        let source_bytes = model.capacity_bytes();
        assert_eq!(source_budget.used(), source_bytes);
        let mut cache = rrrah_cache::LeaseCache::new(rrrah_cache::CacheLimits::bytes(source_bytes));
        cache.insert(1u8, model.clone(), source_bytes).unwrap();
        let source_lease = cache.get(&1).unwrap();
        assert!(cache.set_limits(rrrah_cache::CacheLimits::bytes(0)).is_none());
        let generation = Arc::new(AtomicU64::new(7));
        let mut worker = ModelUploadWorker::default();
        let (tx, rx) = std::sync::mpsc::sync_channel(1);
        worker.set_waker(move || { let _ = tx.try_send(()); });
        let (release_tx, release_rx) = std::sync::mpsc::sync_channel(1);
        let release_rx = Mutex::new(release_rx);
        let (started_tx, started_rx) = std::sync::mpsc::sync_channel(1);
        worker.before_prepare = Some(Arc::new(move |id| {
            if id == 1 {
                started_tx.send(()).unwrap();
                let _ = release_rx.lock().unwrap().recv_timeout(std::time::Duration::from_secs(10));
            }
        }));
        let obsolete = worker.submit(model.clone(), renderer.upload_preparer(), 6,
            rrrah_decode::GenerationToken::new(generation.clone(), 6)).unwrap_err();
        assert_eq!(obsolete.kind(), std::io::ErrorKind::Interrupted);
        assert!(worker.thread.is_none());
        assert_eq!(worker.shared.revision.load(Ordering::Acquire), 0);
        worker.submit(model.clone(), renderer.upload_preparer(), 7,
            rrrah_decode::GenerationToken::new(generation.clone(), 7)).unwrap();
        started_rx.recv_timeout(std::time::Duration::from_secs(10)).unwrap();
        let mut latest = 0;
        for _ in 0..1000 {
            latest = worker.submit(model.clone(), renderer.upload_preparer(), 7,
                rrrah_decode::GenerationToken::new(generation.clone(), 7)).unwrap();
        }
        {
            let slots = worker.shared.slots.lock().unwrap();
            assert_eq!(slots.pending.as_ref().unwrap().id, latest);
            assert!(slots.completed.is_none());
            assert_eq!(budget.used(), 0);
        }
        release_tx.send(()).unwrap();
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
        let completed = loop {
            if let Some(completed) = worker.try_take() { break completed; }
            assert!(std::time::Instant::now() < deadline);
            let _ = rx.recv_timeout(std::time::Duration::from_millis(100));
        };
        assert_eq!(completed.id, latest);
        assert_eq!(completed.generation, 7);
        assert_eq!(source_budget.used(), source_bytes);
        assert!(cache.set_limits(rrrah_cache::CacheLimits::bytes(0)).is_none());
        drop(source_lease);
        drop(cache.set_limits(rrrah_cache::CacheLimits::bytes(0)).unwrap());
        drop(cache);
        renderer.install_prepared_upload(completed.result.unwrap()).unwrap();
        assert_eq!(budget.used(), 36);
        // Hold a completed GPU result in its slot, then change generation
        // before the event loop consumes it. No timing-based cancellation race.
        for next_generation in [7, 8] {
            generation.store(next_generation, Ordering::Release);
            while rx.try_recv().is_ok() {}
            worker.submit(model.clone(), renderer.upload_preparer(), next_generation,
                rrrah_decode::GenerationToken::new(generation.clone(), next_generation)).unwrap();
            let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
            loop {
                if worker.shared.slots.lock().unwrap().completed.is_some() { break; }
                assert!(std::time::Instant::now() < deadline);
                let _ = rx.recv_timeout(std::time::Duration::from_millis(100));
            }
            assert_eq!(budget.used(), 72);
            if next_generation == 7 {
                generation.store(8, Ordering::Release);
                assert!(worker.try_take().is_none());
            } else {
                let revision = worker.shared.revision.load(Ordering::Acquire);
                let stale = worker.submit(model.clone(), renderer.upload_preparer(), 7,
                    rrrah_decode::GenerationToken::new(generation.clone(), 7));
                assert!(stale.is_err(), "obsolete model request must not replace current prepared upload");
                assert_eq!(worker.shared.revision.load(Ordering::Acquire), revision);
                assert!(worker.shared.slots.lock().unwrap().completed.is_some());
                assert_eq!(budget.used(), 72, "current ready geometry must remain owned");
                worker.cancel();
                assert!(worker.try_take().is_none());
            }
            assert_eq!(budget.used(), 36);
        }
        worker.submit(model, renderer.upload_preparer(), 8,
            rrrah_decode::GenerationToken::new(generation.clone(), 8)).unwrap();
        generation.store(9, Ordering::Release);
        worker.cancel();
        drop(worker);
        assert_eq!(source_budget.used(), 0);
        assert_eq!(budget.used(), 36);
        drop(renderer);
        assert_eq!(budget.used(), 0);
        // Compare every pixel with the synchronous renderer path across the
        // worker/device boundary, including an odd-sized depth attachment.
        for format in [wgpu::TextureFormat::Rgba8UnormSrgb, wgpu::TextureFormat::Rgba16Float] {
            let first_budget = rrrah_core::MemoryBudget::new(36 + 63 * 65 * 4);
            let mut first_open = ModelUploadWorker::default();
            let (tx, rx) = std::sync::mpsc::sync_channel(1);
            first_open.set_waker(move || { tx.send(std::thread::current().name().unwrap().to_owned()).unwrap(); });
            let model = rrrah_decode::decode_model(&request).unwrap();
            let mut reference = rrrah_gpu::ModelRenderer::new(&gpu.device, format);
            reference.upload(&gpu.device, model.triangles()).unwrap();
            reference.resize(&gpu.device, [63, 65]).unwrap();
            reference.update_view(&gpu.queue, 63. / 65., 0.4, -0.2, 1.);
            let id = first_open.submit_new(model, gpu.device.clone(), format, Some(first_budget.clone()), 9,
                rrrah_decode::GenerationToken::new(generation.clone(), 9)).unwrap();
            assert_eq!(rx.recv_timeout(std::time::Duration::from_secs(10)).unwrap(), "model-upload");
            let completed = first_open.try_take().unwrap();
            assert_eq!(completed.id, id);
            assert_eq!(source_budget.used(), 0);
            let mut renderer = completed.renderer.unwrap();
            renderer.install_prepared_upload(completed.result.unwrap()).unwrap();
            assert_eq!(first_budget.used(), 36);
            renderer.resize(&gpu.device, [63, 65]).unwrap();
            renderer.update_view(&gpu.queue, 63. / 65., 0.4, -0.2, 1.);
            assert_eq!(first_budget.used(), first_budget.limit());
            if format == wgpu::TextureFormat::Rgba16Float {
                let expected = gpu.read_half_frame([63, 65], |e, t| reference.encode(e, t));
                let actual = gpu.read_half_frame([63, 65], |e, t| renderer.encode(e, t));
                assert_eq!(actual, expected);
            } else {
                let expected = gpu.read_frame([63, 65], |e, t| reference.encode(e, t));
                let actual = gpu.read_frame([63, 65], |e, t| renderer.encode(e, t));
                assert_eq!(actual.pixels, expected.pixels);
            }
            drop(renderer);
            drop(first_open);
            assert_eq!(first_budget.used(), 0);
        }
        // Hold a newly compiled renderer and geometry in the completion slot.
        // Invalidate it before UI consumption, deterministically after publish.
        for format in [wgpu::TextureFormat::Rgba8UnormSrgb, wgpu::TextureFormat::Rgba16Float] {
            for mode in 0..3 {
                let root = rrrah_core::MemoryBudget::new(36);
                let mut first_open = ModelUploadWorker::default();
                let (tx, rx) = std::sync::mpsc::sync_channel(1);
                first_open.set_waker(move || { let _ = tx.send(()); });
                let generation = Arc::new(AtomicU64::new(1));
                let model = rrrah_decode::decode_model(&request).unwrap();
                first_open.submit_new(model, gpu.device.clone(), format, Some(root.clone()), 1,
                    rrrah_decode::GenerationToken::new(generation.clone(), 1)).unwrap();
                rx.recv_timeout(std::time::Duration::from_secs(10)).unwrap();
                assert_eq!(source_budget.used(), 0);
                assert_eq!(root.used(), 36);
                assert!(first_open.shared.slots.lock().unwrap().completed.as_ref().unwrap().renderer.is_some());
                match mode {
                    0 => {
                        generation.store(2, Ordering::Release);
                        assert!(first_open.try_take().is_none());
                        assert_eq!(root.used(), 0);
                    }
                    1 => {
                        first_open.cancel();
                        assert!(first_open.try_take().is_none());
                        assert_eq!(root.used(), 0);
                    }
                    _ => {}, // Shutdown must drop the unconsumed result too.
                }
                drop(first_open);
                assert_eq!(root.used(), 0);
                assert_eq!(source_budget.used(), 0);
            }
        }
        eprintln!("model first open cancellation: SDR/HDR generation/cancel/shutdown; six completed pipelines discarded; final CPU/GPU zero");
        eprintln!("model first open: worker-built SDR/HDR pipelines; owner-bound install; final CPU/GPU zero");
        eprintln!("model first open frames: 63x65 SDR/HDR; every sample equals synchronous path; exact geometry/depth budget");
        eprintln!("model upload bounded queue: first active job blocked; 1000 replacements; one latest pending; obsolete active discarded");
        eprintln!("model upload source ownership: managed-source exact; lease prevents eviction; cancellation/shutdown releases source CPU credit");
        eprintln!("model stale submission: no thread/revision for obsolete initial job; current prepared geometry/revision/credit retained; final CPU/GPU zero");
        eprintln!("model upload completed races: generation-change and explicit-cancel release queued GPU credit");
        eprintln!("model upload worker: 1000 replacements; latest only; generation cancellation; final credit zero");
    }
}

#[cfg(test)]
mod swap_tests {
    use super::*;
    #[test]
    #[ignore = "requires actual GPU for model swap-to-worker SDR/HDR qualification"]
    fn native_models_swap_through_worker_preserve_sdr_hdr() {
        let gpu = crate::navigation_metal_readback::qualification_gpu().expect("actual GPU required");
        let root = rrrah_core::MemoryBudget::new(128 * 1024);
        let queue = rrrah_core::MemoryBudget::new(128 * 1024);
        let gpu_budget = rrrah_core::MemoryBudget::new(128 * 1024);
        let directory = tempfile::tempdir().unwrap();
        let swap: rrrah_cache::ImageSwapCache<usize, crate::model_swap::ModelPayload> =
            rrrah_cache::ImageSwapCache::new_with_budgets(directory.path(), rrrah_cache::ImageSwapConfig {
                limits: rrrah_cache::CacheLimits::bytes(1024 * 1024),
                queue_bytes: 128 * 1024, queue_count: 1, restore_bytes: 128 * 1024,
            }, queue.clone(), root.clone()).unwrap();
        let generation = Arc::new(AtomicU64::new(0));
        let mut worker = ModelUploadWorker::default();
        let (tx, rx) = std::sync::mpsc::sync_channel(1);
        worker.set_waker(move || { let _ = tx.try_send(()); });
        for (key, name) in ["triangle.stl", "obj-shared-metadata.obj", "off-concave-u.off",
            "ply-32-ascii.ply", "ply-offset64-be.ply", "ply-u64-le.ply"].into_iter().enumerate() {
            let mut request = rrrah_decode::DecodeRequest::new(std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("../../tests/fixtures/models").join(name));
            request.memory_budget = Some(root.clone());
            let original = rrrah_decode::decode_model(&request).unwrap();
            let triangles: Vec<_> = original.triangles().collect();
            let sdr = gpu.render_model(&triangles, [63, 65], 0.4, -0.2).pixels;
            let hdr = gpu.render_model_half(&triangles, [63, 65], 0.4, -0.2);
            swap.enqueue(key, crate::model_swap::ModelPayload::from_model(original).unwrap());
            swap.wait_idle().unwrap();
            assert_eq!((root.used(), queue.used()), (0, 0));
            for format in [crate::navigation_metal_readback::READBACK_FORMAT, wgpu::TextureFormat::Rgba16Float] {
                let restored = swap.try_get(&key, || false).unwrap().unwrap();
                let mut renderer = rrrah_gpu::ModelRenderer::new_with_budget(&gpu.device, format, gpu_budget.clone());
                generation.fetch_add(1, Ordering::AcqRel);
                let current = generation.load(Ordering::Acquire);
                let pressure = gpu_budget.try_reserve(gpu_budget.available_bytes()).unwrap();
                let refused_id = worker.submit(restored.0, renderer.upload_preparer(), current,
                    rrrah_decode::GenerationToken::new(generation.clone(), current)).unwrap();
                let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
                let refused = loop {
                    if let Some(result) = worker.try_take() { break result; }
                    assert!(std::time::Instant::now() < deadline);
                    let _ = rx.recv_timeout(std::time::Duration::from_millis(100));
                };
                assert_eq!(refused.id, refused_id);
                assert!(matches!(refused.result, Err(rrrah_gpu::ModelUploadError::Memory(_))));
                assert_eq!(root.used(), 0);
                assert_eq!(gpu_budget.used(), gpu_budget.limit());
                assert_eq!(swap.stats().errors, 0);
                drop(pressure);
                let restored = swap.try_get(&key, || false).unwrap().unwrap();
                let id = worker.submit(restored.0, renderer.upload_preparer(), current,
                    rrrah_decode::GenerationToken::new(generation.clone(), current)).unwrap();
                let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
                let completed = loop {
                    if let Some(result) = worker.try_take() { break result; }
                    assert!(std::time::Instant::now() < deadline);
                    let _ = rx.recv_timeout(std::time::Duration::from_millis(100));
                };
                assert_eq!(completed.id, id);
                assert_eq!(root.used(), 0);
                renderer.install_prepared_upload(completed.result.unwrap()).unwrap();
                renderer.resize(&gpu.device, [63, 65]).unwrap();
                renderer.update_view(&gpu.queue, 63. / 65., 0.4, -0.2, 1.);
                if format == wgpu::TextureFormat::Rgba16Float {
                    assert_eq!(gpu.read_half_frame([63, 65], |e, t| renderer.encode(e, t)), hdr);
                } else {
                    assert_eq!(gpu.read_frame([63, 65], |e, t| renderer.encode(e, t)).pixels, sdr);
                }
                drop(renderer);
                assert_eq!(gpu_budget.used(), 0);
            }
            eprintln!("model swap worker: {name}; SDR/HDR exact; source CPU/GPU credit zero");
        }
        drop(worker);
        assert_eq!((root.used(), queue.used(), gpu_budget.used()), (0, 0, 0));
        assert_eq!((swap.stats().writes, swap.stats().reads, swap.stats().errors), (6, 24, 0));
        eprintln!("model swap worker pressure: refusals=12; source CPU released; swap retry exact");
    }
}

#[cfg(test)]
mod large_tests {
    use super::*;
    #[test]
    #[ignore = "manual million-triangle native STL and Metal worker qualification"]
    fn million_triangle_worker_keeps_submission_light_and_releases_memory() {
        use std::io::Write;
        let gpu = crate::navigation_metal_readback::qualification_gpu().expect("actual GPU required");
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("million.stl");
        let mut writer = std::io::BufWriter::new(std::fs::File::create(&path).unwrap());
        writer.write_all(&[0; 80]).unwrap();
        writer.write_all(&1_000_000u32.to_le_bytes()).unwrap();
        let mut record = [0u8; 50];
        for (i, value) in [0f32, 0., 1., -1., -1., 0., 1., -1., 0., 0., 1., 0.].into_iter().enumerate() {
            record[i * 4..i * 4 + 4].copy_from_slice(&value.to_le_bytes());
        }
        for _ in 0..1_000_000 { writer.write_all(&record).unwrap(); }
        writer.flush().unwrap();
        drop(writer);
        let cpu = rrrah_core::MemoryBudget::new(256 * 1024 * 1024);
        let mut request = rrrah_decode::DecodeRequest::new(path);
        request.memory_budget = Some(cpu.clone());
        let decode_started = std::time::Instant::now();
        let model = rrrah_decode::decode_model(&request).unwrap();
        let decode_ms = decode_started.elapsed().as_secs_f64() * 1000.;
        assert_eq!(model.triangle_count(), 1_000_000);
        assert_eq!(cpu.used(), model.capacity_bytes());
        let budget = rrrah_core::MemoryBudget::new(36_000_000 + 16 * 16 * 4);
        let mut renderer = rrrah_gpu::ModelRenderer::new_with_budget(
            &gpu.device, crate::navigation_metal_readback::READBACK_FORMAT, budget.clone());
        let mut worker = ModelUploadWorker::default();
        let (tx, rx) = std::sync::mpsc::sync_channel(1);
        worker.set_waker(move || { let _ = tx.try_send(()); });
        let generation = Arc::new(AtomicU64::new(1));
        let submitted = std::time::Instant::now();
        let id = worker.submit(model, renderer.upload_preparer(), 1,
            rrrah_decode::GenerationToken::new(generation, 1)).unwrap();
        let submit_ms = submitted.elapsed().as_secs_f64() * 1000.;
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(60);
        let completed = loop {
            if let Some(result) = worker.try_take() { break result; }
            assert!(std::time::Instant::now() < deadline);
            let _ = rx.recv_timeout(std::time::Duration::from_millis(100));
        };
        let ready_ms = submitted.elapsed().as_secs_f64() * 1000.;
        assert_eq!(completed.id, id);
        assert_eq!(cpu.used(), 0);
        renderer.install_prepared_upload(completed.result.unwrap()).unwrap();
        assert_eq!((budget.used(), renderer.resident_bytes()), (36_000_000, 36_000_000));
        let triangle = [[[-1f64, -1., 0.], [1., -1., 0.], [0., 1., 0.]]];
        let expected = gpu.render_model(&triangle, [16, 16], 0., 0.).pixels;
        renderer.resize(&gpu.device, [16, 16]).unwrap();
        renderer.update_view(&gpu.queue, 1., 0., 0., 1.);
        let render_started = std::time::Instant::now();
        let actual = gpu.read_frame([16, 16], |encoder, target| renderer.encode(encoder, target));
        let render_ms = render_started.elapsed().as_secs_f64() * 1000.;
        assert_eq!(actual.pixels, expected);
        assert_eq!(budget.used(), 36_000_000 + 16 * 16 * 4);
        eprintln!("million triangle render: output=16x16; every SDR pixel exact; render_readback_ms={render_ms:.3}; geometry=36000000; depth=1024");
        drop(worker);
        drop(renderer);
        assert_eq!((cpu.used(), budget.used()), (0, 0));
        eprintln!("million triangle worker: decode_ms={decode_ms:.3}; submit_ms={submit_ms:.3}; ready_ms={ready_ms:.3}; gpu_bytes=36000000; source CPU and GPU final zero");
    }
}

#[cfg(test)]
mod full_grid_tests {
    use super::*;
    #[test]
    #[ignore = "manual million-triangle grid with full HD SDR/HDR Metal readback"]
    fn million_triangle_grid_worker_full_hd_sdr_hdr() {
        use std::io::Write;
        let gpu = crate::navigation_metal_readback::qualification_gpu().expect("actual GPU required");
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("million-grid.stl");
        let mut writer = std::io::BufWriter::new(std::fs::File::create(&path).unwrap());
        writer.write_all(&[0; 80]).unwrap();
        writer.write_all(&1_000_000u32.to_le_bytes()).unwrap();
        for y in 0..1000 {
            for x in 0..500 {
                let x0 = -1. + 2. * x as f32 / 500.;
                let x1 = -1. + 2. * (x + 1) as f32 / 500.;
                let y0 = -1. + 2. * y as f32 / 1000.;
                let y1 = -1. + 2. * (y + 1) as f32 / 1000.;
                for vertices in [[[x0, y0, 0.], [x1, y0, 0.], [x1, y1, 0.]],
                    [[x0, y0, 0.], [x1, y1, 0.], [x0, y1, 0.]]] {
                    let mut record = [0u8; 50];
                    record[8..12].copy_from_slice(&1f32.to_le_bytes());
                    for (i, value) in vertices.into_iter().flatten().enumerate() {
                        record[12 + i * 4..16 + i * 4].copy_from_slice(&value.to_le_bytes());
                    }
                    writer.write_all(&record).unwrap();
                }
            }
        }
        writer.flush().unwrap();
        drop(writer);
        let reference = [[[-1f64, -1., 0.], [1., -1., 0.], [1., 1., 0.]],
            [[-1., -1., 0.], [1., 1., 0.], [-1., 1., 0.]]];
        let cpu = rrrah_core::MemoryBudget::new(256 * 1024 * 1024);
        let budget = rrrah_core::MemoryBudget::new(36_000_000 + 1920 * 1080 * 4);
        let queue_budget = rrrah_core::MemoryBudget::new(256 * 1024 * 1024);
        std::fs::create_dir(directory.path().join("swap")).unwrap();
        let swap: rrrah_cache::ImageSwapCache<u8, crate::model_swap::ModelPayload> =
            rrrah_cache::ImageSwapCache::new_with_budgets(&directory.path().join("swap"), rrrah_cache::ImageSwapConfig {
                limits: rrrah_cache::CacheLimits::bytes(256 * 1024 * 1024),
                queue_bytes: 256 * 1024 * 1024, queue_count: 1, restore_bytes: 256 * 1024 * 1024,
            }, queue_budget.clone(), cpu.clone()).unwrap();
        let mut request = rrrah_decode::DecodeRequest::new(path.clone());
        request.memory_budget = Some(cpu.clone());
        let original = rrrah_decode::decode_model(&request).unwrap();
        assert_eq!(original.triangle_count(), 1_000_000);
        let payload = crate::model_swap::ModelPayload::from_model(original).unwrap();
        let mut expected_payload = Vec::new();
        rrrah_cache::SwapPayload::write_payload(&payload, &mut expected_payload).unwrap();
        swap.enqueue(0, payload);
        swap.wait_idle().unwrap();
        assert_eq!((cpu.used(), queue_budget.used()), (0, 0));
        let mut worker = ModelUploadWorker::default();
        let (tx, rx) = std::sync::mpsc::sync_channel(1);
        worker.set_waker(move || { let _ = tx.try_send(()); });
        let generation = Arc::new(AtomicU64::new(1));
        for format in [crate::navigation_metal_readback::READBACK_FORMAT, wgpu::TextureFormat::Rgba16Float] {
            let pressure = cpu.try_reserve(cpu.available_bytes()).unwrap();
            assert!(swap.try_get(&0, || false).is_err());
            assert_eq!(swap.stats().errors, 0);
            drop(pressure);
            let mut cancelled_after_admission = false;
            assert!(swap.try_get(&0, || {
                if cpu.used() > 0 { cancelled_after_admission = true; true } else { false }
            }).unwrap().is_none());
            assert!(cancelled_after_admission);
            assert_eq!((cpu.used(), queue_budget.used()), (0, 0));
            assert_eq!(swap.stats().errors, 0);
            let restored = swap.try_get(&0, || false).unwrap().unwrap();
            let mut actual_payload = Vec::new();
            rrrah_cache::SwapPayload::write_payload(&restored, &mut actual_payload).unwrap();
            assert_eq!(actual_payload, expected_payload);
            drop(actual_payload);
            let model = restored.0;
            assert_eq!(model.triangle_count(), 1_000_000);
            let mut renderer = rrrah_gpu::ModelRenderer::new_with_budget(&gpu.device, format, budget.clone());
            let started = std::time::Instant::now();
            let id = worker.submit(model, renderer.upload_preparer(), 1,
                rrrah_decode::GenerationToken::new(generation.clone(), 1)).unwrap();
            let deadline = started + std::time::Duration::from_secs(60);
            let completed = loop {
                if let Some(result) = worker.try_take() { break result; }
                assert!(std::time::Instant::now() < deadline);
                let _ = rx.recv_timeout(std::time::Duration::from_millis(100));
            };
            let prepare_ms = started.elapsed().as_secs_f64() * 1000.;
            assert_eq!(completed.id, id);
            assert_eq!(cpu.used(), 0);
            renderer.install_prepared_upload(completed.result.unwrap()).unwrap();
            renderer.resize(&gpu.device, [1920, 1080]).unwrap();
            renderer.update_view(&gpu.queue, 1920. / 1080., 0., 0., 1.);
            let render_started = std::time::Instant::now();
            if format == wgpu::TextureFormat::Rgba16Float {
                let actual = gpu.read_half_frame([1920, 1080], |e, t| renderer.encode(e, t));
                let render_ms = render_started.elapsed().as_secs_f64() * 1000.;
                assert!(actual.iter().all(|v| v & 0x7c00 != 0x7c00));
                assert_eq!(actual, gpu.render_model_half(&reference, [1920, 1080], 0., 0.));
                eprintln!("million grid full HD: HDR; every pixel exact; prepare_ms={prepare_ms:.3}; render_readback_ms={render_ms:.3}");
            } else {
                let actual = gpu.read_frame([1920, 1080], |e, t| renderer.encode(e, t));
                let render_ms = render_started.elapsed().as_secs_f64() * 1000.;
                assert_eq!(actual.pixels, gpu.render_model(&reference, [1920, 1080], 0., 0.).pixels);
                eprintln!("million grid full HD: SDR; every pixel exact; prepare_ms={prepare_ms:.3}; render_readback_ms={render_ms:.3}");
            }
            assert_eq!(budget.used(), 36_000_000 + 1920 * 1080 * 4);
            drop(renderer);
            assert_eq!(budget.used(), 0);
        }
        drop(worker);
        assert_eq!((cpu.used(), queue_budget.used(), budget.used()), (0, 0, 0));
        assert_eq!((swap.stats().writes, swap.stats().reads, swap.stats().errors), (1, 2, 0));
        eprintln!("million grid restore failures: pressure=2; admitted-cancel=2; retry exact; temporary CPU zero");
        eprintln!("million grid swap complete: writes=1; reads=2; payload exact; final CPU/queue/GPU zero");
        eprintln!("million grid complete: triangles=1000000; 1920x1080 SDR/HDR exact; CPU/GPU credit zero");
    }
}
