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
use rrrah_cache::{CacheKey, DiskMosaicCache, SourceFingerprint};
use rrrah_decode::{DecodeRequest, GenerationToken, NativeRawDecoder, RawDecoder};
use std::{
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
    fn window(self) -> (usize, usize) {
        match self {
            Self::None | Self::Forward => (PREFETCH_BEHIND, PREFETCH_AHEAD),
            Self::Backward => (PREFETCH_AHEAD, PREFETCH_BEHIND),
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
        let start = center.saturating_sub(radius);
        let end = (center.saturating_add(radius + 1)).min(self.items.len());
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
    tx: Sender<RawPrefetchCommand>,
    pending: Receiver<RawPrefetchCommand>,
    generation: Arc<AtomicU64>,
    state: Arc<AtomicU8>,
    decode_gate: Arc<DecodeGate>,
    telemetry: Arc<CacheTelemetry>,
    enabled: bool,
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

impl RawPrefetcher {
    pub fn new(
        cache_root: Option<PathBuf>,
        no_cache: bool,
        decode_gate: Arc<DecodeGate>,
        telemetry: Arc<CacheTelemetry>,
    ) -> Self {
        let (tx, commands) = bounded::<RawPrefetchCommand>(1);
        let pending = commands.clone();
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
                    let cache = DiskMosaicCache::new(cache_root);
                    while let Ok(command) = commands.recv() {
                        for path in command.paths {
                            worker_telemetry.set_prefetch_phase(command.generation, PrefetchPhase::Checking);
                            while worker_state.load(Ordering::Acquire) & RAW_PREFETCH_FOREGROUND != 0 {
                                if worker_generation.load(Ordering::Acquire) != command.generation {
                                    break;
                                }
                                thread::sleep(Duration::from_millis(10));
                            }
                            if worker_generation.load(Ordering::Acquire) != command.generation {
                                break;
                            }
                            let Ok(fingerprint) = SourceFingerprint::from_path(&path) else {
                                worker_telemetry.record_prefetch_failure(command.generation);
                                continue;
                            };
                            let recipe_request = DecodeRequest::new(&path);
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
                                continue;
                            }
                            let cancelled = || {
                                worker_generation.load(Ordering::Acquire) != command.generation
                                    || worker_state.load(Ordering::Acquire) & RAW_PREFETCH_FOREGROUND != 0
                            };
                            let Some(decode_permit) = worker_gate.acquire_prefetch(cancelled) else {
                                break;
                            };
                            // A foreground cache read may have populated this
                            // path while the speculative worker waited for the
                            // shared decoder permit.
                            if worker_generation.load(Ordering::Acquire) != command.generation
                                || worker_state.load(Ordering::Acquire) & RAW_PREFETCH_FOREGROUND != 0
                                || cache.contains(key)
                            {
                                if worker_generation.load(Ordering::Acquire) == command.generation
                                    && worker_state.load(Ordering::Acquire) & RAW_PREFETCH_FOREGROUND == 0
                                {
                                    worker_telemetry.record_prefetch_cached(command.generation);
                                }
                                continue;
                            }
                            worker_telemetry.set_prefetch_phase(command.generation, PrefetchPhase::Decoding);
                            let token =
                                GenerationToken::new(Arc::clone(&worker_generation), command.generation);
                            let mut request = DecodeRequest::new(&path);
                            request.cancellation = Some(token);
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
                                || worker_generation.load(Ordering::Acquire) != command.generation
                            {
                                continue;
                            }
                            worker_telemetry.set_prefetch_phase(command.generation, PrefetchPhase::Writing);
                            let mosaic_bytes = u64::try_from(output.mosaic.byte_len()).unwrap_or(u64::MAX);
                            match cache.store(key, &output.mosaic) {
                                Ok(_) => {
                                    worker_telemetry.record_prefetch_stored(command.generation, mosaic_bytes);
                                    log::debug!("prefetched full RAW mosaic: {}", path.display());
                                }
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
                        if worker_generation.load(Ordering::Acquire) == command.generation {
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
            tx,
            pending,
            generation,
            state,
            decode_gate,
            telemetry,
            enabled,
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
    /// write-back owns the current frame; this worker warms two previous, then
    /// five following paths without racing to decode the current frame twice.
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
        let paths = raw_prefetch_paths(gallery, selected, direction);
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

fn raw_prefetch_paths(gallery: &[PathBuf], selected: usize, direction: NavDirection) -> Vec<PathBuf> {
    if selected >= gallery.len() {
        return Vec::new();
    }
    let (behind, ahead) = direction.window();
    let mut paths = Vec::with_capacity(behind + ahead);
    for delta in 1..=behind {
        if let Some(index) = selected.checked_sub(delta) {
            paths.push(gallery[index].clone());
        }
    }
    for delta in 1..=ahead {
        if let Some(path) = selected.checked_add(delta).and_then(|index| gallery.get(index)) {
            paths.push(path.clone());
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

#[derive(Debug, Clone)]
pub struct ThumbnailReady {
    pub index: usize,
    /// CPU-side RGBA8 pixels; upload to a persistent texture atlas on UI side.
    pub width: u32,
    pub height: u32,
    pub pixels: Vec<u8>,
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
                if let Some(result) = loader(job) {
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
        for job in jobs {
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

pub fn is_supported(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .is_some_and(|extension| {
            extension.eq_ignore_ascii_case("cr3")
                || extension.eq_ignore_ascii_case("dng")
                || extension.eq_ignore_ascii_case("tif")
                || extension.eq_ignore_ascii_case("tiff")
        })
}

pub fn scan_folder(folder: &Path) -> Vec<PathBuf> {
    let Ok(entries) = std::fs::read_dir(folder) else {
        return Vec::new();
    };
    let mut paths = entries
        .filter_map(Result::ok)
        .filter_map(|entry| {
            let path = entry.path();
            let metadata = std::fs::symlink_metadata(&path).ok()?;
            (metadata.file_type().is_file() && is_supported(&path)).then_some(path)
        })
        .collect::<Vec<_>>();
    paths.sort_by_cached_key(|p| {
        p.file_name()
            .map(|n| n.to_string_lossy().to_ascii_lowercase())
            .unwrap_or_default()
    });
    paths.truncate(MAX_ITEMS);
    paths
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
    let mut candidates = std::fs::read_dir(folder)
        .ok()?
        .filter_map(Result::ok)
        .filter_map(|entry| {
            let path = entry.path();
            let metadata = std::fs::symlink_metadata(&path).ok()?;
            (metadata.file_type().is_file() && is_supported(&path)).then_some(path)
        })
        .collect::<Vec<_>>();
    candidates.sort_by_cached_key(|p| {
        p.file_name()
            .map(|n| n.to_string_lossy().to_ascii_lowercase())
            .unwrap_or_default()
    });
    candidates.into_iter().next()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write_file(path: &std::path::Path) {
        std::fs::write(path, b"synthetic").expect("write test file");
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
            index,
            width: 1,
            height: 1,
            pixels: vec![index as u8, 0, 0, 255],
        }
    }

    fn raw_prefetcher_without_worker() -> RawPrefetcher {
        let (tx, pending) = bounded(1);
        let decode_gate = Arc::new(DecodeGate::new());
        let telemetry = Arc::new(CacheTelemetry::new(true, 1024));
        RawPrefetcher {
            tx,
            pending,
            generation: decode_gate.speculative_generation(),
            state: Arc::new(AtomicU8::new(0)),
            decode_gate,
            telemetry,
            enabled: true,
        }
    }

    fn recv_test<T>(rx: &Receiver<T>) -> T {
        rx.recv_timeout(Duration::from_secs(2))
            .expect("synchronized test worker did not make progress")
    }

    #[test]
    fn extension_filter_is_case_insensitive() {
        assert!(is_supported(Path::new("a.CR3")));
        assert!(!is_supported(Path::new("a.CR2")));
        assert!(is_supported(Path::new("a.DNG")));
        assert!(is_supported(Path::new("a.TIFF")));
        assert!(!is_supported(Path::new("a.jpg")));
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
            raw_prefetch_paths(&paths, 10, NavDirection::None),
            "forward travel keeps the default forward-biased window"
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
