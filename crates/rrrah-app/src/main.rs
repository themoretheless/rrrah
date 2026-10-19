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

mod accessibility;
mod cache_maintenance;
mod cache_telemetry;
mod cache_writer;
mod decode_gate;
mod drop_inspection;
mod edit_store;
mod editor;
mod filmstrip_ui;
mod gallery;
mod histogram_worker;
mod model_swap;
mod pipeline_telemetry;
mod raster_playback;
mod raw_load_policy;
mod source_validation;

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
    LinearSrgb,
}
impl UntaggedColor {
    fn apply(self, request: &mut DecodeRequest) {
        request.assume_untagged_srgb = matches!(self, Self::Srgb);
        request.assume_untagged_linear_srgb = matches!(self, Self::LinearSrgb);
    }
}

#[derive(Debug, Clone, Copy, clap::ValueEnum)]
enum RlaAlphaChoice {
    Straight,
    Premultiplied,
}

#[derive(Debug, Clone, Copy, clap::ValueEnum)]
enum RlaFloatOrderChoice {
    Little,
    Big,
}

#[derive(Debug, Clone, Copy, clap::ValueEnum)]
enum RlaColorChoice {
    Srgb,
    LinearSrgb,
}

#[derive(Debug, Clone, Copy)]
struct RasterInterpretation {
    color: UntaggedColor,
    pfm_top_down: bool,
    alpha: Option<rrrah_decode::RlaAlphaMode>,
    float_order: Option<rrrah_decode::RlaFloatByteOrder>,
    rla_color: Option<rrrah_decode::RlaColorSpace>,
    pq_white: Option<f32>,
    hlg_display: Option<(f32, f32, f32)>,
    rpf: Option<(
        [u16; 3],
        Option<u16>,
        rrrah_decode::RlaAlphaMode,
        rrrah_decode::RlaColorSpace,
    )>,
}
impl From<UntaggedColor> for RasterInterpretation {
    fn from(color: UntaggedColor) -> Self {
        Self {
            color,
            pfm_top_down: false,
            alpha: None,
            float_order: None,
            rla_color: None,
            pq_white: None,
            hlg_display: None,
            rpf: None,
        }
    }
}
impl RasterInterpretation {
    fn apply(self, request: &mut DecodeRequest) {
        self.color.apply(request);
        request.pfm_top_down = self.pfm_top_down;
        request.rla_alpha_mode = self.alpha;
        request.rla_float_byte_order = self.float_order;
        request.rla_color_space = self.rla_color;
        request.pq_reference_white_nits = self.pq_white;
        request.hlg_display = self.hlg_display;
        request.rpf_interpretation =
            self.rpf.map(
                |(rgb, matte, alpha_mode, color)| rrrah_decode::RpfRasterInterpretation {
                    rgb,
                    matte,
                    alpha_mode,
                    color_space: match color {
                        rrrah_decode::RlaColorSpace::Srgb => rrrah_core::RasterColorSpace::Srgb,
                        rrrah_decode::RlaColorSpace::LinearSrgb => rrrah_core::RasterColorSpace::LinearSrgb,
                    },
                },
            );
    }
}
impl Cli {
    fn raster_interpretation(&self) -> RasterInterpretation {
        RasterInterpretation {
            rpf: self.rpf_rgb.as_ref().zip(self.rpf_alpha).zip(self.rpf_color).map(
                |((rgb, alpha), color)| {
                    (
                        [rgb[0], rgb[1], rgb[2]],
                        self.rpf_matte,
                        match alpha {
                            RlaAlphaChoice::Straight => rrrah_decode::RlaAlphaMode::Straight,
                            RlaAlphaChoice::Premultiplied => rrrah_decode::RlaAlphaMode::Premultiplied,
                        },
                        match color {
                            RlaColorChoice::Srgb => rrrah_decode::RlaColorSpace::Srgb,
                            RlaColorChoice::LinearSrgb => rrrah_decode::RlaColorSpace::LinearSrgb,
                        },
                    )
                },
            ),
            color: self.untagged_color,
            pfm_top_down: self.pfm_top_down,
            pq_white: self.pq_reference_white_nits,
            hlg_display: self
                .hlg_reference_white_nits
                .zip(self.hlg_peak_nits)
                .zip(self.hlg_system_gamma)
                .map(|((white, peak), gamma)| (white, peak, gamma)),
            rla_color: self.rla_color.map(|color| match color {
                RlaColorChoice::Srgb => rrrah_decode::RlaColorSpace::Srgb,
                RlaColorChoice::LinearSrgb => rrrah_decode::RlaColorSpace::LinearSrgb,
            }),
            float_order: self.rla_float_byte_order.map(|order| match order {
                RlaFloatOrderChoice::Little => rrrah_decode::RlaFloatByteOrder::Little,
                RlaFloatOrderChoice::Big => rrrah_decode::RlaFloatByteOrder::Big,
            }),
            alpha: self.rla_alpha.map(|value| match value {
                RlaAlphaChoice::Straight => rrrah_decode::RlaAlphaMode::Straight,
                RlaAlphaChoice::Premultiplied => rrrah_decode::RlaAlphaMode::Premultiplied,
            }),
        }
    }
}

fn parse_pq_white(value: &str) -> Result<f32, String> {
    let white: f32 = value
        .parse()
        .map_err(|_| "Expected a luminance number".to_owned())?;
    if !white.is_finite() || !(1.0..=10000.0).contains(&white) {
        return Err("Reference white must be between 1 and 10000 cd/m²".to_owned());
    }
    Ok(white)
}

fn parse_hlg_gamma(value: &str) -> Result<f32, String> {
    let gamma: f32 = value.parse().map_err(|_| "Expected a gamma number".to_owned())?;
    if !gamma.is_finite() || !(0.1..=4.0).contains(&gamma) {
        return Err("HLG gamma must be between 0.1 and 4".to_owned());
    }
    Ok(gamma)
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, clap::ValueEnum)]
enum DemosaicAlgorithmChoice {
    #[default]
    Auto,
    Nearest,
    Bilinear,
    SmoothHue,
    LarochePrescott,
    HamiltonAdams,
    #[value(alias = "mhc")]
    MalvarHeCutler,
    Ppg,
    Vng,
    Rcd,
    Ahd,
    Lmmse,
    Dcb,
    Igv,
    Amaze,
    AmazeVng,
    AmazeLmmse,
    RcdVng,
    RcdLmmse,
    DcbVng,
    XtransDirectional,
    XtransMarkesteijn,
    XtransMarkesteijn3Pass,
    XtransDual,
    QuadBayer,
    NonaBayer,
    HexadecaBayer,
    #[value(alias = "generic")]
    GenericCfa,
}

impl From<DemosaicAlgorithmChoice> for rrrah_core::develop::DemosaicAlgorithm {
    fn from(choice: DemosaicAlgorithmChoice) -> Self {
        match choice {
            DemosaicAlgorithmChoice::Auto => Self::Auto,
            DemosaicAlgorithmChoice::Nearest => Self::Nearest,
            DemosaicAlgorithmChoice::Bilinear => Self::Bilinear,
            DemosaicAlgorithmChoice::SmoothHue => Self::SmoothHue,
            DemosaicAlgorithmChoice::LarochePrescott => Self::LarochePrescott,
            DemosaicAlgorithmChoice::HamiltonAdams => Self::HamiltonAdams,
            DemosaicAlgorithmChoice::MalvarHeCutler => Self::MalvarHeCutler,
            DemosaicAlgorithmChoice::Ppg => Self::Ppg,
            DemosaicAlgorithmChoice::Vng => Self::Vng,
            DemosaicAlgorithmChoice::Rcd => Self::Rcd,
            DemosaicAlgorithmChoice::Ahd => Self::Ahd,
            DemosaicAlgorithmChoice::Lmmse => Self::Lmmse,
            DemosaicAlgorithmChoice::Dcb => Self::Dcb,
            DemosaicAlgorithmChoice::Igv => Self::Igv,
            DemosaicAlgorithmChoice::Amaze => Self::Amaze,
            DemosaicAlgorithmChoice::AmazeVng => Self::AmazeVng,
            DemosaicAlgorithmChoice::AmazeLmmse => Self::AmazeLmmse,
            DemosaicAlgorithmChoice::RcdVng => Self::RcdVng,
            DemosaicAlgorithmChoice::RcdLmmse => Self::RcdLmmse,
            DemosaicAlgorithmChoice::DcbVng => Self::DcbVng,
            DemosaicAlgorithmChoice::XtransDirectional => Self::XTransDirectional,
            DemosaicAlgorithmChoice::XtransMarkesteijn => Self::XTransMarkesteijn,
            DemosaicAlgorithmChoice::XtransMarkesteijn3Pass => Self::XTransMarkesteijn3Pass,
            DemosaicAlgorithmChoice::XtransDual => Self::XTransDual,
            DemosaicAlgorithmChoice::QuadBayer => Self::QuadBayer,
            DemosaicAlgorithmChoice::NonaBayer => Self::NonaBayer,
            DemosaicAlgorithmChoice::HexadecaBayer => Self::HexadecaBayer,
            DemosaicAlgorithmChoice::GenericCfa => Self::GenericCfa,
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, clap::ValueEnum)]
enum GpuDemosaicAlgorithmChoice {
    #[default]
    Bilinear,
    SmoothHue,
    #[value(alias = "mhc")]
    MalvarHeCutler,
    Directional,
    Vng,
    Rcd,
}

impl From<GpuDemosaicAlgorithmChoice> for rrrah_gpu::GpuDemosaicAlgorithm {
    fn from(choice: GpuDemosaicAlgorithmChoice) -> Self {
        match choice {
            GpuDemosaicAlgorithmChoice::Bilinear => Self::Bilinear,
            GpuDemosaicAlgorithmChoice::SmoothHue => Self::SmoothHue,
            GpuDemosaicAlgorithmChoice::MalvarHeCutler => Self::MalvarHeCutler,
            GpuDemosaicAlgorithmChoice::Directional => Self::Directional,
            GpuDemosaicAlgorithmChoice::Vng => Self::Vng,
            GpuDemosaicAlgorithmChoice::Rcd => Self::Rcd,
        }
    }
}

fn parse_false_color_passes(value: &str) -> Result<u8, String> {
    let passes: u8 = value
        .parse()
        .map_err(|_| "Expected false-color pass count 0..=4".to_owned())?;
    if passes > 4 {
        return Err("False-color pass count must be between 0 and 4".to_owned());
    }
    Ok(passes)
}

fn parse_unit_float(value: &str) -> Result<f32, String> {
    let v: f32 = value
        .parse()
        .map_err(|_| "Expected a floating-point number in 0.0..=1.0".to_owned())?;
    if !v.is_finite() || !(0.0..=1.0).contains(&v) {
        return Err("Value must be finite and between 0.0 and 1.0".to_owned());
    }
    Ok(v)
}

fn parse_nonnegative_float(value: &str) -> Result<f32, String> {
    let v: f32 = value
        .parse()
        .map_err(|_| "Expected a non-negative floating-point number".to_owned())?;
    if !v.is_finite() || v < 0.0 {
        return Err("Value must be finite and non-negative".to_owned());
    }
    Ok(v)
}

#[derive(Debug, Parser)]
#[command(name = "rrrah", about = "Native RAW and raster image viewer")]
struct Cli {
    /// Explicit alpha association for RLA files; omitted leaves ambiguous alpha refused.
    #[arg(long, value_enum, conflicts_with = "inspect")]
    rla_alpha: Option<RlaAlphaChoice>,
    /// Producer byte order of raw float32 RLA channel records; no automatic guessing.
    #[arg(long, value_enum, conflicts_with = "inspect")]
    rla_float_byte_order: Option<RlaFloatOrderChoice>,
    /// Producer-declared RLA color space; overrides the generic untagged-color setting for RLA.
    #[arg(long, value_enum, conflicts_with = "inspect")]
    rla_color: Option<RlaColorChoice>,
    /// RPF color-bank indices for red, green and blue, explicitly selected.
    #[arg(long, num_args = 3, requires_all = ["rpf_alpha", "rpf_color"])]
    rpf_rgb: Option<Vec<u16>>,
    /// Optional RPF matte-bank index; omit for an opaque import.
    #[arg(long, requires = "rpf_rgb")]
    rpf_matte: Option<u16>,
    #[arg(long, value_enum, requires = "rpf_rgb")]
    rpf_alpha: Option<RlaAlphaChoice>,
    #[arg(long, value_enum, requires = "rpf_rgb")]
    rpf_color: Option<RlaColorChoice>,
    /// PQ reference white in cd/m² (1–10000); enables explicit HDR interpretation.
    #[arg(long, value_parser = parse_pq_white, conflicts_with = "inspect")]
    pq_reference_white_nits: Option<f32>,
    /// HLG reference white in cd/m², for a zero-black display.
    #[arg(long, value_parser = parse_pq_white, requires_all = ["hlg_peak_nits", "hlg_system_gamma"], conflicts_with = "inspect")]
    hlg_reference_white_nits: Option<f32>,
    /// HLG display peak in cd/m² (1–10000).
    #[arg(long, value_parser = parse_pq_white, requires = "hlg_reference_white_nits", conflicts_with = "inspect")]
    hlg_peak_nits: Option<f32>,
    /// HLG system gamma (0.1–4); depends on display and viewing conditions.
    #[arg(long, value_parser = parse_hlg_gamma, requires = "hlg_reference_white_nits", conflicts_with = "inspect")]
    hlg_system_gamma: Option<f32>,
    /// Explicit untagged color: srgb for LDR; linear-srgb for linear data with unspecified primaries.
    #[arg(long, value_enum, default_value = "strict", conflicts_with = "inspect")]
    untagged_color: UntaggedColor,
    /// Interpret PFM rows as Adobe-style top-down (default: bottom-up).
    #[arg(long, conflicts_with = "inspect")]
    pfm_top_down: bool,
    /// Develop RAW on the CPU with AHD/X-Trans and spatial highlight recovery.
    #[arg(long)]
    raw_quality: bool,
    /// Select CPU demosaicing algorithm (implies --raw-quality).
    #[arg(long, value_enum)]
    demosaic: Option<DemosaicAlgorithmChoice>,
    /// Contrast threshold (0.0..=1.0) for dual demosaicing algorithms (implies --raw-quality).
    #[arg(long, value_parser = parse_unit_float)]
    dual_contrast_threshold: Option<f32>,
    /// Median colour-difference false-colour suppression passes (0..=4; >0 implies --raw-quality).
    #[arg(long, value_parser = parse_false_color_passes)]
    false_color_passes: Option<u8>,
    /// Pre-demosaic Bayer G1/G2 green equilibration strength (0.0..=1.0; >0 implies --raw-quality).
    #[arg(long, value_parser = parse_unit_float)]
    green_equilibration: Option<f32>,
    /// Pre-demosaic CFA impulse / hot-pixel outlier threshold (>0 implies --raw-quality).
    #[arg(long, value_parser = parse_nonnegative_float)]
    impulse_threshold: Option<f32>,
    /// Post-demosaic high-contrast chromatic fringe suppression (implies --raw-quality).
    #[arg(long)]
    defringe: bool,
    /// Select GPU fragment/compute demosaicing algorithm for fast 2x2 Bayer preview.
    #[arg(long, value_enum, default_value = "bilinear")]
    gpu_demosaic: GpuDemosaicAlgorithmChoice,
    /// Compute full-resolution calibrated Bayer on GPU before raster presentation.
    #[arg(long)]
    gpu_raw_compute: bool,
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
    /// Disable RAW, raster and model caches, swap and neighbour preloading.
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
        region_store: Option<rrrah_swap::SwapStore>,
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
    let development = if cli.raw_quality
        || cli.demosaic.is_some()
        || cli.dual_contrast_threshold.is_some()
        || cli.false_color_passes.is_some()
        || cli.green_equilibration.is_some()
        || cli.impulse_threshold.is_some()
        || cli.defringe
        || cli.tone_curve.is_some()
        || cli.no_highlight_recovery
    {
        let curve = match &cli.tone_curve {
            Some(points) => parse_tone_curve(points)?,
            None => rrrah_core::develop::MonotoneCurve::identity(),
        };
        Some(rrrah_core::develop::DevelopOptions {
            recover_highlights: !cli.no_highlight_recovery,
            curve,
            demosaic: cli
                .demosaic
                .map_or(rrrah_core::develop::DemosaicAlgorithm::Auto, Into::into),
            dual_contrast_threshold: cli.dual_contrast_threshold.unwrap_or(0.04),
            false_color_passes: cli.false_color_passes.unwrap_or(0),
            green_equilibration: cli.green_equilibration.unwrap_or(0.0),
            impulse_threshold: cli.impulse_threshold.unwrap_or(0.0),
            suppress_fringe: cli.defringe,
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
            cli.gpu_demosaic.into(),
            cli.gpu_raw_compute,
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
    gpu_demosaic: rrrah_gpu::GpuDemosaicAlgorithm,
    gpu_raw_compute: bool,
    development: Option<rrrah_core::develop::DevelopOptions>,
) -> Result<()> {
    let (sender, receiver) = unbounded();
    let event_loop = EventLoop::<WakeEvent>::with_user_event()
        .build()
        .context("create event loop")?;
    let proxy = event_loop.create_proxy();
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
        proxy.clone(),
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
        prefetch_window,
        // Disable only the independent disk warmer when the loader warms RAW RAM.
        // Gallery navigation must retain the user's configured raster/model window.
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
        gpu_demosaic,
        untagged_color,
    );
    app.gpu_raw_compute = gpu_raw_compute;
    app.event_proxy = Some(proxy.clone());
    let histogram_proxy = proxy.clone();
    app.histogram_worker.set_waker(move || {
        let _ = histogram_proxy.send_event(WakeEvent::LoadProgress);
    });
    let editor_proxy = proxy.clone();
    app.editor.set_waker(move || {
        let _ = editor_proxy.send_event(WakeEvent::LoadProgress);
    });
    let model_proxy = proxy.clone();
    app.model_upload_worker.set_waker(move || {
        let _ = model_proxy.send_event(WakeEvent::LoadProgress);
    });
    app.raster_lod_worker.set_waker(move || {
        let _ = proxy.send_event(WakeEvent::LoadProgress);
    });
    app.image_index = image_index;
    app.scalar_window = scalar_window;
    event_loop.run_app(&mut app).context("run event loop")?;
    #[cfg(debug_assertions)]
    if let Some(probe) = &app.recovery_probe {
        if !probe.success {
            bail!("live recovery qualification failed: {:?}", probe.failure);
        }
    }
    Ok(())
}

#[cfg(debug_assertions)]
mod gpu_recovery_probe;

#[derive(Debug)]
enum WakeEvent {
    AccessKit(accesskit_winit::Event),
    LoadProgress,
}
impl From<accesskit_winit::Event> for WakeEvent {
    fn from(event: accesskit_winit::Event) -> Self {
        Self::AccessKit(event)
    }
}

#[derive(Debug)]
struct LoadRequest {
    prefetch_paths: Arc<std::sync::Mutex<Option<Vec<PathBuf>>>>,
    raster_prefetch_hint: Arc<std::sync::atomic::AtomicU8>,
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

#[derive(Debug, Clone)]
struct ForegroundIdentity {
    prefetch_paths: Arc<std::sync::Mutex<Option<Vec<PathBuf>>>>,
    raster_prefetch_hint: Arc<std::sync::atomic::AtomicU8>,
    path: PathBuf,
    source: Option<gallery::SourceStamp>,
    image_index: usize,
    scalar_window: Option<rrrah_decode::ScalarWindow>,
    development: Option<rrrah_core::develop::DevelopOptions>,
    decode_first: bool,
    automatic_policy: bool,
}

impl ForegroundIdentity {
    fn same_decode(&self, other: &Self) -> bool {
        self.path == other.path
            && self.source == other.source
            && self.image_index == other.image_index
            && self.scalar_window == other.scalar_window
            && self.development == other.development
            && self.decode_first == other.decode_first
            && self.automatic_policy == other.automatic_policy
    }
}

type ForegroundFlight = Arc<std::sync::Mutex<Option<(ForegroundIdentity, u64)>>>;

struct DuplicateForeground {
    identity: ForegroundIdentity,
    generation: u64,
    check: source_validation::Check,
}

fn finish_foreground_flight(flight: &ForegroundFlight, generation: u64) {
    let mut flight = flight.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
    if flight.as_ref().is_some_and(|(_, current)| *current == generation) {
        *flight = None;
    }
}

fn admit_foreground_source(
    path: &Path,
    token: &GenerationToken,
    read: impl FnOnce(&Path) -> Result<gallery::SourceStamp, rrrah_cache::CacheError>,
) -> Option<Result<gallery::SourceStamp, String>> {
    if token.is_cancelled() {
        return None;
    }
    let result = read(path);
    if token.is_cancelled() {
        return None;
    }
    Some(result.map_err(|error| error.to_string()))
}

/// One persistent foreground decoder with a latest-wins queue. Serializing
/// requests is essential while a plane is inside entropy decode: rapid
/// navigation keeps at most one active decode and one pending path instead of
/// spawning an unbounded set of competing decoder threads.
struct ForegroundLoader {
    worker: Option<thread::JoinHandle<()>>,
    decode_first: bool,
    automatic_policy: bool,
    flight: ForegroundFlight,
    duplicate_validator: source_validation::SourceValidator,
    duplicate: std::sync::Mutex<Option<DuplicateForeground>>,
    tx: Sender<LoadRequest>,
    pending: Receiver<LoadRequest>,
    generation: Arc<AtomicU64>,
    decode_gate: Arc<DecodeGate>,
    telemetry: Arc<CacheTelemetry>,
    development: Option<rrrah_core::develop::DevelopOptions>,
}

impl Drop for ForegroundLoader {
    fn drop(&mut self) {
        self.shutdown();
    }
}

impl ForegroundLoader {
    fn shutdown(&mut self) {
        self.cancel_pending();
        let (disconnected, _) = bounded(1);
        drop(std::mem::replace(&mut self.tx, disconnected));
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
    /// Supersede a file selection even when no replacement file is available.
    fn cancel_pending(&self) {
        self.generation.fetch_add(1, Ordering::AcqRel);
        self.duplicate
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .take();
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
            && (!rrrah_decode::is_supported_raw_path(path)
                || tiff
                || matches!(
                    rrrah_decode::image_source_kind(&DecodeRequest::new(path)),
                    Ok(rrrah_decode::ImageSourceKind::Raster)
                )))
}

fn resolve_foreground_prefetch_paths(
    known: Option<Vec<PathBuf>>,
    path: &Path,
    previous: Option<&Path>,
    window: gallery::PrefetchWindow,
    scan: impl FnOnce(&Path) -> Option<Vec<PathBuf>>,
) -> Vec<PathBuf> {
    if let Some(paths) = known {
        return paths;
    }
    let Some(folder) = path.parent() else {
        return Vec::new();
    };
    let Some(paths) = scan(folder) else {
        return Vec::new();
    };
    let Some(selected) = paths.iter().position(|candidate| candidate == path) else {
        return Vec::new();
    };
    let direction = previous
        .filter(|old| old.parent() == path.parent())
        .and_then(|old| paths.iter().position(|candidate| candidate == old))
        .map_or(NavDirection::None, |old| {
            if selected < old {
                NavDirection::Backward
            } else {
                NavDirection::Forward
            }
        });
    gallery::neighbour_prefetch_paths(&paths, selected, direction, window)
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
        let worker = thread::Builder::new()
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
                    usize,
                    u64,
                    Option<rrrah_decode::ScalarWindow>,
                )> = std::collections::VecDeque::new();
                let mut protected_rasters = Vec::new();
                let mut protected_models = Vec::new();
                let mut protected_raw = Vec::new();
                let mut load_costs = raw_load_policy::RawLoadCosts::default();
                let mut previous: Option<PathBuf> = None;
                let mut previous_image_index = 0;
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
                            if let Some((path, image_index, generation, window)) = neighbours.pop_front() {
                                if worker_generation.load(Ordering::Acquire) == generation {
                                    let mut request = DecodeRequest::new(path);
                                    request.image_index = image_index;
                                    untagged_color.apply(&mut request);
                                    request.memory_budget = managed_budget.clone();
                                    request.cancellation = Some(GenerationToken::new(
                                        Arc::clone(&worker_generation),
                                        generation,
                                    ));
                                    if rrrah_decode::is_supported_model_path(&request.path) {
                                        if model_limits.max_bytes > 0 && model_limits.max_entries != Some(0) {
                                            let warmed = preload_model_with_lease(&request, &mut model_cache, &worker_gate);
                                            #[cfg(debug_assertions)]
                                            gpu_recovery_probe::record_model_prefetch(generation, &request.path,
                                                warmed.as_ref().is_ok_and(|(_, lease)| lease.is_some()));
                                            if let Ok((_, Some(lease))) = warmed {
                                                protected_models.push(lease);
                                            }
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
                                                if let Ok((true, Some(lease))) = preload_raw_with_lease(
                                                    &request,
                                                    disk,
                                                    ram,
                                                    cache_writer.as_ref(),
                                                    generation,
                                                    &worker_gate,
                                                ) {
                                                    protected_raw.push(lease);
                                                }
                                            }
                                        }
                                    } else if raster_limits.max_bytes > 0
                                        && raster_limits.max_entries != Some(0)
                                    {
                                        let mut warmed = preload_raster_with_lease(&request, window, &mut raster_cache, &worker_gate);
                                        if warmed.is_err() && window.is_some() {
                                            warmed = preload_raster_with_lease(&request, None, &mut raster_cache, &worker_gate);
                                        }
                                        match warmed {
                                            Ok((raster, _, lease)) => {
                                                #[cfg(debug_assertions)]
                                                gpu_recovery_probe::record_raster_prefetch(generation, &request.path, lease.is_some());
                                                log::debug!(
                                                    "raster prefetch ready generation={generation} path={} image_index={} image_count={} retained={}",
                                                    request.path.display(), raster.image_index(),
                                                    raster.image_count(), lease.is_some(),
                                                );
                                                if let Some(lease) = lease {
                                                    protected_rasters.push(lease);
                                                }
                                            }
                                            Err(error) => {
                                                #[cfg(debug_assertions)]
                                                gpu_recovery_probe::record_raster_prefetch(generation, &request.path, false);
                                                log::debug!(
                                                    "raster prefetch refused generation={generation} path={} image_index={image_index}: {error}",
                                                    request.path.display(),
                                                );
                                            }
                                        }
                                    }
                                } else {
                                    log::debug!(
                                        "prefetch stale generation={generation} path={} image_index={image_index}",
                                        path.display(),
                                    );
                                }
                                continue;
                            }
                            protected_rasters.clear();
                            protected_models.clear();
                            protected_raw.clear();
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
                    protected_rasters.clear();
                    protected_models.clear();
                    protected_raw.clear();
                    let path = request.path.clone();
                    let generation = request.generation;
                    let window = request.scalar_window;
                    let image_index = request.image_index;
                    let token = GenerationToken::new(worker_generation.clone(), generation);
                    // Establish the active source identity on the decoder thread.
                    // A new selection must never open its file on the UI thread.
                    let source = match admit_foreground_source(&path, &token, gallery::SourceStamp::try_read) {
                        None => continue,
                        Some(Ok(source)) => source,
                        Some(Err(error)) => {
                            publish_load_failure(generation, error, &token, &sender, &proxy);
                            finish_foreground_flight(&worker_flight, generation);
                            continue;
                        }
                    };
                    {
                        let mut flight = worker_flight.lock()
                            .unwrap_or_else(std::sync::PoisonError::into_inner);
                        if let Some((identity, current)) = flight.as_mut() {
                            if *current == generation {
                                identity.source = Some(source);
                            }
                        }
                    }
                    let request = request;
                    let prefetch_plan = Arc::clone(&request.prefetch_paths);
                    let raster_hint=Arc::clone(&request.raster_prefetch_hint);
                    let mut selected_raster_prefetch = None;
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
                        &mut selected_raster_prefetch,
                    );
                    finish_foreground_flight(&worker_flight, generation);
                    if !no_cache
                        && ((ram_cache_limits.max_bytes > 0 && ram_cache_limits.max_entries != Some(0))
                            || (model_limits.max_bytes > 0 && model_limits.max_entries != Some(0))
                            || (raster_limits.max_bytes > 0 && raster_limits.max_entries != Some(0)))
                        && worker_generation.load(Ordering::Acquire) == generation
                    {
                        if raster_limits.max_bytes > 0 && raster_limits.max_entries != Some(0) {
                            if let Some((count,looping)) = selected_raster_prefetch {
                                let previous_index=(previous.as_ref()==Some(&path)).then_some(previous_image_index);
                                let automatic_wrap=match raster_hint.load(Ordering::Acquire) { 1=>Some(false),2=>Some(true),_=>None };
                                let plan=gallery::container_prefetch_plan(count,image_index,previous_index,looping,automatic_wrap,prefetch_window);
                                let direction=plan.direction;let looping=plan.cyclic;let indices=plan.indices;
                                log::debug!(
                                    "raster prefetch plan generation={generation} path={} selected={image_index} count={count} looping={looping} direction={direction:?} ahead={} behind={} indices={indices:?}",
                                    path.display(), prefetch_window.ahead, prefetch_window.behind,
                                );
                                neighbours.extend(indices.into_iter()
                                    .map(|index| (path.clone(), index, generation, window)));
                            }
                        }
                        let prefetch_paths = prefetch_plan.lock()
                            .unwrap_or_else(std::sync::PoisonError::into_inner).clone();
                        let paths = resolve_foreground_prefetch_paths(
                            prefetch_paths, &path, previous.as_deref(), prefetch_window,
                            |folder| gallery::scan_folder_with_cancel(folder, &|| {
                                worker_generation.load(Ordering::Acquire) != generation
                            }));
                        neighbours.extend(paths.into_iter()
                            .filter(|path| foreground_neighbour_supported(path))
                            .map(|path| (path, 0, generation, window)));
                    }
                    previous = Some(path);
                    previous_image_index = image_index;
                }
                if let Some(swap) = raster_cache.swap() { swap.shutdown(); }
                if let Some(swap) = model_cache.swap.as_ref() { swap.shutdown(); }
                if let Some(swap) = ram_cache.as_ref().and_then(|cache| cache.swap()) { swap.shutdown(); }
            })?;
        Ok(Self {
            worker: Some(worker),
            decode_first,
            automatic_policy,
            flight,
            duplicate_validator: source_validation::SourceValidator::new(),
            duplicate: std::sync::Mutex::new(None),
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

    #[cfg(test)]
    fn submit(&self, path: PathBuf) -> Result<u64> {
        self.submit_image(path, 0)
    }
    #[cfg(test)]
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
        self.submit_request_with_prefetch(path, image_index, scalar_window, advance, None)
    }

    fn submit_request_with_prefetch(
        &self,
        path: PathBuf,
        image_index: usize,
        scalar_window: Option<rrrah_decode::ScalarWindow>,
        advance: bool,
        prefetch_paths: Option<Vec<PathBuf>>,
    ) -> Result<u64> {
        self.submit_request_with_context(path, image_index, scalar_window, advance, prefetch_paths, None)
    }
    fn submit_animation_frame(
        &self,
        path: PathBuf,
        image_index: usize,
        scalar_window: Option<rrrah_decode::ScalarWindow>,
        wrap: bool,
    ) -> Result<u64> {
        self.submit_request_with_context(path, image_index, scalar_window, true, None, Some(wrap))
    }

    fn submit_request_with_context(
        &self,
        path: PathBuf,
        image_index: usize,
        scalar_window: Option<rrrah_decode::ScalarWindow>,
        advance: bool,
        prefetch_paths: Option<Vec<PathBuf>>,
        animation_wrap: Option<bool>,
    ) -> Result<u64> {
        let mut identity = ForegroundIdentity {
            prefetch_paths: Arc::new(std::sync::Mutex::new(None)),
            raster_prefetch_hint: Arc::new(std::sync::atomic::AtomicU8::new(match animation_wrap {
                None => 0,
                Some(false) => 1,
                Some(true) => 2,
            })),
            source: None,
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
        if let Some((current, generation)) = flight.as_ref() {
            identity.source = current.source.clone();
            if current.same_decode(&identity) && *generation == self.current_generation() {
                current.raster_prefetch_hint.store(
                    match animation_wrap {
                        None => 0,
                        Some(false) => 1,
                        Some(true) => 2,
                    },
                    Ordering::Release,
                );
                if prefetch_paths.is_some() {
                    *current
                        .prefetch_paths
                        .lock()
                        .unwrap_or_else(std::sync::PoisonError::into_inner) = prefetch_paths;
                }
                if let Some(source) = current.source.as_ref() {
                    let mut duplicate = self
                        .duplicate
                        .lock()
                        .unwrap_or_else(std::sync::PoisonError::into_inner);
                    // Each new intent replaces validation evidence: a result
                    // completed before this request cannot prove that the file
                    // is still unchanged. The validator keeps only one queued
                    // check, while the useful decode remains uninterrupted.
                    *duplicate = Some(DuplicateForeground {
                        identity: current.clone(),
                        generation: *generation,
                        check: self.duplicate_validator.submit(path, source.clone()),
                    });
                }
                // A pending job with no source identity reads the current file
                // when its worker begins. Active jobs validate duplicates in
                // the background without interrupting unchanged decodes.
                return Ok(*generation);
            }
        }
        if flight.as_ref().is_none_or(|(current, _)| current.path != path) {
            identity.source = None;
        }
        self.duplicate
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .take();
        self.enqueue_foreground(identity, advance, prefetch_paths, &mut flight)
    }

    fn enqueue_foreground(
        &self,
        identity: ForegroundIdentity,
        advance: bool,
        prefetch_paths: Option<Vec<PathBuf>>,
        flight: &mut Option<(ForegroundIdentity, u64)>,
    ) -> Result<u64> {
        let requested_at = Instant::now();
        let generation = if advance {
            self.generation.fetch_add(1, Ordering::AcqRel).wrapping_add(1)
        } else {
            self.current_generation()
        };
        self.telemetry.begin_lookup(generation);
        *identity
            .prefetch_paths
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = prefetch_paths;
        let result = self.replace_pending(LoadRequest {
            prefetch_paths: Arc::clone(&identity.prefetch_paths),
            raster_prefetch_hint: Arc::clone(&identity.raster_prefetch_hint),
            decode_first: self.decode_first,
            automatic_policy: self.automatic_policy,
            path: identity.path.clone(),
            image_index: identity.image_index,
            scalar_window: identity.scalar_window,
            generation,
            requested_at,
            foreground: self.decode_gate.request_foreground(),
            development: identity.development.clone(),
        });
        *flight = result.as_ref().ok().map(|_| (identity, generation));
        result.map(|()| generation)
    }

    fn has_pending_duplicate(&self) -> bool {
        self.duplicate
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .is_some()
    }

    /// Called by the event loop; never waits for filesystem I/O. Only the
    /// generation that requested validation may schedule a replacement decode.
    fn poll_duplicate(&self) -> Option<Result<u64>> {
        let mut flight = self
            .flight
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let mut duplicate = self
            .duplicate
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let pending = duplicate.as_ref()?;
        if pending.generation != self.current_generation()
            || pending.identity.development != self.development
            || pending.identity.decode_first != self.decode_first
            || pending.identity.automatic_policy != self.automatic_policy
        {
            duplicate.take();
            return None;
        }
        let valid = pending.check.poll()?;
        let mut pending = duplicate.take()?;
        if valid {
            return None;
        }
        pending.identity.source = None;
        pending.identity.raster_prefetch_hint.store(0, Ordering::Release);
        // A duplicate without an explicit plan leaves the shared plan intact.
        // Carry that latest plan through a source-change retry as well.
        let prefetch_paths = pending
            .identity
            .prefetch_paths
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone();
        Some(self.enqueue_foreground(pending.identity, true, prefetch_paths, &mut flight))
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
fn model_display_key(request: &DecodeRequest) -> Option<ModelDisplayKey> {
    SourceFingerprint::from_path(&request.path).ok().map(|source| {
        (
            request.path.clone(),
            source.file_size,
            source.modified_ns,
            source.sampled_blake3,
            request.image_index,
        )
    })
}
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
    #[cfg(test)]
    fn prefetch_lease(
        &mut self,
        request: &DecodeRequest,
    ) -> Option<rrrah_cache::CacheLease<rrrah_decode::DecodedModel>> {
        self.entries.get(&model_display_key(request)?)
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
        decode: impl FnMut() -> Result<rrrah_decode::DecodedModel, ModelLoadError>,
    ) -> Result<rrrah_decode::DecodedModel, String> {
        self.load_with_cancel(request, visible, decode, || {
            request
                .cancellation
                .as_ref()
                .is_some_and(|token| token.is_cancelled())
        })
    }
    fn load_with_cancel(
        &mut self,
        request: &DecodeRequest,
        visible: bool,
        decode: impl FnMut() -> Result<rrrah_decode::DecodedModel, ModelLoadError>,
        cancelled: impl FnMut() -> bool,
    ) -> Result<rrrah_decode::DecodedModel, String> {
        self.load_with_cancel_resolving_key(|| model_display_key(request), visible, decode, cancelled)
    }
    fn load_with_cancel_resolving_key(
        &mut self,
        resolve_key: impl FnOnce() -> Option<ModelDisplayKey>,
        visible: bool,
        decode: impl FnMut() -> Result<rrrah_decode::DecodedModel, ModelLoadError>,
        cancelled: impl FnMut() -> bool,
    ) -> Result<rrrah_decode::DecodedModel, String> {
        self.load_with_cancel_resolving_key_mode(resolve_key, visible, false, decode, cancelled)
    }
    fn load_with_cancel_resolving_key_mode(
        &mut self,
        resolve_key: impl FnOnce() -> Option<ModelDisplayKey>,
        visible: bool,
        require_retention: bool,
        mut decode: impl FnMut() -> Result<rrrah_decode::DecodedModel, ModelLoadError>,
        mut cancelled: impl FnMut() -> bool,
    ) -> Result<rrrah_decode::DecodedModel, String> {
        if cancelled() {
            return Err("model load cancelled".into());
        }
        let key = resolve_key();
        self.spill_expired();
        if let Some(key) = &key {
            if let Some(model) = self.entries.get_cloned(key) {
                if cancelled() {
                    return Err("model load cancelled".into());
                }
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
        if require_retention
            && !visible
            && let Some(key) = &key
            && !self.entries.can_admit_key_by_count(key)
        {
            return Err("model prefetch skipped: all count slots are protected".into());
        }
        if require_retention && !visible
            && let Some(key) = &key
            // Even an empty model owns a nonzero model-header allocation.
            && !self.entries.can_admit_key_by_minimum_weight(key, 1)
        {
            return Err("model prefetch skipped: protected byte budget exhausted".into());
        }
        let mut restored = None;
        if let (Some(key), Some(swap)) = (&key, &self.swap) {
            loop {
                match swap.try_get(key, &mut cancelled) {
                    Ok(value) => {
                        restored = value.map(|payload| payload.0);
                        break;
                    }
                    Err(error) if visible && retryable_memory_pressure(&error) => {
                        if cancelled() {
                            return Err("model load cancelled".into());
                        }
                        swap.discard_pending_writes();
                        if self.entries.take_lru().is_none() {
                            break;
                        }
                    }
                    Err(_) => break,
                }
            }
        }
        // Cancellation is terminal, not a cache miss requiring native decode.
        if cancelled() {
            return Err("model load cancelled".into());
        }
        let model = if let Some(model) = restored {
            model
        } else {
            loop {
                if cancelled() {
                    return Err("model load cancelled".into());
                }
                match decode() {
                    Ok(model) => break model,
                    Err(error)
                        if visible
                            && error.retryable_memory_pressure()
                            && !cancelled()
                            && self.release_lru_unpinned() =>
                    {
                        continue;
                    }
                    Err(error) => return Err(error.to_string()),
                }
            }
        };
        if cancelled() {
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

#[cfg(test)]
fn preload_model(
    request: &DecodeRequest,
    cache: &mut ModelDisplayCache,
    gate: &Arc<DecodeGate>,
) -> Result<rrrah_decode::DecodedModel, String> {
    preload_model_mode(request, cache, gate, false).map(|(model, _)| model)
}

fn preload_model_with_lease(
    request: &DecodeRequest,
    cache: &mut ModelDisplayCache,
    gate: &Arc<DecodeGate>,
) -> Result<
    (
        rrrah_decode::DecodedModel,
        Option<rrrah_cache::CacheLease<rrrah_decode::DecodedModel>>,
    ),
    String,
> {
    preload_model_mode(request, cache, gate, true)
}

fn preload_model_mode(
    request: &DecodeRequest,
    cache: &mut ModelDisplayCache,
    gate: &Arc<DecodeGate>,
    retain: bool,
) -> Result<
    (
        rrrah_decode::DecodedModel,
        Option<rrrah_cache::CacheLease<rrrah_decode::DecodedModel>>,
    ),
    String,
> {
    let mut resolved = None;
    let model = cache.load_with_cancel_resolving_key_mode(
        || {
            resolved = model_display_key(request);
            resolved.clone()
        },
        false,
        retain,
        || {
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
        },
        || {
            request
                .cancellation
                .as_ref()
                .is_some_and(GenerationToken::is_cancelled)
        },
    )?;
    let lease = if retain {
        resolved.and_then(|key| cache.entries.get(&key))
    } else {
        None
    };
    Ok((model, lease))
}

#[cfg(test)]
fn preload_raw(
    request: &DecodeRequest,
    disk: &DiskMosaicCache,
    ram: &mut MosaicRamCache,
    writer: Option<&CacheWriter>,
    generation: u64,
    gate: &Arc<DecodeGate>,
) -> Result<bool, String> {
    preload_raw_with_lease(request, disk, ram, writer, generation, gate).map(|(admitted, _)| admitted)
}

fn preload_raw_with_lease(
    request: &DecodeRequest,
    disk: &DiskMosaicCache,
    ram: &mut MosaicRamCache,
    writer: Option<&CacheWriter>,
    generation: u64,
    gate: &Arc<DecodeGate>,
) -> Result<(bool, Option<rrrah_cache::CacheLease<rrrah_core::DecodedMosaic>>), String> {
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
    if cancelled() {
        return Err("RAW prefetch cancelled".into());
    }
    // Every valid nonempty u16 sensor requires at least one sample. A lower
    // bound avoids guessing dimensions and still detects a fully protected budget.
    if !ram.can_admit_key_by_minimum_weight(&key, 2) {
        return Ok((false, None));
    }
    if ram.get_background_with_cancel(&key, cancelled).is_some() {
        return Ok((true, ram.get_lease(&key)));
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
        Err(rrrah_cache::CacheError::Memory(_)) => return Ok((false, None)),
        _ => {
            if !prefetch_source_fits(request) {
                return Ok((false, None));
            }
            let mosaic = if recipe.decoder_backend_id() == rrrah_decode::NATIVE_CR3_BACKEND_ID {
                let prepared = match rrrah_decode::NativeCr3Decoder.prepare(request) {
                    Ok(prepared) => prepared,
                    Err(rrrah_decode::DecodeError::Memory(_)) => return Ok((false, None)),
                    Err(error) => return Err(error.to_string()),
                };
                if !ram.can_admit_key_by_minimum_weight(&key, prepared.minimum_pixel_bytes()) {
                    return Ok((false, None));
                }
                prepared.decode().map_err(|error| error.to_string())?.mosaic
            } else {
                NativeRawDecoder
                    .decode(request)
                    .map_err(|error| error.to_string())?
                    .mosaic
            };
            (mosaic, true)
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
    let lease = admitted.then(|| ram.get_lease(&key)).flatten();
    Ok((admitted, lease))
}

#[cfg(test)]
fn preload_raster(
    request: &DecodeRequest,
    window: Option<rrrah_decode::ScalarWindow>,
    cache: &mut RasterDisplayCache,
    gate: &Arc<DecodeGate>,
) -> Result<(rrrah_core::DecodedRaster, bool), String> {
    preload_raster_mode(request, window, cache, gate, false).map(|(frame, assumed, _)| (frame, assumed))
}

fn preload_raster_with_lease(
    request: &DecodeRequest,
    window: Option<rrrah_decode::ScalarWindow>,
    cache: &mut RasterDisplayCache,
    gate: &Arc<DecodeGate>,
) -> Result<
    (
        rrrah_core::DecodedRaster,
        bool,
        Option<rrrah_cache::CacheLease<rrrah_core::DecodedRaster>>,
    ),
    String,
> {
    preload_raster_mode(request, window, cache, gate, true)
}

fn preload_raster_mode(
    request: &DecodeRequest,
    window: Option<rrrah_decode::ScalarWindow>,
    cache: &mut RasterDisplayCache,
    gate: &Arc<DecodeGate>,
    retain: bool,
) -> Result<
    (
        rrrah_core::DecodedRaster,
        bool,
        Option<rrrah_cache::CacheLease<rrrah_core::DecodedRaster>>,
    ),
    String,
> {
    let mut resolved = None;
    let skip_unretained_swap = retain;
    // Explicit opt-in benchmark control; absent from production builds.
    #[cfg(test)]
    let skip_unretained_swap =
        skip_unretained_swap && std::env::var("RRRAH_BENCH_LEGACY_PREFETCH_SWAP").as_deref() != Ok("1");
    let (frame, assumed) = load_cached_raster_mode_resolving_key_policy(
        request,
        || {
            resolved = raster_display_key(request, window);
            resolved.clone()
        },
        cache,
        false,
        retain,
        skip_unretained_swap,
        || {
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
        },
    )?;
    let lease = if retain {
        resolved.and_then(|mut key| {
            key.6 = assumed;
            cache.get_lease(&key)
        })
    } else {
        None
    };
    Ok((frame, assumed, lease))
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
    Option<rrrah_decode::RlaFloatByteOrder>,
    (
        Option<rrrah_decode::RlaColorSpace>,
        Option<u32>,
        Option<(u32, u32, u32)>,
        Option<rrrah_decode::RpfRasterInterpretation>,
        bool,
    ),
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
// Keep the tuple layout stable while binding cached pixels to native external inputs.
fn raster_cache_source_fingerprint(request: &DecodeRequest) -> Option<SourceFingerprint> {
    use std::io::Read;
    let mut source = SourceFingerprint::from_path(&request.path).ok()?;
    if rrrah_decode::wal_palette_path(&request.path).is_none() {
        return Some(source);
    }
    let mut file = std::fs::File::open(&request.path).ok()?;
    let mut header = [0u8; 64];
    let mut length = 0;
    while length < header.len() {
        match file.read(&mut header[length..]) {
            Ok(0) => break,
            Ok(count) => length += count,
            Err(error) if error.kind() == std::io::ErrorKind::Interrupted => continue,
            Err(_) => return None,
        }
    }
    let dependencies = rrrah_decode::raster_external_dependencies(request, &header[..length]);
    if dependencies.is_empty() {
        return Some(source);
    }
    let mut hash = blake3::Hasher::new();
    hash.update(b"rrrah/raster-source-dependencies/v1\0");
    hash.update(&source.sampled_blake3);
    for path in dependencies {
        // Missing palettes must reach the decoder rather than an old RAM/swap entry.
        let dependency = SourceFingerprint::from_path(&path).ok()?;
        hash.update(&dependency.file_size.to_le_bytes());
        hash.update(&dependency.modified_ns.to_le_bytes());
        hash.update(&dependency.sampled_blake3);
    }
    source.sampled_blake3 = *hash.finalize().as_bytes();
    Some(source)
}

fn raster_display_key(
    request: &DecodeRequest,
    window: Option<rrrah_decode::ScalarWindow>,
) -> Option<RasterDisplayKey> {
    let fingerprint = raster_cache_source_fingerprint(request);
    fingerprint.map(|source| {
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
            request.rla_float_byte_order,
            (
                request.rla_color_space,
                request.pq_reference_white_nits.map(f32::to_bits),
                request
                    .hlg_display
                    .map(|(white, peak, gamma)| (white.to_bits(), peak.to_bits(), gamma.to_bits())),
                request.rpf_interpretation.clone(),
                request.pfm_top_down,
            ),
        )
    })
}
#[cfg(test)]
fn retain_prefetched_raster(
    request: &DecodeRequest,
    window: Option<rrrah_decode::ScalarWindow>,
    assumed: bool,
    cache: &mut RasterDisplayCache,
) -> Option<rrrah_cache::CacheLease<rrrah_core::DecodedRaster>> {
    let mut key = raster_display_key(request, window)?;
    key.6 = assumed;
    cache.get_lease(&key)
}

fn load_cached_raster_mode(
    request: &DecodeRequest,
    window: Option<rrrah_decode::ScalarWindow>,
    cache: &mut RasterDisplayCache,
    visible: bool,
    load: impl FnMut() -> Result<(rrrah_core::DecodedRaster, bool), RasterLoadError>,
) -> Result<(rrrah_core::DecodedRaster, bool), String> {
    load_cached_raster_mode_resolving_key(
        request,
        || raster_display_key(request, window),
        cache,
        visible,
        false,
        load,
    )
}

fn load_cached_raster_mode_resolving_key(
    request: &DecodeRequest,
    resolve_key: impl FnOnce() -> Option<RasterDisplayKey>,
    cache: &mut RasterDisplayCache,
    visible: bool,
    require_retention: bool,
    load: impl FnMut() -> Result<(rrrah_core::DecodedRaster, bool), RasterLoadError>,
) -> Result<(rrrah_core::DecodedRaster, bool), String> {
    load_cached_raster_mode_resolving_key_policy(
        request,
        resolve_key,
        cache,
        visible,
        require_retention,
        false,
        load,
    )
}

fn load_cached_raster_mode_resolving_key_policy(
    request: &DecodeRequest,
    resolve_key: impl FnOnce() -> Option<RasterDisplayKey>,
    cache: &mut RasterDisplayCache,
    visible: bool,
    require_retention: bool,
    skip_unretained_swap: bool,
    mut load: impl FnMut() -> Result<(rrrah_core::DecodedRaster, bool), RasterLoadError>,
) -> Result<(rrrah_core::DecodedRaster, bool), String> {
    if request
        .cancellation
        .as_ref()
        .is_some_and(|token| token.is_cancelled())
    {
        return Err("raster load cancelled".into());
    }
    let key = resolve_key();
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
            #[cfg(debug_assertions)]
            let qualification_reads = (visible && gpu_recovery_probe::traces_raster_cache(&request.path))
                .then(|| cache.swap().map_or(0, |swap| swap.stats().reads));
            let hit = if visible {
                cache.get_with_cancel(&probe, cancelled)
            } else if skip_unretained_swap && !cache.can_admit_key_by_minimum_weight(&probe, 4) {
                None
            } else {
                cache.get_background_with_cancel(&probe, cancelled)
            };
            if cancelled() {
                return Err("raster load cancelled".into());
            }
            if let Some(raster) = hit {
                #[cfg(debug_assertions)]
                if let Some(before) = qualification_reads {
                    let after = cache.swap().map_or(0, |swap| swap.stats().reads);
                    gpu_recovery_probe::record_raster_cache_hit(&request.path, before, after);
                }
                if visible {
                    cache.mark_visible(probe);
                }
                return Ok((raster, assumed));
            }
        }
    }
    if require_retention
        && !visible
        && let Some(key) = &key
    {
        let fits_count = [false, true].into_iter().any(|assumed| {
            let mut probe = key.clone();
            probe.6 = assumed;
            cache.can_admit_key_by_count(&probe)
        });
        if !fits_count {
            return Err("raster prefetch skipped: all count slots are protected".into());
        }
    }
    if skip_unretained_swap
        && !visible
        && let Some(key) = &key
        && ![false, true].into_iter().any(|assumed| {
            let mut probe = key.clone();
            probe.6 = assumed;
            cache.can_admit_key_by_minimum_weight(&probe, 4)
        })
    {
        return Err("raster prefetch skipped: protected byte budget exhausted".into());
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
            Self::Decode(rrrah_decode::RasterDecodeError::Eps(error)) => error.memory_error(),
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
    #[cfg(debug_assertions)]
    gpu_recovery_probe::check_raster_decode(&request.path).map_err(RasterLoadError::Message)?;
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
    selected_raster_prefetch: &mut Option<(usize, bool)>,
) {
    let LoadRequest {
        prefetch_paths: _,
        raster_prefetch_hint: _,
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
                Ok((raster, assumed_srgb)) => {
                    *selected_raster_prefetch = Some((
                        raster.image_count(),
                        raster
                            .animation_timing()
                            .is_some_and(|timing| timing.num_plays() != 1),
                    ));
                    publish_load_event(
                        LoadEvent::RasterReady {
                            region_store: raster_cache.swap().map(|swap| swap.region_store()),
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
                    );
                }
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
    validation: Option<source_validation::Check>,
}

impl DeferredThumbnail {
    fn matches_tile(&self, tiles: &[gallery::FolderTile]) -> bool {
        tiles
            .get(self.ready.index)
            .is_some_and(|tile| tile.folder == self.folder && tile.cover == self.cover)
    }
    fn poll_source(&mut self, validator: &source_validation::SourceValidator) -> Option<bool> {
        let check = self
            .validation
            .get_or_insert_with(|| validator.submit(self.cover.clone(), self.ready.source_stamp.clone()));
        check.poll()
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
    editor: editor::Editor,
    accesskit: Option<accesskit_winit::Adapter>,
    access_focus: accesskit::NodeId,
    event_proxy: Option<EventLoopProxy<WakeEvent>>,
    before: bool,
    ready_generation: Option<u64>,
    playback: raster_playback::Playback,
    command_focus_visible: bool,
    split_comparison: bool,
    histogram_worker: histogram_worker::HistogramWorker,
    histogram: Option<rrrah_core::histogram::Histogram>,
    path: PathBuf,
    receiver: Receiver<LoadEvent>,
    foreground_loader: ForegroundLoader,
    folder_scanner: gallery::FolderScanner,
    pending_folder_scan: Option<(u64, bool)>,
    drop_inspector: drop_inspection::DropInspector,
    pending_drop: Option<u64>,
    gallery: Vec<PathBuf>,
    gallery_index: Option<usize>,
    image_index: usize,
    pending_image_index: Option<(u64, usize)>,
    image_count: usize,
    scalar_window: Option<rrrah_decode::ScalarWindow>,
    last_nav_direction: NavDirection,
    raw_prefetcher: gallery::RawPrefetcher,
    prefetch_window: gallery::PrefetchWindow,
    strip: FolderStrip,
    strip_thumbs: ThumbCache,
    thumbnail_prefetcher: gallery::Prefetcher,
    thumbnail_validator: source_validation::SourceValidator,
    deferred_thumbnail: Option<DeferredThumbnail>,
    cache_telemetry: Arc<CacheTelemetry>,
    window: Option<Arc<Window>>,
    gpu: Option<GpuState>,
    #[cfg(debug_assertions)]
    recovery_probe: Option<gpu_recovery_probe::Probe>,
    last_gpu_recovery: Option<Instant>,
    gpu_recovery_generation: Option<u64>,
    raster_gpu_budget: Option<rrrah_core::MemoryBudget>,
    raster_lod_budget: rrrah_core::MemoryBudget,
    raster_lod_worker: raster_lod_worker::LodWorker,
    pending_raster_lod: Option<(u64, u64, LoadEvent)>,
    model_upload_worker: model_upload_worker::ModelUploadWorker,
    pending_model_upload: Option<(u64, u64, LoadEvent, Instant)>,
    model_gpu_budget: Option<rrrah_core::MemoryBudget>,
    raw_gpu_budget: Option<rrrah_core::MemoryBudget>,
    filmstrip_gpu_budget: Option<rrrah_core::MemoryBudget>,
    gpu_backend: rrrah_gpu::GpuBackend,
    gpu_vendor: rrrah_gpu::GpuVendor,
    gpu_demosaic: rrrah_gpu::GpuDemosaicAlgorithm,
    gpu_raw_compute: bool,
    upload_memory_budget: Option<rrrah_core::MemoryBudget>,
    telemetry_window: Option<Arc<Window>>,
    telemetry: Option<TelemetryState>,
    last_telemetry_recovery: Option<Instant>,
    telemetry_recovery_view: Option<[f32; 2]>,
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
        raw_disk_prefetch_window: gallery::PrefetchWindow,
        disk_limits: rrrah_cache::CacheLimits,
        managed_budget: Option<rrrah_cache::MemoryBudget>,
        raster_gpu_budget: Option<rrrah_core::MemoryBudget>,
        model_gpu_budget: Option<rrrah_core::MemoryBudget>,
        raw_gpu_budget: Option<rrrah_core::MemoryBudget>,
        filmstrip_gpu_budget: Option<rrrah_core::MemoryBudget>,
        gpu_backend: rrrah_gpu::GpuBackend,
        gpu_vendor: rrrah_gpu::GpuVendor,
        gpu_demosaic: rrrah_gpu::GpuDemosaicAlgorithm,
        untagged_color: RasterInterpretation,
    ) -> Self {
        let thumbnail_prefetcher = gallery::Prefetcher::new_with_cancel(
            32,
            thumbnail_loader_with_cancel(Arc::clone(&decode_gate), managed_budget.clone(), untagged_color),
        );
        let edit_journal_root = cache_root
            .clone()
            .or_else(default_cache_dir)
            .unwrap_or_else(std::env::temp_dir)
            .join("edit-journal");
        let raw_prefetcher = gallery::RawPrefetcher::with_budget(
            cache_root,
            no_cache,
            decode_gate,
            Arc::clone(&cache_telemetry),
            raw_disk_prefetch_window,
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
            folder_scanner: gallery::FolderScanner::new(),
            pending_folder_scan: None,
            drop_inspector: drop_inspection::DropInspector::new(),
            pending_drop: None,
            gallery: Vec::new(),
            gallery_index: None,
            image_index: 0,
            pending_image_index: None,
            image_count: 1,
            scalar_window: None,
            last_nav_direction: NavDirection::None,
            raw_prefetcher,
            prefetch_window,
            strip: FolderStrip::default(),
            strip_thumbs: ThumbCache::new(THUMB_CACHE_CAPACITY),
            thumbnail_prefetcher,
            thumbnail_validator: source_validation::SourceValidator::new(),
            deferred_thumbnail: None,
            cache_telemetry,
            window: None,
            gpu: None,
            #[cfg(debug_assertions)]
            recovery_probe: gpu_recovery_probe::Probe::from_env(),
            last_gpu_recovery: None,
            gpu_recovery_generation: None,
            raster_gpu_budget,
            raster_lod_worker: raster_lod_worker::LodWorker::default(),
            pending_raster_lod: None,
            model_upload_worker: model_upload_worker::ModelUploadWorker::default(),
            pending_model_upload: None,
            raster_lod_budget: managed_budget.as_ref().map_or_else(
                || rrrah_core::MemoryBudget::new(16 * 1024 * 1024),
                |root| root.child(16 * 1024 * 1024),
            ),
            model_gpu_budget,
            raw_gpu_budget,
            filmstrip_gpu_budget,
            gpu_backend,
            gpu_vendor,
            gpu_demosaic,
            gpu_raw_compute: false,
            editor: editor::Editor::new(
                managed_budget.as_ref().map_or_else(
                    || rrrah_core::MemoryBudget::new(1024 * 1024),
                    |b| b.child(1024 * 1024),
                ),
                edit_journal_root,
            ),
            before: false,
            ready_generation: None,
            playback: Default::default(),
            command_focus_visible: false,
            split_comparison: false,
            histogram_worker: histogram_worker::HistogramWorker::default(),
            histogram: None,
            accesskit: None,
            access_focus: accesskit::NodeId(10),
            event_proxy: None,
            upload_memory_budget: managed_budget,
            telemetry_window: None,
            telemetry: None,
            last_telemetry_recovery: None,
            telemetry_recovery_view: None,
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
        // A worker wake can carry a viewport replacement without a loader event.
        if self
            .gpu
            .as_ref()
            .is_some_and(|gpu| gpu.viewport_worker.has_completed() || gpu.storage_worker.has_completed())
        {
            self.update_view();
        }
        loop {
            let current = self.foreground_loader.current_generation();
            if self
                .pending_raster_lod
                .as_ref()
                .is_some_and(|(_, generation, _)| *generation != current)
            {
                self.pending_raster_lod = None;
                self.raster_lod_worker.cancel();
            }
            if self
                .pending_model_upload
                .as_ref()
                .is_some_and(|(_, g, _, _)| *g != current)
            {
                self.pending_model_upload = None;
                self.model_upload_worker.cancel();
            }
            let mut prepared_model = None;
            let mut prepared_model_renderer = None;
            let mut model_upload_started = None;
            let mut prepared_lod = None;
            let mut completed_event = None;
            if let Some(completed) = self.model_upload_worker.try_take() {
                if self.pending_model_upload.as_ref().is_some_and(|(id, g, _, _)| {
                    *id == completed.id && *g == completed.generation && *g == current
                }) {
                    let (_, _, event, started) = self.pending_model_upload.take().unwrap();
                    match completed.result {
                        Ok(prepared) => {
                            prepared_model = Some(prepared);
                            prepared_model_renderer = completed.renderer;
                            model_upload_started = Some(started);
                            completed_event = Some(event);
                        }
                        Err(error) => {
                            self.pending_first_present = None;
                            self.pipeline = PipelineSnapshot::idle(current);
                            if let Some(telemetry) = self.telemetry.as_mut() {
                                telemetry.set_pipeline(&self.pipeline);
                            }
                            self.set_status(format!("GPU model preparation failed: {error}"));
                            continue;
                        }
                    }
                }
            }
            if let Some(completed) = completed_event
                .is_none()
                .then(|| self.raster_lod_worker.try_take())
                .flatten()
            {
                if self.pending_raster_lod.as_ref().is_some_and(|(id, g, _)| {
                    *id == completed.id && *g == completed.generation && *g == current
                }) {
                    let (_, _, event) = self.pending_raster_lod.take().unwrap();
                    match completed.into_frame() {
                        Ok(lod) => {
                            prepared_lod = Some(lod);
                            completed_event = Some(event);
                        }
                        Err(error) => {
                            self.set_status(format!("HDR level preparation failed: {error}"));
                            continue;
                        }
                    }
                }
            }
            let event = match completed_event {
                Some(event) => event,
                None => match self.receiver.try_recv() {
                    Ok(event) => event,
                    Err(TryRecvError::Empty) | Err(TryRecvError::Disconnected) => break,
                },
            };
            if prepared_model.is_none() {
                if let LoadEvent::ModelReady { generation, mesh, .. } = &event {
                    if *generation != current {
                        continue;
                    }
                    if let Some(gpu) = self.gpu.as_mut() {
                        if gpu.config.format.is_srgb()
                            || (gpu.config.format == wgpu::TextureFormat::Rgba16Float
                                && gpu.config.color_space == wgpu::SurfaceColorSpace::ExtendedSrgbLinear)
                        {
                            let started = Instant::now();
                            let token =
                                GenerationToken::new(self.foreground_loader.generation.clone(), *generation);
                            let submitted = match gpu.model.as_ref() {
                                Some(renderer) => self.model_upload_worker.submit(
                                    mesh.clone(),
                                    renderer.upload_preparer(),
                                    *generation,
                                    token,
                                ),
                                None => self.model_upload_worker.submit_new(
                                    mesh.clone(),
                                    gpu.device.clone(),
                                    gpu.config.format,
                                    self.model_gpu_budget.clone(),
                                    *generation,
                                    token,
                                ),
                            };
                            match submitted {
                                Ok(id) => {
                                    self.pending_model_upload = Some((id, *generation, event, started));
                                    self.set_status("preparing model on GPU".into());
                                }
                                Err(error) => {
                                    self.pending_first_present = None;
                                    self.pipeline = PipelineSnapshot::idle(current);
                                    if let Some(telemetry) = self.telemetry.as_mut() {
                                        telemetry.set_pipeline(&self.pipeline);
                                    }
                                    self.set_status(format!("model worker unavailable: {error}"));
                                }
                            }
                            continue;
                        }
                    }
                }
            }
            // Plan with current device headroom here; all pixel filtering runs on the
            // replaceable background worker, retaining the original event/consumer lease.
            if prepared_lod.is_none() {
                if let LoadEvent::RasterReady {
                    generation, raster, ..
                } = &event
                {
                    if *generation != current {
                        continue;
                    }
                    if let Some(gpu) = self.gpu.as_ref() {
                        if let Some(renderer) = gpu.raster.as_ref() {
                            let view = ViewParameters {
                                viewport: [gpu.size.width as f32, gpu.size.height as f32],
                                ..self.view
                            };
                            let region = rrrah_gpu::RasterRenderer::visible_source_region(raster, view);
                            let region_fits = region.as_ref().is_ok_and(|r| {
                                r.is_none_or(|r| renderer.can_admit_texture_extent(&gpu.device, r[2], r[3]))
                            });
                            if !renderer.can_admit_texture_extent(
                                &gpu.device,
                                raster.width(),
                                raster.height(),
                            ) && !region_fits
                            {
                                let size = match raster_viewport::ResidentSource::plan_lod_size(
                                    raster,
                                    &self.raster_lod_budget,
                                    renderer,
                                    &gpu.device,
                                ) {
                                    Ok(size) => size,
                                    Err(error) => {
                                        self.set_status(format!("HDR level unavailable: {error}"));
                                        continue;
                                    }
                                };
                                let token = GenerationToken::new(
                                    Arc::clone(&self.foreground_loader.generation),
                                    *generation,
                                );
                                match self.raster_lod_worker.submit(
                                    raster.clone(),
                                    size,
                                    self.raster_lod_budget.clone(),
                                    *generation,
                                    token,
                                ) {
                                    Ok(id) => {
                                        self.pending_raster_lod = Some((id, *generation, event));
                                        self.set_status("preparing HDR level".into());
                                        continue;
                                    }
                                    Err(error) => {
                                        self.set_status(format!("HDR worker unavailable: {error}"));
                                        continue;
                                    }
                                }
                            }
                        }
                    }
                }
            }
            match &event {
                LoadEvent::Progress { .. } => {}
                LoadEvent::Ready { generation, .. }
                | LoadEvent::RasterReady { generation, .. }
                | LoadEvent::ModelReady { generation, .. }
                | LoadEvent::Failed { generation, .. } => {
                    finish_foreground_flight(&self.foreground_loader.flight, *generation);
                    if *generation == self.foreground_loader.current_generation() {
                        self.pending_image_index = None;
                    }
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
                    let upload_token =
                        GenerationToken::new(Arc::clone(&self.foreground_loader.generation), generation);
                    let gpu_prepare_result: Option<Result<(GpuUploadTimings, Duration), String>> =
                        if let Some(gpu) = self.gpu.as_mut() {
                            let use_compute = self.gpu_raw_compute
                                && mosaic.metadata.width >= 2
                                && mosaic.metadata.height >= 2
                                && mosaic
                                    .metadata
                                    .cfa
                                    .as_ref()
                                    .is_some_and(|cfa| cfa.bayer_quad().is_ok());
                            let upload = if use_compute {
                                gpu.upload_bayer_compute(&mosaic, self.gpu_demosaic, || {
                                    upload_token.is_cancelled()
                                })
                                .map_err(|e| e.to_string())
                            } else {
                                let upload = gpu
                                    .prepare_raw_renderer(self.view)
                                    .map_err(|e| e.to_string())
                                    .and_then(|()| {
                                        gpu.renderer
                                            .as_mut()
                                            .expect("RAW renderer prepared")
                                            .upload_mosaic_with_cancel(
                                                &gpu.device,
                                                &gpu.queue,
                                                &mosaic,
                                                || upload_token.is_cancelled(),
                                            )
                                            .map_err(|e| e.to_string())
                                    });
                                upload
                            };
                            match upload {
                                Ok(upload_timings) => {
                                    gpu.upload_completions.track_submitted(&gpu.queue);
                                    // Transfer the consumer lease through actual GPU completion.
                                    // Rejected or stale events release theirs automatically.
                                    if let Some(lease) = lease {
                                        submit_upload_lease(&gpu.queue, &gpu.upload_completions, lease);
                                    }
                                    self.image_index = 0;
                                    self.ready_generation = Some(generation);
                                    self.image_count = 1;
                                    gpu.raster_active = use_compute;
                                    gpu.raster_viewport = None;
                                    gpu.raster_region_error = None;
                                    gpu.model_active = false;
                                    if let Some(model) = gpu.model.as_mut() {
                                        model.clear_model();
                                    }
                                    if use_compute {
                                        if let Some(renderer) = gpu.renderer.as_mut() {
                                            renderer.clear_image();
                                        }
                                    } else if let Some(raster) = gpu.raster.as_mut() {
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
                    let upload_started = model_upload_started.unwrap_or_else(Instant::now);
                    let result = self.gpu.as_mut().map(|gpu| {
                        if let Some(renderer) = prepared_model_renderer.take() {
                            gpu.model = Some(renderer);
                        }
                        let renderer = gpu
                            .model
                            .as_mut()
                            .ok_or_else(|| "surface has no supported linear model target".to_string())?;
                        renderer
                            .resize(&gpu.device, [gpu.size.width.max(1), gpu.size.height.max(1)])
                            .map_err(|e| e.to_string())?;
                        let prepared = prepared_model
                            .take()
                            .ok_or_else(|| "model preparation unavailable".to_string())?;
                        renderer
                            .install_prepared_upload(prepared)
                            .map_err(|e| e.to_string())?;
                        if let Some(lease) = lease {
                            submit_upload_lease(&gpu.queue, &gpu.upload_completions, lease);
                        }
                        if let Some(renderer) = &mut gpu.renderer {
                            renderer.clear_image();
                        }
                        if let Some(raster) = gpu.raster.as_mut() {
                            raster.clear_image();
                        }
                        gpu.raster_active = false;
                        gpu.raster_viewport = None;
                        gpu.raster_region_error = None;
                        gpu.model_active = true;
                        if self.gpu_recovery_generation != Some(generation) {
                            self.view = ViewParameters {
                                viewport: [gpu.size.width as f32, gpu.size.height as f32],
                                ..ViewParameters::default()
                            };
                        }
                        gpu.update_view(self.view);
                        Ok::<(), String>(())
                    });
                    self.status = match result {
                        Some(Ok(())) => {
                            self.image_index = 0;
                            self.ready_generation = Some(generation);
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
                    region_store,
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
                    let upload_token =
                        GenerationToken::new(Arc::clone(&self.foreground_loader.generation), generation);
                    self.histogram = None;
                    if raster.color_space() == &rrrah_core::RasterColorSpace::LinearSrgb {
                        let token =
                            GenerationToken::new(Arc::clone(&self.foreground_loader.generation), generation);
                        if let Err(error) = self.histogram_worker.submit(raster.pixels().clone(), token) {
                            log::warn!("histogram unavailable: {error}");
                        }
                    }
                    let result = self.gpu.as_mut().map(|gpu| {
                        let renderer = gpu
                            .raster
                            .as_mut()
                            .ok_or_else(|| "surface has no sRGB target".to_string())?;
                        self.view.viewport = [gpu.size.width as f32, gpu.size.height as f32];
                        let retained = raster_viewport::ResidentSource::install_with_prepared_lod(
                            renderer,
                            &gpu.device,
                            &gpu.queue,
                            &raster,
                            self.view,
                            development.as_ref().map(|options| &options.curve),
                            &self.raster_lod_budget,
                            prepared_lod.take(),
                            || upload_token.is_cancelled(),
                        )
                        .map_err(|e| e.to_string())?;
                        gpu.raster_viewport = retained;
                        gpu.storage_worker.cancel();
                        gpu.storage_pending = None;
                        if let (Some(store), Some(source)) = (region_store, gpu.raster_viewport.as_ref()) {
                            if let Some((pixels, index)) = source.persistence_source() {
                                let identity = source.identity();
                                gpu.viewport_generation
                                    .store(identity, std::sync::atomic::Ordering::Release);
                                let token = GenerationToken::new(gpu.viewport_generation.clone(), identity);
                                match gpu
                                    .storage_worker
                                    .submit_swap(pixels, store, index, identity, token)
                                {
                                    Ok(id) => gpu.storage_pending = Some((id, identity)),
                                    Err(error) => log::warn!("visible raster swap unavailable: {error}"),
                                }
                            }
                        }
                        gpu.raster_region_error = None;
                        gpu.upload_completions.track_submitted(&gpu.queue);
                        if let Some(lease) = lease {
                            submit_upload_lease(&gpu.queue, &gpu.upload_completions, lease);
                        }
                        if let Some(options) = development.as_ref() {
                            renderer.set_raw_development(&gpu.queue, &options.curve);
                        }
                        if let Some(renderer) = &mut gpu.renderer {
                            renderer.clear_image();
                        }
                        gpu.raster_active = true;
                        gpu.model_active = false;
                        if let Some(model) = gpu.model.as_mut() {
                            model.clear_model();
                        }
                        self.view.viewport = [gpu.size.width as f32, gpu.size.height as f32];
                        gpu.update_view(self.view);
                        Ok::<(), String>(())
                    });
                    // A superseded upload must not overwrite the current request's status
                    // or start neighbour work from this stale ready event.
                    if upload_token.is_cancelled() {
                        continue;
                    }
                    self.status = match result {
                        Some(Ok(())) => {
                            log::info!(
                                "Raster ready: generation={generation} path={} index={image_index} dimensions={}x{} elapsed={elapsed:?}",
                                self.path.display(),
                                raster.width(),
                                raster.height()
                            );
                            self.image_index = image_index;
                            self.ready_generation = Some(generation);
                            self.scalar_window = scalar_window;
                            self.image_count = raster.image_count();
                            self.playback.ready(
                                generation,
                                image_index,
                                raster.image_count(),
                                raster.animation_timing(),
                            );
                            let paused = if self.playback.is_paused(generation) {
                                "; animation paused (P to resume)"
                            } else {
                                ""
                            };
                            let page = if raster.image_count() > 1 {
                                format!(" ({} of {})", image_index + 1, raster.image_count())
                            } else {
                                String::new()
                            };
                            format!(
                                "image {}x{}{page} ready in {elapsed:.2?}{}{}{paused}",
                                raster.width(),
                                raster.height(),
                                if assumed_srgb { "; assumed sRGB" } else { "" },
                                scalar_window
                                    .map(|w| format!("; window {}..{}", w.minimum(), w.maximum()))
                                    .unwrap_or_default()
                            )
                        }
                        Some(Err(error)) => {
                            log::warn!(
                                "Raster GPU upload rejected: generation={generation} path={} index={image_index} error={error}",
                                self.path.display()
                            );
                            format!("GPU upload rejected image: {error}")
                        }
                        None => "image ready; waiting for window".into(),
                    };
                    if let Some(telemetry) = self.telemetry.as_mut() {
                        telemetry.set_pipeline(&self.pipeline);
                    }
                    self.submit_raw_neighbours();
                }
                LoadEvent::Failed { generation, error } => {
                    if generation == self.foreground_loader.current_generation() {
                        log::warn!(
                            "Image load failed: generation={generation} path={} error={error}",
                            self.path.display()
                        );
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
            if self.gpu_recovery_generation.is_some() && self.gpu_recovery_generation == self.ready_generation
            {
                self.gpu_recovery_generation = None;
            }
            if let Some(window) = &self.window {
                window.set_title(&self.window_title());
                window.request_redraw();
            }
        }
    }

    fn window_title(&self) -> String {
        let command = if self.command_focus_visible {
            accessibility::COMMANDS
                .get(self.access_focus.0.saturating_sub(accessibility::FIRST_COMMAND) as usize)
                .map_or_else(String::new, |label| format!("{label} (Enter) — "))
        } else {
            String::new()
        };
        format!("Rrrah — {command}{}", self.status)
    }
    fn accessibility_tree(&self) -> accesskit::TreeUpdate {
        let name = format!(
            "{}; image {} of {}; exposure {:+.2} stops{}",
            display_name(&self.path),
            self.image_index + 1,
            self.image_count,
            self.view.exposure_stops,
            if self.before { "; before" } else { "" }
        );
        let folder = self
            .strip
            .current
            .and_then(|i| self.strip.tiles.get(i))
            .map(|t| t.folder.display().to_string());
        accessibility::tree(&name, &self.status, folder.as_deref(), self.access_focus)
    }
    fn update_accessibility(&mut self) {
        if self.accesskit.is_some() {
            let tree = self.accessibility_tree();
            self.accesskit.as_mut().unwrap().update_if_active(|| tree);
        }
    }
    fn access_command(&mut self, id: accesskit::NodeId) {
        match id.0 {
            10 => self.navigate_gallery(-1),
            11 => self.navigate_gallery(1),
            12 => self.set_exposure((self.editor.current().exposure_stops - 0.25).max(-10.)),
            13 => self.set_exposure((self.editor.current().exposure_stops + 0.25).min(10.)),
            14 => {
                self.undo_redo_edit(false);
            }
            15 => {
                self.undo_redo_edit(true);
            }
            16 => self.save_edits(),
            17 => {
                self.before = !self.before;
                self.update_view();
            }
            18 => self.resolve_edit_conflict(true),
            19 => self.resolve_edit_conflict(false),
            20 => self.restart_refused_recovery(),
            _ => {}
        }
    }
    fn sync_editor(&mut self) {
        if self.ready_generation != Some(self.foreground_loader.current_generation()) {
            return;
        }
        if self.path.as_os_str().is_empty() {
            return;
        }
        match self.editor.activate_version(
            self.path.clone(),
            self.image_index,
            self.foreground_loader.current_generation(),
        ) {
            Ok(true) => {
                self.before = false;
            }
            Ok(false) => {}
            Err(error) => {
                self.status = format!("edit session unavailable: {error:#}");
            }
        }
    }
    fn resolve_edit_conflict(&mut self, use_recovery: bool) {
        if self.ready_generation != Some(self.foreground_loader.current_generation()) {
            self.set_status("wait for the requested image before resolving edits".into());
            return;
        }
        self.sync_editor();
        match self.editor.resolve_conflict(use_recovery) {
            Ok(true) => self.set_status(if use_recovery {
                "using recovered edits; Ctrl/Cmd+S saves them to XMP".into()
            } else {
                "using saved XMP edits; updating recovery snapshot".into()
            }),
            Ok(false) => self.set_status("no recovery/XMP conflict to resolve".into()),
            Err(error) => self.set_status(format!("edit conflict retained: {error:#}")),
        }
        self.update_view();
    }
    fn restart_refused_recovery(&mut self) {
        if self.ready_generation != Some(self.foreground_loader.current_generation()) {
            self.set_status("wait for the requested image before restarting edits".into());
            return;
        }
        self.sync_editor();
        match self.editor.request_restart_recovery() {
            Ok(true) => self.set_status("retaining refused recovery before starting clean edits…".into()),
            Ok(false) => self.set_status("no refused recovery ready to restart".into()),
            Err(error) => self.set_status(format!("recovery retained; restart refused: {error:#}")),
        }
        self.update_view();
    }
    fn edit_ready(&mut self) -> bool {
        if self.ready_generation != Some(self.foreground_loader.current_generation()) {
            self.set_status("wait for the requested image to finish loading before editing".into());
            return false;
        }
        self.sync_editor();
        if !self.editor.is_ready() {
            self.set_status(self.editor.blocked_message().into());
            return false;
        }
        true
    }
    fn undo_redo_edit(&mut self, redo: bool) {
        if self.edit_ready() {
            self.editor.undo_redo(redo);
            self.update_view();
        }
    }
    fn save_edits(&mut self) {
        if !self.edit_ready() {
            return;
        }
        if self.editor.request_save() {
            self.set_status("saving XMP edits…".into());
        } else {
            self.set_status("edits cannot be saved: resolve source/recovery errors first".into());
        }
    }
    fn set_exposure(&mut self, exposure: f32) {
        if !self.edit_ready() {
            return;
        }
        self.sync_editor();
        if let Err(error) = self.editor.apply_exposure(exposure) {
            self.set_status(format!("edit refused: {error:#}"));
        }
        self.update_view();
    }
    fn poll_editor_without_view(&mut self) {
        let (_, message) = self.editor.poll();
        if let Some(message) = message {
            self.status = message;
        }
    }
    fn poll_editor(&mut self) {
        let (changed, message) = self.editor.poll();
        if let Some(message) = message {
            self.set_status(message);
        }
        if changed {
            self.update_view();
        }
    }
    fn update_view(&mut self) {
        self.poll_editor_without_view();
        self.sync_editor();
        self.view.exposure_stops = if self.before {
            self.editor.original().exposure_stops
        } else {
            self.editor.current().exposure_stops
        };
        if let Some(gpu) = self.gpu.as_mut() {
            gpu.update_view(self.view);
            let split = if self.split_comparison && !self.before {
                0.5
            } else {
                0.
            };
            gpu.update_comparison(split, self.editor.original().exposure_stops);
            if let Some(error) = gpu.raster_region_error.take() {
                self.status = error;
            } else if self.status.starts_with("Image region unavailable:") {
                self.status = "image region ready".into();
            }
        }
        self.update_accessibility();
        // Uniform writes alone do not wake the event-driven renderer. Exposure,
        // zoom, pan and reset must present their new state without another event.
        if let Some(window) = &self.window {
            window.set_title(&self.window_title());
            window.request_redraw();
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
        self.request_folder_scan(folder, false);
    }

    fn request_folder_scan(&mut self, folder: PathBuf, open_first: bool) {
        let generation = self
            .folder_scanner
            .submit_with_sources(folder, self.strip_thumbs.source_snapshot());
        self.pending_folder_scan = Some((generation, open_first));
    }

    fn drain_folder_scan(&mut self) {
        // Keep earlier folder context until the drop is accepted or rejected.
        // An invalid drop must not lose an already running sibling scan.
        if self.pending_drop.is_some() {
            return;
        }
        let ready = match self.folder_scanner.try_recv() {
            Ok(Some(ready)) => ready,
            Ok(None) => return,
            Err(error) => {
                if self.pending_folder_scan.take().is_some() {
                    self.set_status(format!("folder scan failed: {error}"));
                }
                return;
            }
        };
        let Some((generation, open_first)) = self.pending_folder_scan else {
            return;
        };
        if ready.generation != generation {
            return;
        }
        self.pending_folder_scan = None;
        let has_paths = !ready.paths.is_empty();
        if !has_paths && open_first {
            self.set_status(format!("no supported images in {}", ready.folder.display()));
        }
        // A completed scan replaces the navigation context even when empty.
        // Retaining the previous list would navigate/preload an unrelated folder.
        self.gallery_index = ready.paths.iter().position(|candidate| candidate == &self.path);
        self.gallery = ready.paths;
        for id in self.strip_thumbs.invalidate_sources(ready.stale_thumbnails) {
            if let Some(gpu) = self.gpu.as_mut() {
                gpu.filmstrip.remove_tile(id);
            }
        }
        self.strip
            .set_folder(ready.tiles, &ready.folder, self.view.viewport[0]);
        self.submit_thumb_jobs();
        if open_first && has_paths {
            self.last_nav_direction = NavDirection::None;
            self.open_gallery_index(0);
        }
        if let Some(window) = &self.window {
            window.request_redraw();
        }
    }

    fn submit_thumb_jobs(&mut self) {
        self.deferred_thumbnail = None;
        let jobs = self
            .strip
            .tiles
            .iter()
            .enumerate()
            .filter(|(_, tile)| !self.strip_thumbs.contains(&tile.folder))
            .map(|(index, tile)| gallery::ThumbnailJob {
                index,
                source: tile.cover.clone(),
                edge: gallery::THUMB_EDGE,
            });
        self.thumbnail_prefetcher.submit(jobs);
    }

    /// Open a different folder (filmstrip click or dropped directory): scan
    /// it, retarget the strip and load its first image.
    fn open_folder(&mut self, folder: PathBuf) {
        self.cancel_drop();
        self.foreground_loader.cancel_pending();
        self.raw_prefetcher.begin_foreground();
        self.request_folder_scan(folder, true);
    }

    /// Drain finished cover thumbnails, upload them to the strip and free
    /// LRU-evicted textures.
    fn drain_thumbnails(&mut self) {
        loop {
            let now = Instant::now();
            let (ready, expires_at) = if let Some(mut deferred) = self.deferred_thumbnail.take() {
                if deferred.expired(now) {
                    continue;
                }
                if !deferred.due(now) {
                    self.deferred_thumbnail = Some(deferred);
                    break;
                }
                let matches = deferred.matches_tile(&self.strip.tiles);
                if !matches {
                    continue;
                }
                match deferred.poll_source(&self.thumbnail_validator) {
                    Some(true) => {}
                    Some(false) => continue,
                    None => {
                        deferred.retry_at = now + Duration::from_millis(25);
                        self.deferred_thumbnail = Some(deferred);
                        break;
                    }
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
                            validation: None,
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
                            validation: None,
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
        self.last_nav_direction = NavDirection::None;
        // Earlier scan results remain queued until this intent is resolved.
        self.pending_drop = Some(self.drop_inspector.submit(path.clone()));
        self.set_status(format!("checking dropped path: {}", display_name(&path)));
    }

    fn cancel_drop(&mut self) {
        self.drop_inspector.cancel();
        self.pending_drop = None;
    }

    fn drain_dropped_paths(&mut self) {
        let ready = match self.drop_inspector.try_recv() {
            Ok(Some(ready)) => ready,
            Ok(None) => return,
            Err(error) => {
                if self.pending_drop.take().is_some() {
                    self.set_status(format!("drop inspection failed: {error}"));
                }
                return;
            }
        };
        if self.pending_drop != Some(ready.generation) {
            return;
        }
        self.pending_drop = None;
        match ready.kind {
            Ok(drop_inspection::DropKind::Folder) => self.open_folder(ready.path),
            Ok(drop_inspection::DropKind::File) => {
                let path = ready.path;
                self.gallery = vec![path.clone()];
                if let Some(parent) = path.parent().map(Path::to_path_buf) {
                    self.request_folder_scan(parent, false);
                }
                self.open_gallery_index(0);
            }
            Err(error) => self.set_status(format!("drop rejected: {error}")),
        }
    }

    fn open_gallery_index(&mut self, index: usize) {
        let Some(path) = self.gallery.get(index).cloned() else {
            return;
        };
        self.cancel_drop();
        self.playback.reset();
        self.gallery_index = Some(index);
        self.image_index = 0;
        self.pending_image_index = None;
        self.image_count = 1;
        self.scalar_window = None;
        self.raw_prefetcher.begin_foreground();
        self.path.clone_from(&path);
        self.histogram_worker.cancel();
        self.histogram = None;
        if let Some(gpu) = self.gpu.as_ref() {
            self.view = ViewParameters {
                viewport: [gpu.size.width as f32, gpu.size.height as f32],
                ..ViewParameters::default()
            };
        }
        let neighbours = self.pending_folder_scan.is_none().then(|| {
            gallery::neighbour_prefetch_paths(
                &self.gallery,
                index,
                self.last_nav_direction,
                self.prefetch_window,
            )
        });
        let generation =
            match self
                .foreground_loader
                .submit_request_with_prefetch(path.clone(), 0, None, true, neighbours)
            {
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

    fn image_navigation_index(&self) -> usize {
        self.pending_image_index
            .filter(|(generation, _)| *generation == self.foreground_loader.current_generation())
            .map_or(self.image_index, |(_, index)| index)
    }

    fn advance_animation(&mut self) {
        let current = self.foreground_loader.current_generation();
        if self.ready_generation != Some(current) {
            return;
        }
        let Some(next) = self.playback.take_next(current, Instant::now()) else {
            return;
        };
        let wrap = self.playback.repeat_after_current_cycle(current).unwrap_or(false);
        self.raw_prefetcher.begin_foreground();
        match self
            .foreground_loader
            .submit_animation_frame(self.path.clone(), next, self.scalar_window, wrap)
        {
            Ok(generation) => {
                self.playback.submitted(generation);
                self.pending_image_index = Some((generation, next));
                self.pending_first_present = None;
                self.pipeline = PipelineSnapshot::waiting(generation, RawKind::from_path(&self.path));
            }
            Err(error) => {
                self.playback.reset();
                self.set_status(format!("animation loader failed: {error}"));
            }
        }
    }

    fn navigate_image(&mut self, direction: isize) {
        if self.path.as_os_str().is_empty() {
            return;
        }
        let next = next_image_index(self.image_navigation_index(), self.image_count, direction);
        let Some(next) = next else {
            return;
        };
        self.playback.seek();
        self.cancel_drop();
        self.raw_prefetcher.begin_foreground();
        match self
            .foreground_loader
            .submit_image_with_window(self.path.clone(), next, self.scalar_window)
        {
            Ok(generation) => {
                self.playback.submitted(generation);
                self.pending_image_index = Some((generation, next));
                self.pending_first_present = None;
                self.pipeline = PipelineSnapshot::waiting(generation, RawKind::from_path(&self.path));
                self.set_status(format!(
                    "decoding image {}: {}",
                    next + 1,
                    display_name(&self.path)
                ));
            }
            Err(error) => {
                self.playback.reset();
                self.set_status(format!("image loader failed: {error}"));
            }
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
        self.cancel_drop();
        self.raw_prefetcher.begin_foreground();
        let index = self.image_navigation_index();
        match self
            .foreground_loader
            .submit_image_with_window(self.path.clone(), index, Some(window))
        {
            Ok(generation) => {
                self.pending_image_index = Some((generation, index));
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

    fn recover_lost_gpu(&mut self, event_loop: &ActiveEventLoop) {
        if let Some(reason) = self.gpu.as_ref().and_then(GpuState::poll_loss) {
            self.restore_gpu(event_loop, &reason, true);
        }
        if let Some(reason) = self.telemetry.as_ref().and_then(TelemetryState::poll_loss) {
            self.restore_telemetry_gpu(event_loop, &reason, true);
        }
    }

    fn restore_telemetry_gpu(&mut self, event_loop: &ActiveEventLoop, reason: &str, automatic: bool) {
        log::error!("telemetry GPU device lost: {reason}");
        let Some(window) = self.telemetry_window.clone() else {
            return;
        };
        if let Some(old) = self.telemetry.take() {
            self.telemetry_recovery_view = Some(old.hud.cards_view());
            drop(old);
        }
        if automatic
            && self
                .last_telemetry_recovery
                .is_some_and(|last| last.elapsed() < Duration::from_secs(30))
        {
            window.set_title("Rrrah — display unavailable; refocus to retry");
            return;
        }
        self.last_telemetry_recovery = Some(Instant::now());
        match pollster::block_on(TelemetryState::new(
            event_loop.owned_display_handle(),
            window.clone(),
            self.gpu_backend,
            self.gpu_vendor,
        )) {
            Ok(mut telemetry) => {
                telemetry.set_pipeline(&self.pipeline);
                telemetry.set_cache_snapshot(self.cache_telemetry.snapshot());
                if let Some(view) = self.telemetry_recovery_view.take() {
                    telemetry.hud.restore_cards_view(view);
                    telemetry.rebuild_hud();
                }
                self.telemetry = Some(telemetry);
                window.set_title("Rrrah — processing pipeline");
                window.request_redraw();
            }
            Err(error) => {
                log::error!("telemetry GPU recreation failed: {error:#}");
                window.set_title("Rrrah — display unavailable; refocus to retry");
            }
        }
    }

    fn restore_gpu(&mut self, event_loop: &ActiveEventLoop, reason: &str, automatic: bool) {
        log::error!("viewer GPU device lost: {reason}");
        let Some(window) = self.window.clone() else {
            return;
        };
        let image_index = self.image_navigation_index();
        self.foreground_loader.cancel_pending();
        self.gpu_recovery_generation = None;
        self.pending_first_present = None;
        self.ready_generation = None;
        self.pending_model_upload = None;
        self.model_upload_worker.cancel();
        self.pending_raster_lod = None;
        self.raster_lod_worker.cancel();
        self.raw_prefetcher.begin_foreground();
        // Release device-owned textures and upload leases before admitting new ones.
        drop(self.gpu.take());
        if automatic
            && self
                .last_gpu_recovery
                .is_some_and(|last| last.elapsed() < Duration::from_secs(30))
        {
            self.set_status("display unavailable — refocus window to retry".into());
            return;
        }
        self.last_gpu_recovery = Some(Instant::now());
        self.set_status("restoring display".into());
        let mut gpu = match pollster::block_on(GpuState::new(
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
                log::error!("viewer GPU recreation failed: {error:#}");
                self.set_status("display unavailable — refocus window to retry".into());
                return;
            }
        };
        gpu.raw_demosaic = Some(self.gpu_demosaic);
        gpu.viewport_worker = self.raster_lod_worker.with_same_waker();
        gpu.storage_worker = self.raster_lod_worker.with_same_waker();
        self.view.viewport = [gpu.size.width as f32, gpu.size.height as f32];
        gpu.update_view(self.view);
        gpu.update_comparison(
            if self.split_comparison && !self.before {
                0.5
            } else {
                0.0
            },
            self.editor.original().exposure_stops,
        );
        self.gpu = Some(gpu);
        self.strip.dirty = true;
        if !self.path.as_os_str().is_empty() {
            let neighbours = self.gallery_index.map(|index| {
                gallery::neighbour_prefetch_paths(
                    &self.gallery,
                    index,
                    self.last_nav_direction,
                    self.prefetch_window,
                )
            });
            match self.foreground_loader.submit_request_with_prefetch(
                self.path.clone(),
                image_index,
                self.scalar_window,
                true,
                neighbours,
            ) {
                Ok(generation) => {
                    self.pending_image_index = Some((generation, image_index));
                    self.gpu_recovery_generation = Some(generation);
                    self.pipeline = PipelineSnapshot::waiting(generation, RawKind::from_path(&self.path));
                    if let Some(telemetry) = self.telemetry.as_mut() {
                        telemetry.set_pipeline(&self.pipeline);
                    }
                }
                Err(error) => self.set_status(format!("image reload failed: {error}")),
            }
        }
        window.request_redraw();
    }

    fn set_status(&mut self, status: String) {
        self.status = status;
        self.update_accessibility();
        if let Some(window) = &self.window {
            window.set_title(&self.window_title());
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
    fn exiting(&mut self, _event_loop: &ActiveEventLoop) {
        self.editor.shutdown();
        self.histogram_worker.cancel();
        self.raster_lod_worker.cancel();
        self.pending_raster_lod = None;
        // macOS terminate may exit without Rust stack unwinding. Release transient
        // storage owners here while the application still receives exit callbacks.
        self.gpu = None;
        self.foreground_loader.shutdown();
        while self.receiver.try_recv().is_ok() {}
        log::info!("Viewer transient storage shutdown complete");
    }

    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        let window = match event_loop.create_window(
            Window::default_attributes()
                .with_title("Rrrah — loading…")
                .with_visible(false),
        ) {
            Ok(window) => Arc::new(window),
            Err(error) => {
                log::error!("failed to create viewer window: {error}");
                self.status = format!("window creation failed: {error}");
                event_loop.exit();
                return;
            }
        };
        if let Some(proxy) = self.event_proxy.clone() {
            self.accesskit = Some(accesskit_winit::Adapter::with_event_loop_proxy(
                event_loop, &window, proxy,
            ));
        }
        window.set_visible(true);
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
        self.sync_editor();
        self.view.exposure_stops = self.editor.current().exposure_stops;
        self.view.viewport = [gpu.size.width as f32, gpu.size.height as f32];
        self.window = Some(window.clone());
        let mut gpu = gpu;
        gpu.raw_demosaic = Some(self.gpu_demosaic);
        gpu.viewport_worker = self.raster_lod_worker.with_same_waker();
        gpu.storage_worker = self.raster_lod_worker.with_same_waker();
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
        self.recover_lost_gpu(event_loop);
        self.drain_dropped_paths();
        if let Some(result) = self.foreground_loader.poll_duplicate() {
            match result {
                Ok(generation) => {
                    self.raw_prefetcher.begin_foreground();
                    self.pending_first_present = None;
                    self.pipeline = PipelineSnapshot::waiting(generation, RawKind::from_path(&self.path));
                    if let Some(telemetry) = self.telemetry.as_mut() {
                        telemetry.set_pipeline(&self.pipeline);
                    }
                    self.set_status(format!("source changed; reloading {}", display_name(&self.path)));
                }
                Err(error) => self.set_status(format!("image loader failed: {error}")),
            }
        }
        self.drain_folder_scan();
        self.drain_load_events();
        self.advance_animation();
        if !self.path.as_os_str().is_empty()
            && !self.editor.is_current_version(
                &self.path,
                self.image_index,
                self.foreground_loader.current_generation(),
            )
        {
            self.update_view();
        }
        self.drain_thumbnails();
        if let Some(gpu) = self.gpu.as_mut() {
            if let Some((size, deadline)) = gpu.model_resize_retry {
                if Instant::now() >= deadline {
                    gpu.resize(size);
                    if gpu.model_resize_retry.is_none() {
                        self.view.viewport = [gpu.size.width as f32, gpu.size.height as f32];
                        gpu.update_view(self.view);
                        gpu.window.request_redraw();
                    }
                }
            }
        }
        let telemetry_pending = self.telemetry.as_ref().is_some_and(|telemetry| {
            telemetry
                .upload_completions
                .poll(&telemetry.device)
                .unwrap_or_else(|error| {
                    log::warn!("telemetry completion polling failed: {error}");
                    false
                })
        });
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
            .map(|thumbnail| thumbnail.retry_at)
            .into_iter()
            .chain(
                self.gpu
                    .as_ref()
                    .and_then(|gpu| gpu.model_resize_retry.map(|(_, deadline)| deadline)),
            )
            .min();
        if retry.is_some()
            && let Some(gpu) = &self.gpu
        {
            // Budget callbacks must advance without a visible redraw.
            if let Err(error) = gpu.device.poll(wgpu::PollType::Poll) {
                log::warn!("thumbnail upload completion polling failed: {error}");
            }
        }
        #[cfg(debug_assertions)]
        self.qualify_recovery_step(event_loop);
        event_loop.set_control_flow(
            if pending
                || telemetry_pending
                || cfg!(debug_assertions) && {
                    #[cfg(debug_assertions)]
                    {
                        self.recovery_probe.is_some()
                    }
                    #[cfg(not(debug_assertions))]
                    {
                        false
                    }
                }
                || self.pending_folder_scan.is_some()
                || self.pending_drop.is_some()
                || self.foreground_loader.has_pending_duplicate()
            {
                ControlFlow::WaitUntil(Instant::now() + Duration::from_millis(10))
            } else if let Some(deadline) = match (
                retry,
                self.playback
                    .deadline(self.foreground_loader.current_generation()),
            ) {
                (Some(a), Some(b)) => Some(a.min(b)),
                (a, b) => a.or(b),
            } {
                ControlFlow::WaitUntil(deadline)
            } else {
                ControlFlow::Wait
            },
        );
    }

    fn user_event(&mut self, _event_loop: &ActiveEventLoop, event: WakeEvent) {
        if let WakeEvent::AccessKit(event) = event {
            if self.window.as_ref().is_some_and(|w| w.id() == event.window_id) {
                match event.window_event {
                    accesskit_winit::WindowEvent::InitialTreeRequested => self.update_accessibility(),
                    accesskit_winit::WindowEvent::ActionRequested(request) => {
                        if request.target_tree != accesskit::TreeId::ROOT
                            || !(accessibility::FIRST_COMMAND
                                ..accessibility::FIRST_COMMAND + accessibility::COMMANDS.len() as u64)
                                .contains(&request.target_node.0)
                        {
                            return;
                        }
                        if request.action == accesskit::Action::Focus {
                            self.access_focus = request.target_node;
                            self.update_accessibility();
                        } else if request.action == accesskit::Action::Click {
                            self.access_command(request.target_node);
                        }
                    }
                    accesskit_winit::WindowEvent::AccessibilityDeactivated => {}
                }
            }
        }
        self.poll_editor();
        if let Some(histogram) = self.histogram_worker.poll() {
            self.histogram = Some(histogram);
        }
        self.drain_load_events();
        if !self.path.as_os_str().is_empty()
            && !self.editor.is_current_version(
                &self.path,
                self.image_index,
                self.foreground_loader.current_generation(),
            )
        {
            self.update_view();
        }
        self.drain_thumbnails();
        if let Some(window) = &self.window {
            window.request_redraw();
        }
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, id: WindowId, event: WindowEvent) {
        if matches!(&event, WindowEvent::RedrawRequested | WindowEvent::Resized(_)) {
            self.recover_lost_gpu(event_loop);
        }
        if let (Some(adapter), Some(window)) = (self.accesskit.as_mut(), self.window.as_ref()) {
            if window.id() == id {
                adapter.process_event(window, &event);
            }
        }
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
                WindowEvent::Focused(true) if self.telemetry.is_none() => {
                    self.restore_telemetry_gpu(event_loop, "retry after window focus", false);
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
            WindowEvent::ModifiersChanged(modifiers) => {
                self.modifiers = modifiers.state();
            }
            WindowEvent::Focused(false) => {
                self.before = false;
                self.update_view();
            }
            WindowEvent::KeyboardInput { event, .. }
                if event.state == ElementState::Released
                    && event.physical_key == PhysicalKey::Code(KeyCode::KeyB) =>
            {
                self.before = false;
                self.update_view();
            }
            WindowEvent::Focused(true) => {
                if self.gpu.is_none() {
                    self.restore_gpu(event_loop, "retry after window focus", false);
                }
                if let Some(window) = &self.window {
                    window.request_redraw();
                }
            }
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
                    self.view.viewport = [gpu.size.width as f32, gpu.size.height as f32];
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
                #[cfg(debug_assertions)]
                if display_submit.is_some() {
                    if let Some(gpu) = &mut self.gpu {
                        gpu.presented_generation = self.ready_generation;
                    }
                }
                if let Some(display_submit) = display_submit {
                    if let Some(generation) = self
                        .ready_generation
                        .filter(|g| *g == self.foreground_loader.current_generation())
                    {
                        self.playback.presented(generation, Instant::now());
                    }
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
                    PhysicalKey::Code(KeyCode::KeyR)
                        if self.modifiers.shift_key()
                            && (self.modifiers.control_key() || self.modifiers.super_key()) =>
                    {
                        self.resolve_edit_conflict(true);
                    }
                    PhysicalKey::Code(KeyCode::KeyX)
                        if self.modifiers.shift_key()
                            && (self.modifiers.control_key() || self.modifiers.super_key()) =>
                    {
                        self.resolve_edit_conflict(false);
                    }
                    PhysicalKey::Code(KeyCode::KeyN)
                        if self.modifiers.shift_key()
                            && (self.modifiers.control_key() || self.modifiers.super_key()) =>
                    {
                        self.restart_refused_recovery();
                    }
                    PhysicalKey::Code(KeyCode::KeyP)
                        if !self.modifiers.control_key()
                            && !self.modifiers.super_key()
                            && !self.modifiers.alt_key() =>
                    {
                        let generation = self.foreground_loader.current_generation();
                        if let Some(paused) = self.playback.toggle_pause(generation, Instant::now()) {
                            self.set_status(if paused {
                                "Animation paused (P to resume)".into()
                            } else {
                                "Animation playing (P to pause)".into()
                            });
                        }
                    }
                    PhysicalKey::Code(KeyCode::Tab) => {
                        self.command_focus_visible = true;
                        self.access_focus =
                            accessibility::next_focus(self.access_focus, self.modifiers.shift_key());
                        self.update_accessibility();
                        if let Some(window) = &self.window {
                            window.set_title(&self.window_title());
                        }
                    }
                    PhysicalKey::Code(KeyCode::Enter) | PhysicalKey::Code(KeyCode::Space)
                        if self.command_focus_visible =>
                    {
                        self.access_command(self.access_focus)
                    }
                    PhysicalKey::Code(KeyCode::KeyZ)
                        if self.modifiers.control_key() || self.modifiers.super_key() =>
                    {
                        self.undo_redo_edit(self.modifiers.shift_key());
                    }
                    PhysicalKey::Code(KeyCode::KeyY)
                        if self.modifiers.control_key() || self.modifiers.super_key() =>
                    {
                        self.undo_redo_edit(true);
                    }
                    PhysicalKey::Code(KeyCode::KeyS)
                        if self.modifiers.control_key() || self.modifiers.super_key() =>
                    {
                        self.save_edits();
                    }
                    PhysicalKey::Code(KeyCode::KeyH) => {
                        let message = self.histogram.as_ref().map_or_else(|| "Source-linear histogram unavailable for this frame".to_string(), |h| {
                            let peaks: Vec<_> = h.channels.iter().map(|bins| bins.iter().enumerate().max_by_key(|(_, n)| *n).map_or(0, |(i, _)| i)).collect();
                            format!("Source-linear HDR histogram: {} sampled; R/G/B/Y peak bins {:?}; negative {:?}; overflow {:?}; invalid {}", h.sampled, peaks, h.negative, h.overflow, h.invalid)
                        });
                        self.set_status(message);
                    }
                    PhysicalKey::Code(KeyCode::KeyC) => {
                        self.split_comparison = !self.split_comparison;
                        self.update_view();
                    }
                    PhysicalKey::Code(KeyCode::KeyB) => {
                        self.before = true;
                        self.update_view();
                    }
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
                        self.set_exposure(0.);
                        self.view = ViewParameters {
                            viewport: self.view.viewport,
                            ..ViewParameters::default()
                        };
                        self.update_view();
                    }
                    PhysicalKey::Code(KeyCode::Equal) | PhysicalKey::Code(KeyCode::NumpadAdd) => {
                        self.set_exposure((self.editor.current().exposure_stops + 0.25).min(10.0));
                    }
                    PhysicalKey::Code(KeyCode::Minus) | PhysicalKey::Code(KeyCode::NumpadSubtract) => {
                        self.set_exposure((self.editor.current().exposure_stops - 0.25).max(-10.0));
                    }
                    _ => {}
                }
            }
            _ => {}
        }
    }
}

struct TelemetryState {
    #[cfg(debug_assertions)]
    presented_frames: u64,
    #[cfg(debug_assertions)]
    hud_revision: u64,
    #[cfg(debug_assertions)]
    presented_hud_revision: Option<u64>,
    #[cfg(debug_assertions)]
    device_epoch: u64,
    loss: DeviceLossSignal,
    upload_completions: UploadCompletions,
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
    fn poll_loss(&self) -> Option<String> {
        if let Err(error) = self.device.poll(wgpu::PollType::Poll) {
            self.loss
                .report(wgpu::DeviceLostReason::Unknown, error.to_string());
        }
        self.loss.take()
    }

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
        let loss = DeviceLossSignal::default();
        let callback_loss = loss.clone();
        let callback_window = window.clone();
        device.set_device_lost_callback(move |reason, message| {
            if callback_loss.report(reason, message) {
                callback_window.request_redraw();
            }
        });
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
            #[cfg(debug_assertions)]
            presented_frames: 0,
            #[cfg(debug_assertions)]
            hud_revision: 0,
            #[cfg(debug_assertions)]
            presented_hud_revision: None,
            #[cfg(debug_assertions)]
            device_epoch: gpu_recovery_probe::next_device_epoch(),
            loss,
            upload_completions: UploadCompletions::default(),
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
        #[cfg(debug_assertions)]
        {
            self.hud_revision = self.hud_revision.saturating_add(1);
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
        self.upload_completions.track_submitted(&self.queue);
        self.window.pre_present_notify();
        self.queue.present(output);
        #[cfg(debug_assertions)]
        {
            self.presented_frames = self.presented_frames.saturating_add(1);
            self.presented_hud_revision = Some(self.hud_revision);
        }
    }
}

mod model_upload_worker;
mod raster_lod_worker;
mod raster_viewport;

#[cfg(test)]
mod device_loss_signal_tests {
    use super::*;
    #[test]
    #[ignore = "actual Metal tiny RAW atlas budget and old-layout pixel comparison"]
    fn tiny_raw_atlas_budget_preserves_sdr_hdr_for_every_algorithm_metal() {
        let gpu = navigation_metal_readback::qualification_gpu().expect("actual GPU required");
        eprintln!("tiny RAW atlas adapter: {}", gpu.adapter_name());
        for (width, height) in [(16, 16), (7, 31), (31, 4)] {
            let mosaic = navigation_metal_readback::pattern_mosaic(width, height, 65535.0, |x, y| {
                (1000 + (x * 431 + y * 127) % 30000) as u16
            });
            let size = [64, 64];
            let fit = (32.0 / width as f32).min(32.0 / height as f32);
            let cover = (64.0 / width as f32).max(64.0 / height as f32);
            let view = ViewParameters {
                viewport: [64.0, 64.0],
                zoom: cover / fit,
                ..Default::default()
            };
            for algorithm in rrrah_gpu::GpuDemosaicAlgorithm::ALL {
                let halo = algorithm.min_tile_halo();
                for hdr in [false, true] {
                    let format = if hdr {
                        wgpu::TextureFormat::Rgba16Float
                    } else {
                        wgpu::TextureFormat::Rgba8UnormSrgb
                    };
                    let mut reference = RawRenderer::new(&gpu.device, format);
                    reference.set_demosaic_algorithm(&gpu.queue, algorithm);
                    assert_eq!(reference.demosaic_algorithm(), algorithm);
                    reference
                        .upload_mosaic_with_tiling(
                            &gpu.device,
                            &gpu.queue,
                            &mosaic,
                            rrrah_gpu::TilingOverrides {
                                tile_size: Some(1024 - halo * 2),
                                tile_halo: Some(halo),
                            },
                        )
                        .unwrap();
                    reference.update_view(&gpu.queue, view);
                    let budget = rrrah_core::MemoryBudget::new(1024 * 1024);
                    let mut compact = RawRenderer::new_with_budget(&gpu.device, format, budget.clone());
                    compact.set_demosaic_algorithm(&gpu.queue, algorithm);
                    assert_eq!(compact.demosaic_algorithm(), algorithm);
                    compact.upload_mosaic(&gpu.device, &gpu.queue, &mosaic).unwrap();
                    compact.update_view(&gpu.queue, view);
                    assert!(budget.used() < 4096);
                    if hdr {
                        reference
                            .set_output_mode(&gpu.queue, rrrah_gpu::RawOutputMode::SceneLinear)
                            .unwrap();
                        compact
                            .set_output_mode(&gpu.queue, rrrah_gpu::RawOutputMode::SceneLinear)
                            .unwrap();
                        assert_eq!(
                            gpu.render_existing_raw_half(&compact, size),
                            gpu.render_existing_raw_half(&reference, size)
                        );
                    } else {
                        assert_eq!(
                            gpu.render_existing_raw(&compact, size).pixels,
                            gpu.render_existing_raw(&reference, size).pixels
                        );
                    }
                    drop(compact);
                    assert_eq!(budget.used(), 0);
                }
                eprintln!(
                    "tiny RAW atlas {width}x{height} algorithm={algorithm:?}: SDR/HDR exact vs old1024 atlas; managed GPU<4096; final=0"
                );
            }
        }
    }

    #[test]
    #[ignore = "actual Metal HUD view/frame preservation across two independent devices"]
    fn hud_view_and_pixels_survive_independent_device_recreation_metal() {
        let size = [320, 240];
        let cards = vec![rrrah_gpu::HudCard::new("RECOVERY", "READY", "bounded cache and GPU state"); 48];
        let gpu = navigation_metal_readback::qualification_gpu().expect("actual GPU required");
        eprintln!("HUD first device: {}", gpu.adapter_name());
        let mut hud = HudRenderer::new(&gpu.device, wgpu::TextureFormat::Rgba8UnormSrgb, [320.0, 240.0]);
        hud.zoom_cards_by(1.37);
        hud.update_cards(&gpu.device, &gpu.queue, &cards);
        hud.scroll_cards_by(73.0);
        hud.update_cards(&gpu.device, &gpu.queue, &cards);
        let view = hud.cards_view();
        // Responsive dashboard fits all cards; preserve its actual clamped
        // scroll (zero here) rather than requiring an impossible offset.
        assert_ne!(view[0].to_bits(), 0.75_f32.to_bits());
        let reference = gpu.read_frame(size, |encoder, target| hud.encode(encoder, target));
        assert!(
            reference
                .pixels
                .chunks_exact(4)
                .any(|pixel| pixel[..3].iter().any(|&v| v != 0))
        );
        drop(hud);
        drop(gpu);
        let gpu = navigation_metal_readback::qualification_gpu().expect("actual replacement GPU required");
        eprintln!("HUD replacement device: {}", gpu.adapter_name());
        let mut hud = HudRenderer::new(&gpu.device, wgpu::TextureFormat::Rgba8UnormSrgb, [320.0, 240.0]);
        hud.restore_cards_view(view);
        hud.update_cards(&gpu.device, &gpu.queue, &cards);
        assert_eq!(hud.cards_view().map(f32::to_bits), view.map(f32::to_bits));
        let restored = gpu.read_frame(size, |encoder, target| hud.encode(encoder, target));
        assert_eq!(restored.pixels, reference.pixels);
        eprintln!(
            "HUD independent-device recreation: zoom/scroll bits and whole320x240 SDR frame exact; no window loss injected"
        );
    }

    #[test]
    fn destruction_is_ignored_and_first_loss_survives_duplicate_notifications() {
        let signal = DeviceLossSignal::default();
        assert!(!signal.report(wgpu::DeviceLostReason::Destroyed, "normal teardown".into()));
        assert!(signal.take().is_none());
        let other = signal.clone();
        assert!(other.report(wgpu::DeviceLostReason::Unknown, "first failure".into()));
        assert!(signal.report(wgpu::DeviceLostReason::Unknown, "later notification".into()));
        assert_eq!(signal.take().as_deref(), Some("first failure"));
        assert!(signal.take().is_none());
        assert!(!other.report(wgpu::DeviceLostReason::Destroyed, "old device teardown".into()));
        assert!(signal.take().is_none());
    }
}

#[derive(Clone, Default)]
struct DeviceLossSignal(Arc<std::sync::Mutex<Option<String>>>);

impl DeviceLossSignal {
    fn report(&self, reason: wgpu::DeviceLostReason, message: String) -> bool {
        if reason == wgpu::DeviceLostReason::Destroyed {
            return false;
        }
        let mut loss = self.0.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        if loss.is_none() {
            *loss = Some(message);
        }
        true
    }

    fn take(&self) -> Option<String> {
        self.0
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .take()
    }
}

struct GpuState {
    #[cfg(debug_assertions)]
    presented_frames: u64,
    #[cfg(debug_assertions)]
    presented_generation: Option<u64>,
    #[cfg(debug_assertions)]
    raw_initializations: u64,
    #[cfg(debug_assertions)]
    device_epoch: u64,
    loss: DeviceLossSignal,
    _instance: wgpu::Instance,
    window: Arc<Window>,
    surface: wgpu::Surface<'static>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    upload_completions: UploadCompletions,
    config: wgpu::SurfaceConfiguration,
    size: PhysicalSize<u32>,
    bayer_compute: Option<rrrah_gpu::BayerDemosaicCompute>,
    renderer: Option<RawRenderer>,
    raw_gpu_budget: Option<rrrah_core::MemoryBudget>,
    raw_upload_memory_budget: Option<rrrah_core::MemoryBudget>,
    raw_demosaic: Option<rrrah_gpu::GpuDemosaicAlgorithm>,
    raw_comparison: (f32, f32),
    raster: Option<rrrah_gpu::RasterRenderer>,
    raster_active: bool,
    raster_viewport: Option<raster_viewport::ResidentSource>,
    raster_region_error: Option<String>,
    viewport_worker: raster_lod_worker::LodWorker,
    storage_worker: raster_lod_worker::LodWorker,
    storage_pending: Option<(u64, u64)>,
    viewport_pending: Option<(u64, u64, [u32; 2])>,
    viewport_ready: Option<(u64, rrrah_core::DecodedRaster)>,
    viewport_generation: Arc<std::sync::atomic::AtomicU64>,
    model: Option<rrrah_gpu::ModelRenderer>,
    model_active: bool,
    model_resize_retry: Option<(PhysicalSize<u32>, Instant)>,
    filmstrip: FilmstripRenderer,
}

impl GpuState {
    fn update_comparison(&mut self, split: f32, baseline: f32) {
        self.raw_comparison = (split, baseline);
        if let Some(renderer) = &mut self.renderer {
            renderer.set_comparison(&self.queue, split, baseline);
        }
        if let Some(raster) = &mut self.raster {
            raster.set_comparison(&self.queue, split, baseline);
        }
    }

    fn upload_bayer_compute(
        &mut self,
        mosaic: &DecodedMosaic,
        algorithm: rrrah_gpu::GpuDemosaicAlgorithm,
        cancelled: impl FnMut() -> bool,
    ) -> Result<GpuUploadTimings> {
        let started = Instant::now();
        let gpu_budget = self
            .raw_gpu_budget
            .clone()
            .unwrap_or_else(|| rrrah_core::MemoryBudget::new(u64::MAX));
        let cpu_budget = self
            .raw_upload_memory_budget
            .clone()
            .unwrap_or_else(|| rrrah_core::MemoryBudget::new(u64::MAX));
        let compute = self
            .bayer_compute
            .get_or_insert_with(|| rrrah_gpu::BayerDemosaicCompute::new(&self.device));
        let mut cancelled = cancelled;
        let output = compute.execute_mosaic_resident(
            &self.queue,
            mosaic,
            algorithm,
            &gpu_budget,
            &cpu_budget,
            &mut cancelled,
        )?;
        let raster = self
            .raster
            .as_mut()
            .ok_or_else(|| anyhow::anyhow!("Bayer compute requires linear/sRGB raster presentation"))?;
        raster.upload_bayer_resident(&self.device, &self.queue, &output, &mut cancelled)?;
        if self.config.color_space != wgpu::SurfaceColorSpace::ExtendedSrgbLinear {
            raster.set_raw_development(&self.queue, &rrrah_core::develop::MonotoneCurve::identity());
        }
        raster.set_comparison(&self.queue, self.raw_comparison.0, self.raw_comparison.1);
        let total = started.elapsed();
        log::info!(
            "viewer Bayer compute upload: algorithm={algorithm:?} sensor={}x{} total_ms={:.3}",
            mosaic.metadata.width,
            mosaic.metadata.height,
            total.as_secs_f64() * 1000.0
        );
        Ok(GpuUploadTimings {
            total,
            ..Default::default()
        })
    }

    fn prepare_raw_renderer(&mut self, view: ViewParameters) -> Result<()> {
        if self.renderer.is_some() {
            return Ok(());
        }
        let started = Instant::now();
        let mut renderer = match self.raw_gpu_budget.clone() {
            Some(budget) => RawRenderer::new_with_budget(&self.device, self.config.format, budget),
            None => RawRenderer::new(&self.device, self.config.format),
        };
        if self.config.color_space == wgpu::SurfaceColorSpace::ExtendedSrgbLinear {
            renderer.set_output_mode(&self.queue, rrrah_gpu::RawOutputMode::SceneLinear)?;
        }
        if let Some(budget) = self.raw_upload_memory_budget.clone() {
            renderer = renderer
                .with_upload_memory_budget(budget.clone())
                .with_upload_queue_budget(budget);
        }
        if let Some(algorithm) = self.raw_demosaic {
            renderer.set_demosaic_algorithm(&self.queue, algorithm);
        }
        renderer.set_comparison(&self.queue, self.raw_comparison.0, self.raw_comparison.1);
        renderer.update_view(&self.queue, view);
        self.renderer = Some(renderer);
        #[cfg(debug_assertions)]
        {
            self.raw_initializations = self.raw_initializations.saturating_add(1);
        }
        log::info!(
            "viewer lazy RAW setup: ms={:.3}",
            started.elapsed().as_secs_f64() * 1000.0
        );
        Ok(())
    }

    fn poll_loss(&self) -> Option<String> {
        if let Err(error) = self.device.poll(wgpu::PollType::Poll) {
            self.loss
                .report(wgpu::DeviceLostReason::Unknown, error.to_string());
        }
        self.loss.take()
    }

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
        let setup_started = Instant::now();
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
            .request_device(&wgpu::DeviceDescriptor {
                required_limits: viewer_device_limits(&adapter.limits()),
                ..Default::default()
            })
            .await
            .context("request GPU device")?;
        let device_ready = Instant::now();
        let loss = DeviceLossSignal::default();
        let callback_loss = loss.clone();
        let callback_window = window.clone();
        device.set_device_lost_callback(move |reason, message| {
            if callback_loss.report(reason, message) {
                callback_window.request_redraw();
            }
        });
        let capabilities = surface.get_capabilities(&adapter);
        let hdr_surface = match std::env::var("RRRAH_HDR_SURFACE").as_deref() {
            Ok("1") => true,
            Ok("0") | Err(_) => false,
            Ok(value) => anyhow::bail!("invalid RRRAH_HDR_SURFACE={value}; expected 0 or 1"),
        };
        let (surface_format, surface_color_space) = select_viewer_surface(&capabilities, hdr_surface)?;
        log::info!(
            "Viewer surface: hdr={} format={:?} color_space={:?}",
            hdr_surface,
            surface_format,
            surface_color_space
        );
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
        let raw_started = Instant::now();
        // RAW shaders are created only when a sensor mosaic actually arrives.
        let renderer = None;
        let raw_upload_memory_budget = upload_memory_budget.clone();
        let raster_started = Instant::now();
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
        let filmstrip_started = Instant::now();
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
        log::info!(
            "viewer GPU setup: device_ms={:.3} surface_ms={:.3} raw_ms={:.3} raster_ms={:.3} filmstrip_ms={:.3} total_ms={:.3}",
            device_ready.duration_since(setup_started).as_secs_f64() * 1000.0,
            raw_started.duration_since(device_ready).as_secs_f64() * 1000.0,
            raster_started.duration_since(raw_started).as_secs_f64() * 1000.0,
            filmstrip_started.duration_since(raster_started).as_secs_f64() * 1000.0,
            filmstrip_started.elapsed().as_secs_f64() * 1000.0,
            setup_started.elapsed().as_secs_f64() * 1000.0,
        );
        Ok(Self {
            #[cfg(debug_assertions)]
            presented_frames: 0,
            #[cfg(debug_assertions)]
            presented_generation: None,
            #[cfg(debug_assertions)]
            raw_initializations: 0,
            #[cfg(debug_assertions)]
            device_epoch: gpu_recovery_probe::next_device_epoch(),
            loss,
            _instance: instance,
            window,
            surface,
            device,
            queue,
            upload_completions: UploadCompletions::default(),
            config,
            size,
            renderer,
            bayer_compute: None,
            raw_gpu_budget,
            raw_upload_memory_budget,
            raw_demosaic: None,
            raw_comparison: (0.0, 0.0),
            raster,
            raster_active: false,
            raster_viewport: None,
            raster_region_error: None,
            viewport_worker: raster_lod_worker::LodWorker::default(),
            storage_worker: raster_lod_worker::LodWorker::default(),
            storage_pending: None,
            viewport_pending: None,
            viewport_ready: None,
            viewport_generation: Arc::new(std::sync::atomic::AtomicU64::new(0)),
            model: None,
            model_active: false,
            model_resize_retry: None,
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
        if let Some(renderer) = &mut self.renderer {
            renderer.update_view(&self.queue, view);
        }
        let identity = self
            .raster_viewport
            .as_ref()
            .filter(|_| self.raster_active)
            .map_or(0, |source| source.identity());
        self.viewport_generation
            .store(identity, std::sync::atomic::Ordering::Release);
        if self.storage_pending.is_some_and(|(_, source)| source != identity) {
            self.storage_worker.cancel();
            self.storage_pending = None;
        }
        if let Some(completed) = self.storage_worker.try_take() {
            if self.storage_pending == Some((completed.id, identity)) {
                self.storage_pending = None;
                match completed.into_swapped() {
                    Ok(swapped) => {
                        if self
                            .raster_viewport
                            .as_mut()
                            .is_some_and(|source| source.adopt_swapped(identity, swapped))
                        {
                            self.viewport_worker.cancel();
                            self.viewport_pending = None;
                            self.viewport_ready = None;
                            log::info!(
                                "Visible raster source migrated to segmented swap: identity={identity}"
                            );
                        }
                    }
                    Err(error) => log::warn!("visible raster swap retained source in RAM: {error}"),
                }
            }
        }
        if self
            .viewport_ready
            .as_ref()
            .is_some_and(|(source, _)| *source != identity)
        {
            self.viewport_ready = None;
        }
        if self
            .viewport_pending
            .is_some_and(|(_, source, _)| source != identity)
        {
            self.viewport_worker.cancel();
            self.viewport_pending = None;
        }
        if let Some(raster) = self.raster.as_mut() {
            if self.raster_active {
                if let Some(source) = self.raster_viewport.as_mut() {
                    if source.is_swapped() {
                        let token = GenerationToken::new(self.viewport_generation.clone(), identity);
                        match source.refresh_swapped(
                            raster,
                            &self.device,
                            &self.queue,
                            view,
                            &mut self.viewport_worker,
                            token,
                        ) {
                            Ok(uploaded) => {
                                self.raster_region_error = None;
                                if uploaded {
                                    self.upload_completions.track_submitted(&self.queue);
                                }
                            }
                            Err(error) => {
                                self.raster_region_error = Some(format!("Image region unavailable: {error}"))
                            }
                        }
                        raster.update_view(&self.queue, view);
                        return;
                    }
                    let needed = source.needs_lod(raster, &self.device, view).unwrap_or(false);
                    if let Some(completed) = self.viewport_worker.try_take() {
                        if self
                            .viewport_pending
                            .is_some_and(|(id, state, _)| id == completed.id && state == identity)
                        {
                            self.viewport_pending = None;
                            if needed {
                                match completed.into_frame() {
                                    Ok(lod) => self.viewport_ready = Some((identity, lod)),
                                    Err(error) => {
                                        self.raster_region_error =
                                            Some(format!("Image region unavailable: {error}"));
                                        raster.update_view(&self.queue, view);
                                        return;
                                    }
                                }
                            }
                        }
                    }
                    if needed {
                        if let Some((_, lod)) = &self.viewport_ready {
                            match source.publish_lod(raster, &self.device, &self.queue, lod.clone()) {
                                Ok(()) => {
                                    self.viewport_ready = None;
                                    self.upload_completions.track_submitted(&self.queue);
                                }
                                Err(error) => {
                                    // Keep the prepared pixels: resource pressure must not
                                    // start a filter/wake/retry loop on the same image.
                                    self.raster_region_error =
                                        Some(format!("Image region unavailable: {error}"));
                                    raster.update_view(&self.queue, view);
                                    return;
                                }
                            }
                        }
                    } else {
                        self.viewport_ready = None;
                    }
                    if source.needs_lod(raster, &self.device, view).unwrap_or(false) {
                        // Do not replan against memory already owned by the active/result job.
                        if self.viewport_pending.is_none() {
                            match source.refresh_job(raster, &self.device, view) {
                                Ok(Some((pixels, size, budget))) => {
                                    let token =
                                        GenerationToken::new(self.viewport_generation.clone(), identity);
                                    match self.viewport_worker.submit(pixels, size, budget, identity, token) {
                                        Ok(id) => self.viewport_pending = Some((id, identity, size)),
                                        Err(error) => {
                                            self.raster_region_error =
                                                Some(format!("Image region unavailable: {error}"))
                                        }
                                    }
                                }
                                Err(error) => {
                                    self.raster_region_error =
                                        Some(format!("Image region unavailable: {error}"))
                                }
                                Ok(None) => {}
                            }
                        }
                        raster.update_view(&self.queue, view);
                        return;
                    }
                    if self.viewport_pending.take().is_some() {
                        self.viewport_worker.cancel();
                    }
                    match source.refresh_deferred(raster, &self.device, &self.queue, view) {
                        Ok(uploaded) => {
                            self.raster_region_error = None;
                            if uploaded {
                                self.upload_completions.track_submitted(&self.queue);
                            }
                        }
                        Err(error) => {
                            self.raster_region_error = Some(format!("Image region unavailable: {error}"));
                        }
                    }
                }
            }
            raster.update_view(&self.queue, view);
        }
    }

    fn resize(&mut self, size: PhysicalSize<u32>) {
        if size.width == 0 || size.height == 0 {
            // A later nonzero resize supplies the current desired viewport.
            // Do not retry obsolete dimensions while the window is minimized.
            self.model_resize_retry = None;
            return;
        }
        // Admit the replacement depth attachment before changing the surface.
        // A refused resize must preserve the current model and compatible target.
        if self.model_active {
            if let Some(model) = self.model.as_mut() {
                if let Err(error) = model.resize(&self.device, [size.width, size.height]) {
                    log::debug!("model viewport resize deferred; retaining previous viewport: {error}");
                    self.model_resize_retry = matches!(error, rrrah_gpu::ModelUploadError::Memory(_))
                        .then(|| (size, Instant::now() + Duration::from_millis(250)));
                    return;
                }
            }
        }
        self.model_resize_retry = None;
        self.size = size;
        self.config.width = size.width;
        self.config.height = size.height;
        self.surface.configure(&self.device, &self.config);
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
        } else if let Some(renderer) = &self.renderer {
            renderer.encode(&mut encoder, &view);
        } else {
            let _clear = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Rrrah empty viewport"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: 0.012,
                            g: 0.014,
                            b: 0.018,
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
        } else if let Some(renderer) = &self.renderer {
            resources.extend(renderer.resource_lease());
        }
        if draw_strip {
            resources.extend(self.filmstrip.resource_lease());
        }
        self.queue.submit([command_buffer]);
        if !resources.is_empty() {
            resources.retain_until_complete(&self.queue);
        }
        self.upload_completions.track_submitted(&self.queue);
        let queue_submit = queue_submit_started.elapsed();
        let present_request_started = Instant::now();
        self.window.pre_present_notify();
        self.queue.present(output);
        #[cfg(debug_assertions)]
        {
            self.presented_frames = self.presented_frames.saturating_add(1);
        }
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
#[path = "../../rrrah-gpu/tests/common/mod.rs"]
mod navigation_metal_readback;

#[cfg(test)]
mod linear_pfm_viewer_tests;

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
    fn hlg_display_cli_separates_all_policy_fields_and_reaches_thumbnail() {
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../tests/fixtures/raster/avif-hlg10-color-svt.avif");
        let cli = Cli::try_parse_from([
            "rrrah",
            "--hlg-reference-white-nits",
            "100",
            "--hlg-peak-nits",
            "1000",
            "--hlg-system-gamma",
            "1.2",
        ])
        .unwrap();
        let policy = cli.raster_interpretation();
        let budget = rrrah_cache::MemoryBudget::new(1024 * 1024);
        let mut request = DecodeRequest::new(&path);
        request.memory_budget = Some(budget.clone());
        policy.apply(&mut request);
        assert_eq!(request.hlg_display, Some((100., 1000., 1.2)));
        let mut cache = RasterDisplayCache::new(rrrah_cache::CacheLimits::bytes(budget.limit()));
        let (first, _) = load_cached_raster_for_display(&request, None, &mut cache).unwrap();
        request.hlg_display = Some((100., 2000., 1.2));
        let (second, _) = load_cached_raster_for_display(&request, None, &mut cache).unwrap();
        let rrrah_core::RasterPixels::Rgba32Float(a) = first.pixels() else {
            panic!()
        };
        let rrrah_core::RasterPixels::Rgba32Float(b) = second.pixels() else {
            panic!()
        };
        assert!(
            a.iter()
                .zip(b.iter())
                .enumerate()
                .all(|(i, (a, b))| if i % 4 == 3 {
                    a == b
                } else {
                    (*a * 2. - b).abs() < 0.00001
                })
        );
        request.hlg_display = Some((100., 2000., 1.4));
        let (third, _) = load_cached_raster_for_display(&request, None, &mut cache).unwrap();
        let rrrah_core::RasterPixels::Rgba32Float(c) = third.pixels() else {
            panic!()
        };
        assert!(b.iter().zip(c.iter()).any(|(b, c)| b.to_bits() != c.to_bits()));
        request.hlg_display = Some((200., 2000., 1.4));
        let (fourth, _) = load_cached_raster_for_display(&request, None, &mut cache).unwrap();
        assert_eq!(cache.len(), 4);
        policy.apply(&mut request);
        let (hit, _) = load_cached_raster_with(&request, None, &mut cache, || {
            panic!("HLG policy must reuse its own entry")
        })
        .unwrap();
        let rrrah_core::RasterPixels::Rgba32Float(h) = hit.pixels() else {
            panic!()
        };
        assert!(a.ptr_eq(h));
        request.hlg_display = None;
        assert!(load_cached_raster_for_display(&request, None, &mut cache).is_err());
        drop((first, second, third, fourth, hit, cache));
        assert_eq!(budget.used(), 0);
        let loader = thumbnail_loader_with_cancel(Arc::new(DecodeGate::new()), Some(budget.clone()), policy);
        let thumbnail = loader(
            gallery::ThumbnailJob {
                index: 0,
                source: path,
                edge: 32,
            },
            GenerationToken::new(Arc::new(AtomicU64::new(1)), 1),
        )
        .unwrap();
        assert_eq!((thumbnail.width, thumbnail.height), (32, 16));
        drop(thumbnail);
        assert_eq!(budget.used(), 0);
        policy.apply(&mut request);
        Cli::try_parse_from(["rrrah"])
            .unwrap()
            .raster_interpretation()
            .apply(&mut request);
        assert_eq!(request.hlg_display, None);
        for flag in [
            "--hlg-reference-white-nits",
            "--hlg-peak-nits",
            "--hlg-system-gamma",
        ] {
            assert!(Cli::try_parse_from(["rrrah", flag, "100"]).is_err());
        }
        for gamma in ["0", "NaN", "inf", "4.1"] {
            assert!(
                Cli::try_parse_from([
                    "rrrah",
                    "--hlg-reference-white-nits",
                    "100",
                    "--hlg-peak-nits",
                    "1000",
                    "--hlg-system-gamma",
                    gamma
                ])
                .is_err()
            );
        }
    }
    #[test]
    fn pq_white_cli_separates_cache_and_reaches_thumbnail() {
        let path =
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/raster/avif-pq10-svt.avif");
        let cli = Cli::try_parse_from(["rrrah", "--pq-reference-white-nits", "100"]).unwrap();
        let policy = cli.raster_interpretation();
        let budget = rrrah_cache::MemoryBudget::new(512 * 1024);
        let mut request = DecodeRequest::new(&path);
        request.memory_budget = Some(budget.clone());
        policy.apply(&mut request);
        let mut cache = RasterDisplayCache::new(rrrah_cache::CacheLimits::bytes(budget.limit()));
        let (first, _) = load_cached_raster_for_display(&request, None, &mut cache).unwrap();
        request.pq_reference_white_nits = Some(200.);
        let (second, _) = load_cached_raster_for_display(&request, None, &mut cache).unwrap();
        let rrrah_core::RasterPixels::Rgba32Float(a) = first.pixels() else {
            panic!()
        };
        let rrrah_core::RasterPixels::Rgba32Float(b) = second.pixels() else {
            panic!()
        };
        assert_eq!(cache.len(), 2);
        assert!(
            a.iter()
                .zip(b.iter())
                .enumerate()
                .all(|(i, (a, b))| if i % 4 == 3 {
                    a == b
                } else {
                    (a / 2. - b).abs() < 0.00001
                })
        );
        request.pq_reference_white_nits = Some(100.);
        let (hit, _) = load_cached_raster_with(&request, None, &mut cache, || {
            panic!("PQ white must hit its own cache entry")
        })
        .unwrap();
        let rrrah_core::RasterPixels::Rgba32Float(h) = hit.pixels() else {
            panic!()
        };
        assert!(a.ptr_eq(h));
        drop((first, second, hit, cache));
        assert_eq!(budget.used(), 0);
        let loader = thumbnail_loader_with_cancel(Arc::new(DecodeGate::new()), Some(budget.clone()), policy);
        let thumbnail = loader(
            gallery::ThumbnailJob {
                index: 0,
                source: path,
                edge: 32,
            },
            GenerationToken::new(Arc::new(AtomicU64::new(1)), 1),
        )
        .unwrap();
        assert_eq!((thumbnail.width, thumbnail.height), (32, 16));
        drop(thumbnail);
        assert_eq!(budget.used(), 0);
        Cli::try_parse_from(["rrrah"])
            .unwrap()
            .raster_interpretation()
            .apply(&mut request);
        assert_eq!(request.pq_reference_white_nits, None);
        for value in ["0", "NaN", "inf", "10001"] {
            assert!(Cli::try_parse_from(["rrrah", "--pq-reference-white-nits", value]).is_err());
        }
    }
    #[test]
    fn rla_linear_color_cli_preserves_hdr_and_separates_cached_transfer() {
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../tests/fixtures/raster/rla-float32-le-hdr-oiio.rla");
        let cli = Cli::try_parse_from([
            "rrrah",
            "--rla-color",
            "linear-srgb",
            "--rla-float-byte-order",
            "little",
            "--rla-alpha",
            "straight",
        ])
        .unwrap();
        let policy = cli.raster_interpretation();
        let budget = rrrah_cache::MemoryBudget::new(4096);
        let mut request = DecodeRequest::new(&path);
        request.memory_budget = Some(budget.clone());
        policy.apply(&mut request);
        assert_eq!(
            request.rla_color_space,
            Some(rrrah_decode::RlaColorSpace::LinearSrgb)
        );
        let mut cache = RasterDisplayCache::new(rrrah_cache::CacheLimits::bytes(4096));
        let (linear, _) = load_cached_raster_for_display(&request, None, &mut cache).unwrap();
        let rrrah_core::RasterPixels::Rgba32Float(pixels) = linear.pixels() else {
            panic!()
        };
        assert_eq!(pixels.as_ref(), &[4.0, 2.0, -0.5, 1.0, 4.0, 2.0, -0.5, 1.0]);
        request.rla_color_space = Some(rrrah_decode::RlaColorSpace::Srgb);
        let (encoded, _) = load_cached_raster_for_display(&request, None, &mut cache).unwrap();
        let rrrah_core::RasterPixels::Rgba32Float(other) = encoded.pixels() else {
            panic!()
        };
        assert_ne!(pixels[0].to_bits(), other[0].to_bits());
        assert_eq!(cache.len(), 2);
        request.rla_color_space = Some(rrrah_decode::RlaColorSpace::LinearSrgb);
        let (hit, _) = load_cached_raster_with(&request, None, &mut cache, || {
            panic!("linear color must hit its own existing entry")
        })
        .unwrap();
        let rrrah_core::RasterPixels::Rgba32Float(hit_pixels) = hit.pixels() else {
            panic!()
        };
        assert!(pixels.ptr_eq(hit_pixels));
        drop(hit);
        let loader = thumbnail_loader_with_cancel(Arc::new(DecodeGate::new()), Some(budget.clone()), policy);
        let thumbnail = loader(
            gallery::ThumbnailJob {
                index: 0,
                source: path,
                edge: 32,
            },
            GenerationToken::new(Arc::new(AtomicU64::new(1)), 1),
        )
        .unwrap();
        assert_eq!((thumbnail.width, thumbnail.height), (2, 1));
        drop((thumbnail, linear, encoded, cache));
        assert_eq!(budget.used(), 0);
        Cli::try_parse_from(["rrrah"])
            .unwrap()
            .raster_interpretation()
            .apply(&mut request);
        assert_eq!(request.rla_color_space, None);
        assert!(Cli::try_parse_from(["rrrah", "--rla-color", "auto"]).is_err());
    }
    #[test]
    fn rla_float_cli_reaches_decode_and_thumbnail_and_clears_defaults() {
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../tests/fixtures/raster/rla-float32-le-hdr-oiio.rla");
        let cli = Cli::try_parse_from([
            "rrrah",
            "--rla-float-byte-order",
            "little",
            "--rla-alpha",
            "straight",
            "--untagged-color",
            "srgb",
        ])
        .unwrap();
        let policy = cli.raster_interpretation();
        let budget = rrrah_cache::MemoryBudget::new(4096);
        let mut request = DecodeRequest::new(&path);
        request.memory_budget = Some(budget.clone());
        policy.apply(&mut request);
        assert_eq!(
            request.rla_float_byte_order,
            Some(rrrah_decode::RlaFloatByteOrder::Little)
        );
        let (prepared, _) = load_raster_for_display_typed(&request, None).unwrap();
        assert_eq!((prepared.width(), prepared.height()), (2, 1));
        drop(prepared);
        let loader = thumbnail_loader_with_cancel(Arc::new(DecodeGate::new()), Some(budget.clone()), policy);
        let owner = Arc::new(AtomicU64::new(1));
        let thumbnail = loader(
            gallery::ThumbnailJob {
                index: 0,
                source: path,
                edge: 32,
            },
            GenerationToken::new(owner, 1),
        )
        .unwrap();
        assert_eq!((thumbnail.width, thumbnail.height), (2, 1));
        drop(thumbnail);
        assert_eq!(budget.used(), 0);
        Cli::try_parse_from(["rrrah"])
            .unwrap()
            .raster_interpretation()
            .apply(&mut request);
        assert_eq!(request.rla_float_byte_order, None);
        assert!(load_raster_for_display_typed(&request, None).is_err());
        assert_eq!(budget.used(), 0);
        let big = Cli::try_parse_from(["rrrah", "--rla-float-byte-order", "big"]).unwrap();
        big.raster_interpretation().apply(&mut request);
        assert_eq!(
            request.rla_float_byte_order,
            Some(rrrah_decode::RlaFloatByteOrder::Big)
        );
        assert!(Cli::try_parse_from(["rrrah", "--rla-float-byte-order", "native"]).is_err());
    }
    #[test]
    fn rla_float_byte_order_isolated_in_actual_display_cache() {
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../tests/fixtures/raster/rla-float32-le-hdr-oiio.rla");
        let budget = rrrah_cache::MemoryBudget::new(1024 * 1024);
        let mut request = DecodeRequest::new(path);
        request.memory_budget = Some(budget.clone());
        request.assume_untagged_srgb = true;
        request.rla_alpha_mode = Some(rrrah_decode::RlaAlphaMode::Straight);
        let mut cache = RasterDisplayCache::new(rrrah_cache::CacheLimits::bytes(4096));
        request.rla_float_byte_order = Some(rrrah_decode::RlaFloatByteOrder::Little);
        let (little, _) = load_cached_raster_for_display(&request, None, &mut cache).unwrap();
        request.rla_float_byte_order = Some(rrrah_decode::RlaFloatByteOrder::Big);
        let (big, _) = load_cached_raster_for_display(&request, None, &mut cache).unwrap();
        let rrrah_core::RasterPixels::Rgba32Float(little_pixels) = little.pixels() else {
            panic!()
        };
        let rrrah_core::RasterPixels::Rgba32Float(big_pixels) = big.pixels() else {
            panic!()
        };
        assert_ne!(little_pixels[0].to_bits(), big_pixels[0].to_bits());
        assert_eq!(cache.len(), 2);
        request.rla_float_byte_order = None;
        assert!(load_cached_raster_for_display(&request, None, &mut cache).is_err());
        for (order, expected) in [
            (rrrah_decode::RlaFloatByteOrder::Little, &little),
            (rrrah_decode::RlaFloatByteOrder::Big, &big),
        ] {
            request.rla_float_byte_order = Some(order);
            let (hit, _) = load_cached_raster_with(&request, None, &mut cache, || {
                panic!("byte-order-specific result must hit its existing cache entry")
            })
            .unwrap();
            let rrrah_core::RasterPixels::Rgba32Float(a) = hit.pixels() else {
                panic!()
            };
            let rrrah_core::RasterPixels::Rgba32Float(b) = expected.pixels() else {
                panic!()
            };
            assert!(a.ptr_eq(b));
        }
        drop((little, big, cache));
        assert_eq!(budget.used(), 0);
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
        qualify_real_ai_pages(None);
    }
    #[test]
    #[ignore = "requires pinned AI corpus and actual Metal for cached production-page transport"]
    fn real_ai_cached_pages_preserve_metal_sdr_hdr_under_full_pressure() {
        assert_eq!(std::env::var("RRRAH_GPU_BACKEND").as_deref(), Ok("metal"));
        let gpu = navigation_metal_readback::qualification_gpu().expect("actual Metal required");
        eprintln!("real AI cache adapter: {}", gpu.adapter_name());
        qualify_real_ai_pages(Some(&gpu));
    }
    #[test]
    #[ignore = "requires pinned VectorApple original and actual Metal; observes render scratch admission"]
    fn real_ai_cancel_after_render_scratch_admission_recovers_metal() {
        assert_eq!(std::env::var("RRRAH_GPU_BACKEND").as_deref(), Ok("metal"));
        let source =
            PathBuf::from(std::env::var_os("RRRAH_AI_CORPUS").expect("AI corpus")).join("VectorApple.ai");
        let hash = std::process::Command::new("shasum")
            .args(["-a", "256"])
            .arg(&source)
            .output()
            .unwrap();
        assert!(hash.status.success());
        assert_eq!(
            String::from_utf8(hash.stdout).unwrap().split_whitespace().next(),
            Some("c5fc34346629c3379cee6065c45b7de0b620be822604551d1463d8e691b03d06")
        );
        let gpu = navigation_metal_readback::qualification_gpu().expect("actual Metal required");
        eprintln!("AI render cancellation adapter: {}", gpu.adapter_name());
        let reference_root = rrrah_core::MemoryBudget::new(64 * 1024 * 1024);
        let mut reference_request = DecodeRequest::new(&source);
        reference_request.memory_budget = Some(reference_root.clone());
        let (reference, _) = load_raster_for_display_typed(&reference_request, None).unwrap();
        let view = rrrah_gpu::ViewParameters {
            viewport: [128., 96.],
            ..Default::default()
        };
        let sdr = gpu.render_raster(&reference, view, [128, 96]).pixels;
        let hdr = gpu.render_raster_half(&reference, view, [128, 96]);
        let budget = rrrah_core::MemoryBudget::new(64 * 1024 * 1024);
        let generation = Arc::new(AtomicU64::new(1));
        let mut request = DecodeRequest::new(&source);
        request.memory_budget = Some(budget.clone());
        request.cancellation = Some(GenerationToken::new(generation.clone(), 1));
        // Two owned source copies plus final RGBA8 and renderer pixmap credit.
        // More live credit proves renderer scratch admission after page setup.
        let before_scratch = std::fs::metadata(&source).unwrap().len() * 2 + 301 * 246 * 8;
        let worker = thread::spawn(move || load_raster_for_display_typed(&request, None));
        let deadline = Instant::now() + Duration::from_secs(10);
        let mut observed = 0;
        while Instant::now() < deadline && !worker.is_finished() {
            observed = observed.max(budget.used());
            if observed > before_scratch {
                break;
            }
            thread::yield_now();
        }
        generation.store(2, Ordering::Release);
        let result = worker.join().unwrap();
        assert!(
            observed > before_scratch,
            "renderer scratch never observed: {observed} <= {before_scratch}"
        );
        assert!(matches!(
            result,
            Err(RasterLoadError::Decode(rrrah_decode::RasterDecodeError::Source(
                rrrah_decode::DecodeError::Cancelled
            )))
        ));
        assert_eq!(budget.used(), 0);
        let mut latest = DecodeRequest::new(&source);
        latest.memory_budget = Some(budget.clone());
        latest.cancellation = Some(GenerationToken::new(generation, 2));
        let mut cache = RasterDisplayCache::new(rrrah_cache::CacheLimits::bytes(32 * 1024 * 1024));
        let (retry, _) = load_cached_raster_for_display(&latest, None, &mut cache).unwrap();
        let (rrrah_core::RasterPixels::Rgba32Float(a), rrrah_core::RasterPixels::Rgba32Float(b)) =
            (reference.pixels(), retry.pixels())
        else {
            panic!()
        };
        assert_eq!(&a[..], &b[..]);
        assert_eq!(gpu.render_raster(&retry, view, [128, 96]).pixels, sdr);
        assert_eq!(gpu.render_raster_half(&retry, view, [128, 96]), hdr);
        drop((reference, retry, cache));
        assert_eq!((budget.used(), reference_root.used()), (0, 0));
        eprintln!(
            "AI render cancellation: live_credit={observed}; setup_credit={before_scratch}; typed_cancelled; retry_samples_Metal_SDR_HDR_exact; root=0; reference_root=0; appearance_unqualified"
        );
    }
    fn qualify_real_ai_pages(gpu: Option<&navigation_metal_readback::GpuReadback>) {
        let corpus = PathBuf::from(std::env::var("RRRAH_AI_CORPUS").unwrap());
        for (name, expected) in [
            (
                "VectorApple.ai",
                "c5fc34346629c3379cee6065c45b7de0b620be822604551d1463d8e691b03d06",
            ),
            (
                "one.ai",
                "97786949c268233a446e1738fb9c2f941e4d109811bd2f972948cdb7a45d64b4",
            ),
        ] {
            let hash = std::process::Command::new("shasum")
                .args(["-a", "256"])
                .arg(corpus.join(name))
                .output()
                .unwrap();
            assert!(hash.status.success());
            assert_eq!(
                String::from_utf8(hash.stdout).unwrap().split_whitespace().next(),
                Some(expected)
            );
        }
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
            let view = rrrah_gpu::ViewParameters {
                viewport: [128., 96.],
                ..Default::default()
            };
            let expected = gpu.map(|gpu| {
                (
                    gpu.render_raster(&original, view, [128, 96]).pixels,
                    gpu.render_raster_half(&original, view, [128, 96]),
                )
            });
            let pressure = budget.try_reserve(budget.available_bytes()).unwrap();
            assert_eq!(budget.available_bytes(), 0);
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
            if let (Some(gpu), Some((sdr, hdr))) = (gpu, expected) {
                assert_eq!(gpu.render_raster(&hit, view, [128, 96]).pixels, sdr);
                assert_eq!(gpu.render_raster_half(&hit, view, [128, 96]), hdr);
                eprintln!(
                    "real AI cache page: {name}; index={index}; shared_full_pressure_hit; Metal_SDR_HDR_exact"
                );
            }
            drop(pressure);
            assert_eq!(budget.used(), used);
        }
        assert_eq!(cache.len(), 3);
        drop(cache);
        assert_eq!(budget.used(), 0);
        eprintln!("real AI cache: pages=3; independent_keys=3; root=0; appearance_unqualified");
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
        assert_eq!(error, rrrah_decode::RasterColorError::InvalidInput.to_string());
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
    fn missing_pdf_font_failure_preserves_visible_cache_and_retries_same_path() {
        let fixtures = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/pdf");
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("font.pdf");
        let original = include_str!("../../../tests/fixtures/pdf/nested-inherited-font.pdf");
        let broken = original.replace("/F0 5 0 R", "/F1 5 0 R");
        assert_ne!(broken, original);
        std::fs::write(&path, broken).unwrap();
        let root = rrrah_cache::MemoryBudget::new(65536);
        let mut visible_request = DecodeRequest::new(fixtures.join("red-green-pages.pdf"));
        visible_request.memory_budget = Some(root.clone());
        let mut request = DecodeRequest::new(&path);
        request.memory_budget = Some(root.clone());
        let mut cache = RasterDisplayCache::new(rrrah_cache::CacheLimits {
            max_bytes: 65536,
            max_entries: Some(2),
            ttl: None,
        });
        let (visible, _) = load_cached_raster_for_display(&visible_request, None, &mut cache).unwrap();
        let used = root.used();
        let gate = Arc::new(DecodeGate::new());
        for prefetch in [false, true] {
            let result = if prefetch {
                preload_raster(&request, None, &mut cache, &gate)
            } else {
                load_cached_raster_for_display(&request, None, &mut cache)
            };
            let error = result.unwrap_err();
            assert!(error.contains("missing font resource"), "{error}");
            assert_eq!(cache.len(), 1);
            assert_eq!(root.used(), used);
            assert!(cache.get_visible_lease_for(&visible).is_some());
            let pressure = root.try_reserve(root.available_bytes()).unwrap();
            let (hit, _) = load_cached_raster_with(&visible_request, None, &mut cache, || {
                panic!("failed PDF evicted visible frame")
            })
            .unwrap();
            let (rrrah_core::RasterPixels::Rgba32Float(a), rrrah_core::RasterPixels::Rgba32Float(b)) =
                (visible.pixels(), hit.pixels())
            else {
                panic!()
            };
            assert!(a.ptr_eq(b));
            assert!(b.chunks_exact(4).all(|pixel| pixel == [1., 0., 0., 1.]));
            drop((hit, pressure));
            assert_eq!(root.used(), used);
        }
        // Repair the same file, exercising source identity and releasing the
        // failed prefetch's gate permit rather than using a separate request key.
        std::fs::write(&path, original).unwrap();
        let (repaired, _) = preload_raster(&request, None, &mut cache, &gate).unwrap();
        assert_eq!(cache.len(), 2);
        assert_eq!((repaired.width(), repaired.height()), (100, 20));
        let (hit, _) = load_cached_raster_with(&request, None, &mut cache, || {
            panic!("repaired PDF did not enter RAM cache")
        })
        .unwrap();
        let (rrrah_core::RasterPixels::Rgba32Float(a), rrrah_core::RasterPixels::Rgba32Float(b)) =
            (repaired.pixels(), hit.pixels())
        else {
            panic!()
        };
        assert!(a.ptr_eq(b));
        drop((hit, repaired, visible, cache));
        assert_eq!(root.used(), 0);
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
        let palette_time = std::fs::metadata(&palette).unwrap().modified().unwrap();
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
        std::fs::File::options()
            .write(true)
            .open(&palette)
            .unwrap()
            .set_times(std::fs::FileTimes::new().set_modified(palette_time))
            .unwrap();
        let second = loader(job).unwrap();
        assert_ne!(first.source_stamp, second.source_stamp);
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
            worker: None,
            decode_first: true,
            automatic_policy: false,
            flight: Arc::new(std::sync::Mutex::new(None)),
            duplicate_validator: source_validation::SourceValidator::new(),
            duplicate: std::sync::Mutex::new(None),
            tx,
            pending: worker_rx.clone(),
            generation: Arc::new(AtomicU64::new(0)),
            decode_gate: Arc::new(DecodeGate::new()),
            telemetry: Arc::new(CacheTelemetry::new(true, 1024)),
            development: None,
        };
        let first = loader.submit(path.clone()).unwrap();
        assert!(
            loader.flight.lock().unwrap().as_ref().unwrap().0.source.is_none(),
            "a fresh selection leaves file fingerprinting to the worker"
        );
        assert_eq!(loader.submit(path.clone()).unwrap(), first);
        assert!(
            loader.flight.lock().unwrap().as_ref().unwrap().0.source.is_none(),
            "a pending duplicate must not read the source on its caller"
        );
        loader.flight.lock().unwrap().as_mut().unwrap().0.source = Some(gallery::SourceStamp::read(&path)); // Simulate the worker starting.
        let active = worker_rx.try_recv().unwrap();
        let token = GenerationToken::new(Arc::clone(&loader.generation), first);
        let gate_generation = loader.decode_gate.speculative_generation();
        let gate_before = gate_generation.load(Ordering::Acquire);
        let neighbours = vec![root.join("previous.png"), root.join("next.png")];
        assert_eq!(
            loader
                .submit_request_with_prefetch(path.clone(), 0, None, true, Some(neighbours.clone()))
                .unwrap(),
            first
        );
        assert_eq!(*active.prefetch_paths.lock().unwrap(), Some(neighbours));
        assert_eq!(
            loader
                .submit_request_with_prefetch(path.clone(), 0, None, true, Some(Vec::new()))
                .unwrap(),
            first
        );
        assert_eq!(*active.prefetch_paths.lock().unwrap(), Some(Vec::new()));
        assert!(!token.is_cancelled());
        assert!(worker_rx.try_recv().is_err());
        let deadline = Instant::now() + Duration::from_secs(5);
        while loader.has_pending_duplicate() {
            assert!(
                loader.poll_duplicate().is_none(),
                "unchanged source must not restart decode"
            );
            assert!(Instant::now() < deadline);
            thread::yield_now();
        }
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
        let joined = loader
            .submit_image_with_window(path.clone(), 1, Some(window))
            .unwrap();
        assert_eq!(joined, recipe);
        let deadline = Instant::now() + Duration::from_secs(5);
        let changed = loop {
            if let Some(result) = loader.poll_duplicate() {
                break result.unwrap();
            }
            assert!(Instant::now() < deadline, "changed source was not reloaded");
            thread::yield_now();
        };
        assert_ne!(changed, recipe);
        let reloaded = worker_rx.try_recv().unwrap();
        assert_eq!(reloaded.generation, changed);
        assert_eq!(reloaded.path, path);
        assert_eq!(reloaded.image_index, 1);
        assert_eq!(reloaded.scalar_window, Some(window));
        assert_eq!(reloaded.development, loader.development);
        drop(reloaded);
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
        assert_eq!(loader.submit(path).unwrap(), missing);
        // The pending worker has not read this missing source yet; it will
        // publish its failure without requiring another speculative restart.
        drop(active);
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn changed_source_retry_keeps_explicit_or_disabled_prefetch_plan() {
        for disabled in [false, true] {
            let directory = tempfile::tempdir().unwrap();
            let path = directory.path().join("source.png");
            std::fs::write(&path, b"original source").unwrap();
            let plan = if disabled {
                Vec::new()
            } else {
                vec![
                    directory.path().join("previous.png"),
                    directory.path().join("next.png"),
                ]
            };
            let (loader, requests) = loader_with_validator(source_validation::SourceValidator::new());
            let generation = loader
                .submit_request_with_prefetch(path.clone(), 0, None, true, Some(plan.clone()))
                .unwrap();
            let active = requests.try_recv().unwrap();
            loader.flight.lock().unwrap().as_mut().unwrap().0.source =
                Some(gallery::SourceStamp::read(&path));
            std::fs::write(&path, b"changed source with different contents").unwrap();
            // A plain duplicate carries no replacement plan. It must retain
            // the explicit selection when validation restarts the decode.
            assert_eq!(loader.submit(path.clone()).unwrap(), generation);
            let deadline = Instant::now() + Duration::from_secs(5);
            let replacement = loop {
                if let Some(result) = loader.poll_duplicate() {
                    break result.unwrap();
                }
                assert!(Instant::now() < deadline);
                thread::yield_now();
            };
            assert_ne!(replacement, generation);
            let retry = requests.try_recv().unwrap();
            assert_eq!(retry.path, path);
            assert_eq!(*retry.prefetch_paths.lock().unwrap(), Some(plan));
            let active_plan = active.prefetch_paths.lock().unwrap().clone();
            let retry_plan = retry.prefetch_paths.lock().unwrap().clone();
            assert_eq!(active_plan, retry_plan);
        }
    }

    fn loader_with_validator(
        validator: source_validation::SourceValidator,
    ) -> (ForegroundLoader, Receiver<LoadRequest>) {
        let (tx, requests) = bounded(1);
        let loader = ForegroundLoader {
            worker: None,
            decode_first: true,
            automatic_policy: false,
            flight: Arc::new(std::sync::Mutex::new(None)),
            duplicate_validator: validator,
            duplicate: std::sync::Mutex::new(None),
            tx,
            pending: requests.clone(),
            generation: Arc::new(AtomicU64::new(0)),
            decode_gate: Arc::new(DecodeGate::new()),
            telemetry: Arc::new(CacheTelemetry::new(true, 1024)),
            development: None,
        };
        (loader, requests)
    }

    #[test]
    fn animation_prefetch_phase_updates_coalesced_request_without_redecode() {
        let (loader, requests) = loader_with_validator(source_validation::SourceValidator::new());
        let path = PathBuf::from("phase.gif");
        let generation = loader
            .submit_animation_frame(path.clone(), 2, None, true)
            .unwrap();
        let request = requests.try_recv().unwrap();
        assert_eq!(request.raster_prefetch_hint.load(Ordering::Acquire), 2);
        assert_eq!(
            loader
                .submit_animation_frame(path.clone(), 2, None, false)
                .unwrap(),
            generation
        );
        assert_eq!(request.raster_prefetch_hint.load(Ordering::Acquire), 1);
        assert!(requests.try_recv().is_err());
        assert_eq!(
            loader
                .submit_request_with_prefetch(path, 2, None, true, None)
                .unwrap(),
            generation
        );
        assert_eq!(request.raster_prefetch_hint.load(Ordering::Acquire), 0);
        assert!(requests.try_recv().is_err());
    }

    #[test]
    fn animation_controller_submits_once_and_waits_for_ready_then_presentation() {
        let (mut app, requests) = app_with_drop_inspector(drop_inspection::DropInspector::new());
        app.path = PathBuf::from("animation.gif");
        app.image_index = 0;
        app.image_count = 4;
        let initial = app.foreground_loader.current_generation();
        let elapsed = Instant::now() - Duration::from_secs(1);
        let timing = rrrah_core::RasterAnimationTiming::new(100, 1, 0).unwrap();
        app.ready_generation = Some(initial);
        app.playback.ready(initial, 0, 4, Some(timing));
        app.playback.presented(initial, elapsed);
        app.advance_animation();
        let first = requests.try_recv().unwrap();
        assert_eq!(first.image_index, 1);
        assert_eq!(first.path, app.path);
        assert_eq!(app.pending_image_index, Some((first.generation, 1)));
        assert_eq!(app.pipeline.generation(), first.generation);
        for _ in 0..10 {
            app.advance_animation();
        }
        assert!(requests.try_recv().is_err());
        assert_eq!(app.foreground_loader.current_generation(), first.generation);
        app.ready_generation = Some(first.generation);
        for _ in 0..10 {
            app.advance_animation();
        }
        assert!(requests.try_recv().is_err());
        app.playback.ready(first.generation, 1, 4, Some(timing));
        app.advance_animation();
        assert!(
            requests.try_recv().is_err(),
            "ready without presentation must not advance"
        );
        app.playback.presented(first.generation, elapsed);
        app.advance_animation();
        let second = requests.try_recv().unwrap();
        assert_eq!(second.image_index, 2);
        assert_ne!(second.generation, first.generation);
        assert_eq!(app.pending_image_index, Some((second.generation, 2)));
    }
    #[test]
    fn changed_animation_source_discards_old_repeat_phase_before_retry() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("changed.gif");
        std::fs::write(&path, b"old").unwrap();
        let stamp = gallery::SourceStamp::read(&path);
        let (loader, requests) = loader_with_validator(source_validation::SourceValidator::new());
        let first = loader
            .submit_animation_frame(path.clone(), 1, None, false)
            .unwrap();
        let active = requests.try_recv().unwrap();
        loader.flight.lock().unwrap().as_mut().unwrap().0.source = Some(stamp);
        std::fs::write(&path, b"changed content and size").unwrap();
        assert_eq!(
            loader
                .submit_animation_frame(path.clone(), 1, None, false)
                .unwrap(),
            first
        );
        let deadline = Instant::now() + Duration::from_secs(5);
        let replacement = loop {
            if let Some(result) = loader.poll_duplicate() {
                break result.unwrap();
            }
            assert!(Instant::now() < deadline, "source validation did not finish");
            thread::yield_now();
        };
        assert_ne!(replacement, first);
        let retry = requests.try_recv().unwrap();
        assert_eq!(retry.path, path);
        assert_eq!(retry.image_index, 1);
        assert_eq!(retry.generation, replacement);
        assert_eq!(retry.raster_prefetch_hint.load(Ordering::Acquire), 0);
        assert_eq!(active.raster_prefetch_hint.load(Ordering::Acquire), 0);
        assert!(requests.try_recv().is_err());
    }

    #[test]
    fn animation_controller_final_pass_clears_wrap_hint_and_stops_requests() {
        let (mut app, requests) = app_with_drop_inspector(drop_inspection::DropInspector::new());
        app.path = PathBuf::from("two-passes.gif");
        app.image_count = 3;
        let timing = rrrah_core::RasterAnimationTiming::new(10, 1, 2).unwrap();
        let mut generation = app.foreground_loader.current_generation();
        for step in 0..6 {
            let index = step % 3;
            app.image_index = index;
            app.ready_generation = Some(generation);
            app.pending_image_index = None;
            app.playback.ready(generation, index, 3, Some(timing));
            app.playback
                .presented(generation, Instant::now() - Duration::from_secs(1));
            app.advance_animation();
            if step == 5 {
                assert!(
                    requests.try_recv().is_err(),
                    "finite playback must stop after two passes"
                );
                assert!(app.playback.deadline(generation).is_none());
                for _ in 0..8 {
                    app.advance_animation();
                }
                assert!(requests.try_recv().is_err());
                break;
            }
            let request = requests.try_recv().unwrap();
            assert_eq!(request.image_index, (step + 1) % 3);
            let hint = request.raster_prefetch_hint.load(Ordering::Acquire);
            assert_eq!(hint, if step < 2 { 2 } else { 1 });
            let plan = gallery::container_prefetch_plan(
                3,
                request.image_index,
                Some(index),
                true,
                Some(hint == 2),
                gallery::PrefetchWindow { ahead: 2, behind: 0 },
            );
            assert_eq!(plan.direction, gallery::NavDirection::Forward);
            assert_eq!(plan.cyclic, step < 2);
            if step == 4 {
                assert!(plan.indices.is_empty(), "final frame must not warm a third pass");
            }
            generation = request.generation;
            for _ in 0..8 {
                app.advance_animation();
            }
            assert!(
                requests.try_recv().is_err(),
                "pending frame must not advance twice"
            );
        }
    }

    #[test]
    fn animation_controller_pause_blocks_expired_deadline_until_resume() {
        let (mut app, requests) = app_with_drop_inspector(drop_inspection::DropInspector::new());
        app.path = PathBuf::from("animation.gif");
        app.image_count = 4;
        let generation = app.foreground_loader.current_generation();
        let now = Instant::now();
        let timing = rrrah_core::RasterAnimationTiming::new(100, 1, 0).unwrap();
        app.ready_generation = Some(generation);
        app.playback.ready(generation, 0, 4, Some(timing));
        app.playback.presented(generation, now - Duration::from_secs(1));
        assert_eq!(app.playback.toggle_pause(generation, now), Some(true));
        for _ in 0..10 {
            app.advance_animation();
        }
        assert!(requests.try_recv().is_err());
        assert_eq!(app.foreground_loader.current_generation(), generation);
        app.playback.toggle_pause(generation, Instant::now());
        app.advance_animation();
        assert_eq!(requests.try_recv().unwrap().image_index, 1);
    }
    #[test]
    fn animation_controller_never_replaces_a_new_source_request_with_old_timer() {
        let (mut app, requests) = app_with_drop_inspector(drop_inspection::DropInspector::new());
        app.path = PathBuf::from("old.gif");
        app.image_count = 4;
        let generation = app.foreground_loader.current_generation();
        let timing = rrrah_core::RasterAnimationTiming::new(100, 1, 0).unwrap();
        app.ready_generation = Some(generation);
        app.playback.ready(generation, 0, 4, Some(timing));
        app.playback
            .presented(generation, Instant::now() - Duration::from_secs(1));
        let replacement = app
            .foreground_loader
            .submit_image_with_window(PathBuf::from("replacement.png"), 0, None)
            .unwrap();
        for _ in 0..10 {
            app.advance_animation();
        }
        assert_eq!(app.foreground_loader.current_generation(), replacement);
        let request = requests.try_recv().unwrap();
        assert_eq!(request.path, PathBuf::from("replacement.png"));
        assert_eq!(request.generation, replacement);
        assert!(requests.try_recv().is_err());
    }

    #[test]
    fn manual_animation_boundary_keeps_active_deadline_and_does_not_submit() {
        let (mut app, requests) = app_with_drop_inspector(drop_inspection::DropInspector::new());
        app.path = PathBuf::from("animation.gif");
        app.image_index = 0;
        app.image_count = 4;
        let generation = app.foreground_loader.current_generation();
        let now = Instant::now();
        let timing = rrrah_core::RasterAnimationTiming::new(100, 1, 3).unwrap();
        app.playback.ready(generation, 0, 4, Some(timing));
        app.playback.presented(generation, now);
        let deadline = app.playback.deadline(generation);
        app.navigate_image(-1);
        assert_eq!(app.playback.deadline(generation), deadline);
        assert_eq!(app.foreground_loader.current_generation(), generation);
        assert!(requests.try_recv().is_err());
    }

    #[test]
    fn manual_animation_seek_keeps_paused_through_actual_app_submission() {
        let (mut app, requests) = app_with_drop_inspector(drop_inspection::DropInspector::new());
        app.path = PathBuf::from("animation.gif");
        app.image_index = 0;
        app.image_count = 4;
        let generation = app.foreground_loader.current_generation();
        let now = Instant::now();
        let timing = rrrah_core::RasterAnimationTiming::new(100, 1, 3).unwrap();
        app.playback.ready(generation, 0, 4, Some(timing));
        app.playback.presented(generation, now);
        app.playback
            .toggle_pause(generation, now + Duration::from_millis(30));
        app.navigate_image(1);
        let request = requests.try_recv().unwrap();
        assert_eq!(request.image_index, 1);
        assert!(app.playback.is_paused(request.generation));
        assert!(app.playback.deadline(request.generation).is_none());
        app.playback.ready(request.generation, 1, 4, Some(timing));
        app.playback.presented(request.generation, now);
        assert!(app.playback.is_paused(request.generation));
        app.playback.toggle_pause(request.generation, now);
        assert_eq!(
            app.playback.deadline(request.generation),
            Some(now + Duration::from_millis(100))
        );
    }

    fn app_with_drop_inspector(inspector: drop_inspection::DropInspector) -> (App, Receiver<LoadRequest>) {
        app_with_prefetch_windows(
            inspector,
            gallery::PrefetchWindow { behind: 0, ahead: 0 },
            gallery::PrefetchWindow { behind: 0, ahead: 0 },
        )
    }

    fn app_with_prefetch_windows(
        inspector: drop_inspection::DropInspector,
        window: gallery::PrefetchWindow,
        raw_disk_window: gallery::PrefetchWindow,
    ) -> (App, Receiver<LoadRequest>) {
        let (loader, requests) = loader_with_validator(source_validation::SourceValidator::new());
        let gate = loader.decode_gate.clone();
        let telemetry = loader.telemetry.clone();
        let (_, receiver) = unbounded();
        let mut app = App::new(
            PathBuf::new(),
            None,
            true,
            receiver,
            loader,
            gate,
            telemetry,
            window,
            raw_disk_window,
            rrrah_cache::CacheLimits {
                max_bytes: 0,
                max_entries: Some(0),
                ttl: None,
            },
            None,
            None,
            None,
            None,
            None,
            rrrah_gpu::GpuBackend::Auto,
            rrrah_gpu::GpuVendor::Any,
            rrrah_gpu::GpuDemosaicAlgorithm::Bilinear,
            UntaggedColor::Strict.into(),
        );
        app.drop_inspector = inspector;
        (app, requests)
    }

    #[test]
    fn disabled_raw_disk_warmer_preserves_gallery_prefetch_in_both_directions() {
        let (mut app, requests) = app_with_prefetch_windows(
            drop_inspection::DropInspector::new(),
            gallery::PrefetchWindow { behind: 2, ahead: 1 },
            gallery::PrefetchWindow { behind: 0, ahead: 0 },
        );
        let paths: Vec<_> = (0..6)
            .map(|index| PathBuf::from(format!("frame-{index}.png")))
            .collect();
        app.gallery = paths.clone();
        app.gallery_index = Some(2);
        app.path = paths[2].clone();
        for (direction, selected, neighbours) in [(1, 3, [4, 2, 1]), (-1, 2, [1, 3, 4])] {
            app.navigate_gallery(direction);
            let request = requests.try_recv().unwrap();
            assert_eq!(request.path, paths[selected]);
            assert_eq!(
                *request.prefetch_paths.lock().unwrap(),
                Some(neighbours.map(|index| paths[index].clone()).to_vec())
            );
            assert_eq!((app.prefetch_window.behind, app.prefetch_window.ahead), (2, 1));
        }
    }

    #[test]
    fn rapid_page_navigation_accumulates_pending_intent_and_reverses() {
        let (mut app, requests) = app_with_drop_inspector(drop_inspection::DropInspector::new());
        app.path = PathBuf::from("pages.tif");
        app.image_count = 3;
        let mut previous = None;
        for (direction, index) in [(1, 1), (1, 2), (-1, 1), (-1, 0)] {
            app.navigate_image(direction);
            let request = requests.try_recv().unwrap();
            assert_eq!(request.image_index, index);
            assert_eq!(app.image_index, 0, "visible page changes only when accepted");
            assert_eq!(app.image_navigation_index(), index);
            if let Some(token) = previous {
                assert!(GenerationToken::is_cancelled(&token));
            }
            previous = Some(GenerationToken::new(
                app.foreground_loader.generation.clone(),
                request.generation,
            ));
        }
        let generation = app.foreground_loader.current_generation();
        app.navigate_image(-1);
        assert_eq!(app.foreground_loader.current_generation(), generation);
        assert!(requests.try_recv().is_err());
    }

    #[test]
    fn stale_page_failure_retains_latest_intent_and_current_failure_resets_it() {
        let (mut app, requests) = app_with_drop_inspector(drop_inspection::DropInspector::new());
        let (events, receiver) = unbounded();
        app.receiver = receiver;
        app.path = PathBuf::from("pages.tif");
        app.image_count = 3;
        app.navigate_image(1);
        let old = requests.try_recv().unwrap();
        app.navigate_image(1);
        let latest = requests.try_recv().unwrap();
        let raster = rrrah_core::DecodedRaster::new(
            1,
            1,
            rrrah_core::RasterPixels::Rgba32Float(Arc::new(vec![1., 0., 0., 1.]).into()),
            rrrah_core::RasterColorSpace::LinearSrgb,
        )
        .unwrap()
        .with_image_selection(1, 3)
        .unwrap();
        events
            .send(LoadEvent::RasterReady {
                region_store: None,
                generation: old.generation,
                image_index: 1,
                scalar_window: None,
                raster,
                lease: None,
                assumed_srgb: false,
                development: None,
                elapsed: Duration::ZERO,
            })
            .unwrap();
        events
            .send(LoadEvent::Failed {
                generation: old.generation,
                error: "obsolete".into(),
            })
            .unwrap();
        app.drain_load_events();
        assert_eq!(app.image_navigation_index(), 2);
        assert_eq!(app.image_index, 0, "stale raster must not replace visible page");
        events
            .send(LoadEvent::Failed {
                generation: latest.generation,
                error: "current".into(),
            })
            .unwrap();
        app.drain_load_events();
        assert_eq!(app.image_navigation_index(), 0);
        app.navigate_image(1);
        assert_eq!(requests.try_recv().unwrap().image_index, 1);
        app.foreground_loader.cancel_pending();
        assert_eq!(
            app.image_navigation_index(),
            0,
            "cancelled generation cannot steer navigation"
        );
    }

    #[test]
    fn opening_empty_folder_cancels_old_load_and_drains_queued_file() {
        let root = tempfile::tempdir().unwrap();
        let (mut app, requests) = app_with_drop_inspector(drop_inspection::DropInspector::new());
        app.foreground_loader
            .submit(root.path().join("active.png"))
            .unwrap();
        let active = requests.try_recv().unwrap();
        let active_token = GenerationToken::new(app.foreground_loader.generation.clone(), active.generation);
        app.foreground_loader
            .submit(root.path().join("queued.png"))
            .unwrap();
        let queued_token = GenerationToken::new(
            app.foreground_loader.generation.clone(),
            app.foreground_loader.current_generation(),
        );
        assert!(!queued_token.is_cancelled());
        app.open_folder(root.path().to_owned());
        assert!(active_token.is_cancelled());
        assert!(queued_token.is_cancelled());
        assert!(requests.try_recv().is_err());
        assert!(app.foreground_loader.flight.lock().unwrap().is_none());
        let deadline = Instant::now() + Duration::from_secs(5);
        while !app.folder_scanner.has_ready() {
            assert!(Instant::now() < deadline);
            thread::yield_now();
        }
        app.drain_folder_scan();
        assert!(app.gallery.is_empty());
        assert!(requests.try_recv().is_err());
        drop(active);
    }

    #[test]
    fn opening_empty_folder_never_reloads_previous_animation_from_expired_timer() {
        let folder = tempfile::tempdir().unwrap();
        let (mut app, requests) = app_with_drop_inspector(drop_inspection::DropInspector::new());
        app.path = PathBuf::from("previous.gif");
        app.gallery = vec![app.path.clone()];
        app.gallery_index = Some(0);
        app.image_count = 4;
        let generation = app.foreground_loader.current_generation();
        let timing = rrrah_core::RasterAnimationTiming::new(100, 1, 0).unwrap();
        app.ready_generation = Some(generation);
        app.playback.ready(generation, 0, 4, Some(timing));
        app.playback
            .presented(generation, Instant::now() - Duration::from_secs(1));
        app.open_folder(folder.path().to_owned());
        assert_ne!(app.foreground_loader.current_generation(), generation);
        app.advance_animation();
        assert!(requests.try_recv().is_err());
        let deadline = Instant::now() + Duration::from_secs(5);
        while !app.folder_scanner.has_ready() {
            assert!(Instant::now() < deadline);
            thread::yield_now();
        }
        app.drain_folder_scan();
        assert!(app.gallery.is_empty());
        assert_eq!(app.gallery_index, None);
        for _ in 0..10 {
            app.advance_animation();
        }
        assert!(requests.try_recv().is_err());
        assert!(
            app.playback
                .deadline(app.foreground_loader.current_generation())
                .is_none()
        );
        assert_eq!(
            app.path,
            PathBuf::from("previous.gif"),
            "previous visible image may stay while new folder is empty"
        );
    }
    #[test]
    fn passive_empty_sibling_scan_does_not_stop_current_standalone_animation() {
        let folder = tempfile::tempdir().unwrap();
        let (mut app, requests) = app_with_drop_inspector(drop_inspection::DropInspector::new());
        app.path = PathBuf::from("standalone.gif");
        app.image_count = 4;
        let generation = app.foreground_loader.current_generation();
        let timing = rrrah_core::RasterAnimationTiming::new(100, 1, 0).unwrap();
        app.ready_generation = Some(generation);
        app.playback.ready(generation, 0, 4, Some(timing));
        app.playback
            .presented(generation, Instant::now() - Duration::from_secs(1));
        let expected = app.playback.deadline(generation);
        app.request_folder_scan(folder.path().to_owned(), false);
        let deadline = Instant::now() + Duration::from_secs(5);
        while !app.folder_scanner.has_ready() {
            assert!(Instant::now() < deadline);
            thread::yield_now();
        }
        app.drain_folder_scan();
        assert!(app.gallery.is_empty());
        assert_eq!(app.playback.deadline(generation), expected);
        app.advance_animation();
        let next = requests.try_recv().unwrap();
        assert_eq!(next.path, PathBuf::from("standalone.gif"));
        assert_eq!(next.image_index, 1);
    }

    #[test]
    fn empty_folder_scan_clears_previous_navigation_without_reopening_old_files() {
        let root = tempfile::tempdir().unwrap();
        for open_first in [false, true] {
            let (mut app, requests) = app_with_drop_inspector(drop_inspection::DropInspector::new());
            let previous = root.path().join("previous.png");
            app.path = previous.clone();
            app.gallery = vec![previous.clone()];
            app.gallery_index = Some(0);
            app.request_folder_scan(root.path().to_owned(), open_first);
            let deadline = Instant::now() + Duration::from_secs(5);
            while !app.folder_scanner.has_ready() {
                assert!(Instant::now() < deadline);
                thread::yield_now();
            }
            app.drain_folder_scan();
            assert!(app.pending_folder_scan.is_none());
            assert!(app.gallery.is_empty());
            assert_eq!(app.gallery_index, None);
            app.open_gallery_index(0);
            assert!(requests.try_recv().is_err());
            assert_eq!(app.path, previous);
        }
    }

    #[test]
    fn rejected_drop_preserves_queued_folder_context_without_opening_it_early() {
        let root = tempfile::tempdir().unwrap();
        let folder = root.path().join("collection");
        std::fs::create_dir(&folder).unwrap();
        let image = folder.join("image.png");
        std::fs::write(&image, b"fixture").unwrap();
        let (started, observed) = bounded(1);
        let (release, gate) = bounded(1);
        let inspector = drop_inspection::DropInspector::with_inspector(move |_| {
            started.send(()).unwrap();
            gate.recv_timeout(Duration::from_secs(5)).unwrap();
            Err("unsupported drop".into())
        });
        let (mut app, requests) = app_with_drop_inspector(inspector);
        app.request_folder_scan(folder, true);
        let expected_scan = app.pending_folder_scan;
        let deadline = Instant::now() + Duration::from_secs(5);
        while !app.folder_scanner.has_ready() {
            assert!(Instant::now() < deadline);
            thread::yield_now();
        }
        app.open_dropped_path("invalid.txt".into());
        observed.recv_timeout(Duration::from_secs(5)).unwrap();
        app.drain_folder_scan();
        assert_eq!(app.pending_folder_scan, expected_scan);
        assert!(app.folder_scanner.has_ready());
        assert!(requests.try_recv().is_err());
        release.send(()).unwrap();
        while app.pending_drop.is_some() {
            app.drain_dropped_paths();
            assert!(Instant::now() < deadline);
            thread::yield_now();
        }
        assert!(app.status.starts_with("drop rejected:"));
        app.drain_folder_scan();
        assert!(app.pending_folder_scan.is_none());
        assert_eq!(requests.try_recv().unwrap().path, image);
        assert_eq!(app.path, image);
    }

    #[test]
    fn newer_gallery_navigation_cancels_pending_drop() {
        let (started, observed) = bounded(1);
        let (release, gate) = bounded(1);
        let inspector = drop_inspection::DropInspector::with_inspector(move |_| {
            started.send(()).unwrap();
            gate.recv_timeout(Duration::from_secs(5)).unwrap();
            Ok(drop_inspection::DropKind::File)
        });
        let (mut app, requests) = app_with_drop_inspector(inspector);
        app.gallery = vec![PathBuf::from("chosen.png")];
        app.open_dropped_path("late.png".into());
        observed.recv_timeout(Duration::from_secs(5)).unwrap();
        app.open_gallery_index(0);
        assert!(app.pending_drop.is_none());
        assert_eq!(requests.try_recv().unwrap().path, PathBuf::from("chosen.png"));
        release.send(()).unwrap();
        app.drain_dropped_paths();
        assert_eq!(app.path, PathBuf::from("chosen.png"));
        assert!(requests.try_recv().is_err());
    }

    #[test]
    fn cancelled_startup_admission_never_reads_the_source() {
        let generation = Arc::new(AtomicU64::new(2));
        let token = GenerationToken::new(generation, 1);
        assert!(
            admit_foreground_source(Path::new("cancelled.png"), &token, |_| panic!(
                "cancelled source must not be inspected"
            ))
            .is_none()
        );
    }

    #[test]
    fn blocked_startup_admission_does_not_block_navigation_or_publish_obsolete_error() {
        let (loader, requests) = loader_with_validator(source_validation::SourceValidator::new());
        loader.submit_initial("blocked.png".into()).unwrap();
        let active = requests.try_recv().unwrap();
        let token = GenerationToken::new(loader.generation.clone(), active.generation);
        let (started, observed) = bounded(1);
        let (release, gate) = bounded(1);
        let worker = thread::spawn(move || {
            let result = admit_foreground_source(&active.path, &token, |path| {
                started.send(()).unwrap();
                gate.recv_timeout(Duration::from_secs(5)).unwrap();
                Err(rrrah_cache::CacheError::NotAFile(path.to_path_buf()))
            });
            drop(active);
            result
        });
        observed.recv_timeout(Duration::from_secs(5)).unwrap();
        let latest = loader.submit("latest.png".into()).unwrap();
        release.send(()).unwrap();
        assert!(worker.join().unwrap().is_none());
        assert_eq!(loader.current_generation(), latest);
        let queued = requests.try_recv().unwrap();
        assert_eq!(queued.path, PathBuf::from("latest.png"));
        assert_eq!(queued.generation, latest);
    }

    #[test]
    fn startup_admission_error_reaches_app_and_later_selection_can_load() {
        let directory = tempfile::tempdir().unwrap();
        let missing = directory.path().join("missing.png");
        let valid = directory.path().join("valid.png");
        std::fs::write(&valid, b"source bytes").unwrap();
        let (mut app, requests) = app_with_drop_inspector(drop_inspection::DropInspector::new());
        let (send, receive) = unbounded();
        app.receiver = receive;
        app.path = missing.clone();
        app.foreground_loader.submit_initial(missing.clone()).unwrap();
        app.raw_prefetcher.begin_foreground();
        let request = requests.try_recv().unwrap();
        let token = GenerationToken::new(app.foreground_loader.generation.clone(), request.generation);
        let error = admit_foreground_source(&missing, &token, gallery::SourceStamp::try_read)
            .unwrap()
            .unwrap_err();
        send.send(LoadEvent::Failed {
            generation: request.generation,
            error,
        })
        .unwrap();
        drop(request);
        app.drain_load_events();
        assert!(app.status.starts_with("image load failed:"));
        assert!(app.status.contains("missing.png"));
        app.gallery = vec![valid.clone()];
        app.open_gallery_index(0);
        let next = requests.try_recv().unwrap();
        assert_eq!(next.path, valid);
        assert_eq!(next.generation, 1);
        assert!(gallery::SourceStamp::try_read(&next.path).is_ok());
        assert!(app.status.starts_with("decoding image"));
    }

    #[test]
    fn blocked_duplicate_validation_keeps_ui_submissions_and_navigation_responsive() {
        let (started, observed) = bounded(1);
        let (release, gate) = bounded(1);
        let (finished, complete) = bounded(1);
        let validator = source_validation::SourceValidator::with_reader(move |_| {
            started.send(()).unwrap();
            gate.recv_timeout(Duration::from_secs(5)).unwrap();
            finished.send(()).unwrap();
            gallery::SourceStamp::default()
        });
        let (loader, requests) = loader_with_validator(validator);
        let first = loader.submit("first.png".into()).unwrap();
        let active = requests.try_recv().unwrap();
        loader.flight.lock().unwrap().as_mut().unwrap().0.source = Some(gallery::SourceStamp::default());
        let token = GenerationToken::new(loader.generation.clone(), first);
        let speculative = loader.decode_gate.speculative_generation();
        let before = speculative.load(Ordering::Acquire);
        assert_eq!(loader.submit("first.png".into()).unwrap(), first);
        observed.recv_timeout(Duration::from_secs(5)).unwrap();
        for _ in 0..100 {
            assert_eq!(loader.submit("first.png".into()).unwrap(), first);
            assert!(loader.poll_duplicate().is_none());
        }
        assert!(!token.is_cancelled());
        assert_eq!(speculative.load(Ordering::Acquire), before);
        assert!(requests.try_recv().is_err());
        let latest = loader.submit("latest.png".into()).unwrap();
        assert_ne!(latest, first);
        assert!(!loader.has_pending_duplicate());
        release.send(()).unwrap();
        complete.recv_timeout(Duration::from_secs(5)).unwrap();
        assert!(loader.poll_duplicate().is_none());
        assert_eq!(loader.current_generation(), latest);
        assert_eq!(requests.try_recv().unwrap().path, PathBuf::from("latest.png"));
        drop(active);
    }

    #[test]
    fn new_duplicate_cannot_reuse_validation_completed_before_source_change() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("source.png");
        std::fs::write(&path, b"old").unwrap();
        let stamp = gallery::SourceStamp::read(&path);
        let (loader, requests) = loader_with_validator(source_validation::SourceValidator::new());
        let first = loader.submit(path.clone()).unwrap();
        let active = requests.try_recv().unwrap();
        loader.flight.lock().unwrap().as_mut().unwrap().0.source = Some(stamp);
        assert_eq!(loader.submit(path.clone()).unwrap(), first);
        let deadline = Instant::now() + Duration::from_secs(5);
        while !loader
            .duplicate
            .lock()
            .unwrap()
            .as_ref()
            .unwrap()
            .check
            .has_result()
        {
            assert!(Instant::now() < deadline);
            thread::yield_now();
        }
        // The first successful result has not yet been consumed by the UI.
        std::fs::write(&path, b"new").unwrap();
        assert_eq!(loader.submit(path.clone()).unwrap(), first);
        let replacement = loop {
            if let Some(result) = loader.poll_duplicate() {
                break result.unwrap();
            }
            assert!(
                Instant::now() < deadline,
                "new request reused obsolete validation"
            );
            thread::yield_now();
        };
        assert_ne!(replacement, first);
        assert_eq!(requests.try_recv().unwrap().path, path);
        drop(active);
    }

    #[test]
    fn missing_source_duplicate_reloads_once_even_after_original_decode_finishes() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("source.png");
        std::fs::write(&path, b"original source").unwrap();
        let stamp = gallery::SourceStamp::read(&path);
        let (loader, requests) = loader_with_validator(source_validation::SourceValidator::new());
        let first = loader.submit(path.clone()).unwrap();
        let active = requests.try_recv().unwrap();
        loader.flight.lock().unwrap().as_mut().unwrap().0.source = Some(stamp);
        let token = GenerationToken::new(loader.generation.clone(), first);
        std::fs::remove_file(&path).unwrap();
        assert_eq!(loader.submit(path.clone()).unwrap(), first);
        finish_foreground_flight(&loader.flight, first);
        let deadline = Instant::now() + Duration::from_secs(5);
        let replacement = loop {
            if let Some(result) = loader.poll_duplicate() {
                break result.unwrap();
            }
            assert!(Instant::now() < deadline);
            thread::yield_now();
        };
        assert_ne!(replacement, first);
        assert!(token.is_cancelled());
        let request = requests.try_recv().unwrap();
        assert_eq!(request.path, path);
        assert_eq!(request.generation, replacement);
        assert!(loader.poll_duplicate().is_none());
        assert!(requests.try_recv().is_err());
        drop(active);
    }

    #[test]
    fn dropping_foreground_loader_invalidates_an_unsuperseded_active_token() {
        let (tx, worker_rx) = bounded(1);
        let loader = ForegroundLoader {
            worker: None,
            decode_first: true,
            automatic_policy: false,
            flight: Arc::new(std::sync::Mutex::new(None)),
            duplicate_validator: source_validation::SourceValidator::new(),
            duplicate: std::sync::Mutex::new(None),
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
            worker: None,
            decode_first: true,
            automatic_policy: false,
            flight: Arc::new(std::sync::Mutex::new(None)),
            duplicate_validator: source_validation::SourceValidator::new(),
            duplicate: std::sync::Mutex::new(None),
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
    fn known_prefetch_windows_bypass_scanning_and_keep_exact_priority() {
        let selected = Path::new("collection/050000.png");
        let window = gallery::PrefetchWindow { behind: 2, ahead: 5 };
        for known in [
            Vec::new(),
            vec![
                PathBuf::from("collection/050001.png"),
                PathBuf::from("collection/049999.png"),
            ],
        ] {
            let result =
                resolve_foreground_prefetch_paths(Some(known.clone()), selected, None, window, |_| {
                    panic!("known window rescanned directory")
                });
            assert_eq!(result, known);
        }
    }

    #[test]
    fn unknown_prefetch_window_scans_once_and_preserves_backward_order() {
        let scans = std::cell::Cell::new(0);
        let paths: Vec<_> = (0..100_000)
            .map(|index| PathBuf::from(format!("collection/{index:06}.png")))
            .collect();
        let selected = paths[50_000].clone();
        let previous = paths[50_001].clone();
        let expected =
            [49_999, 49_998, 49_997, 49_996, 49_995, 50_001, 50_002].map(|index| paths[index].clone());
        let result = resolve_foreground_prefetch_paths(
            None,
            &selected,
            Some(&previous),
            gallery::PrefetchWindow { behind: 2, ahead: 5 },
            |folder| {
                assert_eq!(folder, Path::new("collection"));
                scans.set(scans.get() + 1);
                Some(paths)
            },
        );
        assert_eq!(scans.get(), 1);
        assert_eq!(result, expected);
        assert!(
            resolve_foreground_prefetch_paths(
                None,
                &selected,
                None,
                gallery::PrefetchWindow::default(),
                |_| None
            )
            .is_empty()
        );
    }

    #[test]
    fn foreground_prefetch_respects_asymmetric_and_disabled_windows() {
        let paths: Vec<_> = (0..8)
            .map(|index| PathBuf::from(format!("collection/{index}.png")))
            .collect();
        for (selected, previous, window, expected) in [
            (
                3,
                2,
                gallery::PrefetchWindow { behind: 1, ahead: 3 },
                vec![4, 5, 6, 2],
            ),
            (6, 5, gallery::PrefetchWindow { behind: 1, ahead: 3 }, vec![7, 5]),
            (1, 2, gallery::PrefetchWindow { behind: 1, ahead: 3 }, vec![0, 2]),
            (3, 2, gallery::PrefetchWindow { behind: 0, ahead: 0 }, vec![]),
        ] {
            let result = resolve_foreground_prefetch_paths(
                None,
                &paths[selected],
                Some(&paths[previous]),
                window,
                |_| Some(paths.clone()),
            );
            assert_eq!(
                result,
                expected
                    .into_iter()
                    .map(|index| paths[index].clone())
                    .collect::<Vec<_>>()
            );
        }
    }

    #[test]
    fn rapid_submissions_keep_only_the_latest_pending_request() {
        let (tx, worker_rx) = bounded(1);
        let loader = ForegroundLoader {
            worker: None,
            decode_first: true,
            automatic_policy: false,
            flight: Arc::new(std::sync::Mutex::new(None)),
            duplicate_validator: source_validation::SourceValidator::new(),
            duplicate: std::sync::Mutex::new(None),
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
        assert!(ordinary.prefetch_paths.lock().unwrap().is_none());
        let planned = vec![PathBuf::from("next.png"), PathBuf::from("previous.png")];
        loader
            .submit_request_with_prefetch(
                PathBuf::from("selected.png"),
                0,
                None,
                true,
                Some(planned.clone()),
            )
            .unwrap();
        let original_generation = loader.current_generation();
        let reversed = vec![PathBuf::from("previous.png"), PathBuf::from("next.png")];
        assert_eq!(
            loader
                .submit_request_with_prefetch(
                    PathBuf::from("selected.png"),
                    0,
                    None,
                    true,
                    Some(reversed.clone())
                )
                .unwrap(),
            original_generation
        );
        assert_eq!(loader.current_generation(), original_generation);
        let selected = worker_rx.try_recv().unwrap();
        assert_eq!(*selected.prefetch_paths.lock().unwrap(), Some(reversed));
        assert_eq!(selected.generation, original_generation);
        assert!(worker_rx.try_recv().is_err());
        loader
            .submit_request_with_prefetch(PathBuf::from("last.png"), 0, None, true, Some(Vec::new()))
            .unwrap();
        let original_generation = loader.current_generation();
        loader
            .submit_request_with_prefetch(PathBuf::from("last.png"), 0, None, true, Some(planned))
            .unwrap();
        loader
            .submit_request_with_prefetch(PathBuf::from("last.png"), 0, None, true, Some(Vec::new()))
            .unwrap();
        let disabled = worker_rx.try_recv().unwrap();
        assert_eq!(*disabled.prefetch_paths.lock().unwrap(), Some(Vec::new()));
        // The worker may already own the request while the UI revises its plan.
        let new_plan = vec![PathBuf::from("closer.png")];
        assert_eq!(
            loader
                .submit_request_with_prefetch(
                    PathBuf::from("last.png"),
                    0,
                    None,
                    true,
                    Some(new_plan.clone())
                )
                .unwrap(),
            original_generation
        );
        assert_eq!(*disabled.prefetch_paths.lock().unwrap(), Some(new_plan));
        loader
            .submit_request_with_prefetch(PathBuf::from("last.png"), 0, None, true, Some(Vec::new()))
            .unwrap();
        assert_eq!(*disabled.prefetch_paths.lock().unwrap(), Some(Vec::new()));
        assert_eq!(disabled.generation, original_generation);
        assert!(worker_rx.try_recv().is_err());
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
    fn model_cancelled_resident_does_not_transfer_visible_pin() {
        let request = DecodeRequest::new(
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/models/triangle.stl"),
        );
        let mut cache = ModelDisplayCache::new(rrrah_cache::CacheLimits::bytes(1024 * 1024));
        cache
            .load(&request, false, || {
                rrrah_decode::decode_model(&request).map_err(ModelLoadError::Decode)
            })
            .unwrap();
        assert!(cache.visible.is_none());
        let mut polls = 0;
        assert!(
            cache
                .load_with_cancel(
                    &request,
                    true,
                    || panic!("resident decoded"),
                    || {
                        polls += 1;
                        polls >= 2
                    }
                )
                .is_err()
        );
        assert!(cache.visible.is_none());
        assert_eq!(cache.entries.len(), 1);
    }

    #[test]
    fn model_cancelled_swap_lookup_does_not_fall_through_to_decode() {
        let parent = tempfile::tempdir().unwrap();
        let root = rrrah_cache::MemoryBudget::new(1024 * 1024);
        let cli = Cli::try_parse_from(["rrrah", "--model-swap-mb", "1"]).unwrap();
        let mut cache = model_cache_with_swap(
            Some(parent.path()),
            false,
            rrrah_cache::CacheLimits::bytes(root.limit()),
            cli.model_swap_config(),
            Some(&root),
        );
        let request = DecodeRequest::new(
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/models/triangle.stl"),
        );
        let mut polls = 0;
        assert!(
            cache
                .load_with_cancel(
                    &request,
                    true,
                    || panic!("cancelled swap fell through to decode"),
                    || {
                        polls += 1;
                        polls >= 2
                    }
                )
                .is_err()
        );
        assert_eq!(polls, 3);
        assert!(cache.entries.is_empty());
        assert!(cache.visible.is_none());
        assert_eq!(root.used(), 0);
        drop(cache);
        assert_eq!(root.used(), 0);
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
    fn model_retained_prefetch_skips_protected_count_slots_and_retries() {
        for name in [
            "triangle.stl",
            "off-attributes-binary.off",
            "obj-shared-metadata.obj",
            "ply-64-le.ply",
        ] {
            for by_count in [true, false] {
                let fixtures = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/models");
                let weight = rrrah_decode::decode_model(&DecodeRequest::new(fixtures.join(name)))
                    .unwrap()
                    .capacity_bytes();
                let directory = tempfile::tempdir().unwrap();
                let root = rrrah_core::MemoryBudget::new(128 * 1024);
                let mut cache = ModelDisplayCache::new(rrrah_cache::CacheLimits {
                    max_bytes: if by_count { root.limit() } else { weight * 2 },
                    max_entries: by_count.then_some(2),
                    ttl: None,
                });
                let mut selected = DecodeRequest::new(fixtures.join(name));
                selected.memory_budget = Some(root.clone());
                drop(
                    cache
                        .load(&selected, true, || {
                            rrrah_decode::decode_model(&selected).map_err(ModelLoadError::Decode)
                        })
                        .unwrap(),
                );
                let mut nearest = selected.clone();
                nearest.path = directory.path().join(format!("nearest-{name}"));
                std::fs::copy(&selected.path, &nearest.path).unwrap();
                let gate = Arc::new(DecodeGate::new());
                let (model, lease) = preload_model_with_lease(&nearest, &mut cache, &gate).unwrap();
                let lease = lease.unwrap();
                drop(model);
                let visible = cache.visible.clone();
                let mut far = selected.clone();
                far.path = directory.path().join(format!("far-{name}"));
                std::fs::copy(&selected.path, &far.path).unwrap();
                let peak = root.peak();
                assert_eq!(
                    preload_model_with_lease(&far, &mut cache, &gate).unwrap_err(),
                    if by_count {
                        "model prefetch skipped: all count slots are protected"
                    } else {
                        "model prefetch skipped: protected byte budget exhausted"
                    }
                );
                assert_eq!(root.peak(), peak, "refused prefetch allocated geometry: {name}");
                let key = model_display_key(&far);
                assert!(
                    cache
                        .load_with_cancel_resolving_key_mode(
                            || key,
                            false,
                            true,
                            || panic!("protected slots must skip the decoder"),
                            || false
                        )
                        .is_err()
                );
                let (hit, hit_lease) = preload_model_with_lease(&nearest, &mut cache, &gate).unwrap();
                assert!(hit_lease.is_some(), "ready hit must bypass count refusal");
                assert_eq!(cache.visible, visible);
                drop((hit, hit_lease, lease));
                let (retry, retry_lease) = preload_model_with_lease(&far, &mut cache, &gate).unwrap();
                assert!(retry_lease.is_some());
                assert_eq!(cache.visible, visible);
                assert_eq!(cache.entries.len(), 2);
                drop((retry, retry_lease, cache));
                assert_eq!(root.used(), 0);
            }
        }
    }

    #[test]
    fn retained_model_prefetch_avoids_unretainable_swap_reads() {
        for name in [
            "triangle.stl",
            "off-attributes-binary.off",
            "obj-shared-metadata.obj",
            "ply-64-le.ply",
        ] {
            for by_count in [true, false] {
                let fixtures = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/models");
                let weight = rrrah_decode::decode_model(&DecodeRequest::new(fixtures.join(name)))
                    .unwrap()
                    .capacity_bytes();
                let directory = tempfile::tempdir().unwrap();
                let root = rrrah_core::MemoryBudget::new(128 * 1024);
                let cli = Cli::try_parse_from(["rrrah", "--model-swap-mb", "1"]).unwrap();
                let mut cache = model_cache_with_swap(
                    Some(directory.path()),
                    false,
                    rrrah_cache::CacheLimits {
                        max_bytes: if by_count { root.limit() } else { weight * 2 },
                        max_entries: by_count.then_some(2),
                        ttl: None,
                    },
                    cli.model_swap_config(),
                    Some(&root),
                );
                let mut selected = DecodeRequest::new(fixtures.join(name));
                selected.memory_budget = Some(root.clone());
                drop(
                    cache
                        .load(&selected, true, || {
                            rrrah_decode::decode_model(&selected).map_err(ModelLoadError::Decode)
                        })
                        .unwrap(),
                );
                let mut nearest = selected.clone();
                nearest.path = directory.path().join(format!("nearest-{name}"));
                std::fs::copy(&selected.path, &nearest.path).unwrap();
                let gate = Arc::new(DecodeGate::new());
                let (ready, lease) = preload_model_with_lease(&nearest, &mut cache, &gate).unwrap();
                drop(ready);
                let lease = lease.unwrap();
                let mut far = selected.clone();
                far.path = directory.path().join(format!("far-{name}"));
                std::fs::copy(&selected.path, &far.path).unwrap();
                let far_model = rrrah_decode::decode_model(&far).unwrap();
                let mut expected = Vec::new();
                rrrah_cache::SwapPayload::write_payload(
                    &model_swap::ModelPayload(far_model.clone()),
                    &mut expected,
                )
                .unwrap();
                cache.swap.as_ref().unwrap().enqueue(
                    model_display_key(&far).unwrap(),
                    model_swap::ModelPayload(far_model),
                );
                cache.swap.as_ref().unwrap().wait_idle().unwrap();
                let peak = root.peak();
                let visible = cache.visible.clone();
                assert_eq!(
                    preload_model_with_lease(&far, &mut cache, &gate).unwrap_err(),
                    if by_count {
                        "model prefetch skipped: all count slots are protected"
                    } else {
                        "model prefetch skipped: protected byte budget exhausted"
                    }
                );
                assert_eq!(cache.swap.as_ref().unwrap().stats().reads, 0);
                assert_eq!(root.peak(), peak);
                assert_eq!(cache.visible, visible);
                drop(lease);
                let (retry, lease) = preload_model_with_lease(&far, &mut cache, &gate).unwrap();
                assert!(lease.is_some());
                assert_eq!(cache.swap.as_ref().unwrap().stats().reads, 1);
                let mut actual = Vec::new();
                rrrah_cache::SwapPayload::write_payload(
                    &model_swap::ModelPayload(retry.clone()),
                    &mut actual,
                )
                .unwrap();
                assert_eq!(actual, expected);
                assert_eq!(cache.visible, visible);
                cache.swap.as_ref().unwrap().wait_idle().unwrap();
                cache.swap.as_ref().unwrap().shutdown();
                drop((retry, lease, cache));
                assert_eq!(root.used(), 0);
            }
        }
    }

    #[test]
    fn retained_raster_prefetch_avoids_unretainable_swap_reads() {
        for by_count in [true, false] {
            let source = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("../../tests/fixtures/raster/associated-emission-rec709.exr");
            let weight = load_raster_for_display_typed(&DecodeRequest::new(&source), None)
                .unwrap()
                .0
                .capacity_bytes();
            let directory = tempfile::tempdir().unwrap();
            let root = rrrah_core::MemoryBudget::new(1024 * 1024);
            let cli = Cli::try_parse_from(["rrrah", "--raster-swap-mb", "1"]).unwrap();
            let mut cache = raster_cache_with_swap(
                Some(directory.path()),
                false,
                rrrah_cache::CacheLimits {
                    max_bytes: if by_count { root.limit() } else { weight * 2 },
                    max_entries: by_count.then_some(2),
                    ttl: None,
                },
                cli.raster_swap_config(),
                Some(&root),
            );
            let mut selected = DecodeRequest::new(&source);
            selected.memory_budget = Some(root.clone());
            drop(load_cached_raster_for_display(&selected, None, &mut cache).unwrap());
            let mut nearest = selected.clone();
            nearest.path = directory.path().join("nearest.exr");
            std::fs::copy(&source, &nearest.path).unwrap();
            let gate = Arc::new(DecodeGate::new());
            let (frame, _, lease) = preload_raster_with_lease(&nearest, None, &mut cache, &gate).unwrap();
            drop(frame);
            let lease = lease.unwrap();
            let mut far = selected.clone();
            far.path = directory.path().join("far.exr");
            std::fs::copy(&source, &far.path).unwrap();
            let (frame, assumed) = load_raster_for_display_typed(&far, None).unwrap();
            let mut expected = Vec::new();
            rrrah_cache::write_raster_payload(&mut expected, &frame).unwrap();
            let mut key = raster_display_key(&far, None).unwrap();
            key.6 = assumed;
            cache.swap().unwrap().enqueue(key, frame);
            cache.swap().unwrap().wait_idle().unwrap();
            assert_eq!(
                preload_raster_with_lease(&far, None, &mut cache, &gate).unwrap_err(),
                if by_count {
                    "raster prefetch skipped: all count slots are protected"
                } else {
                    "raster prefetch skipped: protected byte budget exhausted"
                }
            );
            assert_eq!(cache.swap().unwrap().stats().reads, 0);
            let (hit, _, hit_lease) = preload_raster_with_lease(&nearest, None, &mut cache, &gate).unwrap();
            assert!(hit_lease.is_some());
            drop((hit, hit_lease, lease));
            let (retry, _, lease) = preload_raster_with_lease(&far, None, &mut cache, &gate).unwrap();
            assert!(lease.is_some());
            assert_eq!(cache.swap().unwrap().stats().reads, 1);
            let mut actual = Vec::new();
            rrrah_cache::write_raster_payload(&mut actual, &retry).unwrap();
            assert_eq!(actual, expected);
            let selected = load_cached_raster_with(&selected, None, &mut cache, || {
                panic!("selected raster must remain resident")
            })
            .unwrap();
            cache.swap().unwrap().wait_idle().unwrap();
            cache.swap().unwrap().shutdown();
            drop((selected, retry, lease, cache));
            assert_eq!(root.used(), 0);
        }
    }

    #[test]
    fn empty_stl_remains_admissible_with_its_header_budget() {
        let directory = tempfile::tempdir().unwrap();
        let mut empty = DecodeRequest::new(directory.path().join("empty.stl"));
        std::fs::write(&empty.path, b"solid\nendsolid\n").unwrap();
        let decoded = rrrah_decode::decode_model(&empty).unwrap();
        assert_eq!(decoded.bounds(), None);
        let header = decoded.capacity_bytes();
        assert!(header > 0, "empty geometry still owns a model header");
        drop(decoded);
        let mut selected = DecodeRequest::new(
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/models/triangle.stl"),
        );
        let selected_bytes = rrrah_decode::decode_model(&selected).unwrap().capacity_bytes();
        let root = rrrah_core::MemoryBudget::new(128 * 1024);
        selected.memory_budget = Some(root.clone());
        empty.memory_budget = Some(root.clone());
        let mut cache = ModelDisplayCache::new(rrrah_cache::CacheLimits::bytes(selected_bytes + header));
        drop(
            cache
                .load(&selected, true, || {
                    rrrah_decode::decode_model(&selected).map_err(ModelLoadError::Decode)
                })
                .unwrap(),
        );
        let gate = Arc::new(DecodeGate::new());
        let (model, lease) = preload_model_with_lease(&empty, &mut cache, &gate).unwrap();
        assert_eq!(model.bounds(), None);
        let lease = lease.unwrap();
        drop(model);
        let mut next = empty.clone();
        next.path = directory.path().join("next.stl");
        std::fs::write(&next.path, b"solid\nendsolid\n").unwrap();
        assert_eq!(
            preload_model_with_lease(&next, &mut cache, &gate).unwrap_err(),
            "model prefetch skipped: protected byte budget exhausted"
        );
        drop(lease);
        let (retry, lease) = preload_model_with_lease(&next, &mut cache, &gate).unwrap();
        assert_eq!(retry.bounds(), None);
        assert!(lease.is_some());
        assert_eq!(cache.entries.len(), 2);
        drop((retry, lease, cache));
        assert_eq!(root.used(), 0);
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
    fn profiled_jpeg_neighbour_windows_preserve_visible_and_respect_count_size_ttl() {
        let fixtures = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/raster");
        let paths: Vec<_> = (1..=5)
            .map(|i| {
                fixtures.join(format!(
                    "{}-profiled-orientation-{i}.jpg",
                    if i % 2 == 0 { "ycck" } else { "cmyk" }
                ))
            })
            .collect();
        let weight = 17 * 13 * 16;
        for limits in [
            rrrah_cache::CacheLimits::bytes(weight * 2),
            rrrah_cache::CacheLimits {
                max_bytes: weight * 10,
                max_entries: Some(2),
                ttl: None,
            },
        ] {
            let budget = rrrah_cache::MemoryBudget::new(1024 * 1024);
            let request_for = |path| {
                let mut request = DecodeRequest::new(path);
                request.memory_budget = Some(budget.clone());
                request
            };
            let mut cache = RasterDisplayCache::new(limits);
            let gate = Arc::new(DecodeGate::new());
            let visible = request_for(paths[2].clone());
            let (frame, _) = load_cached_raster_for_display(&visible, None, &mut cache).unwrap();
            assert_eq!(frame.capacity_bytes(), weight);
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
                    assert_eq!(cache.resident_bytes(), weight * 2);
                    let (hit, _) = load_cached_raster_with(&visible, None, &mut cache, || {
                        panic!("JPEG prefetch displaced visible image")
                    })
                    .unwrap();
                    let (rrrah_core::RasterPixels::Rgba32Float(a), rrrah_core::RasterPixels::Rgba32Float(b)) =
                        (frame.pixels(), hit.pixels())
                    else {
                        panic!()
                    };
                    assert!(a.ptr_eq(b));
                }
            }
            assert!(cache.set_limits_and_spill(rrrah_cache::CacheLimits {
                max_bytes: weight * 2,
                max_entries: Some(2),
                ttl: Some(Duration::ZERO),
            }));
            drop(preload_raster(&request_for(paths[4].clone()), None, &mut cache, &gate).unwrap());
            cache.spill_expired();
            assert_eq!(cache.len(), 1);
            assert_eq!(cache.resident_bytes(), weight);
            drop(
                load_cached_raster_with(&visible, None, &mut cache, || panic!("TTL expired pinned JPEG"))
                    .unwrap(),
            );
            drop(frame);
            drop(cache);
            assert_eq!(budget.used(), 0);
        }
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
    #[ignore = "manual 100000 valid file paths with mixed-format cache navigation"]
    fn hundred_thousand_valid_sources_navigate_with_bounded_prefetch_and_pressure() {
        qualify_hundred_thousand_sources(None);
    }
    #[test]
    #[ignore = "manual100k valid sources with actual Metal SDR/HDR and nearest prefetch protection"]
    fn hundred_thousand_valid_sources_preserve_metal_frames_and_priority() {
        let gpu = crate::navigation_metal_readback::qualification_gpu().expect("actual GPU required");
        eprintln!("100k source adapter: {}", gpu.adapter_name());
        qualify_hundred_thousand_sources(Some(&gpu));
        eprintln!(
            "100k source Metal: 512 steps; SDR/HDR exact; nearest protected; full pressure and stale cancellation; final=0"
        );
    }
    fn qualify_hundred_thousand_sources(gpu: Option<&crate::navigation_metal_readback::GpuReadback>) {
        let fixtures = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/raster");
        let originals: Vec<_> = [
            "pattern.profiled.png",
            "gif-animation-disposal-3.gif",
            "float-none-rec709-declared.exr",
            "rle-hdr-rec709-declared.hdr",
        ]
        .into_iter()
        .map(|name| fixtures.join(name))
        .collect();
        let directory = tempfile::tempdir().unwrap();
        for index in 0..100_000 {
            let original = &originals[index % 4];
            let extension = original.extension().unwrap().to_str().unwrap();
            std::fs::hard_link(original, directory.path().join(format!("{index:06}.{extension}"))).unwrap();
        }
        let start = Instant::now();
        let paths = gallery::scan_folder(directory.path());
        assert_eq!(paths.len(), 100_000);
        let budget = rrrah_cache::MemoryBudget::new(1024 * 1024);
        let mut oracle_cache = RasterDisplayCache::new(rrrah_cache::CacheLimits::bytes(64 * 1024));
        let mut expected = Vec::new();
        let mut expected_frames = Vec::new();
        let view = rrrah_gpu::ViewParameters {
            viewport: [128., 96.],
            ..Default::default()
        };
        for original in &originals {
            let mut request = DecodeRequest::new(original);
            request.memory_budget = Some(budget.clone());
            let (raster, _) = load_cached_raster_for_display(&request, None, &mut oracle_cache).unwrap();
            let rrrah_core::RasterPixels::Rgba32Float(pixels) = raster.pixels() else {
                panic!()
            };
            expected.push((
                raster.width(),
                raster.height(),
                pixels.iter().map(|v| v.to_bits()).collect::<Vec<_>>(),
            ));
            expected_frames.push(gpu.map(|gpu| {
                (
                    gpu.render_raster(&raster, view, [128, 96]).pixels,
                    gpu.render_raster_half(&raster, view, [128, 96]),
                )
            }));
        }
        drop(oracle_cache);
        assert_eq!(budget.used(), 0);
        let generation = Arc::new(AtomicU64::new(0));
        let gate = Arc::new(DecodeGate::new());
        let mut cache = RasterDisplayCache::new(rrrah_cache::CacheLimits {
            max_bytes: 64 * 1024,
            max_entries: Some(2),
            ttl: None,
        });
        let mut protected_count_refusals = 0;
        for step in 0..512usize {
            let selected = match step {
                0 => 0,
                1 => 99_999,
                _ => (step * 7919) % paths.len(),
            };
            let epoch = step as u64 * 2 + 1;
            generation.store(epoch, Ordering::Release);
            let request_for = |path: &Path| {
                let mut request = DecodeRequest::new(path);
                request.memory_budget = Some(budget.clone());
                request.cancellation = Some(GenerationToken::new(generation.clone(), epoch));
                request
            };
            let request = request_for(&paths[selected]);
            let (visible, _) = load_cached_raster_for_display(&request, None, &mut cache).unwrap();
            let rrrah_core::RasterPixels::Rgba32Float(pixels) = visible.pixels() else {
                panic!()
            };
            let reference = &expected[selected % 4];
            assert_eq!((visible.width(), visible.height()), (reference.0, reference.1));
            assert_eq!(
                pixels.iter().map(|v| v.to_bits()).collect::<Vec<_>>(),
                reference.2
            );
            if let Some(gpu) = gpu {
                let (sdr, hdr) = expected_frames[selected % 4].as_ref().unwrap();
                assert_eq!(gpu.render_raster(&visible, view, [128, 96]).pixels, *sdr);
                assert_eq!(gpu.render_raster_half(&visible, view, [128, 96]), *hdr);
            }
            let direction = if step % 2 == 0 {
                NavDirection::Forward
            } else {
                NavDirection::Backward
            };
            let neighbours = gallery::neighbour_prefetch_paths(
                &paths,
                selected,
                direction,
                gallery::PrefetchWindow { behind: 1, ahead: 2 },
            );
            assert!(neighbours.len() <= 3);
            let nearest = neighbours.first().cloned().unwrap();
            let mut protected = Vec::new();
            for (index, path) in neighbours.into_iter().enumerate() {
                let before = (budget.used(), cache.len(), cache.resident_bytes());
                let lease = match preload_raster_with_lease(&request_for(&path), None, &mut cache, &gate) {
                    Ok((_, _, lease)) => lease,
                    Err(error) => {
                        assert!(index > 0, "nearest neighbour must be retained: {error}");
                        assert_eq!(error, "raster prefetch skipped: all count slots are protected");
                        assert_eq!(cache.len(), 2);
                        assert!(!protected.is_empty());
                        assert_eq!((budget.used(), cache.len(), cache.resident_bytes()), before);
                        protected_count_refusals += 1;
                        None
                    }
                };
                if index == 0 {
                    assert!(lease.is_some());
                }
                if let Some(lease) = lease {
                    protected.push(lease);
                }
                assert!(cache.len() <= 2 && cache.resident_bytes() <= 64 * 1024);
                assert_eq!(
                    pixels.iter().map(|v| v.to_bits()).collect::<Vec<_>>(),
                    reference.2
                );
            }
            let (nearest_hit, _) =
                load_cached_raster_mode(&request_for(&nearest), None, &mut cache, false, || {
                    panic!("100k priority: farther neighbor displaced nearest")
                })
                .unwrap();
            let (rrrah_core::RasterPixels::Rgba32Float(a), rrrah_core::RasterPixels::Rgba32Float(b)) =
                (nearest_hit.pixels(), protected[0].pixels())
            else {
                panic!()
            };
            assert!(a.ptr_eq(b));
            drop(nearest_hit);
            let blocker = budget.try_reserve(budget.available_bytes()).unwrap();
            let (hit, _) = load_cached_raster_with(&request, None, &mut cache, || {
                panic!("visible frame decoded under pressure")
            })
            .unwrap();
            let rrrah_core::RasterPixels::Rgba32Float(hit_pixels) = hit.pixels() else {
                panic!()
            };
            assert!(pixels.ptr_eq(hit_pixels));
            drop(hit);
            drop(blocker);
            generation.store(epoch + 1, Ordering::Release);
            let before = (budget.used(), cache.len());
            assert!(preload_raster(&request, None, &mut cache, &gate).is_err());
            assert_eq!((budget.used(), cache.len()), before);
            drop(visible);
            drop(protected);
        }
        assert!(protected_count_refusals > 0);
        drop(cache);
        assert_eq!(budget.used(), 0);
        eprintln!(
            "100k valid-source navigation: seconds={} steps=512 protected_count_refusals={protected_count_refusals} cpu_peak={} final=0",
            start.elapsed().as_secs_f64(),
            budget.peak()
        );
    }

    #[test]
    fn mixed_gallery_reversals_bound_prefetch_and_release_managed_pixels() {
        qualify_mixed_gallery_reversals(None);
    }

    #[test]
    #[ignore = "requires actual GPU for mixed-format prefetch navigation readback"]
    fn mixed_gallery_reversals_preserve_metal_frames_and_prefetch_pressure() {
        let _ = env_logger::Builder::from_env(
            env_logger::Env::default().default_filter_or("wgpu_hal=warn,wgpu_core=warn"),
        )
        .is_test(true)
        .try_init();
        let gpu = crate::navigation_metal_readback::qualification_gpu().expect("actual GPU required");
        eprintln!("mixed prefetch navigation adapter: {}", gpu.adapter_name());
        qualify_mixed_gallery_reversals(Some(&gpu));
        eprintln!(
            "mixed prefetch navigation: 120 ping-pong steps; all visible Metal frames exact; stale requests refused; managed root final=0; eight files across seven formats; SDR/HDR exact; linear CLI policy"
        );
    }

    fn qualify_mixed_gallery_reversals(gpu: Option<&crate::navigation_metal_readback::GpuReadback>) {
        let fixtures = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/raster");
        let paths: Vec<_> = [
            "pattern.profiled.png",
            "gif-animation-disposal-3.gif",
            "float-none-rec709-declared.exr",
            "rle-hdr-rec709-declared.hdr",
            "transport-linear.pfm",
            "hdr-compressed.astc",
            "ravif-8bit-prem.avif",
            "ravif-10bit-prem.avif",
        ]
        .into_iter()
        .map(|name| fixtures.join(name))
        .collect();
        let interpretation = Cli::try_parse_from(["rrrah", "--untagged-color", "linear-srgb"])
            .unwrap()
            .raster_interpretation();
        let params = rrrah_gpu::ViewParameters {
            viewport: [128.0, 96.0],
            ..Default::default()
        };
        let expected_frames = gpu.map(|gpu| {
            paths
                .iter()
                .map(|path| {
                    let mut reference_cache =
                        RasterDisplayCache::new(rrrah_cache::CacheLimits::bytes(1024 * 1024));
                    let mut request = DecodeRequest::new(path);
                    interpretation.apply(&mut request);
                    let (reference, _) =
                        load_cached_raster_for_display(&request, None, &mut reference_cache).unwrap();
                    (
                        gpu.render_raster(&reference, params, [128, 96]).pixels,
                        gpu.render_raster_half(&reference, params, [128, 96]),
                    )
                })
                .collect::<Vec<_>>()
        });
        let budget = rrrah_cache::MemoryBudget::new(1024 * 1024);
        let generation = Arc::new(AtomicU64::new(1));
        let gate = Arc::new(DecodeGate::new());
        let mut cache = RasterDisplayCache::new(rrrah_cache::CacheLimits {
            max_bytes: 64 * 1024,
            max_entries: Some(2),
            ttl: None,
        });
        let request_for = |index: usize, epoch| {
            let mut request = DecodeRequest::new(paths[index].clone());
            interpretation.apply(&mut request);
            request.memory_budget = Some(budget.clone());
            request.cancellation = Some(GenerationToken::new(generation.clone(), epoch));
            request
        };
        // Repeated ping-pong across formats exercises both ends and reversals.
        for step in 0..120 {
            let period = (paths.len() - 1) * 2;
            let phase = step % period;
            let selected = if phase < paths.len() {
                phase
            } else {
                period - phase
            };
            let epoch = step as u64 + 1;
            generation.store(epoch, Ordering::Release);
            let request = request_for(selected, epoch);
            let (visible, _) = load_cached_raster_for_display(&request, None, &mut cache).unwrap();
            let rrrah_core::RasterPixels::Rgba32Float(pixels) = visible.pixels() else {
                panic!()
            };
            let expected: Vec<_> = pixels.iter().map(|v| v.to_bits()).collect();
            let direction = if phase < paths.len() {
                NavDirection::Forward
            } else {
                NavDirection::Backward
            };
            // Each window spans a full forward/backward period, including both
            // collection boundaries, rather than coupling a policy to one file.
            let window = [
                gallery::PrefetchWindow { behind: 1, ahead: 2 },
                gallery::PrefetchWindow { behind: 0, ahead: 3 },
                gallery::PrefetchWindow { behind: 3, ahead: 0 },
                gallery::PrefetchWindow { behind: 0, ahead: 0 },
            ][(step / period) % 4];
            let neighbours = gallery::neighbour_prefetch_paths(&paths, selected, direction, window);
            assert!(neighbours.len() <= window.behind + window.ahead);
            for path in neighbours {
                let index = paths.iter().position(|p| *p == path).unwrap();
                let neighbour = request_for(index, epoch);
                drop(preload_raster(&neighbour, None, &mut cache, &gate).unwrap());
                assert!(cache.len() <= 2);
                assert!(cache.resident_bytes() <= 64 * 1024);
                assert_eq!(pixels.iter().map(|v| v.to_bits()).collect::<Vec<_>>(), expected);
            }
            // Ready foreground pixels require no allocation under full pressure.
            let blocker = budget.try_reserve(budget.available_bytes()).unwrap();
            let (hit, _) = load_cached_raster_with(&request, None, &mut cache, || {
                panic!("visible frame decoded under pressure")
            })
            .unwrap();
            let rrrah_core::RasterPixels::Rgba32Float(hit_pixels) = hit.pixels() else {
                panic!()
            };
            assert!(pixels.ptr_eq(hit_pixels));
            drop(hit);
            drop(blocker);
            generation.store(epoch + 1, Ordering::Release);
            let used = budget.used();
            let count = cache.len();
            assert!(preload_raster(&request, None, &mut cache, &gate).is_err());
            assert_eq!(budget.used(), used);
            assert_eq!(cache.len(), count);
            if let Some(gpu) = gpu {
                let actual = gpu.render_raster(&visible, params, [128, 96]).pixels;
                let reference = &expected_frames.as_ref().unwrap()[selected].0;
                assert!(
                    actual == *reference,
                    "SDR step={step} selected={selected} fixture={} first_diff={:?} actual_prefix={:?} expected_prefix={:?} zero_bytes={} managed_bytes={} cache_entries={}",
                    paths[selected].display(),
                    actual.iter().zip(reference).position(|(a, b)| a != b),
                    &actual[..actual.len().min(16)],
                    &reference[..reference.len().min(16)],
                    actual.iter().filter(|&&v| v == 0).count(),
                    budget.used(),
                    cache.len(),
                );
                assert_eq!(
                    gpu.render_raster_half(&visible, params, [128, 96]),
                    expected_frames.as_ref().unwrap()[selected].1,
                    "HDR step={step} selected={selected}"
                );
            }
            drop(visible);
        }
        drop(cache);
        assert_eq!(budget.used(), 0);
        assert!(budget.peak() <= budget.limit());
    }
    #[test]
    fn rpf_cli_preload_and_cache_keys_keep_explicit_interpretations_separate() {
        let cli = Cli::try_parse_from([
            "rrrah",
            "--rpf-rgb",
            "0",
            "1",
            "2",
            "--rpf-matte",
            "0",
            "--rpf-alpha",
            "straight",
            "--rpf-color",
            "linear-srgb",
        ])
        .unwrap();
        assert!(Cli::try_parse_from(["rrrah", "--rpf-rgb", "0", "1", "2"]).is_err());
        let parent = tempfile::tempdir().unwrap();
        let path = parent.path().join("synthetic.rpf");
        let mut bytes = vec![0u8; 744];
        for (at, value) in [
            (2, 1u16),
            (10, 1),
            (20, 3),
            (22, 1),
            (26, 0xfffd),
            (658, 32),
            (662, 32),
        ] {
            bytes[at..at + 2].copy_from_slice(&value.to_be_bytes());
        }
        bytes[400..413].copy_from_slice(b"3ds max : ( )");
        bytes[572..580].copy_from_slice(b"3.00000\0");
        bytes[740..744].copy_from_slice(&744u32.to_be_bytes());
        for channel in [[0.125f32, 1.0], [0.25, 2.0], [0.5, 4.0], [0.25, 0.5]] {
            bytes.extend(8u16.to_be_bytes());
            for value in channel {
                bytes.extend(value.to_bits().to_be_bytes());
            }
        }
        std::fs::write(&path, bytes).unwrap();
        let renamed = parent.path().join("synthetic.CR3");
        std::fs::copy(&path, &renamed).unwrap();
        assert!(foreground_neighbour_supported(&renamed));
        let budget = rrrah_core::MemoryBudget::new(1024 * 1024);
        let mut request = DecodeRequest::new(&path);
        request.memory_budget = Some(budget.clone());
        cli.raster_interpretation().apply(&mut request);
        let mut cache = RasterDisplayCache::new(rrrah_cache::CacheLimits::bytes(1024 * 1024));
        let gate = Arc::new(DecodeGate::new());
        let (warmed, _) = preload_raster(&request, None, &mut cache, &gate).unwrap();
        let mut renamed_request = request.clone();
        renamed_request.path = renamed;
        let (renamed_frame, _) = preload_raster(&renamed_request, None, &mut cache, &gate).unwrap();
        assert_eq!(renamed_frame.pixel_aspect(), Some(1.5));
        let rrrah_core::RasterPixels::Rgba32Float(renamed_pixels) = renamed_frame.pixels() else {
            panic!()
        };
        assert_eq!(renamed_pixels[0], 0.125);
        // Use a separate cache for the renamed-source route so policy-count
        // checks below still count only variants of the original source.
        drop(renamed_frame);
        drop(warmed);
        drop(cache);
        let mut cache = RasterDisplayCache::new(rrrah_cache::CacheLimits::bytes(1024 * 1024));
        let (warmed, _) = preload_raster(&request, None, &mut cache, &gate).unwrap();
        assert_eq!(warmed.pixel_aspect(), Some(1.5));
        let (hit, _) = load_cached_raster_with(&request, None, &mut cache, || {
            panic!("configured RPF preload decoded again")
        })
        .unwrap();
        let rrrah_core::RasterPixels::Rgba32Float(a) = warmed.pixels() else {
            panic!()
        };
        let rrrah_core::RasterPixels::Rgba32Float(b) = hit.pixels() else {
            panic!()
        };
        assert!(a.ptr_eq(b));
        let mut changed = request.clone();
        changed.rpf_interpretation.as_mut().unwrap().alpha_mode = rrrah_decode::RlaAlphaMode::Premultiplied;
        let (other, _) = load_cached_raster_for_display(&changed, None, &mut cache).unwrap();
        let rrrah_core::RasterPixels::Rgba32Float(values) = other.pixels() else {
            panic!()
        };
        assert_eq!(a[0], 0.125);
        assert_eq!(values[0], 0.5);
        assert_eq!(cache.len(), 2);
        let mut swapped = request.clone();
        swapped.rpf_interpretation.as_mut().unwrap().rgb = [2, 1, 0];
        let (swapped, _) = load_cached_raster_for_display(&swapped, None, &mut cache).unwrap();
        let rrrah_core::RasterPixels::Rgba32Float(values) = swapped.pixels() else {
            panic!()
        };
        assert_eq!(values[0], 0.5);
        assert_eq!(values[2], 0.125);
        assert_eq!(cache.len(), 3);
        let mut encoded = request.clone();
        encoded.rpf_interpretation.as_mut().unwrap().color_space = rrrah_core::RasterColorSpace::Srgb;
        let (encoded, _) = load_cached_raster_for_display(&encoded, None, &mut cache).unwrap();
        let rrrah_core::RasterPixels::Rgba32Float(values) = encoded.pixels() else {
            panic!()
        };
        assert!(values[0] < a[0]);
        assert_eq!(cache.len(), 4);
        let mut opaque = request.clone();
        opaque.rpf_interpretation.as_mut().unwrap().matte = None;
        let (opaque, _) = load_cached_raster_for_display(&opaque, None, &mut cache).unwrap();
        let rrrah_core::RasterPixels::Rgba32Float(values) = opaque.pixels() else {
            panic!()
        };
        assert_eq!(values[3], 1.0);
        assert_eq!(cache.len(), 5);
        Cli::try_parse_from(["rrrah"])
            .unwrap()
            .raster_interpretation()
            .apply(&mut request);
        assert!(request.rpf_interpretation.is_none());
        assert!(load_cached_raster_for_display(&request, None, &mut cache).is_err());
        assert_eq!(cache.len(), 5);
        drop(warmed);
        drop(hit);
        drop(other);
        drop(swapped);
        drop(encoded);
        drop(opaque);
        drop(cache);
        assert_eq!(budget.used(), 0);
        let mut cache = raster_cache_with_swap(
            Some(parent.path()),
            false,
            rrrah_cache::CacheLimits {
                max_bytes: 1024 * 1024,
                max_entries: Some(1),
                ttl: None,
            },
            Some(rrrah_cache::ImageSwapConfig {
                limits: rrrah_cache::CacheLimits::bytes(4096),
                queue_bytes: 1024,
                queue_count: 2,
                restore_bytes: 1024,
            }),
            Some(&budget),
        );
        cli.raster_interpretation().apply(&mut request);
        let (first, _) = load_cached_raster_for_display(&request, None, &mut cache).unwrap();
        let mut changed = request.clone();
        changed.rpf_interpretation.as_mut().unwrap().alpha_mode = rrrah_decode::RlaAlphaMode::Premultiplied;
        drop(load_cached_raster_for_display(&changed, None, &mut cache).unwrap());
        cache.swap().unwrap().wait_idle().unwrap();
        assert_eq!(cache.swap().unwrap().stats().writes, 1);
        let (restored, _) = load_cached_raster_with(&request, None, &mut cache, || {
            panic!("configured RPF swap entry decoded again")
        })
        .unwrap();
        assert_eq!(restored.pixel_aspect(), Some(1.5));
        let (rrrah_core::RasterPixels::Rgba32Float(a), rrrah_core::RasterPixels::Rgba32Float(b)) =
            (first.pixels(), restored.pixels())
        else {
            panic!()
        };
        assert_eq!(
            a.iter().map(|v| v.to_bits()).collect::<Vec<_>>(),
            b.iter().map(|v| v.to_bits()).collect::<Vec<_>>()
        );
        cache.swap().unwrap().wait_idle().unwrap();
        assert_eq!(cache.len(), 1);
        assert_eq!(cache.swap().unwrap().stats().reads, 1);
        drop(first);
        drop(restored);
        drop(cache);
        assert_eq!(budget.used(), 0);
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
        assert!(Cli::try_parse_from(["rrrah", "--untagged-color", "linear-srgb"]).is_ok());
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
    fn queued_navigation_cancels_profiled_jpeg_preload_and_decodes_latest_raster() {
        let fixtures = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures");
        let (tx, worker_rx) = bounded(1);
        let gate = Arc::new(DecodeGate::new());
        let loader = ForegroundLoader {
            worker: None,
            decode_first: false,
            automatic_policy: false,
            flight: Arc::new(std::sync::Mutex::new(None)),
            duplicate_validator: source_validation::SourceValidator::new(),
            duplicate: std::sync::Mutex::new(None),
            tx,
            pending: worker_rx.clone(),
            generation: Arc::new(AtomicU64::new(0)),
            decode_gate: gate.clone(),
            telemetry: Arc::new(CacheTelemetry::new(true, 4096)),
            development: None,
        };
        let first = loader
            .submit(fixtures.join("raster/cmyk-profiled-orientation-1.jpg"))
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
            .submit(fixtures.join("raster/cmyk-profiled-orientation-1.jpg"))
            .unwrap();
        loader
            .submit(fixtures.join("raster/ycck-profiled-orientation-2.jpg"))
            .unwrap();
        let latest = loader
            .submit(fixtures.join("raster/ycck-profiled-orientation-6.jpg"))
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
        assert_eq!((ready.width(), ready.height()), (13, 17));
        assert_eq!(ready.color_space(), &rrrah_core::RasterColorSpace::LinearSrgb);
        drop(ready);
        drop(cache);
        assert_eq!(budget.used(), 0);
    }
    #[test]
    fn queued_navigation_cancels_pict_preload_and_decodes_latest_raster() {
        let fixtures = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures");
        let (tx, worker_rx) = bounded(1);
        let gate = Arc::new(DecodeGate::new());
        let loader = ForegroundLoader {
            worker: None,
            decode_first: false,
            automatic_policy: false,
            flight: Arc::new(std::sync::Mutex::new(None)),
            duplicate_validator: source_validation::SourceValidator::new(),
            duplicate: std::sync::Mutex::new(None),
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
            worker: None,
            decode_first: false,
            automatic_policy: false,
            flight: Arc::new(std::sync::Mutex::new(None)),
            duplicate_validator: source_validation::SourceValidator::new(),
            duplicate: std::sync::Mutex::new(None),
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
    #[ignore = "requires pinned AI corpus and actual Metal for queued page navigation"]
    fn queued_ai_page_reversal_cancels_stale_preload_and_preserves_metal() {
        assert_eq!(std::env::var("RRRAH_GPU_BACKEND").as_deref(), Ok("metal"));
        let corpus = PathBuf::from(std::env::var_os("RRRAH_AI_CORPUS").expect("AI corpus"));
        for (name, expected) in [
            (
                "VectorApple.ai",
                "c5fc34346629c3379cee6065c45b7de0b620be822604551d1463d8e691b03d06",
            ),
            (
                "one.ai",
                "97786949c268233a446e1738fb9c2f941e4d109811bd2f972948cdb7a45d64b4",
            ),
        ] {
            let hash = std::process::Command::new("shasum")
                .args(["-a", "256"])
                .arg(corpus.join(name))
                .output()
                .unwrap();
            assert!(hash.status.success());
            assert_eq!(
                String::from_utf8(hash.stdout).unwrap().split_whitespace().next(),
                Some(expected)
            );
        }
        let gpu = navigation_metal_readback::qualification_gpu().expect("actual Metal required");
        eprintln!("AI queued navigation adapter: {}", gpu.adapter_name());
        let (tx, worker_rx) = bounded(1);
        let gate = Arc::new(DecodeGate::new());
        let loader = ForegroundLoader {
            worker: None,
            decode_first: false,
            automatic_policy: false,
            flight: Arc::new(std::sync::Mutex::new(None)),
            duplicate_validator: source_validation::SourceValidator::new(),
            duplicate: std::sync::Mutex::new(None),
            tx,
            pending: worker_rx.clone(),
            generation: Arc::new(AtomicU64::new(0)),
            decode_gate: gate.clone(),
            telemetry: Arc::new(CacheTelemetry::new(true, 4096)),
            development: None,
        };
        let first = loader.submit(corpus.join("VectorApple.ai")).unwrap();
        let active = worker_rx.try_recv().unwrap();
        let token = GenerationToken::new(loader.generation.clone(), first);
        let held = active.foreground.acquire_decode(|| token.is_cancelled()).unwrap();
        let budget = rrrah_cache::MemoryBudget::new(64 * 1024 * 1024);
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
        loader.submit_image(corpus.join("one.ai"), 1).unwrap();
        loader.submit_image(corpus.join("one.ai"), 0).unwrap();
        let latest = loader.submit_image(corpus.join("one.ai"), 1).unwrap();
        assert!(token.is_cancelled());
        drop(held);
        worker.join().unwrap();
        assert_eq!(budget.used(), 0);
        let selected = worker_rx.try_recv().unwrap();
        assert_eq!(selected.generation, latest);
        assert_eq!(selected.path, corpus.join("one.ai"));
        assert_eq!(selected.image_index, 1);
        assert!(worker_rx.try_recv().is_err());
        // Real source decoding and display preparation remain possible after cancellation.
        drop(selected.foreground);
        let mut request = DecodeRequest::new(selected.path);
        request.image_index = selected.image_index;
        request.memory_budget = Some(budget.clone());
        request.cancellation = Some(GenerationToken::new(loader.generation.clone(), latest));
        let mut cache = RasterDisplayCache::new(rrrah_cache::CacheLimits::bytes(budget.limit()));
        let (ready, _) = preload_raster(&request, None, &mut cache, &gate).unwrap();
        assert_eq!(
            (
                ready.width(),
                ready.height(),
                ready.image_index(),
                ready.image_count()
            ),
            (177, 175, 1, 2)
        );
        let view = rrrah_gpu::ViewParameters {
            viewport: [128., 96.],
            ..Default::default()
        };
        let sdr = gpu.render_raster(&ready, view, [128, 96]).pixels;
        let hdr = gpu.render_raster_half(&ready, view, [128, 96]);
        let mut neighbour = request.clone();
        neighbour.image_index = 0;
        let (other, _) = preload_raster(&neighbour, None, &mut cache, &gate).unwrap();
        assert_eq!((other.width(), other.height()), (1366, 768));
        assert_eq!(cache.len(), 2);
        let pressure = budget.try_reserve(budget.available_bytes()).unwrap();
        let (hit, _) = load_cached_raster_with(&request, None, &mut cache, || {
            panic!("latest AI page decoded under pressure")
        })
        .unwrap();
        let (rrrah_core::RasterPixels::Rgba32Float(a), rrrah_core::RasterPixels::Rgba32Float(b)) =
            (ready.pixels(), hit.pixels())
        else {
            panic!()
        };
        assert!(a.ptr_eq(b));
        assert_eq!(gpu.render_raster(&hit, view, [128, 96]).pixels, sdr);
        assert_eq!(gpu.render_raster_half(&hit, view, [128, 96]), hdr);
        drop((pressure, hit, other, ready, cache));
        assert_eq!(budget.used(), 0);
        eprintln!(
            "AI queued navigation: page1->page0->page1; stale preload refused; pending_latest_only; independent_keys=2; full_pressure_shared_hit; Metal_SDR_HDR_exact; root=0; appearance_unqualified"
        );
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
    fn dpx_cineon_prefetch_keeps_srgb_and_linear_cache_entries_distinct() {
        let fixtures = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/raster");
        for name in ["dpx-16-be-50-p0.dpx", "cineon-16-be-3-p3.cin"] {
            let budget = rrrah_cache::MemoryBudget::new(1024 * 1024);
            let mut srgb = DecodeRequest::new(fixtures.join(name));
            srgb.memory_budget = Some(budget.clone());
            Cli::try_parse_from(["rrrah", "--untagged-color", "srgb"])
                .unwrap()
                .raster_interpretation()
                .apply(&mut srgb);
            let mut linear = srgb.clone();
            Cli::try_parse_from(["rrrah", "--untagged-color", "linear-srgb"])
                .unwrap()
                .raster_interpretation()
                .apply(&mut linear);
            let gate = Arc::new(DecodeGate::new());
            let mut cache = RasterDisplayCache::new(rrrah_cache::CacheLimits::bytes(budget.limit()));
            let (a, _) = preload_raster(&srgb, None, &mut cache, &gate).unwrap();
            let (b, _) = preload_raster(&linear, None, &mut cache, &gate).unwrap();
            assert_eq!(cache.len(), 2);
            let (rrrah_core::RasterPixels::Rgba32Float(pa), rrrah_core::RasterPixels::Rgba32Float(pb)) =
                (a.pixels(), b.pixels())
            else {
                panic!()
            };
            assert!(!pa.ptr_eq(pb));
            assert!(
                pa.iter()
                    .zip(pb.iter())
                    .enumerate()
                    .any(|(index, (a, b))| index % 4 != 3 && a < b),
                "policies must produce different prepared colors"
            );
            assert!(
                pa.chunks_exact(4)
                    .zip(pb.chunks_exact(4))
                    .all(|(a, b)| a[3] == b[3])
            );
            let mut strict = srgb.clone();
            Cli::try_parse_from(["rrrah"])
                .unwrap()
                .raster_interpretation()
                .apply(&mut strict);
            assert!(preload_raster(&strict, None, &mut cache, &gate).is_err());
            assert_eq!(cache.len(), 2);
            let pressure = budget.try_reserve(budget.available_bytes()).unwrap();
            for (request, expected) in [(&srgb, pa), (&linear, pb)] {
                let (hit, _) = load_cached_raster_with(request, None, &mut cache, || {
                    panic!("prefetched policy decoded again under pressure")
                })
                .unwrap();
                let rrrah_core::RasterPixels::Rgba32Float(values) = hit.pixels() else {
                    panic!()
                };
                assert!(values.ptr_eq(expected));
            }
            drop(pressure);
            drop(a);
            drop(b);
            drop(cache);
            assert_eq!(budget.used(), 0);
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
        for (name, linear) in [
            ("pattern.tif", false),
            ("pattern.pfm", true),
            ("pkm-eac-5-4x4.pkm", true),
            ("pkm-eac-6-4x4.pkm", true),
            ("pkm-eac-7-4x4.pkm", true),
            ("pkm-eac-8-4x4.pkm", true),
            ("ktx-eac-9270-le-00.ktx", true),
            ("ktx-eac-9271-le-00.ktx", true),
            ("ktx-eac-9272-be-11.ktx", true),
            ("ktx-eac-9273-be-11.ktx", true),
        ] {
            let budget = rrrah_cache::MemoryBudget::new(1024 * 1024);
            let mut request = DecodeRequest::new(
                PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                    .join("../../tests/fixtures/raster")
                    .join(name),
            );
            if name == "pattern.pfm" {
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
    fn linear_texture_policy_isolated_across_ram_expiry_and_swap_restore() {
        let fixtures = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/raster");
        let pkm = (5..=8).map(|kind| (format!("pkm-eac-{kind}-4x4.pkm"), 0usize));
        let ktx = include_str!("../../../tests/fixtures/raster/ktx-eac-manifest.tsv")
            .lines()
            .chain(include_str!("../../../tests/fixtures/raster/ktx-rg-manifest.tsv").lines())
            .filter(|line| !line.starts_with('#') && !line.is_empty())
            .map(|line| {
                let mut fields = line.split('\t');
                (
                    fields.next().unwrap().to_owned(),
                    fields.next().unwrap().parse::<usize>().unwrap(),
                )
            });
        let mut cases = 0;
        for (name, index) in pkm.chain(ktx) {
            let parent = tempfile::tempdir().unwrap();
            let root = rrrah_cache::MemoryBudget::new(1024 * 1024);
            let swap = rrrah_cache::RasterSwapCache::new_with_budgets(
                parent.path(),
                rrrah_cache::ImageSwapConfig {
                    limits: rrrah_cache::CacheLimits {
                        max_bytes: 65536,
                        max_entries: Some(16),
                        ttl: None,
                    },
                    queue_bytes: 4096,
                    queue_count: 4,
                    restore_bytes: 1024 * 1024,
                },
                rrrah_cache::MemoryBudget::new(4096),
                root.child(1024 * 1024),
            )
            .unwrap();
            let mut cache = RasterDisplayCache::new(rrrah_cache::CacheLimits {
                max_bytes: 1024 * 1024,
                max_entries: Some(1),
                ttl: Some(Duration::ZERO),
            });
            cache.enable_swap(swap);
            let mut request = DecodeRequest::new(fixtures.join(name));
            request.image_index = index;
            request.memory_budget = Some(root.clone());
            request.assume_untagged_linear_srgb = true;
            let (first, assumed) = load_cached_raster_for_display(&request, None, &mut cache).unwrap();
            assert!(!assumed);
            let rrrah_core::RasterPixels::Rgba32Float(values) = first.pixels() else {
                panic!()
            };
            let expected: Vec<_> = values.iter().map(|v| v.to_bits()).collect();
            let mut strict = request.clone();
            strict.assume_untagged_linear_srgb = false;
            cache.mark_visible(raster_display_key(&strict, None).unwrap());
            assert_eq!(cache.spill_expired(), 1);
            cache.swap().unwrap().wait_idle().unwrap();
            drop(first);
            assert_eq!(root.used(), 0);
            let disk = cache.swap().unwrap().disk_usage().unwrap();
            for encoded in [false, true] {
                strict.assume_untagged_srgb = encoded;
                let mut calls = 0;
                assert!(
                    load_cached_raster_with(&strict, None, &mut cache, || {
                        calls += 1;
                        load_raster_for_display_typed(&strict, None)
                    })
                    .is_err()
                );
                assert_eq!(calls, 1);
                let gate = Arc::new(DecodeGate::new());
                assert!(preload_raster(&strict, None, &mut cache, &gate).is_err());
                assert_eq!(root.used(), 0);
                assert_eq!(cache.swap().unwrap().stats().reads, 0);
                assert_eq!(cache.swap().unwrap().disk_usage().unwrap(), disk);
            }
            let (restored, assumed) = load_cached_raster_with(&request, None, &mut cache, || {
                panic!("qualified swap entry must survive strict requests")
            })
            .unwrap();
            assert!(!assumed);
            let rrrah_core::RasterPixels::Rgba32Float(values) = restored.pixels() else {
                panic!()
            };
            assert_eq!(values.iter().map(|v| v.to_bits()).collect::<Vec<_>>(), expected);
            assert_eq!(cache.swap().unwrap().stats().reads, 1);
            drop(restored);
            cache.swap().unwrap().shutdown();
            drop(cache);
            assert_eq!(root.used(), 0);
            assert_eq!(std::fs::read_dir(parent.path()).unwrap().count(), 0);
            cases += 1;
        }
        assert_eq!(cases, 324);
        eprintln!(
            "LINEAR_TEXTURE_COLOR_POLICY cases={cases} refusals={} swap_restores={cases} final=0",
            cases * 4
        );
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
            None,
            (None, None, None, None, false),
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
            None,
            (None, None, None, None, false),
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
            None,
            (None, None, None, None, false),
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
            None,
            (None, None, None, None, false),
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
    fn cancelled_loads_skip_lazy_key_resolution_and_decode() {
        let mut models = ModelDisplayCache::new(rrrah_cache::CacheLimits::bytes(4096));
        assert!(
            models
                .load_with_cancel_resolving_key(
                    || panic!("cancelled model fingerprint"),
                    false,
                    || panic!("cancelled model decode"),
                    || true
                )
                .is_err()
        );
        let mut rasters = RasterDisplayCache::new(rrrah_cache::CacheLimits::bytes(4096));
        let mut request = DecodeRequest::new("missing.png");
        request.cancellation = Some(GenerationToken::new(Arc::new(AtomicU64::new(2)), 1));
        assert!(
            load_cached_raster_mode_resolving_key(
                &request,
                || panic!("cancelled raster fingerprint"),
                &mut rasters,
                false,
                true,
                || panic!("cancelled raster decode")
            )
            .is_err()
        );
        assert!(models.entries.is_empty());
        assert!(rasters.is_empty());
    }
    #[test]
    fn wal_prefetch_replaces_palette_identity_while_old_lease_stays_immutable() {
        let directory = tempfile::tempdir().unwrap();
        std::fs::create_dir(directory.path().join("textures")).unwrap();
        std::fs::create_dir(directory.path().join("pics")).unwrap();
        let path = directory.path().join("textures/grid.wal");
        let palette = directory.path().join("pics/colormap.pcx");
        std::fs::write(
            &path,
            include_bytes!("../../../tests/fixtures/raster/palette-grid.wal"),
        )
        .unwrap();
        let mut pcx = include_bytes!("../../../tests/fixtures/raster/wal-colormap.pcx").to_vec();
        std::fs::write(&palette, &pcx).unwrap();
        let time = std::fs::metadata(&palette).unwrap().modified().unwrap();
        let root = rrrah_cache::MemoryBudget::new(1024 * 1024);
        let mut request = DecodeRequest::new(&path);
        request.memory_budget = Some(root.clone());
        let mut cache = RasterDisplayCache::new(rrrah_cache::CacheLimits {
            max_bytes: 1024 * 1024,
            max_entries: Some(4),
            ttl: None,
        });
        let gate = Arc::new(DecodeGate::new());
        let (first, _, old_lease) = preload_raster_with_lease(&request, None, &mut cache, &gate).unwrap();
        let old_lease = old_lease.expect("old neighbour retained");
        let offset = pcx.len() - 768;
        pcx[offset..offset + 3].copy_from_slice(&[0, 255, 0]);
        std::fs::write(&palette, pcx).unwrap();
        std::fs::File::options()
            .write(true)
            .open(&palette)
            .unwrap()
            .set_times(std::fs::FileTimes::new().set_modified(time))
            .unwrap();
        let (second, _, new_lease) = preload_raster_with_lease(&request, None, &mut cache, &gate).unwrap();
        let new_lease = new_lease.expect("new neighbour retained");
        let rrrah_core::RasterPixels::Rgba32Float(old) = first.pixels() else {
            panic!()
        };
        let rrrah_core::RasterPixels::Rgba32Float(new) = second.pixels() else {
            panic!()
        };
        assert_eq!(&old[..4], &[0., 0., 0., 1.]);
        assert_eq!(&new[..4], &[0., 1., 0., 1.]);
        let generation = Arc::new(AtomicU64::new(1));
        request.cancellation = Some(GenerationToken::new(generation.clone(), 1));
        let (stop, stopped) = std::sync::mpsc::channel();
        let watchdog = thread::spawn(move || {
            if stopped.recv_timeout(Duration::from_secs(5)).is_err() {
                generation.store(2, Ordering::Release);
            }
        });
        let ticket = gate.request_foreground();
        // A current prefetch hit must bypass a blocked decode gate.
        let (hit, _, lease) = preload_raster_with_lease(&request, None, &mut cache, &gate).unwrap();
        assert!(lease.is_some());
        let rrrah_core::RasterPixels::Rgba32Float(values) = hit.pixels() else {
            panic!()
        };
        assert_eq!(&values[..4], &[0., 1., 0., 1.]);
        let _ = stop.send(());
        watchdog.join().unwrap();
        drop(ticket);
        drop(hit);
        drop(lease);
        drop(first);
        drop(second);
        drop(cache);
        assert!(root.used() > 0, "neighbour leases retain pixel credits");
        drop(old_lease);
        drop(new_lease);
        assert_eq!(root.used(), 0);
    }

    #[test]
    fn recognized_raster_with_wal_suffix_has_no_palette_cache_dependency() {
        let directory = tempfile::tempdir().unwrap();
        std::fs::create_dir(directory.path().join("textures")).unwrap();
        std::fs::create_dir(directory.path().join("pics")).unwrap();
        let path = directory.path().join("textures/fake.wal");
        let palette = directory.path().join("pics/colormap.pcx");
        std::fs::write(
            &path,
            include_bytes!("../../../tests/fixtures/raster/pattern.png"),
        )
        .unwrap();
        let request = DecodeRequest::new(&path);
        let original = raster_display_key(&request, None).unwrap();
        assert_eq!(
            original.3,
            SourceFingerprint::from_path(&path).unwrap().sampled_blake3
        );
        std::fs::write(&palette, b"unrelated palette").unwrap();
        assert_eq!(raster_display_key(&request, None).unwrap(), original);
        std::fs::remove_file(&palette).unwrap();
        assert_eq!(raster_display_key(&request, None).unwrap(), original);
        let root = rrrah_cache::MemoryBudget::new(1024 * 1024);
        let mut cache = RasterDisplayCache::new(rrrah_cache::CacheLimits::bytes(1024 * 1024));
        let mut request = request;
        request.memory_budget = Some(root.clone());
        load_cached_raster_for_display(&request, None, &mut cache).unwrap();
        load_cached_raster_with(&request, None, &mut cache, || {
            panic!("recognized PNG must hit RAM")
        })
        .unwrap();
        drop(cache);
        assert_eq!(root.used(), 0);
    }

    #[test]
    fn wal_palette_change_invalidates_ram_and_swap_without_changing_wal() {
        qualify_wal_palette_change(None);
    }

    #[test]
    #[ignore = "actual Metal SDR/half-float external WAL palette replacement verification"]
    fn wal_palette_change_ram_swap_and_metal() {
        let gpu = crate::navigation_metal_readback::qualification_gpu().expect("actual Metal required");
        eprintln!("WAL_PALETTE_METAL adapter={}", gpu.adapter_name());
        qualify_wal_palette_change(Some(&gpu));
    }

    fn qualify_wal_palette_change(gpu: Option<&crate::navigation_metal_readback::GpuReadback>) {
        for spilled in [false, true] {
            let directory = tempfile::tempdir().unwrap();
            std::fs::create_dir(directory.path().join("textures")).unwrap();
            std::fs::create_dir(directory.path().join("pics")).unwrap();
            let path = directory.path().join("textures/grid.wal");
            let palette = directory.path().join("pics/colormap.pcx");
            std::fs::write(
                &path,
                include_bytes!("../../../tests/fixtures/raster/palette-grid.wal"),
            )
            .unwrap();
            let mut pcx = include_bytes!("../../../tests/fixtures/raster/wal-colormap.pcx").to_vec();
            std::fs::write(&palette, &pcx).unwrap();
            let palette_time = std::fs::metadata(&palette).unwrap().modified().unwrap();
            let old_palette = SourceFingerprint::from_path(&palette).unwrap();
            let wal = SourceFingerprint::from_path(&path).unwrap();
            let root = rrrah_cache::MemoryBudget::new(8 * 1024 * 1024);
            let mut cache = raster_cache_with_swap(
                Some(directory.path()),
                false,
                rrrah_cache::CacheLimits {
                    max_bytes: 1024 * 1024,
                    max_entries: Some(1),
                    ttl: None,
                },
                Some(rrrah_cache::ImageSwapConfig {
                    limits: rrrah_cache::CacheLimits::bytes(1024 * 1024),
                    queue_bytes: 1024 * 1024,
                    queue_count: 4,
                    restore_bytes: 1024 * 1024,
                }),
                Some(&root),
            );
            let mut request = DecodeRequest::new(&path);
            request.memory_budget = Some(root.clone());
            let (first, _) = load_cached_raster_for_display(&request, None, &mut cache).unwrap();
            let old_key = raster_display_key(&request, None).unwrap();
            let rrrah_core::RasterPixels::Rgba32Float(values) = first.pixels() else {
                panic!()
            };
            assert_eq!(&values[..4], &[0., 0., 0., 1.]);
            if spilled {
                let mut other = DecodeRequest::new(
                    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/raster/pattern.png"),
                );
                other.memory_budget = Some(root.clone());
                load_cached_raster_for_display(&other, None, &mut cache).unwrap();
                cache.swap().unwrap().wait_idle().unwrap();
                assert_eq!(cache.swap().unwrap().stats().writes, 1);
            }
            let at = pcx.len() - 768;
            pcx[at..at + 3].copy_from_slice(&[255, 0, 0]);
            std::fs::write(&palette, &pcx).unwrap();
            std::fs::File::options()
                .write(true)
                .open(&palette)
                .unwrap()
                .set_times(std::fs::FileTimes::new().set_modified(palette_time))
                .unwrap();
            let changed = SourceFingerprint::from_path(&palette).unwrap();
            assert_eq!(
                (old_palette.file_size, old_palette.modified_ns),
                (changed.file_size, changed.modified_ns)
            );
            assert_ne!(old_palette.sampled_blake3, changed.sampled_blake3);
            assert_eq!(SourceFingerprint::from_path(&path).unwrap(), wal);
            let calls = std::cell::Cell::new(0);
            let (second, _) = load_cached_raster_with(&request, None, &mut cache, || {
                calls.set(calls.get() + 1);
                load_raster_for_display_typed(&request, None)
            })
            .unwrap();
            assert_eq!(
                calls.get(),
                1,
                "palette change served stale pixels; spilled={spilled}"
            );
            let rrrah_core::RasterPixels::Rgba32Float(values) = second.pixels() else {
                panic!()
            };
            assert_eq!(&values[..4], &[1., 0., 0., 1.]);
            if let Some(gpu) = gpu {
                let original = include_bytes!("../../../tests/fixtures/raster/palette-grid.wal.rgba");
                let wal_bytes = include_bytes!("../../../tests/fixtures/raster/palette-grid.wal");
                let offset = u32::from_le_bytes(wal_bytes[40..44].try_into().unwrap()) as usize;
                let mut changed = original.to_vec();
                for (index, pixel) in changed.chunks_exact_mut(4).enumerate() {
                    if wal_bytes[offset + index] == 0 {
                        pixel.copy_from_slice(&[255, 0, 0, 255]);
                    }
                }
                let view = rrrah_gpu::ViewParameters {
                    viewport: [128., 128.],
                    zoom: 8.,
                    ..Default::default()
                };
                for (actual, oracle) in [(&first, original.as_slice()), (&second, changed.as_slice())] {
                    let raw = rrrah_core::DecodedRaster::new(
                        16,
                        16,
                        rrrah_core::RasterPixels::Rgba8(Arc::new(oracle.to_vec()).into()),
                        rrrah_core::RasterColorSpace::AssumedSrgb,
                    )
                    .unwrap();
                    let expected = rrrah_decode::prepare_raster_for_display(&raw).unwrap();
                    assert_eq!(
                        gpu.render_raster(actual, view, [128, 128]).pixels,
                        gpu.render_raster(&expected, view, [128, 128]).pixels
                    );
                    assert_eq!(
                        gpu.render_raster_half(actual, view, [128, 128]),
                        gpu.render_raster_half(&expected, view, [128, 128])
                    );
                }
            }
            assert_ne!(raster_display_key(&request, None).unwrap(), old_key);
            assert_eq!(
                cache.swap().unwrap().stats().reads,
                0,
                "old palette blob must not be restored"
            );
            cache.swap().unwrap().wait_idle().unwrap();
            std::fs::remove_file(&palette).unwrap();
            assert!(load_cached_raster_for_display(&request, None, &mut cache).is_err());
            assert_eq!(
                cache.len(),
                1,
                "failed new palette keeps previous valid membership"
            );
            let rrrah_core::RasterPixels::Rgba32Float(values) = first.pixels() else {
                panic!()
            };
            assert_eq!(
                &values[..4],
                &[0., 0., 0., 1.],
                "held previous pixels are immutable"
            );
            drop(first);
            drop(second);
            drop(cache);
            assert_eq!(root.used(), 0);
        }
    }

    #[test]
    fn protected_count_slots_skip_far_decode_but_keep_hits_and_retry_after_release() {
        let base = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/data");
        let mut cache = RasterDisplayCache::new(rrrah_cache::CacheLimits {
            max_bytes: 1024 * 1024,
            max_entries: Some(2),
            ttl: None,
        });
        let selected = DecodeRequest::new(base.join("apng-16bit-precision.png"));
        load_cached_raster_with(&selected, None, &mut cache, || {
            load_raster_for_display_typed(&selected, None)
        })
        .unwrap();
        let nearest = DecodeRequest::new(base.join("apng-2bit-packed.png"));
        let gate = Arc::new(DecodeGate::new());
        let (_, _, lease) = preload_raster_with_lease(&nearest, None, &mut cache, &gate).unwrap();
        let lease = lease.expect("nearest retained");
        let far = DecodeRequest::new(base.join("apng-1bit-packed.png"));
        let far_key = raster_display_key(&far, None).expect("far source key");
        let error = load_cached_raster_mode_resolving_key(
            &far,
            || Some(far_key.clone()),
            &mut cache,
            false,
            true,
            || panic!("protected count must skip decode"),
        )
        .unwrap_err();
        assert!(error.contains("all count slots are protected"));
        let near_key = raster_display_key(&nearest, None).unwrap();
        load_cached_raster_mode_resolving_key(
            &nearest,
            || Some(near_key),
            &mut cache,
            false,
            true,
            || panic!("cache hit must precede count preflight"),
        )
        .unwrap();
        drop(lease);
        let calls = std::cell::Cell::new(0);
        load_cached_raster_mode_resolving_key(
            &far,
            || Some(far_key),
            &mut cache,
            false,
            true,
            || {
                calls.set(calls.get() + 1);
                load_raster_for_display_typed(&far, None)
            },
        )
        .unwrap();
        assert_eq!(
            calls.get(),
            1,
            "released neighbour makes count admission possible"
        );
    }

    #[test]
    fn count_preflight_keeps_swap_hits_available_when_all_ram_slots_are_protected() {
        let directory = tempfile::tempdir().unwrap();
        let root = rrrah_cache::MemoryBudget::new(1024 * 1024);
        let mut cache = raster_cache_with_swap(
            Some(directory.path()),
            false,
            rrrah_cache::CacheLimits {
                max_bytes: 65536,
                max_entries: Some(2),
                ttl: None,
            },
            Some(rrrah_cache::ImageSwapConfig {
                limits: rrrah_cache::CacheLimits::bytes(65536),
                queue_bytes: 65536,
                queue_count: 1,
                restore_bytes: 65536,
            }),
            Some(&root),
        );
        let base = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/data");
        let request = |name| {
            let mut request = DecodeRequest::new(base.join(name));
            request.memory_budget = Some(root.clone());
            request
        };
        let selected = request("apng-16bit-precision.png");
        load_cached_raster_for_display(&selected, None, &mut cache).unwrap();
        let nearest = request("apng-2bit-packed.png");
        let gate = Arc::new(DecodeGate::new());
        let (_, _, lease) = preload_raster_with_lease(&nearest, None, &mut cache, &gate).unwrap();
        let lease = lease.expect("nearest retained");
        let far = request("apng-1bit-packed.png");
        let (frame, assumed) = load_raster_for_display_typed(&far, None).unwrap();
        let rrrah_core::RasterPixels::Rgba32Float(values) = frame.pixels() else {
            panic!("prepared float")
        };
        let expected: Vec<_> = values.iter().map(|value| value.to_bits()).collect();
        let mut key = raster_display_key(&far, None).unwrap();
        key.6 = assumed;
        cache.swap().unwrap().enqueue(key.clone(), frame);
        cache.swap().unwrap().wait_idle().unwrap();
        let resident = cache.resident_bytes();
        let used = root.used();
        assert!(!cache.can_admit_key_by_count(&key));
        let reads = cache.swap().unwrap().stats().reads;
        let (restored, actual_assumption) = load_cached_raster_mode_resolving_key(
            &far,
            || Some(key),
            &mut cache,
            false,
            true,
            || panic!("swap hit must precede protected-count preflight"),
        )
        .unwrap();
        assert_eq!(actual_assumption, assumed);
        let rrrah_core::RasterPixels::Rgba32Float(values) = restored.pixels() else {
            panic!("restored float")
        };
        assert_eq!(
            values.iter().map(|value| value.to_bits()).collect::<Vec<_>>(),
            expected
        );
        assert_eq!(cache.swap().unwrap().stats().reads, reads + 1);
        assert_eq!((cache.len(), cache.resident_bytes()), (2, resident));
        drop(restored);
        assert_eq!(root.used(), used);
        load_cached_raster_with(&selected, None, &mut cache, || panic!("visible displaced")).unwrap();
        drop(cache);
        assert!(root.used() > 0, "nearest lease still owns managed data");
        drop(lease);
        assert_eq!(root.used(), 0);
    }

    #[test]
    fn protected_preloads_return_leases_for_the_loaded_allocation() {
        let root = rrrah_cache::MemoryBudget::new(1024 * 1024);
        let gate = Arc::new(DecodeGate::new());
        let base = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures");
        let mut models = ModelDisplayCache::new(rrrah_cache::CacheLimits::bytes(1024 * 1024));
        let mut request = DecodeRequest::new(base.join("models/triangle.stl"));
        request.memory_budget = Some(root.clone());
        let (model, lease) = preload_model_with_lease(&request, &mut models, &gate).unwrap();
        let lease = lease.unwrap();
        let (rrrah_decode::DecodedModel::Stl(a), rrrah_decode::DecodedModel::Stl(b)) = (&model, &*lease)
        else {
            panic!()
        };
        assert!(a.ptr_eq(b));
        drop(model);
        drop(models);
        assert!(root.used() > 0);
        drop(lease);
        assert_eq!(root.used(), 0);
        let mut rasters = RasterDisplayCache::new(rrrah_cache::CacheLimits::bytes(1024 * 1024));
        request.path = base.join("data/apng-16bit-precision.png");
        request.image_index = 1;
        let (frame, assumed, lease) = preload_raster_with_lease(&request, None, &mut rasters, &gate).unwrap();
        assert!(!assumed);
        let lease = lease.unwrap();
        let (rrrah_core::RasterPixels::Rgba32Float(a), rrrah_core::RasterPixels::Rgba32Float(b)) =
            (frame.pixels(), lease.pixels())
        else {
            panic!()
        };
        assert!(a.ptr_eq(b));
        assert_eq!((lease.image_index(), lease.image_count()), (1, 2));
        drop(frame);
        drop(rasters);
        assert_eq!(root.used(), 16);
        drop(lease);
        assert_eq!(root.used(), 0);
    }

    #[test]
    fn model_prefetch_priority_survives_count_bytes_and_failed_neighbor() {
        for name in [
            "triangle.stl",
            "off-attributes-binary.off",
            "obj-shared-metadata.obj",
            "ply-64-le.ply",
        ] {
            let source = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("../../tests/fixtures/models")
                .join(name);
            let weight = rrrah_decode::decode_model(&DecodeRequest::new(&source))
                .unwrap()
                .capacity_bytes();
            for limits in [
                rrrah_cache::CacheLimits {
                    max_bytes: 128 * 1024,
                    max_entries: Some(2),
                    ttl: None,
                },
                rrrah_cache::CacheLimits {
                    max_bytes: weight * 2,
                    max_entries: None,
                    ttl: None,
                },
            ] {
                let root = rrrah_cache::MemoryBudget::new(128 * 1024);
                let dir = tempfile::tempdir().unwrap();
                let mut cache = ModelDisplayCache::new(limits);
                let mut selected = DecodeRequest::new(&source);
                selected.memory_budget = Some(root.clone());
                let model = cache
                    .load(&selected, true, || {
                        rrrah_decode::decode_model(&selected).map_err(ModelLoadError::Decode)
                    })
                    .unwrap();
                drop(model);
                let mut bad = selected.clone();
                bad.path = dir.path().join("corrupt.stl");
                std::fs::write(&bad.path, b"truncated").unwrap();
                assert!(
                    cache
                        .load(&bad, false, || rrrah_decode::decode_model(&bad)
                            .map_err(ModelLoadError::Decode))
                        .is_err()
                );
                assert!(cache.prefetch_lease(&bad).is_none());
                assert_eq!(cache.entries.len(), 1);
                let mut nearest = selected.clone();
                nearest.path = dir.path().join(format!("nearest-{name}"));
                std::fs::copy(&source, &nearest.path).unwrap();
                let model = cache
                    .load(&nearest, false, || {
                        rrrah_decode::decode_model(&nearest).map_err(ModelLoadError::Decode)
                    })
                    .unwrap();
                let protected = cache.prefetch_lease(&nearest).unwrap();
                drop(model);
                let mut farther = selected.clone();
                farther.path = dir.path().join(format!("farther-{name}"));
                std::fs::copy(&source, &farther.path).unwrap();
                let model = cache
                    .load(&farther, false, || {
                        rrrah_decode::decode_model(&farther).map_err(ModelLoadError::Decode)
                    })
                    .unwrap();
                drop(model);
                let hit = cache
                    .load(&nearest, false, || {
                        panic!("farther model displaced protected nearest: {name}")
                    })
                    .unwrap();
                drop(hit);
                assert_eq!(cache.entries.len(), 2);
                drop(protected);
                let model = cache
                    .load(&farther, false, || {
                        rrrah_decode::decode_model(&farther).map_err(ModelLoadError::Decode)
                    })
                    .unwrap();
                drop(model);
                assert!(cache.prefetch_lease(&nearest).is_none());
                drop(cache);
                assert_eq!(root.used(), 0);
            }
        }
    }

    #[test]
    fn nearest_prefetched_raster_survives_count_and_byte_pressure_after_failed_neighbor() {
        let bad_dir = tempfile::tempdir().unwrap();
        let bad_path = bad_dir.path().join("corrupt.png");
        std::fs::write(&bad_path, b"\x89PNG\r\n\x1a\ntruncated").unwrap();
        for limits in [
            rrrah_cache::CacheLimits {
                max_bytes: 1024 * 1024,
                max_entries: Some(2),
                ttl: None,
            },
            rrrah_cache::CacheLimits {
                max_bytes: 80,
                max_entries: None,
                ttl: None,
            },
        ] {
            let root = rrrah_cache::MemoryBudget::new(1024 * 1024);
            let mut cache = RasterDisplayCache::new(limits);
            let base = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/data");
            let mut selected = DecodeRequest::new(base.join("apng-16bit-precision.png"));
            selected.memory_budget = Some(root.clone());
            let (visible, _) = load_cached_raster_with(&selected, None, &mut cache, || {
                load_raster_for_display_typed(&selected, None)
            })
            .unwrap();
            drop(visible);
            let mut bad = DecodeRequest::new(&bad_path);
            bad.memory_budget = Some(root.clone());
            assert!(
                load_cached_raster_mode(&bad, None, &mut cache, false, || load_raster_for_display_typed(
                    &bad, None
                ))
                .is_err()
            );
            assert!(retain_prefetched_raster(&bad, None, false, &mut cache).is_none());
            assert_eq!((cache.len(), root.used()), (1, 16));
            let mut nearest = DecodeRequest::new(base.join("apng-2bit-packed.png"));
            nearest.memory_budget = Some(root.clone());
            let (frame, assumed) = load_cached_raster_mode(&nearest, None, &mut cache, false, || {
                load_raster_for_display_typed(&nearest, None)
            })
            .unwrap();
            let protected = retain_prefetched_raster(&nearest, None, assumed, &mut cache).unwrap();
            drop(frame);
            let mut farther = DecodeRequest::new(base.join("apng-4bit-packed.png"));
            farther.memory_budget = Some(root.clone());
            let (frame, _) = load_cached_raster_mode(&farther, None, &mut cache, false, || {
                load_raster_for_display_typed(&farther, None)
            })
            .unwrap();
            drop(frame);
            let (hit, _) = load_cached_raster_mode(&nearest, None, &mut cache, false, || {
                panic!("farther preload evicted protected nearest")
            })
            .unwrap();
            drop(hit);
            assert_eq!(cache.len(), 2);
            drop(protected);
            let (frame, _) = load_cached_raster_mode(&farther, None, &mut cache, false, || {
                load_raster_for_display_typed(&farther, None)
            })
            .unwrap();
            drop(frame);
            assert!(retain_prefetched_raster(&nearest, None, assumed, &mut cache).is_none());
            drop(cache);
            assert_eq!(root.used(), 0);
        }
    }

    #[test]
    fn apng_selected_frames_expire_and_prefetch_from_swap_without_decode() {
        qualify_apng_selected_frame_ttl(None);
    }

    #[test]
    #[ignore = "manual actual Metal APNG nonzero TTL and decoder-blocked prefetch qualification"]
    fn apng_selected_frame_ttl_prefetch_preserves_metal_sdr_hdr() {
        let gpu = crate::navigation_metal_readback::qualification_gpu().expect("actual GPU required");
        eprintln!("APNG viewer TTL adapter: {}", gpu.adapter_name());
        qualify_apng_selected_frame_ttl(Some(&gpu));
        eprintln!(
            "APNG viewer TTL: four distinct frames; nonzero expiry; reverse prefetch under blocked decoder; SDR/HDR exact; final=0"
        );
    }

    #[test]
    fn tiff_selected_pages_loader_cache_and_blocked_prefetch() {
        qualify_tiff_page_loader(None);
    }

    #[test]
    #[ignore = "actual Metal TIFF viewer selected-page cache/TTL/prefetch qualification"]
    fn tiff_selected_pages_loader_cache_and_blocked_prefetch_metal() {
        let gpu = crate::navigation_metal_readback::GpuReadback::new().expect("actual Metal required");
        eprintln!("TIFF viewer pages adapter: {}", gpu.adapter_name());
        qualify_tiff_page_loader(Some(&gpu));
    }

    fn qualify_tiff_page_loader(gpu: Option<&crate::navigation_metal_readback::GpuReadback>) {
        qualify_selected_container_loader(gpu, "tiff-distinct-page-profiles.tif");
    }

    #[test]
    fn ico_cur_selected_images_loader_cache_and_blocked_prefetch() {
        for name in [
            "ico-selected-images.ico",
            "cur-selected-images.cur",
            "cur-selected-dib.cur",
        ] {
            qualify_selected_container_loader(None, name);
        }
    }

    #[test]
    #[ignore = "actual Metal ICO/CUR production loader cache and blocked prefetch"]
    fn ico_cur_selected_images_loader_cache_and_blocked_prefetch_metal() {
        let gpu = crate::navigation_metal_readback::GpuReadback::new().expect("actual Metal required");
        eprintln!("ICO/CUR viewer adapter: {}", gpu.adapter_name());
        for name in [
            "ico-selected-images.ico",
            "cur-selected-images.cur",
            "cur-selected-dib.cur",
        ] {
            qualify_selected_container_loader(Some(&gpu), name);
        }
    }

    #[test]
    #[ignore = "manual 64MiB selected-page swap cancellation after admission and Metal retry"]
    fn selected_page_prefetch_cancel_after_restore_admission_metal() {
        let gpu = crate::navigation_metal_readback::GpuReadback::new().expect("actual Metal required");
        eprintln!(
            "Selected-page admitted cancellation adapter: {}",
            gpu.adapter_name()
        );
        let bytes = 2048u64 * 2048 * 16;
        let root = rrrah_cache::MemoryBudget::new(bytes * 2);
        let dir = tempfile::tempdir().unwrap();
        let mut cache = raster_cache_with_swap(
            Some(dir.path()),
            false,
            rrrah_cache::CacheLimits {
                max_bytes: bytes,
                max_entries: Some(0),
                ttl: None,
            },
            Some(rrrah_cache::ImageSwapConfig {
                limits: rrrah_cache::CacheLimits {
                    max_bytes: bytes * 3,
                    max_entries: Some(2),
                    ttl: None,
                },
                queue_bytes: bytes,
                queue_count: 1,
                restore_bytes: bytes,
            }),
            Some(&root),
        );
        let generation = Arc::new(AtomicU64::new(1));
        let mut request = DecodeRequest::new(
            PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("../../tests/fixtures/raster/ico-selected-images.ico"),
        );
        // The source provides a stable key only; these authored transport payloads
        // deliberately avoid decoder work in the blocked foreground-gate scenario.
        request.memory_budget = Some(root.clone());
        for index in [1, 2] {
            request.image_index = index;
            let values = if index == 1 {
                [4., -0.5, 0.25, 1.]
            } else {
                [-0.25, 0.5, 4., 1.]
            };
            let frame = rrrah_core::DecodedRaster::new(
                2048,
                2048,
                rrrah_core::RasterPixels::Rgba32Float(Arc::new(values.repeat(2048 * 2048)).into()),
                rrrah_core::RasterColorSpace::LinearSrgb,
            )
            .unwrap()
            .with_image_selection(index, 3)
            .unwrap()
            .try_manage_pixels(&root)
            .unwrap();
            cache
                .swap()
                .unwrap()
                .enqueue(raster_display_key(&request, None).unwrap(), frame);
            cache.swap().unwrap().wait_idle().unwrap();
            assert_eq!(root.used(), 0);
        }
        let gate = Arc::new(DecodeGate::new());
        let ticket = gate.request_foreground();
        let (stop, stopped) = std::sync::mpsc::channel();
        let watchdog_generation = generation.clone();
        let watchdog = thread::spawn(move || {
            if stopped.recv_timeout(Duration::from_secs(30)).is_err() {
                watchdog_generation.store(3, Ordering::Release);
            }
        });
        request.image_index = 1;
        request.cancellation = Some(GenerationToken::new(generation.clone(), 1));
        let worker_request = request.clone();
        let worker_gate = gate.clone();
        let worker = thread::spawn(move || {
            let result = preload_raster_with_lease(&worker_request, None, &mut cache, &worker_gate);
            (cache, result)
        });
        let deadline = Instant::now() + Duration::from_secs(10);
        while root.used() < bytes && !worker.is_finished() {
            assert!(
                Instant::now() < deadline,
                "selected-page restore admission not observed"
            );
            thread::yield_now();
        }
        let admitted = root.used() >= bytes;
        generation.store(2, Ordering::Release);
        let (mut cache, result) = worker.join().unwrap();
        assert!(admitted, "must cancel an actually admitted restore");
        assert!(
            result.is_err(),
            "obsolete restored page must not publish a frame or lease"
        );
        assert_eq!(root.used(), 0);
        assert!(cache.is_empty());
        assert_eq!(cache.swap().unwrap().stats().reads, 0);
        request.image_index = 2;
        request.cancellation = Some(GenerationToken::new(generation, 2));
        let (frame, _, lease) = preload_raster_with_lease(&request, None, &mut cache, &gate).unwrap();
        assert!(lease.is_none());
        assert_eq!((frame.image_index(), frame.image_count()), (2, 3));
        let rrrah_core::RasterPixels::Rgba32Float(samples) = frame.pixels() else {
            panic!()
        };
        for pixel in samples.chunks_exact(4) {
            assert_eq!(pixel, &[-0.25, 0.5, 4., 1.]);
        }
        let view = rrrah_gpu::ViewParameters {
            viewport: [128., 96.],
            ..Default::default()
        };
        let reference = rrrah_core::DecodedRaster::new(
            2,
            2,
            rrrah_core::RasterPixels::Rgba32Float(Arc::new([-0.25, 0.5, 4., 1.].repeat(4)).into()),
            rrrah_core::RasterColorSpace::LinearSrgb,
        )
        .unwrap();
        assert_eq!(
            gpu.render_raster(&frame, view, [128, 96]).pixels,
            gpu.render_raster(&reference, view, [128, 96]).pixels
        );
        assert_eq!(
            gpu.render_raster_half(&frame, view, [128, 96]),
            gpu.render_raster_half(&reference, view, [128, 96])
        );
        assert_eq!(
            (
                cache.swap().unwrap().stats().writes,
                cache.swap().unwrap().stats().reads,
                cache.swap().unwrap().stats().errors
            ),
            (2, 1, 0)
        );
        drop(frame);
        drop(ticket);
        drop(cache);
        let _ = stop.send(());
        watchdog.join().unwrap();
        assert_eq!(root.used(), 0);
        eprintln!(
            "Selected-page admitted cancellation: bytes={bytes}; obsolete page1 unpublished; RAM=0; page2 retry payload/SDR/HDR exact; blocked decoder bypassed; final=0"
        );
    }

    fn qualify_selected_container_loader(
        gpu: Option<&crate::navigation_metal_readback::GpuReadback>,
        name: &str,
    ) {
        let tiff = name.starts_with("tiff");
        let max_bytes = if tiff { 96 } else { 192 };
        let queue_bytes = if tiff { 128 } else { 256 };
        let dir = tempfile::tempdir().unwrap();
        let root = rrrah_cache::MemoryBudget::new(64 * 1024);
        let mut cache = raster_cache_with_swap(
            Some(dir.path()),
            false,
            rrrah_cache::CacheLimits {
                max_bytes,
                max_entries: Some(3),
                ttl: Some(Duration::from_secs(1)),
            },
            Some(rrrah_cache::ImageSwapConfig {
                limits: rrrah_cache::CacheLimits {
                    max_bytes: 65536,
                    max_entries: Some(3),
                    ttl: None,
                },
                queue_bytes,
                queue_count: 3,
                restore_bytes: queue_bytes,
            }),
            Some(&root),
        );
        let mut request = DecodeRequest::new(
            PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("../../tests/fixtures/raster")
                .join(name),
        );
        request.memory_budget = Some(root.clone());
        let params = rrrah_gpu::ViewParameters {
            viewport: [128., 96.],
            ..Default::default()
        };
        let mut expected = Vec::new();
        for index in 0..3 {
            request.image_index = index;
            let (frame, _) = load_cached_raster_mode(&request, None, &mut cache, false, || {
                load_raster_for_display_typed(&request, None)
            })
            .unwrap();
            assert_eq!((frame.image_index(), frame.image_count()), (index, 3));
            let mut payload = Vec::new();
            rrrah_cache::SwapPayload::write_payload(&frame, &mut payload).unwrap();
            expected.push((
                payload,
                gpu.map(|gpu| gpu.render_raster(&frame, params, [128, 96]).pixels),
                gpu.map(|gpu| gpu.render_raster_half(&frame, params, [128, 96])),
            ));
        }
        assert_eq!(cache.len(), 3);
        if gpu.is_some() && tiff {
            assert_eq!(expected[0].1, expected[2].1);
            assert_ne!(expected[0].1, expected[1].1);
        }
        let pressure = root.try_reserve(root.available_bytes()).unwrap();
        for index in [1usize, 2, 0] {
            request.image_index = index;
            let (hit, _) = load_cached_raster_mode(&request, None, &mut cache, false, || {
                panic!("selected TIFF RAM hit decoded again")
            })
            .unwrap();
            assert_eq!((hit.image_index(), hit.image_count()), (index, 3));
            let mut payload = Vec::new();
            rrrah_cache::SwapPayload::write_payload(&hit, &mut payload).unwrap();
            assert_eq!(payload, expected[index].0);
        }
        drop(pressure);
        let deadline = Instant::now() + Duration::from_secs(5);
        while !cache.is_empty() {
            cache.spill_expired();
            assert!(Instant::now() < deadline, "TIFF selected page TTL did not expire");
            thread::sleep(Duration::from_millis(10));
        }
        cache.swap().unwrap().wait_idle().unwrap();
        assert_eq!(root.used(), 0);
        assert!(cache.set_limits_and_spill(rrrah_cache::CacheLimits {
            max_bytes,
            max_entries: Some(0),
            ttl: None
        }));
        let gate = Arc::new(DecodeGate::new());
        let ticket = gate.request_foreground();
        let generation = Arc::new(AtomicU64::new(1));
        request.cancellation = Some(GenerationToken::new(generation.clone(), 0));
        let (stop, stopped) = std::sync::mpsc::channel();
        let watchdog_generation = generation.clone();
        let watchdog = thread::spawn(move || {
            if stopped.recv_timeout(Duration::from_secs(5)).is_err() {
                watchdog_generation.store(2, Ordering::Release);
            }
        });
        let window = gallery::PrefetchWindow { behind: 0, ahead: 2 };
        let mut planned = gallery::neighbour_prefetch_indices(3, 2, gallery::NavDirection::Backward, window);
        planned.extend(gallery::neighbour_prefetch_indices(
            3,
            1,
            gallery::NavDirection::Forward,
            gallery::PrefetchWindow { behind: 0, ahead: 1 },
        ));
        assert_eq!(planned, [1, 0, 2]);
        for &index in &planned {
            request.image_index = index;
            assert!(
                preload_raster_with_lease(&request, None, &mut cache, &gate).is_err(),
                "obsolete selected-page prefetch must refuse before swap or decode"
            );
            assert_eq!(cache.swap().unwrap().stats().reads, 0);
            assert_eq!(root.used(), 0);
            assert!(cache.is_empty());
        }
        request.cancellation = Some(GenerationToken::new(generation, 1));
        eprintln!(
            "Selected container stale prefetch {name}: three obsolete indices refused; swap reads=0; RAM entries=0; shared credit=0; fresh generation retry follows"
        );
        for index in planned {
            request.image_index = index;
            let (frame, _) = preload_raster(&request, None, &mut cache, &gate)
                .expect("TIFF selected-page swap hit must bypass blocked decoder");
            assert_eq!((frame.image_index(), frame.image_count()), (index, 3));
            let mut payload = Vec::new();
            rrrah_cache::SwapPayload::write_payload(&frame, &mut payload).unwrap();
            assert_eq!(payload, expected[index].0);
            if let Some(gpu) = gpu {
                assert_eq!(
                    gpu.render_raster(&frame, params, [128, 96]).pixels,
                    *expected[index].1.as_ref().unwrap()
                );
                assert_eq!(
                    gpu.render_raster_half(&frame, params, [128, 96]),
                    *expected[index].2.as_ref().unwrap()
                );
            }
            drop(frame);
            assert_eq!(root.used(), 0);
        }
        let _ = stop.send(());
        watchdog.join().unwrap();
        drop(ticket);
        eprintln!(
            "Selected container planned prefetch {name}: backward2 -> [1,0]; forward1 -> [2]; swap bypasses blocked decoder; final=0"
        );
        assert_eq!(
            (
                cache.swap().unwrap().stats().writes,
                cache.swap().unwrap().stats().reads,
                cache.swap().unwrap().stats().errors
            ),
            (3, 3, 0)
        );
        drop(cache);
        assert_eq!(root.used(), 0);
        if !tiff && gpu.is_none() {
            eprintln!(
                "ICO/CUR viewer CPU {name}: selected keys; pressure RAM hits; TTL; blocked decoder reverse prefetch; payload exact; final=0"
            );
        } else if !tiff {
            eprintln!(
                "ICO/CUR viewer {name}: selected keys; pressure RAM hits; TTL; blocked decoder reverse prefetch; payload/hotspot/SDR/HDR exact; final=0"
            );
        } else if gpu.is_some() {
            eprintln!(
                "TIFF viewer pages: three independent selected keys; full-pressure RAM hits; nonzero TTL; blocked decoder reverse prefetch; payload and SDR/HDR exact; final=0"
            );
        } else {
            eprintln!(
                "TIFF viewer pages CPU: three selected keys; full-pressure RAM hits; nonzero TTL; blocked decoder reverse prefetch; payload exact; final=0"
            );
        }
    }

    fn qualify_apng_selected_frame_ttl(gpu: Option<&crate::navigation_metal_readback::GpuReadback>) {
        for name in ["apng-16bit-precision.png", "apng-2bit-packed.png"] {
            let dir = tempfile::tempdir().unwrap();
            let root = rrrah_cache::MemoryBudget::new(64 * 1024);
            let mut cache = raster_cache_with_swap(
                Some(dir.path()),
                false,
                rrrah_cache::CacheLimits {
                    max_bytes: 128,
                    max_entries: Some(2),
                    ttl: Some(Duration::from_secs(1)),
                },
                Some(rrrah_cache::ImageSwapConfig {
                    limits: rrrah_cache::CacheLimits {
                        max_bytes: 65536,
                        max_entries: Some(2),
                        ttl: None,
                    },
                    queue_bytes: 128,
                    queue_count: 2,
                    restore_bytes: 128,
                }),
                Some(&root),
            );
            let mut request = DecodeRequest::new(
                PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                    .join("../../tests/fixtures/data")
                    .join(name),
            );
            request.memory_budget = Some(root.clone());
            let params = rrrah_gpu::ViewParameters {
                viewport: [128., 96.],
                ..Default::default()
            };
            let mut expected = Vec::new();
            for index in 0..2 {
                request.image_index = index;
                let (frame, _) = load_cached_raster_mode(&request, None, &mut cache, false, || {
                    load_raster_for_display_typed(&request, None)
                })
                .unwrap();
                assert_eq!((frame.image_index(), frame.image_count()), (index, 2));
                let mut payload = Vec::new();
                rrrah_cache::SwapPayload::write_payload(&frame, &mut payload).unwrap();
                let sdr = gpu.map(|gpu| gpu.render_raster(&frame, params, [128, 96]).pixels);
                let hdr = gpu.map(|gpu| gpu.render_raster_half(&frame, params, [128, 96]));
                expected.push((payload, sdr, hdr));
            }
            assert_ne!(expected[0].0, expected[1].0);
            assert_eq!(cache.len(), 2);
            assert_eq!(cache.spill_expired(), 0);
            let deadline = Instant::now() + Duration::from_secs(5);
            let mut expired = 0;
            while !cache.is_empty() {
                expired += cache.spill_expired();
                assert!(
                    Instant::now() < deadline,
                    "nonzero TTL did not expire selected frames"
                );
                thread::sleep(Duration::from_millis(10));
            }
            cache.swap().unwrap().wait_idle().unwrap();
            assert_eq!(expired, 2);
            assert_eq!(root.used(), 0);
            assert!(cache.set_limits_and_spill(rrrah_cache::CacheLimits {
                max_bytes: 128,
                max_entries: Some(0),
                ttl: None,
            }));
            let gate = Arc::new(DecodeGate::new());
            let ticket = gate.request_foreground();
            let generation = Arc::new(AtomicU64::new(0));
            request.cancellation = Some(GenerationToken::new(generation.clone(), 0));
            let (stop, stopped) = std::sync::mpsc::channel();
            let watchdog = thread::spawn(move || {
                if stopped.recv_timeout(Duration::from_secs(5)).is_err() {
                    generation.store(1, Ordering::Release);
                }
            });
            for index in [1usize, 0] {
                request.image_index = index;
                let (frame, _) = preload_raster(&request, None, &mut cache, &gate)
                    .expect("APNG swap preload must bypass blocked decoder");
                let mut payload = Vec::new();
                rrrah_cache::SwapPayload::write_payload(&frame, &mut payload).unwrap();
                assert_eq!(payload, expected[index].0);
                if let Some(gpu) = gpu {
                    assert_eq!(
                        gpu.render_raster(&frame, params, [128, 96]).pixels,
                        *expected[index].1.as_ref().unwrap()
                    );
                    assert_eq!(
                        gpu.render_raster_half(&frame, params, [128, 96]),
                        *expected[index].2.as_ref().unwrap()
                    );
                }
                drop(frame);
                assert_eq!(root.used(), 0);
            }
            let _ = stop.send(());
            watchdog.join().unwrap();
            drop(ticket);
            assert_eq!(
                (
                    cache.swap().unwrap().stats().writes,
                    cache.swap().unwrap().stats().reads,
                    cache.swap().unwrap().stats().errors
                ),
                (2, 2, 0)
            );
            drop(cache);
            assert_eq!(root.used(), 0);
        }
    }

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
    fn repeated_hdr_ttl_spill_and_restore_preserves_payload_without_decode() {
        qualify_hdr_ttl_viewer_restore(None);
    }

    #[test]
    #[ignore = "requires actual Metal for viewer TTL swap and prefetch readback"]
    fn repeated_hdr_ttl_viewer_prefetch_restore_preserves_metal_frames() {
        let gpu = crate::navigation_metal_readback::qualification_gpu().expect("actual GPU required");
        eprintln!("viewer HDR TTL adapter: {}", gpu.adapter_name());
        qualify_hdr_ttl_viewer_restore(Some(&gpu));
        eprintln!(
            "viewer HDR TTL: 128 restores without decoder fallback plus 2 real preloads under blocked decoder; all Metal frames exact; final=0"
        );
        eprintln!("viewer HDR TTL binary16: 130 finite HDR frames exact; above-one oracle retained");
    }

    fn qualify_hdr_ttl_viewer_restore(gpu: Option<&crate::navigation_metal_readback::GpuReadback>) {
        let parent = tempfile::tempdir().unwrap();
        let root = rrrah_cache::MemoryBudget::new(64 * 1024);
        let cli = cli();
        let mut cache = raster_cache_with_swap(
            Some(parent.path()),
            false,
            rrrah_cache::CacheLimits {
                max_bytes: 4096,
                max_entries: Some(1),
                ttl: Some(Duration::ZERO),
            },
            cli.raster_swap_config(),
            Some(&root),
        );
        for name in ["float-none-rec709-declared.exr", "rle-hdr-rec709-declared.hdr"] {
            let mut request = DecodeRequest::new(
                PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                    .join("../../tests/fixtures/raster")
                    .join(name),
            );
            request.memory_budget = Some(root.clone());
            let (frame, _) = load_cached_raster_mode(&request, None, &mut cache, false, || {
                load_raster_for_display_typed(&request, None)
            })
            .unwrap();
            let rrrah_core::RasterPixels::Rgba32Float(samples) = frame.pixels() else {
                panic!()
            };
            assert!(samples.iter().any(|v| *v > 1.0));
            let mut expected = Vec::new();
            rrrah_cache::SwapPayload::write_payload(&frame, &mut expected).unwrap();
            let params = rrrah_gpu::ViewParameters {
                viewport: [128.0, 96.0],
                ..Default::default()
            };
            let expected_frame = gpu.map(|gpu| gpu.render_raster(&frame, params, [128, 96]).pixels);
            let expected_hdr = gpu.map(|gpu| {
                let hdr = gpu.render_raster_half(&frame, params, [128, 96]);
                assert!(hdr.iter().all(|v| v & 0x7c00 != 0x7c00));
                assert!(
                    hdr.chunks_exact(4)
                        .any(|p| { p[..3].iter().any(|v| v & 0x8000 == 0 && *v > 0x3c00) })
                );
                hdr
            });
            drop(frame);
            if let Some(gpu) = gpu {
                assert_eq!(cache.spill_expired(), 1);
                cache.swap().unwrap().wait_idle().unwrap();
                let gate = Arc::new(DecodeGate::new());
                let ticket = gate.request_foreground();
                let generation = Arc::new(AtomicU64::new(0));
                request.cancellation = Some(GenerationToken::new(generation.clone(), 0));
                let (stop, stopped) = std::sync::mpsc::channel();
                let watchdog = thread::spawn(move || {
                    if stopped.recv_timeout(Duration::from_secs(5)).is_err() {
                        generation.store(1, Ordering::Release);
                    }
                });
                let result = preload_raster(&request, None, &mut cache, &gate);
                let _ = stop.send(());
                watchdog.join().unwrap();
                let (prefetched, _) = result.expect("swap preload must bypass blocked decoder gate");
                drop(ticket);
                request.cancellation = None;
                assert_eq!(
                    gpu.render_raster(&prefetched, params, [128, 96]).pixels,
                    *expected_frame.as_ref().unwrap()
                );
                assert_eq!(
                    gpu.render_raster_half(&prefetched, params, [128, 96]),
                    *expected_hdr.as_ref().unwrap()
                );
                drop(prefetched);
            }
            for _ in 0..64 {
                assert_eq!(cache.spill_expired(), 1);
                cache.swap().unwrap().wait_idle().unwrap();
                assert!(cache.is_empty());
                assert_eq!(root.used(), 0);
                let (restored, _) = load_cached_raster_mode(&request, None, &mut cache, false, || {
                    panic!("expired HDR frame decoded instead of restored")
                })
                .unwrap();
                let mut actual = Vec::new();
                rrrah_cache::SwapPayload::write_payload(&restored, &mut actual).unwrap();
                assert_eq!(actual, expected);
                if let Some(gpu) = gpu {
                    assert_eq!(
                        gpu.render_raster(&restored, params, [128, 96]).pixels,
                        *expected_frame.as_ref().unwrap()
                    );
                    assert_eq!(
                        gpu.render_raster_half(&restored, params, [128, 96]),
                        *expected_hdr.as_ref().unwrap()
                    );
                }
                assert_eq!(cache.len(), 1);
                assert!(cache.resident_bytes() <= 4096);
                drop(restored);
            }
            assert_eq!(cache.spill_expired(), 1);
            cache.swap().unwrap().wait_idle().unwrap();
            assert_eq!(root.used(), 0);
        }
        assert_eq!(
            cache.swap().unwrap().stats().reads,
            if gpu.is_some() { 130 } else { 128 }
        );
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
                    ttl: Some(Duration::from_millis(100)),
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
            assert_eq!(
                cache.spill_expired(),
                0,
                "unexpired entry must survive maintenance"
            );
            assert!(root.used() > 0);
            let deadline = Instant::now() + Duration::from_secs(3);
            receive_with_maintenance(&rx, Some(Duration::from_millis(1)), || {
                assert!(
                    Instant::now() < deadline,
                    "idle TTL must expire without navigation"
                );
                if cache.spill_expired() == 1 {
                    tx.try_send(()).unwrap();
                }
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
    fn mng_single_play_stops_and_retains_timing_on_swap_restore() {
        let parent = tempfile::tempdir().unwrap();
        let root = rrrah_cache::MemoryBudget::new(1024 * 1024);
        let mut cache = raster_cache_with_swap(
            Some(parent.path()),
            false,
            rrrah_cache::CacheLimits {
                max_bytes: 8192,
                max_entries: Some(1),
                ttl: None,
            },
            Some(rrrah_cache::ImageSwapConfig {
                limits: rrrah_cache::CacheLimits {
                    max_bytes: 65536,
                    max_entries: Some(8),
                    ttl: None,
                },
                queue_bytes: 8192,
                queue_count: 4,
                restore_bytes: 8192,
            }),
            Some(&root),
        );
        let mut request = DecodeRequest::new(
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/mng/rgb8-two-frames.mng"),
        );
        request.assume_untagged_srgb = true;
        request.memory_budget = Some(root.clone());
        let mut clock = raster_playback::Playback::default();
        let now = Instant::now();
        for index in 0..2 {
            request.image_index = index;
            let (frame, _) = load_cached_raster_for_display(&request, None, &mut cache).unwrap();
            let timing = frame.animation_timing().unwrap();
            assert_eq!(timing.num_plays(), 1);
            assert_eq!(timing.delay(), Duration::from_millis(100));
            clock.ready(index as u64 + 1, index, frame.image_count(), Some(timing));
            clock.presented(index as u64 + 1, now);
            let next = clock.take_next(index as u64 + 1, now + Duration::from_millis(100));
            if index == 0 {
                assert_eq!(next, Some(1));
                clock.submitted(2);
            } else {
                assert_eq!(next, None);
            }
            drop(frame);
            cache.swap().unwrap().wait_idle().unwrap();
        }
        request.image_index = 0;
        let (restored, _) = load_cached_raster_with(&request, None, &mut cache, || {
            panic!("MNG first frame must restore without decode")
        })
        .unwrap();
        assert_eq!(restored.animation_timing().unwrap().num_plays(), 1);
        assert_eq!(cache.swap().unwrap().stats().reads, 1);
        drop(restored);
        cache.swap().unwrap().shutdown();
        drop(cache);
        assert_eq!(root.used(), 0);
    }
    #[test]
    fn aseprite_non_square_frames_restore_aspect_without_repeat_decode() {
        qualify_aseprite_aspect_swap(None);
    }
    #[test]
    #[ignore = "requires actual Metal for Aseprite non-square pixel swap rendering"]
    fn aseprite_non_square_swap_preserves_metal_output() {
        let gpu = crate::navigation_metal_readback::qualification_gpu().expect("actual Metal required");
        eprintln!("ASE_ASPECT_METAL adapter={}", gpu.adapter_name());
        qualify_aseprite_aspect_swap(Some(&gpu));
    }
    fn qualify_aseprite_aspect_swap(gpu: Option<&crate::navigation_metal_readback::GpuReadback>) {
        let fixture = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../tests/fixtures/raster/ase-32-0-2019-1.aseprite");
        let original = std::fs::read(fixture).unwrap();
        let mut outputs = Vec::new();
        for (width, height) in [
            (2u8, 1u8),
            (1, 2),
            (255, 1),
            (1, 255),
            (7, 3),
            (0, 255),
            (255, 0),
            (0, 0),
            (3, 3),
        ] {
            let dir = tempfile::tempdir().unwrap();
            let path = dir.path().join("aspect.aseprite");
            let mut bytes = original.clone();
            bytes[34] = width;
            bytes[35] = height;
            std::fs::write(&path, bytes).unwrap();
            let root = rrrah_cache::MemoryBudget::new(1024 * 1024);
            let mut cache = raster_cache_with_swap(
                Some(dir.path()),
                false,
                rrrah_cache::CacheLimits {
                    max_bytes: 8192,
                    max_entries: Some(1),
                    ttl: None,
                },
                Some(rrrah_cache::ImageSwapConfig {
                    limits: rrrah_cache::CacheLimits::bytes(65536),
                    queue_bytes: 8192,
                    queue_count: 4,
                    restore_bytes: 8192,
                }),
                Some(&root),
            );
            let mut request = DecodeRequest::new(path);
            request.memory_budget = Some(root.clone());
            let (first, _) = load_cached_raster_for_display(&request, None, &mut cache).unwrap();
            let expected = if width == 0 || height == 0 || width == height {
                None
            } else {
                Some(f32::from(width) / f32::from(height))
            };
            assert_eq!(first.pixel_aspect(), expected);
            let params = rrrah_gpu::ViewParameters {
                viewport: [128., 96.],
                ..Default::default()
            };
            let rendered = gpu.map(|gpu| {
                (
                    gpu.render_raster(&first, params, [128, 96]).pixels,
                    gpu.render_raster_half(&first, params, [128, 96]),
                )
            });
            request.image_index = 1;
            drop(load_cached_raster_for_display(&request, None, &mut cache).unwrap());
            cache.swap().unwrap().wait_idle().unwrap();
            request.image_index = 0;
            let (restored, _) = load_cached_raster_with(&request, None, &mut cache, || {
                panic!("Aseprite swap restore must not decode again")
            })
            .unwrap();
            assert_eq!(restored.pixel_aspect(), expected);
            if let Some(gpu) = gpu {
                assert_eq!(
                    Some((
                        gpu.render_raster(&restored, params, [128, 96]).pixels,
                        gpu.render_raster_half(&restored, params, [128, 96])
                    )),
                    rendered
                );
            }
            if let Some(output) = rendered {
                outputs.push(output);
            }
            let mut before = Vec::new();
            let mut after = Vec::new();
            rrrah_cache::SwapPayload::write_payload(&first, &mut before).unwrap();
            rrrah_cache::SwapPayload::write_payload(&restored, &mut after).unwrap();
            assert_eq!(before, after);
            assert_eq!(cache.swap().unwrap().stats().reads, 1);
            drop(first);
            drop(restored);
            cache.swap().unwrap().shutdown();
            drop(cache);
            assert_eq!(root.used(), 0);
        }
        if gpu.is_some() {
            assert_eq!(outputs.len(), 9);
            assert_ne!(
                outputs[0].0, outputs[1].0,
                "SDR must reflect wide versus tall pixels"
            );
            assert_ne!(
                outputs[0].1, outputs[1].1,
                "float output must reflect wide versus tall pixels"
            );
            for index in 6..9 {
                assert_eq!(
                    outputs[5], outputs[index],
                    "zero/default square aspect must render identically"
                );
            }
        }
        if gpu.is_some() {
            eprintln!("ASE_ASPECT_METAL cases=9 readbacks=36 swap_reads=9 final=0");
        }
    }

    #[test]
    fn looping_animation_prefetch_warms_wrapped_first_frame_under_full_memory_pressure() {
        let fixtures = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/raster");
        for name in [
            "gif-animation-disposal-3.gif",
            "apng-compose-0-0.apng",
            "webp-animation-0.webp",
        ] {
            let root = rrrah_cache::MemoryBudget::new(1024 * 1024);
            let mut request = DecodeRequest::new(fixtures.join(name));
            request.image_index = 3;
            request.memory_budget = Some(root.clone());
            let mut cache = RasterDisplayCache::new(rrrah_cache::CacheLimits {
                max_bytes: 8192,
                max_entries: Some(4),
                ttl: None,
            });
            let (selected, _) = load_cached_raster_for_display(&request, None, &mut cache).unwrap();
            assert_ne!(selected.animation_timing().unwrap().num_plays(), 1);
            let window = gallery::PrefetchWindow { ahead: 2, behind: 1 };
            let direction = gallery::container_prefetch_direction(selected.image_count(), 3, Some(2), true);
            let plan =
                gallery::cyclic_neighbour_prefetch_indices(selected.image_count(), 3, direction, window);
            assert_eq!(plan, [0, 1, 2]);
            let gate = Arc::new(DecodeGate::new());
            for index in plan {
                let mut next = request.clone();
                next.image_index = index;
                drop(preload_raster(&next, None, &mut cache, &gate).unwrap());
            }
            let pressure = root.try_reserve(root.available_bytes()).unwrap();
            request.image_index = 0;
            let (wrapped, _) = load_cached_raster_with(&request, None, &mut cache, || {
                panic!("wrapped first frame must already be prefetched")
            })
            .unwrap();
            assert_eq!((wrapped.image_index(), wrapped.image_count()), (0, 4));
            assert_eq!(
                wrapped.animation_timing().unwrap().num_plays(),
                selected.animation_timing().unwrap().num_plays()
            );
            drop(pressure);
            drop(wrapped);
            drop(selected);
            drop(cache);
            assert_eq!(root.used(), 0);
        }
    }
    #[test]
    fn playback_clock_drives_real_animation_cache_and_swap_without_repeat_decode() {
        qualify_animation_clock_swap(None);
    }
    #[test]
    #[ignore = "requires actual Metal for animation playback clock and swap readback"]
    fn playback_clock_animation_swap_preserves_metal_sdr_and_float_output() {
        let gpu = crate::navigation_metal_readback::qualification_gpu().expect("actual Metal required");
        eprintln!("ANIMATION_CLOCK_METAL adapter={}", gpu.adapter_name());
        qualify_animation_clock_swap(Some(&gpu));
    }
    fn qualify_animation_clock_swap(gpu: Option<&crate::navigation_metal_readback::GpuReadback>) {
        let fixtures = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/raster");
        let mut states = 0;
        for (name, plays) in [
            ("gif-animation-disposal-3.gif", 3),
            ("apng-compose-0-0.apng", 0),
            ("webp-animation-0.webp", 0),
        ] {
            let parent = tempfile::tempdir().unwrap();
            let root = rrrah_cache::MemoryBudget::new(1024 * 1024);
            let mut cache = raster_cache_with_swap(
                Some(parent.path()),
                false,
                rrrah_cache::CacheLimits {
                    max_bytes: 8192,
                    max_entries: Some(1),
                    ttl: None,
                },
                Some(rrrah_cache::ImageSwapConfig {
                    limits: rrrah_cache::CacheLimits {
                        max_bytes: 65536,
                        max_entries: Some(8),
                        ttl: None,
                    },
                    queue_bytes: 8192,
                    queue_count: 4,
                    restore_bytes: 8192,
                }),
                Some(&root),
            );
            let mut request = DecodeRequest::new(fixtures.join(name));
            request.memory_budget = Some(root.clone());
            let mut clock = raster_playback::Playback::default();
            let mut generation = 1;
            let mut now = Instant::now();
            let mut decodes = 0;
            let mut expected = Vec::new();
            let params = rrrah_gpu::ViewParameters {
                viewport: [64., 48.],
                ..Default::default()
            };
            for step in 0..12 {
                let (frame, _) = load_cached_raster_with(&request, None, &mut cache, || {
                    decodes += 1;
                    load_raster_for_display_typed(&request, None)
                })
                .unwrap();
                assert_eq!((frame.image_index(), frame.image_count()), (step % 4, 4));
                if let Some(gpu) = gpu {
                    let output = (
                        gpu.render_raster(&frame, params, [64, 48]).pixels,
                        gpu.render_raster_half(&frame, params, [64, 48]),
                    );
                    if step < 4 {
                        expected.push(output);
                    } else {
                        assert_eq!(
                            output,
                            expected[step % 4],
                            "swap playback pixels changed: {name} step={step}"
                        );
                    }
                }
                let timing = frame.animation_timing().unwrap();
                assert_eq!(timing.num_plays(), plays);
                clock.ready(generation, frame.image_index(), frame.image_count(), Some(timing));
                assert!(clock.deadline(generation).is_none());
                clock.presented(generation, now);
                let deadline = clock.deadline(generation).unwrap();
                let next = clock.take_next(generation, deadline);
                if plays != 0 && step == 11 {
                    assert_eq!(next, None);
                } else {
                    assert_eq!(next, Some((step + 1) % 4));
                    request.image_index = next.unwrap();
                    generation += 1;
                    clock.submitted(generation);
                }
                now = deadline + Duration::from_secs(1);
                drop(frame);
                cache.swap().unwrap().wait_idle().unwrap();
                states += 1;
            }
            if gpu.is_some() {
                assert_eq!(expected.len(), 4);
                assert!(
                    expected.windows(2).any(|pair| pair[0].0 != pair[1].0),
                    "SDR readback must distinguish animation frames: {name}"
                );
                assert!(
                    expected.windows(2).any(|pair| pair[0].1 != pair[1].1),
                    "float readback must distinguish animation frames: {name}"
                );
            }
            assert_eq!(decodes, 4, "each unique selected frame decodes once");
            assert_eq!(cache.swap().unwrap().stats().reads, 8);
            clock.reset();
            cache.swap().unwrap().shutdown();
            drop(cache);
            assert_eq!(root.used(), 0);
        }
        assert_eq!(states, 36);
        if gpu.is_some() {
            eprintln!("ANIMATION_CLOCK_METAL states=36 decodes=12 swap_reads=24 readbacks=72 final=0");
        }
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
        assert!(first.animation_timing().is_some());
        assert_eq!(first.animation_timing(), restored.animation_timing());
        assert_eq!(first.animation_timing(), background.animation_timing());
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
            ttl: Some(Duration::from_millis(100)),
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
        assert_eq!(ram.spill_expired(), 0, "unexpired entry must survive maintenance");
        assert!(root.used() > 0);
        let deadline = Instant::now() + Duration::from_secs(3);
        receive_with_maintenance(&rx, Some(Duration::from_millis(1)), || {
            assert!(
                Instant::now() < deadline,
                "idle TTL must expire without navigation"
            );
            if ram.spill_expired() == 1 {
                tx.try_send(()).unwrap();
            }
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
mod dicom_pipeline_tests;
#[cfg(test)]
mod large_psd_pipeline_tests;

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
                region_store: None,
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
            "quality_raw: {}x{}, scene-linear RGBA32F, demosaic={}, false_color_passes={}, highlight_recovery={}, develop={:.2?}, display_curve_points={}",
            raster.width(),
            raster.height(),
            options.demosaic.as_str(),
            options.false_color_passes,
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
        eprintln!("Thumbnail completion adapter: {:?}", adapter.get_info());
        let budget = rrrah_core::MemoryBudget::new(256);
        let textures = rrrah_core::MemoryBudget::new(8);
        let (started, observed) = bounded(1);
        let (release, gate) = bounded(1);
        let inspector = drop_inspection::DropInspector::with_inspector(move |_| {
            started.send(()).unwrap();
            gate.recv_timeout(Duration::from_secs(15)).unwrap();
            Ok(drop_inspection::DropKind::File)
        });
        inspector.submit("blocked-metadata.png".into());
        observed.recv_timeout(Duration::from_secs(5)).unwrap();
        for _ in 0..2 {
            let mut strip = rrrah_gpu::FilmstripRenderer::new(
                &device,
                wgpu::TextureFormat::Rgba8UnormSrgb,
                [640.0, 480.0],
            )
            .with_texture_budget(textures.clone())
            .with_upload_queue_budget(budget.clone());
            strip.try_upload_tile(&device, &queue, 1, 1, &[255; 4]).unwrap();
            assert_eq!((textures.used(), budget.used()), (4, 256));
            assert!(strip.try_upload_tile(&device, &queue, 1, 1, &[0; 4]).is_err());
            assert_eq!((textures.used(), budget.used()), (4, 256));
            let completions = UploadCompletions::default();
            completions.track_submitted(&queue);
            drop(strip);
            // Dropping the renderer cannot retire an in-flight texture.
            assert_eq!((textures.used(), budget.used()), (4, 256));
            let deadline = Instant::now() + Duration::from_secs(10);
            while completions.poll(&device).unwrap() {
                assert!(Instant::now() < deadline);
                thread::sleep(Duration::from_millis(1));
            }
            assert_eq!((textures.used(), budget.used()), (0, 0));
            assert!(
                inspector.try_recv().unwrap().is_none(),
                "GPU completion must not wait for metadata"
            );
        }
        eprintln!(
            "thumbnail upload completion: rounds=2; pressure_refused=2; metadata_blocked=true; root=0; queue=0"
        );
        release.send(()).unwrap();
    }

    #[test]
    fn retry_keeps_managed_pixels_and_rejects_expired_or_changed_source() {
        let dir = tempfile::tempdir().unwrap();
        let cover = dir.path().join("cover.png");
        std::fs::write(&cover, [1u8; 32]).unwrap();
        let budget = rrrah_core::MemoryBudget::new(4);
        let pixels = budget.try_buffer(4, 255u8).unwrap().freeze();
        let now = Instant::now();
        let mut retry = DeferredThumbnail {
            validation: None,
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
        assert!(retry.matches_tile(std::slice::from_ref(&tile)));
        assert!(!retry.matches_tile(&[]));
        assert!(!retry.due(now));
        assert!(retry.due(now + Duration::from_millis(25)));
        assert!(!retry.expired(now + Duration::from_secs(1)));
        assert!(retry.expired(now + Duration::from_secs(2)));
        assert_eq!(budget.used(), 4);
        std::fs::write(&tile.cover, [2u8; 64]).unwrap();
        let validator = source_validation::SourceValidator::new();
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            if let Some(valid) = retry.poll_source(&validator) {
                assert!(!valid);
                break;
            }
            assert!(Instant::now() < deadline);
            thread::sleep(Duration::from_millis(1));
        }
        drop(retry);
        assert_eq!(budget.used(), 0);
    }
}

// The portable default (8192) can be lower than the selected adapter's
// capability. Request its actual 2D extent; upload byte budgets still apply.
fn viewer_device_limits(supported: &wgpu::Limits) -> wgpu::Limits {
    wgpu::Limits {
        max_texture_dimension_2d: supported.max_texture_dimension_2d,
        max_buffer_size: supported.max_buffer_size.min(2 * 1024 * 1024 * 1024),
        ..Default::default()
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

#[cfg(test)]
mod gpr_preload_tests {
    use super::*;
    #[test]
    fn prepared_cr3_invalid_source_releases_encoded_credit() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("invalid.CR3");
        let bytes = b"invalid bounded CR3 source";
        std::fs::write(&path, bytes).unwrap();
        let root = rrrah_core::MemoryBudget::new(1024);
        let mut request = DecodeRequest::new(&path);
        request.memory_budget = Some(root.clone());
        assert!(matches!(
            rrrah_decode::NativeCr3Decoder.prepare(&request),
            Err(rrrah_decode::DecodeError::NativeCr3(_))
        ));
        assert_eq!(root.used(), 0);
        assert_eq!(root.peak(), bytes.len() as u64);
        let empty = rrrah_core::MemoryBudget::new(0);
        request.memory_budget = Some(empty.clone());
        assert!(matches!(
            rrrah_decode::NativeCr3Decoder.prepare(&request),
            Err(rrrah_decode::DecodeError::Memory(_))
        ));
        assert_eq!(empty.peak(), 0);
    }

    #[test]
    #[ignore = "requires original local CR3; prepared snapshot ownership and bit oracle"]
    fn prepared_cr3_source_reuses_snapshot_and_releases_cancelled_owner() {
        let original = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/IMG_9043.CR3");
        let reference = rrrah_decode::NativeCr3Decoder
            .decode(&DecodeRequest::new(&original))
            .unwrap();
        let directory = tempfile::tempdir().unwrap();
        let copy = directory.path().join("owned.CR3");
        std::fs::copy(&original, &copy).unwrap();
        let source_bytes = std::fs::metadata(&copy).unwrap().len();
        let pixel_bytes = reference.mosaic.pixels.len() as u64 * 2;
        let root = rrrah_core::MemoryBudget::new(source_bytes + pixel_bytes);
        let mut request = DecodeRequest::new(&copy);
        request.memory_budget = Some(root.clone());
        let prepared = rrrah_decode::NativeCr3Decoder.prepare(&request).unwrap();
        assert_eq!(prepared.minimum_pixel_bytes(), pixel_bytes);
        assert_eq!(prepared.source_bytes() as u64, source_bytes);
        assert_eq!(root.used(), source_bytes, "preparation allocated sensor pixels");
        std::fs::remove_file(&copy).unwrap(); // owned temporary copy only
        let decoded = prepared.decode().unwrap();
        assert_eq!(decoded.mosaic.pixels, reference.mosaic.pixels);
        assert_eq!(decoded.mosaic.metadata, reference.mosaic.metadata);
        assert_eq!(
            root.peak(),
            source_bytes + pixel_bytes,
            "encoded source was copied during decode"
        );
        drop(decoded);
        assert_eq!(root.used(), 0);
        std::fs::copy(&original, &copy).unwrap();
        let epoch = Arc::new(AtomicU64::new(1));
        request.cancellation = Some(GenerationToken::new(epoch.clone(), 1));
        let prepared = rrrah_decode::NativeCr3Decoder.prepare(&request).unwrap();
        epoch.store(2, Ordering::Release);
        assert!(matches!(
            prepared.decode(),
            Err(rrrah_decode::DecodeError::Cancelled)
        ));
        assert_eq!(root.used(), 0);
        request.cancellation = None;
        drop(rrrah_decode::NativeCr3Decoder.prepare(&request).unwrap());
        assert_eq!(root.used(), 0);
        eprintln!(
            "PREPARED_CR3 snapshot survives temporary source removal; all sensor bits/metadata exact; source+sensor peak exact; cancel/drop final=0"
        );
    }

    #[test]
    #[ignore = "manual original CR3 foreground prefetch priority qualification"]
    fn real_cr3_prefetch_leases_keep_nearest_in_both_directions() {
        let source = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/IMG_9043.CR3");
        let dir = tempfile::tempdir().unwrap();
        let paths: Vec<_> = (0..5)
            .map(|i| {
                let path = dir.path().join(format!("{i}.CR3"));
                std::fs::copy(&source, &path).unwrap();
                std::fs::File::open(&path)
                    .unwrap()
                    .set_times(
                        std::fs::FileTimes::new()
                            .set_modified(std::time::UNIX_EPOCH + Duration::from_secs(1_700_000_000 + i)),
                    )
                    .unwrap();
                path
            })
            .collect();
        let root = rrrah_cache::MemoryBudget::new(512 * 1024 * 1024);
        let disk = DiskMosaicCache::with_max_bytes(dir.path().join("cache"), 512 * 1024 * 1024);
        let gate = Arc::new(DecodeGate::new());
        let request_for = |path: PathBuf| {
            let mut r = DecodeRequest::new(path);
            r.memory_budget = Some(root.clone());
            r
        };
        let key_for = |r: &DecodeRequest| {
            CacheKey::for_mosaic_recipe(
                &SourceFingerprint::from_path(&r.path).unwrap(),
                0,
                NativeRawDecoder.mosaic_recipe(r).unwrap(),
            )
        };
        let selected = request_for(paths[2].clone());
        let key = key_for(&selected);
        let visible = NativeRawDecoder.decode(&selected).unwrap().mosaic;
        let weight = visible.pixels.capacity_bytes();
        for direction in [gallery::NavDirection::Forward, gallery::NavDirection::Backward] {
            for limits in [
                rrrah_cache::CacheLimits {
                    max_bytes: 512 * 1024 * 1024,
                    max_entries: Some(2),
                    ttl: None,
                },
                rrrah_cache::CacheLimits {
                    max_bytes: weight * 2,
                    max_entries: None,
                    ttl: None,
                },
                rrrah_cache::CacheLimits {
                    max_bytes: weight * 2 + weight / 2,
                    max_entries: None,
                    ttl: None,
                },
            ] {
                let mut ram = MosaicRamCache::with_limits(limits);
                assert!(ram.insert_visible(key, visible.clone()));
                let bad = dir.path().join("corrupt.CR3");
                std::fs::write(&bad, b"truncated").unwrap();
                assert!(preload_raw_with_lease(&request_for(bad), &disk, &mut ram, None, 1, &gate).is_err());
                assert_eq!(ram.len(), 1);
                let planned = gallery::neighbour_prefetch_paths(
                    &paths,
                    2,
                    direction,
                    gallery::PrefetchWindow { behind: 1, ahead: 2 },
                );
                assert_eq!(planned.len(), 3);
                let nearest = request_for(planned[0].clone());
                let near_key = key_for(&nearest);
                assert_ne!(key, near_key);
                let (admitted, lease) =
                    preload_raw_with_lease(&nearest, &disk, &mut ram, None, 1, &gate).unwrap();
                assert!(admitted);
                let lease = lease.unwrap();
                assert_eq!(
                    &*lease.pixels, &*visible.pixels,
                    "prepared CR3 changed sensor samples"
                );
                assert_eq!(lease.metadata, visible.metadata);
                let before_far_peak = root.peak();
                for path in &planned[1..] {
                    let (admitted, extra) =
                        preload_raw_with_lease(&request_for(path.clone()), &disk, &mut ram, None, 1, &gate)
                            .unwrap();
                    assert!(!admitted);
                    assert!(extra.is_none());
                }
                assert_eq!(
                    root.peak(),
                    before_far_peak,
                    "protected count/byte budget allocated another RAW sensor"
                );
                let pressure = root.try_reserve(root.available_bytes()).unwrap();
                let (admitted, hit) =
                    preload_raw_with_lease(&nearest, &disk, &mut ram, None, 1, &gate).unwrap();
                assert!(admitted);
                let hit = hit.unwrap();
                assert!(hit.pixels.ptr_eq(&lease.pixels));
                assert_eq!(ram.visible(), Some(key));
                assert_eq!(ram.len(), 2);
                drop(hit);
                drop(pressure);
                drop(lease);
                let last = request_for(planned[2].clone());
                let (admitted, last_lease) =
                    preload_raw_with_lease(&last, &disk, &mut ram, None, 1, &gate).unwrap();
                assert!(admitted);
                drop(last_lease);
                assert!(ram.get_resident(&near_key).is_none());
                drop(ram);
                assert_eq!(root.used(), weight);
            }
        }
        drop(visible);
        assert_eq!(root.used(), 0);
        eprintln!(
            "RAW scoped prefetch: original CR3; both directions; count/bytes; failed neighbor; nearest retained; release admits farther; final=0"
        );
    }

    #[test]
    #[ignore = "requires pinned HERO9 GPR"]
    fn gpr_preload_hits_ram_and_disk_and_cancels_stale_generation() {
        let path = PathBuf::from(std::env::var("RRRAH_GPR_SOURCE").expect("RRRAH_GPR_SOURCE"));
        assert!(rrrah_decode::is_supported_raw_path(&path));
        assert!(rrrah_decode::is_supported_image_path(&path));
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

    #[test]
    #[ignore = "requires pinned HERO9 GPR"]
    fn gpr_neighbour_windows_preserve_visible_and_bound_count_and_bytes() {
        let source = std::env::var("RRRAH_GPR_SOURCE").expect("RRRAH_GPR_SOURCE");
        let dir = tempfile::tempdir().unwrap();
        let paths: Vec<_> = (0..5)
            .map(|i| {
                let path = dir.path().join(format!("{i}.GPR"));
                std::fs::copy(&source, &path).unwrap();
                std::fs::File::open(&path)
                    .unwrap()
                    .set_times(
                        std::fs::FileTimes::new()
                            .set_modified(std::time::UNIX_EPOCH + Duration::from_secs(1_700_000_000 + i)),
                    )
                    .unwrap();
                path
            })
            .collect();
        let root = rrrah_cache::MemoryBudget::new(256 * 1024 * 1024);
        let disk = DiskMosaicCache::with_max_bytes(dir.path().join("cache"), 256 * 1024 * 1024);
        let gate = Arc::new(DecodeGate::new());
        let request_for = |path: PathBuf| {
            let mut request = DecodeRequest::new(path);
            request.memory_budget = Some(root.clone());
            request
        };
        let key_for = |request: &DecodeRequest| {
            CacheKey::for_mosaic_recipe(
                &SourceFingerprint::from_path(&request.path).unwrap(),
                0,
                NativeRawDecoder.mosaic_recipe(request).unwrap(),
            )
        };
        let visible_request = request_for(paths[2].clone());
        let visible_key = key_for(&visible_request);
        let visible = NativeRawDecoder.decode(&visible_request).unwrap().mosaic;
        let sensor_bytes = visible.pixels.capacity_bytes();
        let mut ram = MosaicRamCache::with_limits(rrrah_cache::CacheLimits {
            max_bytes: 2 * sensor_bytes,
            max_entries: Some(2),
            ttl: None,
        });
        assert!(ram.insert_visible(visible_key, visible.clone()));
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
                assert!(rrrah_decode::is_supported_raw_path(&path));
                let request = request_for(path);
                let key = key_for(&request);
                assert!(preload_raw(&request, &disk, &mut ram, None, 1, &gate).unwrap());
                let warmed = ram.get(&key).unwrap();
                let blocker = root.try_reserve(root.available_bytes()).unwrap();
                assert!(preload_raw(&request, &disk, &mut ram, None, 1, &gate).unwrap());
                let hit = ram.get(&key).unwrap();
                assert!(hit.pixels.ptr_eq(&warmed.pixels));
                drop(blocker);
                assert_eq!(ram.len(), 2);
                assert_eq!(ram.visible(), Some(visible_key));
                assert!(ram.get(&visible_key).unwrap().pixels.ptr_eq(&visible.pixels));
                drop(hit);
                drop(warmed);
                assert_eq!(root.used(), 2 * sensor_bytes);
            }
        }
        drop(visible);
        drop(ram);
        assert_eq!(root.used(), 0);
    }
}

#[cfg(test)]
mod gpr_development_tests {
    use super::*;
    #[test]
    #[ignore = "requires pinned HERO9 GPR"]
    fn gpr_opcodes_trigger_default_quality_development() {
        let budget = rrrah_core::MemoryBudget::new(1024 * 1024 * 1024);
        let mut request = DecodeRequest::new(std::env::var("RRRAH_GPR_SOURCE").unwrap());
        request.memory_budget = Some(budget.clone());
        let mosaic = NativeRawDecoder.decode(&request).unwrap().mosaic;
        let (raster, _) = prepare_quality_raw(&mosaic, &request, None)
            .unwrap()
            .expect("GPR corrections must trigger development");
        assert_eq!((raster.width(), raster.height()), (5568, 4176));
        assert!(budget.peak() <= budget.limit());
        drop(raster);
        drop(mosaic);
        assert_eq!(budget.used(), 0);
    }

    #[test]
    #[ignore = "requires pinned HERO9 GPR"]
    fn navigation_cancels_gpr_quality_development_after_float_admission() {
        let budget = rrrah_core::MemoryBudget::new(1024 * 1024 * 1024);
        let generation = Arc::new(AtomicU64::new(1));
        let mut request = DecodeRequest::new(std::env::var("RRRAH_GPR_SOURCE").unwrap());
        request.memory_budget = Some(budget.clone());
        let mosaic = NativeRawDecoder.decode(&request).unwrap().mosaic;
        request.cancellation = Some(GenerationToken::new(generation.clone(), 1));
        let worker = std::thread::spawn(move || prepare_quality_raw(&mosaic, &request, None));
        let deadline = std::time::Instant::now() + Duration::from_secs(30);
        // The decoded sensor and compressed source peak are below this bound.
        // Observe live retained bytes, proving float development admission.
        while budget.used() <= 256 * 1024 * 1024 && !worker.is_finished() {
            assert!(std::time::Instant::now() < deadline, "float admission timed out");
            std::thread::yield_now();
        }
        assert!(budget.used() > 256 * 1024 * 1024);
        generation.store(2, Ordering::Release);
        let error = worker.join().unwrap().unwrap_err();
        assert!(error.to_lowercase().contains("cancel"), "{error}");
        assert_eq!(budget.used(), 0);
        let small = rrrah_core::MemoryBudget::new(128 * 1024 * 1024);
        let mut retry = DecodeRequest::new(std::env::var("RRRAH_GPR_SOURCE").unwrap());
        retry.memory_budget = Some(small.clone());
        let retained = NativeRawDecoder.decode(&retry).unwrap().mosaic;
        let sensor_bytes = retained.pixels.capacity_bytes();
        assert!(prepare_quality_raw(&retained, &retry, None).is_err());
        assert_eq!(small.used(), sensor_bytes);
        assert_eq!(retained.pixels.len(), 23_251_968);
        drop(retained);
        assert_eq!(small.used(), 0);
    }
}

#[cfg(test)]
mod fusion_warp_tests {
    use super::*;
    #[test]
    #[ignore = "requires pinned official Fusion GPR"]
    fn fusion_warp_triggers_quality_and_changes_corrected_channels() {
        let path = PathBuf::from(std::env::var("RRRAH_GPR_CORPUS").unwrap()).join("Fusion-back.GPR");
        let budget = rrrah_core::MemoryBudget::new(512 * 1024 * 1024);
        let mut request = DecodeRequest::new(path);
        request.memory_budget = Some(budget.clone());
        let mosaic = NativeRawDecoder.decode(&request).unwrap().mosaic;
        let lists = rrrah_decode::raw_development_opcodes(&request).unwrap();
        assert_eq!(lists.list3.len(), 1);
        let (corrected, _) = prepare_quality_raw(&mosaic, &request, None)
            .unwrap()
            .expect("warp must trigger quality");
        let plain = rrrah_core::develop::develop_raw(
            &mosaic,
            &Default::default(),
            &Default::default(),
            Some(&budget),
            &|| false,
        )
        .unwrap();
        assert_eq!((corrected.width(), corrected.height()), (3104, 3000));
        let (rrrah_core::RasterPixels::Rgba32Float(a), rrrah_core::RasterPixels::Rgba32Float(b)) =
            (corrected.pixels(), plain.pixels())
        else {
            panic!()
        };
        assert!(a.iter().zip(b.iter()).any(|(x, y)| x.to_bits() != y.to_bits()));
        drop(corrected);
        drop(plain);
        drop(mosaic);
        assert_eq!(budget.used(), 0);
    }
}

#[cfg(test)]
mod eps_preload_tests {
    use super::*;
    const EPS: &[u8] = b"%!PS-Adobe-3.0 EPSF-3.0\n%%BoundingBox: 0 0 4 4\n%%EndComments\n1 0 0 setrgbcolor 0 0 moveto 4 0 lineto 4 4 lineto 0 4 lineto closepath fill\n";
    #[test]
    fn expired_eps_preload_redecodes_without_invalidating_external_frame() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("expired.eps");
        std::fs::write(&path, EPS).unwrap();
        for (max_bytes, max_entries) in [(256, None), (4096, Some(1))] {
            let root = rrrah_core::MemoryBudget::new(32 * 1024 * 1024);
            let mut request = DecodeRequest::new(&path);
            request.memory_budget = Some(root.clone());
            let mut cache = RasterDisplayCache::new(rrrah_cache::CacheLimits {
                max_bytes,
                max_entries,
                ttl: Some(std::time::Duration::ZERO),
            });
            let gate = Arc::new(DecodeGate::new());
            let (warmed, _) = preload_raster(&request, None, &mut cache, &gate).unwrap();
            let mut loads = 0;
            let (fresh, _) = load_cached_raster_mode(&request, None, &mut cache, false, || {
                loads += 1;
                load_raster_for_display_typed(&request, None)
            })
            .unwrap();
            assert_eq!(loads, 1, "expired preload must not produce a cache hit");
            let (rrrah_core::RasterPixels::Rgba32Float(a), rrrah_core::RasterPixels::Rgba32Float(b)) =
                (warmed.pixels(), fresh.pixels())
            else {
                panic!()
            };
            assert!(!a.ptr_eq(b));
            assert_eq!(&a[..], &b[..]);
            drop(cache);
            assert_eq!(root.used(), 512, "both external frames retain their own credit");
            drop(fresh);
            assert_eq!(root.used(), 256);
            drop(warmed);
            assert_eq!(root.used(), 0);
        }
    }
    #[test]
    fn eps_neighbour_windows_reuse_preload_preserve_visible_and_cancel_stale() {
        let dir = tempfile::tempdir().unwrap();
        let paths: Vec<_> = (0..5)
            .map(|i| {
                let path = dir.path().join(format!("{i}.eps"));
                std::fs::write(&path, EPS).unwrap();
                path
            })
            .collect();
        for limits in [
            rrrah_cache::CacheLimits {
                max_bytes: 512,
                max_entries: None,
                ttl: None,
            },
            rrrah_cache::CacheLimits {
                max_bytes: 4096,
                max_entries: Some(2),
                ttl: None,
            },
        ] {
            let root = rrrah_core::MemoryBudget::new(32 * 1024 * 1024);
            let request_for = |path: PathBuf| {
                let mut request = DecodeRequest::new(path);
                request.memory_budget = Some(root.clone());
                request
            };
            let mut cache = RasterDisplayCache::new(limits);
            let gate = Arc::new(DecodeGate::new());
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
                        panic!("EPS preload decoded again")
                    })
                    .unwrap();
                    let (rrrah_core::RasterPixels::Rgba32Float(a), rrrah_core::RasterPixels::Rgba32Float(b)) =
                        (warmed.pixels(), hit.pixels())
                    else {
                        panic!()
                    };
                    assert!(a.ptr_eq(b));
                    assert_eq!(cache.len(), 2);
                    assert_eq!(cache.resident_bytes(), 512);
                    drop(hit);
                    drop(warmed);
                    drop(
                        load_cached_raster_with(&visible, None, &mut cache, || {
                            panic!("visible EPS displaced")
                        })
                        .unwrap(),
                    );
                }
            }
            let generation = Arc::new(std::sync::atomic::AtomicU64::new(1));
            let mut stale = request_for(paths[0].clone());
            stale.cancellation = Some(rrrah_decode::GenerationToken::new(generation.clone(), 1));
            generation.store(2, std::sync::atomic::Ordering::Release);
            let used = root.used();
            assert!(preload_raster(&stale, None, &mut cache, &gate).is_err());
            assert_eq!(root.used(), used);
            drop(frame);
            drop(cache);
            assert_eq!(root.used(), 0);
        }
    }
    #[test]
    fn eps_source_vm_and_graphics_admission_errors_are_retryable() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("pressure.eps");
        std::fs::write(&path, EPS).unwrap();
        for limit in [1, 1024, 65536, 1024 * 1024] {
            let root = rrrah_core::MemoryBudget::new(32 * 1024 * 1024);
            let held = root.try_reserve(root.limit() - limit).unwrap();
            let mut request = DecodeRequest::new(&path);
            request.memory_budget = Some(root.clone());
            let error = load_raster_for_display_typed(&request, None).unwrap_err();
            assert!(error.retryable_memory_pressure(), "available {limit}: {error}");
            drop(held);
            assert_eq!(root.used(), 0);
        }
    }
}

#[cfg(test)]
mod external_collection_qualification;
