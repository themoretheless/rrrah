//! Debug-only, explicitly opted-in live-window lifecycle qualification.
//! Simulates a loss notification, never a GPU hang or invalid command.
use super::*;

#[derive(Debug)]
pub(super) struct Probe {
    started: Instant,
    phase: u8,
    clock_started: bool,
    old_frames: u64,
    old_devices: [u64; 2],
    path: PathBuf,
    index: usize,
    view: [u32; 4],
    cards: [u32; 2],
    comparison: [u32; 2],
    switch_path: Option<PathBuf>,
    raster_switch_path: Option<PathBuf>,
    navigation_generation: u64,
    navigation_frames: u64,
    require_raster_cache: bool,
    model_switch: bool,
    required_prefetch: usize,
    required_prefetch_attempts: usize,
    raw_budget: Option<rrrah_core::MemoryBudget>,
    raw_initializations: u64,
    recovered_device: u64,
    pub(super) success: bool,
    pub(super) failure: Option<String>,
}
impl Probe {
    pub(super) fn from_env() -> Option<Self> {
        (std::env::var("RRRAH_QUALIFY_GPU_RECOVERY").as_deref() == Ok("1")).then(|| Self {
            started: Instant::now(),
            phase: 0,
            clock_started: false,
            old_frames: 0,
            old_devices: [0; 2],
            path: PathBuf::new(),
            index: 0,
            view: [0; 4],
            cards: [0; 2],
            comparison: [0; 2],
            switch_path: std::env::var_os("RRRAH_QUALIFY_RAW_SWITCH").map(PathBuf::from),
            raster_switch_path: std::env::var_os("RRRAH_QUALIFY_RASTER_SWITCH").map(PathBuf::from),
            navigation_generation: 0,
            navigation_frames: 0,
            require_raster_cache: std::env::var("RRRAH_QUALIFY_RASTER_REQUIRE_CACHE").as_deref() == Ok("1"),
            model_switch: std::env::var("RRRAH_QUALIFY_MODEL_SWITCH").as_deref() == Ok("1"),
            required_prefetch: std::env::var("RRRAH_QUALIFY_PREFETCH_COUNT")
                .map(|value| value.parse().expect("valid qualification prefetch count"))
                .unwrap_or(0),
            required_prefetch_attempts: std::env::var("RRRAH_QUALIFY_PREFETCH_ATTEMPTS")
                .map(|value| value.parse().expect("valid qualification attempt count"))
                .unwrap_or(0),
            raw_budget: None,
            raw_initializations: 0,
            recovered_device: 0,
            success: false,
            failure: None,
        })
    }
}
pub(super) fn next_device_epoch() -> u64 {
    static NEXT: AtomicU64 = AtomicU64::new(1);
    NEXT.fetch_add(1, Ordering::Relaxed)
}
fn view_bits(view: ViewParameters) -> [u32; 4] {
    [view.pan[0], view.pan[1], view.zoom, view.exposure_stops].map(f32::to_bits)
}
fn wait(device: &wgpu::Device) -> Result<(), String> {
    device
        .poll(wgpu::PollType::Wait {
            submission_index: None,
            timeout: Some(Duration::from_secs(5)),
        })
        .map(|_| ())
        .map_err(|error| error.to_string())
}
impl App {
    pub(super) fn qualify_recovery_step(&mut self, event_loop: &ActiveEventLoop) {
        let Some(mut probe) = self.recovery_probe.take() else {
            return;
        };
        if probe.success || probe.failure.is_some() {
            self.recovery_probe = Some(probe);
            return;
        }
        let result = (|| -> Result<(), String> {
            // App/GPU construction runs before the event loop can service its
            // first window frame. Bound active qualification separately.
            if !probe.clock_started {
                probe.started = Instant::now();
                probe.clock_started = true;
            }
            if probe.started.elapsed() > Duration::from_secs(30) {
                return Err(format!(
                    "live recovery timed out phase={} ready={:?}",
                    probe.phase, self.ready_generation
                ));
            }
            let (Some(gpu), Some(telemetry)) = (&self.gpu, &self.telemetry) else {
                return Ok(());
            };
            match probe.phase {
                0 if self.ready_generation == Some(self.foreground_loader.current_generation())
                    && gpu.presented_generation == self.ready_generation =>
                {
                    if let Some(window) = &self.telemetry_window {
                        window.set_minimized(false);
                        window.request_redraw();
                    }
                    probe.old_frames = gpu.presented_frames;
                    self.view.pan = [13.0, -9.0];
                    self.view.zoom = 1.25;
                    self.split_comparison = true;
                    self.update_view();
                    if let Some(telemetry) = &mut self.telemetry {
                        telemetry.zoom_cards(1.2);
                    }
                    probe.phase = 1;
                }
                1 if gpu.presented_frames > probe.old_frames
                    && gpu.presented_generation == self.ready_generation
                    && telemetry.presented_hud_revision == Some(telemetry.hud_revision) =>
                {
                    wait(&gpu.device)?;
                    wait(&telemetry.device)?;
                    probe.old_devices = [gpu.device_epoch, telemetry.device_epoch];
                    probe.path = self.path.clone();
                    probe.index = self.image_navigation_index();
                    probe.view = view_bits(self.view);
                    probe.cards = telemetry.hud.cards_view().map(f32::to_bits);
                    probe.comparison = [gpu.raw_comparison.0, gpu.raw_comparison.1].map(f32::to_bits);
                    eprintln!(
                        "LIVE_RECOVERY_INJECT path={} page={} devices={:?} simulated_notification=true",
                        probe.path.display(),
                        probe.index,
                        probe.old_devices
                    );
                    gpu.loss.report(
                        wgpu::DeviceLostReason::Unknown,
                        "debug qualification simulated loss".into(),
                    );
                    telemetry.loss.report(
                        wgpu::DeviceLostReason::Unknown,
                        "debug qualification simulated loss".into(),
                    );
                    gpu.window.request_redraw();
                    telemetry.window.request_redraw();
                    probe.phase = 2;
                }
                2 if self.ready_generation == Some(self.foreground_loader.current_generation())
                    && gpu.presented_generation == self.ready_generation
                    && telemetry.presented_hud_revision == Some(telemetry.hud_revision)
                    && self.last_gpu_recovery.is_some()
                    && self.last_telemetry_recovery.is_some() =>
                {
                    wait(&gpu.device)?;
                    wait(&telemetry.device)?;
                    let devices = [gpu.device_epoch, telemetry.device_epoch];
                    if devices[0] == probe.old_devices[0] || devices[1] == probe.old_devices[1] {
                        return Err("successful device-construction epoch did not change".into());
                    }
                    if self.path != probe.path
                        || self.image_index != probe.index
                        || view_bits(self.view) != probe.view
                        || telemetry.hud.cards_view().map(f32::to_bits) != probe.cards
                        || [gpu.raw_comparison.0, gpu.raw_comparison.1].map(f32::to_bits) != probe.comparison
                    {
                        return Err("selection or view changed during recreation".into());
                    }
                    if probe.raster_switch_path.is_some()
                        && (self.pending_folder_scan.is_some()
                            || !prefetch_complete(
                                self.foreground_loader.current_generation(),
                                probe.required_prefetch,
                                probe.required_prefetch_attempts,
                            ))
                    {
                        return Ok(());
                    }
                    eprintln!(
                        "LIVE_RECOVERY_PASS path={} page={} devices={devices:?} main_frames={} telemetry_frames={} selection_view_exact=true gpu_wait_ok=true simulated_notification=true",
                        self.path.display(),
                        self.image_index,
                        gpu.presented_frames,
                        telemetry.presented_frames
                    );
                    if let Some(path) = &probe.raster_switch_path {
                        if probe.switch_path.is_some() {
                            return Err("choose only one RAW or raster navigation probe".into());
                        }
                        if self.pending_folder_scan.is_some() {
                            return Ok(());
                        }
                        if !gpu.raster_active || self.image_index != 0 {
                            return Err("raster navigation probe requires a selected raster at index0".into());
                        }
                        let index = gallery_target(&self.gallery, path)?;
                        if gallery_target(&self.gallery, &probe.path)? == index {
                            return Err("raster navigation probe requires two distinct files".into());
                        }
                        probe.recovered_device = gpu.device_epoch;
                        probe.navigation_generation = self.foreground_loader.current_generation();
                        probe.navigation_frames = gpu.presented_frames;
                        probe.phase = 5;
                        if probe.required_prefetch > 0 {
                            if self.gallery_index.and_then(|current| current.checked_add(1)) != Some(index) {
                                return Err("prefetch probe requires the next gallery file".into());
                            }
                            self.navigate_gallery(1);
                        } else {
                            self.open_gallery_index(index);
                        }
                    } else if let Some(path) = &probe.switch_path {
                        if self.pending_folder_scan.is_some() {
                            return Ok(());
                        }
                        let budget = gpu
                            .raw_gpu_budget
                            .clone()
                            .ok_or("RAW switch probe requires raw GPU budget")?;
                        if gpu.renderer.is_none() || budget.used() == 0 {
                            return Err("RAW texture was not admitted before switch".into());
                        }
                        probe.raw_initializations = gpu.raw_initializations;
                        probe.recovered_device = gpu.device_epoch;
                        probe.raw_budget = Some(budget);
                        let index = self
                            .gallery
                            .iter()
                            .position(|p| p.file_name() == path.file_name())
                            .ok_or("switch raster absent from gallery")?;
                        probe.phase = 3;
                        self.open_gallery_index(index);
                    } else {
                        probe.success = true;
                        event_loop.exit();
                    }
                }
                3 if gpu.raster_active
                    && self.ready_generation == Some(self.foreground_loader.current_generation())
                    && gpu.presented_generation == self.ready_generation =>
                {
                    wait(&gpu.device)?;
                    if gpu.device_epoch != probe.recovered_device
                        || gpu.raw_initializations != probe.raw_initializations
                    {
                        return Err("RAW renderer unexpectedly recreated during raster switch".into());
                    }
                    let used = probe.raw_budget.as_ref().unwrap().used();
                    if used != 0 {
                        return Err(format!("RAW texture credit retained on raster: {used}"));
                    }
                    eprintln!(
                        "LIVE_RAW_SWITCH_RELEASE raw_bytes=0 init_count={} device_epoch={}",
                        gpu.raw_initializations, gpu.device_epoch
                    );
                    let index = self
                        .gallery
                        .iter()
                        .position(|p| p.file_name() == probe.path.file_name())
                        .ok_or("original RAW absent from gallery")?;
                    probe.phase = 4;
                    self.open_gallery_index(index);
                }
                4 if !gpu.raster_active
                    && !gpu.model_active
                    && self.path.file_name() == probe.path.file_name()
                    && self.ready_generation == Some(self.foreground_loader.current_generation())
                    && gpu.presented_generation == self.ready_generation =>
                {
                    wait(&gpu.device)?;
                    if gpu.device_epoch != probe.recovered_device
                        || gpu.raw_initializations != probe.raw_initializations
                    {
                        return Err("RAW renderer was recreated instead of reused".into());
                    }
                    let budget = probe.raw_budget.as_ref().unwrap();
                    if budget.used() == 0 || budget.peak() > budget.limit() {
                        return Err("RAW texture admission/limit invalid on return".into());
                    }
                    eprintln!(
                        "LIVE_RAW_SWITCH_PASS init_count={} raw_bytes={} raw_peak={} raw_limit={} same_device=true",
                        gpu.raw_initializations,
                        budget.used(),
                        budget.peak(),
                        budget.limit()
                    );
                    probe.success = true;
                    event_loop.exit();
                }
                5 | 6
                    if (gpu.raster_active
                        || (probe.model_switch && probe.phase == 5 && gpu.model_active))
                        && self.ready_generation == Some(self.foreground_loader.current_generation())
                        && gpu.presented_generation == self.ready_generation
                        && gpu.presented_frames > probe.navigation_frames =>
                {
                    wait(&gpu.device)?;
                    let generation = self.foreground_loader.current_generation();
                    if !prefetch_complete(
                        generation,
                        probe.required_prefetch,
                        probe.required_prefetch_attempts,
                    ) {
                        return Ok(());
                    }
                    if generation == probe.navigation_generation || gpu.device_epoch != probe.recovered_device
                    {
                        return Err(
                            "raster navigation did not present a new generation on the same device".into(),
                        );
                    }
                    if probe.model_switch && probe.phase == 5 && !gpu.model_active {
                        return Err("mixed navigation did not activate the model renderer".into());
                    }
                    let expected = if probe.phase == 5 {
                        probe.raster_switch_path.as_ref().unwrap()
                    } else {
                        &probe.path
                    };
                    if gallery_target(&self.gallery, &self.path)? != gallery_target(&self.gallery, expected)?
                        || self.image_index != 0
                    {
                        return Err("raster navigation presented the wrong selection".into());
                    }
                    eprintln!(
                        "LIVE_RASTER_NAV_PRESENT phase={} path={} generation={generation} previous_generation={} frames={} same_device=true gpu_wait_ok=true kind={}",
                        probe.phase,
                        self.path.display(),
                        probe.navigation_generation,
                        gpu.presented_frames,
                        if gpu.model_active { "model" } else { "raster" }
                    );
                    if probe.phase == 5 {
                        let index = gallery_target(&self.gallery, &probe.path)?;
                        probe.navigation_generation = generation;
                        probe.navigation_frames = gpu.presented_frames;
                        if probe.require_raster_cache {
                            let path = probe.path.canonicalize().map_err(|error| error.to_string())?;
                            FORBIDDEN_RASTER_DECODE
                                .set(path)
                                .map_err(|_| "raster decode guard already installed")?;
                            eprintln!(
                                "LIVE_RASTER_CACHE_GUARD path={} fallback_decode=forbidden",
                                probe.path.display()
                            );
                        }
                        probe.phase = 6;
                        if probe.required_prefetch > 0 {
                            if self.gallery_index.and_then(|current| current.checked_sub(1)) != Some(index) {
                                return Err(
                                    "prefetch probe requires return to the previous gallery file".into()
                                );
                            }
                            self.navigate_gallery(-1);
                        } else {
                            self.open_gallery_index(index);
                        }
                    } else {
                        if probe.require_raster_cache && GUARDED_CACHE_HIT.load(Ordering::Acquire) == 0 {
                            return Err("guarded return was not a recorded foreground cache hit".into());
                        }
                        eprintln!(
                            "LIVE_RASTER_NAV_PASS roundtrip=true presented_generations_distinct=true same_device=true cache_required={}",
                            probe.require_raster_cache
                        );
                        if probe.model_switch {
                            eprintln!(
                                "LIVE_MIXED_NAV_PASS raster_model_raster=true model_renderer_confirmed=true"
                            );
                        }
                        probe.success = true;
                        event_loop.exit();
                    }
                }
                _ => {}
            }
            Ok(())
        })();
        if let Err(error) = result {
            eprintln!("LIVE_RECOVERY_FAIL {error}");
            probe.failure = Some(error);
            event_loop.exit();
        }
        self.recovery_probe = Some(probe);
    }
}

// Match actual file identity, not a potentially repeated basename.
fn gallery_target(gallery: &[PathBuf], path: &std::path::Path) -> Result<usize, String> {
    let expected = path
        .canonicalize()
        .map_err(|error| format!("navigation source: {error}"))?;
    gallery
        .iter()
        .position(|candidate| candidate.canonicalize().is_ok_and(|value| value == expected))
        .ok_or_else(|| format!("navigation source absent from gallery: {}", path.display()))
}

static FORBIDDEN_RASTER_DECODE: std::sync::OnceLock<PathBuf> = std::sync::OnceLock::new();

pub(super) fn check_raster_decode(path: &std::path::Path) -> Result<(), String> {
    if let Some(forbidden) = FORBIDDEN_RASTER_DECODE.get() {
        let actual = path.canonicalize().map_err(|error| error.to_string())?;
        if &actual == forbidden {
            eprintln!("LIVE_RASTER_DECODE_REFUSED path={}", path.display());
            return Err("qualification requires cached raster; fallback decode forbidden".into());
        }
    }
    Ok(())
}

static GUARDED_CACHE_HIT: AtomicU64 = AtomicU64::new(0);

pub(super) fn traces_raster_cache(path: &std::path::Path) -> bool {
    FORBIDDEN_RASTER_DECODE
        .get()
        .is_some_and(|expected| path.canonicalize().is_ok_and(|actual| &actual == expected))
}

pub(super) fn record_raster_cache_hit(path: &std::path::Path, before: u64, after: u64) {
    let tier = if after > before { "swap" } else { "ram" };
    GUARDED_CACHE_HIT.store(if after > before { 2 } else { 1 }, Ordering::Release);
    eprintln!(
        "LIVE_RASTER_CACHE_HIT path={} tier={tier} swap_reads_before={before} swap_reads_after={after} foreground=true",
        path.display()
    );
}

static QUALIFIED_PREFETCH: std::sync::Mutex<(u64, usize, usize)> = std::sync::Mutex::new((0, 0, 0));

pub(super) fn record_raster_prefetch(generation: u64, path: &std::path::Path, retained: bool) {
    record_prefetch("RASTER", generation, path, retained);
}
pub(super) fn record_model_prefetch(generation: u64, path: &std::path::Path, retained: bool) {
    record_prefetch("MODEL", generation, path, retained);
}
fn record_prefetch(kind: &str, generation: u64, path: &std::path::Path, retained: bool) {
    if std::env::var_os("RRRAH_QUALIFY_PREFETCH_COUNT").is_none() {
        return;
    }
    let mut state = QUALIFIED_PREFETCH.lock().expect("qualification prefetch state");
    if state.0 != generation {
        *state = (generation, 0, 0);
    }
    state.2 += 1;
    if retained {
        state.1 += 1;
        eprintln!(
            "LIVE_{kind}_PREFETCH_READY generation={generation} path={} retained=true count={}",
            path.display(),
            state.1
        );
    }
    eprintln!(
        "LIVE_{kind}_PREFETCH_ATTEMPT generation={generation} path={} retained={retained} attempts={} retained_count={}",
        path.display(),
        state.2,
        state.1
    );
}
fn prefetch_complete(generation: u64, required: usize, attempts: usize) -> bool {
    if required == 0 && attempts == 0 {
        return true;
    }
    let state = QUALIFIED_PREFETCH.lock().expect("qualification prefetch state");
    state.0 == generation && state.1 >= required && state.2 >= attempts
}
