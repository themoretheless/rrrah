#![allow(dead_code)]
//! Folder gallery model and bounded thumbnail scheduling.
//!
//! The gallery deliberately keeps filesystem work off the winit thread.  The
//! UI owns `GalleryModel`; a worker can consume `ThumbnailJob`s and publish
//! `ThumbnailReady` messages without touching wgpu resources.

use crate::{
    cache_telemetry::{CacheTelemetry, PrefetchPhase},
    decode_gate::DecodeGate,
};
use crossbeam_channel::{Receiver, Sender, bounded};
use rrrah_cache::{CacheKey, SourceFingerprint};
use rrrah_decode::{DecodeRequest, GenerationToken, NativeRawDecoder, RawDecoder};
use std::{
    collections::BinaryHeap,
    path::{Path, PathBuf},
    sync::{
        Arc, Mutex,
        atomic::{AtomicU8, AtomicU64, Ordering},
    },
    thread,
    time::Duration,
};

pub const MAX_ITEMS: usize = 10_000;
pub const THUMB_EDGE: u32 = 256;
/// Number of neighbours decoded ahead/behind the current frame.
pub const PREFETCH_BEHIND: usize = 2;
pub const PREFETCH_AHEAD: usize = 5;
#[derive(Debug, Clone, Copy)]
pub struct PrefetchWindow {
    pub behind: usize,
    pub ahead: usize,
}
impl Default for PrefetchWindow {
    fn default() -> Self {
        Self {
            behind: PREFETCH_BEHIND,
            ahead: PREFETCH_AHEAD,
        }
    }
}
const RAW_PREFETCH_FOREGROUND: u8 = 1 << 0;
const RAW_PREFETCH_STORE_ADMITTED: u8 = 1 << 1;

/// Direction of the most recent gallery navigation. The prefetch window is
/// biased toward the direction of travel: backward navigation swaps the
/// behind/ahead extents so revisited frames are warmed first.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum NavDirection {
    #[default]
    None,
    Forward,
    Backward,
}

impl NavDirection {
    /// `(behind, ahead)` prefetch extents for this direction of travel.
    fn window(self, configured: PrefetchWindow) -> (usize, usize) {
        match self {
            Self::None | Self::Forward => (configured.behind, configured.ahead),
            Self::Backward => (configured.ahead, configured.behind),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GalleryItem {
    pub path: PathBuf,
    pub thumbnail: Option<PathBuf>,
}

#[derive(Debug, Default)]
pub struct GalleryModel {
    pub items: Vec<GalleryItem>,
    pub selected: usize,
}

impl GalleryModel {
    pub fn replace_folder(&mut self, folder: &Path) {
        self.items = scan_folder(folder)
            .into_iter()
            .map(|path| GalleryItem {
                path,
                thumbnail: None,
            })
            .collect();
        self.selected = 0;
    }

    pub fn select(&mut self, index: usize) -> Option<&Path> {
        if index < self.items.len() {
            self.selected = index;
            return Some(&self.items[index].path);
        }
        None
    }

    /// Prioritized jobs: caller should enqueue these before distant items.
    pub fn jobs(&self, center: usize, radius: usize) -> impl Iterator<Item = ThumbnailJob> + '_ {
        let start = if center < self.items.len() {
            center.saturating_sub(radius)
        } else {
            self.items.len()
        };
        let end = center
            .saturating_add(radius)
            .saturating_add(1)
            .min(self.items.len());
        (start..end).map(|index| ThumbnailJob {
            index,
            source: self.items[index].path.clone(),
            edge: THUMB_EDGE,
        })
    }

    /// Ordered window used by the background prefetcher: current, two frames
    /// behind, then five ahead. This keeps navigation latency low while
    /// bounding work.
    pub fn prefetch_jobs(&self, center: usize) -> Vec<ThumbnailJob> {
        if center >= self.items.len() {
            return Vec::new();
        }
        let mut out = Vec::with_capacity(PREFETCH_BEHIND + PREFETCH_AHEAD + 1);
        if center < self.items.len() {
            out.push(ThumbnailJob {
                index: center,
                source: self.items[center].path.clone(),
                edge: THUMB_EDGE,
            });
        }
        for delta in 1..=PREFETCH_BEHIND {
            let Some(index) = center.checked_sub(delta) else {
                break;
            };
            out.push(ThumbnailJob {
                index,
                source: self.items[index].path.clone(),
                edge: THUMB_EDGE,
            });
        }
        for index in
            center.saturating_add(1)..(center.saturating_add(PREFETCH_AHEAD + 1)).min(self.items.len())
        {
            out.push(ThumbnailJob {
                index,
                source: self.items[index].path.clone(),
                edge: THUMB_EDGE,
            });
        }
        out
    }

    /// Build a deterministic, de-duplicated prefetch plan.  The selected item
    /// is always first, followed by the two previous frames and then five
    /// following frames. The plan is bounded to eight jobs and carries a
    /// generation token so stale worker results can
    /// be discarded after a folder switch.
    pub fn prefetch_plan(&self, center: usize, generation: u64) -> Vec<PrefetchJob> {
        if center >= self.items.len() {
            return Vec::new();
        }
        let mut indices = Vec::with_capacity(PREFETCH_AHEAD + PREFETCH_BEHIND + 1);
        indices.push(center);
        for delta in 1..=PREFETCH_BEHIND {
            if let Some(index) = center.checked_sub(delta) {
                indices.push(index);
            }
        }
        for delta in 1..=PREFETCH_AHEAD {
            if let Some(index) = center.checked_add(delta).filter(|&i| i < self.items.len()) {
                indices.push(index);
            }
        }
        indices
            .into_iter()
            .map(|index| PrefetchJob {
                generation,
                priority: u8::from(index != center),
                thumbnail: ThumbnailJob {
                    index,
                    source: self.items[index].path.clone(),
                    edge: THUMB_EDGE,
                },
            })
            .collect()
    }
}

#[derive(Debug)]
struct RawPrefetchCommand {
    generation: u64,
    paths: Vec<PathBuf>,
}

/// A single low-priority full-RAW cache warmer.
///
/// It intentionally retains no decoded mosaics: each neighbour is decoded,
/// written to the existing atomic disk cache, and dropped. This keeps RAM
/// bounded even for 50+ MP files while making later foreground opens a cache
/// read. A new selection replaces pending work and invalidates the in-flight
/// result before it can be persisted.
pub struct RawPrefetcher {
    lifetime: Arc<AtomicU64>,
    tx: Sender<RawPrefetchCommand>,
    pending: Receiver<RawPrefetchCommand>,
    generation: Arc<AtomicU64>,
    state: Arc<AtomicU8>,
    decode_gate: Arc<DecodeGate>,
    telemetry: Arc<CacheTelemetry>,
    enabled: bool,
    window: PrefetchWindow,
}

struct RawStoreAdmission {
    state: Arc<AtomicU8>,
}

impl RawStoreAdmission {
    fn try_acquire(state: &Arc<AtomicU8>) -> Option<Self> {
        state
            .compare_exchange(
                0,
                RAW_PREFETCH_STORE_ADMITTED,
                Ordering::AcqRel,
                Ordering::Acquire,
            )
            .ok()
            .map(|_| Self {
                state: Arc::clone(state),
            })
    }
}

impl Drop for RawStoreAdmission {
    fn drop(&mut self) {
        self.state
            .fetch_and(!RAW_PREFETCH_STORE_ADMITTED, Ordering::Release);
    }
}

impl Drop for RawPrefetcher {
    fn drop(&mut self) {
        self.state.fetch_or(RAW_PREFETCH_FOREGROUND, Ordering::AcqRel);
        self.lifetime.fetch_add(1, Ordering::AcqRel);
        while self.pending.try_recv().is_ok() {}
    }
}

impl RawPrefetcher {
    #[cfg(test)]
    pub fn new(
        cache_root: Option<PathBuf>,
        no_cache: bool,
        decode_gate: Arc<DecodeGate>,
        telemetry: Arc<CacheTelemetry>,
        window: PrefetchWindow,
    ) -> Self {
        Self::with_limits(
            cache_root,
            no_cache,
            decode_gate,
            telemetry,
            window,
            rrrah_cache::CacheLimits::bytes(rrrah_cache::DEFAULT_MAX_DISK_CACHE_BYTES),
        )
    }
    #[cfg(test)]
    pub fn with_limits(
        cache_root: Option<PathBuf>,
        no_cache: bool,
        decode_gate: Arc<DecodeGate>,
        telemetry: Arc<CacheTelemetry>,
        window: PrefetchWindow,
        limits: rrrah_cache::CacheLimits,
    ) -> Self {
        Self::with_budget(cache_root, no_cache, decode_gate, telemetry, window, limits, None)
    }
    pub fn with_budget(
        cache_root: Option<PathBuf>,
        no_cache: bool,
        decode_gate: Arc<DecodeGate>,
        telemetry: Arc<CacheTelemetry>,
        window: PrefetchWindow,
        limits: rrrah_cache::CacheLimits,
        managed_budget: Option<rrrah_cache::MemoryBudget>,
    ) -> Self {
        let (tx, commands) = bounded::<RawPrefetchCommand>(1);
        let pending = commands.clone();
        let lifetime = Arc::new(AtomicU64::new(0));
        let worker_lifetime = GenerationToken::new(Arc::clone(&lifetime), 0);
        let generation = decode_gate.speculative_generation();
        let state = Arc::new(AtomicU8::new(0));
        let mut enabled = cache_root.is_some() && !no_cache;

        if let Some(cache_root) = cache_root.filter(|_| !no_cache) {
            let worker_generation = Arc::clone(&generation);
            let worker_state = Arc::clone(&state);
            let worker_gate = Arc::clone(&decode_gate);
            let worker_telemetry = Arc::clone(&telemetry);
            let spawn = thread::Builder::new()
                .name("rrrah-raw-prefetch".into())
                .spawn(move || {
                    let cache = rrrah_cache::DiskMosaicCache::with_max_bytes(cache_root, limits.max_bytes)
                        .with_max_entries(limits.max_entries)
                        .with_ttl(limits.ttl);
                    while let Ok(command) = commands.recv() {
                        let token = GenerationToken::new(Arc::clone(&worker_generation), command.generation)
                            .combine(&worker_lifetime);
                        let mut protected = Vec::new();
                        // Do not let the lower-priority tail of this plan evict
                        // the neighbours we just warmed in a count-limited cache.
                        for path in command.paths {
                            // Count limits constrain retained neighbours, not attempts.
                            // A missing/failed closest file must not consume the slot
                            // intended for the next usable neighbour.
                            if protected.len() >= limits.max_entries.unwrap_or(usize::MAX) {
                                break;
                            }
                            worker_telemetry.set_prefetch_phase(command.generation, PrefetchPhase::Checking);
                            while worker_state.load(Ordering::Acquire) & RAW_PREFETCH_FOREGROUND != 0 {
                                if token.is_cancelled() {
                                    break;
                                }
                                thread::sleep(Duration::from_millis(10));
                            }
                            if token.is_cancelled() {
                                break;
                            }
                            let Ok(fingerprint) = SourceFingerprint::from_path(&path) else {
                                worker_telemetry.record_prefetch_failure(command.generation);
                                continue;
                            };
                            let mut recipe_request = DecodeRequest::new(&path);
                            recipe_request.cancellation = Some(token.clone());
                            if path.extension().is_some_and(|extension| {
                                extension.eq_ignore_ascii_case("tif")
                                    || extension.eq_ignore_ascii_case("tiff")
                            }) {
                                match rrrah_decode::image_source_kind(&recipe_request) {
                                    Ok(rrrah_decode::ImageSourceKind::Raster) => {
                                        worker_telemetry.record_prefetch_skipped(command.generation);
                                        continue;
                                    }
                                    Ok(rrrah_decode::ImageSourceKind::Sensor) => {}
                                    Err(_) => {
                                        worker_telemetry.record_prefetch_failure(command.generation);
                                        continue;
                                    }
                                }
                            }
                            let Ok(recipe) = NativeRawDecoder.mosaic_recipe(&recipe_request) else {
                                worker_telemetry.record_prefetch_failure(command.generation);
                                continue;
                            };
                            let key = CacheKey::for_mosaic_recipe(&fingerprint, 0, recipe);
                            if cache.contains(key) {
                                // `contains` is only a presence probe. The HUD
                                // labels this PRESENT rather than HIT because
                                // checksum validation happens on foreground load.
                                worker_telemetry.record_prefetch_cached(command.generation);
                                protected.push(key);
                                continue;
                            }
                            let cancelled = || {
                                token.is_cancelled()
                                    || worker_state.load(Ordering::Acquire) & RAW_PREFETCH_FOREGROUND != 0
                            };
                            let Some(decode_permit) = worker_gate.acquire_prefetch(cancelled) else {
                                break;
                            };
                            // A foreground cache read may have populated this
                            // path while the speculative worker waited for the
                            // shared decoder permit.
                            if token.is_cancelled()
                                || worker_state.load(Ordering::Acquire) & RAW_PREFETCH_FOREGROUND != 0
                                || cache.contains(key)
                            {
                                if !token.is_cancelled()
                                    && worker_state.load(Ordering::Acquire) & RAW_PREFETCH_FOREGROUND == 0
                                {
                                    worker_telemetry.record_prefetch_cached(command.generation);
                                    protected.push(key);
                                }
                                continue;
                            }
                            worker_telemetry.set_prefetch_phase(command.generation, PrefetchPhase::Decoding);
                            let mut request = recipe_request;
                            request.memory_budget = managed_budget.clone();
                            let Ok(output) = NativeRawDecoder.decode(&request) else {
                                worker_telemetry.record_prefetch_failure(command.generation);
                                continue;
                            };
                            // Atomic cache publication can include a slow
                            // fsync. It has separate admission below and must
                            // not make foreground wait for the decode permit.
                            drop(decode_permit);
                            let Some(_store_admission) = RawStoreAdmission::try_acquire(&worker_state) else {
                                continue;
                            };
                            if worker_state.load(Ordering::Acquire) & RAW_PREFETCH_FOREGROUND != 0
                                || token.is_cancelled()
                            {
                                continue;
                            }
                            worker_telemetry.set_prefetch_phase(command.generation, PrefetchPhase::Writing);
                            let mosaic_bytes = u64::try_from(output.mosaic.byte_len()).unwrap_or(u64::MAX);
                            match cache.store_with_cancel_preserving(key, &output.mosaic, &protected, || {
                                token.is_cancelled()
                                    || worker_state.load(Ordering::Acquire) & RAW_PREFETCH_FOREGROUND != 0
                            }) {
                                Ok(_) => {
                                    protected.push(key);
                                    worker_telemetry.record_prefetch_stored(command.generation, mosaic_bytes);
                                    log::debug!("prefetched full RAW mosaic: {}", path.display());
                                }
                                Err(rrrah_cache::CacheError::Cancelled) => break,
                                Err(error) => {
                                    worker_telemetry.record_prefetch_failure(command.generation);
                                    let disk_pressure = error.is_disk_pressure();
                                    log::warn!(
                                        "RAW prefetch cache write failed for {}: {error}",
                                        path.display()
                                    );
                                    if disk_pressure {
                                        break;
                                    }
                                }
                            }
                        }
                        if !token.is_cancelled() {
                            match cache.usage() {
                                Ok(usage) => worker_telemetry.update_disk_usage(usage),
                                Err(_) => worker_telemetry.record_disk_scan_error(),
                            }
                            worker_telemetry.finish_prefetch(command.generation);
                        }
                    }
                });
            if let Err(error) = spawn {
                enabled = false;
                telemetry.disable_prefetch();
                log::warn!("failed to start RAW prefetch worker: {error}");
            }
        }

        Self {
            lifetime,
            tx,
            pending,
            generation,
            state,
            decode_gate,
            telemetry,
            enabled,
            window,
        }
    }

    /// Immediately invalidates queued/in-flight speculative work. Cancellation
    /// is checked before cache publication, so stale data is never admitted.
    pub fn begin_foreground(&self) {
        // Mark foreground first. A cache store must atomically acquire state
        // from zero, so no new speculative store can pass this point. A store
        // already admitted may finish; waiting for fsync here would stall UI.
        self.state.fetch_or(RAW_PREFETCH_FOREGROUND, Ordering::AcqRel);
        self.telemetry().pause_prefetch();
        self.generation.fetch_add(1, Ordering::AcqRel);
        while self.pending.try_recv().is_ok() {}
    }

    /// Resume background work after the selected frame is ready. Foreground
    /// write-back owns the current frame; this worker warms the configured
    /// neighbours without racing to decode the current frame twice.
    pub fn finish_foreground_and_submit(
        &self,
        gallery: &[PathBuf],
        selected: usize,
        direction: NavDirection,
    ) {
        self.decode_gate.defer_prefetch();
        if !self.enabled {
            self.telemetry().disable_prefetch();
            self.state.fetch_and(!RAW_PREFETCH_FOREGROUND, Ordering::Release);
            return;
        }
        if selected >= gallery.len() {
            self.telemetry().idle_prefetch();
            self.state.fetch_and(!RAW_PREFETCH_FOREGROUND, Ordering::Release);
            return;
        }
        let generation = self.generation.fetch_add(1, Ordering::AcqRel).wrapping_add(1);
        let paths = neighbour_prefetch_paths(gallery, selected, direction, self.window)
            .into_iter()
            .filter(|path| rrrah_decode::is_supported_raw_path(path))
            .collect::<Vec<_>>();
        self.telemetry().begin_prefetch(generation, paths.len());
        while self.pending.try_recv().is_ok() {}
        if self
            .tx
            .try_send(RawPrefetchCommand { generation, paths })
            .is_err()
        {
            self.telemetry().record_prefetch_failure(generation);
        }
        // Publish the fresh generation and its bounded command before
        // allowing the worker to leave its foreground wait loop.
        self.state.fetch_and(!RAW_PREFETCH_FOREGROUND, Ordering::Release);
    }

    fn telemetry(&self) -> &CacheTelemetry {
        &self.telemetry
    }
}

#[cfg(test)]
fn raw_prefetch_paths(gallery: &[PathBuf], selected: usize, direction: NavDirection) -> Vec<PathBuf> {
    neighbour_prefetch_paths(gallery, selected, direction, PrefetchWindow::default())
}

pub fn neighbour_prefetch_paths(
    gallery: &[PathBuf],
    selected: usize,
    direction: NavDirection,
    window: PrefetchWindow,
) -> Vec<PathBuf> {
    if selected >= gallery.len() {
        return Vec::new();
    }
    let (behind, ahead) = direction.window(window);
    let behind = behind.min(selected);
    let ahead = ahead.min(gallery.len() - selected - 1);
    let mut paths = Vec::with_capacity(behind + ahead);
    // Keep nearest-first ordering within each side, but warm the direction
    // of travel before spending the decode budget on the opposite side.
    if direction == NavDirection::Forward {
        for delta in 1..=ahead {
            paths.push(gallery[selected + delta].clone());
        }
    }
    for delta in 1..=behind {
        if let Some(index) = selected.checked_sub(delta) {
            paths.push(gallery[index].clone());
        }
    }
    if direction != NavDirection::Forward {
        for delta in 1..=ahead {
            paths.push(gallery[selected + delta].clone());
        }
    }
    paths
}

#[derive(Debug, Clone)]
pub struct PrefetchJob {
    pub generation: u64,
    /// Lower values must be serviced first by the worker queue.
    pub priority: u8,
    pub thumbnail: ThumbnailJob,
}

#[derive(Debug, Clone)]
pub struct ThumbnailJob {
    pub index: usize,
    pub source: PathBuf,
    pub edge: u32,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SourceStamp {
    image: Option<rrrah_cache::SourceFingerprint>,
    palette: Option<rrrah_cache::SourceFingerprint>,
}
impl SourceStamp {
    pub fn is_readable(&self) -> bool {
        self.image.is_some()
    }
    pub fn read(path: &std::path::Path) -> Self {
        Self {
            image: rrrah_cache::SourceFingerprint::from_path(path).ok(),
            palette: rrrah_decode::wal_palette_path(path)
                .and_then(|p| rrrah_cache::SourceFingerprint::from_path(p).ok()),
        }
    }
}
#[derive(Debug, Clone)]
pub struct ThumbnailReady {
    pub source_stamp: SourceStamp,
    pub index: usize,
    /// CPU-side RGBA8 pixels; upload to a persistent texture atlas on UI side.
    pub width: u32,
    pub height: u32,
    pub pixels: rrrah_core::PixelBuffer<u8>,
}

/// Single background worker for thumbnail decoding. Submitting a new window
/// advances `generation`; stale jobs are discarded before and after decoding.
/// The bounded channel provides backpressure and prevents a large folder from
/// retaining thousands of pixel buffers.
pub struct Prefetcher {
    tx: Sender<(u64, ThumbnailJob)>,
    /// A receiver clone kept by the producer so a newer viewport can evict
    /// stale queued work before publishing its replacement window.
    pending: Receiver<(u64, ThumbnailJob)>,
    rx: Receiver<ThumbnailReady>,
    generation: Arc<AtomicU64>,
    /// Serializes generation changes with ready-result publication. Without
    /// this short critical section a worker could validate the old generation,
    /// lose the CPU immediately before `try_send`, and publish a stale result
    /// after `submit` had already drained the ready queue.
    publication: Arc<Mutex<()>>,
}

impl Prefetcher {
    pub fn new<F>(capacity: usize, loader: F) -> Self
    where
        F: Fn(ThumbnailJob) -> Option<ThumbnailReady> + Send + Sync + 'static,
    {
        Self::new_with_cancel(capacity, move |job, _| loader(job))
    }

    pub fn new_with_cancel<F>(capacity: usize, loader: F) -> Self
    where
        F: Fn(ThumbnailJob, GenerationToken) -> Option<ThumbnailReady> + Send + Sync + 'static,
    {
        let (tx, jobs) = bounded(capacity.max(1));
        let pending = jobs.clone();
        let (ready, rx) = bounded(capacity.max(1));
        let generation = Arc::new(AtomicU64::new(0));
        let current = Arc::clone(&generation);
        let publication = Arc::new(Mutex::new(()));
        let worker_publication = Arc::clone(&publication);
        let loader = Arc::new(loader);
        thread::spawn(move || {
            while let Ok((generation, job)) = jobs.recv() {
                if generation != current.load(Ordering::Acquire) {
                    continue;
                }
                if let Some(result) = loader(job, GenerationToken::new(Arc::clone(&current), generation)) {
                    let _publication = worker_publication
                        .lock()
                        .unwrap_or_else(std::sync::PoisonError::into_inner);
                    if generation == current.load(Ordering::Acquire) {
                        let _ = ready.try_send(result);
                    }
                }
            }
        });
        Self {
            tx,
            pending,
            rx,
            generation,
            publication,
        }
    }

    /// Cancel the previous window, replace its queued jobs, and enqueue at
    /// most `capacity` jobs from the newest viewport.
    pub fn submit(&self, jobs: impl IntoIterator<Item = ThumbnailJob>) {
        let _publication = self
            .publication
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let generation = self.generation.fetch_add(1, Ordering::AcqRel).wrapping_add(1);
        while self.pending.try_recv().is_ok() {}
        // Results from the previous folder/viewport are just as stale as its
        // pending decode jobs. Draining here also frees the bounded pixel
        // buffers before the newest generation starts publishing.
        while self.rx.try_recv().is_ok() {}
        for job in jobs
            .into_iter()
            .take(self.tx.capacity().expect("bounded prefetch queue"))
        {
            if self.tx.try_send((generation, job)).is_err() {
                break;
            }
        }
    }

    pub fn try_recv(&self) -> Option<ThumbnailReady> {
        let _publication = self
            .publication
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        self.rx.try_recv().ok()
    }
}

impl Drop for Prefetcher {
    fn drop(&mut self) {
        let _publication = self
            .publication
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        self.generation.fetch_add(1, Ordering::AcqRel);
        while self.pending.try_recv().is_ok() {}
        while self.rx.try_recv().is_ok() {}
    }
}

pub fn is_supported(path: &Path) -> bool {
    rrrah_decode::is_supported_image_path(path) || rrrah_decode::is_supported_model_path(path)
}

pub fn scan_folder(folder: &Path) -> Vec<PathBuf> {
    let Ok(entries) = std::fs::read_dir(folder) else {
        return Vec::new();
    };
    let paths = entries.filter_map(Result::ok).filter_map(|entry| {
        let path = entry.path();
        let metadata = std::fs::symlink_metadata(&path).ok()?;
        (metadata.file_type().is_file() && is_supported(&path)).then_some(path)
    });
    smallest_paths(paths, MAX_ITEMS)
}

/// Keep only the first `limit` sorted paths while scanning, rather than retaining
/// the entire directory. The original path resolves case-folded name ties.
fn smallest_paths(paths: impl Iterator<Item = PathBuf>, limit: usize) -> Vec<PathBuf> {
    if limit == 0 {
        return Vec::new();
    }
    let mut kept = BinaryHeap::with_capacity(limit);
    for path in paths {
        let name = path
            .file_name()
            .map(|n| n.to_string_lossy().to_ascii_lowercase())
            .unwrap_or_default();
        let candidate = (name, path);
        if kept.len() < limit {
            kept.push(candidate);
        } else if kept.peek().is_some_and(|largest| &candidate < largest) {
            *kept.peek_mut().unwrap() = candidate;
        }
    }
    kept.into_sorted_vec().into_iter().map(|(_, path)| path).collect()
}

/// One folder tile in the filmstrip: the directory plus its cover image (the
/// first supported file in deterministic name order).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FolderTile {
    pub folder: PathBuf,
    pub cover: PathBuf,
}

/// Enumerate sibling directories of `folder` (including `folder` itself) that
/// contain at least one supported image. Deliberately cheap: one readdir of
/// the parent plus one readdir per subdirectory, no recursion, no symlink
/// following.
pub fn sibling_folder_tiles(folder: &Path) -> Vec<FolderTile> {
    let Some(parent) = folder.parent() else {
        return Vec::new();
    };
    let Ok(entries) = std::fs::read_dir(parent) else {
        return Vec::new();
    };
    let mut tiles = entries
        .filter_map(Result::ok)
        .filter_map(|entry| {
            let path = entry.path();
            let metadata = std::fs::symlink_metadata(&path).ok()?;
            if !metadata.file_type().is_dir() {
                return None;
            }
            let cover = first_supported_image(&path)?;
            Some(FolderTile { folder: path, cover })
        })
        .collect::<Vec<_>>();
    tiles.sort_by_cached_key(|tile| {
        tile.folder
            .file_name()
            .map(|n| n.to_string_lossy().to_ascii_lowercase())
            .unwrap_or_default()
    });
    tiles
}

fn first_supported_image(folder: &Path) -> Option<PathBuf> {
    let candidates = std::fs::read_dir(folder)
        .ok()?
        .filter_map(Result::ok)
        .filter_map(|entry| {
            let path = entry.path();
            let metadata = std::fs::symlink_metadata(&path).ok()?;
            (metadata.file_type().is_file() && is_supported(&path)).then_some(path)
        });
    smallest_paths(candidates, 1).pop()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write_file(path: &std::path::Path) {
        std::fs::write(path, b"synthetic").expect("write test file");
    }

    #[test]
    fn streaming_folder_limit_preserves_sorted_prefix_and_name_ties() {
        let mut paths: Vec<_> = (0..MAX_ITEMS * 3)
            .rev()
            .map(|i| PathBuf::from(format!("{i:06}.CR3")))
            .collect();
        paths.extend([PathBuf::from("000001.cr3"), PathBuf::from("000001.Cr3")]);
        let actual = smallest_paths(paths.clone().into_iter(), MAX_ITEMS);
        paths.sort_by_key(|p| {
            (
                p.file_name().unwrap().to_string_lossy().to_ascii_lowercase(),
                p.clone(),
            )
        });
        paths.truncate(MAX_ITEMS);
        assert_eq!(actual, paths);
        assert_eq!(
            smallest_paths([PathBuf::from("z"), PathBuf::from("a")].into_iter(), 1),
            [PathBuf::from("a")]
        );
        assert!(smallest_paths(std::iter::empty(), MAX_ITEMS).is_empty());
        assert!(smallest_paths(std::iter::once(PathBuf::from("a")), 0).is_empty());
    }

    #[test]
    fn sibling_folder_tiles_lists_only_dirs_with_supported_images() {
        let root = tempfile::tempdir().expect("tempdir");
        let parent = root.path();
        for name in ["b-session", "a-session", "c-session"] {
            let dir = parent.join(name);
            std::fs::create_dir(&dir).expect("mkdir");
        }
        write_file(&parent.join("b-session").join("IMG_0002.CR3"));
        write_file(&parent.join("b-session").join("IMG_0001.DNG"));
        write_file(&parent.join("a-session").join("photo.dng"));
        // c-session has only unsupported files and must be skipped.
        write_file(&parent.join("c-session").join("notes.txt"));
        write_file(&parent.join("loose-file.dng"));

        let tiles = sibling_folder_tiles(&parent.join("b-session"));
        let names: Vec<_> = tiles
            .iter()
            .map(|tile| tile.folder.file_name().unwrap().to_string_lossy().into_owned())
            .collect();
        assert_eq!(names, ["a-session", "b-session"], "sorted, supported-only");
        // Cover is the first supported image in deterministic name order.
        assert_eq!(
            tiles[1].cover.file_name().unwrap().to_string_lossy(),
            "IMG_0001.DNG"
        );
    }

    #[test]
    fn sibling_folder_tiles_ignores_symlinked_dirs() {
        let root = tempfile::tempdir().expect("tempdir");
        let parent = root.path();
        let real = parent.join("real");
        std::fs::create_dir(&real).expect("mkdir");
        write_file(&real.join("a.cr3"));
        #[cfg(unix)]
        {
            std::os::unix::fs::symlink(&real, parent.join("linked")).expect("symlink");
            let tiles = sibling_folder_tiles(&real);
            assert_eq!(tiles.len(), 1);
            assert_eq!(tiles[0].folder, real);
        }
        #[cfg(not(unix))]
        {
            assert_eq!(sibling_folder_tiles(&real).len(), 1);
        }
    }

    #[test]
    fn sibling_folder_tiles_handles_missing_parent_and_filesystem_root() {
        assert!(sibling_folder_tiles(Path::new("/definitely/missing/path")).is_empty());
        let root = std::path::Path::new("/");
        // parent of "/" is None.
        let _ = sibling_folder_tiles(root);
    }

    fn thumbnail_job(index: usize) -> ThumbnailJob {
        ThumbnailJob {
            index,
            source: PathBuf::from(format!("{index}.dng")),
            edge: THUMB_EDGE,
        }
    }

    fn thumbnail_ready(index: usize) -> ThumbnailReady {
        ThumbnailReady {
            source_stamp: SourceStamp::default(),
            index,
            width: 1,
            height: 1,
            pixels: Arc::new(vec![index as u8, 0, 0, 255]).into(),
        }
    }

    fn raw_prefetcher_without_worker() -> RawPrefetcher {
        let (tx, pending) = bounded(1);
        let decode_gate = Arc::new(DecodeGate::new());
        let telemetry = Arc::new(CacheTelemetry::new(true, 1024));
        RawPrefetcher {
            lifetime: Arc::new(AtomicU64::new(0)),
            tx,
            pending,
            generation: decode_gate.speculative_generation(),
            state: Arc::new(AtomicU8::new(0)),
            decode_gate,
            telemetry,
            enabled: true,
            window: PrefetchWindow::default(),
        }
    }
    #[test]
    fn raw_prefetch_shutdown_cancels_only_its_own_lifetime() {
        let worker = raw_prefetcher_without_worker();
        let generation = Arc::clone(&worker.generation);
        let pending = worker.pending.clone();
        worker.finish_foreground_and_submit(
            &[PathBuf::from("0.png"), PathBuf::from("1.cr3")],
            0,
            NavDirection::Forward,
        );
        let current = generation.load(Ordering::Acquire);
        let gate_token = GenerationToken::new(Arc::clone(&generation), current);
        let token = gate_token.combine(&GenerationToken::new(Arc::clone(&worker.lifetime), 0));
        assert!(!token.is_cancelled());
        let state = Arc::clone(&worker.state);
        drop(worker);
        assert!(token.is_cancelled());
        assert!(!gate_token.is_cancelled());
        assert_eq!(generation.load(Ordering::Acquire), current);
        assert!(matches!(
            pending.try_recv(),
            Err(crossbeam_channel::TryRecvError::Disconnected)
        ));
        assert!(RawStoreAdmission::try_acquire(&state).is_none());
    }

    #[test]
    fn mixed_gallery_submits_raw_neighbours_from_raster_or_model_selection() {
        let paths: Vec<_> = ["0.cr3", "1.png", "2.nef", "3.obj", "4.arw", "5.jpg"]
            .into_iter()
            .map(PathBuf::from)
            .collect();
        let mut worker = raw_prefetcher_without_worker();
        worker.window = PrefetchWindow { behind: 1, ahead: 2 };
        worker.begin_foreground();
        worker.finish_foreground_and_submit(&paths, 1, NavDirection::Forward);
        assert_eq!(
            worker.pending.try_recv().unwrap().paths,
            [paths[2].clone(), paths[0].clone()]
        );
        worker.begin_foreground();
        worker.finish_foreground_and_submit(&paths, 3, NavDirection::Backward);
        assert_eq!(
            worker.pending.try_recv().unwrap().paths,
            [paths[2].clone(), paths[4].clone()]
        );
    }
    #[test]
    fn raw_worker_skips_ordinary_tiff_without_decoding_failure() {
        let directory = tempfile::tempdir().unwrap();
        let budget = rrrah_cache::MemoryBudget::new(4096);
        let telemetry = Arc::new(CacheTelemetry::new(true, 4096));
        let worker = RawPrefetcher::with_budget(
            Some(directory.path().to_owned()),
            false,
            Arc::new(DecodeGate::new()),
            telemetry.clone(),
            PrefetchWindow { behind: 0, ahead: 1 },
            rrrah_cache::CacheLimits::bytes(4096),
            Some(budget.clone()),
        );
        let paths = [
            PathBuf::from("selected.png"),
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/raster/pattern.tif"),
        ];
        worker.finish_foreground_and_submit(&paths, 0, NavDirection::Forward);
        let deadline = std::time::Instant::now() + Duration::from_secs(5);
        while telemetry.snapshot().prefetch_completed == 0 && std::time::Instant::now() < deadline {
            thread::sleep(Duration::from_millis(1));
        }
        let snapshot = telemetry.snapshot();
        assert_eq!(snapshot.prefetch_planned, 1);
        assert_eq!(snapshot.prefetch_completed, 1);
        assert_eq!(snapshot.prefetch_failures, 0);
        assert_eq!(snapshot.prefetch_stored, 0);
        assert_eq!(snapshot.prefetch_cached, 0);
        assert_eq!(budget.peak(), 0);
        assert_eq!(
            rrrah_cache::DiskMosaicCache::new(directory.path())
                .usage()
                .unwrap()
                .entries,
            0
        );
    }

    #[test]
    #[ignore = "requires local EOS R8 tests/IMG_9043.CR3 fixture"]
    fn raster_selection_warms_real_raw_neighbour_without_nonraw_failures() {
        let directory = tempfile::tempdir().unwrap();
        let budget = rrrah_cache::MemoryBudget::new(128 * 1024 * 1024);
        let fixture = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/IMG_9043.CR3");
        let paths = vec![
            PathBuf::from("selected.png"),
            fixture.clone(),
            PathBuf::from("next.jpg"),
        ];
        let telemetry = Arc::new(CacheTelemetry::new(true, budget.limit()));
        let worker = RawPrefetcher::with_budget(
            Some(directory.path().to_owned()),
            false,
            Arc::new(DecodeGate::new()),
            telemetry.clone(),
            PrefetchWindow { behind: 0, ahead: 2 },
            rrrah_cache::CacheLimits::bytes(budget.limit()),
            Some(budget.clone()),
        );
        worker.begin_foreground();
        worker.finish_foreground_and_submit(&paths, 0, NavDirection::Forward);
        let deadline = std::time::Instant::now() + Duration::from_secs(10);
        while (telemetry.snapshot().prefetch_completed == 0 || budget.used() != 0)
            && std::time::Instant::now() < deadline
        {
            thread::sleep(Duration::from_millis(1));
        }
        let snapshot = telemetry.snapshot();
        assert_eq!(snapshot.prefetch_planned, 1);
        assert_eq!(snapshot.prefetch_completed, 1);
        assert_eq!(snapshot.prefetch_stored, 1);
        assert_eq!(snapshot.prefetch_failures, 0);
        assert_eq!(budget.used(), 0);
        let mut request = DecodeRequest::new(&fixture);
        request.memory_budget = Some(budget.clone());
        let key = CacheKey::for_mosaic_recipe(
            &SourceFingerprint::from_path(&fixture).unwrap(),
            0,
            NativeRawDecoder.mosaic_recipe(&request).unwrap(),
        );
        let cache = rrrah_cache::DiskMosaicCache::new(directory.path());
        let restored = cache.load_with_budget(key, &budget).unwrap().unwrap().mosaic;
        let native = NativeRawDecoder.decode(&request).unwrap().mosaic;
        assert_eq!(restored.metadata, native.metadata);
        assert_eq!(&*restored.pixels, &*native.pixels);
        drop(restored);
        drop(native);
        drop(worker);
        assert_eq!(budget.used(), 0);
    }

    #[test]
    #[ignore = "requires pinned EOS 10D via RRRAH_CRW_SOURCE"]
    fn raster_selection_warms_real_crw_neighbour() {
        let directory = tempfile::tempdir().unwrap();
        let budget = rrrah_cache::MemoryBudget::new(128 * 1024 * 1024);
        let fixture = PathBuf::from(std::env::var("RRRAH_CRW_SOURCE").unwrap());
        let paths = vec![
            PathBuf::from("selected.png"),
            fixture.clone(),
            PathBuf::from("next.jpg"),
        ];
        let telemetry = Arc::new(CacheTelemetry::new(true, budget.limit()));
        let worker = RawPrefetcher::with_budget(
            Some(directory.path().to_owned()),
            false,
            Arc::new(DecodeGate::new()),
            telemetry.clone(),
            PrefetchWindow { behind: 0, ahead: 2 },
            rrrah_cache::CacheLimits::bytes(budget.limit()),
            Some(budget.clone()),
        );
        worker.begin_foreground();
        worker.finish_foreground_and_submit(&paths, 0, NavDirection::Forward);
        let deadline = std::time::Instant::now() + Duration::from_secs(10);
        while (telemetry.snapshot().prefetch_completed == 0 || budget.used() != 0)
            && std::time::Instant::now() < deadline
        {
            thread::sleep(Duration::from_millis(1));
        }
        let snapshot = telemetry.snapshot();
        assert_eq!(snapshot.prefetch_planned, 1);
        assert_eq!(snapshot.prefetch_completed, 1);
        assert_eq!(snapshot.prefetch_stored, 1);
        assert_eq!(snapshot.prefetch_failures, 0);
        assert_eq!(budget.used(), 0);
        let mut request = DecodeRequest::new(&fixture);
        request.memory_budget = Some(budget.clone());
        let key = CacheKey::for_mosaic_recipe(
            &SourceFingerprint::from_path(&fixture).unwrap(),
            0,
            NativeRawDecoder.mosaic_recipe(&request).unwrap(),
        );
        let cache = rrrah_cache::DiskMosaicCache::new(directory.path());
        let restored = cache.load_with_budget(key, &budget).unwrap().unwrap().mosaic;
        let native = NativeRawDecoder.decode(&request).unwrap().mosaic;
        assert_eq!(restored.metadata, native.metadata);
        assert_eq!(&*restored.pixels, &*native.pixels);
        drop(restored);
        drop(native);
        drop(worker);
        assert_eq!(budget.used(), 0);
    }

    fn recv_test<T>(rx: &Receiver<T>) -> T {
        rx.recv_timeout(Duration::from_secs(2))
            .expect("synchronized test worker did not make progress")
    }

    #[test]
    #[ignore = "requires local EOS R8 tests/IMG_9043.CR3 fixture"]
    fn raw_prefetch_byte_limit_preserves_present_priority_neighbour() {
        let root = tempfile::tempdir().unwrap();
        let fixture_root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests");
        let first = fixture_root.join("IMG_9043.CR3");
        let second = fixture_root.join("IMG_9074.CR3");
        let budget = rrrah_cache::MemoryBudget::new(96 * 1024 * 1024);
        let mut request = DecodeRequest::new(&first);
        request.memory_budget = Some(budget.clone());
        let key = CacheKey::for_mosaic_recipe(
            &SourceFingerprint::from_path(&first).unwrap(),
            0,
            NativeRawDecoder.mosaic_recipe(&request).unwrap(),
        );
        let output = NativeRawDecoder.decode(&request).unwrap();
        let cache = rrrah_cache::DiskMosaicCache::new(root.path());
        let stored = cache.store(key, &output.mosaic).unwrap();
        let limit = std::fs::metadata(stored).unwrap().len();
        drop(output);
        assert_eq!(budget.used(), 0);
        let telemetry = Arc::new(CacheTelemetry::new(true, limit));
        let prefetcher = RawPrefetcher::with_budget(
            Some(root.path().to_owned()),
            false,
            Arc::new(DecodeGate::new()),
            telemetry.clone(),
            PrefetchWindow { behind: 0, ahead: 2 },
            rrrah_cache::CacheLimits::bytes(limit),
            Some(budget.clone()),
        );
        let paths = vec![root.path().join("selected.cr3"), first, second];
        prefetcher.finish_foreground_and_submit(&paths, 0, NavDirection::Forward);
        let deadline = std::time::Instant::now() + Duration::from_secs(10);
        while (telemetry.snapshot().prefetch_completed < 2 || budget.used() != 0)
            && std::time::Instant::now() < deadline
        {
            std::thread::sleep(Duration::from_millis(1));
        }
        assert_eq!(telemetry.snapshot().prefetch_completed, 2);
        assert_eq!(telemetry.snapshot().prefetch_stored, 0);
        assert_eq!(telemetry.snapshot().prefetch_failures, 1);
        assert!(cache.load_with_budget(key, &budget).unwrap().is_some());
        assert_eq!(cache.usage().unwrap().entries, 1);
        assert_eq!(budget.used(), 0);
    }

    #[test]
    #[ignore = "requires external Sony DSC-F828 SRF fixture"]
    fn raw_prefetch_count_limit_counts_retained_neighbours_after_missing_file() {
        let root = tempfile::tempdir().unwrap();
        let fixture = PathBuf::from(std::env::var("RRRAH_SRF_SOURCE").unwrap());
        let budget = rrrah_cache::MemoryBudget::new(64 * 1024 * 1024);
        let telemetry = Arc::new(CacheTelemetry::new(true, 64 * 1024 * 1024));
        let mut limits = rrrah_cache::CacheLimits::bytes(64 * 1024 * 1024);
        limits.max_entries = Some(1);
        let prefetcher = RawPrefetcher::with_budget(
            Some(root.path().to_owned()),
            false,
            Arc::new(DecodeGate::new()),
            telemetry.clone(),
            PrefetchWindow { behind: 0, ahead: 2 },
            limits,
            Some(budget.clone()),
        );
        let paths = vec![
            root.path().join("selected.srf"),
            root.path().join("missing.srf"),
            fixture.clone(),
        ];
        prefetcher.finish_foreground_and_submit(&paths, 0, NavDirection::Forward);
        let deadline = std::time::Instant::now() + Duration::from_secs(10);
        while (telemetry.snapshot().prefetch_completed < 2 || budget.used() != 0)
            && std::time::Instant::now() < deadline
        {
            std::thread::sleep(Duration::from_millis(1));
        }
        assert_eq!(telemetry.snapshot().prefetch_failures, 1);
        assert_eq!(telemetry.snapshot().prefetch_stored, 1);
        assert_eq!(telemetry.snapshot().prefetch_completed, 2);
        let request = DecodeRequest::new(&fixture);
        let key = CacheKey::for_mosaic_recipe(
            &SourceFingerprint::from_path(&fixture).unwrap(),
            0,
            NativeRawDecoder.mosaic_recipe(&request).unwrap(),
        );
        let cache = rrrah_cache::DiskMosaicCache::new(root.path());
        let restored = cache.load_with_budget(key, &budget).unwrap().unwrap().mosaic;
        assert_eq!(
            restored.metadata.cfa.as_ref().unwrap().rgbe_quad().unwrap(),
            [3, 0, 2, 1]
        );
        drop(restored);
        assert_eq!(cache.usage().unwrap().entries, 1);
        assert_eq!(budget.used(), 0);
    }

    #[test]
    #[ignore = "requires local EOS R8 tests/IMG_9043.CR3 fixture"]
    fn raw_prefetch_count_limit_preserves_highest_priority_neighbour() {
        let root = tempfile::tempdir().unwrap();
        let budget = rrrah_cache::MemoryBudget::new(96 * 1024 * 1024);
        let telemetry = Arc::new(CacheTelemetry::new(true, 128 * 1024 * 1024));
        let mut limits = rrrah_cache::CacheLimits::bytes(128 * 1024 * 1024);
        limits.max_entries = Some(1);
        let prefetcher = RawPrefetcher::with_budget(
            Some(root.path().to_owned()),
            false,
            Arc::new(DecodeGate::new()),
            telemetry.clone(),
            PrefetchWindow { behind: 1, ahead: 2 },
            limits,
            Some(budget.clone()),
        );
        let fixture = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/IMG_9043.CR3");
        let paths = vec![
            root.path().join("missing-previous.cr3"),
            root.path().join("selected.cr3"),
            fixture.clone(),
            root.path().join("missing-distant.cr3"),
        ];
        prefetcher.finish_foreground_and_submit(&paths, 1, NavDirection::Forward);
        let deadline = std::time::Instant::now() + Duration::from_secs(10);
        while telemetry.snapshot().prefetch_completed == 0 && std::time::Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(1));
        }
        assert!(telemetry.snapshot().prefetch_completed > 0);
        assert_eq!(telemetry.snapshot().prefetch_stored, 1);
        assert_eq!(
            telemetry.snapshot().prefetch_failures,
            0,
            "lower-priority neighbours must not consume work after the count limit"
        );
        let request = DecodeRequest::new(&fixture);
        let key = CacheKey::for_mosaic_recipe(
            &SourceFingerprint::from_path(&fixture).unwrap(),
            0,
            NativeRawDecoder.mosaic_recipe(&request).unwrap(),
        );
        let cache = rrrah_cache::DiskMosaicCache::new(root.path());
        assert!(cache.contains(key));
        assert_eq!(cache.usage().unwrap().entries, 1);
        assert_eq!(budget.used(), 0);
    }

    #[test]
    #[ignore = "requires local EOS R8 tests/IMG_9043.CR3 fixture"]
    fn raw_prefetch_respects_shared_budget_and_recovers_after_release() {
        let root = tempfile::tempdir().unwrap();
        let budget = rrrah_cache::MemoryBudget::new(96 * 1024 * 1024);
        let held = budget.try_reserve(budget.limit()).unwrap();
        let gate = Arc::new(DecodeGate::new());
        let telemetry = Arc::new(CacheTelemetry::new(true, 128 * 1024 * 1024));
        let prefetcher = RawPrefetcher::with_budget(
            Some(root.path().to_owned()),
            false,
            gate,
            telemetry.clone(),
            PrefetchWindow { behind: 0, ahead: 1 },
            rrrah_cache::CacheLimits::bytes(128 * 1024 * 1024),
            Some(budget.clone()),
        );
        let fixture = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/IMG_9043.CR3");
        let paths = vec![PathBuf::from("selected.cr3"), fixture];
        prefetcher.finish_foreground_and_submit(&paths, 0, NavDirection::Forward);
        let deadline = std::time::Instant::now() + Duration::from_secs(5);
        while telemetry.snapshot().prefetch_completed == 0 && std::time::Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(1));
        }
        assert_eq!(telemetry.snapshot().prefetch_failures, 1);
        assert_eq!(
            rrrah_cache::DiskMosaicCache::new(root.path())
                .usage()
                .unwrap()
                .entries,
            0
        );
        assert_eq!(budget.used(), budget.limit());
        drop(held);
        prefetcher.finish_foreground_and_submit(&paths, 0, NavDirection::Forward);
        let deadline = std::time::Instant::now() + Duration::from_secs(5);
        while (telemetry.snapshot().prefetch_stored == 0 || budget.used() != 0)
            && std::time::Instant::now() < deadline
        {
            std::thread::sleep(Duration::from_millis(1));
        }
        assert_eq!(telemetry.snapshot().prefetch_stored, 1);
        assert_eq!(
            rrrah_cache::DiskMosaicCache::new(root.path())
                .usage()
                .unwrap()
                .entries,
            1
        );
        assert_eq!(budget.used(), 0);
        assert!(budget.peak() <= budget.limit());
    }

    #[test]
    fn new_camera_formats_enter_gallery_and_directional_prefetch_window() {
        let directory = tempfile::tempdir().unwrap();
        let names = ["00.ERF", "01.kdc", "02.SRW", "03.3FR", "04.sr2"];
        for name in names {
            std::fs::write(directory.path().join(name), []).unwrap();
        }
        std::fs::write(directory.path().join("unsupported.txt"), []).unwrap();
        std::fs::create_dir(directory.path().join("directory.3FR")).unwrap();
        let paths = scan_folder(directory.path());
        assert_eq!(paths, names.map(|name| directory.path().join(name)));
        let window = PrefetchWindow { behind: 1, ahead: 2 };
        assert_eq!(
            neighbour_prefetch_paths(&paths, 2, NavDirection::Forward, window),
            [paths[3].clone(), paths[4].clone(), paths[1].clone()]
        );
        assert_eq!(
            neighbour_prefetch_paths(&paths, 2, NavDirection::Backward, window),
            [paths[1].clone(), paths[0].clone(), paths[3].clone()]
        );
    }

    #[test]
    fn extension_filter_is_case_insensitive() {
        assert!(is_supported(Path::new("a.CR3")));
        for ext in ["CR2", "NEF", "ARW", "ORF", "PEF", "RW2", "RAF"] {
            assert!(is_supported(Path::new(&format!("a.{ext}"))));
        }
        assert!(is_supported(Path::new("a.DNG")));
        assert!(is_supported(Path::new("a.TIFF")));
        assert!(is_supported(Path::new("a.jpg")));
    }

    #[test]
    fn configured_raw_window_reaches_worker_and_is_clamped() {
        let paths: Vec<_> = (0..8).map(|i| PathBuf::from(format!("{i}.cr3"))).collect();
        let mut worker = raw_prefetcher_without_worker();
        worker.window = PrefetchWindow { behind: 1, ahead: 3 };
        worker.finish_foreground_and_submit(&paths, 3, NavDirection::Forward);
        let command = worker.pending.try_recv().unwrap();
        assert_eq!(
            command.paths,
            [
                paths[4].clone(),
                paths[5].clone(),
                paths[6].clone(),
                paths[2].clone()
            ]
        );
        worker.finish_foreground_and_submit(&paths, 3, NavDirection::Backward);
        assert_eq!(
            worker.pending.try_recv().unwrap().paths,
            [
                paths[2].clone(),
                paths[1].clone(),
                paths[0].clone(),
                paths[4].clone()
            ]
        );
        assert!(
            neighbour_prefetch_paths(
                &paths,
                3,
                NavDirection::Forward,
                PrefetchWindow { behind: 0, ahead: 0 }
            )
            .is_empty()
        );
        let all = neighbour_prefetch_paths(
            &paths,
            3,
            NavDirection::Forward,
            PrefetchWindow {
                behind: usize::MAX,
                ahead: usize::MAX,
            },
        );
        assert_eq!(all.len(), 7);
        assert!(!all.contains(&paths[3]));
        assert_eq!(all.iter().collect::<std::collections::HashSet<_>>().len(), 7);
    }
    #[test]
    fn neighbour_windows_match_distance_contract_for_every_small_gallery() {
        for len in 0..16 {
            let paths: Vec<_> = (0..len).map(|i| PathBuf::from(i.to_string())).collect();
            for selected in 0..=len {
                for behind in [0, 1, 3, usize::MAX] {
                    for ahead in [0, 2, 5, usize::MAX] {
                        for direction in [NavDirection::None, NavDirection::Forward, NavDirection::Backward] {
                            let actual = neighbour_prefetch_paths(
                                &paths,
                                selected,
                                direction,
                                PrefetchWindow { behind, ahead },
                            );
                            let mut expected = Vec::new();
                            if selected < len {
                                let (left, right) = if direction == NavDirection::Backward {
                                    (ahead, behind)
                                } else {
                                    (behind, ahead)
                                };
                                for index in 0..len {
                                    if (index < selected && selected - index <= left)
                                        || (index > selected && index - selected <= right)
                                    {
                                        expected.push(index);
                                    }
                                }
                                expected.sort_by_key(|&index| {
                                    let priority_side = if direction == NavDirection::Forward {
                                        index > selected
                                    } else {
                                        index < selected
                                    };
                                    (!priority_side, index.abs_diff(selected))
                                });
                            }
                            let expected: Vec<_> = expected.into_iter().map(|i| paths[i].clone()).collect();
                            assert_eq!(
                                actual, expected,
                                "len={len} selected={selected} behind={behind} ahead={ahead} direction={direction:?}"
                            );
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn jobs_are_bounded_around_selection() {
        let m = GalleryModel {
            items: (0..5)
                .map(|i| GalleryItem {
                    path: PathBuf::from(format!("{i}.cr3")),
                    thumbnail: None,
                })
                .collect(),
            ..GalleryModel::default()
        };
        let jobs = m.jobs(2, 1).collect::<Vec<_>>();
        assert_eq!(jobs.len(), 3);
        assert_eq!(jobs[0].index, 1);
        assert_eq!(
            m.jobs(2, usize::MAX).map(|job| job.index).collect::<Vec<_>>(),
            vec![0, 1, 2, 3, 4]
        );
        assert_eq!(m.jobs(usize::MAX, usize::MAX).count(), 0);
        assert_eq!(m.jobs(5, 1).count(), 0);
        assert_eq!(GalleryModel::default().jobs(0, usize::MAX).count(), 0);
        assert!(m.prefetch_jobs(5).is_empty());
        assert!(m.prefetch_jobs(usize::MAX).is_empty());
        assert!(GalleryModel::default().prefetch_jobs(1).is_empty());
    }

    #[test]
    fn prefetch_plan_is_forward_biased_and_bounded() {
        let m = GalleryModel {
            items: (0..20)
                .map(|i| GalleryItem {
                    path: PathBuf::from(format!("{i}.cr3")),
                    thumbnail: None,
                })
                .collect(),
            ..GalleryModel::default()
        };
        let plan = m.prefetch_plan(10, 42);
        assert_eq!(plan.len(), 8);
        assert_eq!(plan[0].thumbnail.index, 10);
        assert_eq!(plan[1].thumbnail.index, 9);
        assert_eq!(plan[2].thumbnail.index, 8);
        assert_eq!(plan[7].thumbnail.index, 15);
        assert!(plan.iter().all(|job| job.generation == 42));
        assert_eq!(
            plan.iter().map(|job| job.priority).collect::<Vec<_>>(),
            vec![0, 1, 1, 1, 1, 1, 1, 1]
        );
    }

    #[test]
    fn prefetch_plan_handles_edges_without_duplicates() {
        let m = GalleryModel {
            items: (0..3)
                .map(|i| GalleryItem {
                    path: PathBuf::from(format!("{i}.dng")),
                    thumbnail: None,
                })
                .collect(),
            ..GalleryModel::default()
        };
        let plan = m.prefetch_plan(0, 1);
        assert_eq!(
            plan.iter().map(|j| j.thumbnail.index).collect::<Vec<_>>(),
            vec![0, 1, 2]
        );
    }

    #[test]
    fn raw_prefetch_paths_are_two_back_then_five_ahead() {
        let paths = (0..20)
            .map(|i| PathBuf::from(format!("{i}.cr3")))
            .collect::<Vec<_>>();
        assert_eq!(
            raw_prefetch_paths(&paths, 10, NavDirection::None),
            [9, 8, 11, 12, 13, 14, 15].map(|i| PathBuf::from(format!("{i}.cr3")))
        );
        assert_eq!(
            raw_prefetch_paths(&paths, 10, NavDirection::Forward),
            [11, 12, 13, 14, 15, 9, 8].map(|i| PathBuf::from(format!("{i}.cr3"))),
            "forward travel warms the upcoming frames before revisited frames"
        );
    }

    #[test]
    fn raw_prefetch_paths_swap_window_when_travelling_backward() {
        let paths = (0..20)
            .map(|i| PathBuf::from(format!("{i}.cr3")))
            .collect::<Vec<_>>();
        assert_eq!(
            raw_prefetch_paths(&paths, 10, NavDirection::Backward),
            [9, 8, 7, 6, 5, 11, 12].map(|i| PathBuf::from(format!("{i}.cr3")))
        );
    }

    #[test]
    fn raw_prefetch_backward_window_is_bounded_at_gallery_edges() {
        let paths = (0..7)
            .map(|i| PathBuf::from(format!("{i}.dng")))
            .collect::<Vec<_>>();

        assert_eq!(
            raw_prefetch_paths(&paths, 0, NavDirection::Backward),
            [1, 2].map(|i| PathBuf::from(format!("{i}.dng")))
        );
        assert_eq!(
            raw_prefetch_paths(&paths, 6, NavDirection::Backward),
            [5, 4, 3, 2, 1].map(|i| PathBuf::from(format!("{i}.dng")))
        );
    }

    #[test]
    fn raw_prefetch_window_is_bounded_at_both_gallery_edges() {
        let paths = (0..7)
            .map(|i| PathBuf::from(format!("{i}.dng")))
            .collect::<Vec<_>>();

        assert_eq!(
            raw_prefetch_paths(&paths, 0, NavDirection::None),
            [1, 2, 3, 4, 5].map(|i| PathBuf::from(format!("{i}.dng")))
        );
        assert_eq!(
            raw_prefetch_paths(&paths, 6, NavDirection::None),
            [5, 4].map(|i| PathBuf::from(format!("{i}.dng")))
        );
        assert!(raw_prefetch_paths(&paths, paths.len(), NavDirection::None).is_empty());
        assert!(raw_prefetch_paths(&[], 0, NavDirection::None).is_empty());
    }

    #[test]
    fn dropping_thumbnail_prefetcher_cancels_active_loader() {
        let (started_tx, started_rx) = bounded(1);
        let (release_tx, release_rx) = bounded(1);
        let (finished_tx, finished_rx) = bounded(1);
        let prefetcher = Prefetcher::new_with_cancel(1, move |_, token| {
            started_tx.send(token.clone()).unwrap();
            release_rx.recv().unwrap();
            finished_tx.send(token.is_cancelled()).unwrap();
            None
        });
        prefetcher.submit([thumbnail_job(1)]);
        let token = recv_test(&started_rx);
        assert!(!token.is_cancelled());
        drop(prefetcher);
        assert!(token.is_cancelled());
        release_tx.send(()).unwrap();
        assert!(recv_test(&finished_rx));
    }

    #[test]
    fn thumbnail_viewport_cancels_active_loader() {
        let (started_tx, started_rx) = bounded(1);
        let (release_tx, release_rx) = bounded(1);
        let prefetcher = Prefetcher::new_with_cancel(1, move |job, token| {
            if job.index == 1 {
                started_tx.send(()).unwrap();
                release_rx.recv().unwrap();
                assert!(token.is_cancelled());
                return None;
            }
            assert!(!token.is_cancelled());
            Some(thumbnail_ready(job.index))
        });
        prefetcher.submit([thumbnail_job(1)]);
        recv_test(&started_rx);
        prefetcher.submit([thumbnail_job(2)]);
        release_tx.send(()).unwrap();
        assert_eq!(recv_test(&prefetcher.rx).index, 2);
        assert!(prefetcher.try_recv().is_none());
    }

    #[test]
    fn thumbnail_worker_drops_an_inflight_stale_generation() {
        let (started_tx, started_rx) = bounded(2);
        let (release_tx, release_rx) = bounded(0);
        let (finished_tx, finished_rx) = bounded(2);
        let prefetcher = Prefetcher::new(1, move |job| {
            started_tx.send(job.index).unwrap();
            if job.index == 1 {
                release_rx.recv().unwrap();
            }
            finished_tx.send(job.index).unwrap();
            Some(thumbnail_ready(job.index))
        });

        prefetcher.submit([thumbnail_job(1)]);
        assert_eq!(recv_test(&started_rx), 1);

        // This generation becomes current while job 1 is inside the injected
        // loader. Releasing it must not allow its result into the ready queue.
        prefetcher.submit([thumbnail_job(2)]);
        release_tx.send(()).unwrap();
        assert_eq!(recv_test(&finished_rx), 1);
        assert_eq!(recv_test(&started_rx), 2);
        assert_eq!(recv_test(&finished_rx), 2);

        assert_eq!(recv_test(&prefetcher.rx).index, 2);
        assert!(prefetcher.try_recv().is_none());
    }

    #[test]
    fn thumbnail_submit_never_consumes_beyond_window_capacity() {
        let prefetcher = Prefetcher::new(1, |job| Some(thumbnail_ready(job.index)));
        let jobs = std::iter::once(thumbnail_job(42)).chain(std::iter::from_fn(|| {
            panic!("jobs outside the admitted window must not be requested")
        }));
        prefetcher.submit(jobs);
        assert_eq!(recv_test(&prefetcher.rx).index, 42);
    }

    #[test]
    fn thumbnail_queue_replaces_stale_pending_window_at_capacity() {
        let (started_tx, started_rx) = bounded(8);
        let (release_tx, release_rx) = bounded(0);
        let (finished_tx, finished_rx) = bounded(8);
        let prefetcher = Prefetcher::new(2, move |job| {
            started_tx.send(job.index).unwrap();
            if job.index == 0 {
                release_rx.recv().unwrap();
            }
            finished_tx.send(job.index).unwrap();
            Some(thumbnail_ready(job.index))
        });

        prefetcher.submit([thumbnail_job(0)]);
        assert_eq!(recv_test(&started_rx), 0);
        prefetcher.submit([thumbnail_job(1), thumbnail_job(2)]);

        // The newest submission drains both queued stale jobs. Capacity two
        // admits only 10 and 11; 12 is deterministically rejected.
        prefetcher.submit([thumbnail_job(10), thumbnail_job(11), thumbnail_job(12)]);
        release_tx.send(()).unwrap();
        assert_eq!(recv_test(&finished_rx), 0);
        assert_eq!(recv_test(&started_rx), 10);
        assert_eq!(recv_test(&finished_rx), 10);
        assert_eq!(recv_test(&started_rx), 11);
        assert_eq!(recv_test(&finished_rx), 11);
        assert!(started_rx.try_recv().is_err());

        assert_eq!(recv_test(&prefetcher.rx).index, 10);
        assert_eq!(recv_test(&prefetcher.rx).index, 11);
        assert!(prefetcher.try_recv().is_none());
    }

    #[test]
    fn thumbnail_submit_discards_already_ready_stale_generation() {
        let (started_tx, started_rx) = bounded(4);
        let (release_tx, release_rx) = bounded(0);
        let prefetcher = Prefetcher::new(2, move |job| {
            started_tx.send(job.index).unwrap();
            if job.index == 99 {
                release_rx.recv().unwrap();
            }
            Some(thumbnail_ready(job.index))
        });

        // Reaching job 99 proves that job 1 has completed and its ready pixel
        // buffer is resident in the output channel. Keep 99 in flight while a
        // newer generation atomically replaces both queues.
        prefetcher.submit([thumbnail_job(1), thumbnail_job(99)]);
        assert_eq!(recv_test(&started_rx), 1);
        assert_eq!(recv_test(&started_rx), 99);
        prefetcher.submit([thumbnail_job(2)]);

        release_tx.send(()).unwrap();
        assert_eq!(recv_test(&started_rx), 2);
        assert_eq!(recv_test(&prefetcher.rx).index, 2);
        assert!(prefetcher.try_recv().is_none());
    }

    #[test]
    fn raw_queue_is_latest_wins_and_bounded_to_one_command() {
        let prefetcher = raw_prefetcher_without_worker();
        let paths = (0..20)
            .map(|index| PathBuf::from(format!("{index}.cr3")))
            .collect::<Vec<_>>();

        prefetcher.finish_foreground_and_submit(&paths, 3, NavDirection::None);
        let first_generation = prefetcher.generation.load(Ordering::Acquire);
        prefetcher.finish_foreground_and_submit(&paths, 10, NavDirection::None);

        assert_eq!(prefetcher.pending.len(), 1);
        let command = prefetcher.pending.try_recv().unwrap();
        assert!(command.generation > first_generation);
        assert_eq!(command.generation, prefetcher.generation.load(Ordering::Acquire));
        assert_eq!(command.paths, raw_prefetch_paths(&paths, 10, NavDirection::None));
        assert!(prefetcher.pending.try_recv().is_err());
    }

    #[test]
    fn foreground_pause_invalidates_pending_then_resumes_fresh_generation() {
        let prefetcher = raw_prefetcher_without_worker();
        let paths = (0..12)
            .map(|index| PathBuf::from(format!("{index}.dng")))
            .collect::<Vec<_>>();

        prefetcher.finish_foreground_and_submit(&paths, 4, NavDirection::None);
        let stale_generation = prefetcher.generation.load(Ordering::Acquire);
        assert_eq!(prefetcher.pending.len(), 1);

        prefetcher.begin_foreground();
        assert_ne!(
            prefetcher.state.load(Ordering::Acquire) & RAW_PREFETCH_FOREGROUND,
            0
        );
        assert!(prefetcher.pending.is_empty());
        assert_ne!(prefetcher.generation.load(Ordering::Acquire), stale_generation);

        prefetcher.finish_foreground_and_submit(&paths, 5, NavDirection::None);
        assert_eq!(
            prefetcher.state.load(Ordering::Acquire) & RAW_PREFETCH_FOREGROUND,
            0
        );
        let resumed = prefetcher.pending.try_recv().unwrap();
        assert_eq!(resumed.generation, prefetcher.generation.load(Ordering::Acquire));
        assert_eq!(resumed.paths, raw_prefetch_paths(&paths, 5, NavDirection::None));
    }

    #[test]
    fn foreground_bit_prevents_new_store_admission_without_losing_state() {
        let state = Arc::new(AtomicU8::new(0));
        let admitted = RawStoreAdmission::try_acquire(&state).unwrap();

        state.fetch_or(RAW_PREFETCH_FOREGROUND, Ordering::AcqRel);
        assert!(RawStoreAdmission::try_acquire(&state).is_none());

        drop(admitted);
        assert_eq!(
            state.load(Ordering::Acquire),
            RAW_PREFETCH_FOREGROUND,
            "finishing an admitted store must preserve a concurrent foreground pause"
        );
        state.fetch_and(!RAW_PREFETCH_FOREGROUND, Ordering::Release);
        assert!(RawStoreAdmission::try_acquire(&state).is_some());
    }

    #[test]
    fn rapid_navigation_never_accumulates_raw_commands() {
        let prefetcher = raw_prefetcher_without_worker();
        let paths = (0..128)
            .map(|index| PathBuf::from(format!("{index}.cr2")))
            .collect::<Vec<_>>();

        for step in 0..10_000 {
            if step % 17 == 0 {
                prefetcher.begin_foreground();
                assert!(prefetcher.pending.is_empty());
            }
            let selected = (step * 37) % paths.len();
            prefetcher.finish_foreground_and_submit(&paths, selected, NavDirection::None);
            assert!(prefetcher.pending.len() <= 1, "step {step}");
        }

        let latest = prefetcher.pending.try_recv().unwrap();
        let selected = ((10_000 - 1) * 37) % paths.len();
        assert_eq!(
            latest.paths,
            raw_prefetch_paths(&paths, selected, NavDirection::None)
        );
        assert_eq!(latest.generation, prefetcher.generation.load(Ordering::Acquire));
    }
}
