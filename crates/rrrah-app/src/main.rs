#![allow(
    clippy::cast_possible_truncation,
    clippy::cast_precision_loss,
    clippy::collapsible_if,
    clippy::large_enum_variant,
    clippy::too_many_lines,
    clippy::match_same_arms,
    clippy::redundant_guards,
    clippy::needless_pass_by_value,
    clippy::uninlined_format_args,
    clippy::unnested_or_patterns,
    clippy::while_let_loop
)]

use std::{
    fs,
    path::{Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
    thread,
    time::{Duration, Instant},
};

use anyhow::{Context, Result, bail};
use clap::Parser;
use crossbeam_channel::{Receiver, Sender, TryRecvError, bounded, unbounded};
use directories::ProjectDirs;
use rrrah_cache::{CacheKey, DEFAULT_MAX_DISK_CACHE_BYTES, DiskMosaicCache, SourceFingerprint};
use rrrah_core::DecodedMosaic;
use rrrah_decode::{DecodeRequest, DecodeTimings, GenerationToken, NativeRawDecoder, RawDecoder};
use rrrah_gpu::{GpuUploadTimings, HudCard, HudRenderer, RawRenderer, ViewParameters};
use winit::{
    application::ApplicationHandler,
    dpi::{LogicalSize, PhysicalPosition, PhysicalSize},
    event::{ElementState, MouseButton, MouseScrollDelta, WindowEvent},
    event_loop::{ActiveEventLoop, ControlFlow, EventLoop, EventLoopProxy, OwnedDisplayHandle},
    keyboard::{KeyCode, PhysicalKey},
    window::{Window, WindowId},
};

mod cache_telemetry;
mod cache_writer;
mod decode_gate;
mod gallery;
mod pipeline_telemetry;

use cache_telemetry::{CacheTelemetry, CacheTelemetrySnapshot};
use cache_writer::CacheWriter;
use decode_gate::{DecodeGate, ForegroundTicket};
use pipeline_telemetry::{
    CacheRoute, FrameSubmitTimings, FrontendTimings, PipelineSnapshot, PipelineStageState, RawKind,
};

#[derive(Debug, Parser)]
#[command(name = "rrrah", about = "Native full-sensor CR3 and DNG viewer")]
struct Cli {
    /// Decode and print metadata/timings without opening a window.
    #[arg(long)]
    inspect: bool,
    /// Do not read or write the decoded-mosaic disk cache.
    #[arg(long)]
    no_cache: bool,
    /// Override the cache directory.
    #[arg(long)]
    cache_dir: Option<PathBuf>,
    #[arg(value_name = "RAW")]
    path: Option<PathBuf>,
}

#[derive(Debug)]
enum LoadEvent {
    Progress {
        generation: u64,
        raw_kind: RawKind,
        cache_route: Option<CacheRoute>,
        timings: FrontendTimings,
    },
    Ready {
        generation: u64,
        mosaic: DecodedMosaic,
        raw_kind: RawKind,
        cache_route: CacheRoute,
        elapsed: Duration,
        requested_at: Instant,
        ready_published_at: Instant,
        frontend: FrontendTimings,
        decode: Option<DecodeTimings>,
    },
    Failed {
        generation: u64,
        error: String,
    },
}

fn main() -> Result<()> {
    env_logger::init();
    let cli = Cli::parse();
    let cache_root = cli.cache_dir.or_else(default_cache_dir);
    if cli.inspect {
        let path = cli
            .path
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("--inspect requires a RAW path"))?;
        if !path.is_file() {
            bail!("RAW path is not a regular file: {}", path.display());
        }
        inspect(path, cache_root.as_deref(), cli.no_cache)
    } else {
        run_viewer(cli.path, cache_root, cli.no_cache)
    }
}

fn default_cache_dir() -> Option<PathBuf> {
    ProjectDirs::from("org", "rrrah", "rrrah").map(|dirs| dirs.cache_dir().join("mosaics"))
}

fn inspect(path: &PathBuf, cache_root: Option<&std::path::Path>, no_cache: bool) -> Result<()> {
    let started = Instant::now();
    let decode_request = DecodeRequest::new(path);
    let recipe = NativeRawDecoder
        .mosaic_recipe(&decode_request)
        .map_err(|error| anyhow::anyhow!(error))?;
    let cache = cache_root.map(DiskMosaicCache::new);
    let fingerprint = if no_cache {
        None
    } else {
        Some(SourceFingerprint::from_path(path).context("fingerprint RAW")?)
    };
    if let (Some(cache), Some(fingerprint)) = (&cache, &fingerprint) {
        let key = CacheKey::for_mosaic_recipe(fingerprint, 0, recipe);
        if let Some(hit) = cache.load(key).context("read decoded-mosaic cache")? {
            print_metadata(&hit.mosaic, true, hit.elapsed, started.elapsed(), None);
            return Ok(());
        }
    }
    let output = NativeRawDecoder
        .decode(&decode_request)
        .map_err(|error| anyhow::anyhow!(error))?;
    if let (Some(cache), Some(fingerprint)) = (&cache, &fingerprint) {
        let key = CacheKey::for_mosaic_recipe(fingerprint, 0, recipe);
        cache
            .store(key, &output.mosaic)
            .context("write decoded-mosaic cache")?;
    }
    print_metadata(
        &output.mosaic,
        false,
        output.timings.total,
        started.elapsed(),
        Some(&output.timings),
    );
    Ok(())
}

fn print_metadata(
    mosaic: &DecodedMosaic,
    cache_hit: bool,
    decode_time: Duration,
    total: Duration,
    decode: Option<&DecodeTimings>,
) {
    let metadata = &mosaic.metadata;
    println!("source: {} {}", metadata.make, metadata.model);
    println!(
        "raw: {}x{} {}-bit cpp={} pixels={} bytes={}",
        metadata.width,
        metadata.height,
        metadata.bits_per_sample,
        metadata.components_per_pixel,
        mosaic.pixels.len(),
        mosaic.byte_len()
    );
    println!(
        "photometric: {:?}, cfa: {:?}, crop: {:?}, orientation: {:?}",
        metadata.photometric,
        metadata.cfa,
        metadata.effective_crop(),
        metadata.orientation
    );
    println!(
        "cache_hit: {cache_hit}, decode_or_cache: {:.2?}, total: {:.2?}",
        decode_time, total
    );
    if let Some(timings) = decode
        && let Some(native) = timings.native
    {
        println!(
            "native_crx: source={:.2?}, parse={:.2?}, workers={}, planes=[{:.2?}, {:.2?}, {:.2?}, {:.2?}], plane_wall={:.2?}, interleave={:.2?}",
            timings.source_open,
            timings.decoder_select,
            native.worker_count,
            native.plane_decode[0],
            native.plane_decode[1],
            native.plane_decode[2],
            native.plane_decode[3],
            native.plane_wall,
            native.interleave,
        );
    }
    if let Some(timings) = decode
        && let Some(dng) = timings.dng
    {
        println!(
            "native_dng: source={:.2?}, header={:.2?}, ifd_walk={:.2?}, raw_ifd={:.2?}, storage={:.2?}, unpack={:.2?}, linearize={:.2?}, metadata={:.2?}",
            timings.source_open,
            dng.tiff_header,
            dng.ifd_walk,
            dng.raw_ifd_select,
            dng.storage_plan,
            dng.pixel_unpack,
            dng.linearization,
            dng.metadata,
        );
    }
    println!("embedded JPEG is not used by this path");
}

fn run_viewer(path: Option<PathBuf>, cache_root: Option<PathBuf>, no_cache: bool) -> Result<()> {
    let (sender, receiver) = unbounded();
    let event_loop = EventLoop::<WakeEvent>::with_user_event()
        .build()
        .context("create event loop")?;
    let proxy = event_loop.create_proxy();
    if let Some(path) = &path {
        if !path.is_file() {
            bail!("RAW path is not a regular file: {}", path.display());
        }
    }
    let decode_gate = Arc::new(DecodeGate::new());
    let cache_telemetry = Arc::new(CacheTelemetry::new(
        cache_root.is_some() && !no_cache,
        DEFAULT_MAX_DISK_CACHE_BYTES,
    ));
    let foreground_loader = ForegroundLoader::new(
        cache_root.clone(),
        no_cache,
        sender,
        proxy,
        Arc::clone(&decode_gate),
        Arc::clone(&cache_telemetry),
    )
    .context("start foreground RAW worker")?;
    if let Some(path) = &path {
        foreground_loader.submit_initial(path.clone())?;
    }
    event_loop.set_control_flow(ControlFlow::Wait);
    let mut app = App::new(
        path.unwrap_or_default(),
        cache_root,
        no_cache,
        receiver,
        foreground_loader,
        decode_gate,
        cache_telemetry,
    );
    event_loop.run_app(&mut app).context("run event loop")?;
    Ok(())
}

#[derive(Debug, Clone, Copy)]
enum WakeEvent {
    LoadProgress,
}

#[derive(Debug)]
struct LoadRequest {
    path: PathBuf,
    generation: u64,
    requested_at: Instant,
    foreground: ForegroundTicket,
}

#[derive(Debug, Clone, Copy)]
struct PendingFirstPresent {
    generation: u64,
    requested_at: Instant,
}

/// One persistent foreground decoder with a latest-wins queue. Serializing
/// requests is essential while a plane is inside entropy decode: rapid
/// navigation keeps at most one active decode and one pending path instead of
/// spawning an unbounded set of competing decoder threads.
struct ForegroundLoader {
    tx: Sender<LoadRequest>,
    pending: Receiver<LoadRequest>,
    generation: Arc<AtomicU64>,
    decode_gate: Arc<DecodeGate>,
    telemetry: Arc<CacheTelemetry>,
}

impl ForegroundLoader {
    fn new(
        cache_root: Option<PathBuf>,
        no_cache: bool,
        sender: Sender<LoadEvent>,
        proxy: EventLoopProxy<WakeEvent>,
        decode_gate: Arc<DecodeGate>,
        telemetry: Arc<CacheTelemetry>,
    ) -> std::io::Result<Self> {
        let (tx, requests) = bounded::<LoadRequest>(1);
        let pending = requests.clone();
        let generation = Arc::new(AtomicU64::new(0));
        let worker_generation = Arc::clone(&generation);
        let worker_telemetry = Arc::clone(&telemetry);
        thread::Builder::new()
            .name("rrrah-raw-decode".into())
            .spawn(move || {
                let cache = cache_root.filter(|_| !no_cache).map(DiskMosaicCache::new);
                if let Some(cache) = &cache {
                    match cache.usage() {
                        Ok(usage) => worker_telemetry.update_disk_usage(usage),
                        Err(_) => worker_telemetry.record_disk_scan_error(),
                    }
                }
                let cache_writer = cache.as_ref().and_then(|cache| {
                    CacheWriter::spawn(
                        cache.clone(),
                        Arc::clone(&worker_generation),
                        Arc::clone(&worker_telemetry),
                    )
                    .map_err(|error| log::warn!("failed to start cache write-back worker: {error}"))
                    .ok()
                });
                while let Ok(request) = requests.recv() {
                    execute_load(
                        request,
                        cache.as_ref(),
                        cache_writer.as_ref(),
                        Arc::clone(&worker_generation),
                        &sender,
                        &proxy,
                        &worker_telemetry,
                    );
                }
            })?;
        Ok(Self {
            tx,
            pending,
            generation,
            decode_gate,
            telemetry,
        })
    }

    fn submit_initial(&self, path: PathBuf) -> Result<()> {
        let requested_at = Instant::now();
        let generation = self.current_generation();
        self.telemetry.begin_lookup(generation);
        self.replace_pending(LoadRequest {
            path,
            generation,
            requested_at,
            foreground: self.decode_gate.request_foreground(),
        })
    }

    fn submit(&self, path: PathBuf) -> Result<u64> {
        let requested_at = Instant::now();
        let generation = self.generation.fetch_add(1, Ordering::AcqRel).wrapping_add(1);
        self.telemetry.begin_lookup(generation);
        self.replace_pending(LoadRequest {
            path,
            generation,
            requested_at,
            foreground: self.decode_gate.request_foreground(),
        })?;
        Ok(generation)
    }

    fn current_generation(&self) -> u64 {
        self.generation.load(Ordering::Acquire)
    }

    fn replace_pending(&self, request: LoadRequest) -> Result<()> {
        while self.pending.try_recv().is_ok() {}
        self.tx
            .try_send(request)
            .map_err(|error| anyhow::anyhow!("foreground RAW queue unavailable: {error}"))
    }
}

fn execute_load(
    request: LoadRequest,
    cache: Option<&DiskMosaicCache>,
    cache_writer: Option<&CacheWriter>,
    load_generation: Arc<AtomicU64>,
    sender: &Sender<LoadEvent>,
    proxy: &EventLoopProxy<WakeEvent>,
    telemetry: &CacheTelemetry,
) {
    let LoadRequest {
        path,
        generation,
        requested_at,
        foreground,
    } = request;
    let token = GenerationToken::new(load_generation, generation);
    if token.is_cancelled() {
        return;
    }
    let mut decode_request = DecodeRequest::new(&path);
    decode_request.cancellation = Some(token.clone());
    let recipe = match NativeRawDecoder.mosaic_recipe(&decode_request) {
        Ok(recipe) => recipe,
        Err(error) => {
            publish_load_failure(generation, error.to_string(), &token, sender, proxy);
            return;
        }
    };
    let raw_kind = RawKind::from_path(&path);
    let mut frontend = FrontendTimings {
        queue_wait: requested_at.elapsed(),
        ..FrontendTimings::default()
    };
    publish_load_event(
        LoadEvent::Progress {
            generation,
            raw_kind,
            cache_route: cache.is_none().then_some(CacheRoute::Disabled),
            timings: frontend,
        },
        sender,
        proxy,
    );
    let fingerprint = match cache {
        Some(_) => {
            let fingerprint_started = Instant::now();
            match SourceFingerprint::from_path(&path) {
                Ok(value) => {
                    frontend.fingerprint = Some(fingerprint_started.elapsed());
                    Some(value)
                }
                Err(error) => {
                    telemetry.record_read_error(generation);
                    publish_load_failure(generation, error.to_string(), &token, sender, proxy);
                    return;
                }
            }
        }
        None => None,
    };
    if cache.is_some() {
        publish_load_event(
            LoadEvent::Progress {
                generation,
                raw_kind,
                cache_route: None,
                timings: frontend,
            },
            sender,
            proxy,
        );
    }
    if token.is_cancelled() {
        return;
    }
    let mut cache_route = if cache.is_some() {
        CacheRoute::Miss
    } else {
        CacheRoute::Disabled
    };
    let mut cache_key = None;
    if let (Some(cache), Some(fingerprint)) = (cache, &fingerprint) {
        let cache_lookup_started = Instant::now();
        let key = CacheKey::for_mosaic_recipe(fingerprint, 0, recipe);
        cache_key = Some(key);
        let cache_result = cache.load(key);
        frontend.cache_lookup = Some(cache_lookup_started.elapsed());
        match cache_result {
            Ok(Some(hit)) => {
                if token.is_cancelled() {
                    return;
                }
                telemetry.record_hit(
                    generation,
                    hit.elapsed,
                    u64::try_from(hit.mosaic.byte_len()).unwrap_or(u64::MAX),
                );
                publish_load_event(
                    LoadEvent::Ready {
                        generation,
                        mosaic: hit.mosaic,
                        raw_kind,
                        cache_route: CacheRoute::Hit,
                        elapsed: requested_at.elapsed(),
                        requested_at,
                        ready_published_at: Instant::now(),
                        frontend,
                        decode: None,
                    },
                    sender,
                    proxy,
                );
                return;
            }
            Ok(None) => telemetry.record_miss(generation),
            Err(error) => {
                cache_route = CacheRoute::ErrorFallback;
                telemetry.record_read_error(generation);
                log::warn!("ignoring corrupt/unreadable cache: {error}");
            }
        }
    }
    if cache.is_some() {
        publish_load_event(
            LoadEvent::Progress {
                generation,
                raw_kind,
                cache_route: Some(cache_route),
                timings: frontend,
            },
            sender,
            proxy,
        );
    }
    // This wait runs only on the persistent foreground worker.  The UI has
    // already published foreground priority through the request ticket and
    // remains free to redraw or replace the pending request.
    let admission_started = Instant::now();
    let Some(decode_permit) = foreground.acquire_decode(|| token.is_cancelled()) else {
        return;
    };
    frontend.admission = Some(admission_started.elapsed());
    if token.is_cancelled() {
        return;
    }
    publish_load_event(
        LoadEvent::Progress {
            generation,
            raw_kind,
            cache_route: Some(cache_route),
            timings: frontend,
        },
        sender,
        proxy,
    );
    let output = match NativeRawDecoder.decode(&decode_request) {
        Ok(output) => output,
        Err(error) => {
            publish_load_failure(generation, error.to_string(), &token, sender, proxy);
            return;
        }
    };
    // Cache persistence is intentionally outside the heavy-decode permit.
    // A foreground request must never wait behind an fsync; the bounded
    // write-back worker serializes persistence independently.
    drop(decode_permit);
    if token.is_cancelled() {
        return;
    }
    let decode_timings = output.timings;
    let mosaic = output.mosaic;
    let write_back = cache_key.map(|key| (key, mosaic.clone()));
    let elapsed = requested_at.elapsed();
    // Publish the usable frame before enqueueing persistence. Even an already
    // active atomic store can never delay this Ready event or the next decode.
    publish_load_event(
        LoadEvent::Ready {
            generation,
            mosaic,
            raw_kind,
            cache_route,
            elapsed,
            requested_at,
            ready_published_at: Instant::now(),
            frontend,
            decode: Some(decode_timings),
        },
        sender,
        proxy,
    );
    if let (Some(cache_writer), Some((key, mosaic))) = (cache_writer, write_back) {
        let _ = cache_writer.submit(generation, key, mosaic);
    }
}

fn publish_load_failure(
    generation: u64,
    error: String,
    token: &GenerationToken,
    sender: &Sender<LoadEvent>,
    proxy: &EventLoopProxy<WakeEvent>,
) {
    if !token.is_cancelled() {
        publish_load_event(LoadEvent::Failed { generation, error }, sender, proxy);
    }
}

fn publish_load_event(event: LoadEvent, sender: &Sender<LoadEvent>, proxy: &EventLoopProxy<WakeEvent>) {
    if sender.send(event).is_ok() {
        let _ = proxy.send_event(WakeEvent::LoadProgress);
    }
}

struct App {
    path: PathBuf,
    receiver: Receiver<LoadEvent>,
    foreground_loader: ForegroundLoader,
    gallery: Vec<PathBuf>,
    gallery_index: Option<usize>,
    raw_prefetcher: gallery::RawPrefetcher,
    cache_telemetry: Arc<CacheTelemetry>,
    window: Option<Arc<Window>>,
    gpu: Option<GpuState>,
    telemetry_window: Option<Arc<Window>>,
    telemetry: Option<TelemetryState>,
    view: ViewParameters,
    dragging: bool,
    last_cursor: Option<PhysicalPosition<f64>>,
    status: String,
    pipeline: PipelineSnapshot,
    pending_first_present: Option<PendingFirstPresent>,
}

impl App {
    fn new(
        path: PathBuf,
        cache_root: Option<PathBuf>,
        no_cache: bool,
        receiver: Receiver<LoadEvent>,
        foreground_loader: ForegroundLoader,
        decode_gate: Arc<DecodeGate>,
        cache_telemetry: Arc<CacheTelemetry>,
    ) -> Self {
        let raw_prefetcher =
            gallery::RawPrefetcher::new(cache_root, no_cache, decode_gate, Arc::clone(&cache_telemetry));
        let generation = foreground_loader.current_generation();
        let has_initial_path = !path.as_os_str().is_empty();
        let raw_kind = RawKind::from_path(&path);
        if has_initial_path {
            raw_prefetcher.begin_foreground();
        }
        Self {
            path,
            receiver,
            foreground_loader,
            gallery: Vec::new(),
            gallery_index: None,
            raw_prefetcher,
            cache_telemetry,
            window: None,
            gpu: None,
            telemetry_window: None,
            telemetry: None,
            view: ViewParameters::default(),
            dragging: false,
            last_cursor: None,
            status: "decoding full RAW mosaic…".into(),
            pipeline: if has_initial_path {
                PipelineSnapshot::waiting(generation, raw_kind)
            } else {
                PipelineSnapshot::idle(generation)
            },
            pending_first_present: None,
        }
    }

    fn drain_load_events(&mut self) {
        loop {
            let event = match self.receiver.try_recv() {
                Ok(event) => event,
                Err(TryRecvError::Empty) | Err(TryRecvError::Disconnected) => break,
            };
            match event {
                LoadEvent::Progress {
                    generation,
                    raw_kind,
                    cache_route,
                    timings,
                } => {
                    if generation != self.foreground_loader.current_generation() {
                        continue;
                    }
                    self.pending_first_present = None;
                    self.pipeline =
                        PipelineSnapshot::from_frontend(generation, raw_kind, cache_route, timings);
                    if let Some(telemetry) = self.telemetry.as_mut() {
                        telemetry.set_pipeline(&self.pipeline);
                    }
                }
                LoadEvent::Ready {
                    generation,
                    mosaic,
                    raw_kind,
                    cache_route,
                    elapsed,
                    requested_at,
                    ready_published_at,
                    frontend,
                    decode,
                } => {
                    if generation != self.foreground_loader.current_generation() {
                        continue;
                    }
                    let cache_hit = cache_route == CacheRoute::Hit;
                    let ui_dispatch = ready_published_at.elapsed();
                    let mut pipeline = PipelineSnapshot::from_worker(
                        generation,
                        raw_kind,
                        cache_route,
                        frontend,
                        decode.as_ref(),
                        ui_dispatch,
                    );
                    let gpu_prepare_result: Option<Result<(GpuUploadTimings, Duration), String>> =
                        if let Some(gpu) = self.gpu.as_mut() {
                            match gpu.renderer.upload_mosaic(&gpu.device, &gpu.queue, &mosaic) {
                                Ok(upload_timings) => {
                                    self.view.viewport = [gpu.size.width as f32, gpu.size.height as f32];
                                    let view_uniform_started = Instant::now();
                                    gpu.renderer.update_view(&gpu.queue, self.view);
                                    Some(Ok((upload_timings, view_uniform_started.elapsed())))
                                }
                                Err(error) => Some(Err(error.to_string())),
                            }
                        } else {
                            None
                        };

                    let gallery_prefix = self
                        .gallery_index
                        .map(|index| format!("RAW {}/{}; ", index + 1, self.gallery.len()))
                        .unwrap_or_default();
                    self.status = if cache_hit {
                        format!(
                            "{gallery_prefix}full RAW ready from mosaic cache in {:.2?}",
                            elapsed
                        )
                    } else if let Some(decode) = decode {
                        format!(
                            "{gallery_prefix}full RAW ready; native CRX decode {:.2?}, total {:.2?}",
                            decode.raw_decode, elapsed
                        )
                    } else {
                        format!("{gallery_prefix}full RAW ready in {:.2?}", elapsed)
                    };

                    match gpu_prepare_result {
                        Some(Ok((upload_timings, view_uniform_elapsed))) => {
                            if pipeline.complete_gpu(upload_timings, view_uniform_elapsed) {
                                self.pending_first_present = Some(PendingFirstPresent {
                                    generation,
                                    requested_at,
                                });
                            } else {
                                log::warn!("incomplete pipeline data for generation {generation}");
                                self.pending_first_present = None;
                                pipeline.mark_failed();
                            }
                        }
                        Some(Err(error)) => {
                            self.status = format!("GPU upload rejected RAW: {error}");
                            self.pending_first_present = None;
                            pipeline.mark_failed();
                        }
                        None => {
                            self.pending_first_present = None;
                        }
                    }
                    self.pipeline = pipeline;
                    if let Some(telemetry) = self.telemetry.as_mut() {
                        telemetry.set_pipeline(&self.pipeline);
                    }
                    if let Some(index) = self.gallery_index {
                        self.raw_prefetcher
                            .finish_foreground_and_submit(&self.gallery, index);
                    } else {
                        self.raw_prefetcher.finish_foreground_and_submit(&[], 0);
                    }
                }
                LoadEvent::Failed { generation, error } => {
                    if generation == self.foreground_loader.current_generation() {
                        self.status = format!("RAW load failed: {error}");
                        self.pending_first_present = None;
                        if self.pipeline.generation() != generation {
                            self.pipeline =
                                PipelineSnapshot::waiting(generation, RawKind::from_path(&self.path));
                        }
                        self.pipeline.mark_failed();
                        if let Some(telemetry) = self.telemetry.as_mut() {
                            telemetry.set_pipeline(&self.pipeline);
                        }
                        self.raw_prefetcher.finish_foreground_and_submit(&[], 0);
                    }
                }
            }
            if let Some(window) = &self.window {
                window.set_title(&format!("Rrrah — {}", self.status));
                window.request_redraw();
            }
        }
    }

    fn update_view(&mut self) {
        if let Some(gpu) = self.gpu.as_mut() {
            gpu.renderer.update_view(&gpu.queue, self.view);
        }
    }

    fn open_dropped_path(&mut self, path: PathBuf) {
        let file_type = match fs::symlink_metadata(&path) {
            Ok(metadata) => metadata.file_type(),
            Err(error) => {
                self.set_status(format!("drop rejected: {error}"));
                return;
            }
        };
        if file_type.is_symlink() {
            self.set_status("drop rejected: symlinks are not opened".into());
            return;
        }
        if file_type.is_dir() {
            let files = gallery::scan_folder(&path);
            if files.is_empty() {
                self.set_status(format!("no supported CR3 or DNG files in {}", path.display()));
                return;
            }
            self.gallery = files;
            self.open_gallery_index(0);
        } else if file_type.is_file() && is_supported_raw(&path) {
            self.gallery = path
                .parent()
                .map(gallery::scan_folder)
                .filter(|files| !files.is_empty())
                .unwrap_or_else(|| vec![path.clone()]);
            let index = self
                .gallery
                .iter()
                .position(|candidate| candidate == &path)
                .unwrap_or(0);
            self.open_gallery_index(index);
        } else {
            self.set_status("drop rejected: expected a supported CR3/DNG file or folder".into());
        }
    }

    fn open_gallery_index(&mut self, index: usize) {
        let Some(path) = self.gallery.get(index).cloned() else {
            return;
        };
        self.gallery_index = Some(index);
        self.raw_prefetcher.begin_foreground();
        self.path.clone_from(&path);
        if let Some(gpu) = self.gpu.as_ref() {
            self.view = ViewParameters {
                viewport: [gpu.size.width as f32, gpu.size.height as f32],
                ..ViewParameters::default()
            };
        }
        let generation = match self.foreground_loader.submit(path.clone()) {
            Ok(generation) => generation,
            Err(error) => {
                self.set_status(format!("RAW loader failed: {error}"));
                return;
            }
        };
        self.pending_first_present = None;
        self.pipeline = PipelineSnapshot::waiting(generation, RawKind::from_path(&path));
        if let Some(telemetry) = self.telemetry.as_mut() {
            telemetry.set_pipeline(&self.pipeline);
        }
        let position = format!("{}/{}", index + 1, self.gallery.len());
        self.set_status(format!("decoding RAW {position}: {}", display_name(&path)));
    }

    fn navigate_gallery(&mut self, direction: isize) {
        let Some(current) = self.gallery_index else {
            return;
        };
        let next = match direction {
            -1 => current.checked_sub(1),
            1 => current.checked_add(1),
            _ => None,
        };
        if let Some(next) = next.filter(|next| *next < self.gallery.len()) {
            self.open_gallery_index(next);
        }
    }

    fn set_status(&mut self, status: String) {
        self.status = status;
        if let Some(window) = &self.window {
            window.set_title(&format!("Rrrah — {}", self.status));
            window.request_redraw();
        }
    }
}

fn is_supported_raw(path: &Path) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| {
            extension.eq_ignore_ascii_case("cr3")
                || extension.eq_ignore_ascii_case("dng")
                || extension.eq_ignore_ascii_case("tif")
                || extension.eq_ignore_ascii_case("tiff")
        })
}

fn display_name(path: &Path) -> String {
    path.file_name().map_or_else(
        || path.display().to_string(),
        |name| name.to_string_lossy().into_owned(),
    )
}

impl ApplicationHandler<WakeEvent> for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        let window = match event_loop
            .create_window(Window::default_attributes().with_title("Rrrah — decoding full RAW…"))
        {
            Ok(window) => Arc::new(window),
            Err(error) => {
                log::error!("failed to create viewer window: {error}");
                self.status = format!("window creation failed: {error}");
                event_loop.exit();
                return;
            }
        };
        let gpu = match pollster::block_on(GpuState::new(event_loop.owned_display_handle(), window.clone())) {
            Ok(gpu) => gpu,
            Err(error) => {
                log::error!("GPU initialization failed: {error:#}");
                window.set_title(&format!("Rrrah — GPU unavailable: {error:#}"));
                self.window = Some(window);
                self.status = format!("GPU initialization failed: {error:#}");
                event_loop.exit();
                return;
            }
        };
        self.view.viewport = [gpu.size.width as f32, gpu.size.height as f32];
        self.window = Some(window.clone());
        self.gpu = Some(gpu);
        match event_loop.create_window(
            Window::default_attributes()
                .with_title("Rrrah — processing pipeline")
                .with_inner_size(LogicalSize::new(1600.0, 1080.0))
                .with_min_inner_size(LogicalSize::new(1180.0, 820.0)),
        ) {
            Ok(telemetry_window) => {
                let telemetry_window = Arc::new(telemetry_window);
                match pollster::block_on(TelemetryState::new(
                    event_loop.owned_display_handle(),
                    telemetry_window.clone(),
                )) {
                    Ok(mut telemetry) => {
                        telemetry.set_pipeline(&self.pipeline);
                        telemetry.set_cache_snapshot(self.cache_telemetry.snapshot());
                        self.telemetry_window = Some(telemetry_window);
                        self.telemetry = Some(telemetry);
                    }
                    Err(error) => {
                        log::warn!("timing window GPU initialization failed: {error:#}");
                    }
                }
            }
            Err(error) => log::warn!("timing window creation failed: {error}"),
        }
        window.request_redraw();
        if let Some(telemetry_window) = &self.telemetry_window {
            telemetry_window.request_redraw();
        }
    }

    fn about_to_wait(&mut self, _event_loop: &ActiveEventLoop) {
        self.drain_load_events();
    }

    fn user_event(&mut self, _event_loop: &ActiveEventLoop, _event: WakeEvent) {
        self.drain_load_events();
        if let Some(window) = &self.window {
            window.request_redraw();
        }
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, id: WindowId, event: WindowEvent) {
        if self
            .telemetry_window
            .as_ref()
            .is_some_and(|window| window.id() == id)
        {
            match event {
                WindowEvent::CloseRequested => {
                    self.telemetry_window = None;
                    self.telemetry = None;
                }
                WindowEvent::Resized(size) => {
                    if let Some(telemetry) = self.telemetry.as_mut() {
                        telemetry.resize(size);
                    }
                }
                WindowEvent::RedrawRequested => {
                    if let Some(telemetry) = self.telemetry.as_mut() {
                        telemetry.set_cache_snapshot(self.cache_telemetry.snapshot());
                        telemetry.render();
                    }
                }
                _ => {}
            }
            return;
        }
        match event {
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::HoveredFile(path) => {
                self.set_status(format!("drop ready: {}", display_name(&path)));
            }
            WindowEvent::HoveredFileCancelled => {
                self.set_status("drop cancelled".into());
            }
            WindowEvent::DroppedFile(path) => {
                log::info!("received dropped path: {}", path.display());
                self.open_dropped_path(path);
            }
            WindowEvent::Resized(size) => {
                if let Some(gpu) = self.gpu.as_mut() {
                    gpu.resize(size);
                    self.view.viewport = [size.width as f32, size.height as f32];
                    gpu.renderer.update_view(&gpu.queue, self.view);
                }
            }
            WindowEvent::RedrawRequested => {
                let display_submit = self.gpu.as_mut().and_then(GpuState::render);
                if let Some(display_submit) = display_submit {
                    if let Some(pending) = self.pending_first_present.take() {
                        let current_generation = self.foreground_loader.current_generation();
                        if pending.generation == current_generation
                            && self.pipeline.generation() == pending.generation
                            && self
                                .pipeline
                                .complete_display(display_submit, pending.requested_at.elapsed())
                        {
                            if let Some(telemetry) = self.telemetry.as_mut() {
                                telemetry.set_pipeline(&self.pipeline);
                            }
                        }
                    }
                }
                if let Some(telemetry_window) = &self.telemetry_window {
                    telemetry_window.request_redraw();
                }
            }
            WindowEvent::CursorMoved { position, .. } => {
                if self.dragging {
                    if let Some(previous) = self.last_cursor {
                        self.view.pan[0] += (position.x - previous.x) as f32;
                        self.view.pan[1] += (position.y - previous.y) as f32;
                        self.update_view();
                    }
                }
                self.last_cursor = Some(position);
            }
            WindowEvent::MouseInput { state, button, .. } if button == MouseButton::Left => {
                self.dragging = state == ElementState::Pressed;
                if !self.dragging {
                    self.last_cursor = None;
                }
            }
            WindowEvent::MouseWheel { delta, .. } => {
                let amount = match delta {
                    MouseScrollDelta::LineDelta(_, y) => y,
                    MouseScrollDelta::PixelDelta(position) => position.y as f32 / 80.0,
                };
                self.view.zoom = (self.view.zoom * 1.12_f32.powf(amount)).clamp(0.02, 128.0);
                self.update_view();
            }
            WindowEvent::KeyboardInput { event, .. }
                if event.state == ElementState::Pressed && !event.repeat =>
            {
                match event.physical_key {
                    PhysicalKey::Code(KeyCode::ArrowLeft) => self.navigate_gallery(-1),
                    PhysicalKey::Code(KeyCode::ArrowRight) => self.navigate_gallery(1),
                    PhysicalKey::Code(KeyCode::KeyF) => {
                        self.view.zoom = 1.0;
                        self.view.pan = [0.0, 0.0];
                        self.update_view();
                    }
                    PhysicalKey::Code(KeyCode::KeyR) => {
                        self.view = ViewParameters {
                            viewport: self.view.viewport,
                            ..ViewParameters::default()
                        };
                        self.update_view();
                    }
                    PhysicalKey::Code(KeyCode::Equal) | PhysicalKey::Code(KeyCode::NumpadAdd) => {
                        self.view.exposure_stops = (self.view.exposure_stops + 0.25).min(10.0);
                        self.update_view();
                    }
                    PhysicalKey::Code(KeyCode::Minus) | PhysicalKey::Code(KeyCode::NumpadSubtract) => {
                        self.view.exposure_stops = (self.view.exposure_stops - 0.25).max(-10.0);
                        self.update_view();
                    }
                    _ => {}
                }
            }
            _ => {}
        }
    }
}

struct TelemetryState {
    _instance: wgpu::Instance,
    window: Arc<Window>,
    surface: wgpu::Surface<'static>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,
    size: PhysicalSize<u32>,
    hud: HudRenderer,
    pipeline: PipelineSnapshot,
    cache_snapshot: Option<CacheTelemetrySnapshot>,
}

impl TelemetryState {
    async fn new(display: OwnedDisplayHandle, window: Arc<Window>) -> Result<Self> {
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_with_display_handle(Box::new(
            display,
        )));
        let surface = instance
            .create_surface(window.clone())
            .context("create telemetry wgpu surface")?;
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::LowPower,
                compatible_surface: Some(&surface),
                force_fallback_adapter: false,
                apply_limit_buckets: false,
            })
            .await
            .context("request telemetry GPU adapter")?;
        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor::default())
            .await
            .context("request telemetry GPU device")?;
        let capabilities = surface.get_capabilities(&adapter);
        let surface_format = capabilities
            .formats
            .iter()
            .copied()
            .find(wgpu::TextureFormat::is_srgb)
            .unwrap_or(capabilities.formats[0]);
        let size = window.inner_size();
        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format: surface_format,
            view_formats: vec![],
            alpha_mode: capabilities.alpha_modes[0],
            width: size.width.max(1),
            height: size.height.max(1),
            present_mode: wgpu::PresentMode::AutoVsync,
            desired_maximum_frame_latency: 2,
            color_space: wgpu::SurfaceColorSpace::Auto,
        };
        surface.configure(&device, &config);
        let hud = HudRenderer::new(&device, surface_format, [size.width as f32, size.height as f32]);
        let mut state = Self {
            _instance: instance,
            window,
            surface,
            device,
            queue,
            config,
            size,
            hud,
            pipeline: PipelineSnapshot::idle(0),
            cache_snapshot: None,
        };
        state.rebuild_hud();
        Ok(state)
    }

    fn set_pipeline(&mut self, pipeline: &PipelineSnapshot) {
        if self.pipeline == *pipeline {
            return;
        }
        self.pipeline.clone_from(pipeline);
        self.rebuild_hud();
        self.window.request_redraw();
    }

    fn set_cache_snapshot(&mut self, snapshot: CacheTelemetrySnapshot) {
        if self.cache_snapshot == Some(snapshot) {
            return;
        }
        self.cache_snapshot = Some(snapshot);
        self.rebuild_hud();
        self.window.request_redraw();
    }

    fn rebuild_hud(&mut self) {
        let pipeline_cards = self.pipeline.cards();
        let cache_hud = self.cache_snapshot.map(CacheTelemetrySnapshot::format_hud);
        let cache_current = cache_hud
            .as_deref()
            .and_then(|text| text.lines().nth(1))
            .unwrap_or("CACHE STATUS WAITING");
        let cache_session = cache_hud
            .as_deref()
            .and_then(|text| text.lines().nth(2))
            .unwrap_or("SESSION --");
        let cache_description = pipeline_cards
            .iter()
            .find(|card| card.cache_footer)
            .map(|card| format!("{} / {} / {}", card.description, cache_current, cache_session));
        let cards: Vec<_> = pipeline_cards
            .iter()
            .map(|card| {
                let accent = match card.state {
                    PipelineStageState::Pending => [0.36, 0.45, 0.58, 0.96],
                    PipelineStageState::Running => [0.96, 0.64, 0.16, 0.96],
                    PipelineStageState::Measured(_) => [0.14, 0.72, 0.48, 0.96],
                    PipelineStageState::Shared(_) => [0.16, 0.68, 0.76, 0.96],
                    PipelineStageState::Conditional(_) => [0.84, 0.52, 0.18, 0.96],
                    PipelineStageState::NotTimed => [0.62, 0.42, 0.88, 0.96],
                    PipelineStageState::Skipped => [0.32, 0.58, 0.86, 0.96],
                    PipelineStageState::Failed => [0.88, 0.24, 0.26, 0.96],
                };
                let description = if card.cache_footer {
                    cache_description.as_deref().unwrap_or(card.description)
                } else {
                    card.description
                };
                HudCard::new(&card.title, &card.time, description)
                    .with_status(&card.status)
                    .with_accent(accent)
            })
            .collect();
        self.hud.update_cards(&self.device, &self.queue, &cards);
    }

    fn resize(&mut self, size: PhysicalSize<u32>) {
        if size.width == 0 || size.height == 0 {
            return;
        }
        self.size = size;
        self.config.width = size.width;
        self.config.height = size.height;
        self.surface.configure(&self.device, &self.config);
        self.hud
            .resize(&self.queue, [size.width as f32, size.height as f32]);
        self.rebuild_hud();
    }

    fn render(&mut self) {
        let output = match self.surface.get_current_texture() {
            wgpu::CurrentSurfaceTexture::Success(texture)
            | wgpu::CurrentSurfaceTexture::Suboptimal(texture) => texture,
            wgpu::CurrentSurfaceTexture::Outdated | wgpu::CurrentSurfaceTexture::Lost => {
                self.surface.configure(&self.device, &self.config);
                return;
            }
            wgpu::CurrentSurfaceTexture::Timeout | wgpu::CurrentSurfaceTexture::Occluded => return,
            wgpu::CurrentSurfaceTexture::Validation => return,
        };
        let view = output
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("Rrrah telemetry frame encoder"),
            });
        {
            let _clear_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Rrrah telemetry clear"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: 0.008,
                            g: 0.012,
                            b: 0.02,
                            a: 1.0,
                        }),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
        }
        self.hud.encode(&mut encoder, &view);
        self.queue.submit([encoder.finish()]);
        self.window.pre_present_notify();
        self.queue.present(output);
        self.window.request_redraw();
    }
}

struct GpuState {
    _instance: wgpu::Instance,
    window: Arc<Window>,
    surface: wgpu::Surface<'static>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,
    size: PhysicalSize<u32>,
    renderer: RawRenderer,
}

impl GpuState {
    async fn new(display: OwnedDisplayHandle, window: Arc<Window>) -> Result<Self> {
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_with_display_handle(Box::new(
            display,
        )));
        let surface = instance
            .create_surface(window.clone())
            .context("create wgpu surface")?;
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                compatible_surface: Some(&surface),
                force_fallback_adapter: false,
                apply_limit_buckets: false,
            })
            .await
            .context("request GPU adapter")?;
        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor::default())
            .await
            .context("request GPU device")?;
        let capabilities = surface.get_capabilities(&adapter);
        let surface_format = capabilities
            .formats
            .iter()
            .copied()
            .find(wgpu::TextureFormat::is_srgb)
            .unwrap_or(capabilities.formats[0]);
        let size = window.inner_size();
        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format: surface_format,
            view_formats: vec![],
            alpha_mode: capabilities.alpha_modes[0],
            width: size.width.max(1),
            height: size.height.max(1),
            present_mode: wgpu::PresentMode::AutoVsync,
            desired_maximum_frame_latency: 2,
            color_space: wgpu::SurfaceColorSpace::Auto,
        };
        surface.configure(&device, &config);
        let renderer = RawRenderer::new(&device, surface_format);
        Ok(Self {
            _instance: instance,
            window,
            surface,
            device,
            queue,
            config,
            size,
            renderer,
        })
    }

    fn resize(&mut self, size: PhysicalSize<u32>) {
        if size.width == 0 || size.height == 0 {
            return;
        }
        self.size = size;
        self.config.width = size.width;
        self.config.height = size.height;
        self.surface.configure(&self.device, &self.config);
    }

    fn render(&mut self) -> Option<FrameSubmitTimings> {
        let total_started = Instant::now();
        let surface_acquire_started = Instant::now();
        let output = match self.surface.get_current_texture() {
            wgpu::CurrentSurfaceTexture::Success(texture)
            | wgpu::CurrentSurfaceTexture::Suboptimal(texture) => texture,
            wgpu::CurrentSurfaceTexture::Outdated | wgpu::CurrentSurfaceTexture::Lost => {
                self.surface.configure(&self.device, &self.config);
                self.window.request_redraw();
                return None;
            }
            wgpu::CurrentSurfaceTexture::Timeout => {
                self.window.request_redraw();
                return None;
            }
            wgpu::CurrentSurfaceTexture::Occluded => return None,
            wgpu::CurrentSurfaceTexture::Validation => return None,
        };
        let surface_acquire = surface_acquire_started.elapsed();
        let frame_encode_started = Instant::now();
        let view = output
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("Rrrah frame encoder"),
            });
        self.renderer.encode(&mut encoder, &view);
        let command_buffer = encoder.finish();
        let frame_encode = frame_encode_started.elapsed();
        let queue_submit_started = Instant::now();
        self.queue.submit([command_buffer]);
        let queue_submit = queue_submit_started.elapsed();
        let present_request_started = Instant::now();
        self.window.pre_present_notify();
        self.queue.present(output);
        let present_request = present_request_started.elapsed();
        Some(FrameSubmitTimings {
            surface_acquire,
            frame_encode,
            queue_submit,
            present_request,
            total: total_started.elapsed(),
        })
    }
}

#[cfg(test)]
mod foreground_loader_tests {
    use super::*;

    #[test]
    fn rapid_submissions_keep_only_the_latest_pending_request() {
        let (tx, worker_rx) = bounded(1);
        let loader = ForegroundLoader {
            tx,
            pending: worker_rx.clone(),
            generation: Arc::new(AtomicU64::new(0)),
            decode_gate: Arc::new(DecodeGate::new()),
            telemetry: Arc::new(CacheTelemetry::new(true, 1024)),
        };

        loader.submit_initial(PathBuf::from("0.cr3")).unwrap();
        let active = worker_rx.try_recv().unwrap();
        let active_token = GenerationToken::new(Arc::clone(&loader.generation), active.generation);

        loader.submit(PathBuf::from("1.cr3")).unwrap();
        loader.submit(PathBuf::from("2.cr3")).unwrap();

        assert!(active_token.is_cancelled());
        let pending = worker_rx.try_recv().unwrap();
        assert_eq!(pending.path, PathBuf::from("2.cr3"));
        assert_eq!(pending.generation, loader.current_generation());
        assert!(worker_rx.try_recv().is_err());
    }
}
