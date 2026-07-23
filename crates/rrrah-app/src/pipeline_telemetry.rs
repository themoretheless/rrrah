//! Generation-bound, route-specific telemetry for the load-to-display HUD.
//!
//! Numeric durations are used only for boundaries owned by rrrah. Internal
//! parser/entropy stages share their measured enclosing span, conditional
//! branches are labelled as such, and fused GPU shader work is never assigned
//! fabricated CPU time.

use std::{path::Path, time::Duration};

use rrrah_gpu::GpuUploadTimings;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RawKind {
    Cr2,
    Cr3,
    Dng,
    Unknown,
}

impl RawKind {
    pub fn from_path(path: &Path) -> Self {
        match path
            .extension()
            .and_then(|extension| extension.to_str())
            .map(str::to_ascii_lowercase)
            .as_deref()
        {
            Some("cr2") => Self::Cr2,
            Some("cr3") => Self::Cr3,
            Some("dng") => Self::Dng,
            _ => Self::Unknown,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CacheRoute {
    Hit,
    Miss,
    Disabled,
    ErrorFallback,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct FrontendTimings {
    pub queue_wait: Duration,
    pub fingerprint: Option<Duration>,
    /// Cache-key derivation and `cache.load`; exact for hit, miss, and error.
    pub cache_lookup: Option<Duration>,
    pub admission: Option<Duration>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct FrameSubmitTimings {
    pub surface_acquire: Duration,
    pub frame_encode: Duration,
    pub queue_submit: Duration,
    pub present_request: Duration,
    pub total: Duration,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PipelineStageState {
    Pending,
    Running,
    Measured(Duration),
    /// The algorithm ran inside one enclosing parser/decode span. This is not an
    /// independently measured duration.
    Shared(Duration),
    /// The enclosing span is known, but the backend does not report whether this
    /// format-dependent branch was taken for the current file.
    Conditional(Duration),
    NotTimed,
    Skipped,
    Failed,
}

impl PipelineStageState {
    fn timing_text(self) -> String {
        match self {
            Self::Pending => "--".into(),
            Self::Running => "RUNNING".into(),
            Self::Measured(duration) => format_duration(duration),
            Self::Shared(duration) => format_enclosing_duration("IN", duration),
            Self::Conditional(duration) => format_enclosing_duration("IF", duration),
            Self::NotTimed => "NOT TIMED".into(),
            Self::Skipped => "SKIPPED".into(),
            Self::Failed => "FAILED".into(),
        }
    }

    const fn is_resolved(self) -> bool {
        matches!(
            self,
            Self::Measured(_) | Self::Shared(_) | Self::Conditional(_) | Self::NotTimed | Self::Skipped
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PipelineCard {
    pub title: String,
    pub time: String,
    pub status: String,
    pub description: &'static str,
    pub state: PipelineStageState,
    pub cache_footer: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum StageId {
    Idle,
    Queue,
    Fingerprint,
    CacheLookup,
    Admission,
    Source,
    DecoderSelect,
    RawImageSpan,
    Format1,
    Format2,
    Format3,
    Format4,
    Format5,
    Format6,
    Format7,
    Format8,
    Format9,
    Format10,
    AdaptLayout,
    AdaptLevels,
    AdaptColor,
    AdaptGeometry,
    AdaptFinalize,
    UiDispatch,
    GpuValidate,
    GpuAtlasPlan,
    GpuTextureAllocate,
    GpuHaloPack,
    GpuRowPack,
    GpuTextureWrite,
    GpuUniformWrite,
    GpuBind,
    GpuViewUniform,
    ShaderView,
    ShaderNormalize,
    ShaderDemosaic,
    ShaderColorTone,
    FrameAcquire,
    FrameEncode,
    FrameSubmit,
    FramePresent,
    Total,
}

const FORMAT_STAGES: [StageId; 10] = [
    StageId::Format1,
    StageId::Format2,
    StageId::Format3,
    StageId::Format4,
    StageId::Format5,
    StageId::Format6,
    StageId::Format7,
    StageId::Format8,
    StageId::Format9,
    StageId::Format10,
];

const SHADER_STAGES: [StageId; 4] = [
    StageId::ShaderView,
    StageId::ShaderNormalize,
    StageId::ShaderDemosaic,
    StageId::ShaderColorTone,
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Stage {
    id: StageId,
    state: PipelineStageState,
}

impl Stage {
    const fn pending(id: StageId) -> Self {
        Self {
            id,
            state: PipelineStageState::Pending,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PipelineSnapshot {
    generation: u64,
    raw_kind: RawKind,
    cache_route: Option<CacheRoute>,
    stages: Vec<Stage>,
    frame_submit_total: Option<Duration>,
    request_total: Option<Duration>,
    native_worker_count: Option<u8>,
    native_plane_wall: Option<Duration>,
    failed_stage: Option<StageId>,
    active: bool,
}

impl PipelineSnapshot {
    pub fn idle(generation: u64) -> Self {
        Self {
            generation,
            raw_kind: RawKind::Unknown,
            cache_route: None,
            stages: vec![Stage::pending(StageId::Idle)],
            frame_submit_total: None,
            request_total: None,
            native_worker_count: None,
            native_plane_wall: None,
            failed_stage: None,
            active: false,
        }
    }

    pub fn waiting(generation: u64, raw_kind: RawKind) -> Self {
        let mut snapshot = Self::decode_route(generation, raw_kind, None);
        snapshot.set(StageId::Queue, PipelineStageState::Running);
        snapshot
    }

    pub fn from_frontend(
        generation: u64,
        raw_kind: RawKind,
        cache_route: Option<CacheRoute>,
        timings: FrontendTimings,
    ) -> Self {
        let mut snapshot = if cache_route == Some(CacheRoute::Hit) {
            Self::cache_hit_route(generation, raw_kind)
        } else {
            Self::decode_route(generation, raw_kind, cache_route)
        };
        snapshot.apply_frontend(cache_route, timings);
        snapshot
    }

    pub fn from_worker(
        generation: u64,
        raw_kind: RawKind,
        cache_route: CacheRoute,
        frontend: FrontendTimings,
        decode: Option<&rrrah_decode::DecodeTimings>,
        ui_dispatch: Duration,
    ) -> Self {
        let mut snapshot = if cache_route == CacheRoute::Hit {
            Self::cache_hit_route(generation, raw_kind)
        } else {
            Self::decode_route(generation, raw_kind, Some(cache_route))
        };
        snapshot.apply_frontend(Some(cache_route), frontend);
        snapshot.set(StageId::UiDispatch, PipelineStageState::Measured(ui_dispatch));

        if cache_route == CacheRoute::Hit {
            return snapshot;
        }

        if let Some(timings) = decode {
            if let Some(native) = timings.native {
                snapshot.native_worker_count = Some(native.worker_count);
                snapshot.native_plane_wall = Some(native.plane_wall);
            }
            snapshot.set(StageId::Source, PipelineStageState::Measured(timings.source_open));
            snapshot.set(
                StageId::DecoderSelect,
                PipelineStageState::Measured(timings.decoder_select),
            );
            snapshot.set(
                StageId::RawImageSpan,
                PipelineStageState::Measured(timings.raw_image),
            );
            for (index, id) in FORMAT_STAGES.into_iter().enumerate() {
                if raw_kind == RawKind::Cr3
                    && let Some(native) = timings.native
                {
                    let exact = match index {
                        5..=8 => Some(native.plane_decode[index - 5]),
                        9 => Some(native.interleave),
                        _ => None,
                    };
                    if let Some(duration) = exact {
                        snapshot.set(id, PipelineStageState::Measured(duration));
                        continue;
                    }
                }
                let enclosing = if raw_kind == RawKind::Cr3 && index < 5 {
                    timings.decoder_select
                } else {
                    timings.raw_image
                };
                let state = if format_stage_copy(raw_kind, index).conditional {
                    PipelineStageState::Conditional(enclosing)
                } else {
                    PipelineStageState::Shared(enclosing)
                };
                snapshot.set(id, state);
            }
            snapshot.set(
                StageId::AdaptLayout,
                PipelineStageState::Measured(timings.adapt.layout_cfa),
            );
            snapshot.set(
                StageId::AdaptLevels,
                PipelineStageState::Measured(timings.adapt.levels),
            );
            snapshot.set(
                StageId::AdaptColor,
                PipelineStageState::Measured(timings.adapt.color),
            );
            snapshot.set(
                StageId::AdaptGeometry,
                PipelineStageState::Measured(timings.adapt.geometry),
            );
            snapshot.set(
                StageId::AdaptFinalize,
                PipelineStageState::Measured(timings.adapt.finalize),
            );
        } else {
            snapshot.set(StageId::Source, PipelineStageState::Running);
        }
        snapshot
    }

    pub const fn generation(&self) -> u64 {
        self.generation
    }

    /// Records CPU preparation and GPU-copy enqueue timings. It does not claim
    /// completion of the enqueued texture writes.
    pub fn complete_gpu(&mut self, timings: GpuUploadTimings, view_uniform: Duration) -> bool {
        let Some(gpu_index) = self.index_of(StageId::GpuValidate) else {
            return false;
        };
        if self.failed_stage.is_some()
            || self.stages[gpu_index].state != PipelineStageState::Pending
            || !self.stages[..gpu_index]
                .iter()
                .all(|stage| stage.state.is_resolved())
        {
            return false;
        }

        self.set(
            StageId::GpuValidate,
            PipelineStageState::Measured(timings.validate),
        );
        self.set(
            StageId::GpuAtlasPlan,
            PipelineStageState::Measured(timings.atlas_plan),
        );
        self.set(
            StageId::GpuTextureAllocate,
            PipelineStageState::Measured(timings.texture_allocate),
        );
        self.set(
            StageId::GpuHaloPack,
            PipelineStageState::Measured(timings.halo_pack),
        );
        self.set(
            StageId::GpuRowPack,
            PipelineStageState::Measured(timings.row_pack),
        );
        self.set(
            StageId::GpuTextureWrite,
            PipelineStageState::Measured(timings.texture_write_enqueue),
        );
        self.set(
            StageId::GpuUniformWrite,
            PipelineStageState::Measured(timings.uniform_write),
        );
        self.set(StageId::GpuBind, PipelineStageState::Measured(timings.bind));
        self.set(
            StageId::GpuViewUniform,
            PipelineStageState::Measured(view_uniform),
        );
        for id in SHADER_STAGES {
            self.set(id, PipelineStageState::NotTimed);
        }
        self.set(StageId::FrameAcquire, PipelineStageState::Running);
        true
    }

    /// Latches the first successful CPU present request. `total` is measured
    /// from request enqueue to that API call, not to GPU completion or scanout.
    pub fn complete_display(&mut self, timings: FrameSubmitTimings, total: Duration) -> bool {
        if self.failed_stage.is_some()
            || self.request_total.is_some()
            || self
                .state(StageId::GpuViewUniform)
                .is_none_or(|state| !state.is_resolved())
            || self.state(StageId::FrameAcquire) != Some(PipelineStageState::Running)
            || self.state(StageId::Total) != Some(PipelineStageState::Pending)
        {
            return false;
        }
        self.set(
            StageId::FrameAcquire,
            PipelineStageState::Measured(timings.surface_acquire),
        );
        self.set(
            StageId::FrameEncode,
            PipelineStageState::Measured(timings.frame_encode),
        );
        self.set(
            StageId::FrameSubmit,
            PipelineStageState::Measured(timings.queue_submit),
        );
        self.set(
            StageId::FramePresent,
            PipelineStageState::Measured(timings.present_request),
        );
        self.set(StageId::Total, PipelineStageState::Measured(total));
        self.frame_submit_total = Some(timings.total);
        self.request_total = Some(total);
        self.active = false;
        true
    }

    pub fn mark_failed(&mut self) {
        self.active = false;
        if self.failed_stage.is_some() {
            return;
        }
        let failed_index = self
            .stages
            .iter()
            .position(|stage| stage.state == PipelineStageState::Running)
            .or_else(|| {
                self.stages
                    .iter()
                    .position(|stage| stage.state == PipelineStageState::Pending)
            });
        let Some(failed_index) = failed_index else {
            return;
        };
        self.failed_stage = Some(self.stages[failed_index].id);
        self.stages[failed_index].state = PipelineStageState::Failed;
        for stage in &mut self.stages[failed_index + 1..] {
            if matches!(
                stage.state,
                PipelineStageState::Pending | PipelineStageState::Running
            ) {
                stage.state = PipelineStageState::Skipped;
            }
        }
    }

    pub fn cards(&self) -> Vec<PipelineCard> {
        self.stages
            .iter()
            .enumerate()
            .map(|(index, stage)| self.card(index, *stage))
            .collect()
    }

    fn apply_frontend(&mut self, cache_route: Option<CacheRoute>, timings: FrontendTimings) {
        self.set(StageId::Queue, PipelineStageState::Measured(timings.queue_wait));
        if cache_route == Some(CacheRoute::Disabled) {
            self.set(StageId::Fingerprint, PipelineStageState::Skipped);
            self.set(StageId::CacheLookup, PipelineStageState::Skipped);
        } else {
            self.set(
                StageId::Fingerprint,
                timings.fingerprint.map_or_else(
                    || {
                        if cache_route.is_none() {
                            PipelineStageState::Running
                        } else {
                            PipelineStageState::NotTimed
                        }
                    },
                    PipelineStageState::Measured,
                ),
            );
            self.set(
                StageId::CacheLookup,
                timings.cache_lookup.map_or_else(
                    || {
                        if cache_route.is_none() && timings.fingerprint.is_some() {
                            PipelineStageState::Running
                        } else if cache_route.is_none() {
                            PipelineStageState::Pending
                        } else {
                            PipelineStageState::NotTimed
                        }
                    },
                    PipelineStageState::Measured,
                ),
            );
        }

        if self.index_of(StageId::Admission).is_some() {
            if let Some(duration) = timings.admission {
                self.set(StageId::Admission, PipelineStageState::Measured(duration));
                self.set(StageId::Source, PipelineStageState::Running);
            } else if cache_route.is_some() {
                self.set(StageId::Admission, PipelineStageState::Running);
            }
        }
    }

    fn decode_route(generation: u64, raw_kind: RawKind, cache_route: Option<CacheRoute>) -> Self {
        let ids = [
            StageId::Queue,
            StageId::Fingerprint,
            StageId::CacheLookup,
            StageId::Admission,
            StageId::Source,
            StageId::DecoderSelect,
            StageId::RawImageSpan,
            StageId::Format1,
            StageId::Format2,
            StageId::Format3,
            StageId::Format4,
            StageId::Format5,
            StageId::Format6,
            StageId::Format7,
            StageId::Format8,
            StageId::Format9,
            StageId::Format10,
            StageId::AdaptLayout,
            StageId::AdaptLevels,
            StageId::AdaptColor,
            StageId::AdaptGeometry,
            StageId::AdaptFinalize,
            StageId::UiDispatch,
            StageId::GpuValidate,
            StageId::GpuAtlasPlan,
            StageId::GpuTextureAllocate,
            StageId::GpuHaloPack,
            StageId::GpuRowPack,
            StageId::GpuTextureWrite,
            StageId::GpuUniformWrite,
            StageId::GpuBind,
            StageId::GpuViewUniform,
            StageId::ShaderView,
            StageId::ShaderNormalize,
            StageId::ShaderDemosaic,
            StageId::ShaderColorTone,
            StageId::FrameAcquire,
            StageId::FrameEncode,
            StageId::FrameSubmit,
            StageId::FramePresent,
            StageId::Total,
        ];
        Self::with_stages(generation, raw_kind, cache_route, &ids)
    }

    fn cache_hit_route(generation: u64, raw_kind: RawKind) -> Self {
        let ids = [
            StageId::Queue,
            StageId::Fingerprint,
            StageId::CacheLookup,
            StageId::UiDispatch,
            StageId::GpuValidate,
            StageId::GpuAtlasPlan,
            StageId::GpuTextureAllocate,
            StageId::GpuHaloPack,
            StageId::GpuRowPack,
            StageId::GpuTextureWrite,
            StageId::GpuUniformWrite,
            StageId::GpuBind,
            StageId::GpuViewUniform,
            StageId::ShaderView,
            StageId::ShaderNormalize,
            StageId::ShaderDemosaic,
            StageId::ShaderColorTone,
            StageId::FrameAcquire,
            StageId::FrameEncode,
            StageId::FrameSubmit,
            StageId::FramePresent,
            StageId::Total,
        ];
        Self::with_stages(generation, raw_kind, Some(CacheRoute::Hit), &ids)
    }

    fn with_stages(
        generation: u64,
        raw_kind: RawKind,
        cache_route: Option<CacheRoute>,
        ids: &[StageId],
    ) -> Self {
        Self {
            generation,
            raw_kind,
            cache_route,
            stages: ids.iter().copied().map(Stage::pending).collect(),
            frame_submit_total: None,
            request_total: None,
            native_worker_count: None,
            native_plane_wall: None,
            failed_stage: None,
            active: true,
        }
    }

    fn card(&self, index: usize, stage: Stage) -> PipelineCard {
        let copy = self.stage_copy(stage.id);
        PipelineCard {
            title: format!("{:02} {}", index + 1, copy.label),
            time: stage.state.timing_text(),
            status: self.status(stage),
            description: copy.description,
            state: stage.state,
            cache_footer: stage.id == StageId::CacheLookup,
        }
    }

    fn stage_copy(&self, id: StageId) -> StageCopy {
        match id {
            StageId::Idle => copy(
                "PIPELINE",
                "DROP A CANON EOS R8 CR3 TO TRACE THE COMPLETE NATIVE LOAD-TO-SUBMIT ROUTE",
            ),
            StageId::Queue => copy(
                "REQUEST QUEUE",
                "GENERATION, LATEST-WINS QUEUE AND FOREGROUND WORKER DEQUEUE",
            ),
            StageId::Fingerprint => copy(
                "SOURCE FINGERPRINT",
                "STAT FILE, SAMPLE SOURCE RANGES AND HASH SIZE, MTIME AND BYTES WITH BLAKE3",
            ),
            StageId::CacheLookup if self.cache_route == Some(CacheRoute::Hit) => copy(
                "CACHE MATERIALIZE",
                "DERIVE MOSAIC KEY; READ AND VERIFY HEADER, CHECKSUM AND U16 PAYLOAD; REBUILD MOSAIC",
            ),
            StageId::CacheLookup if self.cache_route == Some(CacheRoute::Disabled) => copy(
                "CACHE BYPASS",
                "--NO-CACHE SKIPS FINGERPRINT, KEY DERIVATION, LOOKUP AND ASYNC WRITE-BACK",
            ),
            StageId::CacheLookup => copy(
                "CACHE KEY + LOOKUP",
                "DERIVE THE VERSIONED MOSAIC KEY AND LOOK UP THE VERIFIED DECODED-MOSAIC OBJECT",
            ),
            StageId::Admission => copy(
                "DECODE ADMISSION",
                "WAIT FOR FOREGROUND DECODE PERMIT AND RECHECK GENERATION CANCELLATION",
            ),
            StageId::Source => copy(
                "SOURCE READ",
                "OPEN THE CR3 AND READ ITS BYTES INTO THE BOUNDED NATIVE DECODER INPUT BUFFER",
            ),
            StageId::DecoderSelect => copy(
                "CR3 PARSE + SELECT",
                "VALIDATE BMFF, SELECT THE FULL-RESOLUTION CRX SAMPLE AND EXTRACT CMT/CTMD METADATA",
            ),
            StageId::RawImageSpan => copy(
                "NATIVE CRX DECODE",
                "STREAM FOUR LOSSLESS CRX PLANES IN 32-ROW BATCHES AND ASSEMBLE THE FULL-SENSOR U16 CFA",
            ),
            StageId::Format1
            | StageId::Format2
            | StageId::Format3
            | StageId::Format4
            | StageId::Format5
            | StageId::Format6
            | StageId::Format7
            | StageId::Format8
            | StageId::Format9
            | StageId::Format10 => format_stage_copy(self.raw_kind, format_stage_index(id).unwrap_or(0)),
            StageId::AdaptLayout => copy(
                "ADAPT LAYOUT + CFA",
                "MAP DIMENSIONS, CPP/BPS, PHOTOMETRIC TYPE AND CFA GRID INTO RRRAH TYPES",
            ),
            StageId::AdaptLevels => copy(
                "ADAPT SENSOR LEVELS",
                "COPY THE EOS R8 BLACK-LEVEL GRID AND WHITE LEVEL INTO VALIDATED FINITE F32 VALUES",
            ),
            StageId::AdaptColor => copy(
                "ADAPT COLOR DATA",
                "CONVERT EXACT CTMD WHITE-BALANCE RATIOS AND APPLY THE EOS R8 CAMERA MATRIX",
            ),
            StageId::AdaptGeometry => copy(
                "ADAPT RAW GEOMETRY",
                "CONVERT ACTIVE AREA, CROP RECTANGLE AND EXIF ORIENTATION TO SENSOR COORDINATES",
            ),
            StageId::AdaptFinalize => copy(
                "FINALIZE MOSAIC",
                "MOVE U16 PIXELS INTO ARC, BUILD RAWMETADATA AND VALIDATE THE COMPLETE DECODEDMOSAIC",
            ),
            StageId::UiDispatch => copy(
                "WORKER -> UI",
                "CHANNEL SEND, EVENT-LOOP WAKEUP AND GENERATION-CHECKED READY EVENT DISPATCH",
            ),
            StageId::GpuValidate => copy(
                "GPU INPUT VALIDATE",
                "REQUIRE BAYER U16, RESOLVE CFA AND LEVEL QUADS, CAMERA MATRIX AND EFFECTIVE CROP",
            ),
            StageId::GpuAtlasPlan => copy(
                "GPU ATLAS PLAN",
                "CHOOSE TILE SIZE, GRID, ARRAY LAYERS, HALO AND BOUNDED R16UINT ATLAS SIZE",
            ),
            StageId::GpuTextureAllocate => copy(
                "GPU TEXTURE ALLOC",
                "CREATE THE R16UINT 2D-ARRAY TEXTURE FOR ALL FULL-SENSOR MOSAIC TILES",
            ),
            StageId::GpuHaloPack => copy(
                "GPU HALO PACK",
                "COPY EACH SENSOR TILE WITH A CLAMPED 1-PIXEL NEIGHBOR HALO FOR DEMOSAIC EDGES",
            ),
            StageId::GpuRowPack => copy(
                "GPU ROW PACK",
                "ENCODE U16 LITTLE-ENDIAN ROWS AND PAD BYTES-PER-ROW TO WEBGPU ALIGNMENT",
            ),
            StageId::GpuTextureWrite => copy(
                "GPU TEXTURE ENQUEUE",
                "QUEUE WRITE_TEXTURE FOR EVERY ARRAY LAYER; CPU ENQUEUE TIME, NOT GPU COPY COMPLETION",
            ),
            StageId::GpuUniformWrite => copy(
                "GPU RAW UNIFORMS",
                "ENQUEUE RAW SIZE, TILE GRID, CFA, LEVELS, WB, MATRIX, CROP AND ORIENTATION UNIFORMS",
            ),
            StageId::GpuBind => copy(
                "GPU VIEW + BIND",
                "CREATE THE D2-ARRAY TEXTURE VIEW AND BIND IT WITH THE RAW PARAMETER BUFFER",
            ),
            StageId::GpuViewUniform => copy(
                "GPU VIEW UNIFORM",
                "ENQUEUE VIEWPORT, PAN, ZOOM AND EXPOSURE CONTROLS FOR THE FIRST FRAME",
            ),
            StageId::ShaderView => copy(
                "SHADER VIEW GEOMETRY",
                "FIT VIEWPORT, APPLY PAN/ZOOM, CROP, INVERSE ORIENTATION AND RAW COORDINATE MAP",
            ),
            StageId::ShaderNormalize => copy(
                "SHADER SENSOR SAMPLE",
                "ADDRESS ATLAS LAYER AND HALO, RESOLVE CFA PHASE, SUBTRACT BLACK AND NORMALIZE WHITE",
            ),
            StageId::ShaderDemosaic => copy(
                "SHADER DEMOSAIC",
                "BILINEAR 3X3 BAYER RECONSTRUCTION; USE FOUR STRATIFIED SAMPLES WHEN MINIFYING",
            ),
            StageId::ShaderColorTone => copy(
                "SHADER COLOR + TONE",
                "APPLY WB, CAMERA-TO-RGB MATRIX, EXP2 EXPOSURE AND FITTED ACES TONE MAP",
            ),
            StageId::FrameAcquire => copy(
                "SURFACE ACQUIRE",
                "WAIT FOR AND ACQUIRE THE FIRST AVAILABLE SWAPCHAIN TEXTURE",
            ),
            StageId::FrameEncode => copy(
                "FRAME ENCODE",
                "CREATE TARGET VIEW AND COMMAND ENCODER, RECORD THE FULL-SCREEN RAW DRAW, FINISH BUFFER",
            ),
            StageId::FrameSubmit => copy(
                "QUEUE SUBMIT",
                "SUBMIT THE FIRST RAW-DRAW COMMAND BUFFER; CPU API TIME, NOT GPU EXECUTION",
            ),
            StageId::FramePresent => copy(
                "PRESENT REQUEST",
                "NOTIFY WINDOWING AND REQUEST PRESENT; COMPOSITOR, SCANOUT AND PHOTON TIME UNOBSERVED",
            ),
            StageId::Total => copy(
                "TOTAL LOAD -> SUBMIT",
                "WALL TIME FROM REQUEST ENQUEUE TO FIRST SUCCESSFUL PRESENT REQUEST; GPU FINISH EXCLUDED",
            ),
        }
    }

    fn status(&self, stage: Stage) -> String {
        match stage.state {
            PipelineStageState::Pending if !self.active && stage.id == StageId::Idle => {
                "WAITING FOR RAW".into()
            }
            PipelineStageState::Pending => "WAITING".into(),
            PipelineStageState::Running => "RUNNING".into(),
            PipelineStageState::Failed => "FAILED / STAGE DID NOT COMPLETE".into(),
            PipelineStageState::Skipped if self.failed_stage.is_some() => "NOT REACHED".into(),
            PipelineStageState::Skipped
                if matches!(stage.id, StageId::Fingerprint | StageId::CacheLookup) =>
            {
                "CACHE DISABLED / BYPASS".into()
            }
            PipelineStageState::Skipped => "SKIPPED".into(),
            PipelineStageState::Shared(_) => "WITHIN PARSE OR DECODE / NOT ISOLATED".into(),
            PipelineStageState::Conditional(_) => "CONDITIONAL / BRANCH NOT REPORTED".into(),
            PipelineStageState::NotTimed if is_shader(stage.id) => "FUSED FRAGMENT PASS / NOT TIMED".into(),
            PipelineStageState::NotTimed => "BOUNDARY NOT TIMED".into(),
            PipelineStageState::Measured(_) if stage.id == StageId::CacheLookup => match self.cache_route {
                Some(CacheRoute::Hit) => "CACHE HIT / VERIFIED MOSAIC".into(),
                Some(CacheRoute::Miss) => "CACHE MISS / DECODE REQUIRED".into(),
                Some(CacheRoute::Disabled) => "CACHE DISABLED / BYPASS".into(),
                Some(CacheRoute::ErrorFallback) => "CACHE ERROR / DECODE FALLBACK".into(),
                None => "CACHE LOOKUP COMPLETE".into(),
            },
            PipelineStageState::Measured(_) if stage.id == StageId::RawImageSpan => {
                match (self.native_worker_count, self.native_plane_wall) {
                    (Some(workers), Some(wall)) => {
                        format!(
                            "{workers} PLANE WORKERS / STREAMED WALL {} / ASSEMBLY OVERLAPS",
                            format_duration(wall)
                        )
                    }
                    _ => "EXACT SENSOR-DECODE TOTAL".into(),
                }
            }
            PipelineStageState::Measured(_) if is_gpu_upload(stage.id) => {
                "CPU PREP OR ENQUEUE / GPU FINISH EXCLUDED".into()
            }
            PipelineStageState::Measured(_) if stage.id == StageId::FramePresent => {
                self.frame_submit_total.map_or_else(
                    || "PRESENT REQUESTED".into(),
                    |duration| format!("FRAME CPU TOTAL {}", format_duration(duration)),
                )
            }
            PipelineStageState::Measured(_) if stage.id == StageId::Total => {
                "END-TO-END / GPU COMPLETION NOT TIMED".into()
            }
            PipelineStageState::Measured(_) => "DONE".into(),
        }
    }

    fn index_of(&self, id: StageId) -> Option<usize> {
        self.stages.iter().position(|stage| stage.id == id)
    }

    fn state(&self, id: StageId) -> Option<PipelineStageState> {
        self.index_of(id).map(|index| self.stages[index].state)
    }

    fn set(&mut self, id: StageId, state: PipelineStageState) {
        if let Some(index) = self.index_of(id) {
            self.stages[index].state = state;
        }
    }
}

#[derive(Debug, Clone, Copy)]
struct StageCopy {
    label: &'static str,
    description: &'static str,
    conditional: bool,
}

const fn copy(label: &'static str, description: &'static str) -> StageCopy {
    StageCopy {
        label,
        description,
        conditional: false,
    }
}

const fn conditional_copy(label: &'static str, description: &'static str) -> StageCopy {
    StageCopy {
        label,
        description,
        conditional: true,
    }
}

fn format_stage_copy(raw_kind: RawKind, index: usize) -> StageCopy {
    match (raw_kind, index) {
        (RawKind::Cr2, 0) => copy(
            "CR2 TIFF HEADER",
            "PARSE TIFF BYTE ORDER, HEADER AND CANON CR2 CONTAINER SIGNATURE",
        ),
        (RawKind::Cr2, 1) => copy(
            "CR2 RAW IFD",
            "FIND RAW IFD, STRIP OFFSET/SIZE, CAMERA MODE, EXIF AND CANON MAKERNOTE TAGS",
        ),
        (RawKind::Cr2, 2) => copy(
            "CR2 LJPEG MARKERS",
            "PARSE LOSSLESS JPEG SOF3, DHT AND SOS MARKERS, DIMENSIONS AND HUFFMAN TABLES",
        ),
        (RawKind::Cr2, 3) => copy(
            "CR2 HUFFMAN + PREDICT",
            "DECODE ENTROPY DIFFERENCES AND APPLY LOSSLESS JPEG SAMPLE PREDICTORS",
        ),
        (RawKind::Cr2, 4) => conditional_copy(
            "CR2 LINEARIZE",
            "D2000-ONLY GRAY-RESPONSE TABLE LOOKUP WITH DITHER; BACKEND DOES NOT REPORT THIS BRANCH",
        ),
        (RawKind::Cr2, 5) => conditional_copy(
            "CR2 SLICE REORDER",
            "REASSEMBLE CANON VERTICAL SLICES/STRIPES INTO SENSOR ROW ORDER WHEN TAGGED",
        ),
        (RawKind::Cr2, _) => copy(
            "CR2 SENSOR TAGS",
            "READ CFA, WB, BLACK/WHITE LEVELS, ACTIVE AREA AND CROP; SRAW/MRAW MAY BRANCH",
        ),
        (RawKind::Cr3, 0) => copy(
            "CR3 BMFF BOX WALK",
            "BOUNDED ISO BMFF WALK: VALIDATE BOX SIZES, MOOV/TRAK/MDIA/MINF/STBL NESTING AND LIMITS",
        ),
        (RawKind::Cr3, 1) => copy(
            "CR3 SAMPLE TABLES",
            "RESOLVE STSD/STSC/STSZ/STCO OR CO64 INTO CHECKED SAMPLE OFFSETS, SIZES AND DESCRIPTION IDS",
        ),
        (RawKind::Cr3, 2) => copy(
            "CR3 RAW TRACK SELECT",
            "RANK CRX CANDIDATES BY FULL SENSOR AND CROP SIZE; REJECT AMBIGUOUS OR OUT-OF-RANGE SAMPLES",
        ),
        (RawKind::Cr3, 3) => copy(
            "CRX FRAME + 4 PLANES",
            "VALIDATE CMP1/CDI1/IAD1, FF01/FF02/FF03 MARKERS AND BORROW FOUR EXACT PLANE RANGES",
        ),
        (RawKind::Cr3, 4) => copy(
            "CR3 CMT + CTMD",
            "READ CANON/MODEL, SENSOR PROFILE, ACTIVE/CROP AREAS AND EXACT AS-SHOT WB INTEGER RATIOS",
        ),
        (RawKind::Cr3, 5) => copy(
            "CRX PLANE R / AGENT 1",
            "DECODE EVEN-ROW/EVEN-COLUMN R: 41-ZERO ESCAPE, ADAPTIVE RICE K, RUNS AND MED PREDICTOR",
        ),
        (RawKind::Cr3, 6) => copy(
            "CRX PLANE G1 / AGENT 2",
            "DECODE EVEN-ROW/ODD-COLUMN G1 WITH ITS OWN BIT READER, RICE/RUN STATE AND ROW CONTEXT",
        ),
        (RawKind::Cr3, 7) => copy(
            "CRX PLANE G2 / AGENT 3",
            "DECODE ODD-ROW/EVEN-COLUMN G2 WITH ITS OWN BIT READER, RICE/RUN STATE AND ROW CONTEXT",
        ),
        (RawKind::Cr3, 8) => copy(
            "CRX PLANE B / AGENT 4",
            "DECODE ODD-ROW/ODD-COLUMN B WITH ITS OWN BIT READER, RICE/RUN STATE AND ROW CONTEXT",
        ),
        (RawKind::Cr3, _) => copy(
            "CRX CFA ACTIVE COPY",
            "VALIDATE EVERY ROW, THEN INTERLEAVE ARRIVING 32-ROW R/G1/G2/B BATCHES; TIME OVERLAPS DECODE",
        ),
        (RawKind::Dng, 0) => copy(
            "DNG TIFF + RAW IFD",
            "SELECT THE FULL-RESOLUTION TIFF RAW IFD AND READ IMAGE SAMPLE DESCRIPTORS",
        ),
        (RawKind::Dng, 1) => copy(
            "DNG STORAGE DISPATCH",
            "RESOLVE STRIP/TILE STORAGE, SAMPLE TYPE, BITS AND COMPRESSION CODEC",
        ),
        (RawKind::Dng, 2) => copy(
            "DNG STRIP/TILE DECODE",
            "UNPACK OR DECOMPRESS RAW STRIPS/TILES; LOSSLESS JPEG/JPEG XL TILES MAY RUN IN PARALLEL",
        ),
        (RawKind::Dng, 3) => copy(
            "DNG PADDING CROP",
            "REMOVE TILE OR JPEG PADDING AND RESTORE THE DECLARED TIFF IMAGE SIZE",
        ),
        (RawKind::Dng, 4) => conditional_copy(
            "DNG LINEARIZE",
            "OPTIONAL LINEARIZATION-TABLE LOOKUP WITH DITHER; BACKEND DOES NOT REPORT THIS BRANCH",
        ),
        (RawKind::Dng, 5) => conditional_copy(
            "DNG DEINTERLEAVE",
            "OPTIONAL 2X2 ROW/COLUMN REORDER; BACKEND DOES NOT REPORT WHETHER IT RAN",
        ),
        (RawKind::Dng, _) => copy(
            "DNG SENSOR TAGS",
            "READ CFA, WB, LEVELS, MATRIX, CROP AND ORIENTATION; OPCODELISTS ARE NOT APPLIED",
        ),
        (RawKind::Unknown, step) => copy(
            match step {
                0 => "RAW CONTAINER",
                1 => "RAW DIRECTORY MAP",
                2 => "RAW CODEC DISPATCH",
                3 => "RAW ENTROPY DECODE",
                4 => "RAW SAMPLE RECONSTRUCT",
                5 => "RAW CONDITIONAL FIXUPS",
                _ => "RAW SENSOR METADATA",
            },
            "DECODER-SPECIFIC WORK ENCLOSED BY THE MEASURED PARSE OR SENSOR-DECODE SPAN",
        ),
    }
}

const fn format_stage_index(id: StageId) -> Option<usize> {
    match id {
        StageId::Format1 => Some(0),
        StageId::Format2 => Some(1),
        StageId::Format3 => Some(2),
        StageId::Format4 => Some(3),
        StageId::Format5 => Some(4),
        StageId::Format6 => Some(5),
        StageId::Format7 => Some(6),
        StageId::Format8 => Some(7),
        StageId::Format9 => Some(8),
        StageId::Format10 => Some(9),
        _ => None,
    }
}

const fn is_shader(id: StageId) -> bool {
    matches!(
        id,
        StageId::ShaderView | StageId::ShaderNormalize | StageId::ShaderDemosaic | StageId::ShaderColorTone
    )
}

const fn is_gpu_upload(id: StageId) -> bool {
    matches!(
        id,
        StageId::GpuValidate
            | StageId::GpuAtlasPlan
            | StageId::GpuTextureAllocate
            | StageId::GpuHaloPack
            | StageId::GpuRowPack
            | StageId::GpuTextureWrite
            | StageId::GpuUniformWrite
            | StageId::GpuBind
            | StageId::GpuViewUniform
    )
}

fn format_duration(duration: Duration) -> String {
    if duration >= Duration::from_millis(1) {
        format!("{:.2} MS", duration.as_secs_f64() * 1_000.0)
    } else if duration >= Duration::from_micros(1) {
        format!("{:.0} US", duration.as_secs_f64() * 1_000_000.0)
    } else {
        format!("{} NS", duration.as_nanos())
    }
}

fn format_enclosing_duration(prefix: &str, duration: Duration) -> String {
    if duration >= Duration::from_millis(1) {
        format!("{prefix} {:.1}MS", duration.as_secs_f64() * 1_000.0)
    } else if duration >= Duration::from_micros(1) {
        format!("{prefix} {:.0}US", duration.as_secs_f64() * 1_000_000.0)
    } else {
        format!("{prefix} {}NS", duration.as_nanos())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rrrah_decode::{AdaptTimings, DecodeTimings, NativeDecodeTimings};

    fn frontend(route: CacheRoute) -> FrontendTimings {
        FrontendTimings {
            queue_wait: Duration::from_millis(2),
            fingerprint: (route != CacheRoute::Disabled).then_some(Duration::from_micros(300)),
            cache_lookup: (route != CacheRoute::Disabled).then_some(Duration::from_millis(3)),
            admission: (route != CacheRoute::Hit).then_some(Duration::from_micros(400)),
        }
    }

    fn decode_timings() -> DecodeTimings {
        DecodeTimings {
            source_open: Duration::from_micros(700),
            decoder_select: Duration::from_micros(900),
            raw_image: Duration::from_millis(24),
            raw_decode: Duration::from_micros(24_900),
            native: Some(NativeDecodeTimings {
                plane_decode: [
                    Duration::from_millis(1),
                    Duration::from_millis(2),
                    Duration::from_millis(3),
                    Duration::from_millis(4),
                ],
                plane_wall: Duration::from_millis(5),
                interleave: Duration::from_micros(600),
                worker_count: 4,
            }),
            adapt: AdaptTimings {
                layout_cfa: Duration::from_micros(10),
                levels: Duration::from_micros(20),
                color: Duration::from_micros(30),
                geometry: Duration::from_micros(40),
                finalize: Duration::from_micros(50),
                total: Duration::from_micros(150),
            },
            adapt_metadata: Duration::from_micros(150),
            total: Duration::from_millis(26),
        }
    }

    fn decoded(kind: RawKind, route: CacheRoute) -> PipelineSnapshot {
        let timings = decode_timings();
        PipelineSnapshot::from_worker(
            7,
            kind,
            route,
            frontend(route),
            Some(&timings),
            Duration::from_micros(250),
        )
    }

    fn upload_timings() -> GpuUploadTimings {
        GpuUploadTimings {
            validate: Duration::from_micros(1),
            atlas_plan: Duration::from_micros(2),
            texture_allocate: Duration::from_micros(3),
            halo_pack: Duration::from_micros(4),
            row_pack: Duration::from_micros(5),
            texture_write_enqueue: Duration::from_micros(6),
            uniform_write: Duration::from_micros(7),
            bind: Duration::from_micros(8),
            total: Duration::from_micros(36),
        }
    }

    fn frame_timings() -> FrameSubmitTimings {
        FrameSubmitTimings {
            surface_acquire: Duration::from_micros(11),
            frame_encode: Duration::from_micros(12),
            queue_submit: Duration::from_micros(13),
            present_request: Duration::from_micros(14),
            total: Duration::from_micros(50),
        }
    }

    #[test]
    fn source_kind_is_case_insensitive() {
        assert_eq!(RawKind::from_path(Path::new("a.CR2")), RawKind::Cr2);
        assert_eq!(RawKind::from_path(Path::new("a.cr3")), RawKind::Cr3);
        assert_eq!(RawKind::from_path(Path::new("a.DnG")), RawKind::Dng);
        assert_eq!(RawKind::from_path(Path::new("a.raw")), RawKind::Unknown);
    }

    #[test]
    fn decode_route_has_forty_one_cards_and_ten_format_steps() {
        for kind in [RawKind::Cr2, RawKind::Cr3, RawKind::Dng] {
            let cards = decoded(kind, CacheRoute::Miss).cards();
            assert_eq!(cards.len(), 41);
            assert_eq!(cards[6].title, "07 NATIVE CRX DECODE");
            assert_eq!(cards[17].title, "18 ADAPT LAYOUT + CFA");
            assert_eq!(cards[40].title, "41 TOTAL LOAD -> SUBMIT");
            assert!(cards[7..12].iter().all(|card| matches!(
                card.state,
                PipelineStageState::Shared(_) | PipelineStageState::Conditional(_)
            )));
            if kind == RawKind::Cr3 {
                assert!(
                    cards[12..17]
                        .iter()
                        .all(|card| matches!(card.state, PipelineStageState::Measured(_)))
                );
            }
        }
    }

    #[test]
    fn format_routes_are_distinct_and_document_conditional_work() {
        let cr2 = decoded(RawKind::Cr2, CacheRoute::Miss).cards();
        let cr3 = decoded(RawKind::Cr3, CacheRoute::Miss).cards();
        let dng = decoded(RawKind::Dng, CacheRoute::Miss).cards();
        assert!(cr2[7].title.contains("CR2"));
        assert!(cr3[7].title.contains("CR3"));
        assert!(dng[7].title.contains("DNG"));
        assert_eq!(
            cr2[11].state,
            PipelineStageState::Conditional(Duration::from_millis(24))
        );
        assert_eq!(
            cr3[12].state,
            PipelineStageState::Measured(Duration::from_millis(1))
        );
        assert!(dng[13].description.contains("OPCODELISTS ARE NOT APPLIED"));
    }

    #[test]
    fn cache_hit_route_has_twenty_two_cards_and_no_decoder_stages() {
        let cards = PipelineSnapshot::from_worker(
            2,
            RawKind::Cr3,
            CacheRoute::Hit,
            frontend(CacheRoute::Hit),
            None,
            Duration::from_micros(50),
        )
        .cards();
        assert_eq!(cards.len(), 22);
        assert!(cards[2].title.contains("CACHE MATERIALIZE"));
        assert!(cards.iter().all(|card| !card.title.contains("NATIVE CRX DECODE")));
        assert_eq!(cards.iter().filter(|card| card.cache_footer).count(), 1);
    }

    #[test]
    fn no_cache_skips_both_cache_frontend_stages_without_claiming_miss() {
        let cards = decoded(RawKind::Cr2, CacheRoute::Disabled).cards();
        assert_eq!(cards[1].state, PipelineStageState::Skipped);
        assert_eq!(cards[2].state, PipelineStageState::Skipped);
        assert!(cards[2].title.contains("CACHE BYPASS"));
        assert!(!cards[2].status.contains("MISS"));
    }

    #[test]
    fn exact_decode_and_adapter_boundaries_map_to_separate_cards() {
        let cards = decoded(RawKind::Dng, CacheRoute::Miss).cards();
        assert_eq!(
            cards[4].state,
            PipelineStageState::Measured(Duration::from_micros(700))
        );
        assert_eq!(
            cards[5].state,
            PipelineStageState::Measured(Duration::from_micros(900))
        );
        assert_eq!(
            cards[6].state,
            PipelineStageState::Measured(Duration::from_millis(24))
        );
        for (index, micros) in (17..22).zip([10, 20, 30, 40, 50]) {
            assert_eq!(
                cards[index].state,
                PipelineStageState::Measured(Duration::from_micros(micros))
            );
        }
    }

    #[test]
    fn gpu_and_shader_stages_are_maximally_split_without_fake_gpu_time() {
        let mut snapshot = decoded(RawKind::Cr3, CacheRoute::Miss);
        assert!(snapshot.complete_gpu(upload_timings(), Duration::from_micros(9)));
        let cards = snapshot.cards();
        for (index, micros) in (23..32).zip(1..=9) {
            assert_eq!(
                cards[index].state,
                PipelineStageState::Measured(Duration::from_micros(micros))
            );
        }
        assert!(
            cards[32..36]
                .iter()
                .all(|card| card.state == PipelineStageState::NotTimed)
        );
        assert_eq!(cards[36].state, PipelineStageState::Running);
    }

    #[test]
    fn frame_stages_and_end_to_end_total_latch_once() {
        let mut snapshot = decoded(RawKind::Cr2, CacheRoute::Miss);
        assert!(snapshot.complete_gpu(upload_timings(), Duration::from_micros(9)));
        assert!(snapshot.complete_display(frame_timings(), Duration::from_millis(45)));
        assert!(!snapshot.complete_display(frame_timings(), Duration::from_millis(99)));
        let cards = snapshot.cards();
        for (index, micros) in (36..40).zip(11..=14) {
            assert_eq!(
                cards[index].state,
                PipelineStageState::Measured(Duration::from_micros(micros))
            );
        }
        assert_eq!(cards[40].time, "45.00 MS");
        assert!(cards[39].status.contains("50 US"));
    }

    #[test]
    fn sub_millisecond_timings_never_collapse_to_zero_ms() {
        assert_eq!(
            PipelineStageState::Measured(Duration::from_micros(850)).timing_text(),
            "850 US"
        );
        assert_eq!(
            PipelineStageState::Measured(Duration::from_nanos(420)).timing_text(),
            "420 NS"
        );
        assert_eq!(
            PipelineStageState::Shared(Duration::from_micros(850)).timing_text(),
            "IN 850US"
        );
        assert_eq!(
            PipelineStageState::Conditional(Duration::from_millis(24)).timing_text(),
            "IF 24.0MS"
        );
    }

    #[test]
    fn frontend_progress_moves_explicitly_from_fingerprint_to_cache() {
        let first = PipelineSnapshot::from_frontend(
            3,
            RawKind::Cr2,
            None,
            FrontendTimings {
                queue_wait: Duration::from_millis(1),
                ..FrontendTimings::default()
            },
        );
        assert_eq!(first.cards()[1].state, PipelineStageState::Running);
        let second = PipelineSnapshot::from_frontend(
            3,
            RawKind::Cr2,
            None,
            FrontendTimings {
                queue_wait: Duration::from_millis(1),
                fingerprint: Some(Duration::from_micros(20)),
                ..FrontendTimings::default()
            },
        );
        assert_eq!(
            second.cards()[1].state,
            PipelineStageState::Measured(Duration::from_micros(20))
        );
        assert_eq!(second.cards()[2].state, PipelineStageState::Running);
    }

    #[test]
    fn failure_preserves_finished_work_and_marks_downstream_not_reached() {
        let mut snapshot = PipelineSnapshot::from_frontend(
            8,
            RawKind::Cr2,
            Some(CacheRoute::Miss),
            frontend(CacheRoute::Miss),
        );
        snapshot.mark_failed();
        let cards = snapshot.cards();
        assert_eq!(cards[4].state, PipelineStageState::Failed);
        assert!(
            cards[5..]
                .iter()
                .all(|card| card.state == PipelineStageState::Skipped)
        );
    }

    #[test]
    fn idle_route_is_one_waiting_card() {
        let snapshot = PipelineSnapshot::idle(0);
        assert_eq!(snapshot.generation(), 0);
        let cards = snapshot.cards();
        assert_eq!(cards.len(), 1);
        assert_eq!(cards[0].status, "WAITING FOR RAW");
    }
}
