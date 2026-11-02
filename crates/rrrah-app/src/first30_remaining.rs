//! Opt-in measurements of the actual foreground submit path, not decode/open latency.
use super::*;
use std::cell::RefCell;
#[derive(Default)]
struct Probe {
    legacy: bool,
    active: bool,
    waits: Vec<u128>,
    holds: Vec<u128>,
}
thread_local! { static PROBE: RefCell<Probe> = RefCell::new(Probe::default()); }
pub fn legacy_copy(paths: &Option<Vec<PathBuf>>) -> Option<Vec<PathBuf>> {
    PROBE.with(|p| if p.borrow().legacy { paths.clone() } else { None })
}
pub fn record_wait(elapsed: Duration) {
    PROBE.with(|p| {
        let mut p = p.borrow_mut();
        if p.active {
            p.waits.push(elapsed.as_nanos());
        }
    });
}
pub struct Hold(Option<Instant>);
impl Hold {
    pub fn start() -> Self {
        Self(PROBE.with(|p| p.borrow().active.then(Instant::now)))
    }
}
impl Drop for Hold {
    fn drop(&mut self) {
        if let Some(start) = self.0 {
            let elapsed = start.elapsed().as_nanos();
            PROBE.with(|p| p.borrow_mut().holds.push(elapsed));
        }
    }
}
fn quantile(values: &[u128], percent: usize) -> u128 {
    let mut v = values.to_vec();
    v.sort_unstable();
    v[(v.len() * percent).div_ceil(100).saturating_sub(1)]
}
#[test]
#[ignore = "manual actual foreground submit contention A/A and A/B; no decoder latency claim"]
fn foreground_submit_contention_pairs() {
    let dir = tempfile::tempdir().unwrap();
    let paths: Vec<_> = (0..4)
        .map(|i| {
            let p = dir.path().join(format!("{i}.pfm"));
            std::fs::write(&p, b"actual source stat").unwrap();
            p
        })
        .collect();
    let neighbours: Vec<_> = (0..7)
        .map(|i| dir.path().join(format!("{i}-{}", "a".repeat(180))))
        .collect();
    for producers in [1, 2, 4] {
        for control in [true, false] {
            for pair in 0..13 {
                for mode in [pair % 2, 1 - pair % 2] {
                    let (tx, rx) = bounded(1);
                    let stop = Arc::new(std::sync::atomic::AtomicBool::new(false));
                    let flight: ForegroundFlight = Arc::new(std::sync::Mutex::new(None));
                    let generation = Arc::new(AtomicU64::new(0));
                    let loader = Arc::new(ForegroundLoader {
                        edit_admission: Arc::new(std::sync::Mutex::new(None)),
                        worker: None,
                        decode_first: true,
                        automatic_policy: false,
                        flight: flight.clone(),
                        duplicate_validator: source_validation::SourceValidator::new(),
                        duplicate: std::sync::Mutex::new(None),
                        tx,
                        pending: rx.clone(),
                        generation: generation.clone(),
                        decode_gate: Arc::new(DecodeGate::new()),
                        telemetry: Arc::new(CacheTelemetry::new(true, 1024)),
                        development: None,
                    });
                    let worker_stop = stop.clone();
                    let worker_generation = generation.clone();
                    let observed = Arc::new(AtomicU64::new(0));
                    let output = observed.clone();
                    let consumer = std::thread::spawn(move || {
                        while !worker_stop.load(Ordering::Acquire) {
                            if let Ok(job) = rx.recv_timeout(Duration::from_millis(1)) {
                                let source = gallery::SourceStamp::read(&job.path);
                                let mut guard = flight.lock().unwrap();
                                if job.generation == worker_generation.load(Ordering::Acquire) {
                                    if let Some((identity, id)) = guard.as_mut() {
                                        if *id == job.generation {
                                            identity.source = Some(source);
                                            output.store(job.generation, Ordering::Release);
                                        }
                                    }
                                }
                            }
                        }
                    });
                    let barrier = Arc::new(std::sync::Barrier::new(producers + 1));
                    let mut handles = Vec::new();
                    for producer in 0..producers {
                        let loader = loader.clone();
                        let paths = paths.clone();
                        let neighbours = neighbours.clone();
                        let barrier = barrier.clone();
                        handles.push(std::thread::spawn(move || {
                            PROBE.with(|p| {
                                *p.borrow_mut() = Probe {
                                    legacy: !control && mode == 0,
                                    active: true,
                                    waits: Vec::new(),
                                    holds: Vec::new(),
                                }
                            });
                            barrier.wait();
                            let mut wall = Vec::new();
                            for i in 0..2000 {
                                let prepared = Some(neighbours.clone());
                                let start = Instant::now();
                                // Distinct container index per producer prevents duplicate coalescing
                                // from turning the enqueue benchmark into a different operation.
                                loader
                                    .submit_request_with_prefetch(
                                        paths[i % 4].clone(),
                                        producer,
                                        None,
                                        true,
                                        prepared,
                                    )
                                    .unwrap();
                                wall.push(start.elapsed().as_nanos());
                            }
                            let (waits, holds) = PROBE.with(|p| {
                                let mut p = p.borrow_mut();
                                p.active = false;
                                (std::mem::take(&mut p.waits), std::mem::take(&mut p.holds))
                            });
                            (wall, waits, holds)
                        }));
                    }
                    let started = Instant::now();
                    barrier.wait();
                    let mut wall = Vec::new();
                    let mut waits = Vec::new();
                    let mut holds = Vec::new();
                    for handle in handles {
                        let (w, l, h) = handle.join().unwrap();
                        wall.extend(w);
                        waits.extend(l);
                        holds.extend(h);
                    }
                    let total = started.elapsed().as_nanos();
                    let latest = loader.current_generation();
                    assert_eq!(latest, (2000 * producers) as u64);
                    assert_eq!(loader.flight.lock().unwrap().as_ref().unwrap().1, latest);
                    let deadline = Instant::now() + Duration::from_secs(5);
                    while observed.load(Ordering::Acquire) != latest {
                        assert!(Instant::now() < deadline, "latest submission absent");
                        std::thread::yield_now();
                    }
                    assert_eq!(waits.len(), 2000 * producers);
                    assert_eq!(holds.len(), waits.len());
                    stop.store(true, Ordering::Release);
                    consumer.join().unwrap();
                    drop(loader);
                    if pair >= 3 {
                        eprintln!(
                            "FOREGROUND_PAIR producers={producers} control={control} pair={} mode={mode} iterations={} total_ns={total} p50_ns={} p95_ns={} lock_p50_ns={} lock_p95_ns={} hold_p50_ns={} hold_p95_ns={} latest_exact=true",
                            pair - 3,
                            2000 * producers,
                            quantile(&wall, 50),
                            quantile(&wall, 95),
                            quantile(&waits, 50),
                            quantile(&waits, 95),
                            quantile(&holds, 50),
                            quantile(&holds, 95)
                        );
                    }
                }
            }
        }
    }
}

#[test]
#[cfg(target_os = "macos")]
fn upload_callbacks_require_poll_and_idle_poll_is_bounded_metal() {
    let gpu = crate::navigation_metal_readback::qualification_gpu().expect("actual Metal required");
    assert!(gpu.adapter_name().contains("Metal"));
    let completions = UploadCompletions::default();
    let budget = rrrah_core::MemoryBudget::new(16);
    let reservation = budget.try_reserve(16).unwrap();
    let held = completions.pending.clone();
    held.fetch_add(1, Ordering::AcqRel);
    gpu.queue.submit([]);
    gpu.queue.on_submitted_work_done(move || {
        drop(reservation);
        held.fetch_sub(1, Ordering::AcqRel);
    });
    let deadline = Instant::now() + Duration::from_secs(5);
    while completions.poll(&gpu.device).unwrap() {
        assert!(Instant::now() < deadline);
        std::thread::yield_now();
    }
    assert_eq!(budget.used(), 0);
    let start = Instant::now();
    for _ in 0..100_000 {
        assert!(!completions.poll(&gpu.device).unwrap());
    }
    eprintln!(
        "IDLE_POLL iterations=100000 total_ns={} credits_final=0",
        start.elapsed().as_nanos()
    );
}

fn queue_only_loader(tx: Sender<LoadRequest>, pending: Receiver<LoadRequest>) -> ForegroundLoader {
    ForegroundLoader {
        edit_admission: Arc::new(std::sync::Mutex::new(None)),
        worker: None,
        decode_first: true,
        automatic_policy: false,
        flight: Arc::new(std::sync::Mutex::new(None)),
        duplicate_validator: source_validation::SourceValidator::new(),
        duplicate: std::sync::Mutex::new(None),
        tx,
        pending,
        generation: Arc::new(AtomicU64::new(0)),
        decode_gate: Arc::new(DecodeGate::new()),
        telemetry: Arc::new(CacheTelemetry::new(true, 1024)),
        development: None,
    }
}
#[test]
fn concurrent_submit_cancel_and_queue_refusal_preserve_latest_intent() {
    for producers in [1, 2, 4] {
        let (tx, rx) = bounded(1);
        let loader = Arc::new(queue_only_loader(tx, rx.clone()));
        let first = loader.submit("first.pfm".into()).unwrap();
        let obsolete = GenerationToken::new(loader.generation.clone(), first);
        let barrier = Arc::new(std::sync::Barrier::new(producers + 2));
        std::thread::scope(|scope| {
            for producer in 0..producers {
                let loader = loader.clone();
                let barrier = barrier.clone();
                scope.spawn(move || {
                    barrier.wait();
                    for i in 0..1000 {
                        loader
                            .submit_request(PathBuf::from(format!("{i}.pfm")), producer, None, true)
                            .unwrap();
                    }
                });
            }
            let cancel = loader.clone();
            let ready = barrier.clone();
            scope.spawn(move || {
                ready.wait();
                for _ in 0..1000 {
                    cancel.cancel_pending();
                }
            });
            barrier.wait();
        });
        assert!(obsolete.is_cancelled());
        let latest = loader.submit("final.pfm".into()).unwrap();
        let request = rx.try_recv().unwrap();
        assert_eq!(request.path, PathBuf::from("final.pfm"));
        assert_eq!(request.generation, latest);
        assert!(rx.is_empty());
        assert_eq!(loader.flight.lock().unwrap().as_ref().unwrap().1, latest);
        let active = GenerationToken::new(loader.generation.clone(), latest);
        loader.cancel_pending();
        assert!(active.is_cancelled());
        assert!(loader.flight.lock().unwrap().is_none());
        assert!(rx.is_empty());
        drop(request);
    }
    let (tx, rx) = bounded(1);
    drop(rx);
    let (_, different_receiver) = bounded(1);
    let loader = queue_only_loader(tx, different_receiver);
    assert!(loader.submit("refused.pfm".into()).is_err());
    assert!(loader.flight.lock().unwrap().is_none());
    eprintln!(
        "FOREGROUND_SAFETY producers=1,2,4 cancelled_old=true final_exact=true disconnected_refused=true"
    );
}

thread_local! {
    static ENQUEUE_PAUSE: RefCell<Option<Box<dyn Fn(u64)>>> = RefCell::new(None);
}
pub fn before_enqueue_publication(generation: u64) {
    ENQUEUE_PAUSE.with(|hook| {
        if let Some(hook) = hook.borrow().as_ref() {
            hook(generation);
        }
    });
}
#[test]
fn releasing_flight_before_publication_can_erase_the_latest_request() {
    let (tx, rx) = bounded(1);
    let loader = Arc::new(queue_only_loader(tx, rx.clone()));
    let (paused, ready) = bounded(1);
    let (release, resume) = bounded(1);
    let first = loader.clone();
    let make_identity = |name: &str| ForegroundIdentity {
        prefetch_paths: Arc::new(std::sync::Mutex::new(None)),
        raster_prefetch_hint: Arc::new(std::sync::atomic::AtomicU8::new(0)),
        source: None,
        path: PathBuf::from(name),
        image_index: 0,
        scalar_window: None,
        development: None,
        decode_first: true,
        automatic_policy: false,
    };
    let older = make_identity("older.pfm");
    let thread = std::thread::spawn(move || {
        ENQUEUE_PAUSE.with(|hook| {
            *hook.borrow_mut() = Some(Box::new(move |id| {
                paused.send(id).unwrap();
                resume.recv_timeout(Duration::from_secs(5)).unwrap();
            }))
        });
        // Candidate deliberately releases the shared flight guard before enqueue.
        let mut local_flight = None;
        first
            .enqueue_foreground(older, true, None, &mut local_flight)
            .unwrap();
        *first.flight.lock().unwrap() = local_flight;
    });
    let older_generation = ready.recv_timeout(Duration::from_secs(5)).unwrap();
    let newer_generation = loader.submit("newer.pfm".into()).unwrap();
    assert!(newer_generation > older_generation);
    release.send(()).unwrap();
    thread.join().unwrap();
    let queued = rx.try_recv().unwrap();
    assert_eq!(queued.path, PathBuf::from("older.pfm"));
    assert_eq!(queued.generation, older_generation);
    assert_eq!(loader.current_generation(), newer_generation);
    assert_eq!(
        loader.flight.lock().unwrap().as_ref().unwrap().1,
        older_generation
    );
    assert!(GenerationToken::new(loader.generation.clone(), queued.generation).is_cancelled());
    assert!(rx.is_empty(), "newest intent was erased by the unsafe candidate");
    eprintln!(
        "EARLY_UNLOCK_NEGATIVE old_replaces_new=true newest_decode_lost=true production_serializes=true"
    );
}

#[test]
#[cfg(target_os = "macos")]
#[ignore = "real Metal hidden-window negative control: skipped callback polling retains upload credit"]
fn skipping_idle_poll_retains_completed_upload_credit_metal() {
    let gpu = crate::navigation_metal_readback::qualification_gpu().expect("actual Metal required");
    assert!(gpu.adapter_name().contains("Metal"));
    let budget = rrrah_core::MemoryBudget::new(16);
    let reservation = budget.try_reserve(16).unwrap();
    let completions = UploadCompletions::default();
    let copy = gpu.device.create_buffer(&wgpu::BufferDescriptor {
        label: None,
        size: 16,
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    gpu.queue.write_buffer(&copy, 0, &[0x5a; 16]);
    gpu.queue.submit([]);
    completions.pending.fetch_add(1, Ordering::AcqRel);
    let pending = completions.pending.clone();
    gpu.queue.on_submitted_work_done(move || {
        drop(reservation);
        pending.fetch_sub(1, Ordering::AcqRel);
    });
    // Simulate a hidden window: no redraw, queue submit or device poll.
    std::thread::sleep(Duration::from_millis(20));
    assert_eq!(budget.used(), 16);
    assert!(budget.try_reserve(16).is_err());
    let deadline = Instant::now() + Duration::from_secs(5);
    while completions.poll(&gpu.device).unwrap() {
        assert!(Instant::now() < deadline);
        std::thread::yield_now();
    }
    assert_eq!(budget.used(), 0);
    drop(budget.try_reserve(16).unwrap());
    let (tx, rx) = std::sync::mpsc::channel();
    copy.slice(..)
        .map_async(wgpu::MapMode::Read, move |r| tx.send(r).unwrap());
    gpu.device.poll(wgpu::PollType::wait_indefinitely()).unwrap();
    rx.recv().unwrap().unwrap();
    let mapped = copy.slice(..).get_mapped_range().unwrap();
    assert_eq!(&mapped[..], &[0x5a; 16]);
    drop(mapped);
    copy.unmap();
    eprintln!(
        "SKIP_IDLE_POLL_NEGATIVE window_redraws=0 idle_ms=20 replacement_refused_without_poll=true exact_gpu_bytes=true credits_after_poll=0"
    );
}
