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
    clippy::while_let_loop,
    clippy::too_many_arguments
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
use rrrah_cache::{
    CacheKey, DEFAULT_MAX_DISK_CACHE_BYTES, DEFAULT_RAM_CACHE_BYTES, DiskMosaicCache, MosaicRamCache,
    SourceFingerprint,
};
use rrrah_core::DecodedMosaic;
use rrrah_decode::{DecodeRequest, DecodeTimings, GenerationToken, NativeRawDecoder, RawDecoder};
use rrrah_gpu::{FilmstripRenderer, GpuUploadTimings, HudCard, HudRenderer, RawRenderer, ViewParameters};
use winit::{
    application::ApplicationHandler,
    dpi::{LogicalSize, PhysicalPosition, PhysicalSize},
    event::{ElementState, MouseButton, MouseScrollDelta, WindowEvent},
    event_loop::{ActiveEventLoop, ControlFlow, EventLoop, EventLoopProxy, OwnedDisplayHandle},
    keyboard::{KeyCode, ModifiersState, PhysicalKey},
    window::{Window, WindowId},
};

mod cache_maintenance;
mod cache_telemetry;
mod cache_writer;
mod decode_gate;
mod filmstrip_ui;
mod gallery;
mod model_swap;
mod pipeline_telemetry;
mod raw_load_policy;

use cache_telemetry::{CacheTelemetry, CacheTelemetrySnapshot};
use cache_writer::CacheWriter;
use decode_gate::{DecodeGate, ForegroundTicket};
use filmstrip_ui::{FolderStrip, THUMB_CACHE_CAPACITY, ThumbCache};
use gallery::NavDirection;
use pipeline_telemetry::{
    CacheRoute, FrameSubmitTimings, FrontendTimings, PipelineSnapshot, PipelineStageState, RawKind,
};

#[derive(Debug, Clone, Copy, Default, clap::ValueEnum)]
enum UntaggedColor {
    #[default]
    Strict,
    Srgb,
}
impl UntaggedColor {
    fn apply(self, request: &mut DecodeRequest) {
        request.assume_untagged_srgb = matches!(self, Self::Srgb);
        request.assume_untagged_linear_srgb = false;
    }
}

#[derive(Debug, Clone, Copy, clap::ValueEnum)]
enum RlaAlphaChoice {
    Straight,
    Premultiplied,
}

#[derive(Debug, Clone, Copy)]
struct RasterInterpretation {
    color: UntaggedColor,
    alpha: Option<rrrah_decode::RlaAlphaMode>,
}
impl From<UntaggedColor> for RasterInterpretation {
    fn from(color: UntaggedColor) -> Self {
        Self { color, alpha: None }
    }
}
impl RasterInterpretation {
    fn apply(self, request: &mut DecodeRequest) {
        self.color.apply(request);
        request.rla_alpha_mode = self.alpha;
    }
}
impl Cli {
    fn raster_interpretation(&self) -> RasterInterpretation {
        RasterInterpretation {
            color: self.untagged_color,
            alpha: self.rla_alpha.map(|value| match value {
                RlaAlphaChoice::Straight => rrrah_decode::RlaAlphaMode::Straight,
                RlaAlphaChoice::Premultiplied => rrrah_decode::RlaAlphaMode::Premultiplied,
            }),
        }
    }
}

#[derive(Debug, Parser)]
#[command(name = "rrrah", about = "Native RAW and raster image viewer")]
struct Cli {
    /// Explicit alpha association for RLA files; omitted leaves ambiguous alpha refused.
    #[arg(long, value_enum, conflicts_with = "inspect")]
    rla_alpha: Option<RlaAlphaChoice>,
    /// Explicit interpretation of raster images without a color profile (viewer only).
    #[arg(long, value_enum, default_value = "strict", conflicts_with = "inspect")]
    untagged_color: UntaggedColor,
    /// Develop RAW on the CPU with AHD/X-Trans and spatial highlight recovery.
    #[arg(long)]
    raw_quality: bool,
    /// Disable spatial highlight recovery in the quality path.
    #[arg(long)]
    no_highlight_recovery: bool,
    /// Monotone tone points, e.g. 0:0,0.25:0.15,0.75:0.9,1:1; implies quality.
    #[arg(long)]
    tone_curve: Option<String>,
    /// Decode and print metadata/timings without opening a window.
    #[arg(long)]
    inspect: bool,
    /// Select a zero-based page, texture or scientific-array section.
    #[arg(long, default_value_t = 0)]
    image_index: usize,
    /// Explicit scalar window: NRRD/MRC raw units, FITS/DICOM rescaled units; minimum maximum.
    #[arg(long, num_args = 2, allow_hyphen_values = true, value_names = ["MIN", "MAX"])]
    window: Option<Vec<f32>>,
    /// Do not read or write the decoded-mosaic disk cache.
    #[arg(long)]
    no_cache: bool,
    /// Override the cache directory.
    #[arg(long)]
    cache_dir: Option<PathBuf>,
    #[arg(long)]
    disk_cache_mb: Option<u64>,
    #[arg(long)]
    disk_cache_count: Option<usize>,
    #[arg(long)]
    disk_cache_ttl_secs: Option<u64>,
    /// Pixel capacity held by active/pending persistent-cache writes, in MiB.
    #[arg(long, default_value_t = 128)]
    cache_write_queue_mb: u64,
    /// Shared cap for RAW restore and retained background-write reservations; not total process RAM.
    #[arg(long)]
    managed_memory_mb: Option<u64>,
    /// Renderer-owned raster texture cap in MiB; excludes staging/driver/in-flight retention.
    #[arg(long)]
    raster_gpu_mb: Option<u64>,
    /// Owned model vertex/depth GPU footprint cap in MiB; excludes driver/in-flight retention.
    #[arg(long)]
    model_gpu_mb: Option<u64>,
    /// Shared owned RAW/raster/model/thumbnail GPU resource cap in MiB; excludes staging/driver retention.
    #[arg(long)]
    gpu_memory_mb: Option<u64>,
    /// GPU API: auto, metal, vulkan, dx12 or gl; explicit choices never fall back.
    #[arg(long, default_value = "auto")]
    gpu_backend: rrrah_gpu::GpuBackend,
    /// Adapter vendor filter: any or nvidia; nvidia refuses other vendors.
    #[arg(long, default_value = "any")]
    gpu_vendor: rrrah_gpu::GpuVendor,
    /// Owned RAW atlas footprint cap in MiB, including tile halos; excludes staging/driver retention.
    #[arg(long)]
    raw_gpu_mb: Option<u64>,
    /// In-memory decoded-mosaic cache budget in MiB (0 disables the RAM cache).
    #[arg(long)]
    ram_cache_mb: Option<u64>,
    /// Maximum decoded RAW entries in the RAM cache (0 rejects all entries).
    #[arg(long)]
    ram_cache_count: Option<usize>,
    /// RAM-entry lifetime in seconds since insertion; hits do not renew it.
    #[arg(long)]
    ram_cache_ttl_secs: Option<u64>,
    /// Prepared raster RAM-cache capacity in MiB (0 refuses admission).
    #[arg(long, default_value_t = 512)]
    raster_cache_mb: u64,
    #[arg(long)]
    raster_cache_count: Option<usize>,
    #[arg(long)]
    raster_cache_ttl_secs: Option<u64>,
    /// Retained model RAM-cache capacity in MiB.
    #[arg(long, default_value_t = 512)]
    model_cache_mb: u64,
    #[arg(long)]
    model_cache_count: Option<usize>,
    #[arg(long)]
    model_cache_ttl_secs: Option<u64>,
    /// Temporary STL/OBJ/PLY/OFF model swap capacity in MiB (0 disables).
    #[arg(long, default_value_t = 0)]
    model_swap_mb: u64,
    #[arg(long)]
    model_swap_count: Option<usize>,
    #[arg(long)]
    model_swap_ttl_secs: Option<u64>,
    #[arg(long, default_value_t = 128)]
    model_swap_queue_mb: u64,
    #[arg(long, default_value_t = 4, value_parser = parse_swap_queue_count)]
    model_swap_queue_count: usize,
    #[arg(long, default_value_t = 512)]
    model_swap_restore_mb: u64,
    /// Temporary RAW swap capacity in MiB (0 disables; requires cache enabled).
    #[arg(long, default_value_t = 0)]
    swap_mb: u64,
    /// Maximum temporary RAW swap objects.
    #[arg(long)]
    swap_count: Option<usize>,
    /// Temporary RAW swap-entry lifetime since publication, in seconds.
    #[arg(long)]
    swap_ttl_secs: Option<u64>,
    /// Pixel capacity retained by pending/in-flight swap writes, in MiB.
    #[arg(long, default_value_t = 128)]
    swap_queue_mb: u64,
    /// Maximum waiting RAW swap writes, excluding the active write (0 disables spills).
    #[arg(long, default_value_t = 4, value_parser = parse_swap_queue_count)]
    swap_queue_count: usize,
    /// RAM capacity of live RAW frames restored from swap, in MiB (0 rejects restores).
    #[arg(long, default_value_t = 512)]
    swap_restore_mb: u64,
    /// Temporary prepared-raster swap capacity in MiB (0 disables).
    #[arg(long, default_value_t = 0)]
    raster_swap_mb: u64,
    #[arg(long)]
    raster_swap_count: Option<usize>,
    /// Fixed lifetime since raster swap publication; hits do not renew it.
    #[arg(long)]
    raster_swap_ttl_secs: Option<u64>,
    /// Active/pending raster swap pixel capacity in MiB.
    #[arg(long, default_value_t = 128)]
    raster_swap_queue_mb: u64,
    /// Maximum waiting raster swap writes, excluding the active write (0 disables spills).
    #[arg(long, default_value_t = 4, value_parser = parse_swap_queue_count)]
    raster_swap_queue_count: usize,
    /// Live restored raster pixel capacity in MiB (0 refuses restoration).
    #[arg(long, default_value_t = 512)]
    raster_swap_restore_mb: u64,
    /// Files to preload behind the current frame in the direction of navigation.
    #[arg(long, visible_alias = "prefetch-previous", default_value_t = gallery::PREFETCH_BEHIND)]
    prefetch_behind: usize,
    /// Files to preload ahead in the direction of navigation; zero disables that side.
    #[arg(long, visible_alias = "prefetch-next", default_value_t = gallery::PREFETCH_AHEAD)]
    prefetch_ahead: usize,
    #[arg(value_name = "IMAGE")]
    path: Option<PathBuf>,
}

fn parse_swap_queue_count(value: &str) -> Result<usize, String> {
    let count = value.parse::<usize>().map_err(|error| error.to_string())?;
    if count > rrrah_cache::MAX_SWAP_QUEUE_COUNT {
        return Err(format!("must be at most {}", rrrah_cache::MAX_SWAP_QUEUE_COUNT));
    }
    Ok(count)
}

impl Cli {
    fn raster_swap_config(&self) -> Option<rrrah_cache::ImageSwapConfig> {
        (!self.no_cache && self.raster_swap_mb > 0 && self.raster_swap_count != Some(0)).then_some(
            rrrah_cache::ImageSwapConfig {
                limits: rrrah_cache::CacheLimits {
                    max_bytes: self.raster_swap_mb.saturating_mul(1024 * 1024),
                    max_entries: self.raster_swap_count,
                    ttl: self.raster_swap_ttl_secs.map(Duration::from_secs),
                },
                queue_bytes: self.raster_swap_queue_mb.saturating_mul(1024 * 1024),
                queue_count: self.raster_swap_queue_count,
                restore_bytes: self.raster_swap_restore_mb.saturating_mul(1024 * 1024),
            },
        )
    }
    fn model_swap_config(&self) -> Option<rrrah_cache::ImageSwapConfig> {
        (!self.no_cache && self.model_swap_mb > 0 && self.model_swap_count != Some(0)).then_some(
            rrrah_cache::ImageSwapConfig {
                limits: rrrah_cache::CacheLimits {
                    max_bytes: self.model_swap_mb.saturating_mul(1024 * 1024),
                    max_entries: self.model_swap_count,
                    ttl: self.model_swap_ttl_secs.map(Duration::from_secs),
                },
                queue_bytes: self.model_swap_queue_mb.saturating_mul(1024 * 1024),
                queue_count: self.model_swap_queue_count,
                restore_bytes: self.model_swap_restore_mb.saturating_mul(1024 * 1024),
            },
        )
    }
    fn model_limits(&self) -> rrrah_cache::CacheLimits {
        rrrah_cache::CacheLimits {
            max_bytes: self.model_cache_mb.saturating_mul(1024 * 1024),
            max_entries: self.model_cache_count,
            ttl: self.model_cache_ttl_secs.map(Duration::from_secs),
        }
    }
    fn raster_limits(&self) -> rrrah_cache::CacheLimits {
        rrrah_cache::CacheLimits {
            max_bytes: self.raster_cache_mb.saturating_mul(1024 * 1024),
            max_entries: self.raster_cache_count,
            ttl: self.raster_cache_ttl_secs.map(Duration::from_secs),
        }
    }
    fn disk_limits(&self) -> rrrah_cache::CacheLimits {
        rrrah_cache::CacheLimits {
            max_bytes: self
                .disk_cache_mb
                .map_or(DEFAULT_MAX_DISK_CACHE_BYTES, |mb| mb.saturating_mul(1024 * 1024)),
            max_entries: self.disk_cache_count,
            ttl: self.disk_cache_ttl_secs.map(Duration::from_secs),
        }
    }
    fn scalar_window(&self) -> Result<Option<rrrah_decode::ScalarWindow>> {
        self.window
            .as_ref()
            .map(|bounds| rrrah_decode::ScalarWindow::new(bounds[0], bounds[1]).map_err(Into::into))
            .transpose()
    }
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
        lease: Option<rrrah_cache::CacheLease<DecodedMosaic>>,
        raw_kind: RawKind,
        cache_route: CacheRoute,
        elapsed: Duration,
        requested_at: Instant,
        ready_published_at: Instant,
        frontend: FrontendTimings,
        decode: Option<DecodeTimings>,
    },
    ModelReady {
        generation: u64,
        mesh: rrrah_decode::DecodedModel,
        lease: Option<rrrah_cache::CacheLease<rrrah_decode::DecodedModel>>,
        elapsed: Duration,
    },
    RasterReady {
        generation: u64,
        image_index: usize,
        scalar_window: Option<rrrah_decode::ScalarWindow>,
        raster: rrrah_core::DecodedRaster,
        lease: Option<rrrah_cache::CacheLease<rrrah_core::DecodedRaster>>,
        assumed_srgb: bool,
        development: Option<rrrah_core::develop::DevelopOptions>,
        elapsed: Duration,
    },
    Failed {
        generation: u64,
        error: String,
    },
}

fn main() -> Result<()> {
    env_logger::init();
    let cli = Cli::parse();
    let raster_interpretation = cli.raster_interpretation();
    let development = if cli.raw_quality || cli.tone_curve.is_some() || cli.no_highlight_recovery {
        let curve = match &cli.tone_curve {
            Some(points) => parse_tone_curve(points)?,
            None => rrrah_core::develop::MonotoneCurve::identity(),
        };
        Some(rrrah_core::develop::DevelopOptions {
            recover_highlights: !cli.no_highlight_recovery,
            curve,
        })
    } else {
        None
    };
    if cli.image_index != 0 && cli.path.is_none() {
        bail!("--image-index requires an image path");
    }
    let scalar_window = cli.scalar_window()?;
    if scalar_window.is_some() && cli.path.is_none() {
        bail!("--window requires an image path");
    }
    let disk_limits = cli.disk_limits();
    let raster_limits = cli.raster_limits();
    let model_limits = cli.model_limits();
    let raster_swap_config = cli.raster_swap_config();
    let model_swap_config = cli.model_swap_config();
    let cache_root = cli.cache_dir.or_else(default_cache_dir);
    if cli.inspect {
        let path = cli
            .path
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("--inspect requires an image path"))?;
        if !path.is_file() {
            bail!("image path is not a regular file: {}", path.display());
        }
        inspect_with_limits(
            path,
            cache_root.as_deref(),
            cli.no_cache,
            cli.image_index,
            scalar_window,
            disk_limits,
            cli.managed_memory_mb
                .map(|mb| rrrah_core::MemoryBudget::new(mb.saturating_mul(1024 * 1024)))
                .as_ref(),
            development.as_ref(),
        )
    } else {
        let ram_cache_bytes = cli
            .ram_cache_mb
            .map_or(DEFAULT_RAM_CACHE_BYTES, |mb| mb.saturating_mul(1024 * 1024));
        let ram_cache_limits = rrrah_cache::CacheLimits {
            max_bytes: ram_cache_bytes,
            max_entries: cli.ram_cache_count,
            ttl: cli.ram_cache_ttl_secs.map(Duration::from_secs),
        };
        let (raster_gpu_budget, model_gpu_budget, raw_gpu_budget, filmstrip_gpu_budget) = viewer_gpu_budgets(
            cli.gpu_memory_mb,
            cli.raster_gpu_mb,
            cli.model_gpu_mb,
            cli.raw_gpu_mb,
        );
        run_viewer(
            cli.path,
            cache_root,
            cli.no_cache,
            ram_cache_limits,
            cli.image_index,
            disk_limits,
            scalar_window,
            gallery::PrefetchWindow {
                behind: cli.prefetch_behind,
                ahead: cli.prefetch_ahead,
            },
            raster_interpretation,
            (cli.swap_mb > 0 && cli.swap_count != Some(0)).then_some(rrrah_cache::MosaicSwapConfig {
                limits: rrrah_cache::CacheLimits {
                    max_bytes: cli.swap_mb.saturating_mul(1024 * 1024),
                    max_entries: cli.swap_count,
                    ttl: cli.swap_ttl_secs.map(Duration::from_secs),
                },
                queue_bytes: cli.swap_queue_mb.saturating_mul(1024 * 1024),
                queue_count: cli.swap_queue_count,
                restore_bytes: cli.swap_restore_mb.saturating_mul(1024 * 1024),
            }),
            raster_swap_config,
            model_swap_config,
            cli.cache_write_queue_mb.saturating_mul(1024 * 1024),
            cli.managed_memory_mb
                .map(|mb| rrrah_cache::MemoryBudget::new(mb.saturating_mul(1024 * 1024))),
            raster_limits,
            model_limits,
            raster_gpu_budget,
            model_gpu_budget,
            raw_gpu_budget,
            filmstrip_gpu_budget,
            cli.gpu_backend,
            cli.gpu_vendor,
            development,
        )
    }
}

// Queue occupancy counts references, not new allocations. Pixel owners retain
// their original managed reservation while a write job holds them.
fn retained_queue_budget(bytes: u64) -> rrrah_cache::MemoryBudget {
    rrrah_cache::MemoryBudget::new(bytes)
}

fn viewer_gpu_budgets(
    shared_mb: Option<u64>,
    raster_mb: Option<u64>,
    model_mb: Option<u64>,
    raw_mb: Option<u64>,
) -> (
    Option<rrrah_core::MemoryBudget>,
    Option<rrrah_core::MemoryBudget>,
    Option<rrrah_core::MemoryBudget>,
    Option<rrrah_core::MemoryBudget>,
) {
    let shared = shared_mb.map(|mb| rrrah_core::MemoryBudget::new(mb.saturating_mul(1024 * 1024)));
    let budget = |local: Option<u64>| match &shared {
        Some(parent) => Some(parent.child(local.map_or(parent.limit(), |mb| mb.saturating_mul(1024 * 1024)))),
        None => local.map(|mb| rrrah_core::MemoryBudget::new(mb.saturating_mul(1024 * 1024))),
    };
    (
        budget(raster_mb),
        budget(model_mb),
        budget(raw_mb),
        budget(None).filter(|_| shared.is_some()),
    )
}

fn disk_cache_with_limits(root: PathBuf, limits: rrrah_cache::CacheLimits) -> DiskMosaicCache {
    DiskMosaicCache::with_max_bytes(root, limits.max_bytes)
        .with_max_entries(limits.max_entries)
        .with_ttl(limits.ttl)
}

fn default_cache_dir() -> Option<PathBuf> {
    ProjectDirs::from("org", "rrrah", "rrrah").map(|dirs| dirs.cache_dir().join("mosaics"))
}

#[cfg(test)]
fn inspect(
    path: &PathBuf,
    cache_root: Option<&std::path::Path>,
    no_cache: bool,
    image_index: usize,
    scalar_window: Option<rrrah_decode::ScalarWindow>,
) -> Result<()> {
    inspect_with_limits(
        path,
        cache_root,
        no_cache,
        image_index,
        scalar_window,
        rrrah_cache::CacheLimits::bytes(DEFAULT_MAX_DISK_CACHE_BYTES),
        None,
        None,
    )
}

fn inspect_with_limits(
    path: &PathBuf,
    cache_root: Option<&std::path::Path>,
    no_cache: bool,
    image_index: usize,
    scalar_window: Option<rrrah_decode::ScalarWindow>,
    disk_limits: rrrah_cache::CacheLimits,
    managed_budget: Option<&rrrah_core::MemoryBudget>,
    development: Option<&rrrah_core::develop::DevelopOptions>,
) -> Result<()> {
    let started = Instant::now();
    let mut decode_request = DecodeRequest::new(path);
    decode_request.image_index = image_index;
    decode_request.memory_budget = managed_budget.cloned();
    if rrrah_decode::is_supported_model_path(path) {
        if scalar_window.is_some() {
            bail!("scalar window does not apply to a model");
        }
        let mesh = rrrah_decode::decode_model(&decode_request)?;
        println!(
            "{}: {} triangles, bounds {:?}, decoded in {:.2?}",
            mesh.format_name(),
            mesh.triangle_count(),
            mesh.bounds(),
            started.elapsed()
        );
        print_managed_budget(managed_budget);
        return Ok(());
    }

    if rrrah_decode::image_source_kind(&decode_request)? == rrrah_decode::ImageSourceKind::Raster {
        let raster = if let Some(window) = scalar_window {
            rrrah_decode::decode_raster_with_window(&decode_request, window)?
        } else {
            rrrah_decode::decode_raster(&decode_request)?
        };
        let precision = match raster.pixels() {
            rrrah_core::RasterPixels::Rgba8(_) => "RGBA8",
            rrrah_core::RasterPixels::Rgba16(_) => "RGBA16",
            rrrah_core::RasterPixels::Rgba32Float(_) => "RGBA32F",
        };
        println!(
            "raster: {}x{} {precision}, image_index={image_index}, image_count={}",
            raster.width(),
            raster.height(),
            raster.image_count()
        );
        let color_space = match raster.color_space() {
            rrrah_core::RasterColorSpace::Icc(profile) => format!("ICC ({} bytes)", profile.len()),
            other => format!("{other:?}"),
        };
        println!(
            "color_space: {color_space}, sample_scale: {}",
            raster.sample_scale()
        );
        println!("total: {:.2?}", started.elapsed());
        print_managed_budget(managed_budget);
        return Ok(());
    }
    if scalar_window.is_some() {
        bail!("--window requires a scientific scalar array");
    }
    if image_index != 0 {
        bail!("sensor image index {image_index} is unsupported");
    }
    let recipe = NativeRawDecoder
        .mosaic_recipe(&decode_request)
        .map_err(|error| anyhow::anyhow!(error))?;
    let cache = cache_root.map(|root| disk_cache_with_limits(root.to_path_buf(), disk_limits));
    let fingerprint = if no_cache {
        None
    } else {
        Some(SourceFingerprint::from_path(path).context("fingerprint RAW")?)
    };
    if let (Some(cache), Some(fingerprint)) = (&cache, &fingerprint) {
        let key = CacheKey::for_mosaic_recipe(fingerprint, 0, recipe);
        if let Some(hit) = cache
            .load_with_cancel(key, managed_budget, || false)
            .context("read decoded-mosaic cache")?
        {
            inspect_development(&hit.mosaic, &decode_request, development)?;
            print_metadata(&hit.mosaic, true, hit.elapsed, started.elapsed(), None);
            print_managed_budget(managed_budget);
            return Ok(());
        }
    }
    let output = NativeRawDecoder
        .decode(&decode_request)
        .map_err(|error| anyhow::anyhow!(error))?;
    if let (Some(cache), Some(fingerprint)) = (&cache, &fingerprint) {
        let key = CacheKey::for_mosaic_recipe(fingerprint, 0, recipe);
        match cache.store(key, &output.mosaic) {
            Ok(_) => {}
            Err(
                error @ (rrrah_cache::CacheError::DiskCountExceeded { .. }
                | rrrah_cache::CacheError::DiskBudgetExceeded { .. }),
            ) => {
                log::debug!("decoded RAW without cache admission: {error}");
            }
            Err(error) => return Err(error).context("write decoded-mosaic cache"),
        }
    }
    inspect_development(&output.mosaic, &decode_request, development)?;
    print_metadata(
        &output.mosaic,
        false,
        output.timings.total,
        started.elapsed(),
        Some(&output.timings),
    );
    print_managed_budget(managed_budget);
    Ok(())
}

fn print_managed_budget(budget: Option<&rrrah_core::MemoryBudget>) {
    if let Some(budget) = budget {
        println!(
            "managed_memory: used={}, peak={}, limit={}",
            budget.used(),
            budget.peak(),
            budget.limit()
        );
    }
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

fn run_viewer(
    path: Option<PathBuf>,
    cache_root: Option<PathBuf>,
    no_cache: bool,
    ram_cache_limits: rrrah_cache::CacheLimits,
    image_index: usize,
    disk_limits: rrrah_cache::CacheLimits,
    scalar_window: Option<rrrah_decode::ScalarWindow>,
    prefetch_window: gallery::PrefetchWindow,
    untagged_color: RasterInterpretation,
    swap_config: Option<rrrah_cache::MosaicSwapConfig>,
    raster_swap_config: Option<rrrah_cache::ImageSwapConfig>,
    model_swap_config: Option<rrrah_cache::ImageSwapConfig>,
    write_queue_bytes: u64,
    managed_budget: Option<rrrah_cache::MemoryBudget>,
    raster_limits: rrrah_cache::CacheLimits,
    model_limits: rrrah_cache::CacheLimits,
    raster_gpu_budget: Option<rrrah_core::MemoryBudget>,
    model_gpu_budget: Option<rrrah_core::MemoryBudget>,
    raw_gpu_budget: Option<rrrah_core::MemoryBudget>,
    filmstrip_gpu_budget: Option<rrrah_core::MemoryBudget>,
    gpu_backend: rrrah_gpu::GpuBackend,
    gpu_vendor: rrrah_gpu::GpuVendor,
    development: Option<rrrah_core::develop::DevelopOptions>,
) -> Result<()> {
    let (sender, receiver) = unbounded();
    let event_loop = EventLoop::<WakeEvent>::with_user_event()
        .build()
        .context("create event loop")?;
    let proxy = event_loop.create_proxy();
    if let Some(path) = &path {
        if !path.is_file() {
            bail!("image path is not a regular file: {}", path.display());
        }
    }
    let decode_gate = Arc::new(DecodeGate::new());
    let cache_telemetry = Arc::new(CacheTelemetry::new(
        cache_root.is_some() && !no_cache,
        disk_limits.max_bytes,
    ));
    let mut foreground_loader = ForegroundLoader::new(
        cache_root.clone(),
        no_cache,
        ram_cache_limits,
        sender,
        proxy,
        Arc::clone(&decode_gate),
        Arc::clone(&cache_telemetry),
        swap_config,
        raster_swap_config,
        model_swap_config,
        disk_limits,
        write_queue_bytes,
        managed_budget.clone(),
        raster_limits,
        model_limits,
        prefetch_window,
        untagged_color,
    )
    .context("start foreground RAW worker")?;
    foreground_loader.development = development;
    if let Some(path) = &path {
        foreground_loader.submit_initial_with_window(path.clone(), image_index, scalar_window)?;
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
        // The loader now warms RAM and submits decoded neighbours to write-back.
        // Keep the independent disk warmer only when RAW RAM admission is disabled.
        if ram_cache_limits.max_bytes > 0 && ram_cache_limits.max_entries != Some(0) {
            gallery::PrefetchWindow { behind: 0, ahead: 0 }
        } else {
            prefetch_window
        },
        disk_limits,
        managed_budget,
        raster_gpu_budget,
        model_gpu_budget,
        raw_gpu_budget,
        filmstrip_gpu_budget,
        gpu_backend,
        gpu_vendor,
        untagged_color,
    );
    app.image_index = image_index;
    app.scalar_window = scalar_window;
    event_loop.run_app(&mut app).context("run event loop")?;
    Ok(())
}

#[derive(Debug, Clone, Copy)]
enum WakeEvent {
    LoadProgress,
}

#[derive(Debug)]
struct LoadRequest {
    decode_first: bool,
    automatic_policy: bool,
    path: PathBuf,
    image_index: usize,
    scalar_window: Option<rrrah_decode::ScalarWindow>,
    generation: u64,
    requested_at: Instant,
    foreground: ForegroundTicket,
    development: Option<rrrah_core::develop::DevelopOptions>,
}

#[derive(Debug, Clone, Copy)]
struct PendingFirstPresent {
    generation: u64,
    requested_at: Instant,
}

#[derive(Debug, Clone, PartialEq)]
struct ForegroundIdentity {
    path: PathBuf,
    source: gallery::SourceStamp,
    image_index: usize,
    scalar_window: Option<rrrah_decode::ScalarWindow>,
    development: Option<rrrah_core::develop::DevelopOptions>,
    decode_first: bool,
    automatic_policy: bool,
}

type ForegroundFlight = Arc<std::sync::Mutex<Option<(ForegroundIdentity, u64)>>>;

fn finish_foreground_flight(flight: &ForegroundFlight, generation: u64) {
    let mut flight = flight.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
    if flight.as_ref().is_some_and(|(_, current)| *current == generation) {
        *flight = None;
    }
}

/// One persistent foreground decoder with a latest-wins queue. Serializing
/// requests is essential while a plane is inside entropy decode: rapid
/// navigation keeps at most one active decode and one pending path instead of
/// spawning an unbounded set of competing decoder threads.
struct ForegroundLoader {
    decode_first: bool,
    automatic_policy: bool,
    flight: ForegroundFlight,
    tx: Sender<LoadRequest>,
    pending: Receiver<LoadRequest>,
    generation: Arc<AtomicU64>,
    decode_gate: Arc<DecodeGate>,
    telemetry: Arc<CacheTelemetry>,
    development: Option<rrrah_core::develop::DevelopOptions>,
}

impl Drop for ForegroundLoader {
    fn drop(&mut self) {
        self.generation.fetch_add(1, Ordering::AcqRel);
        *self
            .flight
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = None;
        while self.pending.try_recv().is_ok() {}
    }
}

/// Services cache expiry while idle without creating another cache owner/thread.
/// A ready foreground request is returned before running background maintenance.
fn receive_with_maintenance<T>(
    requests: &Receiver<T>,
    interval: Option<Duration>,
    mut maintain: impl FnMut(),
) -> Result<T, crossbeam_channel::RecvError> {
    let Some(interval) = interval else {
        return requests.recv();
    };
    loop {
        match requests.recv_timeout(interval) {
            Ok(request) => return Ok(request),
            Err(crossbeam_channel::RecvTimeoutError::Disconnected) => {
                return Err(crossbeam_channel::RecvError);
            }
            Err(crossbeam_channel::RecvTimeoutError::Timeout) => maintain(),
        }
    }
}

fn foreground_neighbour_supported(path: &Path) -> bool {
    // TIFF can contain either a raster or a sensor mosaic. Let the worker's
    // bounded content classifier choose the cache instead of excluding both.
    let tiff = path
        .extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| {
            extension.eq_ignore_ascii_case("tif") || extension.eq_ignore_ascii_case("tiff")
        });
    rrrah_decode::is_supported_model_path(path)
        || (rrrah_decode::is_supported_image_path(path)
            && (!rrrah_decode::is_supported_raw_path(path) || tiff))
}

impl ForegroundLoader {
    fn new(
        cache_root: Option<PathBuf>,
        no_cache: bool,
        ram_cache_limits: rrrah_cache::CacheLimits,
        sender: Sender<LoadEvent>,
        proxy: EventLoopProxy<WakeEvent>,
        decode_gate: Arc<DecodeGate>,
        telemetry: Arc<CacheTelemetry>,
        swap_config: Option<rrrah_cache::MosaicSwapConfig>,
        raster_swap_config: Option<rrrah_cache::ImageSwapConfig>,
        model_swap_config: Option<rrrah_cache::ImageSwapConfig>,
        disk_limits: rrrah_cache::CacheLimits,
        write_queue_bytes: u64,
        managed_budget: Option<rrrah_cache::MemoryBudget>,
        raster_limits: rrrah_cache::CacheLimits,
        model_limits: rrrah_cache::CacheLimits,
        prefetch_window: gallery::PrefetchWindow,
        untagged_color: RasterInterpretation,
    ) -> std::io::Result<Self> {
        let (decode_first, automatic_policy) = match std::env::var("RRRAH_RAW_LOAD_POLICY") {
            Err(std::env::VarError::NotPresent) => (false, false),
            Ok(value) if value == "cache-first" => (false, false),
            Ok(value) if value == "decode-first" => (true, false),
            Ok(value) if value == "auto" => (false, true),
            _ => {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::InvalidInput,
                    "RRRAH_RAW_LOAD_POLICY must be cache-first, decode-first or auto",
                ));
            }
        };
        let (tx, requests) = bounded::<LoadRequest>(1);
        let pending = requests.clone();
        let generation = Arc::new(AtomicU64::new(0));
        let worker_generation = Arc::clone(&generation);
        let flight: ForegroundFlight = Arc::new(std::sync::Mutex::new(None));
        let worker_flight = Arc::clone(&flight);
        let worker_telemetry = Arc::clone(&telemetry);
        let worker_gate = Arc::clone(&decode_gate);
        thread::Builder::new()
            .name("rrrah-raw-decode".into())
            .spawn(move || {
                let admission = |bytes| {
                    managed_budget
                        .as_ref()
                        .map_or_else(|| rrrah_cache::MemoryBudget::new(bytes), |root| root.child(bytes))
                };
                let swap_parent = cache_root.clone();
                let cache = cache_root
                    .filter(|_| !no_cache)
                    .map(|root| disk_cache_with_limits(root, disk_limits));
                // The RAM cache shares the disk cache's key derivation, so it
                // exists exactly when the disk cache does. It is owned solely
                // by this worker thread: no locking on the load path.
                let mut ram_cache = cache
                    .as_ref()
                    .filter(|_| ram_cache_limits.max_bytes > 0 || swap_config.is_some())
                    .map(|_| MosaicRamCache::with_limits(ram_cache_limits));
                if let (Some(ram), Some(parent), Some(config)) =
                    (ram_cache.as_mut(), swap_parent.as_ref(), swap_config)
                {
                    let swap = std::fs::create_dir_all(parent).and_then(|()| {
                        rrrah_cache::MosaicSwapCache::new_with_budgets(
                            parent,
                            config,
                            retained_queue_budget(config.queue_bytes),
                            admission(config.restore_bytes),
                        )
                    });
                    match swap {
                        Ok(swap) => ram.enable_swap(swap),
                        Err(error) => log::warn!("RAW swap unavailable: {error}"),
                    }
                }
                if let Some(cache) = &cache {
                    match cache.usage() {
                        Ok(usage) => worker_telemetry.update_disk_usage(usage),
                        Err(_) => worker_telemetry.record_disk_scan_error(),
                    }
                }
                let cache_writer = cache.as_ref().and_then(|cache| {
                    CacheWriter::spawn_with_budget(
                        cache.clone(),
                        Arc::clone(&worker_generation),
                        Arc::clone(&worker_telemetry),
                        retained_queue_budget(write_queue_bytes),
                    )
                    .map_err(|error| log::warn!("failed to start cache write-back worker: {error}"))
                    .ok()
                });
                let mut raster_cache = raster_cache_with_swap(
                    swap_parent.as_deref(),
                    no_cache,
                    raster_limits,
                    raster_swap_config,
                    managed_budget.as_ref(),
                );
                let mut model_cache = model_cache_with_swap(
                    swap_parent.as_deref(),
                    no_cache,
                    model_limits,
                    model_swap_config,
                    managed_budget.as_ref(),
                );
                let mut neighbours: std::collections::VecDeque<(
                    PathBuf,
                    u64,
                    Option<rrrah_decode::ScalarWindow>,
                )> = std::collections::VecDeque::new();
                let mut load_costs = raw_load_policy::RawLoadCosts::default();
                let mut previous: Option<PathBuf> = None;
                let maintenance_interval = [ram_cache_limits.ttl, raster_limits.ttl, model_limits.ttl]
                    .iter()
                    .any(Option::is_some)
                    .then_some(Duration::from_secs(1));
                let mut maintenance =
                    cache_maintenance::CacheMaintenance::new(maintenance_interval, Instant::now());
                loop {
                    let request = match requests.try_recv() {
                        Ok(request) => request,
                        Err(crossbeam_channel::TryRecvError::Disconnected) => break,
                        Err(crossbeam_channel::TryRecvError::Empty) => {
                            // Ready foreground requests keep priority. Expiry still runs
                            // between neighbours even if their queue never becomes idle.
                            if maintenance.due(Instant::now()) {
                                if let Some(ram) = ram_cache.as_mut() {
                                    ram.spill_expired();
                                }
                                raster_cache.spill_expired();
                                model_cache.spill_expired();
                            }
                            if let Some((path, generation, window)) = neighbours.pop_front() {
                                if worker_generation.load(Ordering::Acquire) == generation {
                                    let mut request = DecodeRequest::new(path);
                                    untagged_color.apply(&mut request);
                                    request.memory_budget = managed_budget.clone();
                                    request.cancellation = Some(GenerationToken::new(
                                        Arc::clone(&worker_generation),
                                        generation,
                                    ));
                                    if rrrah_decode::is_supported_model_path(&request.path) {
                                        if model_limits.max_bytes > 0 && model_limits.max_entries != Some(0) {
                                            let _ = preload_model(&request, &mut model_cache, &worker_gate);
                                        }
                                    } else if matches!(
                                        rrrah_decode::image_source_kind(&request),
                                        Ok(rrrah_decode::ImageSourceKind::Sensor)
                                    ) {
                                        if let (Some(disk), Some(ram)) = (cache.as_ref(), ram_cache.as_mut())
                                        {
                                            if ram_cache_limits.max_bytes > 0
                                                && ram_cache_limits.max_entries != Some(0)
                                            {
                                                let _ = preload_raw(
                                                    &request,
                                                    disk,
                                                    ram,
                                                    cache_writer.as_ref(),
                                                    generation,
                                                    &worker_gate,
                                                );
                                            }
                                        }
                                    } else if raster_limits.max_bytes > 0
                                        && raster_limits.max_entries != Some(0)
                                    {
                                        let warmed =
                                            preload_raster(&request, window, &mut raster_cache, &worker_gate);
                                        if warmed.is_err() && window.is_some() {
                                            let _ = preload_raster(
                                                &request,
                                                None,
                                                &mut raster_cache,
                                                &worker_gate,
                                            );
                                        }
                                    }
                                }
                                continue;
                            }
                            match receive_with_maintenance(&requests, maintenance_interval, || {
                                if let Some(ram) = ram_cache.as_mut() {
                                    ram.spill_expired();
                                }
                                raster_cache.spill_expired();
                                model_cache.spill_expired();
                                maintenance.completed(Instant::now());
                            }) {
                                Ok(request) => request,
                                Err(_) => break,
                            }
                        }
                    };
                    neighbours.clear();
                    let path = request.path.clone();
                    let generation = request.generation;
                    let window = request.scalar_window;
                    execute_load(
                        request,
                        untagged_color,
                        cache.as_ref(),
                        ram_cache.as_mut(),
                        &mut raster_cache,
                        &mut model_cache,
                        cache_writer.as_ref(),
                        managed_budget.as_ref(),
                        Arc::clone(&worker_generation),
                        &sender,
                        &proxy,
                        &worker_telemetry,
                        &mut load_costs,
                    );
                    finish_foreground_flight(&worker_flight, generation);
                    if !no_cache
                        && ((ram_cache_limits.max_bytes > 0 && ram_cache_limits.max_entries != Some(0))
                            || (model_limits.max_bytes > 0 && model_limits.max_entries != Some(0))
                            || (raster_limits.max_bytes > 0 && raster_limits.max_entries != Some(0)))
                        && worker_generation.load(Ordering::Acquire) == generation
                    {
                        let gallery = path.parent().map(gallery::scan_folder).unwrap_or_default();
                        if let Some(selected) = gallery.iter().position(|candidate| candidate == &path) {
                            let direction = previous
                                .as_ref()
                                .filter(|old| old.parent() == path.parent())
                                .and_then(|old| gallery.iter().position(|candidate| candidate == old))
                                .map_or(NavDirection::None, |old| {
                                    if selected < old {
                                        NavDirection::Backward
                                    } else {
                                        NavDirection::Forward
                                    }
                                });
                            neighbours.extend(
                                gallery::neighbour_prefetch_paths(
                                    &gallery,
                                    selected,
                                    direction,
                                    prefetch_window,
                                )
                                .into_iter()
                                .filter(|path| foreground_neighbour_supported(path))
                                .map(|path| (path, generation, window)),
                            );
                        }
                    }
                    previous = Some(path);
                }
            })?;
        Ok(Self {
            decode_first,
            automatic_policy,
            flight,
            tx,
            pending,
            generation,
            decode_gate,
            telemetry,
            development: None,
        })
    }

    #[cfg(test)]
    fn submit_initial(&self, path: PathBuf) -> Result<()> {
        self.submit_initial_image(path, 0)
    }
    #[cfg(test)]
    fn submit_initial_image(&self, path: PathBuf, image_index: usize) -> Result<()> {
        self.submit_initial_with_window(path, image_index, None)
    }
    fn submit_initial_with_window(
        &self,
        path: PathBuf,
        image_index: usize,
        scalar_window: Option<rrrah_decode::ScalarWindow>,
    ) -> Result<()> {
        self.submit_request(path, image_index, scalar_window, false)
            .map(|_| ())
    }

    fn submit(&self, path: PathBuf) -> Result<u64> {
        self.submit_image(path, 0)
    }
    fn submit_image(&self, path: PathBuf, image_index: usize) -> Result<u64> {
        self.submit_image_with_window(path, image_index, None)
    }
    fn submit_image_with_window(
        &self,
        path: PathBuf,
        image_index: usize,
        scalar_window: Option<rrrah_decode::ScalarWindow>,
    ) -> Result<u64> {
        self.submit_request(path, image_index, scalar_window, true)
    }

    fn submit_request(
        &self,
        path: PathBuf,
        image_index: usize,
        scalar_window: Option<rrrah_decode::ScalarWindow>,
        advance: bool,
    ) -> Result<u64> {
        let identity = ForegroundIdentity {
            source: gallery::SourceStamp::read(&path),
            path: path.clone(),
            image_index,
            scalar_window,
            development: self.development.clone(),
            decode_first: self.decode_first,
            automatic_policy: self.automatic_policy,
        };
        let mut flight = self
            .flight
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if identity.source.is_readable() {
            if let Some((current, generation)) = flight.as_ref() {
                if current == &identity && *generation == self.current_generation() {
                    return Ok(*generation);
                }
            }
        }
        let requested_at = Instant::now();
        let generation = if advance {
            self.generation.fetch_add(1, Ordering::AcqRel).wrapping_add(1)
        } else {
            self.current_generation()
        };
        self.telemetry.begin_lookup(generation);
        let result = self.replace_pending(LoadRequest {
            decode_first: self.decode_first,
            automatic_policy: self.automatic_policy,
            path,
            image_index,
            scalar_window,
            generation,
            requested_at,
            foreground: self.decode_gate.request_foreground(),
            development: self.development.clone(),
        });
        *flight = result.as_ref().ok().map(|_| (identity, generation));
        result.map(|()| generation)
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

fn next_image_index(index: usize, count: usize, direction: isize) -> Option<usize> {
    if index >= count {
        return None;
    }
    match direction {
        -1 => index.checked_sub(1),
        1 => index.checked_add(1).filter(|v| *v < count),
        _ => None,
    }
}

fn adjusted_scalar_window(
    window: rrrah_decode::ScalarWindow,
    shift: f64,
    width_factor: f64,
) -> Option<rrrah_decode::ScalarWindow> {
    if !shift.is_finite() || !width_factor.is_finite() || width_factor <= 0. {
        return None;
    }
    let minimum = f64::from(window.minimum());
    let maximum = f64::from(window.maximum());
    let width = maximum - minimum;
    let center = (minimum + maximum) * 0.5 + width * shift;
    let radius = width * width_factor * 0.5;
    rrrah_decode::ScalarWindow::new((center - radius) as f32, (center + radius) as f32).ok()
}

#[derive(Debug)]
enum ModelLoadError {
    Decode(rrrah_decode::ModelDecodeError),
    Message(String),
}
impl From<&str> for ModelLoadError {
    fn from(value: &str) -> Self {
        Self::Message(value.into())
    }
}
impl From<String> for ModelLoadError {
    fn from(value: String) -> Self {
        Self::Message(value)
    }
}
impl std::fmt::Display for ModelLoadError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Decode(error) => error.fmt(f),
            Self::Message(message) => f.write_str(message),
        }
    }
}
impl ModelLoadError {
    fn retryable_memory_pressure(&self) -> bool {
        use rrrah_decode::{DecodeError as D, ModelDecodeError as M};
        let memory = match self {
            Self::Decode(M::Memory(error)) | Self::Decode(M::Source(D::Memory(error))) => Some(error),
            Self::Decode(M::Stl(rrrah_decode::StlDecodeError::Source(D::Memory(error)))) => Some(error),
            Self::Decode(M::Obj(rrrah_decode::ObjDecodeError::Source(D::Memory(error)))) => Some(error),
            Self::Decode(M::Ply(rrrah_decode::PlyDecodeError::Source(D::Memory(error)))) => Some(error),
            Self::Decode(M::Off(rrrah_decode::OffDecodeError::Source(D::Memory(error)))) => Some(error),
            _ => None,
        };
        memory.is_some_and(retryable_memory_pressure)
    }
}

type ModelDisplayKey = (PathBuf, u64, u128, [u8; 32], usize);
struct ModelDisplayCache {
    swap: Option<rrrah_cache::ImageSwapCache<ModelDisplayKey, model_swap::ModelPayload>>,
    entries: rrrah_cache::LeaseCache<ModelDisplayKey, rrrah_decode::DecodedModel>,
    visible: Option<ModelDisplayKey>,
}
impl ModelDisplayCache {
    fn new(limits: rrrah_cache::CacheLimits) -> Self {
        Self {
            entries: rrrah_cache::LeaseCache::new(limits),
            visible: None,
            swap: None,
        }
    }
    fn spill_expired(&mut self) {
        let victims = self.entries.drain_expired();
        if let Some(swap) = &self.swap {
            for (key, model) in victims {
                if let Some(payload) = model_swap::ModelPayload::from_model(model) {
                    swap.enqueue(key, payload);
                }
            }
        }
    }
    fn release_lru_unpinned(&mut self) -> bool {
        if let Some(swap) = &self.swap {
            swap.discard_pending_writes();
        }
        self.entries.take_lru().is_some()
    }
    fn insert(&mut self, key: ModelDisplayKey, model: rrrah_decode::DecodedModel) -> bool {
        self.spill_expired();
        let weight = model.capacity_bytes();
        let (admitted, victims) = match self.entries.insert(key, model, weight) {
            Ok(victims) => (true, victims),
            Err(_) => (false, Vec::new()),
        };
        if let Some(swap) = &self.swap {
            for (key, model) in victims {
                if let Some(payload) = model_swap::ModelPayload::from_model(model) {
                    swap.enqueue(key, payload);
                }
            }
        }
        admitted
    }
    fn visible_lease_for(
        &mut self,
        model: &rrrah_decode::DecodedModel,
    ) -> Option<rrrah_cache::CacheLease<rrrah_decode::DecodedModel>> {
        let key = self.visible.clone()?;
        let lease = self.entries.get(&key)?;
        let shared = match (&*lease, model) {
            (rrrah_decode::DecodedModel::Stl(a), rrrah_decode::DecodedModel::Stl(b)) => a.ptr_eq(b),
            (rrrah_decode::DecodedModel::Off(a), rrrah_decode::DecodedModel::Off(b)) => a.ptr_eq(b),
            (rrrah_decode::DecodedModel::Obj(a), rrrah_decode::DecodedModel::Obj(b)) => a.ptr_eq(b),
            (rrrah_decode::DecodedModel::Ply(a), rrrah_decode::DecodedModel::Ply(b)) => a.ptr_eq(b),
            _ => false,
        };
        shared.then_some(lease)
    }
    fn load(
        &mut self,
        request: &DecodeRequest,
        visible: bool,
        mut decode: impl FnMut() -> Result<rrrah_decode::DecodedModel, ModelLoadError>,
    ) -> Result<rrrah_decode::DecodedModel, String> {
        if request
            .cancellation
            .as_ref()
            .is_some_and(|token| token.is_cancelled())
        {
            return Err("model load cancelled".into());
        }
        let key = SourceFingerprint::from_path(&request.path).ok().map(|source| {
            (
                request.path.clone(),
                source.file_size,
                source.modified_ns,
                source.sampled_blake3,
                request.image_index,
            )
        });
        self.spill_expired();
        if let Some(key) = &key {
            if let Some(model) = self.entries.get_cloned(key) {
                if visible {
                    if let Some(previous) = self.visible.take() {
                        self.entries.unpin(&previous);
                    }
                    self.entries.pin(key);
                    self.visible = Some(key.clone());
                }
                return Ok(model);
            }
        }
        let mut restored = None;
        if let (Some(key), Some(swap)) = (&key, &self.swap) {
            loop {
                match swap.try_get(key, || {
                    request
                        .cancellation
                        .as_ref()
                        .is_some_and(|token| token.is_cancelled())
                }) {
                    Ok(value) => {
                        restored = value.map(|payload| payload.0);
                        break;
                    }
                    Err(error) if visible && retryable_memory_pressure(&error) => {
                        swap.discard_pending_writes();
                        if self.entries.take_lru().is_none() {
                            break;
                        }
                    }
                    Err(_) => break,
                }
            }
        }
        let model = if let Some(model) = restored {
            model
        } else {
            loop {
                match decode() {
                    Ok(model) => break model,
                    Err(error)
                        if visible && error.retryable_memory_pressure() && self.release_lru_unpinned() =>
                    {
                        continue;
                    }
                    Err(error) => return Err(error.to_string()),
                }
            }
        };
        if request
            .cancellation
            .as_ref()
            .is_some_and(|token| token.is_cancelled())
        {
            return Err("model load cancelled".into());
        }
        if let Some(key) = key {
            if !visible {
                self.insert(key, model.clone());
                return Ok(model);
            }
            let previous = self.visible.take();
            if let Some(previous) = &previous {
                self.entries.unpin(previous);
            }
            if self.insert(key.clone(), model.clone()) {
                self.entries.pin(&key);
                self.visible = Some(key);
            } else {
                if let Some(previous) = &previous {
                    self.entries.pin(previous);
                }
                self.visible = previous;
            }
        }
        Ok(model)
    }
}

fn model_cache_with_swap(
    parent: Option<&Path>,
    no_cache: bool,
    limits: rrrah_cache::CacheLimits,
    config: Option<rrrah_cache::ImageSwapConfig>,
    managed: Option<&rrrah_cache::MemoryBudget>,
) -> ModelDisplayCache {
    let mut cache = ModelDisplayCache::new(if no_cache {
        rrrah_cache::CacheLimits::bytes(0)
    } else {
        limits
    });
    if !no_cache {
        if let (Some(parent), Some(config)) = (parent, config) {
            let restore = managed.map_or_else(
                || rrrah_cache::MemoryBudget::new(config.restore_bytes),
                |root| root.child(config.restore_bytes),
            );
            match std::fs::create_dir_all(parent).and_then(|()| {
                rrrah_cache::ImageSwapCache::new_with_budgets(
                    parent,
                    config,
                    retained_queue_budget(config.queue_bytes),
                    restore,
                )
            }) {
                Ok(swap) => cache.swap = Some(swap),
                Err(error) => log::warn!("model swap unavailable: {error}"),
            }
        }
    }
    cache
}

/// Advisory minimum for current whole-file decoders. Metadata errors are left
/// to the decoder, and successful checks do not replace atomic admission.
fn prefetch_source_fits(request: &DecodeRequest) -> bool {
    let Some(budget) = &request.memory_budget else {
        return true;
    };
    std::fs::metadata(&request.path).map_or(true, |metadata| metadata.len() <= budget.available_bytes())
}

fn preload_model(
    request: &DecodeRequest,
    cache: &mut ModelDisplayCache,
    gate: &Arc<DecodeGate>,
) -> Result<rrrah_decode::DecodedModel, String> {
    cache.load(request, false, || {
        let cancelled = || {
            request
                .cancellation
                .as_ref()
                .is_some_and(|token| token.is_cancelled())
        };
        let Some(permit) = gate.acquire_prefetch(cancelled) else {
            return Err("model prefetch cancelled".into());
        };
        if !prefetch_source_fits(request) {
            return Err("model prefetch skipped: insufficient source memory headroom".into());
        }
        let model = rrrah_decode::decode_model(request).map_err(ModelLoadError::Decode);
        drop(permit);
        model
    })
}

fn preload_raw(
    request: &DecodeRequest,
    disk: &DiskMosaicCache,
    ram: &mut MosaicRamCache,
    writer: Option<&CacheWriter>,
    generation: u64,
    gate: &Arc<DecodeGate>,
) -> Result<bool, String> {
    let cancelled = || {
        request
            .cancellation
            .as_ref()
            .is_some_and(|token| token.is_cancelled())
    };
    if cancelled() {
        return Err("RAW prefetch cancelled".into());
    }
    let recipe = NativeRawDecoder
        .mosaic_recipe(request)
        .map_err(|e| e.to_string())?;
    let fingerprint = SourceFingerprint::from_path(&request.path).map_err(|e| e.to_string())?;
    let key = CacheKey::for_mosaic_recipe(&fingerprint, 0, recipe);
    if ram.get_background_with_cancel(&key, cancelled).is_some() {
        return Ok(true);
    }
    let Some(permit) = gate.acquire_prefetch(cancelled) else {
        return Err("RAW prefetch cancelled".into());
    };
    // Recheck persistent cache after waiting for another foreground/prefetch decode.
    let budget = ram
        .swap()
        .map(|swap| swap.restore_budget())
        .or(request.memory_budget.as_ref());
    let cached = disk.load_with_cancel(key, budget, cancelled);
    // Cancellation is terminal for speculative work, not a cache miss. Avoid
    // entering native decode after a generation changes during disk restore.
    if cancelled() {
        return Err("RAW prefetch cancelled".into());
    }
    let (mosaic, decoded) = match cached {
        Ok(Some(hit)) => (hit.mosaic, false),
        Err(rrrah_cache::CacheError::Cancelled) => return Err("RAW prefetch cancelled".into()),
        Err(rrrah_cache::CacheError::Memory(_)) => return Ok(false),
        _ => {
            if !prefetch_source_fits(request) {
                return Ok(false);
            }
            (
                NativeRawDecoder
                    .decode(request)
                    .map_err(|e| e.to_string())?
                    .mosaic,
                true,
            )
        }
    };
    drop(permit);
    if cancelled() {
        return Err("RAW prefetch cancelled".into());
    }
    let admitted = ram.insert(key, mosaic.clone());
    if decoded {
        if let Some(writer) = writer {
            writer.submit(generation, key, mosaic);
        }
    }
    Ok(admitted)
}

fn preload_raster(
    request: &DecodeRequest,
    window: Option<rrrah_decode::ScalarWindow>,
    cache: &mut RasterDisplayCache,
    gate: &Arc<DecodeGate>,
) -> Result<(rrrah_core::DecodedRaster, bool), String> {
    load_cached_raster_mode(request, window, cache, false, || {
        let cancelled = || {
            request
                .cancellation
                .as_ref()
                .is_some_and(|token| token.is_cancelled())
        };
        let Some(permit) = gate.acquire_prefetch(cancelled) else {
            return Err("raster prefetch cancelled".into());
        };
        // Whole-file decoding needs at least the source allocation. Check after
        // waiting and after cache probes; a ready RAM frame needs no new buffer.
        if !prefetch_source_fits(request) {
            return Err(RasterLoadError::Message(
                "raster prefetch skipped: insufficient source memory headroom".into(),
            ));
        }
        let result = load_raster_for_display_typed(request, window);
        drop(permit);
        result
    })
}

type RasterDisplayKey = (
    PathBuf,
    u64,
    u128,
    [u8; 32],
    usize,
    Option<(u32, u32)>,
    bool,
    bool,
    bool,
    Option<rrrah_decode::RlaAlphaMode>,
);
type RasterDisplayCache = rrrah_cache::RasterRamCache<RasterDisplayKey>;
fn raster_cache_with_swap(
    parent: Option<&std::path::Path>,
    no_cache: bool,
    limits: rrrah_cache::CacheLimits,
    config: Option<rrrah_cache::ImageSwapConfig>,
    managed: Option<&rrrah_cache::MemoryBudget>,
) -> RasterDisplayCache {
    let mut cache = RasterDisplayCache::new(if no_cache {
        rrrah_cache::CacheLimits::bytes(0)
    } else {
        limits
    });
    if !no_cache {
        if let (Some(parent), Some(config)) = (parent, config) {
            let restore = managed.map_or_else(
                || rrrah_cache::MemoryBudget::new(config.restore_bytes),
                |root| root.child(config.restore_bytes),
            );
            let swap = std::fs::create_dir_all(parent).and_then(|()| {
                rrrah_cache::RasterSwapCache::new_with_budgets(
                    parent,
                    config,
                    retained_queue_budget(config.queue_bytes),
                    restore,
                )
            });
            match swap {
                Ok(swap) => cache.enable_swap(swap),
                Err(error) => log::warn!("raster swap unavailable: {error}"),
            }
        }
    }
    cache
}

#[cfg(test)]
fn load_cached_raster_for_display(
    request: &DecodeRequest,
    window: Option<rrrah_decode::ScalarWindow>,
    cache: &mut RasterDisplayCache,
) -> Result<(rrrah_core::DecodedRaster, bool), String> {
    load_cached_raster_with(request, window, cache, || {
        load_raster_for_display_typed(request, window)
    })
}

fn load_cached_raster_with(
    request: &DecodeRequest,
    window: Option<rrrah_decode::ScalarWindow>,
    cache: &mut RasterDisplayCache,
    load: impl FnMut() -> Result<(rrrah_core::DecodedRaster, bool), RasterLoadError>,
) -> Result<(rrrah_core::DecodedRaster, bool), String> {
    load_cached_raster_mode(request, window, cache, true, load)
}
fn load_cached_raster_mode(
    request: &DecodeRequest,
    window: Option<rrrah_decode::ScalarWindow>,
    cache: &mut RasterDisplayCache,
    visible: bool,
    mut load: impl FnMut() -> Result<(rrrah_core::DecodedRaster, bool), RasterLoadError>,
) -> Result<(rrrah_core::DecodedRaster, bool), String> {
    if request
        .cancellation
        .as_ref()
        .is_some_and(|token| token.is_cancelled())
    {
        return Err("raster load cancelled".into());
    }
    let fingerprint = SourceFingerprint::from_path(&request.path).ok();
    let key = fingerprint.map(|source| {
        (
            request.path.clone(),
            source.file_size,
            source.modified_ns,
            source.sampled_blake3,
            request.image_index,
            window.map(|window| (window.minimum().to_bits(), window.maximum().to_bits())),
            false,
            request.assume_untagged_srgb,
            request.assume_untagged_linear_srgb,
            request.rla_alpha_mode,
        )
    });
    if let Some(key) = &key {
        for assumed in [false, true] {
            let mut probe = key.clone();
            probe.6 = assumed;
            let cancelled = || {
                request
                    .cancellation
                    .as_ref()
                    .is_some_and(|token| token.is_cancelled())
            };
            let hit = if visible {
                cache.get_with_cancel(&probe, cancelled)
            } else {
                cache.get_background_with_cancel(&probe, cancelled)
            };
            if cancelled() {
                return Err("raster load cancelled".into());
            }
            if let Some(raster) = hit {
                if visible {
                    cache.mark_visible(probe);
                }
                return Ok((raster, assumed));
            }
        }
    }
    let (raster, assumed) = loop {
        match load() {
            Ok(result) => break result,
            Err(error) if visible && error.retryable_memory_pressure() && cache.release_lru_unpinned() => {
                continue;
            }
            Err(error) => return Err(error.to_string()),
        }
    };
    if request
        .cancellation
        .as_ref()
        .is_some_and(|token| token.is_cancelled())
    {
        return Err("raster load cancelled".into());
    }
    if let Some(mut key) = key {
        key.6 = assumed;
        if visible {
            cache.insert_visible(key, raster.clone());
        } else {
            cache.insert(key, raster.clone());
        }
    }
    Ok((raster, assumed))
}

#[cfg(test)]
fn load_raster_for_display(
    request: &DecodeRequest,
    window: Option<rrrah_decode::ScalarWindow>,
) -> Result<(rrrah_core::DecodedRaster, bool), String> {
    load_raster_for_display_typed(request, window).map_err(|error| error.to_string())
}

#[derive(Debug)]
enum RasterLoadError {
    Decode(rrrah_decode::RasterDecodeError),
    Color(rrrah_decode::RasterColorError),
    Message(String),
}
impl From<&str> for RasterLoadError {
    fn from(value: &str) -> Self {
        Self::Message(value.into())
    }
}
impl std::fmt::Display for RasterLoadError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Decode(error) => error.fmt(formatter),
            Self::Color(error) => error.fmt(formatter),
            Self::Message(message) => formatter.write_str(message),
        }
    }
}
impl RasterLoadError {
    fn retryable_memory_pressure(&self) -> bool {
        let memory = match self {
            Self::Decode(rrrah_decode::RasterDecodeError::X3f(rrrah_decode::X3fError::Memory(error))) => {
                Some(error)
            }
            Self::Decode(rrrah_decode::RasterDecodeError::Source(rrrah_decode::DecodeError::Memory(
                error,
            ))) => Some(error),
            Self::Color(rrrah_decode::RasterColorError::Frame(rrrah_core::RasterError::Memory(error))) => {
                Some(error)
            }
            _ => None,
        };
        memory.is_some_and(retryable_memory_pressure)
    }
}

fn load_raster_for_display_typed(
    request: &DecodeRequest,
    scalar_window: Option<rrrah_decode::ScalarWindow>,
) -> Result<(rrrah_core::DecodedRaster, bool), RasterLoadError> {
    let decoded = if let Some(window) = scalar_window {
        rrrah_decode::decode_raster_with_window(request, window)
    } else {
        rrrah_decode::decode_raster(request)
    };
    let raster = decoded.map_err(RasterLoadError::Decode)?;
    prepare_loaded_raster(request, raster)
}

fn prepare_loaded_raster(
    request: &DecodeRequest,
    raster: rrrah_core::DecodedRaster,
) -> Result<(rrrah_core::DecodedRaster, bool), RasterLoadError> {
    let assumed = raster.color_space() == &rrrah_core::RasterColorSpace::AssumedSrgb;
    rrrah_decode::prepare_raster_for_display_with_budget_and_cancel(
        &raster,
        request.memory_budget.as_ref(),
        || {
            request
                .cancellation
                .as_ref()
                .is_some_and(|token| token.is_cancelled())
        },
    )
    .map(|prepared| (prepared, assumed))
    .map_err(RasterLoadError::Color)
}

fn retryable_memory_pressure(error: &rrrah_core::BufferError) -> bool {
    matches!(error, rrrah_core::BufferError::Capacity { requested, limit, .. } if requested <= limit)
}

fn image_source_kind_with_pressure(
    request: &DecodeRequest,
    ram: &mut Option<&mut MosaicRamCache>,
    mut rasters: Option<&mut RasterDisplayCache>,
    mut models: Option<&mut ModelDisplayCache>,
) -> Result<rrrah_decode::ImageSourceKind, rrrah_decode::RasterDecodeError> {
    loop {
        let result = rrrah_decode::image_source_kind(request);
        if matches!(
            &result,
            Err(rrrah_decode::RasterDecodeError::Source(
                rrrah_decode::DecodeError::Memory(error)
            )) if retryable_memory_pressure(error)
        ) && (rasters.as_mut().is_some_and(|cache| cache.release_lru_unpinned())
            || models.as_mut().is_some_and(|cache| cache.release_lru_unpinned())
            || ram.as_mut().is_some_and(|ram| ram.release_lru_unpinned()))
        {
            continue;
        }
        return result;
    }
}

fn execute_load(
    request: LoadRequest,
    untagged_color: RasterInterpretation,
    cache: Option<&DiskMosaicCache>,
    mut ram_cache: Option<&mut MosaicRamCache>,
    raster_cache: &mut RasterDisplayCache,
    model_cache: &mut ModelDisplayCache,
    cache_writer: Option<&CacheWriter>,
    managed_budget: Option<&rrrah_cache::MemoryBudget>,
    load_generation: Arc<AtomicU64>,
    sender: &Sender<LoadEvent>,
    proxy: &EventLoopProxy<WakeEvent>,
    telemetry: &CacheTelemetry,
    load_costs: &mut raw_load_policy::RawLoadCosts,
) {
    let LoadRequest {
        mut decode_first,
        automatic_policy,
        path,
        image_index,
        scalar_window,
        generation,
        requested_at,
        foreground,
        development,
    } = request;
    let token = GenerationToken::new(load_generation, generation);
    log::debug!(
        "foreground generation={generation} RAW policy={}",
        if decode_first {
            "decode-first"
        } else {
            "cache-first"
        }
    );
    if token.is_cancelled() {
        return;
    }
    let mut decode_request = DecodeRequest::new(&path);
    untagged_color.apply(&mut decode_request);
    decode_request.cancellation = Some(token.clone());
    decode_request.memory_budget = managed_budget.cloned();
    decode_request.image_index = image_index;
    if rrrah_decode::is_supported_model_path(&path) {
        if scalar_window.is_some() {
            publish_load_failure(
                generation,
                "scalar window does not apply to a model".into(),
                &token,
                sender,
                proxy,
            );
            return;
        }
        let result = model_cache.load(&decode_request, true, || {
            let Some(permit) = foreground.acquire_decode(|| token.is_cancelled()) else {
                return Err("model load cancelled".into());
            };
            let result = rrrah_decode::decode_model(&decode_request).map_err(ModelLoadError::Decode);
            drop(permit);
            result
        });
        if token.is_cancelled() {
            return;
        }
        match result {
            Ok(mesh) => publish_load_event(
                LoadEvent::ModelReady {
                    generation,
                    lease: model_cache.visible_lease_for(&mesh),
                    mesh,
                    elapsed: requested_at.elapsed(),
                },
                sender,
                proxy,
            ),
            Err(error) => publish_load_failure(generation, error.to_string(), &token, sender, proxy),
        }
        return;
    }

    match image_source_kind_with_pressure(
        &decode_request,
        &mut ram_cache,
        Some(raster_cache),
        Some(model_cache),
    ) {
        Ok(rrrah_decode::ImageSourceKind::Raster) => {
            let result = load_cached_raster_with(&decode_request, scalar_window, raster_cache, || {
                let Some(permit) = foreground.acquire_decode(|| token.is_cancelled()) else {
                    return Err("raster load cancelled".into());
                };
                let result = load_raster_for_display_typed(&decode_request, scalar_window);
                drop(permit);
                result
            });
            if token.is_cancelled() {
                return;
            }
            match result {
                Ok((raster, assumed_srgb)) => publish_load_event(
                    LoadEvent::RasterReady {
                        generation,
                        image_index,
                        scalar_window,
                        lease: raster_cache.get_visible_lease_for(&raster),
                        raster,
                        assumed_srgb,
                        elapsed: requested_at.elapsed(),
                        development: None,
                    },
                    sender,
                    proxy,
                ),
                Err(error) => publish_load_failure(generation, error, &token, sender, proxy),
            }
            return;
        }
        Ok(rrrah_decode::ImageSourceKind::Sensor) => {}
        Err(error) => {
            publish_load_failure(generation, error.to_string(), &token, sender, proxy);
            return;
        }
    }
    if scalar_window.is_some() {
        publish_load_failure(
            generation,
            "scalar window requires a scientific array".into(),
            &token,
            sender,
            proxy,
        );
        return;
    }
    if image_index != 0 {
        publish_load_failure(
            generation,
            format!("sensor image index {image_index} is unsupported"),
            &token,
            sender,
            proxy,
        );
        return;
    }
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
        let key = CacheKey::for_mosaic_recipe(fingerprint, 0, recipe);
        cache_key = Some(key);
        // A resident frame requires neither restoration nor decode. Keep those
        // hits out of the alternative-route probe cadence.
        let ram_lookup_started = Instant::now();
        let resident = ram_cache.as_mut().and_then(|ram| ram.get_resident(&key));
        if automatic_policy && resident.is_none() {
            decode_first = load_costs.decode_first(key);
            log::debug!(
                "foreground generation={generation} automatic RAW policy={}",
                if decode_first {
                    "decode-first"
                } else {
                    "cache-first"
                }
            );
        }
        if let Some(ram_cache) = ram_cache.as_mut() {
            let restoring = resident.is_none() && !decode_first;
            let candidate = if resident.is_some() {
                resident
            } else if decode_first {
                None
            } else {
                ram_cache.get_with_cancel(&key, || token.is_cancelled())
            };
            if let Some(mosaic) = candidate {
                log::debug!("foreground generation={generation} RAW resident/restore hit");
                let ram_lookup = ram_lookup_started.elapsed();
                if automatic_policy && restoring && !token.is_cancelled() {
                    load_costs.observe_restore(key, ram_lookup);
                }
                if token.is_cancelled() {
                    return;
                }
                // The RAM hit becomes the visible frame: pin it before
                // publishing so no later admission can evict it.
                ram_cache.mark_visible(&key);
                frontend.cache_lookup = Some(ram_lookup);
                telemetry.record_hit(
                    generation,
                    ram_lookup,
                    u64::try_from(mosaic.byte_len()).unwrap_or(u64::MAX),
                );
                publish_raw_event(
                    LoadEvent::Ready {
                        generation,
                        mosaic,
                        lease: ram_cache.get_lease(&key),
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
                    &decode_request,
                    development.as_ref(),
                );
                return;
            }
            frontend.cache_lookup = Some(ram_lookup_started.elapsed());
            if token.is_cancelled() {
                return;
            }
        }
        if !decode_first {
            let cache_lookup_started = Instant::now();
            let cache_result = loop {
                let budget = ram_cache
                    .as_ref()
                    .and_then(|ram| ram.swap())
                    .map(|swap| swap.restore_budget())
                    .or(managed_budget);
                let result = cache.load_with_cancel(key, budget, || token.is_cancelled());
                if matches!(&result, Err(rrrah_cache::CacheError::Memory(error)) if retryable_memory_pressure(error))
                    && ram_cache.as_mut().is_some_and(|ram| ram.release_lru_unpinned())
                {
                    continue;
                }
                break result;
            };
            frontend.cache_lookup = Some(cache_lookup_started.elapsed());
            match cache_result {
                Ok(Some(hit)) => {
                    if token.is_cancelled() {
                        return;
                    }
                    if automatic_policy {
                        load_costs.observe_restore(key, cache_lookup_started.elapsed());
                    }
                    if let Some(ram_cache) = ram_cache.as_mut() {
                        ram_cache.insert_visible(key, hit.mosaic.clone());
                    }
                    telemetry.record_hit(
                        generation,
                        hit.elapsed,
                        u64::try_from(hit.mosaic.byte_len()).unwrap_or(u64::MAX),
                    );
                    publish_raw_event(
                        LoadEvent::Ready {
                            generation,
                            mosaic: hit.mosaic,
                            lease: ram_cache.as_mut().and_then(|ram| ram.get_lease(&key)),
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
                        &decode_request,
                        development.as_ref(),
                    );
                    return;
                }
                Ok(None) => telemetry.record_miss(generation),
                Err(rrrah_cache::CacheError::Cancelled) => return,
                Err(rrrah_cache::CacheError::Memory(_)) => telemetry.record_miss(generation),
                Err(error) => {
                    cache_route = CacheRoute::ErrorFallback;
                    telemetry.record_read_error(generation);
                    log::warn!("ignoring corrupt/unreadable cache: {error}");
                }
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
    log::debug!("foreground generation={generation} native decode selected");
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
    let decode_started = Instant::now();
    let decode_result = loop {
        let result = NativeRawDecoder.decode(&decode_request);
        if matches!(&result, Err(rrrah_decode::DecodeError::Memory(error)) if retryable_memory_pressure(error))
            && ram_cache.as_mut().is_some_and(|ram| ram.release_lru_unpinned())
        {
            if token.is_cancelled() {
                return;
            }
            continue;
        }
        break result;
    };
    let output = match decode_result {
        Ok(output) => output,
        Err(error) => {
            // Faster decode can need more RAM than restoration. Automatic policy
            // must retain a cache recovery path after a transient admission refusal.
            if automatic_policy
                && matches!(&error, rrrah_decode::DecodeError::Memory(_))
                && !token.is_cancelled()
            {
                if let (Some(cache), Some(key)) = (cache, cache_key) {
                    let started = Instant::now();
                    load_costs.defer_decode(key);
                    let restored = raw_load_policy::restore_after_pressure(
                        cache,
                        ram_cache.as_deref_mut(),
                        key,
                        managed_budget,
                        &|| token.is_cancelled(),
                    );
                    if let Some(mosaic) = restored {
                        if token.is_cancelled() {
                            return;
                        }
                        drop(decode_permit);
                        let elapsed = started.elapsed();
                        load_costs.observe_restore(key, elapsed);
                        frontend.cache_lookup = Some(elapsed);
                        if let Some(ram) = ram_cache.as_mut() {
                            ram.insert_visible(key, mosaic.clone());
                        }
                        telemetry.record_hit(
                            generation,
                            elapsed,
                            u64::try_from(mosaic.byte_len()).unwrap_or(u64::MAX),
                        );
                        publish_raw_event(
                            LoadEvent::Ready {
                                generation,
                                mosaic,
                                lease: ram_cache.as_mut().and_then(|ram| ram.get_lease(&key)),
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
                            &decode_request,
                            development.as_ref(),
                        );
                        return;
                    }
                }
            }
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
    if automatic_policy {
        if let Some(key) = cache_key {
            load_costs.observe_decode(key, decode_started.elapsed());
        }
    }
    let decode_timings = output.timings;
    let mosaic = output.mosaic;
    if let (Some(ram_cache), Some(key)) = (ram_cache.as_mut(), cache_key) {
        ram_cache.insert_visible(key, mosaic.clone());
    }
    let write_back = cache_key.map(|key| (key, mosaic.clone()));
    let elapsed = requested_at.elapsed();
    // Publish the usable frame before enqueueing persistence. Even an already
    // active atomic store can never delay this Ready event or the next decode.
    publish_raw_event(
        LoadEvent::Ready {
            generation,
            mosaic,
            lease: ram_cache
                .as_mut()
                .and_then(|ram| cache_key.and_then(|key| ram.get_lease(&key))),
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
        &decode_request,
        development.as_ref(),
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

/// Cover-thumbnail decode for the folder filmstrip. The job waits on the
/// shared decode gate's prefetch admission, so thumbnails never compete with
/// the foreground frame and take turns with RAW neighbor prefetch.
#[allow(clippy::cast_sign_loss)] // Thumbnail transfer clamps linear RGB before byte quantization.
#[cfg(test)]
fn thumbnail_loader(
    decode_gate: Arc<DecodeGate>,
    managed_budget: Option<rrrah_core::MemoryBudget>,
) -> impl Fn(gallery::ThumbnailJob) -> Option<gallery::ThumbnailReady> + Send + Sync + 'static {
    let loader = thumbnail_loader_with_cancel(decode_gate, managed_budget, UntaggedColor::Strict.into());
    move |job| loader(job, GenerationToken::new(Arc::new(AtomicU64::new(0)), 0))
}

fn thumbnail_loader_with_cancel(
    decode_gate: Arc<DecodeGate>,
    managed_budget: Option<rrrah_core::MemoryBudget>,
    untagged_color: RasterInterpretation,
) -> impl Fn(gallery::ThumbnailJob, GenerationToken) -> Option<gallery::ThumbnailReady> + Send + Sync + 'static
{
    move |job: gallery::ThumbnailJob, viewport: GenerationToken| {
        if viewport.is_cancelled() || rrrah_decode::is_supported_model_path(&job.source) {
            return None;
        }
        let permit = decode_gate.acquire_prefetch(|| viewport.is_cancelled())?;
        let token = permit.cancellation_token()?.combine(&viewport);
        if token.is_cancelled() {
            return None;
        }
        let source_stamp = gallery::SourceStamp::read(&job.source);
        if !source_stamp.is_readable() {
            return None;
        }
        let mut request = DecodeRequest::new(&job.source);
        untagged_color.apply(&mut request);
        request.memory_budget = managed_budget.clone();
        request.cancellation = Some(token.clone());
        let result = rrrah_decode::decode_image(&request).ok()?;
        let (width, height, pixels) = match result {
            rrrah_decode::DecodedImage::Sensor(output) => {
                let (width, height) = output.mosaic.metadata.thumbnail_dimensions(job.edge);
                let pixels = if let Some(budget) = &managed_budget {
                    output
                        .mosaic
                        .thumbnail_rgba8_managed_with_cancel(job.edge, budget, &|| token.is_cancelled())
                        .ok()?
                } else {
                    Arc::new(
                        output
                            .mosaic
                            .thumbnail_rgba8_with_cancel(job.edge, &|| token.is_cancelled()),
                    )
                    .into()
                };
                (width, height, pixels)
            }
            rrrah_decode::DecodedImage::Raster(raster) => {
                let raster = rrrah_decode::prepare_raster_for_display_with_budget_and_cancel(
                    &raster,
                    managed_budget.as_ref(),
                    || token.is_cancelled(),
                )
                .ok()?;
                let (width, height) = thumbnail_dimensions(raster.width(), raster.height(), job.edge);
                let rrrah_core::RasterPixels::Rgba32Float(source) = raster.pixels() else {
                    return None;
                };
                let bytes = u64::from(width) * u64::from(height) * 4;
                let reservation = managed_budget
                    .as_ref()
                    .map(|b| b.try_reserve(bytes))
                    .transpose()
                    .ok()?;
                let mut pixels = Vec::new();
                pixels.try_reserve_exact(usize::try_from(bytes).ok()?).ok()?;
                for y in 0..height {
                    if token.is_cancelled() {
                        return None;
                    }
                    for x in 0..width {
                        let sx = (u64::from(x) * u64::from(raster.width()) / u64::from(width)) as usize;
                        let sy = (u64::from(y) * u64::from(raster.height()) / u64::from(height)) as usize;
                        let start = (sy * raster.width() as usize + sx) * 4;
                        let alpha = source[start + 3];
                        for channel in 0..3 {
                            let linear =
                                (source[start + channel] * alpha + 0.018 * (1.0 - alpha)).clamp(0.0, 1.0);
                            let encoded = if linear <= 0.003_130_8 {
                                linear * 12.92
                            } else {
                                1.055 * linear.powf(1.0 / 2.4) - 0.055
                            };
                            pixels.push((encoded * 255.0).round() as u8);
                        }
                        pixels.push(255);
                    }
                }
                let pixels = if let Some(reservation) = reservation {
                    reservation.try_adopt(pixels).ok()?.into()
                } else {
                    Arc::new(pixels).into()
                };
                (width, height, pixels)
            }
        };
        if token.is_cancelled() || gallery::SourceStamp::read(&job.source) != source_stamp {
            return None;
        }
        drop(permit);
        let expected = width as usize * height as usize * 4;
        (pixels.len() == expected).then_some(gallery::ThumbnailReady {
            source_stamp,
            index: job.index,
            width,
            height,
            pixels,
        })
    }
}

/// Output dimensions of `DecodedMosaic::thumbnail_rgba8` for the same input.
fn thumbnail_dimensions(width: u32, height: u32, max_dimension: u32) -> (u32, u32) {
    let max_dimension = max_dimension.max(1);
    let long_edge = width.max(height);
    if long_edge <= max_dimension {
        return (width, height);
    }
    let scaled = |dimension: u32| {
        u32::try_from(
            u64::from(dimension)
                .saturating_mul(u64::from(max_dimension))
                .div_ceil(u64::from(long_edge)),
        )
        .unwrap_or(max_dimension)
        .max(1)
    };
    (scaled(width), scaled(height))
}

struct DeferredThumbnail {
    ready: gallery::ThumbnailReady,
    folder: PathBuf,
    cover: PathBuf,
    retry_at: Instant,
    expires_at: Instant,
}

impl DeferredThumbnail {
    fn is_current(&self, tiles: &[gallery::FolderTile]) -> bool {
        tiles.get(self.ready.index).is_some_and(|tile| {
            tile.folder == self.folder
                && tile.cover == self.cover
                && gallery::SourceStamp::read(&tile.cover) == self.ready.source_stamp
        })
    }
    fn expired(&self, now: Instant) -> bool {
        now >= self.expires_at
    }
    fn due(&self, now: Instant) -> bool {
        now >= self.retry_at
    }
}

/// Tracks callbacks needing event-loop polling even when the surface is hidden.
#[derive(Default)]
struct UploadCompletions {
    pending: Arc<AtomicU64>,
}
impl UploadCompletions {
    fn track_submitted(&self, queue: &wgpu::Queue) {
        let pending = self.pending.clone();
        pending.fetch_add(1, Ordering::AcqRel);
        queue.on_submitted_work_done(move || {
            pending.fetch_sub(1, Ordering::AcqRel);
        });
    }
    fn poll(&self, device: &wgpu::Device) -> Result<bool, wgpu::PollError> {
        if self.pending.load(Ordering::Acquire) == 0 {
            return Ok(false);
        }
        device.poll(wgpu::PollType::Poll)?;
        Ok(self.pending.load(Ordering::Acquire) != 0)
    }
}

/// Flush pending texture writes before attaching a completion callback. A
/// callback on the previous submission would release protection too early.
fn submit_upload_lease<V: Send + Sync + 'static>(
    queue: &wgpu::Queue,
    completions: &UploadCompletions,
    lease: rrrah_cache::CacheLease<V>,
) -> wgpu::SubmissionIndex {
    let submission = queue.submit([]);
    let pending = completions.pending.clone();
    pending.fetch_add(1, Ordering::AcqRel);
    queue.on_submitted_work_done(move || {
        drop(lease);
        let remaining = pending.fetch_sub(1, Ordering::AcqRel) - 1;
        log::debug!("GPU upload consumer lease released; pending={remaining}");
    });
    submission
}

struct App {
    path: PathBuf,
    receiver: Receiver<LoadEvent>,
    foreground_loader: ForegroundLoader,
    gallery: Vec<PathBuf>,
    gallery_index: Option<usize>,
    image_index: usize,
    image_count: usize,
    scalar_window: Option<rrrah_decode::ScalarWindow>,
    last_nav_direction: NavDirection,
    raw_prefetcher: gallery::RawPrefetcher,
    strip: FolderStrip,
    strip_thumbs: ThumbCache,
    thumbnail_prefetcher: gallery::Prefetcher,
    deferred_thumbnail: Option<DeferredThumbnail>,
    cache_telemetry: Arc<CacheTelemetry>,
    window: Option<Arc<Window>>,
    gpu: Option<GpuState>,
    raster_gpu_budget: Option<rrrah_core::MemoryBudget>,
    model_gpu_budget: Option<rrrah_core::MemoryBudget>,
    raw_gpu_budget: Option<rrrah_core::MemoryBudget>,
    filmstrip_gpu_budget: Option<rrrah_core::MemoryBudget>,
    gpu_backend: rrrah_gpu::GpuBackend,
    gpu_vendor: rrrah_gpu::GpuVendor,
    upload_memory_budget: Option<rrrah_core::MemoryBudget>,
    telemetry_window: Option<Arc<Window>>,
    telemetry: Option<TelemetryState>,
    view: ViewParameters,
    dragging: bool,
    last_cursor: Option<PhysicalPosition<f64>>,
    modifiers: ModifiersState,
    telemetry_dragging: bool,
    telemetry_last_cursor: Option<PhysicalPosition<f64>>,
    status: String,
    pipeline: PipelineSnapshot,
    pending_first_present: Option<PendingFirstPresent>,
}

impl App {
    fn submit_raw_neighbours(&self) {
        if let Some(index) = self.gallery_index {
            self.raw_prefetcher
                .finish_foreground_and_submit(&self.gallery, index, self.last_nav_direction);
        } else {
            self.raw_prefetcher
                .finish_foreground_and_submit(&[], 0, NavDirection::None);
        }
    }

    fn new(
        path: PathBuf,
        cache_root: Option<PathBuf>,
        no_cache: bool,
        receiver: Receiver<LoadEvent>,
        foreground_loader: ForegroundLoader,
        decode_gate: Arc<DecodeGate>,
        cache_telemetry: Arc<CacheTelemetry>,
        prefetch_window: gallery::PrefetchWindow,
        disk_limits: rrrah_cache::CacheLimits,
        managed_budget: Option<rrrah_cache::MemoryBudget>,
        raster_gpu_budget: Option<rrrah_core::MemoryBudget>,
        model_gpu_budget: Option<rrrah_core::MemoryBudget>,
        raw_gpu_budget: Option<rrrah_core::MemoryBudget>,
        filmstrip_gpu_budget: Option<rrrah_core::MemoryBudget>,
        gpu_backend: rrrah_gpu::GpuBackend,
        gpu_vendor: rrrah_gpu::GpuVendor,
        untagged_color: RasterInterpretation,
    ) -> Self {
        let thumbnail_prefetcher = gallery::Prefetcher::new_with_cancel(
            32,
            thumbnail_loader_with_cancel(Arc::clone(&decode_gate), managed_budget.clone(), untagged_color),
        );
        let raw_prefetcher = gallery::RawPrefetcher::with_budget(
            cache_root,
            no_cache,
            decode_gate,
            Arc::clone(&cache_telemetry),
            prefetch_window,
            disk_limits,
            managed_budget.clone(),
        );
        let generation = foreground_loader.current_generation();
        let has_initial_path = !path.as_os_str().is_empty();
        let raw_kind = RawKind::from_path(&path);
        if has_initial_path {
            raw_prefetcher.begin_foreground();
        }
        let mut app = Self {
            path,
            receiver,
            foreground_loader,
            gallery: Vec::new(),
            gallery_index: None,
            image_index: 0,
            image_count: 1,
            scalar_window: None,
            last_nav_direction: NavDirection::None,
            raw_prefetcher,
            strip: FolderStrip::default(),
            strip_thumbs: ThumbCache::new(THUMB_CACHE_CAPACITY),
            thumbnail_prefetcher,
            deferred_thumbnail: None,
            cache_telemetry,
            window: None,
            gpu: None,
            raster_gpu_budget,
            model_gpu_budget,
            raw_gpu_budget,
            filmstrip_gpu_budget,
            gpu_backend,
            gpu_vendor,
            upload_memory_budget: managed_budget,
            telemetry_window: None,
            telemetry: None,
            view: ViewParameters::default(),
            dragging: false,
            last_cursor: None,
            modifiers: ModifiersState::empty(),
            telemetry_dragging: false,
            telemetry_last_cursor: None,
            status: "loading image…".into(),
            pipeline: if has_initial_path {
                PipelineSnapshot::waiting(generation, raw_kind)
            } else {
                PipelineSnapshot::idle(generation)
            },
            pending_first_present: None,
        };
        app.initialize_folder_context();
        app
    }

    fn drain_load_events(&mut self) {
        loop {
            let event = match self.receiver.try_recv() {
                Ok(event) => event,
                Err(TryRecvError::Empty) | Err(TryRecvError::Disconnected) => break,
            };
            match &event {
                LoadEvent::Progress { .. } => {}
                LoadEvent::Ready { generation, .. }
                | LoadEvent::RasterReady { generation, .. }
                | LoadEvent::ModelReady { generation, .. }
                | LoadEvent::Failed { generation, .. } => {
                    finish_foreground_flight(&self.foreground_loader.flight, *generation);
                }
            }
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
                    lease,
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
                    log::debug!(
                        "RAW Ready generation={generation} GPU available={}",
                        self.gpu.is_some()
                    );
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
                                    gpu.upload_completions.track_submitted(&gpu.queue);
                                    // Transfer the consumer lease through actual GPU completion.
                                    // Rejected or stale events release theirs automatically.
                                    if let Some(lease) = lease {
                                        submit_upload_lease(&gpu.queue, &gpu.upload_completions, lease);
                                    }
                                    self.image_index = 0;
                                    self.image_count = 1;
                                    gpu.raster_active = false;
                                    gpu.model_active = false;
                                    if let Some(model) = gpu.model.as_mut() {
                                        model.clear_model();
                                    }
                                    if let Some(raster) = gpu.raster.as_mut() {
                                        raster.clear_image();
                                    }
                                    self.view.viewport = [gpu.size.width as f32, gpu.size.height as f32];
                                    let view_uniform_started = Instant::now();
                                    gpu.update_view(self.view);
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
                            "{gallery_prefix}full RAW ready; native RAW decode {:.2?}, total {:.2?}",
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
                            log::warn!("RAW GPU upload generation={generation} rejected: {error}");
                            self.status = format!("GPU upload rejected RAW: {error}");
                            self.pending_first_present = None;
                            pipeline.mark_failed();
                        }
                        None => {
                            log::warn!(
                                "RAW Ready generation={generation} received before GPU initialization"
                            );
                            self.pending_first_present = None;
                        }
                    }
                    self.pipeline = pipeline;
                    if let Some(telemetry) = self.telemetry.as_mut() {
                        telemetry.set_pipeline(&self.pipeline);
                    }
                    self.submit_raw_neighbours();
                }
                LoadEvent::ModelReady {
                    generation,
                    mesh,
                    lease,
                    elapsed,
                } => {
                    if generation != self.foreground_loader.current_generation() {
                        continue;
                    }
                    self.pending_first_present = None;
                    self.pipeline = PipelineSnapshot::idle(generation);
                    let upload_started = Instant::now();
                    let result = self.gpu.as_mut().map(|gpu| {
                        if gpu.model.is_none() && gpu.config.format.is_srgb() {
                            gpu.model = Some(match &self.model_gpu_budget {
                                Some(budget) => rrrah_gpu::ModelRenderer::new_with_budget(
                                    &gpu.device,
                                    gpu.config.format,
                                    budget.clone(),
                                ),
                                None => rrrah_gpu::ModelRenderer::new(&gpu.device, gpu.config.format),
                            });
                        }
                        let renderer = gpu
                            .model
                            .as_mut()
                            .ok_or_else(|| "surface has no sRGB target".to_string())?;
                        renderer
                            .resize(&gpu.device, [gpu.size.width.max(1), gpu.size.height.max(1)])
                            .map_err(|e| e.to_string())?;
                        renderer
                            .upload(&gpu.device, mesh.triangles())
                            .map_err(|e| e.to_string())?;
                        if let Some(lease) = lease {
                            submit_upload_lease(&gpu.queue, &gpu.upload_completions, lease);
                        }
                        gpu.renderer.clear_image();
                        if let Some(raster) = gpu.raster.as_mut() {
                            raster.clear_image();
                        }
                        gpu.raster_active = false;
                        gpu.model_active = true;
                        self.view = ViewParameters {
                            viewport: [gpu.size.width as f32, gpu.size.height as f32],
                            ..ViewParameters::default()
                        };
                        gpu.update_view(self.view);
                        Ok::<(), String>(())
                    });
                    self.status = match result {
                        Some(Ok(())) => {
                            self.image_index = 0;
                            self.image_count = 1;
                            self.scalar_window = None;
                            format!(
                                "{} {} triangles; decoded in {elapsed:.2?}; GPU preparation {:.2?}; drag to orbit",
                                mesh.format_name(),
                                mesh.triangle_count(),
                                upload_started.elapsed()
                            )
                        }
                        Some(Err(error)) => format!("GPU upload rejected model: {error}"),
                        None => "model ready; waiting for window".into(),
                    };
                    if let Some(telemetry) = self.telemetry.as_mut() {
                        telemetry.set_pipeline(&self.pipeline);
                    }
                    self.submit_raw_neighbours();
                }
                LoadEvent::RasterReady {
                    generation,
                    image_index,
                    scalar_window,
                    raster,
                    lease,
                    assumed_srgb,
                    development,
                    elapsed,
                } => {
                    if generation != self.foreground_loader.current_generation() {
                        continue;
                    }
                    self.pending_first_present = None;
                    self.pipeline = PipelineSnapshot::idle(generation);
                    let result = self.gpu.as_mut().map(|gpu| {
                        let renderer = gpu
                            .raster
                            .as_mut()
                            .ok_or_else(|| "surface has no sRGB target".to_string())?;
                        renderer
                            .upload(&gpu.device, &gpu.queue, &raster)
                            .map_err(|e| e.to_string())?;
                        gpu.upload_completions.track_submitted(&gpu.queue);
                        if let Some(lease) = lease {
                            submit_upload_lease(&gpu.queue, &gpu.upload_completions, lease);
                        }
                        if let Some(options) = development.as_ref() {
                            renderer.set_raw_development(&gpu.queue, &options.curve);
                        }
                        gpu.renderer.clear_image();
                        gpu.raster_active = true;
                        gpu.model_active = false;
                        if let Some(model) = gpu.model.as_mut() {
                            model.clear_model();
                        }
                        self.view.viewport = [gpu.size.width as f32, gpu.size.height as f32];
                        gpu.update_view(self.view);
                        Ok::<(), String>(())
                    });
                    self.status = match result {
                        Some(Ok(())) => {
                            self.image_index = image_index;
                            self.scalar_window = scalar_window;
                            self.image_count = raster.image_count();
                            let page = if raster.image_count() > 1 {
                                format!(" ({} of {})", image_index + 1, raster.image_count())
                            } else {
                                String::new()
                            };
                            format!(
                                "image {}x{}{page} ready in {elapsed:.2?}{}{}",
                                raster.width(),
                                raster.height(),
                                if assumed_srgb { "; assumed sRGB" } else { "" },
                                scalar_window
                                    .map(|w| format!("; window {}..{}", w.minimum(), w.maximum()))
                                    .unwrap_or_default()
                            )
                        }
                        Some(Err(error)) => format!("GPU upload rejected image: {error}"),
                        None => "image ready; waiting for window".into(),
                    };
                    if let Some(telemetry) = self.telemetry.as_mut() {
                        telemetry.set_pipeline(&self.pipeline);
                    }
                    self.submit_raw_neighbours();
                }
                LoadEvent::Failed { generation, error } => {
                    if generation == self.foreground_loader.current_generation() {
                        self.status = format!("image load failed: {error}");
                        self.pending_first_present = None;
                        if self.pipeline.generation() != generation {
                            self.pipeline =
                                PipelineSnapshot::waiting(generation, RawKind::from_path(&self.path));
                        }
                        self.pipeline.mark_failed();
                        if let Some(telemetry) = self.telemetry.as_mut() {
                            telemetry.set_pipeline(&self.pipeline);
                        }
                        self.raw_prefetcher
                            .finish_foreground_and_submit(&[], 0, NavDirection::None);
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
            gpu.update_view(self.view);
        }
    }

    /// Establish the folder context for a CLI-opened file: sibling files for
    /// arrow-key navigation plus the filmstrip tile list.
    fn initialize_folder_context(&mut self) {
        if self.path.as_os_str().is_empty() {
            return;
        }
        let Some(folder) = self.path.parent().map(Path::to_path_buf) else {
            return;
        };
        let files = gallery::scan_folder(&folder);
        if !files.is_empty() {
            self.gallery_index = files.iter().position(|candidate| candidate == &self.path);
            self.gallery = files;
        }
        self.set_strip_folder(&folder);
    }

    /// Rebuild the filmstrip for `folder` and queue cover thumbnails that are
    /// not yet cached.
    fn set_strip_folder(&mut self, folder: &Path) {
        for id in self.strip_thumbs.invalidate_changed_sources() {
            if let Some(gpu) = self.gpu.as_mut() {
                gpu.filmstrip.remove_tile(id);
            }
        }
        let tiles = gallery::sibling_folder_tiles(folder);
        let viewport_width = self.view.viewport[0];
        self.strip.set_folder(tiles, folder, viewport_width);
        self.submit_thumb_jobs();
    }

    fn submit_thumb_jobs(&mut self) {
        self.deferred_thumbnail = None;
        let jobs: Vec<_> = self
            .strip
            .tiles
            .iter()
            .enumerate()
            .filter(|(_, tile)| !self.strip_thumbs.contains(&tile.folder))
            .map(|(index, tile)| gallery::ThumbnailJob {
                index,
                source: tile.cover.clone(),
                edge: gallery::THUMB_EDGE,
            })
            .collect();
        self.thumbnail_prefetcher.submit(jobs);
    }

    /// Open a different folder (filmstrip click or dropped directory): scan
    /// it, retarget the strip and load its first image.
    fn open_folder(&mut self, folder: PathBuf) {
        let files = gallery::scan_folder(&folder);
        if files.is_empty() {
            self.set_status(format!("no supported images in {}", folder.display()));
            return;
        }
        self.last_nav_direction = NavDirection::None;
        self.gallery = files;
        self.set_strip_folder(&folder);
        self.open_gallery_index(0);
    }

    /// Drain finished cover thumbnails, upload them to the strip and free
    /// LRU-evicted textures.
    fn drain_thumbnails(&mut self) {
        loop {
            let now = Instant::now();
            let (ready, expires_at) = if let Some(deferred) = self.deferred_thumbnail.take() {
                if deferred.expired(now) {
                    continue;
                }
                if !deferred.due(now) {
                    self.deferred_thumbnail = Some(deferred);
                    break;
                }
                let matches = deferred.is_current(&self.strip.tiles);
                if !matches {
                    continue;
                }
                (deferred.ready, deferred.expires_at)
            } else {
                let Some(ready) = self.thumbnail_prefetcher.try_recv() else {
                    break;
                };
                (ready, now + Duration::from_secs(2))
            };
            let Some(folder) = self.strip.tiles.get(ready.index).map(|tile| tile.folder.clone()) else {
                continue;
            };
            let Some(gpu) = self.gpu.as_mut() else {
                break;
            };
            let id =
                match gpu.filmstrip.try_upload_tile(
                    &gpu.device,
                    &gpu.queue,
                    ready.width,
                    ready.height,
                    &ready.pixels,
                ) {
                    Ok(id) => {
                        gpu.upload_completions.track_submitted(&gpu.queue);
                        id
                    }
                    Err(rrrah_gpu::FilmstripUploadError::TextureMemory(
                        rrrah_core::BufferError::Capacity { requested, limit, .. },
                    )) if requested <= limit => {
                        if let Some(victim) = self.strip_thumbs.evict_oldest() {
                            gpu.filmstrip.remove_tile(victim);
                            self.strip.dirty = true;
                        }
                        self.deferred_thumbnail = Some(DeferredThumbnail {
                            cover: self.strip.tiles[ready.index].cover.clone(),
                            ready,
                            folder,
                            retry_at: now + Duration::from_millis(25),
                            expires_at,
                        });
                        break;
                    }
                    Err(rrrah_gpu::FilmstripUploadError::Memory(rrrah_core::BufferError::Capacity {
                        requested,
                        limit,
                        ..
                    })) if requested <= limit => {
                        self.deferred_thumbnail = Some(DeferredThumbnail {
                            cover: self.strip.tiles[ready.index].cover.clone(),
                            ready,
                            folder,
                            retry_at: now + Duration::from_millis(25),
                            expires_at,
                        });
                        break;
                    }
                    Err(error) => {
                        log::debug!("thumbnail upload skipped: {error}");
                        continue;
                    }
                };
            let source = self.strip.tiles[ready.index].cover.clone();
            if let Some(evicted) =
                self.strip_thumbs
                    .insert_decoded_source(folder, source, ready.source_stamp, id)
            {
                gpu.filmstrip.remove_tile(evicted);
            }
            self.strip.dirty = true;
        }
        if self.strip.dirty
            && let Some(window) = &self.window
        {
            window.request_redraw();
        }
    }

    fn open_dropped_path(&mut self, path: PathBuf) {
        // A fresh drop restarts browsing, so no direction of travel is known.
        self.last_nav_direction = NavDirection::None;
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
            self.open_folder(path);
        } else if file_type.is_file() && is_supported_image(&path) {
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
            if let Some(parent) = path.parent().map(Path::to_path_buf) {
                self.set_strip_folder(&parent);
            }
            self.open_gallery_index(index);
        } else {
            self.set_status("drop rejected: expected a supported image file or folder".into());
        }
    }

    fn open_gallery_index(&mut self, index: usize) {
        let Some(path) = self.gallery.get(index).cloned() else {
            return;
        };
        self.gallery_index = Some(index);
        self.image_index = 0;
        self.image_count = 1;
        self.scalar_window = None;
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
                self.set_status(format!("image loader failed: {error}"));
                return;
            }
        };
        self.pending_first_present = None;
        self.pipeline = PipelineSnapshot::waiting(generation, RawKind::from_path(&path));
        if let Some(telemetry) = self.telemetry.as_mut() {
            telemetry.set_pipeline(&self.pipeline);
        }
        let position = format!("{}/{}", index + 1, self.gallery.len());
        self.set_status(format!("decoding image {position}: {}", display_name(&path)));
    }

    fn navigate_image(&mut self, direction: isize) {
        if self.path.as_os_str().is_empty() {
            return;
        }
        let next = next_image_index(self.image_index, self.image_count, direction);
        let Some(next) = next else {
            return;
        };
        self.raw_prefetcher.begin_foreground();
        match self
            .foreground_loader
            .submit_image_with_window(self.path.clone(), next, self.scalar_window)
        {
            Ok(generation) => {
                self.pending_first_present = None;
                self.pipeline = PipelineSnapshot::waiting(generation, RawKind::from_path(&self.path));
                self.set_status(format!(
                    "decoding image {}: {}",
                    next + 1,
                    display_name(&self.path)
                ));
            }
            Err(error) => self.set_status(format!("image loader failed: {error}")),
        }
    }

    fn adjust_scientific_window(&mut self, shift: f64, width_factor: f64) {
        let Some(current) = self.scalar_window else {
            return;
        };
        let Some(window) = adjusted_scalar_window(current, shift, width_factor) else {
            self.set_status("window adjustment exceeds finite sample bounds".into());
            return;
        };
        self.raw_prefetcher.begin_foreground();
        match self.foreground_loader.submit_image_with_window(
            self.path.clone(),
            self.image_index,
            Some(window),
        ) {
            Ok(generation) => {
                self.pending_first_present = None;
                self.pipeline = PipelineSnapshot::waiting(generation, RawKind::from_path(&self.path));
                self.set_status(format!(
                    "applying window {}..{}",
                    window.minimum(),
                    window.maximum()
                ));
            }
            Err(error) => self.set_status(format!("image loader failed: {error}")),
        }
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
            self.last_nav_direction = match direction {
                -1 => NavDirection::Backward,
                1 => NavDirection::Forward,
                _ => NavDirection::None,
            };
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

fn is_supported_image(path: &Path) -> bool {
    rrrah_decode::is_supported_image_path(path) || rrrah_decode::is_supported_model_path(path)
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
        let gpu = match pollster::block_on(GpuState::new(
            event_loop.owned_display_handle(),
            window.clone(),
            self.raster_gpu_budget.clone(),
            self.raw_gpu_budget.clone(),
            self.filmstrip_gpu_budget.clone(),
            self.upload_memory_budget.clone(),
            self.gpu_backend,
            self.gpu_vendor,
        )) {
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
                .with_min_inner_size(LogicalSize::new(1180.0, 820.0))
                .with_visible(false),
        ) {
            Ok(telemetry_window) => {
                let telemetry_window = Arc::new(telemetry_window);
                match pollster::block_on(TelemetryState::new(
                    event_loop.owned_display_handle(),
                    telemetry_window.clone(),
                    self.gpu_backend,
                    self.gpu_vendor,
                )) {
                    Ok(mut telemetry) => {
                        telemetry.set_pipeline(&self.pipeline);
                        telemetry.set_cache_snapshot(self.cache_telemetry.snapshot());
                        telemetry_window.set_minimized(true);
                        telemetry_window.set_visible(true);
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
        window.focus_window();
        window.request_redraw();
        if let Some(telemetry_window) = &self.telemetry_window {
            telemetry_window.request_redraw();
        }
    }

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        self.drain_load_events();
        self.drain_thumbnails();
        let pending = self.gpu.as_ref().is_some_and(|gpu| {
            gpu.upload_completions.poll(&gpu.device).unwrap_or_else(|error| {
                log::warn!("GPU upload completion polling failed: {error}");
                false
            })
        });
        // Callback delivery must progress without redraw or surface acquisition.
        let retry = self
            .deferred_thumbnail
            .as_ref()
            .map(|thumbnail| thumbnail.retry_at);
        if retry.is_some()
            && let Some(gpu) = &self.gpu
        {
            // Budget callbacks must advance without a visible redraw.
            if let Err(error) = gpu.device.poll(wgpu::PollType::Poll) {
                log::warn!("thumbnail upload completion polling failed: {error}");
            }
        }
        event_loop.set_control_flow(if pending {
            ControlFlow::WaitUntil(Instant::now() + Duration::from_millis(10))
        } else if let Some(deadline) = retry {
            ControlFlow::WaitUntil(deadline)
        } else {
            ControlFlow::Wait
        });
    }

    fn user_event(&mut self, _event_loop: &ActiveEventLoop, _event: WakeEvent) {
        self.drain_load_events();
        self.drain_thumbnails();
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
                WindowEvent::ModifiersChanged(modifiers) => {
                    self.modifiers = modifiers.state();
                }
                WindowEvent::MouseInput { state, button, .. } if button == MouseButton::Left => {
                    self.telemetry_dragging = state == ElementState::Pressed;
                    if !self.telemetry_dragging {
                        self.telemetry_last_cursor = None;
                    }
                }
                WindowEvent::CursorMoved { position, .. } => {
                    if self.telemetry_dragging {
                        if let Some(previous) = self.telemetry_last_cursor {
                            if let Some(telemetry) = self.telemetry.as_mut() {
                                // Grab-pan: dragging down pulls the content
                                // down, i.e. scrolls back toward the top.
                                telemetry.scroll_cards(previous.y as f32 - position.y as f32);
                            }
                        }
                    }
                    self.telemetry_last_cursor = Some(position);
                }
                WindowEvent::MouseWheel { delta, .. } => {
                    if let Some(telemetry) = self.telemetry.as_mut() {
                        let amount = match delta {
                            MouseScrollDelta::LineDelta(_, y) => y,
                            MouseScrollDelta::PixelDelta(position) => position.y as f32 / 80.0,
                        };
                        if self.modifiers.control_key() || self.modifiers.super_key() {
                            telemetry.zoom_cards(1.12_f32.powf(amount));
                        } else {
                            telemetry.scroll_cards(-amount * 48.0);
                        }
                    }
                }
                WindowEvent::KeyboardInput { event, .. }
                    if event.state == ElementState::Pressed && !event.repeat =>
                {
                    if let Some(telemetry) = self.telemetry.as_mut() {
                        match event.physical_key {
                            PhysicalKey::Code(KeyCode::Digit0) | PhysicalKey::Code(KeyCode::KeyF) => {
                                telemetry.reset_cards_view();
                            }
                            PhysicalKey::Code(KeyCode::Equal) | PhysicalKey::Code(KeyCode::NumpadAdd) => {
                                telemetry.zoom_cards(1.12);
                            }
                            PhysicalKey::Code(KeyCode::Minus)
                            | PhysicalKey::Code(KeyCode::NumpadSubtract) => {
                                telemetry.zoom_cards(1.0 / 1.12);
                            }
                            _ => {}
                        }
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
                    gpu.update_view(self.view);
                }
                self.strip.dirty = true;
            }
            WindowEvent::RedrawRequested => {
                if self.strip.dirty
                    && let Some(gpu) = self.gpu.as_mut()
                {
                    let frame = self.strip.build_frame(&mut self.strip_thumbs);
                    gpu.filmstrip.update(&gpu.device, &gpu.queue, &frame);
                    self.strip.dirty = false;
                }
                let draw_strip = self.strip.visible && !self.strip.tiles.is_empty();
                let display_submit = self.gpu.as_mut().and_then(|gpu| gpu.render(draw_strip));
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
                            log::debug!(
                                "first window frame presented generation={} total_ms={:.3}",
                                pending.generation,
                                pending.requested_at.elapsed().as_secs_f64() * 1000.0
                            );
                        }
                    }
                }
                if let Some(telemetry_window) = &self.telemetry_window {
                    telemetry_window.request_redraw();
                }
            }
            WindowEvent::Occluded(false) => {
                // Surface acquisition can defer the first frame while hidden.
                // Keep the uploaded frame and resume presentation on visibility.
                if let Some(window) = &self.window {
                    window.request_redraw();
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
                let strip_click = state == ElementState::Pressed
                    && self.strip.visible
                    && self.last_cursor.is_some_and(|position| {
                        rrrah_gpu::point_in_strip(position.y as f32, self.view.viewport[1])
                    });
                if strip_click {
                    self.dragging = false;
                    let clicked = self
                        .last_cursor
                        .and_then(|position| self.strip.tile_at(position.x as f32));
                    if let Some(index) = clicked.filter(|index| Some(*index) != self.strip.current) {
                        let folder = self.strip.tiles[index].folder.clone();
                        self.open_folder(folder);
                    }
                } else {
                    self.dragging = state == ElementState::Pressed;
                    if !self.dragging {
                        self.last_cursor = None;
                    }
                }
            }
            WindowEvent::MouseWheel { delta, .. } => {
                let amount = match delta {
                    MouseScrollDelta::LineDelta(_, y) => y,
                    MouseScrollDelta::PixelDelta(position) => position.y as f32 / 80.0,
                };
                let over_strip = self.strip.visible
                    && self.last_cursor.is_some_and(|position| {
                        rrrah_gpu::point_in_strip(position.y as f32, self.view.viewport[1])
                    });
                if over_strip {
                    self.strip.scroll_by(-amount * 64.0, self.view.viewport[0]);
                    if let Some(window) = &self.window {
                        window.request_redraw();
                    }
                } else {
                    self.view.zoom = (self.view.zoom * 1.12_f32.powf(amount)).clamp(0.02, 128.0);
                    self.update_view();
                }
            }
            WindowEvent::KeyboardInput { event, .. }
                if event.state == ElementState::Pressed && !event.repeat =>
            {
                match event.physical_key {
                    PhysicalKey::Code(KeyCode::BracketLeft) => self.navigate_image(-1),
                    PhysicalKey::Code(KeyCode::BracketRight) => self.navigate_image(1),
                    PhysicalKey::Code(KeyCode::ArrowLeft) => self.navigate_gallery(-1),
                    PhysicalKey::Code(KeyCode::ArrowRight) => self.navigate_gallery(1),
                    PhysicalKey::Code(KeyCode::ArrowUp) => self.adjust_scientific_window(0.1, 1.),
                    PhysicalKey::Code(KeyCode::ArrowDown) => self.adjust_scientific_window(-0.1, 1.),
                    PhysicalKey::Code(KeyCode::PageUp) => self.adjust_scientific_window(0., 1.25),
                    PhysicalKey::Code(KeyCode::PageDown) => self.adjust_scientific_window(0., 0.8),
                    PhysicalKey::Code(KeyCode::KeyG) => {
                        self.strip.visible = !self.strip.visible;
                        if let Some(window) = &self.window {
                            window.request_redraw();
                        }
                    }
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
    async fn new(
        display: OwnedDisplayHandle,
        window: Arc<Window>,
        backend: rrrah_gpu::GpuBackend,
        vendor: rrrah_gpu::GpuVendor,
    ) -> Result<Self> {
        let mut descriptor = wgpu::InstanceDescriptor::new_with_display_handle(Box::new(display));
        backend.configure(&mut descriptor)?;
        let instance = wgpu::Instance::new(descriptor);
        let surface = instance
            .create_surface(window.clone())
            .context("create telemetry wgpu surface")?;
        let adapter = vendor
            .request_adapter(
                &instance,
                &wgpu::RequestAdapterOptions {
                    power_preference: wgpu::PowerPreference::LowPower,
                    compatible_surface: Some(&surface),
                    force_fallback_adapter: false,
                    apply_limit_buckets: false,
                },
            )
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

    fn scroll_cards(&mut self, delta: f32) {
        self.hud.scroll_cards_by(delta);
        self.rebuild_hud();
        self.window.request_redraw();
    }

    fn zoom_cards(&mut self, factor: f32) {
        self.hud.zoom_cards_by(factor);
        self.rebuild_hud();
        self.window.request_redraw();
    }

    fn reset_cards_view(&mut self) {
        self.hud.reset_cards_view();
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
    upload_completions: UploadCompletions,
    config: wgpu::SurfaceConfiguration,
    size: PhysicalSize<u32>,
    renderer: RawRenderer,
    raster: Option<rrrah_gpu::RasterRenderer>,
    raster_active: bool,
    model: Option<rrrah_gpu::ModelRenderer>,
    model_active: bool,
    filmstrip: FilmstripRenderer,
}

impl GpuState {
    async fn new(
        display: OwnedDisplayHandle,
        window: Arc<Window>,
        raster_gpu_budget: Option<rrrah_core::MemoryBudget>,
        raw_gpu_budget: Option<rrrah_core::MemoryBudget>,
        filmstrip_gpu_budget: Option<rrrah_core::MemoryBudget>,
        upload_memory_budget: Option<rrrah_core::MemoryBudget>,
        backend: rrrah_gpu::GpuBackend,
        vendor: rrrah_gpu::GpuVendor,
    ) -> Result<Self> {
        let mut descriptor = wgpu::InstanceDescriptor::new_with_display_handle(Box::new(display));
        backend.configure(&mut descriptor)?;
        let instance = wgpu::Instance::new(descriptor);
        let surface = instance
            .create_surface(window.clone())
            .context("create wgpu surface")?;
        let adapter = vendor
            .request_adapter(
                &instance,
                &wgpu::RequestAdapterOptions {
                    power_preference: wgpu::PowerPreference::HighPerformance,
                    compatible_surface: Some(&surface),
                    force_fallback_adapter: false,
                    apply_limit_buckets: false,
                },
            )
            .await
            .context("request GPU adapter")?;
        let info = adapter.get_info();
        log::info!(
            "GPU adapter: backend={:?} name={} vendor={:#06x} device={:#06x}",
            info.backend,
            info.name,
            info.vendor,
            info.device
        );
        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor::default())
            .await
            .context("request GPU device")?;
        let capabilities = surface.get_capabilities(&adapter);
        let hdr_surface = match std::env::var("RRRAH_HDR_SURFACE").as_deref() {
            Ok("1") => true,
            Ok("0") | Err(_) => false,
            Ok(value) => anyhow::bail!("invalid RRRAH_HDR_SURFACE={value}; expected 0 or 1"),
        };
        let (surface_format, surface_color_space) = select_viewer_surface(&capabilities, hdr_surface)?;
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
            color_space: surface_color_space,
        };
        surface.configure(&device, &config);
        let renderer = match raw_gpu_budget {
            Some(budget) => RawRenderer::new_with_budget(&device, surface_format, budget),
            None => RawRenderer::new(&device, surface_format),
        };
        let mut renderer = renderer;
        if hdr_surface {
            renderer.set_output_mode(&queue, rrrah_gpu::RawOutputMode::SceneLinear)?;
        }
        let renderer = match upload_memory_budget.clone() {
            Some(budget) => renderer
                .with_upload_memory_budget(budget.clone())
                .with_upload_queue_budget(budget),
            None => renderer,
        };
        let raster = (surface_format.is_srgb() || hdr_surface).then(|| {
            let raster = match (hdr_surface, raster_gpu_budget) {
                (true, Some(budget)) => {
                    rrrah_gpu::RasterRenderer::new_linear_hdr_with_budget(&device, budget)
                }
                (true, None) => rrrah_gpu::RasterRenderer::new_linear_hdr(&device),
                (false, Some(budget)) => {
                    rrrah_gpu::RasterRenderer::new_with_budget(&device, surface_format, budget)
                }
                (false, None) => rrrah_gpu::RasterRenderer::new(&device, surface_format),
            };
            match upload_memory_budget.clone() {
                Some(budget) => raster.with_upload_queue_budget(budget),
                None => raster,
            }
        });
        let filmstrip =
            FilmstripRenderer::new(&device, surface_format, [size.width as f32, size.height as f32]);
        let filmstrip = match filmstrip_gpu_budget {
            Some(budget) => filmstrip.with_texture_budget(budget),
            None => filmstrip,
        };
        let filmstrip = match upload_memory_budget {
            Some(budget) => filmstrip.with_upload_queue_budget(budget),
            None => filmstrip,
        };
        Ok(Self {
            _instance: instance,
            window,
            surface,
            device,
            queue,
            upload_completions: UploadCompletions::default(),
            config,
            size,
            renderer,
            raster,
            raster_active: false,
            model: None,
            model_active: false,
            filmstrip,
        })
    }

    fn update_view(&mut self, view: ViewParameters) {
        if let Some(model) = self.model.as_mut().filter(|_| self.model_active) {
            model.update_view(
                &self.queue,
                view.viewport[0] / view.viewport[1].max(1.),
                0.65 + view.pan[0] * 0.01,
                0.45 + view.pan[1] * 0.01,
                view.zoom,
            );
        }
        self.renderer.update_view(&self.queue, view);
        if let Some(raster) = self.raster.as_mut() {
            raster.update_view(&self.queue, view);
        }
    }

    fn resize(&mut self, size: PhysicalSize<u32>) {
        if size.width == 0 || size.height == 0 {
            return;
        }
        self.size = size;
        self.config.width = size.width;
        self.config.height = size.height;
        self.surface.configure(&self.device, &self.config);
        if self.model_active {
            if let Some(model) = self.model.as_mut() {
                if let Err(error) = model.resize(&self.device, [size.width, size.height]) {
                    log::error!("model viewport resize rejected: {error}");
                    self.model_active = false;
                }
            }
        }
        self.filmstrip
            .resize(&self.queue, [size.width as f32, size.height as f32]);
    }

    fn render(&mut self, draw_strip: bool) -> Option<FrameSubmitTimings> {
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
            wgpu::CurrentSurfaceTexture::Occluded => {
                log::debug!("viewer surface acquisition deferred: occluded");
                return None;
            }
            wgpu::CurrentSurfaceTexture::Validation => {
                log::warn!("viewer surface acquisition rejected by validation");
                return None;
            }
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
        if self.model_active {
            if let Some(model) = &self.model {
                model.encode(&mut encoder, &view);
            }
        } else if self.raster_active {
            if let Some(raster) = &self.raster {
                raster.encode(&mut encoder, &view);
            }
        } else {
            self.renderer.encode(&mut encoder, &view);
        }
        if draw_strip {
            self.filmstrip.encode(&mut encoder, &view);
        }
        let command_buffer = encoder.finish();
        let frame_encode = frame_encode_started.elapsed();
        let queue_submit_started = Instant::now();
        let mut resources = rrrah_gpu::GpuResourceLease::default();
        if self.model_active {
            if let Some(model) = &self.model {
                resources.extend(model.resource_lease());
            }
        } else if self.raster_active {
            if let Some(raster) = &self.raster {
                resources.extend(raster.resource_lease());
            }
        } else {
            resources.extend(self.renderer.resource_lease());
        }
        if draw_strip {
            resources.extend(self.filmstrip.resource_lease());
        }
        self.queue.submit([command_buffer]);
        if !resources.is_empty() {
            resources.retain_until_complete(&self.queue);
            self.upload_completions.track_submitted(&self.queue);
        }
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
    #[test]
    fn rla_straight_and_premultiplied_use_distinct_cached_pixels() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("associated.rla");
        let mut bytes = vec![0u8; 744];
        bytes[2..4].copy_from_slice(&1u16.to_be_bytes());
        let window: Vec<_> = bytes[..8].to_vec();
        bytes[8..16].copy_from_slice(&window);
        for (offset, value) in [(20, 3u16), (22, 1), (658, 8), (662, 8)] {
            bytes[offset..offset + 2].copy_from_slice(&value.to_be_bytes());
        }
        bytes[740..744].copy_from_slice(&744u32.to_be_bytes());
        for channel in [[64, 128], [32, 64], [16, 32], [128, 255]] {
            bytes.extend_from_slice(&[0, 3, 254, channel[0], channel[1]]);
        }
        std::fs::write(&path, bytes).unwrap();
        let budget = rrrah_cache::MemoryBudget::new(4096);
        let mut request = DecodeRequest::new(path);
        request.memory_budget = Some(budget.clone());
        request.assume_untagged_srgb = true;
        let mut cache = RasterDisplayCache::new(rrrah_cache::CacheLimits::bytes(4096));
        request.rla_alpha_mode = Some(rrrah_decode::RlaAlphaMode::Straight);
        let (straight, _) = load_cached_raster_for_display(&request, None, &mut cache).unwrap();
        request.rla_alpha_mode = Some(rrrah_decode::RlaAlphaMode::Premultiplied);
        let (associated, _) = load_cached_raster_for_display(&request, None, &mut cache).unwrap();
        let (rrrah_core::RasterPixels::Rgba32Float(a), rrrah_core::RasterPixels::Rgba32Float(b)) =
            (straight.pixels(), associated.pixels())
        else {
            panic!()
        };
        assert_ne!(a[0].to_bits(), b[0].to_bits());
        assert_eq!(a[3].to_bits(), b[3].to_bits());
        let linear = |encoded: f64| {
            if encoded <= 0.04045 {
                encoded / 12.92
            } else {
                ((encoded + 0.055) / 1.055).powf(2.4)
            }
        };
        assert!((f64::from(a[0]) - linear(64.0 / 255.0)).abs() < 3e-7);
        assert!((f64::from(b[0]) - linear(64.0 / 128.0)).abs() < 3e-7);
        assert_eq!(cache.len(), 2);
        for mode in [
            rrrah_decode::RlaAlphaMode::Straight,
            rrrah_decode::RlaAlphaMode::Premultiplied,
        ] {
            request.rla_alpha_mode = Some(mode);
            let (hit, _) =
                load_cached_raster_with(&request, None, &mut cache, || panic!("alpha policy cache miss"))
                    .unwrap();
            let rrrah_core::RasterPixels::Rgba32Float(pixels) = hit.pixels() else {
                panic!()
            };
            let expected = if mode == rrrah_decode::RlaAlphaMode::Straight {
                a
            } else {
                b
            };
            assert!(pixels.ptr_eq(expected));
        }
        drop((straight, associated, cache));
        assert_eq!(budget.used(), 0);
    }
    #[test]
    fn rla_cli_policy_reaches_requests_and_thumbnail_loader() {
        let cli =
            Cli::try_parse_from(["rrrah", "--rla-alpha", "straight", "--untagged-color", "srgb"]).unwrap();
        let policy = cli.raster_interpretation();
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../tests/fixtures/raster/rla-8-c3-a8-mixed1.rla");
        let budget = rrrah_cache::MemoryBudget::new(1024 * 1024);
        let mut request = DecodeRequest::new(&path);
        request.memory_budget = Some(budget.clone());
        policy.apply(&mut request);
        assert_eq!(request.rla_alpha_mode, Some(rrrah_decode::RlaAlphaMode::Straight));
        let (ready, _) = load_raster_for_display_typed(&request, None).unwrap();
        drop(ready);
        let loader = thumbnail_loader_with_cancel(Arc::new(DecodeGate::new()), Some(budget.clone()), policy);
        let generation = Arc::new(AtomicU64::new(1));
        let thumbnail = loader(
            gallery::ThumbnailJob {
                index: 0,
                source: path,
                edge: 32,
            },
            GenerationToken::new(generation, 1),
        )
        .expect("configured RLA thumbnail must load");
        drop(thumbnail);
        assert_eq!(budget.used(), 0);
        let strict = Cli::try_parse_from(["rrrah"]).unwrap().raster_interpretation();
        strict.apply(&mut request);
        assert_eq!(request.rla_alpha_mode, None);
        assert!(!request.assume_untagged_srgb);
        assert!(Cli::try_parse_from(["rrrah", "--rla-alpha", "unknown"]).is_err());
        let premultiplied = Cli::try_parse_from(["rrrah", "--rla-alpha", "premultiplied"])
            .unwrap()
            .raster_interpretation();
        premultiplied.apply(&mut request);
        assert_eq!(
            request.rla_alpha_mode,
            Some(rrrah_decode::RlaAlphaMode::Premultiplied)
        );
    }
    #[test]
    fn rla_offset_canvas_thumbnail_preserves_extent_and_releases_budget() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("offset.rla");
        let mut source = include_bytes!("../../../tests/fixtures/raster/rla-8-c3-a8-mixed1.rla").to_vec();
        for (offset, coordinate) in [(0, -2i16), (2, 142), (4, -1), (6, 4)] {
            source[offset..offset + 2].copy_from_slice(&coordinate.to_be_bytes());
        }
        std::fs::write(&path, source).unwrap();
        let policy = Cli::try_parse_from(["rrrah", "--rla-alpha", "straight", "--untagged-color", "srgb"])
            .unwrap()
            .raster_interpretation();
        let budget = rrrah_cache::MemoryBudget::new(1024 * 1024);
        let loader = thumbnail_loader_with_cancel(Arc::new(DecodeGate::new()), Some(budget.clone()), policy);
        let generation = Arc::new(AtomicU64::new(1));
        let job = gallery::ThumbnailJob {
            index: 7,
            source: path,
            edge: 145,
        };
        let thumbnail = loader(job.clone(), GenerationToken::new(generation.clone(), 1)).unwrap();
        assert_eq!((thumbnail.width, thumbnail.height, thumbnail.index), (145, 6, 7));
        // Transparent canvas is composited over the thumbnail's declared linear background.
        let background = ((1.055f64 * 0.018f64.powf(1.0 / 2.4) - 0.055) * 255.0).round() as u8;
        for y in 0..6usize {
            for x in 0..145usize {
                if !(2..142).contains(&x) || !(2..5).contains(&y) {
                    assert_eq!(
                        &thumbnail.pixels[(y * 145 + x) * 4..(y * 145 + x + 1) * 4],
                        &[background, background, background, 255]
                    );
                }
            }
        }
        assert!(budget.used() >= 145 * 6 * 4);
        drop(thumbnail);
        assert_eq!(budget.used(), 0);
        generation.store(2, Ordering::Release);
        assert!(loader(job.clone(), GenerationToken::new(generation.clone(), 1)).is_none());
        assert_eq!(budget.used(), 0);
        let short = rrrah_cache::MemoryBudget::new(145 * 6 * 8 - 1);
        let limited = thumbnail_loader_with_cancel(Arc::new(DecodeGate::new()), Some(short.clone()), policy);
        assert!(limited(job, GenerationToken::new(generation, 2)).is_none());
        assert_eq!(short.used(), 0);
    }
    #[test]
    fn rla_cache_alpha_policy_cannot_bypass_strict_refusal() {
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../tests/fixtures/raster/rla-8-c3-a8-mixed1.rla");
        let budget = rrrah_cache::MemoryBudget::new(1024 * 1024);
        let mut request = DecodeRequest::new(path);
        request.memory_budget = Some(budget.clone());
        request.assume_untagged_srgb = true;
        request.rla_alpha_mode = Some(rrrah_decode::RlaAlphaMode::Straight);
        let mut cache = RasterDisplayCache::new(rrrah_cache::CacheLimits::bytes(budget.limit()));
        let (ready, _) = load_cached_raster_for_display(&request, None, &mut cache).unwrap();
        request.rla_alpha_mode = None;
        assert!(
            load_cached_raster_for_display(&request, None, &mut cache).is_err(),
            "strict RLA policy must not reuse the straight-alpha cache entry"
        );
        request.rla_alpha_mode = Some(rrrah_decode::RlaAlphaMode::Straight);
        let (hit, _) = load_cached_raster_with(&request, None, &mut cache, || {
            panic!("straight-alpha frame should remain cached")
        })
        .unwrap();
        drop((ready, hit, cache));
        assert_eq!(budget.used(), 0);
    }
    #[test]
    #[ignore = "requires pinned AI source corpus; set RRRAH_AI_CORPUS"]
    fn real_ai_pages_use_shared_ram_hits_and_independent_prefetch_keys() {
        let corpus = PathBuf::from(std::env::var("RRRAH_AI_CORPUS").unwrap());
        let budget = rrrah_cache::MemoryBudget::new(64 * 1024 * 1024);
        let mut cache = RasterDisplayCache::new(rrrah_cache::CacheLimits::bytes(32 * 1024 * 1024));
        let gate = Arc::new(DecodeGate::new());
        for (name, index, extent, count) in [
            ("VectorApple.ai", 0, (301, 246), 1),
            ("one.ai", 0, (1366, 768), 2),
            ("one.ai", 1, (177, 175), 2),
        ] {
            let mut request = DecodeRequest::new(corpus.join(name));
            request.image_index = index;
            request.memory_budget = Some(budget.clone());
            let (original, _) = if index == 0 {
                load_cached_raster_with(&request, None, &mut cache, || {
                    load_raster_for_display_typed(&request, None)
                })
                .unwrap()
            } else {
                preload_raster(&request, None, &mut cache, &gate).unwrap()
            };
            let used = budget.used();
            let (hit, _) = load_cached_raster_with(&request, None, &mut cache, || {
                panic!("resident AI was decoded again")
            })
            .unwrap();
            assert_eq!((hit.width(), hit.height()), extent);
            assert_eq!((hit.image_index(), hit.image_count()), (index, count));
            let (rrrah_core::RasterPixels::Rgba32Float(a), rrrah_core::RasterPixels::Rgba32Float(b)) =
                (original.pixels(), hit.pixels())
            else {
                panic!()
            };
            assert!(a.ptr_eq(b));
            assert_eq!(budget.used(), used);
        }
        assert_eq!(cache.len(), 3);
        drop(cache);
        assert_eq!(budget.used(), 0);
    }
    #[test]
    fn invalid_float_icc_does_not_pollute_raster_cache_or_evict_visible_pixels() {
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../tests/fixtures/raster/pattern.profiled.png");
        let budget = rrrah_cache::MemoryBudget::new(1024 * 1024);
        let mut request = DecodeRequest::new(&path);
        request.memory_budget = Some(budget.clone());
        let mut cache = RasterDisplayCache::new(rrrah_cache::CacheLimits::bytes(1024 * 1024));
        let (visible, _) = load_cached_raster_with(&request, None, &mut cache, || {
            load_raster_for_display_typed(&request, None)
        })
        .unwrap();
        let decoded = rrrah_decode::decode_raster_file(&path).unwrap();
        let invalid = rrrah_core::DecodedRaster::new(
            1,
            1,
            rrrah_core::RasterPixels::Rgba32Float(Arc::new(vec![4.0, 0.5, 0.5, 1.0]).into()),
            decoded.color_space().clone(),
        )
        .unwrap()
        .try_manage_pixels(&budget)
        .unwrap();
        let used = budget.used();
        let peak = budget.peak();
        let mut other = request.clone();
        other.image_index = 1;
        let mut calls = 0;
        let error = load_cached_raster_with(&other, None, &mut cache, || {
            calls += 1;
            rrrah_decode::prepare_raster_for_display_with_budget(&invalid, Some(&budget))
                .map(|frame| (frame, false))
                .map_err(RasterLoadError::Color)
        })
        .unwrap_err();
        assert!(error.contains("finite and normalized"), "{error}");
        assert_eq!(calls, 1);
        assert_eq!(cache.len(), 1);
        assert_eq!(budget.used(), used);
        assert_eq!(budget.peak(), peak);
        let (hit, _) =
            load_cached_raster_with(&request, None, &mut cache, || panic!("visible pixels evicted")).unwrap();
        let (rrrah_core::RasterPixels::Rgba32Float(a), rrrah_core::RasterPixels::Rgba32Float(b)) =
            (visible.pixels(), hit.pixels())
        else {
            panic!()
        };
        assert!(a.ptr_eq(b));
        drop(invalid);
        let (retry, _) = load_cached_raster_with(&other, None, &mut cache, || {
            Ok((visible.clone().with_image_selection(1, 2).unwrap(), false))
        })
        .unwrap();
        assert_eq!(cache.len(), 2);
        assert_eq!(retry.image_index(), 1);
        drop((retry, hit, visible, cache));
        assert_eq!(budget.used(), 0);
    }
    #[test]
    fn incomplete_pdf_prefetch_is_not_cached_and_next_decode_can_run() {
        let fixtures = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/pdf");
        let budget = rrrah_cache::MemoryBudget::new(65536);
        let mut broken = DecodeRequest::new(fixtures.join("corrupt-embedded-jpeg.pdf"));
        broken.memory_budget = Some(budget.clone());
        let mut cache = RasterDisplayCache::new(rrrah_cache::CacheLimits::bytes(65536));
        let gate = Arc::new(DecodeGate::new());
        let error = preload_raster(&broken, None, &mut cache, &gate).unwrap_err();
        assert!(error.contains("embedded image decode failure"), "{error}");
        assert_eq!(cache.len(), 0);
        assert!(budget.peak() > 800);
        assert_eq!(budget.used(), 0);
        // A failed render must release its prefetch permit as well as buffers.
        let mut good = DecodeRequest::new(fixtures.join("red-green-pages.pdf"));
        good.memory_budget = Some(budget.clone());
        let (red, _) = preload_raster(&good, None, &mut cache, &gate).unwrap();
        assert_eq!(cache.len(), 1);
        let (hit, _) = load_cached_raster_with(&good, None, &mut cache, || {
            panic!("successful retry was not cached")
        })
        .unwrap();
        let rrrah_core::RasterPixels::Rgba32Float(pixels) = hit.pixels() else {
            panic!()
        };
        assert!(pixels.chunks_exact(4).all(|pixel| pixel == [1., 0., 0., 1.]));
        drop((hit, red, cache));
        assert_eq!(budget.used(), 0);
    }
    #[test]
    fn pdf_selected_page_preload_keeps_distinct_cache_keys_and_visible_page() {
        let path =
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/pdf/red-green-pages.pdf");
        let budget = rrrah_cache::MemoryBudget::new(65536);
        let mut first = DecodeRequest::new(&path);
        first.memory_budget = Some(budget.clone());
        let mut second = first.clone();
        second.image_index = 1;
        let mut cache = RasterDisplayCache::new(rrrah_cache::CacheLimits::bytes(65536));
        let (red, _) = load_cached_raster_with(&first, None, &mut cache, || {
            load_raster_for_display_typed(&first, None)
        })
        .unwrap();
        let gate = Arc::new(DecodeGate::new());
        let (green, _) = preload_raster(&second, None, &mut cache, &gate).unwrap();
        assert_eq!(cache.len(), 2);
        let (red_hit, _) =
            load_cached_raster_with(&first, None, &mut cache, || panic!("page zero decoded again")).unwrap();
        let (green_hit, _) = load_cached_raster_mode(&second, None, &mut cache, false, || {
            panic!("page one decoded again")
        })
        .unwrap();
        for (page, hit, index, color) in [
            (&red, &red_hit, 0, [1., 0., 0., 1.]),
            (&green, &green_hit, 1, [0., 1., 0., 1.]),
        ] {
            assert_eq!((hit.image_index(), hit.image_count()), (index, 2));
            let (rrrah_core::RasterPixels::Rgba32Float(a), rrrah_core::RasterPixels::Rgba32Float(b)) =
                (page.pixels(), hit.pixels())
            else {
                panic!()
            };
            assert!(a.ptr_eq(b));
            assert!(b.chunks_exact(4).all(|p| p == color));
        }
        // Prefetched page remains unpinned, so count shrink retains visible page 0.
        let victims = cache
            .set_limits(rrrah_cache::CacheLimits {
                max_bytes: 65536,
                max_entries: Some(1),
                ttl: None,
            })
            .unwrap();
        assert_eq!(cache.len(), 1);
        assert_eq!(victims.len(), 1);
        let (hit, _) = load_cached_raster_with(&first, None, &mut cache, || {
            panic!("visible page evicted by prefetch")
        })
        .unwrap();
        assert_eq!(hit.image_index(), 0);
        drop((red, green, red_hit, green_hit, hit, victims, cache));
        assert_eq!(budget.used(), 0);
    }
    use super::*;

    #[test]
    #[ignore = "requires Metal device and RRRAH_SONY_SR2_CORPUS pinned source"]
    fn metal_upload_retains_raw_lease_until_submission_completion() {
        let path = PathBuf::from(std::env::var("RRRAH_SONY_SR2_CORPUS").unwrap()).join("3221.sr2");
        let budget = rrrah_cache::MemoryBudget::new(128 * 1024 * 1024);
        let mut request = DecodeRequest::new(&path);
        request.memory_budget = Some(budget.clone());
        let mosaic = NativeRawDecoder.decode(&request).unwrap().mosaic;
        let bytes = mosaic.pixels.capacity_bytes();
        let fingerprint = SourceFingerprint::from_path(&path).unwrap();
        let key = CacheKey::for_mosaic(&fingerprint, 0);
        let mut cache = MosaicRamCache::new(bytes);
        assert!(cache.insert(key, mosaic));
        let lease = cache.get_lease(&key).unwrap();
        let mut descriptor = wgpu::InstanceDescriptor::new_without_display_handle();
        rrrah_gpu::GpuBackend::Metal.configure(&mut descriptor).unwrap();
        let instance = wgpu::Instance::new(descriptor);
        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            compatible_surface: None,
            force_fallback_adapter: false,
            power_preference: wgpu::PowerPreference::HighPerformance,
            apply_limit_buckets: false,
        }))
        .unwrap();
        assert_eq!(adapter.get_info().backend, wgpu::Backend::Metal);
        let (device, queue) =
            pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor::default())).unwrap();
        let completions = UploadCompletions::default();
        let mut renderer = rrrah_gpu::RawRenderer::new(&device, wgpu::TextureFormat::Rgba8UnormSrgb);
        renderer.upload_mosaic(&device, &queue, &lease).unwrap();
        assert!(cache.set_limits(rrrah_cache::CacheLimits::bytes(0)).is_none());
        submit_upload_lease(&queue, &completions, lease);
        let deadline = Instant::now() + Duration::from_secs(5);
        while completions.poll(&device).unwrap() {
            assert!(
                Instant::now() < deadline,
                "upload callback needs presentation to complete"
            );
            thread::sleep(Duration::from_millis(1));
        }
        assert_eq!(completions.pending.load(Ordering::Acquire), 0);
        assert!(cache.set_limits(rrrah_cache::CacheLimits::bytes(0)).is_some());
        assert_eq!(budget.used(), 0);
        let raster_budget = rrrah_cache::MemoryBudget::new(16);
        let raster = rrrah_core::DecodedRaster::new(
            1,
            1,
            rrrah_core::RasterPixels::Rgba32Float(Arc::new(vec![4.0, -0.0, -0.5, 1.0]).into()),
            rrrah_core::RasterColorSpace::LinearSrgb,
        )
        .unwrap()
        .try_manage_pixels(&raster_budget)
        .unwrap();
        let mut rasters = rrrah_cache::RasterRamCache::new(rrrah_cache::CacheLimits::bytes(16));
        assert!(rasters.insert(1u8, raster));
        let lease = rasters.get_lease(&1).unwrap();
        let mut raster_renderer = rrrah_gpu::RasterRenderer::new_linear_hdr(&device);
        raster_renderer.upload(&device, &queue, &lease).unwrap();
        assert!(rasters.set_limits(rrrah_cache::CacheLimits::bytes(0)).is_none());
        let submission = submit_upload_lease(&queue, &completions, lease);
        device
            .poll(wgpu::PollType::Wait {
                submission_index: Some(submission),
                timeout: None,
            })
            .unwrap();
        assert!(rasters.set_limits(rrrah_cache::CacheLimits::bytes(0)).is_some());
        assert_eq!(raster_budget.used(), 0);
        for name in [
            "triangle.stl",
            "obj-shared-metadata.obj",
            "ply-64-le.ply",
            "off-attributes-binary.off",
        ] {
            let model_budget = rrrah_cache::MemoryBudget::new(64 * 1024);
            let mut request = DecodeRequest::new(
                PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                    .join("../../tests/fixtures/models")
                    .join(name),
            );
            request.memory_budget = Some(model_budget.clone());
            let model = rrrah_decode::decode_model(&request).unwrap();
            let mut models =
                rrrah_cache::LeaseCache::new(rrrah_cache::CacheLimits::bytes(model.capacity_bytes()));
            let bytes = model.capacity_bytes();
            models.insert(1u8, model, bytes).unwrap();
            let lease = models.get(&1).unwrap();
            let mut renderer = rrrah_gpu::ModelRenderer::new(&device, wgpu::TextureFormat::Rgba8UnormSrgb);
            renderer.upload(&device, lease.triangles()).unwrap();
            assert!(models.set_limits(rrrah_cache::CacheLimits::bytes(0)).is_none());
            let submission = submit_upload_lease(&queue, &completions, lease);
            device
                .poll(wgpu::PollType::Wait {
                    submission_index: Some(submission),
                    timeout: None,
                })
                .unwrap();
            assert!(models.set_limits(rrrah_cache::CacheLimits::bytes(0)).is_some());
            assert_eq!(model_budget.used(), 0, "retained mesh: {name}");
        }
    }

    #[test]
    #[ignore = "requires local EOS R8 tests/IMG_9043.CR3 fixture"]
    fn source_kind_pressure_preserves_visible_then_releases_unpinned_raw() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let path = root.join("tests/IMG_9043.CR3");
        let output = rrrah_decode::decode_file(&path).unwrap();
        let bytes = output.mosaic.pixels.capacity_bytes();
        let budget = rrrah_cache::MemoryBudget::new(bytes);
        let mosaic = output.mosaic.try_manage_pixels(&budget).unwrap();
        let fingerprint = SourceFingerprint::from_path(&path).unwrap();
        let key = CacheKey::for_mosaic(&fingerprint, 0);
        let mut cache = MosaicRamCache::new(bytes);
        assert!(cache.insert_visible(key, mosaic));
        let mut request = DecodeRequest::new(root.join("tests/fixtures/raster/pattern.tif"));
        request.memory_budget = Some(budget.clone());
        assert!(matches!(
            image_source_kind_with_pressure(&request, &mut Some(&mut cache), None, None),
            Err(rrrah_decode::RasterDecodeError::Source(
                rrrah_decode::DecodeError::Memory(_)
            ))
        ));
        assert_eq!(cache.len(), 1);
        assert_eq!(budget.used(), bytes);
        cache.mark_visible(&CacheKey::for_mosaic(&fingerprint, 1));
        request.memory_budget = Some(rrrah_core::MemoryBudget::new(0));
        assert!(image_source_kind_with_pressure(&request, &mut Some(&mut cache), None, None).is_err());
        assert_eq!(cache.len(), 1);
        assert_eq!(budget.used(), bytes);
        request.memory_budget = Some(budget.clone());
        assert_eq!(
            image_source_kind_with_pressure(&request, &mut Some(&mut cache), None, None).unwrap(),
            rrrah_decode::ImageSourceKind::Raster
        );
        assert!(cache.is_empty());
        assert_eq!(budget.used(), 0);
    }

    #[test]
    fn cli_prefetch_counts_support_defaults_aliases_and_disabled_sides() {
        let defaults = Cli::try_parse_from(["rrrah"]).unwrap();
        assert_eq!((defaults.prefetch_behind, defaults.prefetch_ahead), (2, 5));
        let configured =
            Cli::try_parse_from(["rrrah", "--prefetch-previous", "0", "--prefetch-next", "12"]).unwrap();
        assert_eq!((configured.prefetch_behind, configured.prefetch_ahead), (0, 12));
        assert!(Cli::try_parse_from(["rrrah", "--prefetch-ahead", "-1"]).is_err());
    }

    #[test]
    fn cli_ram_limits_are_independent_and_optional() {
        let defaults = Cli::try_parse_from(["rrrah"]).unwrap();
        assert!(defaults.ram_cache_count.is_none());
        assert!(defaults.ram_cache_ttl_secs.is_none());
        let configured = Cli::try_parse_from([
            "rrrah",
            "--ram-cache-mb",
            "512",
            "--ram-cache-count",
            "8",
            "--ram-cache-ttl-secs",
            "30",
        ])
        .unwrap();
        assert_eq!(configured.ram_cache_mb, Some(512));
        assert_eq!(configured.ram_cache_count, Some(8));
        assert_eq!(configured.ram_cache_ttl_secs, Some(30));
        assert!(Cli::try_parse_from(["rrrah", "--ram-cache-count", "-1"]).is_err());
    }

    #[test]
    fn off_discovery_inspection_and_frame_boundary() {
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../tests/fixtures/models/off-attributes-binary.off");
        assert!(is_supported_image(&path));
        assert!(is_supported_image(Path::new("mesh.OFF")));
        assert!(gallery::scan_folder(path.parent().unwrap()).contains(&path));
        inspect(&path, None, true, 0, None).unwrap();
        assert!(inspect(&path, None, true, 1, None).is_err());
        let model = rrrah_decode::decode_model(&DecodeRequest::new(&path)).unwrap();
        assert_eq!(model.format_name(), "OFF");
        assert_eq!(model.triangle_count(), 1);
    }

    #[test]
    fn ply_discovery_and_float64_inspection_are_connected() {
        let path =
            Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/models/ply-offset64-be.ply");
        assert!(is_supported_image(&path));
        assert!(gallery::scan_folder(path.parent().unwrap()).contains(&path));
        inspect(&path, None, true, 0, None).unwrap();
        let model = rrrah_decode::decode_model(&DecodeRequest::new(&path)).unwrap();
        assert_eq!(model.format_name(), "PLY");
        assert_eq!(
            model.triangles().next().unwrap()[1][0] - model.triangles().next().unwrap()[0][0],
            2.
        );
        assert!(inspect(&path, None, true, 1, None).is_err());
        assert!(
            inspect(
                &path,
                None,
                true,
                0,
                Some(rrrah_decode::ScalarWindow::new(0., 1.).unwrap())
            )
            .is_err()
        );
    }

    #[test]
    fn obj_uses_common_model_discovery_and_inspection() {
        let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/models/concave.obj");
        assert!(is_supported_image(&path));
        assert!(gallery::scan_folder(path.parent().unwrap()).contains(&path));
        inspect(&path, None, true, 0, None).unwrap();
        assert!(inspect(&path, None, true, 1, None).is_err());
        let model = rrrah_decode::decode_model(&DecodeRequest::new(&path)).unwrap();
        assert_eq!(model.format_name(), "OBJ");
        assert_eq!(model.triangle_count(), 4);
        assert_eq!(model.triangles().len(), 4);
    }

    #[test]
    fn stl_discovery_inspection_and_thumbnail_boundary() {
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::temp_dir().join(format!("rrrah-model-app-{}-{stamp}", std::process::id()));
        std::fs::create_dir(&root).unwrap();
        let path = root.join("triangle.STL");
        std::fs::write(&path, b"solid triangle\nfacet normal 0 0 1\nouter loop\nvertex 0 0 0\nvertex 1 0 0\nvertex 0 1 0\nendloop\nendfacet\nendsolid triangle\n").unwrap();
        assert!(is_supported_image(&path));
        assert!(gallery::scan_folder(&root).contains(&path));
        inspect(&path, None, true, 0, None).unwrap();
        assert!(inspect(&path, None, true, 1, None).is_err());
        assert!(
            inspect(
                &path,
                None,
                true,
                0,
                Some(rrrah_decode::ScalarWindow::new(0., 1.).unwrap())
            )
            .is_err()
        );
        std::fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn cancelled_thumbnail_viewport_does_not_open_source_or_allocate() {
        let budget = rrrah_core::MemoryBudget::new(1024);
        let loader = thumbnail_loader_with_cancel(
            Arc::new(DecodeGate::new()),
            Some(budget.clone()),
            UntaggedColor::Strict.into(),
        );
        let token = GenerationToken::new(Arc::new(AtomicU64::new(2)), 1);
        assert!(
            loader(
                gallery::ThumbnailJob {
                    index: 0,
                    source: PathBuf::from("missing-viewport-thumbnail.png"),
                    edge: 128,
                },
                token
            )
            .is_none()
        );
        assert_eq!(budget.used(), 0);
        assert_eq!(budget.peak(), 0);
    }

    #[test]
    #[ignore = "requires external Sony DSC-F828 SRF fixture"]
    fn raw_thumbnail_source_and_preview_share_managed_root() {
        let job = gallery::ThumbnailJob {
            index: 0,
            source: PathBuf::from(std::env::var("RRRAH_SRF_SOURCE").unwrap()),
            edge: 128,
        };
        let denied = rrrah_core::MemoryBudget::new(32 * 1024 * 1024);
        let loader = thumbnail_loader(Arc::new(DecodeGate::new()), Some(denied.clone()));
        assert!(loader(job.clone()).is_none());
        assert_eq!(denied.used(), 0);
        let budget = rrrah_core::MemoryBudget::new(64 * 1024 * 1024);
        let loader = thumbnail_loader(Arc::new(DecodeGate::new()), Some(budget.clone()));
        let ready = loader(job).unwrap();
        assert_eq!((ready.width, ready.height), (128, 96));
        assert_eq!(budget.used(), 128 * 96 * 4);
        assert!(budget.peak() >= 17393344 + 16531200);
        drop(ready);
        assert_eq!(budget.used(), 0);
    }

    #[test]
    fn raster_thumbnail_uses_shared_budget_and_last_owner_credit() {
        let source =
            Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/raster/pattern.profiled.png");
        let job = gallery::ThumbnailJob {
            index: 0,
            source,
            edge: 16,
        };
        let denied = rrrah_core::MemoryBudget::new(0);
        let loader = thumbnail_loader(Arc::new(DecodeGate::new()), Some(denied.clone()));
        assert!(loader(job.clone()).is_none());
        assert_eq!(denied.used(), 0);
        let budget = rrrah_core::MemoryBudget::new(1024 * 1024);
        let loader = thumbnail_loader(Arc::new(DecodeGate::new()), Some(budget.clone()));
        let ready = loader(job).unwrap();
        assert!(ready.pixels.is_managed());
        assert_eq!(budget.used(), 1024);
        let held = ready.clone();
        drop(ready);
        assert_eq!(budget.used(), 1024);
        drop(held);
        assert_eq!(budget.used(), 0);
    }

    #[test]
    fn pict_filmstrip_uses_explicit_viewer_policy_and_releases_last_owner() {
        let budget = rrrah_core::MemoryBudget::new(4096);
        let path =
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/pict/red-rectangle-v2.pict");
        let job = || gallery::ThumbnailJob {
            index: 7,
            source: path.clone(),
            edge: 3,
        };
        let token = || GenerationToken::new(Arc::new(AtomicU64::new(0)), 0);
        let gate = Arc::new(DecodeGate::new());
        let strict =
            thumbnail_loader_with_cancel(gate.clone(), Some(budget.clone()), UntaggedColor::Strict.into());
        assert!(strict(job(), token()).is_none());
        assert_eq!(budget.used(), 0);
        let assumed = thumbnail_loader_with_cancel(gate, Some(budget.clone()), UntaggedColor::Srgb.into());
        let thumbnail = assumed(job(), token()).unwrap();
        assert_eq!((thumbnail.width, thumbnail.height), (3, 2));
        assert_eq!(&thumbnail.pixels[..4], &[36, 36, 36, 255]);
        assert_eq!(&thumbnail.pixels[4..8], &[255, 0, 0, 255]);
        assert_eq!(budget.used(), 24);
        let held = thumbnail.pixels.clone();
        drop(thumbnail);
        assert_eq!(budget.used(), 24);
        drop(held);
        assert_eq!(budget.used(), 0);
    }
    #[test]
    fn raster_folder_thumbnail_applies_icc_and_composites_alpha() {
        let source =
            Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/raster/pattern.profiled.png");
        let loader = thumbnail_loader(Arc::new(DecodeGate::new()), None);
        let thumbnail = loader(gallery::ThumbnailJob {
            index: 7,
            source,
            edge: 16,
        })
        .unwrap();
        assert_eq!((thumbnail.index, thumbnail.width, thumbnail.height), (7, 16, 16));
        assert_eq!(&thumbnail.pixels[..4], &[36, 36, 36, 255]);
        let last = &thumbnail.pixels[thumbnail.pixels.len() - 4..];
        for (&actual, expected) in last.iter().zip([255_u8, 255, 238, 255]) {
            assert!(actual.abs_diff(expected) <= 2);
        }
    }

    #[test]
    fn wal_thumbnail_rereads_palette_and_composites_transparency() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir(dir.path().join("textures")).unwrap();
        std::fs::create_dir(dir.path().join("pics")).unwrap();
        let source = dir.path().join("textures/grid.wal");
        let palette = dir.path().join("pics/colormap.pcx");
        std::fs::write(
            &source,
            include_bytes!("../../../tests/fixtures/raster/palette-grid.wal"),
        )
        .unwrap();
        let mut pcx = include_bytes!("../../../tests/fixtures/raster/wal-colormap.pcx").to_vec();
        std::fs::write(&palette, &pcx).unwrap();
        let loader = thumbnail_loader(Arc::new(DecodeGate::new()), None);
        let job = gallery::ThumbnailJob {
            index: 0,
            source,
            edge: 16,
        };
        let first = loader(job.clone()).unwrap();
        assert_eq!(&first.pixels[..4], &[0, 0, 0, 255]);
        let at = pcx.len() - 768;
        pcx[at..at + 3].copy_from_slice(&[255, 0, 0]);
        std::fs::write(&palette, pcx).unwrap();
        let second = loader(job).unwrap();
        assert_eq!(&second.pixels[..4], &[255, 0, 0, 255]);
        assert_eq!(&second.pixels[second.pixels.len() - 4..], &[36, 36, 36, 255]);
    }

    #[test]
    fn navigation_stays_within_decoded_container_bounds() {
        assert_eq!(next_image_index(0, 2, 1), Some(1));
        assert_eq!(next_image_index(1, 2, -1), Some(0));
        for (index, count, dir) in [
            (0, 2, -1),
            (1, 2, 1),
            (0, 1, 1),
            (0, 0, 1),
            (usize::MAX, usize::MAX, 1),
        ] {
            assert_eq!(next_image_index(index, count, dir), None);
        }
    }

    #[test]
    fn selected_raster_reaches_display_preparation() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/raster");
        let mut request = DecodeRequest::new(root.join("wad3-textures.wad"));
        let (first, _) = load_raster_for_display(&request, None).unwrap();
        assert_eq!((first.width(), first.height()), (16, 16));
        assert_eq!((first.image_index(), first.image_count()), (0, 2));
        request.image_index = 1;
        let (second, assumed) = load_raster_for_display(&request, None).unwrap();
        assert!(assumed);
        assert_eq!((second.width(), second.height()), (32, 16));
        assert_eq!((second.image_index(), second.image_count()), (1, 2));
        let rrrah_core::RasterPixels::Rgba32Float(pixels) = second.pixels() else {
            panic!()
        };
        assert_eq!(pixels[pixels.len() - 1], 0.0);
        request.image_index = 2;
        assert!(load_raster_for_display(&request, None).is_err());
        request.path = root.join("two-pages.dcx");
        request.image_index = 0;
        let (a, _) = load_raster_for_display(&request, None).unwrap();
        request.image_index = 1;
        let (b, _) = load_raster_for_display(&request, None).unwrap();
        let (rrrah_core::RasterPixels::Rgba32Float(a), rrrah_core::RasterPixels::Rgba32Float(b)) =
            (a.pixels(), b.pixels())
        else {
            panic!()
        };
        assert_ne!(a.as_slice(), b.as_slice());
        request.image_index = 2;
        assert!(load_raster_for_display(&request, None).is_err());
        request.path = root.join("apng-compose-1-2.apng");
        request.image_index = 2;
        let (frame, assumed) = load_raster_for_display(&request, None).unwrap();
        assert!(!assumed);
        assert_eq!((frame.width(), frame.height()), (5, 3));
        assert_eq!((frame.image_index(), frame.image_count()), (2, 4));
        let rrrah_core::RasterPixels::Rgba32Float(values) = frame.pixels() else {
            panic!()
        };
        assert_eq!(&values[..4], &[1., 0., 0., 1.]);
        assert_eq!(&values[12..16], &[0., 0., 1., 1.]);
        request.path = root.join("gif-animation-disposal-3.gif");
        request.image_index = 3;
        let (frame, assumed) = load_raster_for_display(&request, None).unwrap();
        assert!(!assumed);
        assert_eq!((frame.image_index(), frame.image_count()), (3, 4));
        let rrrah_core::RasterPixels::Rgba32Float(values) = frame.pixels() else {
            panic!()
        };
        assert_eq!(&values[..4], &[1., 0., 0., 1.]);
        assert_eq!(&values[8..12], &[0., 0., 1., 1.]);
        request.path = root.join("webp-animation-0.webp");
        request.image_index = 1;
        let (frame, assumed) = load_raster_for_display(&request, None).unwrap();
        assert!(!assumed);
        assert_eq!((frame.image_index(), frame.image_count()), (1, 4));
        let rrrah_core::RasterPixels::Rgba32Float(values) = frame.pixels() else {
            panic!()
        };
        assert_eq!(&values[..4], &[1., 0., 0., 1.]);
        assert!((values[8] - 127. / 255.).abs() < 1e-7);
        assert!((values[9] - 128. / 255.).abs() < 1e-7);
        assert_eq!(values[11], 1.);
        request.path = root.join("jng-RGB-16-0.jng");
        request.image_index = 0;
        let (frame, assumed) = load_raster_for_display(&request, None).unwrap();
        assert!(!assumed);
        assert_eq!((frame.width(), frame.height()), (8, 3));
        let rrrah_core::RasterPixels::Rgba32Float(values) = frame.pixels() else {
            panic!()
        };
        assert_eq!(values[3], 32768. / 65535.);
        request.path = root.join("jls-ffmpeg-16-1-0-0.jls");
        request.image_index = 0;
        let (frame, assumed) = load_raster_for_display(&request, None).unwrap();
        assert!(assumed);
        assert_eq!((frame.width(), frame.height()), (9, 5));
        let rrrah_core::RasterPixels::Rgba32Float(values) = frame.pixels() else {
            panic!()
        };
        assert!(values[0] > 0.2 && values[0] < 0.22);
        assert_eq!(&values[..4], &[values[0], values[0], values[0], 1.]);
        assert!((values[4] - 19. / 65535. / 12.92).abs() < 1e-9);
        request.path = root.join("ase-profile-linear.aseprite");
        request.image_index = 2;
        let (frame, assumed) = load_raster_for_display(&request, None).unwrap();
        assert!(!assumed);
        assert_eq!((frame.image_index(), frame.image_count()), (2, 3));
        let rrrah_core::RasterPixels::Rgba32Float(values) = frame.pixels() else {
            panic!()
        };
        assert_eq!(&values[..4], &[80. / 255., 87. / 255., 180. / 255., 128. / 255.]);
        request.path = root.join("jxr-float32-4.jxr");
        request.image_index = 0;
        let (frame, assumed) = load_raster_for_display(&request, None).unwrap();
        assert!(!assumed);
        assert_eq!((frame.width(), frame.height()), (5, 3));
        let rrrah_core::RasterPixels::Rgba32Float(values) = frame.pixels() else {
            panic!()
        };
        assert_eq!(&values[..4], &[2., -0.5, 0.125, 0.]);
        assert_eq!(values[7], 0.25);
        request.path = root.join("ktx2-mips-109-3-lu.ktx2");
        request.image_index = 2;
        let (mip, assumed) = load_raster_for_display(&request, None).unwrap();
        assert!(!assumed);
        assert_eq!((mip.width(), mip.height()), (2, 1));
        assert_eq!((mip.image_index(), mip.image_count()), (2, 4));
        let rrrah_core::RasterPixels::Rgba32Float(values) = mip.pixels() else {
            panic!()
        };
        assert_eq!(&values[..4], &[-0.375, 2., 2.0625, 0.0625]);
        request.path = root.join("ktx-mips-4ch-32-1.ktx");
        request.image_index = 2;
        let (mip, assumed) = load_raster_for_display(&request, None).unwrap();
        assert!(!assumed);
        assert_eq!((mip.width(), mip.height()), (2, 1));
        assert_eq!((mip.image_index(), mip.image_count()), (2, 4));
        let rrrah_core::RasterPixels::Rgba32Float(values) = mip.pixels() else {
            panic!()
        };
        assert_eq!(&values[..4], &[-0.375, 2., 2.0625, 0.0625]);
        request.path = root.join("dds-mips-rgba32-srgb.dds");
        request.image_index = 2;
        let (mip, assumed) = load_raster_for_display(&request, None).unwrap();
        assert!(!assumed);
        assert_eq!((mip.width(), mip.height()), (2, 1));
        assert_eq!((mip.image_index(), mip.image_count()), (2, 4));
        let rrrah_core::RasterPixels::Rgba32Float(values) = mip.pixels() else {
            panic!()
        };
        assert_eq!(values[3], 140. / 255.);
        request.path = root.join("pvr-rgba16-be.pvr");
        request.image_index = 1;
        let (mip, assumed) = load_raster_for_display(&request, None).unwrap();
        assert!(!assumed);
        assert_eq!((mip.width(), mip.height()), (2, 1));
        assert_eq!((mip.image_index(), mip.image_count()), (1, 3));
        let rrrah_core::RasterPixels::Rgba32Float(values) = mip.pixels() else {
            panic!()
        };
        assert!((values[0] - (2.0 / 65535.0 / 12.92)).abs() < 1e-9);
        assert_eq!(values[3], 32768.0 / 65535.0);
        let cli = Cli::try_parse_from(["rrrah", "--image-index", "1", "--inspect", "two-pages.dcx"]).unwrap();
        assert_eq!(cli.image_index, 1);
    }

    #[test]
    fn scientific_window_reaches_selected_plane_and_cli_bounds_are_validated() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/raster");
        let window = rrrah_decode::ScalarWindow::new(-32768., 32767.).unwrap();
        for name in ["nrrd-i16-raw-big.nrrd", "mrc-i16-big.mrc", "fits-i16.fits"] {
            let mut request = DecodeRequest::new(root.join(name));
            request.image_index = 1;
            assert!(load_raster_for_display(&request, None).is_err());
            let (raster, assumed) = load_raster_for_display(&request, Some(window)).unwrap();
            assert!(!assumed);
            assert_eq!((raster.image_index(), raster.image_count()), (1, 2));
            assert_eq!(raster.color_space(), &rrrah_core::RasterColorSpace::LinearSrgb);
            let expected = rrrah_decode::decode_raster(&request).unwrap();
            let (rrrah_core::RasterPixels::Rgba32Float(raw), rrrah_core::RasterPixels::Rgba32Float(display)) =
                (expected.pixels(), raster.pixels())
            else {
                panic!()
            };
            for (raw, display) in raw.chunks_exact(4).zip(display.chunks_exact(4)) {
                let gray = ((raw[0] + 32768.) / 65535.).clamp(0., 1.);
                assert_eq!(display, &[gray, gray, gray, 1.]);
            }
        }
        let cli = Cli::try_parse_from([
            "rrrah",
            "--window",
            "-32768",
            "32767",
            "--image-index",
            "1",
            "volume.mrc",
        ])
        .unwrap();
        assert_eq!(cli.scalar_window().unwrap(), Some(window));
        for bounds in [["1", "1"], ["2", "1"], ["NaN", "2"], ["0", "inf"]] {
            let cli = Cli::try_parse_from(["rrrah", "--window", bounds[0], bounds[1], "volume.mrc"]).unwrap();
            assert!(cli.scalar_window().is_err());
        }
        assert!(Cli::try_parse_from(["rrrah", "--window", "1"]).is_err());
    }
    #[test]
    fn keyboard_windows_shift_center_and_resize_without_invalid_ranges() {
        let window = rrrah_decode::ScalarWindow::new(-10., 10.).unwrap();
        let shifted = adjusted_scalar_window(window, 0.1, 1.).unwrap();
        assert_eq!((shifted.minimum(), shifted.maximum()), (-8., 12.));
        let wider = adjusted_scalar_window(window, 0., 1.25).unwrap();
        assert_eq!((wider.minimum(), wider.maximum()), (-12.5, 12.5));
        assert_eq!(adjusted_scalar_window(wider, 0., 0.8), Some(window));
        assert!(adjusted_scalar_window(window, 0., 0.).is_none());
        assert!(adjusted_scalar_window(window, f64::INFINITY, 1.).is_none());
        assert!(adjusted_scalar_window(window, 0., f64::MAX).is_none());
    }

    #[test]
    fn equivalent_foreground_requests_join_pending_and_active_generation() {
        let root = std::env::temp_dir().join(format!(
            "rrrah-flight-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir(&root).unwrap();
        let path = root.join("source.png");
        std::fs::write(&path, b"source-v1").unwrap();
        let (tx, worker_rx) = bounded(1);
        let mut loader = ForegroundLoader {
            decode_first: true,
            automatic_policy: false,
            flight: Arc::new(std::sync::Mutex::new(None)),
            tx,
            pending: worker_rx.clone(),
            generation: Arc::new(AtomicU64::new(0)),
            decode_gate: Arc::new(DecodeGate::new()),
            telemetry: Arc::new(CacheTelemetry::new(true, 1024)),
            development: None,
        };
        let first = loader.submit(path.clone()).unwrap();
        assert_eq!(loader.submit(path.clone()).unwrap(), first);
        let active = worker_rx.try_recv().unwrap();
        let token = GenerationToken::new(Arc::clone(&loader.generation), first);
        let gate_generation = loader.decode_gate.speculative_generation();
        let gate_before = gate_generation.load(Ordering::Acquire);
        assert_eq!(loader.submit(path.clone()).unwrap(), first);
        assert!(!token.is_cancelled());
        assert!(worker_rx.try_recv().is_err());
        assert_eq!(gate_generation.load(Ordering::Acquire), gate_before);
        thread::scope(|scope| {
            let left = scope.spawn(|| loader.submit(path.clone()).unwrap());
            let right = scope.spawn(|| loader.submit(path.clone()).unwrap());
            assert_eq!(left.join().unwrap(), first);
            assert_eq!(right.join().unwrap(), first);
        });
        assert!(!token.is_cancelled());
        assert!(worker_rx.try_recv().is_err());

        // Completing an older job cannot clear a newer job's identity.
        let selected = loader.submit_image(path.clone(), 1).unwrap();
        assert_ne!(selected, first);
        assert!(token.is_cancelled());
        finish_foreground_flight(&loader.flight, first);
        assert_eq!(loader.submit_image(path.clone(), 1).unwrap(), selected);
        let window = rrrah_decode::ScalarWindow::new(0., 1.).unwrap();
        let scalar = loader
            .submit_image_with_window(path.clone(), 1, Some(window))
            .unwrap();
        assert_ne!(scalar, selected);
        loader.development = Some(rrrah_core::develop::DevelopOptions::default());
        let developed = loader
            .submit_image_with_window(path.clone(), 1, Some(window))
            .unwrap();
        assert_ne!(developed, scalar);
        loader.development.as_mut().unwrap().recover_highlights = false;
        let recipe = loader
            .submit_image_with_window(path.clone(), 1, Some(window))
            .unwrap();
        assert_ne!(recipe, developed);
        std::fs::write(&path, b"source-v2-changed").unwrap();
        let changed = loader
            .submit_image_with_window(path.clone(), 1, Some(window))
            .unwrap();
        assert_ne!(changed, recipe);
        finish_foreground_flight(&loader.flight, changed);
        let retry = loader
            .submit_image_with_window(path.clone(), 1, Some(window))
            .unwrap();
        assert_ne!(retry, changed);
        assert_eq!(
            loader
                .submit_image_with_window(path.clone(), 1, Some(window))
                .unwrap(),
            retry
        );
        std::fs::remove_file(&path).unwrap();
        let missing = loader.submit(path.clone()).unwrap();
        assert_ne!(loader.submit(path).unwrap(), missing);
        drop(active);
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn dropping_foreground_loader_invalidates_an_unsuperseded_active_token() {
        let (tx, worker_rx) = bounded(1);
        let loader = ForegroundLoader {
            decode_first: true,
            automatic_policy: false,
            flight: Arc::new(std::sync::Mutex::new(None)),
            tx,
            pending: worker_rx.clone(),
            generation: Arc::new(AtomicU64::new(0)),
            decode_gate: Arc::new(DecodeGate::new()),
            telemetry: Arc::new(CacheTelemetry::new(true, 1024)),
            development: None,
        };
        loader.submit(PathBuf::from("shutdown-only-active.cr3")).unwrap();
        let active = worker_rx.try_recv().unwrap();
        let token = GenerationToken::new(Arc::clone(&loader.generation), active.generation);
        assert!(!token.is_cancelled());
        drop(loader);
        assert!(token.is_cancelled());
        assert!(matches!(worker_rx.try_recv(), Err(TryRecvError::Disconnected)));
        drop(active);
    }

    #[test]
    fn dropping_foreground_loader_cancels_active_and_drains_pending_work() {
        let (tx, worker_rx) = bounded(1);
        let gate = Arc::new(DecodeGate::new());
        let loader = ForegroundLoader {
            decode_first: true,
            automatic_policy: false,
            flight: Arc::new(std::sync::Mutex::new(None)),
            tx,
            pending: worker_rx.clone(),
            generation: Arc::new(AtomicU64::new(0)),
            decode_gate: Arc::clone(&gate),
            telemetry: Arc::new(CacheTelemetry::new(true, 1024)),
            development: None,
        };
        loader.submit(PathBuf::from("shutdown-active.cr3")).unwrap();
        let active = worker_rx.try_recv().unwrap();
        let active_token = GenerationToken::new(Arc::clone(&loader.generation), active.generation);
        let permit = active
            .foreground
            .acquire_decode(|| active_token.is_cancelled())
            .unwrap();
        assert!(!active_token.is_cancelled());
        loader.submit(PathBuf::from("shutdown-pending.cr3")).unwrap();
        let pending_token = GenerationToken::new(Arc::clone(&loader.generation), loader.current_generation());
        let flight = Arc::clone(&loader.flight);
        assert!(!pending_token.is_cancelled());
        drop(loader);
        assert!(active_token.is_cancelled());
        assert!(pending_token.is_cancelled());
        assert!(flight.lock().unwrap().is_none());
        assert!(matches!(worker_rx.try_recv(), Err(TryRecvError::Disconnected)));
        // The active permit remains independently owned until its decoder exits.
        drop(permit);
        drop(active);
        assert!(gate.acquire_prefetch(|| false).is_some());
    }

    #[test]
    fn rapid_submissions_keep_only_the_latest_pending_request() {
        let (tx, worker_rx) = bounded(1);
        let loader = ForegroundLoader {
            decode_first: true,
            automatic_policy: false,
            flight: Arc::new(std::sync::Mutex::new(None)),
            tx,
            pending: worker_rx.clone(),
            generation: Arc::new(AtomicU64::new(0)),
            decode_gate: Arc::new(DecodeGate::new()),
            telemetry: Arc::new(CacheTelemetry::new(true, 1024)),
            development: None,
        };

        loader.submit_initial(PathBuf::from("0.cr3")).unwrap();
        let active = worker_rx.try_recv().unwrap();
        assert!(active.decode_first);
        let active_token = GenerationToken::new(Arc::clone(&loader.generation), active.generation);

        loader.submit(PathBuf::from("1.cr3")).unwrap();
        loader.submit(PathBuf::from("2.cr3")).unwrap();

        assert!(active_token.is_cancelled());
        let pending = worker_rx.try_recv().unwrap();
        assert!(pending.decode_first);
        assert_eq!(pending.path, PathBuf::from("2.cr3"));
        assert_eq!(pending.generation, loader.current_generation());
        assert!(worker_rx.try_recv().is_err());
        loader.submit_image(PathBuf::from("pages.wad"), 1).unwrap();
        loader.submit_image(PathBuf::from("pages.wad"), 2).unwrap();
        let selected = worker_rx.try_recv().unwrap();
        assert_eq!(selected.image_index, 2);
        let window = rrrah_decode::ScalarWindow::new(-10., 100.).unwrap();
        loader
            .submit_image_with_window(PathBuf::from("volume.mrc"), 1, Some(window))
            .unwrap();
        let scalar = worker_rx.try_recv().unwrap();
        assert_eq!(scalar.image_index, 1);
        assert_eq!(scalar.scalar_window, Some(window));
        loader.submit(PathBuf::from("still.png")).unwrap();
        let ordinary = worker_rx.try_recv().unwrap();
        assert_eq!(ordinary.image_index, 0);
        assert_eq!(ordinary.scalar_window, None);
    }
    #[test]
    fn swap_restore_budget_is_independent_and_accepts_zero() {
        let defaults = Cli::try_parse_from(["rrrah"]).unwrap();
        assert_eq!(defaults.swap_restore_mb, 512);
        assert_eq!(defaults.managed_memory_mb, None);
        let zero = Cli::try_parse_from(["rrrah", "--managed-memory-mb", "0"]).unwrap();
        assert_eq!(zero.managed_memory_mb, Some(0));
        let cli = Cli::try_parse_from([
            "rrrah",
            "--ram-cache-mb",
            "1024",
            "--swap-mb",
            "4096",
            "--swap-queue-mb",
            "64",
            "--swap-restore-mb",
            "0",
        ])
        .unwrap();
        assert_eq!(cli.ram_cache_mb, Some(1024));
        assert_eq!(cli.swap_mb, 4096);
        assert_eq!(cli.swap_queue_mb, 64);
        assert_eq!(cli.swap_restore_mb, 0);
        assert!(Cli::try_parse_from(["rrrah", "--swap-restore-mb", "-1"]).is_err());
    }
    #[test]
    fn disk_cli_limits_are_independent_and_configure_real_cache() {
        let cli = Cli::try_parse_from([
            "rrrah",
            "--disk-cache-mb",
            "16",
            "--disk-cache-count",
            "0",
            "--disk-cache-ttl-secs",
            "30",
        ])
        .unwrap();
        let limits = cli.disk_limits();
        assert_eq!(limits.max_bytes, 16 * 1024 * 1024);
        assert_eq!(limits.max_entries, Some(0));
        assert_eq!(limits.ttl, Some(Duration::from_secs(30)));
        let parent = tempfile::tempdir().unwrap();
        let cache = disk_cache_with_limits(parent.path().to_path_buf(), limits);
        assert_eq!(cache.max_bytes(), limits.max_bytes);
        let defaults = Cli::try_parse_from(["rrrah"]).unwrap().disk_limits();
        assert_eq!(defaults.max_bytes, DEFAULT_MAX_DISK_CACHE_BYTES);
        assert!(defaults.max_entries.is_none());
        assert!(defaults.ttl.is_none());
        assert!(Cli::try_parse_from(["rrrah", "--disk-cache-count", "-1"]).is_err());
    }
    #[test]
    fn raster_cache_hits_share_pixels_and_separate_animation_indices() {
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../tests/fixtures/raster/gif-animation-disposal-3.gif");
        let mut cache = RasterDisplayCache::new(rrrah_cache::CacheLimits::bytes(1024 * 1024));
        let mut request = DecodeRequest::new(path);
        let (first, assumed) = load_cached_raster_for_display(&request, None, &mut cache).unwrap();
        let (hit, hit_assumed) = load_cached_raster_for_display(&request, None, &mut cache).unwrap();
        assert_eq!(assumed, hit_assumed);
        let (rrrah_core::RasterPixels::Rgba32Float(a), rrrah_core::RasterPixels::Rgba32Float(b)) =
            (first.pixels(), hit.pixels())
        else {
            panic!()
        };
        assert!(a.ptr_eq(b));
        request.image_index = 3;
        let (other, _) = load_cached_raster_for_display(&request, None, &mut cache).unwrap();
        assert_eq!(other.image_index(), 3);
        assert_eq!(cache.len(), 2);
    }
    #[test]
    fn raster_limits_are_independent_of_raw_limits_and_accept_zero() {
        let defaults = Cli::try_parse_from(["rrrah"]).unwrap().raster_limits();
        assert_eq!(defaults.max_bytes, 512 * 1024 * 1024);
        assert!(defaults.max_entries.is_none());
        assert!(defaults.ttl.is_none());
        let cli = Cli::try_parse_from([
            "rrrah",
            "--ram-cache-mb",
            "2048",
            "--raster-cache-mb",
            "64",
            "--raster-cache-count",
            "3",
            "--raster-cache-ttl-secs",
            "20",
        ])
        .unwrap();
        let limits = cli.raster_limits();
        assert_eq!(limits.max_bytes, 64 * 1024 * 1024);
        assert_eq!(limits.max_entries, Some(3));
        assert_eq!(limits.ttl, Some(Duration::from_secs(20)));
        assert_eq!(cli.ram_cache_mb, Some(2048));
        assert_eq!(
            Cli::try_parse_from(["rrrah", "--raster-cache-mb", "0"])
                .unwrap()
                .raster_limits()
                .max_bytes,
            0
        );
        assert!(Cli::try_parse_from(["rrrah", "--raster-cache-count", "-1"]).is_err());
    }
    #[test]
    fn raster_cache_does_not_hide_replaced_source_or_merge_scalar_windows() {
        let fixtures = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/raster");
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("changing.gif");
        std::fs::copy(fixtures.join("gif-animation-disposal-3.gif"), &path).unwrap();
        let request = DecodeRequest::new(&path);
        let mut cache = RasterDisplayCache::new(rrrah_cache::CacheLimits::bytes(1024 * 1024));
        assert!(load_cached_raster_for_display(&request, None, &mut cache).is_ok());
        std::fs::write(&path, b"replaced invalid image").unwrap();
        assert!(load_cached_raster_for_display(&request, None, &mut cache).is_err());
        let request = DecodeRequest::new(fixtures.join("mrc-i16-big.mrc"));
        let a = rrrah_decode::ScalarWindow::new(-32768., 32767.).unwrap();
        let b = rrrah_decode::ScalarWindow::new(0., 1000.).unwrap();
        let (first, _) = load_cached_raster_for_display(&request, Some(a), &mut cache).unwrap();
        let (second, _) = load_cached_raster_for_display(&request, Some(b), &mut cache).unwrap();
        let (hit, _) = load_cached_raster_for_display(&request, Some(a), &mut cache).unwrap();
        let (
            rrrah_core::RasterPixels::Rgba32Float(first),
            rrrah_core::RasterPixels::Rgba32Float(second),
            rrrah_core::RasterPixels::Rgba32Float(hit),
        ) = (first.pixels(), second.pixels(), hit.pixels())
        else {
            panic!()
        };
        assert!(!first.ptr_eq(second));
        assert!(first.ptr_eq(hit));
        assert_ne!(first, second);
        assert_eq!(cache.len(), 3);
    }
    #[test]
    fn raster_cache_hit_does_not_call_decode_admission_closure() {
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../tests/fixtures/raster/gif-animation-disposal-3.gif");
        let request = DecodeRequest::new(path);
        let mut cache = RasterDisplayCache::new(rrrah_cache::CacheLimits::bytes(1024 * 1024));
        load_cached_raster_for_display(&request, None, &mut cache).unwrap();
        let result = load_cached_raster_with(&request, None, &mut cache, || {
            panic!("RAM hit waited for decoder admission")
        });
        assert!(result.is_ok());
    }
    #[test]
    fn model_cache_hit_shares_geometry_and_bypasses_decoder() {
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../tests/fixtures/models/off-attributes-binary.off");
        let request = DecodeRequest::new(path);
        let mut cache = ModelDisplayCache::new(rrrah_cache::CacheLimits {
            max_bytes: 1024 * 1024,
            max_entries: Some(1),
            ttl: None,
        });
        let first = cache
            .load(&request, true, || {
                rrrah_decode::decode_model(&request).map_err(ModelLoadError::Decode)
            })
            .unwrap();
        let hit = cache
            .load(&request, true, || {
                panic!("model cache hit requested decode admission")
            })
            .unwrap();
        let (rrrah_decode::DecodedModel::Off(a), rrrah_decode::DecodedModel::Off(b)) = (&first, &hit) else {
            panic!()
        };
        assert!(a.ptr_eq(b));
        assert_eq!(cache.entries.resident_weight(), first.capacity_bytes());
    }
    #[test]
    fn model_cache_policy_is_independent_and_expired_hits_decode_again() {
        let cli = Cli::try_parse_from([
            "rrrah",
            "--model-cache-mb",
            "16",
            "--model-cache-count",
            "1",
            "--model-cache-ttl-secs",
            "0",
            "--raster-cache-mb",
            "64",
        ])
        .unwrap();
        let limits = cli.model_limits();
        assert_eq!(limits.max_bytes, 16 * 1024 * 1024);
        assert_eq!(limits.max_entries, Some(1));
        assert_eq!(limits.ttl, Some(Duration::ZERO));
        assert_eq!(cli.raster_limits().max_bytes, 64 * 1024 * 1024);
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../tests/fixtures/models/off-attributes-binary.off");
        let request = DecodeRequest::new(path);
        let decode = || rrrah_decode::decode_model(&request).map_err(ModelLoadError::Decode);
        let mut cache = ModelDisplayCache::new(limits);
        cache.load(&request, true, decode).unwrap();
        let mut called = false;
        cache
            .load(&request, true, || {
                called = true;
                decode()
            })
            .unwrap();
        assert!(called);
        let mut disabled = ModelDisplayCache::new(rrrah_cache::CacheLimits {
            max_bytes: limits.max_bytes,
            max_entries: Some(0),
            ttl: None,
        });
        disabled.load(&request, true, decode).unwrap();
        assert!(disabled.entries.is_empty());
        assert!(Cli::try_parse_from(["rrrah", "--model-cache-count", "-1"]).is_err());
    }
    #[test]
    fn rejected_model_transition_restores_the_visible_pin() {
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../tests/fixtures/models/off-attributes-binary.off");
        let request = DecodeRequest::new(path);
        let model = rrrah_decode::decode_model(&request).unwrap();
        let mut cache = ModelDisplayCache::new(rrrah_cache::CacheLimits {
            max_bytes: model.capacity_bytes(),
            max_entries: Some(1),
            ttl: None,
        });
        cache.load(&request, true, || Ok(model.clone())).unwrap();
        let previous = cache.visible.clone().unwrap();
        let mut another = request.clone();
        another.image_index = 1;
        let oversized = rrrah_decode::DecodedModel::Stl(
            Arc::new(rrrah_decode::StlMesh {
                facets: Vec::with_capacity(10000),
                bounds: None,
            })
            .into(),
        );
        cache.load(&another, true, || Ok(oversized.clone())).unwrap();
        assert_eq!(cache.visible.as_ref(), Some(&previous));
        let mut speculative = previous.clone();
        speculative.4 = 2;
        assert!(
            !cache
                .entries
                .insert(speculative, model.clone(), model.capacity_bytes())
                .is_ok()
        );
        assert!(cache.entries.get(&previous).is_some());
    }
    #[test]
    fn speculative_model_admission_cannot_replace_visible_count_one() {
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../tests/fixtures/models/off-attributes-binary.off");
        let request = DecodeRequest::new(path);
        let model = rrrah_decode::decode_model(&request).unwrap();
        let mut cache = ModelDisplayCache::new(rrrah_cache::CacheLimits {
            max_bytes: model.capacity_bytes() * 2,
            max_entries: Some(1),
            ttl: None,
        });
        cache.load(&request, true, || Ok(model.clone())).unwrap();
        let visible = cache.visible.clone();
        let mut background = request.clone();
        background.image_index = 1;
        cache.load(&background, false, || Ok(model.clone())).unwrap();
        assert_eq!(cache.visible, visible);
        assert_eq!(cache.entries.len(), 1);
        cache
            .load(&request, false, || panic!("visible model was displaced"))
            .unwrap();
        assert_eq!(cache.visible, visible);
    }
    #[test]
    fn real_model_preload_is_available_without_foreground_decode() {
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../tests/fixtures/models/off-attributes-binary.off");
        let request = DecodeRequest::new(path);
        let mut cache = ModelDisplayCache::new(rrrah_cache::CacheLimits::bytes(1024 * 1024));
        let gate = Arc::new(DecodeGate::new());
        let warmed = preload_model(&request, &mut cache, &gate).unwrap();
        assert!(cache.visible.is_none());
        let hit = cache
            .load(&request, true, || panic!("preloaded model decoded again"))
            .unwrap();
        let (rrrah_decode::DecodedModel::Off(a), rrrah_decode::DecodedModel::Off(b)) = (&warmed, &hit) else {
            panic!()
        };
        assert!(a.ptr_eq(b));
    }
    #[test]
    fn model_prefetch_headroom_skips_cold_load_but_keeps_ready_geometry() {
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../tests/fixtures/models/off-attributes-binary.off");
        let root = rrrah_cache::MemoryBudget::new(1024 * 1024);
        let mut request = DecodeRequest::new(path);
        request.memory_budget = Some(root.child(root.limit()));
        let mut cache = ModelDisplayCache::new(rrrah_cache::CacheLimits::bytes(root.limit()));
        let gate = Arc::new(DecodeGate::new());
        let blocker = root.try_reserve(root.limit()).unwrap();
        assert!(
            preload_model(&request, &mut cache, &gate)
                .unwrap_err()
                .contains("source memory headroom")
        );
        assert_eq!(cache.entries.len(), 0);
        drop(blocker);
        let warmed = preload_model(&request, &mut cache, &gate).unwrap();
        let blocker = root.try_reserve(root.available_bytes()).unwrap();
        let hit = preload_model(&request, &mut cache, &gate).unwrap();
        let (rrrah_decode::DecodedModel::Off(a), rrrah_decode::DecodedModel::Off(b)) = (&warmed, &hit) else {
            panic!()
        };
        assert!(a.ptr_eq(b));
        drop(blocker);
        drop(hit);
        drop(warmed);
        drop(cache);
        assert_eq!(root.used(), 0);
    }
    #[test]
    fn raster_prefetch_headroom_preserves_ram_hits_and_recovers_after_pressure() {
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../tests/fixtures/raster/gif-animation-disposal-3.gif");
        let root = rrrah_cache::MemoryBudget::new(1024 * 1024);
        let decode = root.child(1024 * 1024);
        let mut request = DecodeRequest::new(path);
        request.memory_budget = Some(decode);
        let mut cache = RasterDisplayCache::new(rrrah_cache::CacheLimits::bytes(1024 * 1024));
        let gate = Arc::new(DecodeGate::new());
        let blocker = root.try_reserve(root.limit()).unwrap();
        let error = preload_raster(&request, None, &mut cache, &gate).unwrap_err();
        assert!(error.contains("insufficient source memory headroom"));
        assert_eq!(root.used(), root.limit());
        assert_eq!(cache.len(), 0);
        drop(blocker);
        let (warmed, _) = preload_raster(&request, None, &mut cache, &gate).unwrap();
        let blocker = root.try_reserve(root.available_bytes()).unwrap();
        let (hit, _) = preload_raster(&request, None, &mut cache, &gate).unwrap();
        let (rrrah_core::RasterPixels::Rgba32Float(a), rrrah_core::RasterPixels::Rgba32Float(b)) =
            (warmed.pixels(), hit.pixels())
        else {
            panic!()
        };
        assert!(a.ptr_eq(b));
        drop(blocker);
        drop(hit);
        drop(warmed);
        drop(cache);
        assert_eq!(root.used(), 0);
    }
    #[test]
    fn real_raster_preload_preserves_original_pin_and_shares_warmed_storage() {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/raster");
        let request = DecodeRequest::new(root.join("gif-animation-disposal-3.gif"));
        let mut cache = RasterDisplayCache::new(rrrah_cache::CacheLimits::bytes(1024 * 1024));
        let gate = Arc::new(DecodeGate::new());
        let (warmed, assumed) = preload_raster(&request, None, &mut cache, &gate).unwrap();
        let (hit, hit_assumed) = load_cached_raster_with(&request, None, &mut cache, || {
            panic!("preloaded raster decoded again")
        })
        .unwrap();
        assert_eq!(assumed, hit_assumed);
        let (rrrah_core::RasterPixels::Rgba32Float(a), rrrah_core::RasterPixels::Rgba32Float(b)) =
            (warmed.pixels(), hit.pixels())
        else {
            panic!()
        };
        assert!(a.ptr_eq(b));
        let mut pinned = RasterDisplayCache::new(rrrah_cache::CacheLimits {
            max_bytes: 1024 * 1024,
            max_entries: Some(1),
            ttl: None,
        });
        load_cached_raster_for_display(&request, None, &mut pinned).unwrap();
        let mut background = request.clone();
        background.image_index = 1;
        preload_raster(&background, None, &mut pinned, &gate).unwrap();
        load_cached_raster_with(&request, None, &mut pinned, || {
            panic!("background admission displaced visible raster")
        })
        .unwrap();
        assert_eq!(pinned.len(), 1);
    }
    #[test]
    fn tiff_neighbour_is_classified_and_reuses_preloaded_managed_pixels() {
        let budget = rrrah_cache::MemoryBudget::new(1024 * 1024);
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../tests/fixtures/raster/pattern.profiled.tif");
        assert!(foreground_neighbour_supported(&path));
        assert!(foreground_neighbour_supported(Path::new("image.TIFF")));
        assert!(foreground_neighbour_supported(Path::new("image.TIF")));
        assert!(!foreground_neighbour_supported(Path::new("notes.txt")));
        // Dedicated camera extensions retain the separate RAW prefetch worker.
        assert!(!foreground_neighbour_supported(Path::new("image.CR3")));
        let mut request = DecodeRequest::new(path);
        request.memory_budget = Some(budget.clone());
        assert!(matches!(
            rrrah_decode::image_source_kind(&request).unwrap(),
            rrrah_decode::ImageSourceKind::Raster
        ));
        let mut cache = RasterDisplayCache::new(rrrah_cache::CacheLimits::bytes(budget.limit()));
        let gate = Arc::new(DecodeGate::new());
        let (warmed, _) = preload_raster(&request, None, &mut cache, &gate).unwrap();
        let blocker = budget.try_reserve(budget.available_bytes()).unwrap();
        let (hit, _) = load_cached_raster_with(&request, None, &mut cache, || {
            panic!("TIFF neighbour must reuse prepared RAM pixels")
        })
        .unwrap();
        let (rrrah_core::RasterPixels::Rgba32Float(a), rrrah_core::RasterPixels::Rgba32Float(b)) =
            (warmed.pixels(), hit.pixels())
        else {
            panic!()
        };
        assert!(a.ptr_eq(b));
        assert!(a.is_managed());
        drop(blocker);
        drop(hit);
        drop(warmed);
        drop(cache);
        assert_eq!(budget.used(), 0);
    }
    #[test]
    fn viewer_untagged_policy_is_explicit_and_preserves_strict_cache_boundary() {
        assert!(matches!(
            Cli::try_parse_from(["rrrah"]).unwrap().untagged_color,
            UntaggedColor::Strict
        ));
        assert!(Cli::try_parse_from(["rrrah", "--inspect"]).is_ok());
        assert!(Cli::try_parse_from(["rrrah", "--inspect", "--untagged-color", "srgb"]).is_err());
        assert!(Cli::try_parse_from(["rrrah", "--untagged-color", "unknown"]).is_err());
        let budget = rrrah_cache::MemoryBudget::new(4096);
        let path =
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/pict/red-rectangle-v2.pict");
        let mut request = DecodeRequest::new(path);
        request.memory_budget = Some(budget.clone());
        let mut cache = RasterDisplayCache::new(rrrah_cache::CacheLimits::bytes(budget.limit()));
        let gate = Arc::new(DecodeGate::new());
        assert!(preload_raster(&request, None, &mut cache, &gate).is_err());
        assert_eq!(budget.used(), 0);
        let cli = Cli::try_parse_from(["rrrah", "--untagged-color", "srgb"]).unwrap();
        cli.untagged_color.apply(&mut request);
        let (ready, assumed) = preload_raster(&request, None, &mut cache, &gate).unwrap();
        assert!(assumed);
        UntaggedColor::Strict.apply(&mut request);
        assert!(!request.assume_untagged_srgb && !request.assume_untagged_linear_srgb);
        assert!(load_cached_raster_for_display(&request, None, &mut cache).is_err());
        drop(ready);
        drop(cache);
        assert_eq!(budget.used(), 0);
        assert!(Cli::try_parse_from(["rrrah", "--untagged-color", "linear-srgb"]).is_err());
    }

    #[test]
    #[ignore = "requires external SD10 source"]
    fn real_x3f_neighbour_windows_preserve_visible_and_reuse_preload() {
        let source = std::env::var_os("RRRAH_X3F_SOURCE").expect("SD10 source");
        let dir = tempfile::tempdir().unwrap();
        let paths: Vec<_> = (0..5)
            .map(|i| {
                let p = dir.path().join(format!("{i}.x3f"));
                std::fs::copy(&source, &p).unwrap();
                p
            })
            .collect();
        let budget = rrrah_cache::MemoryBudget::new(512 * 1024 * 1024);
        let mut cache = RasterDisplayCache::new(rrrah_cache::CacheLimits {
            max_bytes: 512 * 1024 * 1024,
            max_entries: Some(2),
            ttl: None,
        });
        let gate = Arc::new(DecodeGate::new());
        let request_for = |path: PathBuf| {
            let mut request = DecodeRequest::new(path);
            request.memory_budget = Some(budget.clone());
            request
        };
        let visible = request_for(paths[2].clone());
        let (frame, _) = load_cached_raster_for_display(&visible, None, &mut cache).unwrap();
        for direction in [gallery::NavDirection::Forward, gallery::NavDirection::Backward] {
            let planned = gallery::neighbour_prefetch_paths(
                &paths,
                2,
                direction,
                gallery::PrefetchWindow { behind: 1, ahead: 2 },
            );
            assert_eq!(planned.len(), 3);
            assert_eq!(
                planned[0],
                paths[if direction == gallery::NavDirection::Forward {
                    3
                } else {
                    1
                }]
            );
            for path in planned {
                assert!(foreground_neighbour_supported(&path));
                let request = request_for(path);
                let (warmed, _) = preload_raster(&request, None, &mut cache, &gate).unwrap();
                let (hit, _) = load_cached_raster_mode(&request, None, &mut cache, false, || {
                    panic!("preloaded X3F decoded again")
                })
                .unwrap();
                let (rrrah_core::RasterPixels::Rgba32Float(a), rrrah_core::RasterPixels::Rgba32Float(b)) =
                    (warmed.pixels(), hit.pixels())
                else {
                    panic!()
                };
                assert!(a.ptr_eq(b));
                assert_eq!(cache.len(), 2);
                drop(hit);
                drop(warmed);
                drop(
                    load_cached_raster_with(&visible, None, &mut cache, || panic!("visible X3F displaced"))
                        .unwrap(),
                );
            }
        }
        drop(frame);
        drop(cache);
        assert_eq!(budget.used(), 0);
        eprintln!(
            "X3F previous=1 next=2 prefetch both directions; shared RAM hits and visible pin preserved"
        );
    }
    #[test]
    fn pict_neighbour_preload_recovers_from_pressure_and_reuses_managed_pixels() {
        let budget = rrrah_cache::MemoryBudget::new(4096);
        let path =
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/pict/red-rectangle-v2.pict");
        assert!(foreground_neighbour_supported(&path));
        assert!(foreground_neighbour_supported(Path::new("drawing.PCT")));
        let mut request = DecodeRequest::new(path);
        request.memory_budget = Some(budget.clone());
        request.assume_untagged_srgb = true;
        let mut cache = RasterDisplayCache::new(rrrah_cache::CacheLimits {
            max_bytes: 4096,
            max_entries: Some(1),
            ttl: None,
        });
        let gate = Arc::new(DecodeGate::new());
        let pressure = budget.try_reserve(budget.limit()).unwrap();
        assert!(preload_raster(&request, None, &mut cache, &gate).is_err());
        assert_eq!(cache.len(), 0);
        drop(pressure);
        let (warmed, assumed) = preload_raster(&request, None, &mut cache, &gate).unwrap();
        assert!(assumed);
        let pressure = budget.try_reserve(budget.available_bytes()).unwrap();
        let (hit, hit_assumed) = load_cached_raster_with(&request, None, &mut cache, || {
            panic!("preloaded PICT neighbour must not decode again")
        })
        .unwrap();
        assert!(hit_assumed);
        let (rrrah_core::RasterPixels::Rgba32Float(a), rrrah_core::RasterPixels::Rgba32Float(b)) =
            (warmed.pixels(), hit.pixels())
        else {
            panic!()
        };
        assert!(a.ptr_eq(b));
        assert!(a.is_managed());
        assert_eq!(&a[4..8], &[1.0, 0.0, 0.0, 1.0]);
        assert_eq!(a[3], 0.0);
        drop(pressure);
        drop(hit);
        drop(warmed);
        drop(cache);
        assert_eq!(budget.used(), 0);
    }
    #[test]
    fn pict_neighbour_windows_obey_count_bytes_and_ttl_without_losing_visible_frame() {
        let directory = tempfile::tempdir().unwrap();
        let source =
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/pict/red-rectangle-v2.pict");
        let paths: Vec<_> = (0..5)
            .map(|index| {
                let path = directory.path().join(format!("{index}.pict"));
                std::fs::copy(&source, &path).unwrap();
                path
            })
            .collect();
        // Each prepared 3x2 float RGBA frame weighs 96 bytes. Exercise count
        // and byte admission independently with the same actual decode path.
        for limits in [
            rrrah_cache::CacheLimits {
                max_bytes: 4096,
                max_entries: Some(2),
                ttl: None,
            },
            rrrah_cache::CacheLimits {
                max_bytes: 192,
                max_entries: None,
                ttl: None,
            },
        ] {
            let budget = rrrah_cache::MemoryBudget::new(16384);
            let request_for = |path: PathBuf| {
                let mut request = DecodeRequest::new(path);
                request.memory_budget = Some(budget.clone());
                request.assume_untagged_srgb = true;
                request
            };
            let mut cache = RasterDisplayCache::new(limits);
            let gate = Arc::new(DecodeGate::new());
            let visible = request_for(paths[2].clone());
            let (frame, _) = load_cached_raster_for_display(&visible, None, &mut cache).unwrap();
            assert_eq!(frame.capacity_bytes(), 96);
            for direction in [gallery::NavDirection::Forward, gallery::NavDirection::Backward] {
                let planned = gallery::neighbour_prefetch_paths(
                    &paths,
                    2,
                    direction,
                    gallery::PrefetchWindow { behind: 1, ahead: 2 },
                );
                assert_eq!(planned.len(), 3);
                assert_eq!(
                    planned[0],
                    paths[if direction == gallery::NavDirection::Forward {
                        3
                    } else {
                        1
                    }]
                );
                for path in planned {
                    drop(preload_raster(&request_for(path), None, &mut cache, &gate).unwrap());
                    assert_eq!(cache.len(), 2);
                    assert_eq!(cache.resident_bytes(), 192);
                    drop(
                        load_cached_raster_with(&visible, None, &mut cache, || {
                            panic!("neighbour window displaced visible PICT")
                        })
                        .unwrap(),
                    );
                }
            }
            assert!(cache.set_limits_and_spill(rrrah_cache::CacheLimits {
                max_bytes: 192,
                max_entries: Some(2),
                ttl: Some(Duration::ZERO),
            }));
            // TTL is assigned on insertion; changing policy preserves existing
            // entry deadlines. A newly decoded neighbour receives zero TTL.
            assert_eq!(cache.len(), 2);
            drop(preload_raster(&request_for(paths[4].clone()), None, &mut cache, &gate).unwrap());
            cache.spill_expired();
            assert_eq!(cache.len(), 1);
            assert_eq!(cache.resident_bytes(), 96);
            drop(
                load_cached_raster_with(&visible, None, &mut cache, || panic!("TTL expired visible PICT"))
                    .unwrap(),
            );
            drop(frame);
            drop(cache);
            assert_eq!(budget.used(), 0);
        }
    }
    #[test]
    fn queued_navigation_cancels_pict_preload_and_decodes_latest_raster() {
        let fixtures = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures");
        let (tx, worker_rx) = bounded(1);
        let gate = Arc::new(DecodeGate::new());
        let loader = ForegroundLoader {
            decode_first: false,
            automatic_policy: false,
            flight: Arc::new(std::sync::Mutex::new(None)),
            tx,
            pending: worker_rx.clone(),
            generation: Arc::new(AtomicU64::new(0)),
            decode_gate: gate.clone(),
            telemetry: Arc::new(CacheTelemetry::new(true, 4096)),
            development: None,
        };
        let first = loader
            .submit(fixtures.join("pict/red-rectangle-v2.pict"))
            .unwrap();
        let active = worker_rx.try_recv().unwrap();
        let token = GenerationToken::new(loader.generation.clone(), first);
        let held = active.foreground.acquire_decode(|| token.is_cancelled()).unwrap();
        let budget = rrrah_cache::MemoryBudget::new(1024 * 1024);
        let root = budget.clone();
        let worker_gate = gate.clone();
        let stale_token = token.clone();
        let worker = thread::spawn(move || {
            let mut request = DecodeRequest::new(active.path);
            request.memory_budget = Some(root.clone());
            request.cancellation = Some(stale_token);
            request.assume_untagged_srgb = true;
            let mut cache = RasterDisplayCache::new(rrrah_cache::CacheLimits::bytes(root.limit()));
            let result = preload_raster(&request, None, &mut cache, &worker_gate);
            assert!(result.is_err());
            assert_eq!(cache.len(), 0);
        });
        loader
            .submit(fixtures.join("pict/red-rectangle-v2.pict"))
            .unwrap();
        loader
            .submit(fixtures.join("raster/pattern.profiled.png"))
            .unwrap();
        let latest = loader
            .submit(fixtures.join("raster/gif-animation-disposal-3.gif"))
            .unwrap();
        assert!(token.is_cancelled());
        drop(held);
        worker.join().unwrap();
        assert_eq!(budget.used(), 0);
        let selected = worker_rx.try_recv().unwrap();
        assert_eq!(selected.generation, latest);
        assert!(worker_rx.try_recv().is_err());
        // Real source decoding and display preparation remain possible after cancellation.
        drop(selected.foreground);
        let mut request = DecodeRequest::new(selected.path);
        request.memory_budget = Some(budget.clone());
        request.cancellation = Some(GenerationToken::new(loader.generation.clone(), latest));
        let mut cache = RasterDisplayCache::new(rrrah_cache::CacheLimits::bytes(budget.limit()));
        let (ready, _) = preload_raster(&request, None, &mut cache, &gate).unwrap();
        assert!(ready.width() > 0);
        drop(ready);
        drop(cache);
        assert_eq!(budget.used(), 0);
    }
    #[test]
    #[ignore = "requires external SD10 source"]
    fn queued_navigation_cancels_x3f_preload_and_decodes_latest_raster() {
        let source = std::env::var_os("RRRAH_X3F_SOURCE").expect("SD10 source");
        let fixtures = tempfile::tempdir().unwrap();
        for name in ["first.x3f", "second.x3f", "third.x3f"] {
            std::fs::copy(&source, fixtures.path().join(name)).unwrap();
        }
        let (tx, worker_rx) = bounded(1);
        let gate = Arc::new(DecodeGate::new());
        let loader = ForegroundLoader {
            decode_first: false,
            automatic_policy: false,
            flight: Arc::new(std::sync::Mutex::new(None)),
            tx,
            pending: worker_rx.clone(),
            generation: Arc::new(AtomicU64::new(0)),
            decode_gate: gate.clone(),
            telemetry: Arc::new(CacheTelemetry::new(true, 4096)),
            development: None,
        };
        let first = loader.submit(fixtures.path().join("first.x3f")).unwrap();
        let active = worker_rx.try_recv().unwrap();
        let token = GenerationToken::new(loader.generation.clone(), first);
        let held = active.foreground.acquire_decode(|| token.is_cancelled()).unwrap();
        let budget = rrrah_cache::MemoryBudget::new(256 * 1024 * 1024);
        let root = budget.clone();
        let worker_gate = gate.clone();
        let stale_token = token.clone();
        let worker = thread::spawn(move || {
            let mut request = DecodeRequest::new(active.path);
            request.memory_budget = Some(root.clone());
            request.cancellation = Some(stale_token);
            request.assume_untagged_srgb = true;
            let mut cache = RasterDisplayCache::new(rrrah_cache::CacheLimits::bytes(root.limit()));
            let result = preload_raster(&request, None, &mut cache, &worker_gate);
            assert!(result.is_err());
            assert_eq!(cache.len(), 0);
        });
        loader.submit(fixtures.path().join("first.x3f")).unwrap();
        loader.submit(fixtures.path().join("second.x3f")).unwrap();
        let latest = loader.submit(fixtures.path().join("third.x3f")).unwrap();
        assert!(token.is_cancelled());
        drop(held);
        worker.join().unwrap();
        assert_eq!(budget.used(), 0);
        let selected = worker_rx.try_recv().unwrap();
        assert_eq!(selected.generation, latest);
        assert!(worker_rx.try_recv().is_err());
        // Real source decoding and display preparation remain possible after cancellation.
        drop(selected.foreground);
        let mut request = DecodeRequest::new(selected.path);
        request.memory_budget = Some(budget.clone());
        request.cancellation = Some(GenerationToken::new(loader.generation.clone(), latest));
        let mut cache = RasterDisplayCache::new(rrrah_cache::CacheLimits::bytes(budget.limit()));
        let (ready, _) = preload_raster(&request, None, &mut cache, &gate).unwrap();
        assert_eq!((ready.width(), ready.height()), (2267, 1513));
        drop(ready);
        drop(cache);
        assert_eq!(budget.used(), 0);
        eprintln!("SD10 queued navigation cancels stale preload; latest generation loads; leases released");
    }

    #[test]
    #[ignore = "requires external SD10 source"]
    fn real_x3f_generation_cancellation_after_sensor_allocation_recovers() {
        let source = PathBuf::from(std::env::var_os("RRRAH_X3F_SOURCE").expect("SD10 source"));
        let budget = rrrah_cache::MemoryBudget::new(256 * 1024 * 1024);
        let generation = Arc::new(AtomicU64::new(1));
        let mut request = DecodeRequest::new(source.clone());
        request.memory_budget = Some(budget.clone());
        request.cancellation = Some(GenerationToken::new(generation.clone(), 1));
        let worker = thread::spawn(move || load_raster_for_display_typed(&request, None));
        // The 8 MiB source plus 20 MiB unpacked sensor do not reach 32 MiB;
        // observing more proves admission of processing scratch after unpack.
        let deadline = std::time::Instant::now() + Duration::from_secs(10);
        let mut observed = 0;
        while std::time::Instant::now() < deadline && !worker.is_finished() {
            observed = observed.max(budget.used());
            if observed > 32 * 1024 * 1024 {
                break;
            }
            thread::sleep(Duration::from_millis(1));
        }
        generation.store(2, Ordering::Release);
        let result = worker.join().unwrap();
        assert!(
            observed > 32 * 1024 * 1024,
            "processing allocation never observed: {observed}"
        );
        assert!(matches!(
            result,
            Err(RasterLoadError::Decode(rrrah_decode::RasterDecodeError::Source(
                rrrah_decode::DecodeError::Cancelled
            )))
        ));
        assert_eq!(budget.used(), 0);
        let mut latest = DecodeRequest::new(source);
        latest.memory_budget = Some(budget.clone());
        latest.cancellation = Some(GenerationToken::new(generation, 2));
        let (ready, _) = load_raster_for_display_typed(&latest, None).unwrap();
        assert_eq!((ready.width(), ready.height()), (2267, 1513));
        drop(ready);
        assert_eq!(budget.used(), 0);
        eprintln!(
            "SD10 in-flight cancellation after observed {observed} bytes; latest generation recovers; leases released"
        );
    }
    #[test]
    fn generation_superseded_after_decode_cancels_color_before_output_admission() {
        for (directory, name) in [
            ("raster", "pattern.profiled.png"),
            ("raster", "gif-animation-disposal-3.gif"),
            ("pict", "red-rectangle-v2.pict"),
            ("pict", "unpacked-xrgb.pict"),
            ("pict", "drop-pad-rgb.pict"),
        ] {
            let budget = rrrah_cache::MemoryBudget::new(1024 * 1024);
            let generation = Arc::new(AtomicU64::new(1));
            let mut request = DecodeRequest::new(
                PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                    .join("../../tests/fixtures")
                    .join(directory)
                    .join(name),
            );
            request.memory_budget = Some(budget.clone());
            request.cancellation = Some(GenerationToken::new(generation.clone(), 1));
            request.assume_untagged_srgb = directory == "pict";
            let raster = rrrah_decode::decode_raster(&request).unwrap();
            let peak = budget.peak();
            assert!(budget.used() > 0);
            generation.store(2, Ordering::Release);
            assert!(matches!(
                prepare_loaded_raster(&request, raster),
                Err(RasterLoadError::Color(rrrah_decode::RasterColorError::Frame(
                    rrrah_core::RasterError::Cancelled
                )))
            ));
            assert_eq!(budget.peak(), peak, "cancel must precede output allocation");
            assert_eq!(budget.used(), 0);
            request.cancellation = Some(GenerationToken::new(generation.clone(), 2));
            let raster = rrrah_decode::decode_raster(&request).unwrap();
            let (prepared, _) = prepare_loaded_raster(&request, raster).unwrap();
            assert!(prepared.width() > 0 && prepared.height() > 0);
            assert!(budget.used() > 0);
            drop(prepared);
            assert_eq!(
                budget.used(),
                0,
                "new generation releases all decoded and prepared storage"
            );
        }
    }
    #[test]
    fn strict_raster_request_cannot_reuse_assumed_color_cache_entry() {
        let temporary = tempfile::tempdir().unwrap();
        let mut pfm = b"PF\n1 1\n-1\n".to_vec();
        for sample in [0.25_f32, 0.5, 2.0] {
            pfm.extend_from_slice(&sample.to_le_bytes());
        }
        std::fs::write(temporary.path().join("pattern.pfm"), pfm).unwrap();
        for (name, linear) in [("pattern.tif", false), ("pattern.pfm", true)] {
            let budget = rrrah_cache::MemoryBudget::new(1024 * 1024);
            let mut request = DecodeRequest::new(
                PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                    .join("../../tests/fixtures/raster")
                    .join(name),
            );
            if linear {
                request.path = temporary.path().join(name);
            }
            request.memory_budget = Some(budget.clone());
            request.assume_untagged_srgb = !linear;
            request.assume_untagged_linear_srgb = linear;
            let mut cache = RasterDisplayCache::new(rrrah_cache::CacheLimits::bytes(budget.limit()));
            let (first, _) = load_cached_raster_for_display(&request, None, &mut cache).unwrap();
            let mut strict = request.clone();
            strict.assume_untagged_srgb = false;
            strict.assume_untagged_linear_srgb = false;
            let mut loads = 0;
            assert!(
                load_cached_raster_with(&strict, None, &mut cache, || {
                    loads += 1;
                    load_raster_for_display_typed(&strict, None)
                })
                .is_err()
            );
            assert_eq!(
                loads, 1,
                "strict request must validate color instead of hitting assumed pixels"
            );
            let (hit, _) = load_cached_raster_with(&request, None, &mut cache, || {
                panic!("failed strict request must preserve the original cache entry")
            })
            .unwrap();
            let (rrrah_core::RasterPixels::Rgba32Float(a), rrrah_core::RasterPixels::Rgba32Float(b)) =
                (first.pixels(), hit.pixels())
            else {
                panic!()
            };
            assert!(a.ptr_eq(b));
            drop(first);
            drop(hit);
            drop(cache);
            assert_eq!(budget.used(), 0);
        }
    }

    #[test]
    fn scientific_preload_retains_window_key_and_bypasses_repeat_decode() {
        let path =
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/raster/mrc-i16-big.mrc");
        let request = DecodeRequest::new(path);
        let window = rrrah_decode::ScalarWindow::new(-32768., 32767.).unwrap();
        let mut cache = RasterDisplayCache::new(rrrah_cache::CacheLimits::bytes(1024 * 1024));
        let gate = Arc::new(DecodeGate::new());
        let (warmed, _) = preload_raster(&request, Some(window), &mut cache, &gate).unwrap();
        let (hit, _) = load_cached_raster_with(&request, Some(window), &mut cache, || {
            panic!("scientific preload decoded again")
        })
        .unwrap();
        let (rrrah_core::RasterPixels::Rgba32Float(a), rrrah_core::RasterPixels::Rgba32Float(b)) =
            (warmed.pixels(), hit.pixels())
        else {
            panic!()
        };
        assert!(a.ptr_eq(b));
        assert!(
            load_cached_raster_with(&request, None, &mut cache, || Err("missing scalar window".into()))
                .is_err()
        );
    }
    #[test]
    fn speculative_cache_paths_reject_cancelled_requests_and_stale_decode_results() {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures");
        let generation = Arc::new(AtomicU64::new(1));
        let budget = rrrah_cache::MemoryBudget::new(1024 * 1024);
        let mut model_request = DecodeRequest::new(root.join("models/off-attributes-binary.off"));
        model_request.memory_budget = Some(budget.clone());
        model_request.cancellation = Some(GenerationToken::new(generation.clone(), 1));
        let mut models = ModelDisplayCache::new(rrrah_cache::CacheLimits::bytes(1024 * 1024));
        assert!(
            models
                .load(&model_request, false, || {
                    let decoded = rrrah_decode::decode_model(&model_request).map_err(|e| e.to_string())?;
                    generation.store(2, Ordering::Release);
                    Ok(decoded)
                })
                .is_err()
        );
        assert!(models.entries.is_empty());
        assert!(budget.peak() > 0);
        assert_eq!(budget.used(), 0);
        assert!(
            models
                .load(&model_request, false, || panic!("cancelled model decoded"))
                .is_err()
        );
        generation.store(3, Ordering::Release);
        let mut raster_request = DecodeRequest::new(root.join("raster/gif-animation-disposal-3.gif"));
        let raster_budget = budget.child(budget.limit());
        raster_request.memory_budget = Some(raster_budget.clone());
        raster_request.cancellation = Some(GenerationToken::new(generation.clone(), 3));
        let mut rasters = RasterDisplayCache::new(rrrah_cache::CacheLimits::bytes(1024 * 1024));
        assert!(
            load_cached_raster_mode(&raster_request, None, &mut rasters, false, || {
                let decoded = load_raster_for_display_typed(&raster_request, None)?;
                generation.store(4, Ordering::Release);
                Ok(decoded)
            })
            .is_err()
        );
        assert!(rasters.is_empty());
        assert!(raster_budget.peak() > 0);
        assert_eq!(raster_budget.used(), 0);
        assert_eq!(budget.used(), 0);
        assert!(
            load_cached_raster_mode(&raster_request, None, &mut rasters, false, || panic!(
                "cancelled raster decoded"
            ))
            .is_err()
        );
        assert_eq!(budget.used(), 0);
    }
}

#[cfg(test)]
mod raster_budget_pressure_tests {
    use super::*;
    #[test]
    fn raster_retry_releases_unpinned_storage_and_preserves_visible() {
        let budget = rrrah_core::MemoryBudget::new(32);
        let make = || {
            rrrah_core::DecodedRaster::new(
                1,
                1,
                rrrah_core::RasterPixels::Rgba32Float(budget.try_buffer(4, 1.0f32).unwrap().freeze().into()),
                rrrah_core::RasterColorSpace::LinearSrgb,
            )
            .unwrap()
        };
        let old = (
            PathBuf::from("visible"),
            0,
            0,
            [0; 32],
            0,
            None,
            false,
            false,
            false,
            None,
        );
        let neighbour = (
            PathBuf::from("neighbour"),
            0,
            0,
            [0; 32],
            0,
            None,
            false,
            false,
            false,
            None,
        );
        let mut cache = RasterDisplayCache::new(rrrah_cache::CacheLimits::bytes(1024));
        assert!(cache.insert_visible(old.clone(), make()));
        assert!(cache.insert(neighbour.clone(), make()));
        let request = DecodeRequest::new(
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/raster/pattern.png"),
        );
        let mut attempts = 0;
        let (loaded, _) = load_cached_raster_with(&request, None, &mut cache, || {
            attempts += 1;
            let pixels = budget.try_buffer(4, 1.0f32).map_err(|e| {
                RasterLoadError::Color(rrrah_decode::RasterColorError::Frame(
                    rrrah_core::RasterError::Memory(e),
                ))
            })?;
            Ok((
                rrrah_core::DecodedRaster::new(
                    1,
                    1,
                    rrrah_core::RasterPixels::Rgba32Float(pixels.freeze().into()),
                    rrrah_core::RasterColorSpace::LinearSrgb,
                )
                .unwrap(),
                false,
            ))
        })
        .unwrap();
        assert_eq!(attempts, 2);
        assert!(cache.get(&old).is_some());
        assert!(cache.get(&neighbour).is_none());
        assert_eq!(budget.used(), 32);
        drop(loaded);
        drop(cache);
        assert_eq!(budget.used(), 0);
    }
}

#[cfg(test)]
mod raster_source_pressure_tests {
    use super::*;
    #[test]
    fn tiff_classification_releases_raster_neighbour_and_preserves_visible() {
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/raster/pattern.tif");
        let bytes = std::fs::metadata(&path).unwrap().len();
        let neighbour_pixels = bytes.div_ceil(16) as usize;
        let budget = rrrah_core::MemoryBudget::new(16 + neighbour_pixels as u64 * 16);
        let make = |width: usize| {
            rrrah_core::DecodedRaster::new(
                width as u32,
                1,
                rrrah_core::RasterPixels::Rgba32Float(
                    budget.try_buffer(width * 4, 1.0f32).unwrap().freeze().into(),
                ),
                rrrah_core::RasterColorSpace::LinearSrgb,
            )
            .unwrap()
        };
        let visible = (
            PathBuf::from("visible"),
            0,
            0,
            [0; 32],
            0,
            None,
            false,
            false,
            false,
            None,
        );
        let neighbour = (
            PathBuf::from("neighbour"),
            0,
            0,
            [0; 32],
            0,
            None,
            false,
            false,
            false,
            None,
        );
        let mut cache = RasterDisplayCache::new(rrrah_cache::CacheLimits::bytes(budget.limit()));
        cache.insert_visible(visible.clone(), make(1));
        cache.insert(neighbour.clone(), make(neighbour_pixels));
        let mut request = DecodeRequest::new(path);
        request.memory_budget = Some(rrrah_core::MemoryBudget::new(0));
        assert!(image_source_kind_with_pressure(&request, &mut None, Some(&mut cache), None).is_err());
        assert_eq!(cache.len(), 2, "impossible admission evicted retained frames");
        assert_eq!(budget.used(), budget.limit());
        request.memory_budget = Some(budget.clone());
        assert!(rrrah_decode::image_source_kind(&request).is_err());
        let kind = image_source_kind_with_pressure(&request, &mut None, Some(&mut cache), None).unwrap();
        assert_eq!(kind, rrrah_decode::ImageSourceKind::Raster);
        assert!(cache.get(&visible).is_some());
        assert!(cache.get(&neighbour).is_none());
        assert_eq!(budget.used(), 16);
        drop(cache);
        assert_eq!(budget.used(), 0);
    }
}

#[cfg(test)]
mod raster_gpu_cli_tests {
    use super::*;
    #[test]
    fn raster_gpu_budget_is_opt_in_and_accepts_zero() {
        assert_eq!(Cli::try_parse_from(["rrrah"]).unwrap().raster_gpu_mb, None);
        assert_eq!(
            Cli::try_parse_from(["rrrah", "--raster-gpu-mb", "0"])
                .unwrap()
                .raster_gpu_mb,
            Some(0)
        );
        assert_eq!(
            Cli::try_parse_from(["rrrah", "--raster-gpu-mb", "512"])
                .unwrap()
                .raster_gpu_mb,
            Some(512)
        );
        assert!(Cli::try_parse_from(["rrrah", "--raster-gpu-mb", "-1"]).is_err());
    }
}

#[cfg(test)]
mod model_source_pressure_tests {
    use super::*;
    #[test]
    fn tiff_classification_releases_only_unpinned_model_storage() {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures");
        let original =
            rrrah_decode::decode_model(&DecodeRequest::new(root.join("models/off-attributes-binary.off")))
                .unwrap();
        let rrrah_decode::DecodedModel::Off(mesh) = original else {
            panic!()
        };
        let mut mesh = (*mesh).clone();
        mesh.positions.reserve_exact(1024);
        let model = rrrah_decode::DecodedModel::Off(Arc::new(mesh).into());
        let bytes = model.capacity_bytes();
        let budget = rrrah_core::MemoryBudget::new(bytes);
        let model = model.try_manage(&budget).unwrap();
        let key = (PathBuf::from("visible-model"), 0, 0, [0; 32], 0);
        let mut cache = ModelDisplayCache::new(rrrah_cache::CacheLimits::bytes(bytes));
        assert!(cache.entries.insert(key.clone(), model, bytes).is_ok());
        cache.entries.pin(&key);
        cache.visible = Some(key.clone());
        let mut request = DecodeRequest::new(root.join("raster/pattern.tif"));
        request.memory_budget = Some(budget.clone());
        assert!(image_source_kind_with_pressure(&request, &mut None, None, Some(&mut cache)).is_err());
        assert_eq!(cache.entries.len(), 1);
        assert_eq!(budget.used(), bytes);
        cache.entries.unpin(&key);
        cache.visible = None;
        assert_eq!(
            image_source_kind_with_pressure(&request, &mut None, None, Some(&mut cache)).unwrap(),
            rrrah_decode::ImageSourceKind::Raster
        );
        assert_eq!(cache.entries.len(), 0);
        assert_eq!(budget.used(), 0);
    }
}

#[cfg(test)]
mod model_retry_tests {
    use super::*;
    #[test]
    fn real_stl_retry_releases_off_neighbour_and_preserves_visible_mesh() {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/models");
        let visible_model =
            rrrah_decode::decode_model(&DecodeRequest::new(root.join("off-attributes-binary.off"))).unwrap();
        let rrrah_decode::DecodedModel::Off(mesh) = &visible_model else {
            panic!()
        };
        let mut neighbour_mesh = (**mesh).clone();
        neighbour_mesh.positions.reserve_exact(1024);
        let neighbour_model = rrrah_decode::DecodedModel::Off(Arc::new(neighbour_mesh).into());
        let visible_bytes = visible_model.capacity_bytes();
        let total = visible_bytes + neighbour_model.capacity_bytes();
        let budget = rrrah_core::MemoryBudget::new(total);
        let visible_key = (PathBuf::from("visible-off"), 0, 0, [0; 32], 0);
        let neighbour_key = (PathBuf::from("neighbour-off"), 0, 0, [0; 32], 0);
        let mut cache = ModelDisplayCache::new(rrrah_cache::CacheLimits::bytes(total));
        cache
            .entries
            .insert(
                visible_key.clone(),
                visible_model.try_manage(&budget).unwrap(),
                visible_bytes,
            )
            .unwrap();
        cache.entries.pin(&visible_key);
        cache.visible = Some(visible_key.clone());
        let neighbour_bytes = neighbour_model.capacity_bytes();
        cache
            .entries
            .insert(
                neighbour_key.clone(),
                neighbour_model.try_manage(&budget).unwrap(),
                neighbour_bytes,
            )
            .unwrap();
        let mut request = DecodeRequest::new(root.join("triangle.stl"));
        request.memory_budget = Some(rrrah_core::MemoryBudget::new(0));
        assert!(
            cache
                .load(&request, true, || rrrah_decode::decode_model(&request)
                    .map_err(ModelLoadError::Decode))
                .is_err()
        );
        assert_eq!(cache.entries.len(), 2);
        request.memory_budget = Some(budget.clone());
        let mut attempts = 0;
        let loaded = cache
            .load(&request, true, || {
                attempts += 1;
                rrrah_decode::decode_model(&request).map_err(ModelLoadError::Decode)
            })
            .unwrap();
        assert_eq!(attempts, 2);
        assert_eq!(loaded.triangle_count(), 1);
        assert!(cache.entries.get(&visible_key).is_some());
        assert!(cache.entries.get(&neighbour_key).is_none());
        assert_eq!(budget.used(), visible_bytes + loaded.capacity_bytes());
        drop(cache);
        assert_eq!(budget.used(), loaded.capacity_bytes());
        drop(loaded);
        assert_eq!(budget.used(), 0);
    }
}

#[cfg(test)]
mod model_gpu_cli_tests {
    use super::*;
    #[test]
    fn model_gpu_budget_is_opt_in_and_accepts_zero() {
        assert_eq!(Cli::try_parse_from(["rrrah"]).unwrap().model_gpu_mb, None);
        assert_eq!(
            Cli::try_parse_from(["rrrah", "--model-gpu-mb", "0"])
                .unwrap()
                .model_gpu_mb,
            Some(0)
        );
        assert_eq!(
            Cli::try_parse_from(["rrrah", "--model-gpu-mb", "128"])
                .unwrap()
                .model_gpu_mb,
            Some(128)
        );
        assert!(Cli::try_parse_from(["rrrah", "--model-gpu-mb", "-1"]).is_err());
    }
}

#[cfg(test)]
mod shared_gpu_cli_tests {
    use super::*;
    #[test]
    fn retained_write_queues_do_not_charge_managed_pixels_twice() {
        let root = rrrah_cache::MemoryBudget::new(16);
        let pixels = root.try_buffer(8, 7_u16).unwrap().freeze();
        let swap_queue = retained_queue_budget(16);
        let disk_queue = retained_queue_budget(16);
        let swap_job = (
            pixels.clone(),
            swap_queue.try_reserve(pixels.capacity_bytes()).unwrap(),
        );
        let disk_job = (
            pixels.clone(),
            disk_queue.try_reserve(pixels.capacity_bytes()).unwrap(),
        );
        assert_eq!(root.used(), 16);
        assert!(swap_queue.try_reserve(1).is_err());
        assert!(disk_queue.try_reserve(1).is_err());
        drop(pixels);
        drop(swap_job);
        assert_eq!(swap_queue.used(), 0);
        assert_eq!(root.used(), 16);
        drop(disk_job);
        assert_eq!(disk_queue.used(), 0);
        assert_eq!(root.used(), 0);
    }
    #[test]
    fn viewer_gpu_caps_share_parent_and_preserve_local_limits() {
        let cli = Cli::try_parse_from([
            "rrrah",
            "--gpu-memory-mb",
            "3",
            "--raster-gpu-mb",
            "2",
            "--model-gpu-mb",
            "2",
        ])
        .unwrap();
        let (raster, model, raw, filmstrip) = viewer_gpu_budgets(
            cli.gpu_memory_mb,
            cli.raster_gpu_mb,
            cli.model_gpu_mb,
            cli.raw_gpu_mb,
        );
        let raster = raster.unwrap();
        let model = model.unwrap();
        assert!(raw.is_some());
        assert!(filmstrip.unwrap().try_reserve(4 * 1024 * 1024).is_err());
        let held = raster.try_reserve(2 * 1024 * 1024).unwrap();
        assert!(model.try_reserve(2 * 1024 * 1024).is_err());
        assert!(raster.try_reserve(1).is_err());
        let remaining = model.try_reserve(1024 * 1024).unwrap();
        drop(held);
        assert!(raster.try_reserve(2 * 1024 * 1024).is_ok());
        drop(remaining);
        let (a, b, c, d) = viewer_gpu_budgets(Some(0), None, None, None);
        assert!(a.unwrap().try_reserve(1).is_err());
        assert!(b.unwrap().try_reserve(1).is_err());
        assert!(c.unwrap().try_reserve(1).is_err());
        assert!(d.unwrap().try_reserve(1).is_err());
        assert!(Cli::try_parse_from(["rrrah", "--gpu-memory-mb", "-1"]).is_err());
    }
}

#[cfg(test)]
mod raw_gpu_cli_tests {
    use super::*;
    #[test]
    fn raw_gpu_local_cap_is_independent_under_shared_parent() {
        let cli = Cli::try_parse_from(["rrrah", "--gpu-memory-mb", "3", "--raw-gpu-mb", "1"]).unwrap();
        let (raster, _, raw, _) = viewer_gpu_budgets(
            cli.gpu_memory_mb,
            cli.raster_gpu_mb,
            cli.model_gpu_mb,
            cli.raw_gpu_mb,
        );
        let raw = raw.unwrap();
        let held = raw.try_reserve(1024 * 1024).unwrap();
        assert!(raw.try_reserve(1).is_err());
        assert!(raster.unwrap().try_reserve(2 * 1024 * 1024).is_ok());
        drop(held);
        assert!(Cli::try_parse_from(["rrrah", "--raw-gpu-mb", "-1"]).is_err());
    }
}

#[cfg(test)]
mod managed_model_ttl_tests {
    use super::*;
    #[test]
    fn disabled_ttl_waits_for_request_without_maintenance() {
        let (tx, rx) = bounded(1);
        let worker = thread::spawn(move || {
            receive_with_maintenance(&rx, None, || panic!("disabled TTL must not run maintenance"))
        });
        tx.send(7).unwrap();
        assert_eq!(worker.join().unwrap().unwrap(), 7);
        let (tx, rx) = bounded::<()>(1);
        drop(tx);
        assert!(
            receive_with_maintenance(&rx, None, || {
                panic!("disconnected queue must not run maintenance")
            })
            .is_err()
        );
    }

    #[test]
    fn idle_receive_expires_managed_model_and_accepts_next_request() {
        let request = DecodeRequest::new(
            PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("../../tests/fixtures/models/off-attributes-binary.off"),
        );
        let root = rrrah_core::MemoryBudget::new(128 * 1024);
        let mut request = request;
        request.memory_budget = Some(root.clone());
        let model = rrrah_decode::decode_model(&request).unwrap();
        let mut cache = ModelDisplayCache::new(rrrah_cache::CacheLimits {
            max_bytes: root.limit(),
            max_entries: Some(1),
            ttl: Some(Duration::ZERO),
        });
        drop(cache.load(&request, false, || Ok(model.clone())).unwrap());
        drop(model);
        assert!(root.used() > 0);
        let (tx, rx) = bounded(1);
        let mut ticks = 0;
        let received = receive_with_maintenance(&rx, Some(Duration::from_millis(1)), || {
            ticks += 1;
            cache.spill_expired();
            assert!(cache.entries.is_empty());
            assert_eq!(root.used(), 0);
            tx.try_send(42).unwrap();
        })
        .unwrap();
        assert_eq!(received, 42);
        assert_eq!(ticks, 1);
        tx.try_send(43).unwrap();
        assert_eq!(
            receive_with_maintenance(&rx, Some(Duration::from_millis(1)), || {
                panic!("ready foreground work must precede maintenance")
            })
            .unwrap(),
            43
        );
        drop(tx);
        assert!(
            receive_with_maintenance(&rx, Some(Duration::from_millis(1)), || {
                panic!("disconnected worker must terminate")
            })
            .is_err()
        );
    }

    #[test]
    fn expired_model_entry_preserves_visible_and_external_geometry_owners() {
        let request = DecodeRequest::new(
            PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("../../tests/fixtures/models/off-attributes-binary.off"),
        );
        let model = rrrah_decode::decode_model(&request).unwrap();
        let bytes = model.capacity_bytes();
        let budget = rrrah_core::MemoryBudget::new(bytes);
        let model = model.try_manage(&budget).unwrap();
        let held = model.clone();
        let mut cache = ModelDisplayCache::new(rrrah_cache::CacheLimits {
            max_bytes: bytes,
            max_entries: Some(1),
            ttl: Some(Duration::ZERO),
        });
        let loaded = cache.load(&request, true, || Ok(model.clone())).unwrap();
        drop(model);
        drop(loaded);
        let key = cache.visible.clone().unwrap();
        assert!(cache.entries.get(&key).is_none());
        assert!(!cache.release_lru_unpinned());
        assert_eq!(budget.used(), bytes);
        cache.visible = None;
        cache.entries.unpin(&key);
        assert!(cache.release_lru_unpinned());
        assert_eq!(cache.entries.len(), 0);
        assert_eq!(budget.used(), bytes);
        assert_eq!(held.triangle_count(), 1);
        drop(cache);
        assert_eq!(budget.used(), bytes);
        drop(held);
        assert_eq!(budget.used(), 0);
    }
}

#[cfg(test)]
mod raster_swap_viewer_tests {
    use super::*;
    #[test]
    #[ignore = "requires external SD10 source"]
    fn real_x3f_display_ram_hit_and_swap_restore_exact() {
        let source = std::env::var_os("RRRAH_X3F_SOURCE").expect("X3F source");
        let dimensions =
            std::env::var("RRRAH_X3F_EXPECTED_DIMENSIONS").unwrap_or_else(|_| "2267x1513".into());
        let (width, height) = dimensions.split_once('x').unwrap();
        let dimensions = (width.parse::<u32>().unwrap(), height.parse::<u32>().unwrap());
        let parent = tempfile::tempdir().unwrap();
        let root = rrrah_cache::MemoryBudget::new(256 * 1024 * 1024);
        let cli = Cli::try_parse_from([
            "rrrah",
            "--raster-swap-mb",
            "128",
            "--raster-swap-count",
            "4",
            "--raster-swap-ttl-secs",
            "60",
            "--raster-swap-queue-mb",
            "128",
            "--raster-swap-restore-mb",
            "128",
        ])
        .unwrap();
        let mut cache = raster_cache_with_swap(
            Some(parent.path()),
            false,
            rrrah_cache::CacheLimits {
                max_bytes: root.limit(),
                max_entries: Some(1),
                ttl: None,
            },
            cli.raster_swap_config(),
            Some(&root),
        );
        let mut request = DecodeRequest::new(PathBuf::from(source));
        request.memory_budget = Some(root.clone());
        assert_eq!(
            rrrah_decode::image_source_kind(&request).unwrap(),
            rrrah_decode::ImageSourceKind::Raster
        );
        let (original, assumed) = load_cached_raster_mode(&request, None, &mut cache, false, || {
            load_raster_for_display_typed(&request, None)
        })
        .unwrap();
        assert!(!assumed);
        assert_eq!((original.width(), original.height()), dimensions);
        let mut expected = Vec::new();
        rrrah_cache::SwapPayload::write_payload(&original, &mut expected).unwrap();
        drop(original);
        let (hit, _) = load_cached_raster_mode(&request, None, &mut cache, false, || {
            panic!("RAM hit must not decode")
        })
        .unwrap();
        drop(hit);
        assert!(cache.set_limits_and_spill(rrrah_cache::CacheLimits {
            max_bytes: root.limit(),
            max_entries: Some(0),
            ttl: None
        }));
        cache.swap().unwrap().wait_idle().unwrap();
        assert!(cache.is_empty());
        assert_eq!(root.used(), 0);
        assert_eq!(cache.swap().unwrap().stats().writes, 1);
        let (restored, _) = load_cached_raster_mode(&request, None, &mut cache, false, || {
            panic!("swap restore must not decode")
        })
        .unwrap();
        let mut actual = Vec::new();
        rrrah_cache::SwapPayload::write_payload(&restored, &mut actual).unwrap();
        assert_eq!(actual, expected);
        assert_eq!(cache.swap().unwrap().stats().reads, 1);
        drop(restored);
        drop(cache);
        assert_eq!(root.used(), 0);
        eprintln!(
            "X3F {dimensions:?} application RAM hit and count-pressure swap roundtrip exact; all leases released"
        );
    }

    #[test]
    fn swap_color_assumptions_cannot_satisfy_strict_request() {
        let parent = tempfile::tempdir().unwrap();
        let root = rrrah_cache::MemoryBudget::new(1024 * 1024);
        let cli = cli();
        let mut cache = raster_cache_with_swap(
            Some(parent.path()),
            false,
            rrrah_cache::CacheLimits {
                max_bytes: root.limit(),
                max_entries: Some(1),
                ttl: Some(Duration::ZERO),
            },
            cli.raster_swap_config(),
            Some(&root),
        );
        let mut request = DecodeRequest::new(
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/raster/pattern.tif"),
        );
        request.memory_budget = Some(root.clone());
        request.assume_untagged_srgb = true;
        let (original, assumed) = load_cached_raster_mode(&request, None, &mut cache, false, || {
            load_raster_for_display_typed(&request, None)
        })
        .unwrap();
        assert!(assumed);
        let mut expected = Vec::new();
        rrrah_cache::SwapPayload::write_payload(&original, &mut expected).unwrap();
        drop(original);
        assert_eq!(cache.spill_expired(), 1);
        cache.swap().unwrap().wait_idle().unwrap();
        assert_eq!(root.used(), 0);
        let mut strict = request.clone();
        strict.assume_untagged_srgb = false;
        let mut loads = 0;
        assert!(
            load_cached_raster_mode(&strict, None, &mut cache, false, || {
                loads += 1;
                load_raster_for_display_typed(&strict, None)
            })
            .is_err()
        );
        assert_eq!(loads, 1);
        assert!(cache.is_empty());
        assert_eq!(root.used(), 0);
        assert_eq!(cache.swap().unwrap().stats().reads, 0);
        let (restored, assumed) = load_cached_raster_mode(&request, None, &mut cache, false, || {
            panic!("strict miss must preserve the other color policy's swap entry")
        })
        .unwrap();
        assert!(assumed);
        let mut actual = Vec::new();
        rrrah_cache::SwapPayload::write_payload(&restored, &mut actual).unwrap();
        assert_eq!(actual, expected);
        assert_eq!(cache.swap().unwrap().stats().reads, 1);
        drop(restored);
        drop(cache);
        assert_eq!(root.used(), 0);
    }

    #[test]
    fn idle_timer_spills_profiled_raster_and_restores_exact_payload() {
        for prepared in [false, true] {
            let parent = tempfile::tempdir().unwrap();
            let root = rrrah_cache::MemoryBudget::new(1024 * 1024);
            let cli = cli();
            let mut cache = raster_cache_with_swap(
                Some(parent.path()),
                false,
                rrrah_cache::CacheLimits {
                    max_bytes: root.limit(),
                    max_entries: Some(1),
                    ttl: Some(Duration::ZERO),
                },
                cli.raster_swap_config(),
                Some(&root),
            );
            let mut request = DecodeRequest::new(
                PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                    .join("../../tests/fixtures/raster/pattern.profiled.png"),
            );
            request.memory_budget = Some(root.clone());
            let source = rrrah_decode::decode_raster(&request).unwrap();
            assert!(matches!(
                source.color_space(),
                rrrah_core::RasterColorSpace::Icc(_)
            ));
            let raster = if prepared {
                load_raster_for_display_typed(&request, None).unwrap().0
            } else {
                source.clone()
            };
            drop(source);
            let (original, _) =
                load_cached_raster_mode(&request, None, &mut cache, false, || Ok((raster.clone(), false)))
                    .unwrap();
            let mut expected = Vec::new();
            rrrah_cache::SwapPayload::write_payload(&original, &mut expected).unwrap();
            drop(original);
            drop(raster);
            let (tx, rx) = bounded(1);
            receive_with_maintenance(&rx, Some(Duration::from_millis(1)), || {
                assert_eq!(cache.spill_expired(), 1);
                tx.try_send(()).unwrap();
            })
            .unwrap();
            cache.swap().unwrap().wait_idle().unwrap();
            assert!(cache.is_empty());
            assert_eq!(root.used(), 0);
            assert_eq!(cache.swap().unwrap().stats().writes, 1);
            let (restored, _) = load_cached_raster_mode(&request, None, &mut cache, false, || {
                panic!("idle spill must restore without decode or color preparation")
            })
            .unwrap();
            let mut actual = Vec::new();
            rrrah_cache::SwapPayload::write_payload(&restored, &mut actual).unwrap();
            assert_eq!(actual, expected);
            assert_eq!(cache.swap().unwrap().stats().reads, 1);
            drop(restored);
            drop(cache);
            assert_eq!(root.used(), 0);
        }
    }

    fn cli() -> Cli {
        Cli::try_parse_from([
            "rrrah",
            "--raster-swap-mb",
            "2",
            "--raster-swap-count",
            "4",
            "--raster-swap-ttl-secs",
            "60",
            "--raster-swap-queue-mb",
            "1",
            "--raster-swap-restore-mb",
            "1",
            "--raster-cache-count",
            "1",
        ])
        .unwrap()
    }
    fn request(root: &rrrah_cache::MemoryBudget, index: usize) -> DecodeRequest {
        let mut request = DecodeRequest::new(
            PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("../../tests/fixtures/raster/gif-animation-disposal-3.gif"),
        );
        request.memory_budget = Some(root.clone());
        request.image_index = index;
        request
    }
    #[test]
    fn raster_swap_cli_limits_and_disabled_modes_are_independent() {
        assert!(
            Cli::try_parse_from(["rrrah"])
                .unwrap()
                .raster_swap_config()
                .is_none()
        );
        let cli = cli();
        let config = cli.raster_swap_config().unwrap();
        assert_eq!(config.limits.max_bytes, 2 * 1024 * 1024);
        assert_eq!(config.limits.max_entries, Some(4));
        assert_eq!(config.limits.ttl, Some(Duration::from_secs(60)));
        assert_eq!(config.queue_bytes, 1024 * 1024);
        assert_eq!(config.queue_count, 4);
        let tuned = Cli::try_parse_from([
            "rrrah",
            "--raster-swap-mb",
            "2",
            "--raster-swap-queue-count",
            "2",
            "--swap-queue-count",
            "0",
        ])
        .unwrap();
        assert_eq!(tuned.raster_swap_config().unwrap().queue_count, 2);
        assert_eq!(tuned.swap_queue_count, 0);
        for flag in ["--swap-queue-count", "--raster-swap-queue-count"] {
            assert!(Cli::try_parse_from(["rrrah", flag, "1024"]).is_ok());
            assert!(Cli::try_parse_from(["rrrah", flag, "1025"]).is_err());
            assert!(Cli::try_parse_from(["rrrah", flag, &usize::MAX.to_string()]).is_err());
        }
        assert_eq!(config.restore_bytes, 1024 * 1024);
        assert_eq!(cli.swap_mb, 0);
        for args in [
            vec!["rrrah", "--raster-swap-mb", "2", "--no-cache"],
            vec!["rrrah", "--raster-swap-mb", "2", "--raster-swap-count", "0"],
        ] {
            assert!(Cli::try_parse_from(args).unwrap().raster_swap_config().is_none());
        }
        let parent = tempfile::tempdir().unwrap();
        let disabled =
            raster_cache_with_swap(Some(parent.path()), true, cli.raster_limits(), Some(config), None);
        assert!(disabled.swap().is_none());
        let missing_parent = raster_cache_with_swap(None, false, cli.raster_limits(), Some(config), None);
        assert!(missing_parent.swap().is_none());
        let zero_restore =
            Cli::try_parse_from(["rrrah", "--raster-swap-mb", "2", "--raster-swap-restore-mb", "0"]).unwrap();
        assert_eq!(zero_restore.raster_swap_config().unwrap().restore_bytes, 0);
    }
    #[test]
    fn real_animation_restores_prepared_pixels_without_decode_and_preserves_background_pin() {
        let cli = cli();
        let parent = tempfile::tempdir().unwrap();
        let root = rrrah_cache::MemoryBudget::new(1024 * 1024);
        let mut cache = raster_cache_with_swap(
            Some(parent.path()),
            false,
            cli.raster_limits(),
            cli.raster_swap_config(),
            Some(&root),
        );
        let first_request = request(&root, 0);
        let (first, first_assumed) =
            load_cached_raster_for_display(&first_request, None, &mut cache).unwrap();
        load_cached_raster_for_display(&request(&root, 1), None, &mut cache).unwrap();
        cache.swap().unwrap().wait_idle().unwrap();
        assert_eq!(cache.swap().unwrap().stats().writes, 1);
        let (background, assumed) = load_cached_raster_mode(&first_request, None, &mut cache, false, || {
            panic!("swap hit must bypass decode and color preparation")
        })
        .unwrap();
        assert_eq!(first_assumed, assumed);
        assert!(
            !cache.release_lru_unpinned(),
            "background restore must retain the visible pin"
        );
        let (restored, assumed) = load_cached_raster_with(&first_request, None, &mut cache, || {
            panic!("foreground swap hit must bypass decode and color preparation")
        })
        .unwrap();
        assert_eq!(first_assumed, assumed);
        let (rrrah_core::RasterPixels::Rgba32Float(a), rrrah_core::RasterPixels::Rgba32Float(b)) =
            (first.pixels(), restored.pixels())
        else {
            panic!()
        };
        assert!(b.is_managed());
        assert_eq!(
            a.iter().map(|v| v.to_bits()).collect::<Vec<_>>(),
            b.iter().map(|v| v.to_bits()).collect::<Vec<_>>()
        );
        assert_eq!(first.color_space(), restored.color_space());
        assert_eq!(first.image_index(), restored.image_index());
        assert_eq!(first.image_count(), restored.image_count());
        cache.swap().unwrap().wait_idle().unwrap();
        assert_eq!(cache.swap().unwrap().stats().reads, 2);
        drop(first);
        drop(background);
        drop(restored);
        drop(cache);
        assert_eq!(root.used(), 0);
    }
    #[test]
    fn prepared_hdr_swap_hit_preserves_values_and_cancelled_generation_skips_restore() {
        let cli = cli();
        let parent = tempfile::tempdir().unwrap();
        let root = rrrah_cache::MemoryBudget::new(1024 * 1024);
        let mut cache = raster_cache_with_swap(
            Some(parent.path()),
            false,
            cli.raster_limits(),
            cli.raster_swap_config(),
            Some(&root),
        );
        let fixture = |name: &str| {
            let mut r = DecodeRequest::new(
                PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                    .join("../../tests/fixtures/raster")
                    .join(name),
            );
            r.memory_budget = Some(root.clone());
            r
        };
        let first_request = fixture("float-none.exr");
        // Synthetic, already color-qualified display output exercises the cache
        // boundary. These EXR sources have unqualified primaries and the real
        // display-preparation path must continue refusing them.
        assert!(load_raster_for_display_typed(&first_request, None).is_err());
        let prepared = || {
            let pixels = root.try_buffer(4, 0.0f32).unwrap();
            let mut pixels = pixels;
            pixels.copy_from_slice(&[8.0, -0.0, 2.0, 1.0]);
            rrrah_core::DecodedRaster::new(
                1,
                1,
                rrrah_core::RasterPixels::Rgba32Float(pixels.freeze().into()),
                rrrah_core::RasterColorSpace::LinearSrgb,
            )
            .unwrap()
        };
        let (first, _) =
            load_cached_raster_with(&first_request, None, &mut cache, || Ok((prepared(), false))).unwrap();
        load_cached_raster_with(&fixture("float-zip.exr"), None, &mut cache, || {
            Ok((prepared(), false))
        })
        .unwrap();
        cache.swap().unwrap().wait_idle().unwrap();
        let mut cancelled = first_request.clone();
        cancelled.cancellation = Some(rrrah_decode::GenerationToken::new(Arc::new(AtomicU64::new(2)), 1));
        assert!(
            load_cached_raster_with(&cancelled, None, &mut cache, || panic!("cancelled load decoded"))
                .is_err()
        );
        assert_eq!(cache.swap().unwrap().stats().reads, 0);
        let (restored, _) = load_cached_raster_with(&first_request, None, &mut cache, || {
            panic!("HDR swap hit decoded")
        })
        .unwrap();
        let (rrrah_core::RasterPixels::Rgba32Float(a), rrrah_core::RasterPixels::Rgba32Float(b)) =
            (first.pixels(), restored.pixels())
        else {
            panic!()
        };
        assert!(a.iter().any(|v| *v > 1.0));
        assert_eq!(
            a.iter().map(|v| v.to_bits()).collect::<Vec<_>>(),
            b.iter().map(|v| v.to_bits()).collect::<Vec<_>>()
        );
        assert_eq!(first.color_space(), restored.color_space());
        cache.swap().unwrap().wait_idle().unwrap();
        drop(first);
        drop(restored);
        drop(cache);
        assert_eq!(root.used(), 0);
    }
}

#[cfg(test)]
mod raw_ram_preload_tests {
    use super::*;
    #[test]
    #[ignore = "requires local EOS R8 tests/IMG_9043.CR3 fixture"]
    fn idle_timer_spills_real_raw_and_restores_sensor_values() {
        let parent = tempfile::tempdir().unwrap();
        let root = rrrah_cache::MemoryBudget::new(128 * 1024 * 1024);
        let mut request =
            DecodeRequest::new(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/IMG_9043.CR3"));
        request.memory_budget = Some(root.clone());
        let mosaic = NativeRawDecoder.decode(&request).unwrap().mosaic;
        let metadata = mosaic.metadata.clone();
        let expected: Vec<u16> = mosaic.pixels.iter().copied().collect();
        let recipe = NativeRawDecoder.mosaic_recipe(&request).unwrap();
        let fingerprint = SourceFingerprint::from_path(&request.path).unwrap();
        let key = CacheKey::for_mosaic_recipe(&fingerprint, 0, recipe);
        let mut ram = MosaicRamCache::with_limits(rrrah_cache::CacheLimits {
            max_bytes: root.limit(),
            max_entries: Some(1),
            ttl: Some(Duration::ZERO),
        });
        ram.enable_swap(
            rrrah_cache::MosaicSwapCache::new_with_budgets(
                parent.path(),
                rrrah_cache::MosaicSwapConfig {
                    limits: rrrah_cache::CacheLimits::bytes(128 * 1024 * 1024),
                    queue_bytes: root.limit(),
                    queue_count: 1,
                    restore_bytes: root.limit(),
                },
                retained_queue_budget(root.limit()),
                root.child(root.limit()),
            )
            .unwrap(),
        );
        assert!(ram.insert(key, mosaic));
        let (tx, rx) = bounded(1);
        receive_with_maintenance(&rx, Some(Duration::from_millis(1)), || {
            assert_eq!(ram.spill_expired(), 1);
            tx.try_send(()).unwrap();
        })
        .unwrap();
        ram.swap().unwrap().wait_idle().unwrap();
        assert!(ram.is_empty());
        assert_eq!(root.used(), 0);
        assert_eq!(ram.swap().unwrap().stats().writes, 1);
        let blocker = root.try_reserve(root.limit()).unwrap();
        assert!(ram.get(&key).is_none(), "RAM pressure must refuse restore");
        assert!(ram.is_empty());
        assert_eq!(root.used(), root.limit());
        assert_eq!(ram.swap().unwrap().stats().reads, 0);
        drop(blocker);
        assert_eq!(root.used(), 0);
        assert!(ram.get_with_cancel(&key, || true).is_none());
        assert_eq!(root.used(), 0);
        let restored = ram.get(&key).expect("RAW must restore from idle spill");
        assert!(restored.pixels.is_managed());
        assert_eq!(&*restored.pixels, expected.as_slice());
        assert_eq!(restored.metadata, metadata);
        assert_eq!(ram.swap().unwrap().stats().reads, 1);
        drop(restored);
        drop(ram);
        assert_eq!(root.used(), 0);
    }

    #[test]
    #[ignore = "requires local EOS R8 tests/IMG_9043.CR3 fixture"]
    fn native_raw_preload_hits_ram_and_disk_without_transferring_visible_pin() {
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/IMG_9043.CR3");
        let dir = tempfile::tempdir().unwrap();
        let disk = DiskMosaicCache::with_max_bytes(dir.path().to_path_buf(), 512 * 1024 * 1024);
        let root = rrrah_cache::MemoryBudget::new(512 * 1024 * 1024);
        let mut request = DecodeRequest::new(path);
        request.memory_budget = Some(root.clone());
        let recipe = NativeRawDecoder.mosaic_recipe(&request).unwrap();
        let fingerprint = SourceFingerprint::from_path(&request.path).unwrap();
        let key = CacheKey::for_mosaic_recipe(&fingerprint, 0, recipe);
        let gate = Arc::new(DecodeGate::new());
        let mut ram = MosaicRamCache::new(128 * 1024 * 1024);
        let blocker = root.try_reserve(root.limit()).unwrap();
        assert!(!preload_raw(&request, &disk, &mut ram, None, 1, &gate).unwrap());
        assert_eq!(root.used(), root.limit());
        drop(blocker);
        assert!(preload_raw(&request, &disk, &mut ram, None, 1, &gate).unwrap());
        assert_eq!(ram.visible(), None);
        let first = ram.get(&key).unwrap();
        let blocker = root.try_reserve(root.available_bytes()).unwrap();
        assert!(preload_raw(&request, &disk, &mut ram, None, 1, &gate).unwrap());
        drop(blocker);
        let hit = ram.get(&key).unwrap();
        assert!(first.pixels.ptr_eq(&hit.pixels));
        assert!(first.pixels.is_managed());
        disk.store(key, &first).unwrap();
        drop(hit);
        drop(ram);
        let mut ram = MosaicRamCache::new(128 * 1024 * 1024);
        assert!(preload_raw(&request, &disk, &mut ram, None, 1, &gate).unwrap());
        assert_eq!(ram.visible(), None);
        let restored = ram.get(&key).unwrap();
        assert_eq!(first.metadata, restored.metadata);
        assert_eq!(&*first.pixels, &*restored.pixels);
        ram.mark_visible(&key);
        let generation = Arc::new(AtomicU64::new(2));
        request.cancellation = Some(GenerationToken::new(generation, 1));
        assert!(preload_raw(&request, &disk, &mut ram, None, 1, &gate).is_err());
        assert_eq!(ram.visible(), Some(key));
        drop(first);
        drop(restored);
        drop(ram);
        assert_eq!(root.used(), 0);
    }
}

#[cfg(test)]
mod speculative_pressure_tests {
    use super::*;
    #[test]
    fn raster_preload_refuses_pressure_without_discarding_warmed_neighbour() {
        let root = rrrah_cache::MemoryBudget::new(32);
        let mut request = DecodeRequest::new(
            PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("../../tests/fixtures/raster/gif-animation-disposal-3.gif"),
        );
        let mut cache = RasterDisplayCache::new(rrrah_cache::CacheLimits::bytes(32));
        let prepared = || -> Result<(rrrah_core::DecodedRaster, bool), RasterLoadError> {
            let pixels = root.try_buffer(4, 1.0f32).map_err(|e| {
                RasterLoadError::Color(rrrah_decode::RasterColorError::Frame(
                    rrrah_core::RasterError::Memory(e),
                ))
            })?;
            Ok((
                rrrah_core::DecodedRaster::new(
                    1,
                    1,
                    rrrah_core::RasterPixels::Rgba32Float(pixels.freeze().into()),
                    rrrah_core::RasterColorSpace::LinearSrgb,
                )
                .unwrap(),
                false,
            ))
        };
        load_cached_raster_mode(&request, None, &mut cache, true, prepared).unwrap();
        request.image_index = 1;
        load_cached_raster_mode(&request, None, &mut cache, false, prepared).unwrap();
        request.image_index = 2;
        let mut attempts = 0;
        assert!(
            load_cached_raster_mode(&request, None, &mut cache, false, || {
                attempts += 1;
                prepared()
            })
            .is_err()
        );
        assert_eq!(attempts, 1);
        assert_eq!(cache.len(), 2);
        assert_eq!(root.used(), 32);
        request.image_index = 1;
        load_cached_raster_mode(&request, None, &mut cache, false, || {
            panic!("pressure lost warmed neighbour")
        })
        .unwrap();
        request.image_index = 2;
        let mut attempts = 0;
        load_cached_raster_mode(&request, None, &mut cache, true, || {
            attempts += 1;
            prepared()
        })
        .unwrap();
        assert_eq!(attempts, 2);
        assert_eq!(root.used(), 32);
        drop(cache);
        assert_eq!(root.used(), 0);
    }
}

#[cfg(test)]
mod gpu_backend_cli_tests {
    use super::*;
    #[test]
    fn backend_selection_is_explicit_and_cuda_is_not_a_wgpu_backend() {
        assert_eq!(
            Cli::try_parse_from(["rrrah"]).unwrap().gpu_backend,
            rrrah_gpu::GpuBackend::Auto
        );
        for name in ["auto", "metal", "vulkan", "dx12", "gl"] {
            assert_eq!(
                Cli::try_parse_from(["rrrah", "--gpu-backend", name])
                    .unwrap()
                    .gpu_backend,
                name.parse::<rrrah_gpu::GpuBackend>().unwrap()
            );
        }
        assert!(Cli::try_parse_from(["rrrah", "--gpu-backend", "cuda"]).is_err());
    }
}

#[cfg(test)]
mod gpu_vendor_cli_tests {
    use super::*;
    #[test]
    fn nvidia_vendor_request_is_distinct_from_api_selection() {
        assert_eq!(
            Cli::try_parse_from(["rrrah"]).unwrap().gpu_vendor,
            rrrah_gpu::GpuVendor::Any
        );
        let cli =
            Cli::try_parse_from(["rrrah", "--gpu-backend", "vulkan", "--gpu-vendor", "nvidia"]).unwrap();
        assert_eq!(cli.gpu_vendor, rrrah_gpu::GpuVendor::Nvidia);
        assert_eq!(cli.gpu_backend, rrrah_gpu::GpuBackend::Vulkan);
        assert!(Cli::try_parse_from(["rrrah", "--gpu-vendor", "cuda"]).is_err());
    }
}

#[cfg(test)]
mod dicom_viewer_tests {
    use super::*;
    #[test]
    fn native_dicom_selected_frames_prepare_cache_and_release_managed_pixels() {
        let root = rrrah_cache::MemoryBudget::new(1024 * 1024);
        let mut request = DecodeRequest::new(
            PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("../../tests/fixtures/raster/dicom-signed12-rescale.dcm"),
        );
        request.memory_budget = Some(root.clone());
        let window = rrrah_decode::ScalarWindow::new(-5096., 3094.).unwrap();
        let mut cache = RasterDisplayCache::new(rrrah_cache::CacheLimits::bytes(1024 * 1024));
        assert!(load_cached_raster_for_display(&request, None, &mut cache).is_err());
        let (first, assumed) = load_cached_raster_for_display(&request, Some(window), &mut cache).unwrap();
        assert!(!assumed);
        let (hit, _) = load_cached_raster_with(&request, Some(window), &mut cache, || {
            panic!("DICOM cache hit decoded")
        })
        .unwrap();
        let (rrrah_core::RasterPixels::Rgba32Float(a), rrrah_core::RasterPixels::Rgba32Float(b)) =
            (first.pixels(), hit.pixels())
        else {
            panic!()
        };
        assert!(a.ptr_eq(b));
        assert!(a.is_managed());
        assert_eq!(first.image_count(), 2);
        request.image_index = 1;
        let (second, _) = load_cached_raster_for_display(&request, Some(window), &mut cache).unwrap();
        assert_eq!(second.image_index(), 1);
        assert_eq!(cache.len(), 2);
        let (other, _) = load_cached_raster_for_display(
            &request,
            Some(rrrah_decode::ScalarWindow::new(0., 4096.).unwrap()),
            &mut cache,
        )
        .unwrap();
        assert_eq!(cache.len(), 3);
        let (rrrah_core::RasterPixels::Rgba32Float(a), rrrah_core::RasterPixels::Rgba32Float(b)) =
            (second.pixels(), other.pixels())
        else {
            panic!()
        };
        assert_ne!(a.as_slice(), b.as_slice());
        drop(cache);
        assert!(root.used() > 0);
        drop(first);
        drop(hit);
        drop(second);
        drop(other);
        assert_eq!(root.used(), 0);
    }
}

#[cfg(test)]
mod model_swap_viewer_tests {
    use super::*;
    #[test]
    fn expired_models_spill_and_restore_without_redecoding() {
        for name in [
            "triangle.stl",
            "off-attributes-binary.off",
            "obj-shared-metadata.obj",
            "ply-64-le.ply",
        ] {
            let parent = tempfile::tempdir().unwrap();
            let root = rrrah_cache::MemoryBudget::new(128 * 1024);
            let cli = Cli::try_parse_from(["rrrah", "--model-swap-mb", "1"]).unwrap();
            let mut cache = model_cache_with_swap(
                Some(parent.path()),
                false,
                rrrah_cache::CacheLimits {
                    max_bytes: root.limit(),
                    max_entries: Some(2),
                    ttl: Some(Duration::ZERO),
                },
                cli.model_swap_config(),
                Some(&root),
            );
            let mut request = DecodeRequest::new(
                PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                    .join("../../tests/fixtures/models")
                    .join(name),
            );
            request.memory_budget = Some(root.clone());
            let original = cache
                .load(&request, false, || {
                    rrrah_decode::decode_model(&request).map_err(ModelLoadError::Decode)
                })
                .unwrap();
            let mut expected = Vec::new();
            rrrah_cache::SwapPayload::write_payload(
                &model_swap::ModelPayload::from_model(original.clone()).unwrap(),
                &mut expected,
            )
            .unwrap();
            drop(original);
            let (tx, rx) = bounded(1);
            let mut ticks = 0;
            assert_eq!(
                receive_with_maintenance(&rx, Some(Duration::from_millis(1)), || {
                    ticks += 1;
                    cache.spill_expired();
                    tx.try_send(()).unwrap();
                })
                .unwrap(),
                ()
            );
            assert_eq!(ticks, 1);
            cache.swap.as_ref().unwrap().wait_idle().unwrap();
            assert_eq!(root.used(), 0);
            assert!(cache.entries.is_empty());
            assert_eq!(cache.swap.as_ref().unwrap().stats().writes, 1);
            let restored = cache
                .load(&request, false, || panic!("expired model must restore: {name}"))
                .unwrap();
            let mut actual = Vec::new();
            rrrah_cache::SwapPayload::write_payload(
                &model_swap::ModelPayload::from_model(restored.clone()).unwrap(),
                &mut actual,
            )
            .unwrap();
            assert_eq!(actual, expected, "{name}");
            drop(restored);
            drop(cache);
            assert_eq!(root.used(), 0);
        }
    }

    #[test]
    fn model_consumer_leases_block_background_eviction_then_allow_exact_swap() {
        for name in [
            "triangle.stl",
            "off-attributes-binary.off",
            "obj-shared-metadata.obj",
            "ply-64-le.ply",
        ] {
            let parent = tempfile::tempdir().unwrap();
            let root = rrrah_cache::MemoryBudget::new(128 * 1024);
            let cli = Cli::try_parse_from(["rrrah", "--model-swap-mb", "1"]).unwrap();
            let mut cache = model_cache_with_swap(
                Some(parent.path()),
                false,
                rrrah_cache::CacheLimits {
                    max_bytes: root.limit(),
                    max_entries: Some(2),
                    ttl: None,
                },
                cli.model_swap_config(),
                Some(&root),
            );
            let mut request = DecodeRequest::new(
                PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                    .join("../../tests/fixtures/models")
                    .join(name),
            );
            request.memory_budget = Some(root.clone());
            let model = cache
                .load(&request, true, || {
                    rrrah_decode::decode_model(&request).map_err(ModelLoadError::Decode)
                })
                .unwrap();
            let lease = cache.visible_lease_for(&model).unwrap();
            let second_lease = lease.clone();
            let mut expected = Vec::new();
            rrrah_cache::SwapPayload::write_payload(&model_swap::ModelPayload(model), &mut expected).unwrap();
            let mut other = request.clone();
            other.path = parent.path().join(format!("other-{name}"));
            std::fs::copy(&request.path, &other.path).unwrap();
            let other_model = cache
                .load(&other, true, || {
                    rrrah_decode::decode_model(&other).map_err(ModelLoadError::Decode)
                })
                .unwrap();
            assert!(cache.visible_lease_for(&second_lease).is_none());
            drop(other_model);
            let mut third = request.clone();
            third.path = parent.path().join(format!("third-{name}"));
            std::fs::copy(&request.path, &third.path).unwrap();
            drop(
                cache
                    .load(&third, false, || {
                        rrrah_decode::decode_model(&third).map_err(ModelLoadError::Decode)
                    })
                    .unwrap(),
            );
            assert_eq!(cache.entries.len(), 2);
            assert_eq!(cache.swap.as_ref().unwrap().stats().writes, 0);
            assert!(!cache.release_lru_unpinned());
            drop(lease);
            assert!(!cache.release_lru_unpinned());
            drop(second_lease);
            drop(
                cache
                    .load(&third, false, || {
                        rrrah_decode::decode_model(&third).map_err(ModelLoadError::Decode)
                    })
                    .unwrap(),
            );
            cache.swap.as_ref().unwrap().wait_idle().unwrap();
            assert_eq!(cache.swap.as_ref().unwrap().stats().writes, 1);
            let restored = cache
                .load(&request, false, || panic!("lease eviction swap miss: {name}"))
                .unwrap();
            let mut actual = Vec::new();
            rrrah_cache::SwapPayload::write_payload(&model_swap::ModelPayload(restored), &mut actual)
                .unwrap();
            assert_eq!(actual, expected, "geometry or attributes changed: {name}");
            cache.swap.as_ref().unwrap().wait_idle().unwrap();
            drop(cache);
            assert_eq!(root.used(), 0);
        }
    }
    #[test]
    fn native_model_eviction_restores_before_decoder_and_preserves_managed_credit() {
        for name in [
            "triangle.stl",
            "off-attributes-binary.off",
            "off-concave-u.off",
            "obj-negative-quad.obj",
            "obj-concave-u.obj",
            "obj-shared-metadata.obj",
            "ply-32-ascii.ply",
            "ply-64-le.ply",
            "ply-offset64-be.ply",
            "ply-u64-ascii.ply",
        ] {
            let parent = tempfile::tempdir().unwrap();
            let cli =
                Cli::try_parse_from(["rrrah", "--model-swap-mb", "1", "--model-swap-count", "2"]).unwrap();
            let root = rrrah_cache::MemoryBudget::new(64 * 1024);
            let mut cache = model_cache_with_swap(
                Some(parent.path()),
                false,
                rrrah_cache::CacheLimits {
                    max_bytes: 64 * 1024,
                    max_entries: Some(1),
                    ttl: None,
                },
                cli.model_swap_config(),
                Some(&root),
            );
            let mut request = DecodeRequest::new(
                PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                    .join("../../tests/fixtures/models")
                    .join(name),
            );
            request.memory_budget = Some(root.clone());
            let first = cache
                .load(&request, true, || {
                    rrrah_decode::decode_model(&request).map_err(ModelLoadError::Decode)
                })
                .unwrap();
            assert_eq!(cache.entries.len(), 1, "first admission: {name}");
            let expected = first.bounds();
            let mut expected_payload = Vec::new();
            rrrah_cache::SwapPayload::write_payload(
                &model_swap::ModelPayload::from_model(first.clone()).unwrap(),
                &mut expected_payload,
            )
            .unwrap();
            let second_path = parent.path().join(format!("second-{name}"));
            std::fs::copy(&request.path, &second_path).unwrap();
            let mut second_request = DecodeRequest::new(second_path);
            second_request.memory_budget = Some(root.clone());
            let second = cache
                .load(&second_request, true, || {
                    rrrah_decode::decode_model(&second_request).map_err(ModelLoadError::Decode)
                })
                .unwrap();
            drop(first);
            drop(second);
            cache.swap.as_ref().unwrap().wait_idle().unwrap();
            assert_eq!(cache.swap.as_ref().unwrap().stats().writes, 1, "spill: {name}");
            let visible = cache.visible.clone();
            let pressure = root.try_reserve(root.available_bytes()).unwrap();
            assert!(
                cache
                    .load(&request, false, || rrrah_decode::decode_model(&request)
                        .map_err(ModelLoadError::Decode))
                    .is_err()
            );
            assert_eq!(cache.visible, visible);
            assert_eq!(cache.swap.as_ref().unwrap().stats().errors, 0);
            drop(pressure);
            let background = cache
                .load(&request, false, || panic!("background swap hit invoked decoder"))
                .unwrap();
            assert_eq!(background.bounds(), expected);
            assert_eq!(cache.visible, visible);
            assert_eq!(cache.entries.len(), 1);
            drop(background);
            let restored = cache
                .load(&request, true, || panic!("swap hit invoked decoder"))
                .unwrap();
            assert_eq!(restored.bounds(), expected);
            match &restored {
                rrrah_decode::DecodedModel::Stl(mesh) => assert!(mesh.is_managed()),
                rrrah_decode::DecodedModel::Off(mesh) => assert!(mesh.is_managed()),
                rrrah_decode::DecodedModel::Obj(mesh) => assert!(mesh.is_managed()),
                rrrah_decode::DecodedModel::Ply(mesh) => assert!(mesh.is_managed()),
            }
            let mut restored_payload = Vec::new();
            rrrah_cache::SwapPayload::write_payload(
                &model_swap::ModelPayload::from_model(restored.clone()).unwrap(),
                &mut restored_payload,
            )
            .unwrap();
            assert_eq!(restored_payload, expected_payload);
            assert_eq!(cache.swap.as_ref().unwrap().stats().reads, 2);
            assert!(cache.visible.is_some());
            drop(restored);
            cache.swap.as_ref().unwrap().wait_idle().unwrap();
            drop(cache);
            assert_eq!(root.used(), 0);
        }
    }
    #[test]
    fn impossible_restore_limit_does_not_evict_unpinned_models() {
        let parent = tempfile::tempdir().unwrap();
        let cli =
            Cli::try_parse_from(["rrrah", "--model-swap-mb", "1", "--model-swap-restore-mb", "0"]).unwrap();
        let mut cache = model_cache_with_swap(
            Some(parent.path()),
            false,
            rrrah_cache::CacheLimits::bytes(4096),
            cli.model_swap_config(),
            None,
        );
        let request = DecodeRequest::new(
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/models/triangle.stl"),
        );
        let model = cache
            .load(&request, true, || {
                rrrah_decode::decode_model(&request).map_err(ModelLoadError::Decode)
            })
            .unwrap();
        let key = cache.visible.take().unwrap();
        cache
            .swap
            .as_ref()
            .unwrap()
            .enqueue(key.clone(), model_swap::ModelPayload::from_model(model).unwrap());
        cache.entries.remove(&key);
        cache.swap.as_ref().unwrap().wait_idle().unwrap();
        let other = parent.path().join("other.stl");
        std::fs::copy(&request.path, &other).unwrap();
        let other_request = DecodeRequest::new(other);
        drop(
            cache
                .load(&other_request, false, || {
                    rrrah_decode::decode_model(&other_request).map_err(ModelLoadError::Decode)
                })
                .unwrap(),
        );
        let weight = cache.entries.resident_weight();
        assert!(weight > 0);
        assert!(
            cache
                .load(&request, true, || Err(ModelLoadError::Message(
                    "decoder deliberately unavailable".into()
                )))
                .is_err()
        );
        assert_eq!(cache.entries.resident_weight(), weight);
        assert_eq!(cache.entries.len(), 1);
    }
    #[test]
    fn model_swap_cli_is_independent_and_no_cache_disables_it() {
        let cli = Cli::try_parse_from([
            "rrrah",
            "--model-swap-mb",
            "7",
            "--model-swap-count",
            "3",
            "--model-swap-ttl-secs",
            "9",
            "--model-swap-queue-count",
            "2",
            "--model-swap-restore-mb",
            "0",
        ])
        .unwrap();
        let config = cli.model_swap_config().unwrap();
        assert_eq!(config.limits.max_bytes, 7 * 1024 * 1024);
        assert_eq!(config.limits.max_entries, Some(3));
        assert_eq!(config.limits.ttl, Some(Duration::from_secs(9)));
        assert_eq!(config.queue_count, 2);
        assert_eq!(config.restore_bytes, 0);
        assert!(cli.raster_swap_config().is_none());
        for args in [
            vec!["rrrah"],
            vec!["rrrah", "--model-swap-mb", "7", "--no-cache"],
            vec!["rrrah", "--model-swap-mb", "7", "--model-swap-count", "0"],
        ] {
            assert!(Cli::try_parse_from(args).unwrap().model_swap_config().is_none());
        }
        assert!(Cli::try_parse_from(["rrrah", "--model-swap-queue-count", "1025"]).is_err());
    }
}

fn parse_tone_curve(text: &str) -> Result<rrrah_core::develop::MonotoneCurve> {
    let points = text
        .split(',')
        .map(|p| {
            let (x, y) = p.split_once(':').context("tone curve points must use x:y")?;
            Ok((x.trim().parse::<f64>()?, y.trim().parse::<f64>()?))
        })
        .collect::<Result<Vec<_>>>()?;
    Ok(rrrah_core::develop::MonotoneCurve::new(&points)?)
}

fn publish_raw_event(
    event: LoadEvent,
    sender: &Sender<LoadEvent>,
    proxy: &EventLoopProxy<WakeEvent>,
    request: &DecodeRequest,
    settings: Option<&rrrah_core::develop::DevelopOptions>,
) {
    let LoadEvent::Ready {
        generation,
        ref mosaic,
        requested_at,
        ..
    } = event
    else {
        publish_load_event(event, sender, proxy);
        return;
    };
    let cancelled = || {
        request
            .cancellation
            .as_ref()
            .is_some_and(GenerationToken::is_cancelled)
    };
    let result = prepare_quality_raw(mosaic, request, settings);
    if cancelled() {
        return;
    }
    match result {
        Ok(None) => publish_load_event(event, sender, proxy),
        Ok(Some((raster, options))) => publish_load_event(
            LoadEvent::RasterReady {
                generation,
                image_index: 0,
                scalar_window: None,
                raster,
                lease: None,
                assumed_srgb: false,
                development: Some(options),
                elapsed: requested_at.elapsed(),
            },
            sender,
            proxy,
        ),
        Err(error) => publish_load_event(
            LoadEvent::Failed {
                generation,
                error: format!("RAW development: {error}"),
            },
            sender,
            proxy,
        ),
    }
}

fn prepare_quality_raw(
    mosaic: &DecodedMosaic,
    request: &DecodeRequest,
    settings: Option<&rrrah_core::develop::DevelopOptions>,
) -> Result<Option<(rrrah_core::DecodedRaster, rrrah_core::develop::DevelopOptions)>, String> {
    let cancelled = || {
        request
            .cancellation
            .as_ref()
            .is_some_and(GenerationToken::is_cancelled)
    };
    let opcodes = rrrah_decode::raw_development_opcodes(request).map_err(|e| e.to_string())?;
    let required = mosaic
        .metadata
        .cfa
        .as_ref()
        .is_some_and(|c| c.bayer_quad().is_err())
        || !opcodes.list1.is_empty()
        || !opcodes.list2.is_empty()
        || !opcodes.list3.is_empty();
    if settings.is_none() && !required {
        return Ok(None);
    }
    let options = settings.cloned().unwrap_or_default();
    let raster = rrrah_core::develop::develop_raw(
        mosaic,
        &options,
        &opcodes,
        request.memory_budget.as_ref(),
        &cancelled,
    )
    .map_err(|e| e.to_string())?;
    Ok::<_, String>(Some((raster, options)))
}
fn inspect_development(
    mosaic: &DecodedMosaic,
    request: &DecodeRequest,
    settings: Option<&rrrah_core::develop::DevelopOptions>,
) -> Result<()> {
    let started = Instant::now();
    if let Some((raster, options)) =
        prepare_quality_raw(mosaic, request, settings).map_err(anyhow::Error::msg)?
    {
        println!(
            "quality_raw: {}x{}, scene-linear RGBA32F, highlight_recovery={}, develop={:.2?}, display_curve_points={}",
            raster.width(),
            raster.height(),
            options.recover_highlights,
            started.elapsed(),
            options.curve.points().len()
        );
    }
    Ok(())
}

#[cfg(test)]
mod thumbnail_retry_tests {
    use super::*;
    #[test]
    #[ignore = "requires actual Metal device"]
    fn thumbnail_budget_callbacks_progress_without_surface_redraw() {
        let mut descriptor = wgpu::InstanceDescriptor::new_without_display_handle();
        descriptor.backends = wgpu::Backends::METAL;
        let instance = wgpu::Instance::new(descriptor);
        let adapter =
            pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions::default())).unwrap();
        assert_eq!(adapter.get_info().backend, wgpu::Backend::Metal);
        let (device, queue) =
            pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor::default())).unwrap();
        let budget = rrrah_core::MemoryBudget::new(256);
        let mut strip =
            rrrah_gpu::FilmstripRenderer::new(&device, wgpu::TextureFormat::Rgba8UnormSrgb, [640.0, 480.0])
                .with_upload_queue_budget(budget.clone());
        strip.try_upload_tile(&device, &queue, 1, 1, &[255; 4]).unwrap();
        let completions = UploadCompletions::default();
        completions.track_submitted(&queue);
        drop(strip);
        let deadline = Instant::now() + Duration::from_secs(10);
        while completions.poll(&device).unwrap() {
            assert!(Instant::now() < deadline);
            thread::sleep(Duration::from_millis(1));
        }
        assert_eq!(budget.used(), 0);
    }

    #[test]
    fn retry_keeps_managed_pixels_and_rejects_expired_or_changed_source() {
        let dir = tempfile::tempdir().unwrap();
        let cover = dir.path().join("cover.png");
        std::fs::write(&cover, [1u8; 32]).unwrap();
        let budget = rrrah_core::MemoryBudget::new(4);
        let pixels = budget.try_buffer(4, 255u8).unwrap().freeze();
        let now = Instant::now();
        let retry = DeferredThumbnail {
            ready: gallery::ThumbnailReady {
                source_stamp: gallery::SourceStamp::read(&cover),
                index: 0,
                width: 1,
                height: 1,
                pixels: pixels.into(),
            },
            folder: dir.path().to_path_buf(),
            cover: cover.clone(),
            retry_at: now + Duration::from_millis(25),
            expires_at: now + Duration::from_secs(2),
        };
        let tile = gallery::FolderTile {
            folder: dir.path().to_path_buf(),
            cover,
        };
        assert!(retry.is_current(std::slice::from_ref(&tile)));
        assert!(!retry.is_current(&[]));
        assert!(!retry.due(now));
        assert!(retry.due(now + Duration::from_millis(25)));
        assert!(!retry.expired(now + Duration::from_secs(1)));
        assert!(retry.expired(now + Duration::from_secs(2)));
        assert_eq!(budget.used(), 4);
        std::fs::write(&tile.cover, [2u8; 64]).unwrap();
        assert!(!retry.is_current(std::slice::from_ref(&tile)));
        drop(retry);
        assert_eq!(budget.used(), 0);
    }
}

fn select_viewer_surface(
    capabilities: &wgpu::SurfaceCapabilities,
    hdr: bool,
) -> anyhow::Result<(wgpu::TextureFormat, wgpu::SurfaceColorSpace)> {
    if hdr {
        let supported = capabilities.format_capabilities.iter().any(|entry| {
            entry.format == wgpu::TextureFormat::Rgba16Float
                && entry
                    .color_spaces
                    .contains(wgpu::SurfaceColorSpaces::EXTENDED_SRGB_LINEAR)
        });
        anyhow::ensure!(
            supported,
            "HDR surface requires RGBA16Float with ExtendedSrgbLinear support"
        );
        Ok((
            wgpu::TextureFormat::Rgba16Float,
            wgpu::SurfaceColorSpace::ExtendedSrgbLinear,
        ))
    } else {
        let format = capabilities
            .formats
            .iter()
            .copied()
            .find(wgpu::TextureFormat::is_srgb)
            .or_else(|| capabilities.formats.first().copied())
            .context("surface has no compatible formats")?;
        Ok((format, wgpu::SurfaceColorSpace::Auto))
    }
}

#[cfg(test)]
mod hdr_surface_policy_tests {
    use super::*;
    #[test]
    fn hdr_requires_matching_format_color_space_and_never_silently_falls_back() {
        let mut caps = wgpu::SurfaceCapabilities {
            formats: vec![wgpu::TextureFormat::Bgra8UnormSrgb],
            format_capabilities: vec![],
            present_modes: vec![wgpu::PresentMode::Fifo],
            alpha_modes: vec![wgpu::CompositeAlphaMode::Opaque],
            usages: wgpu::TextureUsages::RENDER_ATTACHMENT,
        };
        assert!(select_viewer_surface(&caps, true).is_err());
        assert_eq!(
            select_viewer_surface(&caps, false).unwrap(),
            (wgpu::TextureFormat::Bgra8UnormSrgb, wgpu::SurfaceColorSpace::Auto)
        );
        caps.format_capabilities.push(wgpu::SurfaceFormatCapabilities {
            format: wgpu::TextureFormat::Rgba16Float,
            color_spaces: wgpu::SurfaceColorSpaces::SRGB,
        });
        assert!(select_viewer_surface(&caps, true).is_err());
        caps.format_capabilities[0].color_spaces = wgpu::SurfaceColorSpaces::EXTENDED_SRGB_LINEAR;
        assert_eq!(
            select_viewer_surface(&caps, true).unwrap(),
            (
                wgpu::TextureFormat::Rgba16Float,
                wgpu::SurfaceColorSpace::ExtendedSrgbLinear
            )
        );
        caps.formats.clear();
        assert!(select_viewer_surface(&caps, false).is_err());
        assert!(select_viewer_surface(&caps, true).is_ok());
    }
}
