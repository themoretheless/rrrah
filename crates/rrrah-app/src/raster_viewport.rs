//! Managed source/LOD ownership for whole-image GPU upload fallback.
use rrrah_core::{DecodedRaster, MemoryBudget, develop::MonotoneCurve};
use rrrah_gpu::{RasterRenderer, RasterUploadError, ViewParameters};

#[derive(Clone, Copy, PartialEq, Eq)]
enum ViewportWork {
    Region([u32; 4]),
    Fit([u32; 2]),
}

pub(crate) struct ResidentSource {
    identity: u64,
    source: Option<DecodedRaster>,
    swapped: Option<rrrah_cache::SwappedRaster>,
    pending_region: Option<(u64, ViewportWork)>,
    ready_region: Option<(ViewportWork, DecodedRaster)>,
    dimensions: [u32; 2],
    aspect: Option<f32>,
    region: [u32; 4],
    curve: Option<MonotoneCurve>,
    lod: Option<DecodedRaster>,
    lod_active: bool,
    lod_budget: MemoryBudget,
}

impl ResidentSource {
    fn upload_visible_region<F: FnMut() -> bool>(
        source: &DecodedRaster,
        visible: [u32; 4],
        renderer: &mut RasterRenderer,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        cancelled: &mut F,
    ) -> Result<[u32; 4], RasterUploadError> {
        let region = Self::with_pan_margin([source.width(), source.height()], visible, renderer, device);
        match renderer.upload_region_with_cancel(device, queue, source, region, &mut *cancelled) {
            Ok(()) => Ok(region),
            Err(RasterUploadError::Memory(_) | RasterUploadError::TextureLimit) if region != visible => {
                renderer.upload_region_with_cancel(device, queue, source, visible, cancelled)?;
                Ok(visible)
            }
            Err(error) => Err(error),
        }
    }
    fn with_pan_margin(
        dimensions: [u32; 2],
        visible: [u32; 4],
        renderer: &RasterRenderer,
        device: &wgpu::Device,
    ) -> [u32; 4] {
        let maximum = (visible[2].min(visible[3]) / 4).min(32);
        for margin in [32, 16, 8, 4, 2, 1] {
            if margin > maximum {
                continue;
            }
            let x = visible[0].saturating_sub(margin);
            let y = visible[1].saturating_sub(margin);
            let right = (u64::from(visible[0]) + u64::from(visible[2]) + u64::from(margin))
                .min(u64::from(dimensions[0])) as u32;
            let bottom = (u64::from(visible[1]) + u64::from(visible[3]) + u64::from(margin))
                .min(u64::from(dimensions[1])) as u32;
            let candidate = [x, y, right - x, bottom - y];
            if renderer.can_admit_texture_extent_with_spare(device, candidate[2], candidate[3]) {
                return candidate;
            }
        }
        visible
    }
    /// Only migrate after a complete verified swap write and a retained fit preview.
    pub(crate) fn adopt_swapped(&mut self, identity: u64, swapped: rrrah_cache::SwappedRaster) -> bool {
        if identity != self.identity
            || self.source.is_none()
            || self.lod.is_none()
            || swapped.dimensions() != self.dimensions
            || swapped.pixel_aspect() != self.aspect
        {
            return false;
        }
        self.swapped = Some(swapped);
        self.source = None;
        true
    }

    pub(crate) fn region_job(
        &self,
        renderer: &RasterRenderer,
        device: &wgpu::Device,
        view: ViewParameters,
    ) -> Result<Option<(rrrah_cache::SwappedRaster, [u32; 4], MemoryBudget, MemoryBudget)>, RasterUploadError>
    {
        let Some(swapped) = &self.swapped else {
            return Ok(None);
        };
        let Some(visible) =
            RasterRenderer::visible_source_region_for_geometry(self.dimensions, self.aspect, view)?
        else {
            return Ok(None);
        };
        let contained = !self.lod_active
            && (0..2).all(|a| {
                visible[a] >= self.region[a]
                    && u64::from(visible[a]) + u64::from(visible[a + 2])
                        <= u64::from(self.region[a]) + u64::from(self.region[a + 2])
            });
        if contained || !renderer.can_admit_texture_extent(device, visible[2], visible[3]) {
            return Ok(None);
        }
        let scratch = self.lod_budget.child(65536);
        let candidate = Self::with_pan_margin(self.dimensions, visible, renderer, device);
        let region = if swapped
            .can_restore_region(candidate, &self.lod_budget, &scratch)
            .unwrap_or(false)
        {
            candidate
        } else {
            visible
        };
        Ok(Some((swapped.clone(), region, self.lod_budget.clone(), scratch)))
    }

    pub(crate) fn publish_region(
        &mut self,
        renderer: &mut RasterRenderer,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        rect: [u32; 4],
        crop: &DecodedRaster,
    ) -> Result<(), RasterUploadError> {
        if self.swapped.is_none()
            || [crop.width(), crop.height()] != [rect[2], rect[3]]
            || crop.pixel_aspect() != self.aspect
        {
            return Err(RasterUploadError::InvalidRegion);
        }
        renderer.upload_packed_region_with_cancel(
            device,
            queue,
            crop,
            self.dimensions,
            [rect[0], rect[1]],
            || false,
        )?;
        if let Some(curve) = &self.curve {
            renderer.set_raw_development(queue, curve);
        }
        self.region = rect;
        self.lod_active = false;
        Ok(())
    }

    pub(crate) fn persistence_source(&self) -> Option<(DecodedRaster, MemoryBudget)> {
        self.lod.as_ref()?;
        Some((self.source.as_ref()?.clone(), self.lod_budget.child(1024 * 1024)))
    }

    pub(crate) fn is_swapped(&self) -> bool {
        self.swapped.is_some()
    }

    pub(crate) fn refresh_swapped(
        &mut self,
        renderer: &mut RasterRenderer,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        view: ViewParameters,
        worker: &mut crate::raster_lod_worker::LodWorker,
        token: rrrah_decode::GenerationToken,
    ) -> Result<bool, String> {
        let visible = RasterRenderer::visible_source_region_for_geometry(self.dimensions, self.aspect, view)
            .map_err(|e| e.to_string())?;
        let contained = !self.lod_active
            && visible.is_some_and(|visible| {
                (0..2).all(|a| {
                    visible[a] >= self.region[a]
                        && u64::from(visible[a]) + u64::from(visible[a + 2])
                            <= u64::from(self.region[a]) + u64::from(self.region[a + 2])
                })
            });
        let needs_fit = !self.lod_active
            && !contained
            && visible.is_some_and(|r| !renderer.can_admit_texture_extent(device, r[2], r[3]));
        if let Some(completed) = worker.try_take() {
            if let Some((id, work)) = self.pending_region {
                if completed.id == id && completed.generation == self.identity {
                    self.pending_region = None;
                    self.ready_region = Some((work, completed.into_frame().map_err(|e| e.to_string())?));
                }
            }
        }
        if let Some((work, pixels)) = self.ready_region.clone() {
            let useful = match work {
                ViewportWork::Region(rect) => visible.is_some_and(|visible| {
                    (0..2).all(|a| {
                        visible[a] >= rect[a]
                            && u64::from(visible[a]) + u64::from(visible[a + 2])
                                <= u64::from(rect[a]) + u64::from(rect[a + 2])
                    })
                }),
                ViewportWork::Fit(_) => visible.is_some() && (needs_fit || self.lod_active),
            };
            if useful {
                // Retain prepared pixels across admission failures, without repeating I/O/filtering.
                match work {
                    ViewportWork::Region(rect) => self.publish_region(renderer, device, queue, rect, &pixels),
                    ViewportWork::Fit(_) => self.publish_lod(renderer, device, queue, pixels),
                }
                .map_err(|e| e.to_string())?;
                self.ready_region = None;
                return Ok(true);
            }
            self.ready_region = None;
        }
        let region_job = self
            .region_job(renderer, device, view)
            .map_err(|e| e.to_string())?;
        let desired = if let Some((_, rect, _, _)) = &region_job {
            Some(ViewportWork::Region(*rect))
        } else if visible.is_some() && (needs_fit || self.lod_active) {
            if let Some((_, work @ ViewportWork::Fit(_))) = self.pending_region {
                Some(work) // Active output already owns its admission credit.
            } else {
                let source = self.swapped.as_ref().unwrap();
                let scratch = self.lod_budget.child(65536);
                let mut size = self.dimensions;
                let current = self.lod.as_ref().map(|lod| [lod.width(), lod.height()]);
                let candidate = loop {
                    size = size.map(|v| v.div_ceil(2));
                    // Keep an admissible cached level; only rebuild to improve it,
                    // or to replace a cached level that no longer fits on the GPU.
                    if current == Some(size) && renderer.can_admit_texture_extent(device, size[0], size[1]) {
                        break None;
                    }
                    if source.can_downsample_linear(size, &self.lod_budget, &scratch)
                        && renderer.can_admit_texture_extent(device, size[0], size[1])
                    {
                        break Some(ViewportWork::Fit(size));
                    }
                    if size == [1, 1] {
                        break None;
                    }
                };
                candidate
            }
        } else {
            None
        };
        if self.pending_region.is_some_and(|(_, work)| Some(work) != desired) {
            worker.cancel();
            self.pending_region = None;
        }
        if let Some(work) = desired {
            if self.pending_region.is_none() {
                let id = match work {
                    ViewportWork::Region(_) => {
                        let (source, rect, budget, scratch) = region_job.unwrap();
                        worker.submit_region(source, rect, budget, scratch, self.identity, token)
                    }
                    ViewportWork::Fit(size) => worker.submit_swap_lod(
                        self.swapped.as_ref().unwrap().clone(),
                        size,
                        self.lod_budget.clone(),
                        self.lod_budget.child(65536),
                        self.identity,
                        token,
                    ),
                }
                .map_err(|e| e.to_string())?;
                self.pending_region = Some((id, work));
            }
            return Ok(false);
        }
        self.refresh_deferred(renderer, device, queue, view)
            .map_err(|e| e.to_string())
    }

    pub(crate) fn identity(&self) -> u64 {
        self.identity
    }

    pub(crate) fn refresh_job(
        &self,
        renderer: &RasterRenderer,
        device: &wgpu::Device,
        view: ViewParameters,
    ) -> Result<Option<(DecodedRaster, [u32; 2], MemoryBudget)>, RasterUploadError> {
        if !self.needs_lod(renderer, device, view)? {
            return Ok(None);
        }
        let Some(source) = &self.source else {
            return Ok(None);
        };
        let size = Self::plan_lod_size(source, &self.lod_budget, renderer, device)?;
        Ok(Some((source.clone(), size, self.lod_budget.clone())))
    }

    pub(crate) fn needs_lod(
        &self,
        renderer: &RasterRenderer,
        device: &wgpu::Device,
        view: ViewParameters,
    ) -> Result<bool, RasterUploadError> {
        if self.source.is_none() {
            return Ok(false);
        }
        let Some(region) =
            RasterRenderer::visible_source_region_for_geometry(self.dimensions, self.aspect, view)?
        else {
            return Ok(false);
        };
        let contained = !self.lod_active
            && (0..2).all(|a| {
                region[a] >= self.region[a]
                    && u64::from(region[a]) + u64::from(region[a + 2])
                        <= u64::from(self.region[a]) + u64::from(self.region[a + 2])
            });
        if contained
            || self.lod_active
            || renderer.can_admit_texture_extent(device, region[2], region[3])
            || self
                .lod
                .as_ref()
                .is_some_and(|lod| renderer.can_admit_texture_extent(device, lod.width(), lod.height()))
        {
            return Ok(false);
        }
        Ok(true)
    }

    pub(crate) fn publish_lod(
        &mut self,
        renderer: &mut RasterRenderer,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        lod: DecodedRaster,
    ) -> Result<(), RasterUploadError> {
        renderer.upload(device, queue, &lod)?;
        if let Some(curve) = &self.curve {
            renderer.set_raw_development(queue, curve);
        }
        self.lod = Some(lod);
        self.lod_active = true;
        Ok(())
    }
    pub(crate) fn plan_lod_size(
        source: &DecodedRaster,
        budget: &MemoryBudget,
        renderer: &RasterRenderer,
        device: &wgpu::Device,
    ) -> Result<[u32; 2], RasterUploadError> {
        let mut size = [source.width(), source.height()];
        loop {
            size = size.map(|v| v.div_ceil(2));
            let bytes = u64::from(size[0]) * u64::from(size[1]) * 16;
            if bytes <= budget.available_bytes()
                && renderer.can_admit_texture_extent(device, size[0], size[1])
            {
                return Ok(size);
            }
            if size == [1, 1] {
                return Err(RasterUploadError::LodUnavailable);
            }
        }
    }
    fn make_lod<F: FnMut() -> bool>(
        &self,
        renderer: &RasterRenderer,
        device: &wgpu::Device,
        cancelled: &mut F,
    ) -> Result<DecodedRaster, RasterUploadError> {
        if cancelled() {
            return Err(RasterUploadError::Cancelled);
        }
        let source = self.source.as_ref().ok_or(RasterUploadError::LodUnavailable)?;
        let size = Self::plan_lod_size(source, &self.lod_budget, renderer, device)?;
        source
            .downsample_linear_with_budget_and_cancel(size, &self.lod_budget, &mut *cancelled)
            .map_err(|e| match e {
                rrrah_core::RasterError::Cancelled => RasterUploadError::Cancelled,
                other => other.into(),
            })
    }

    #[cfg(test)]
    pub(crate) fn install(
        renderer: &mut RasterRenderer,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        source: &DecodedRaster,
        view: ViewParameters,
        curve: Option<&MonotoneCurve>,
        lod_budget: &MemoryBudget,
    ) -> Result<Option<Self>, RasterUploadError> {
        Self::install_with_cancel(renderer, device, queue, source, view, curve, lod_budget, || false)
    }

    pub(crate) fn install_with_cancel<F: FnMut() -> bool>(
        renderer: &mut RasterRenderer,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        source: &DecodedRaster,
        view: ViewParameters,
        curve: Option<&MonotoneCurve>,
        lod_budget: &MemoryBudget,
        cancelled: F,
    ) -> Result<Option<Self>, RasterUploadError> {
        Self::install_with_prepared_lod(
            renderer, device, queue, source, view, curve, lod_budget, None, cancelled,
        )
    }
    pub(crate) fn install_with_prepared_lod<F: FnMut() -> bool>(
        renderer: &mut RasterRenderer,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        source: &DecodedRaster,
        view: ViewParameters,
        curve: Option<&MonotoneCurve>,
        lod_budget: &MemoryBudget,
        prepared: Option<DecodedRaster>,
        mut cancelled: F,
    ) -> Result<Option<Self>, RasterUploadError> {
        if cancelled() {
            return Err(RasterUploadError::Cancelled);
        }
        let error = if prepared.is_some()
            && !renderer.can_admit_texture_extent(device, source.width(), source.height())
        {
            RasterUploadError::LodUnavailable
        } else {
            match renderer.upload_with_cancel(device, queue, source, &mut cancelled) {
                Ok(()) => return Ok(None),
                Err(e @ (RasterUploadError::Memory(_) | RasterUploadError::TextureLimit)) => e,
                Err(e) => return Err(e),
            }
        };
        let visible = RasterRenderer::visible_source_region(source, view)?.unwrap_or([0, 0, 1, 1]);
        let region = Self::with_pan_margin([source.width(), source.height()], visible, renderer, device);
        let mut state = Self {
            identity: {
                static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
                NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
            },
            source: Some(source.clone()),
            swapped: None,
            pending_region: None,
            ready_region: None,
            dimensions: [source.width(), source.height()],
            aspect: source.pixel_aspect(),
            region,
            curve: curve.cloned(),
            lod: prepared,
            lod_active: false,
            lod_budget: lod_budget.clone(),
        };
        if region != [0, 0, source.width(), source.height()]
            && renderer.can_admit_texture_extent(device, region[2], region[3])
        {
            state.region =
                Self::upload_visible_region(source, visible, renderer, device, queue, &mut cancelled)?;
        } else {
            let lod = match state.lod.take() {
                Some(lod) => lod,
                None => state
                    .make_lod(renderer, device, &mut cancelled)
                    .map_err(|e| match e {
                        RasterUploadError::LodUnavailable => error,
                        other => other,
                    })?,
            };
            renderer.upload_with_cancel(device, queue, &lod, &mut cancelled)?;
            state.lod = Some(lod);
            state.lod_active = true;
        }
        let region = state.region;
        log::info!(
            "Raster fallback: source={}x{} region={region:?} lod={}",
            source.width(),
            source.height(),
            state.lod_active
        );
        Ok(Some(state))
    }

    pub(crate) fn refresh(
        &mut self,
        renderer: &mut RasterRenderer,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        view: ViewParameters,
    ) -> Result<bool, RasterUploadError> {
        self.refresh_with_cancel(renderer, device, queue, view, || false)
    }

    pub(crate) fn refresh_deferred(
        &mut self,
        renderer: &mut RasterRenderer,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        view: ViewParameters,
    ) -> Result<bool, RasterUploadError> {
        self.refresh_inner(renderer, device, queue, view, || false, false)
    }

    pub(crate) fn refresh_with_cancel<F: FnMut() -> bool>(
        &mut self,
        renderer: &mut RasterRenderer,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        view: ViewParameters,
        mut cancelled: F,
    ) -> Result<bool, RasterUploadError> {
        self.refresh_inner(renderer, device, queue, view, &mut cancelled, true)
    }

    fn refresh_inner<F: FnMut() -> bool>(
        &mut self,
        renderer: &mut RasterRenderer,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        view: ViewParameters,
        mut cancelled: F,
        allow_filter: bool,
    ) -> Result<bool, RasterUploadError> {
        if cancelled() {
            return Err(RasterUploadError::Cancelled);
        }
        let Some(region) =
            RasterRenderer::visible_source_region_for_geometry(self.dimensions, self.aspect, view)?
        else {
            return Ok(false);
        };
        if !self.lod_active
            && (0..2).all(|a| {
                region[a] >= self.region[a]
                    && u64::from(region[a]) + u64::from(region[a + 2])
                        <= u64::from(self.region[a]) + u64::from(self.region[a + 2])
            })
        {
            return Ok(false);
        }
        if self.lod_active && !renderer.can_admit_texture_extent(device, region[2], region[3]) {
            return Ok(false);
        }
        let Some(source) = &self.source else {
            // Disk reads are scheduled separately; this path never blocks the UI.
            if !renderer.can_admit_texture_extent(device, region[2], region[3]) {
                if let Some(lod) = &self.lod {
                    renderer.upload_with_cancel(device, queue, lod, &mut cancelled)?;
                    self.lod_active = true;
                    if let Some(curve) = &self.curve {
                        renderer.set_raw_development(queue, curve);
                    }
                    return Ok(true);
                }
            }
            return Err(RasterUploadError::LodUnavailable);
        };
        match Self::upload_visible_region(source, region, renderer, device, queue, &mut cancelled) {
            Ok(region) => {
                self.region = region;
                self.lod_active = false;
            }
            Err(error @ (RasterUploadError::Memory(_) | RasterUploadError::TextureLimit)) => {
                if self.lod_active {
                    return Ok(false);
                }
                let replacement =
                    if self.lod.as_ref().is_none_or(|lod| {
                        !renderer.can_admit_texture_extent(device, lod.width(), lod.height())
                    }) {
                        if !allow_filter {
                            return Err(RasterUploadError::LodUnavailable);
                        }
                        Some(
                            self.make_lod(renderer, device, &mut cancelled)
                                .map_err(|e| match e {
                                    RasterUploadError::LodUnavailable => error,
                                    other => other,
                                })?,
                        )
                    } else {
                        None
                    };
                renderer.upload_with_cancel(
                    device,
                    queue,
                    replacement.as_ref().or(self.lod.as_ref()).unwrap(),
                    &mut cancelled,
                )?;
                if let Some(lod) = replacement {
                    self.lod = Some(lod);
                }
                self.lod_active = true;
            }
            Err(e) => return Err(e),
        }
        if let Some(curve) = &self.curve {
            renderer.set_raw_development(queue, curve);
        }
        let region = self.region;
        log::debug!("Raster refreshed: region={region:?} lod={}", self.lod_active);
        Ok(true)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rrrah_core::{MemoryBudget, RasterColorSpace, RasterPixels};
    use std::sync::Arc;
    #[test]
    #[ignore = "actual Metal 36MP original-swap HDR LOD latency and bounded workspace"]
    fn large_swapped_hdr_lod_reaches_exact_metal_frame() {
        let gpu = crate::navigation_metal_readback::qualification_gpu().expect("actual GPU required");
        let cpu = MemoryBudget::new(576_000_000);
        let pixels: Vec<f32> = (0..6000 * 6000)
            .flat_map(|i| {
                [
                    (i % 257) as f32 / 32. - 1.,
                    (i / 6000 % 97) as f32 / 16.,
                    (i % 17) as f32 / 8.,
                    [0., 0.25, 0.75, 1.][i % 4],
                ]
            })
            .collect();
        let source = DecodedRaster::new(
            6000,
            6000,
            RasterPixels::Rgba32Float(Arc::new(pixels).into()),
            RasterColorSpace::LinearSrgb,
        )
        .unwrap()
        .with_pixel_aspect(Some(1.25))
        .unwrap()
        .try_manage_pixels(&cpu)
        .unwrap();
        let reference = source
            .downsample_linear_with_budget_and_cancel([128, 128], &MemoryBudget::new(262144), || false)
            .unwrap();
        let view = ViewParameters {
            viewport: [128.; 2],
            ..Default::default()
        };
        let expected = gpu.render_raster_half(&reference, view, [128, 128]);
        drop(reference);
        let directory = tempfile::tempdir().unwrap();
        let store = rrrah_swap::SwapStore::new(
            directory.path(),
            rrrah_swap::SwapLimits {
                max_bytes: 600_000_000,
                max_objects: Some(2),
            },
        )
        .unwrap();
        let index = MemoryBudget::new(300000);
        let swapped = rrrah_cache::SwappedRaster::write(&store, &source, &index, || false).unwrap();
        drop(source);
        assert_eq!(cpu.used(), 0);
        let output = MemoryBudget::new(500000);
        let scratch = output.child(65536);
        assert!(swapped.can_downsample_linear([128, 128], &output, &scratch));
        let mut worker = crate::raster_lod_worker::LodWorker::default();
        let (tx, rx) = crossbeam_channel::bounded(1);
        worker.set_waker(move || {
            let _ = tx.try_send(());
        });
        let generation = Arc::new(std::sync::atomic::AtomicU64::new(1));
        eprintln!("36MP worker profiling start");
        let started = std::time::Instant::now();
        worker
            .submit_swap_lod(
                swapped.clone(),
                [128, 128],
                output.clone(),
                scratch.clone(),
                1,
                rrrah_decode::GenerationToken::new(generation.clone(), 1),
            )
            .unwrap();
        rx.recv_timeout(std::time::Duration::from_secs(60)).unwrap();
        let prepared = started.elapsed();
        let frame = worker.try_take().unwrap().into_frame().unwrap();
        let texture = MemoryBudget::new(600000);
        let queued = MemoryBudget::new(600000);
        let mut renderer = RasterRenderer::new_linear_hdr_with_budget(&gpu.device, texture.clone())
            .with_upload_queue_budget(queued.clone());
        let publish_started = std::time::Instant::now();
        renderer.upload(&gpu.device, &gpu.queue, &frame).unwrap();
        renderer.update_view(&gpu.queue, view);
        assert_eq!(gpu.render_existing_raster_half(&renderer, [128, 128]), expected);
        eprintln!(
            "36MP swapped HDR Metal: worker_ms={} publish_render_readback_ms={} total_ms={} managed_peak={} index_bytes={} source_cpu={} exact_frame=true",
            prepared.as_secs_f64() * 1000.,
            publish_started.elapsed().as_secs_f64() * 1000.,
            started.elapsed().as_secs_f64() * 1000.,
            output.peak(),
            index.used(),
            cpu.used()
        );
        assert_eq!(output.peak(), 427776);
        drop(frame);
        assert_eq!(store.usage().unwrap().objects, 2);
        let hit_budget = MemoryBudget::new(350000);
        let hit_scratch = hit_budget.child(65536);
        assert!(swapped.can_downsample_linear([128, 128], &hit_budget, &hit_scratch));
        let hit_started = std::time::Instant::now();
        worker
            .submit_swap_lod(
                swapped.clone(),
                [128, 128],
                hit_budget.clone(),
                hit_scratch.clone(),
                1,
                rrrah_decode::GenerationToken::new(generation, 1),
            )
            .unwrap();
        rx.recv_timeout(std::time::Duration::from_secs(60)).unwrap();
        let hit_ms = hit_started.elapsed().as_secs_f64() * 1000.;
        let cached_frame = worker.try_take().unwrap().into_frame().unwrap();
        renderer.upload(&gpu.device, &gpu.queue, &cached_frame).unwrap();
        renderer.update_view(&gpu.queue, view);
        assert_eq!(gpu.render_existing_raster_half(&renderer, [128, 128]), expected);
        assert_eq!(hit_budget.peak(), 327680);
        eprintln!(
            "36MP exact LOD swap cache: hit_worker_ms={hit_ms} hit_peak={} source_cpu={} exact_frame=true objects=2",
            hit_budget.peak(),
            cpu.used()
        );
        drop(cached_frame);
        assert_eq!((hit_budget.used(), hit_scratch.used()), (0, 0));
        drop(worker);
        drop(renderer);
        drop(swapped);
        assert_eq!(
            (
                cpu.used(),
                output.used(),
                scratch.used(),
                index.used(),
                texture.used(),
                queued.used(),
                store.usage().unwrap().bytes
            ),
            (0, 0, 0, 0, 0, 0, 0)
        );
    }

    #[test]
    #[ignore = "actual Metal swapped source residency, worker panning and fit preview"]
    fn swapped_residency_releases_full_source_and_preserves_detail_and_fit() {
        let gpu = crate::navigation_metal_readback::qualification_gpu().expect("actual GPU required");
        let cpu = MemoryBudget::new(512 * 320 * 16 + 300000);
        let source = DecodedRaster::new(
            512,
            320,
            RasterPixels::Rgba32Float(
                Arc::new(
                    (0..512 * 320)
                        .flat_map(|i| {
                            [
                                (i % 29) as f32 / 8. - 0.5,
                                (i % 13) as f32 / 4.,
                                (i / 512) as f32 / 160.,
                                [0., 0.25, 0.75, 1.][i % 4],
                            ]
                        })
                        .collect::<Vec<_>>(),
                )
                .into(),
            ),
            RasterColorSpace::LinearSrgb,
        )
        .unwrap()
        .try_manage_pixels(&cpu)
        .unwrap();
        let fit = ViewParameters {
            viewport: [64.; 2],
            ..Default::default()
        };
        let views: Vec<_> = [-32., 0., 32.]
            .into_iter()
            .map(|x| ViewParameters {
                zoom: 32.,
                pan: [x, 0.],
                ..fit
            })
            .collect();
        let expected: Vec<_> = views
            .iter()
            .map(|v| gpu.render_raster_half(&source, *v, [64, 64]))
            .collect();
        let pressure_reference = source
            .downsample_linear_with_budget_and_cancel([32, 20], &MemoryBudget::new(32 * 20 * 16), || false)
            .unwrap();
        let small_fit_reference = gpu.render_raster_half(&pressure_reference, fit, [64, 64]);
        drop(pressure_reference);
        let budget = cpu.child(300000);
        let preview = source
            .downsample_linear_with_budget_and_cancel([64, 40], &budget, || false)
            .unwrap();
        let texture = MemoryBudget::new(160000);
        let queued = MemoryBudget::new(160000);
        let mut renderer = RasterRenderer::new_linear_hdr_with_budget(&gpu.device, texture.clone())
            .with_upload_queue_budget(queued.clone());
        let mut state = ResidentSource::install_with_prepared_lod(
            &mut renderer,
            &gpu.device,
            &gpu.queue,
            &source,
            fit,
            None,
            &budget,
            Some(preview),
            || false,
        )
        .unwrap()
        .unwrap();
        renderer.update_view(&gpu.queue, fit);
        let expected_fit = gpu.render_existing_raster_half(&renderer, [64, 64]);
        let directory = tempfile::tempdir().unwrap();
        let store = rrrah_swap::SwapStore::new(
            directory.path(),
            rrrah_swap::SwapLimits {
                max_bytes: 3_000_000,
                max_objects: Some(2),
            },
        )
        .unwrap();
        let index = MemoryBudget::new(4096);
        let mut storage_worker = crate::raster_lod_worker::LodWorker::default();
        let (storage_tx, storage_rx) = crossbeam_channel::bounded(1);
        storage_worker.set_waker(move || {
            let _ = storage_tx.try_send(());
        });
        let storage_generation = Arc::new(std::sync::atomic::AtomicU64::new(state.identity()));
        storage_worker
            .submit_swap(
                source.clone(),
                store.clone(),
                index.clone(),
                state.identity(),
                rrrah_decode::GenerationToken::new(storage_generation, state.identity()),
            )
            .unwrap();
        storage_rx
            .recv_timeout(std::time::Duration::from_secs(10))
            .unwrap();
        let swapped = storage_worker.try_take().unwrap().into_swapped().unwrap();
        drop(storage_worker);
        assert!(!state.adopt_swapped(state.identity() + 1, swapped.clone()));
        assert!(state.source.is_some());
        assert!(state.adopt_swapped(state.identity(), swapped.clone()));
        drop(source);
        assert_eq!(cpu.used(), 64 * 40 * 16, "only fit preview remains");
        let mut worker = crate::raster_lod_worker::LodWorker::default();
        let (wake_tx, wake_rx) = crossbeam_channel::bounded(1);
        worker.set_waker(move || {
            let _ = wake_tx.try_send(());
        });
        // GPU spare permits overscan, but a shared CPU parent permits only the
        // exact visible crop plus the verified-read scratch phase.
        let visible =
            RasterRenderer::visible_source_region_for_geometry(state.dimensions, state.aspect, views[0])
                .unwrap()
                .unwrap();
        let pressure = budget.try_reserve(budget.available_bytes() - 95000).unwrap();
        let (backing, planned, output, scratch) = state
            .region_job(&renderer, &gpu.device, views[0])
            .unwrap()
            .unwrap();
        assert_eq!(planned, visible);
        assert!(backing.can_restore_region(planned, &output, &scratch).unwrap());
        let cropped = backing
            .restore_region(planned, &output, &scratch, || false)
            .unwrap();
        assert_eq!(cropped.rect, visible);
        drop(cropped);
        drop(backing);
        drop(pressure);
        assert_eq!(cpu.used(), 64 * 40 * 16);
        let generation = Arc::new(std::sync::atomic::AtomicU64::new(state.identity()));
        let token = || {
            rrrah_decode::GenerationToken::new(
                generation.clone(),
                generation.load(std::sync::atomic::Ordering::Acquire),
            )
        };
        // Corrupt one selected pixel block, after the separately verified prefix.
        // Restore must fail before publication and leave the installed fit frame.
        {
            use std::io::{Read, Seek, SeekFrom, Write};
            let (_, rect, _, _) = state
                .region_job(&renderer, &gpu.device, views[0])
                .unwrap()
                .unwrap();
            let session = std::fs::read_dir(directory.path())
                .unwrap()
                .next()
                .unwrap()
                .unwrap()
                .path();
            let path = std::fs::read_dir(session)
                .unwrap()
                .next()
                .unwrap()
                .unwrap()
                .path();
            let mut file = std::fs::OpenOptions::new()
                .read(true)
                .write(true)
                .open(path)
                .unwrap();
            let offset = 64 + (u64::from(rect[1]) * 512 + u64::from(rect[0])) * 16;
            assert!(offset > 65536, "corrupt selected pixels, not metadata block");
            file.seek(SeekFrom::Start(offset)).unwrap();
            let mut original = [0];
            file.read_exact(&mut original).unwrap();
            file.seek(SeekFrom::Start(offset)).unwrap();
            file.write_all(&[original[0] ^ 0x80]).unwrap();
            file.flush().unwrap();
            assert!(
                !state
                    .refresh_swapped(
                        &mut renderer,
                        &gpu.device,
                        &gpu.queue,
                        views[0],
                        &mut worker,
                        token()
                    )
                    .unwrap()
            );
            wake_rx.recv_timeout(std::time::Duration::from_secs(10)).unwrap();
            let error = state
                .refresh_swapped(
                    &mut renderer,
                    &gpu.device,
                    &gpu.queue,
                    views[0],
                    &mut worker,
                    token(),
                )
                .unwrap_err();
            assert!(error.contains("digest mismatch"), "{error}");
            assert!(state.pending_region.is_none() && state.ready_region.is_none());
            assert_eq!(cpu.used(), 64 * 40 * 16);
            assert_eq!(gpu.render_existing_raster_half(&renderer, [64, 64]), expected_fit);
            assert!(!worker.has_completed());
            assert!(
                wake_rx.try_recv().is_err(),
                "error must not start an automatic retry/wake loop"
            );
            file.seek(SeekFrom::Start(offset)).unwrap();
            file.write_all(&original).unwrap();
            file.flush().unwrap();
            eprintln!(
                "corrupt selected swap block refused; previous HDR frame and preview retained; transient memory=0; no retry wake"
            );
        }
        let seconds: u64 = std::env::var("RRRAH_SWAPPED_VIEWPORT_SOAK_SECONDS")
            .ok()
            .map(|s| s.parse().expect("valid soak seconds"))
            .unwrap_or(0);
        let started = std::time::Instant::now();
        let mut cycles = 0_u64;
        loop {
            // Exhaust the shared output/scratch parent before the first disk read.
            let pressure = budget.try_reserve(budget.available_bytes()).unwrap();
            let old = gpu.render_existing_raster_half(&renderer, [64, 64]);
            assert!(
                !state
                    .refresh_swapped(
                        &mut renderer,
                        &gpu.device,
                        &gpu.queue,
                        views[0],
                        &mut worker,
                        token()
                    )
                    .unwrap()
            );
            wake_rx.recv_timeout(std::time::Duration::from_secs(10)).unwrap();
            assert!(
                state
                    .refresh_swapped(
                        &mut renderer,
                        &gpu.device,
                        &gpu.queue,
                        views[0],
                        &mut worker,
                        token()
                    )
                    .is_err()
            );
            assert!(state.pending_region.is_none() && state.ready_region.is_none());
            assert_eq!(gpu.render_existing_raster_half(&renderer, [64, 64]), old);
            drop(pressure);
            assert_eq!(cpu.used(), 64 * 40 * 16);
            for (step, (view, expected)) in views.iter().zip(&expected).enumerate() {
                assert!(
                    !state
                        .refresh_swapped(
                            &mut renderer,
                            &gpu.device,
                            &gpu.queue,
                            *view,
                            &mut worker,
                            token()
                        )
                        .unwrap()
                );
                wake_rx.recv_timeout(std::time::Duration::from_secs(10)).unwrap();
                let old = gpu.render_existing_raster_half(&renderer, [64, 64]);
                let pressure = if step % 2 == 0 {
                    texture.try_reserve(texture.available_bytes()).unwrap()
                } else {
                    queued.try_reserve(queued.available_bytes()).unwrap()
                };
                assert!(
                    state
                        .refresh_swapped(
                            &mut renderer,
                            &gpu.device,
                            &gpu.queue,
                            *view,
                            &mut worker,
                            token()
                        )
                        .is_err()
                );
                assert!(state.ready_region.is_some());
                assert!(state.pending_region.is_none());
                assert_eq!(gpu.render_existing_raster_half(&renderer, [64, 64]), old);
                drop(pressure);
                assert!(
                    state
                        .refresh_swapped(
                            &mut renderer,
                            &gpu.device,
                            &gpu.queue,
                            *view,
                            &mut worker,
                            token()
                        )
                        .unwrap()
                );
                renderer.update_view(&gpu.queue, *view);
                assert_eq!(gpu.render_existing_raster_half(&renderer, [64, 64]), *expected);
                assert!(
                    !state
                        .refresh_swapped(
                            &mut renderer,
                            &gpu.device,
                            &gpu.queue,
                            *view,
                            &mut worker,
                            token()
                        )
                        .unwrap()
                );
                assert_eq!(cpu.used(), 64 * 40 * 16);
            }
            assert!(
                state
                    .refresh_deferred(&mut renderer, &gpu.device, &gpu.queue, fit)
                    .unwrap()
            );
            renderer.update_view(&gpu.queue, fit);
            assert_eq!(gpu.render_existing_raster_half(&renderer, [64, 64]), expected_fit);
            cycles += 1;
            if started.elapsed() >= std::time::Duration::from_secs(seconds) {
                break;
            }
        }
        eprintln!(
            "swapped viewport soak: seconds={:.3} cycles={cycles} exact_detail_frames={} CPU_pressure_refusals={cycles} GPU_pressure_refusals={} cpu_peak={} texture_peak={} queue_peak={}",
            started.elapsed().as_secs_f64(),
            cycles * 3,
            cycles * 3,
            cpu.peak(),
            texture.peak(),
            queued.peak()
        );
        drop(worker);
        // Deterministically pause the first disk job while three viewport changes
        // replace it. Only the newest rectangle may reach Metal publication.
        let mut worker = crate::raster_lod_worker::LodWorker::default();
        let (entered_tx, entered_rx) = crossbeam_channel::bounded(1);
        let (resume_tx, resume_rx) = crossbeam_channel::bounded(1);
        let (wake_tx, wake_rx) = crossbeam_channel::bounded(1);
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
        assert!(
            !state
                .refresh_swapped(
                    &mut renderer,
                    &gpu.device,
                    &gpu.queue,
                    views[0],
                    &mut worker,
                    token()
                )
                .unwrap()
        );
        entered_rx
            .recv_timeout(std::time::Duration::from_secs(10))
            .unwrap();
        for view in &views[1..] {
            assert!(
                !state
                    .refresh_swapped(
                        &mut renderer,
                        &gpu.device,
                        &gpu.queue,
                        *view,
                        &mut worker,
                        token()
                    )
                    .unwrap()
            );
        }
        assert_eq!(cpu.used(), 64 * 40 * 16);
        resume_tx.send(()).unwrap();
        wake_rx.recv_timeout(std::time::Duration::from_secs(10)).unwrap();
        assert!(
            state
                .refresh_swapped(
                    &mut renderer,
                    &gpu.device,
                    &gpu.queue,
                    views[2],
                    &mut worker,
                    token()
                )
                .unwrap()
        );
        renderer.update_view(&gpu.queue, views[2]);
        assert_eq!(gpu.render_existing_raster_half(&renderer, [64, 64]), expected[2]);
        assert!(state.pending_region.is_none() && state.ready_region.is_none());
        assert_eq!(cpu.used(), 64 * 40 * 16);
        let old = gpu.render_existing_raster_half(&renderer, [64, 64]);
        let texture_pressure = texture.try_reserve(texture.available_bytes() - 20000).unwrap();
        assert!(
            !state
                .refresh_swapped(&mut renderer, &gpu.device, &gpu.queue, fit, &mut worker, token())
                .unwrap()
        );
        assert!(matches!(
            state.pending_region,
            Some((_, ViewportWork::Fit([32, 20])))
        ));
        wake_rx.recv_timeout(std::time::Duration::from_secs(10)).unwrap();
        let queue_pressure = queued.try_reserve(queued.available_bytes()).unwrap();
        assert!(
            state
                .refresh_swapped(&mut renderer, &gpu.device, &gpu.queue, fit, &mut worker, token())
                .is_err()
        );
        assert!(matches!(
            state.ready_region,
            Some((ViewportWork::Fit([32, 20]), _))
        ));
        assert_eq!(gpu.render_existing_raster_half(&renderer, [64, 64]), old);
        assert_eq!(cpu.used(), (64 * 40 + 32 * 20) * 16);
        drop(queue_pressure);
        assert!(
            state
                .refresh_swapped(&mut renderer, &gpu.device, &gpu.queue, fit, &mut worker, token())
                .unwrap()
        );
        renderer.update_view(&gpu.queue, fit);
        assert_eq!(
            gpu.render_existing_raster_half(&renderer, [64, 64]),
            small_fit_reference
        );
        assert!(state.source.is_none() && state.lod_active);
        assert_eq!(cpu.used(), 32 * 20 * 16);
        assert!(state.pending_region.is_none() && state.ready_region.is_none());
        drop(texture_pressure);
        let offscreen = ViewParameters {
            pan: [1e20, 0.],
            ..fit
        };
        assert!(
            RasterRenderer::visible_source_region_for_geometry(state.dimensions, state.aspect, offscreen)
                .unwrap()
                .is_none()
        );
        assert!(
            !state
                .refresh_swapped(
                    &mut renderer,
                    &gpu.device,
                    &gpu.queue,
                    offscreen,
                    &mut worker,
                    token()
                )
                .unwrap()
        );
        assert!(state.pending_region.is_none() && state.ready_region.is_none());
        assert_eq!(cpu.used(), 32 * 20 * 16);
        assert_eq!(
            gpu.render_existing_raster_half(&renderer, [64, 64]),
            small_fit_reference
        );
        assert!(
            !state
                .refresh_swapped(&mut renderer, &gpu.device, &gpu.queue, fit, &mut worker, token())
                .unwrap()
        );
        assert!(matches!(
            state.pending_region,
            Some((_, ViewportWork::Fit([64, 40])))
        ));
        wake_rx.recv_timeout(std::time::Duration::from_secs(10)).unwrap();
        assert!(
            state
                .refresh_swapped(&mut renderer, &gpu.device, &gpu.queue, fit, &mut worker, token())
                .unwrap()
        );
        renderer.update_view(&gpu.queue, fit);
        assert_eq!(gpu.render_existing_raster_half(&renderer, [64, 64]), expected_fit);
        assert_eq!(cpu.used(), 64 * 40 * 16);
        assert!(state.source.is_none());
        assert!(
            !state
                .refresh_swapped(&mut renderer, &gpu.device, &gpu.queue, fit, &mut worker, token())
                .unwrap()
        );
        assert!(state.pending_region.is_none());
        let mut recovery_timings = Vec::new();
        for _ in 0..8 {
            let pressure = texture.try_reserve(texture.available_bytes() - 20000).unwrap();
            assert!(
                !state
                    .refresh_swapped(&mut renderer, &gpu.device, &gpu.queue, fit, &mut worker, token())
                    .unwrap()
            );
            assert!(matches!(
                state.pending_region,
                Some((_, ViewportWork::Fit([32, 20])))
            ));
            assert_eq!(gpu.render_existing_raster_half(&renderer, [64, 64]), expected_fit);
            wake_rx.recv_timeout(std::time::Duration::from_secs(10)).unwrap();
            let blocked_queue = queued.try_reserve(queued.available_bytes()).unwrap();
            assert!(
                state
                    .refresh_swapped(&mut renderer, &gpu.device, &gpu.queue, fit, &mut worker, token())
                    .is_err()
            );
            assert_eq!(gpu.render_existing_raster_half(&renderer, [64, 64]), expected_fit);
            drop(blocked_queue);
            assert!(
                state
                    .refresh_swapped(&mut renderer, &gpu.device, &gpu.queue, fit, &mut worker, token())
                    .unwrap()
            );
            renderer.update_view(&gpu.queue, fit);
            assert_eq!(
                gpu.render_existing_raster_half(&renderer, [64, 64]),
                small_fit_reference
            );
            assert_eq!(cpu.used(), 32 * 20 * 16);
            drop(pressure);
            // CPU exhaustion prevents speculative rebuilding without harming the resident frame.
            let blocked_cpu = budget.try_reserve(budget.available_bytes()).unwrap();
            assert!(
                !state
                    .refresh_swapped(&mut renderer, &gpu.device, &gpu.queue, fit, &mut worker, token())
                    .unwrap()
            );
            assert!(state.pending_region.is_none());
            assert_eq!(
                gpu.render_existing_raster_half(&renderer, [64, 64]),
                small_fit_reference
            );
            drop(blocked_cpu);
            let started = std::time::Instant::now();
            assert!(
                !state
                    .refresh_swapped(&mut renderer, &gpu.device, &gpu.queue, fit, &mut worker, token())
                    .unwrap()
            );
            assert!(matches!(
                state.pending_region,
                Some((_, ViewportWork::Fit([64, 40])))
            ));
            wake_rx.recv_timeout(std::time::Duration::from_secs(10)).unwrap();
            let prepared = started.elapsed();
            let publish_started = std::time::Instant::now();
            assert!(
                state
                    .refresh_swapped(&mut renderer, &gpu.device, &gpu.queue, fit, &mut worker, token())
                    .unwrap()
            );
            renderer.update_view(&gpu.queue, fit);
            assert_eq!(gpu.render_existing_raster_half(&renderer, [64, 64]), expected_fit);
            recovery_timings.push((
                prepared.as_secs_f64() * 1000.,
                publish_started.elapsed().as_secs_f64() * 1000.,
                started.elapsed().as_secs_f64() * 1000.,
            ));
            assert_eq!(cpu.used(), 64 * 40 * 16);
            assert!(state.source.is_none() && state.pending_region.is_none() && state.ready_region.is_none());
        }
        eprintln!(
            "Metal swapped fit timings ms (worker preparation, publish+GPU render+readback, total): {recovery_timings:?}; source512x320 RGBA32F, LOD64x40, framebuffer64x64; warm filesystem; includes readback, excludes display presentation"
        );
        eprintln!(
            "swapped fit repeated pressure:8 downgrade/recovery cycles; blocked queue retains frame; blocked CPU schedules nothing; exact original HDR references; no retained full source"
        );
        eprintln!(
            "swapped fit recovery:32x20->64x40 from original disk source; exact original fit frame; no redundant job"
        );
        eprintln!(
            "swapped fit pressure: original swapped source streamed on worker to32x20; queue failure preserves old frame/ready level; retry exact; full source stays released"
        );
        drop(worker);
        drop(state);
        drop(swapped);
        drop(renderer);
        assert_eq!(
            (
                cpu.used(),
                index.used(),
                texture.used(),
                queued.used(),
                store.usage().unwrap().bytes
            ),
            (0, 0, 0, 0, 0)
        );
    }

    #[test]
    #[ignore = "actual Metal panning overscan residency and exact SDR/HDR frames"]
    fn panning_reuses_margin_with_replacement_headroom_and_exact_frames() {
        let gpu = crate::navigation_metal_readback::qualification_gpu().expect("actual GPU required");
        // A100x100 texture and its aligned queue each fit300k, but their sum does not.
        // Under400k one transfer fits, while its equally sized spare does not.
        for limit in [300000, 400000] {
            let root = MemoryBudget::new(limit);
            let renderer = RasterRenderer::new_linear_hdr_with_budget(&gpu.device, root.child(limit))
                .with_upload_queue_budget(root.child(limit));
            assert_eq!(
                renderer.can_admit_texture_extent(&gpu.device, 100, 100),
                limit == 400000
            );
            assert!(!renderer.can_admit_texture_extent_with_spare(&gpu.device, 100, 100));
            assert_eq!((root.used(), root.peak()), (0, 0));
        }
        let source = DecodedRaster::new(
            512,
            320,
            RasterPixels::Rgba32Float(
                Arc::new(
                    (0..512 * 320)
                        .flat_map(|i| {
                            [
                                (i % 23) as f32 / 8. - 0.5,
                                (i / 512) as f32 / 80.,
                                (i % 7) as f32 / 4.,
                                [0., 0.25, 0.75, 1.][i % 4],
                            ]
                        })
                        .collect::<Vec<_>>(),
                )
                .into(),
            ),
            RasterColorSpace::LinearSrgb,
        )
        .unwrap();
        for (hdr, shared) in [(false, false), (true, false), (false, true), (true, true)] {
            let root = MemoryBudget::new(600000);
            let texture = if shared {
                root.child(1048576)
            } else {
                MemoryBudget::new(1048576)
            };
            let queued = if shared {
                root.child(1048576)
            } else {
                MemoryBudget::new(1048576)
            };
            let mut renderer = if hdr {
                RasterRenderer::new_linear_hdr_with_budget(&gpu.device, texture.clone())
            } else {
                RasterRenderer::new_with_budget(
                    &gpu.device,
                    wgpu::TextureFormat::Rgba8UnormSrgb,
                    texture.clone(),
                )
            }
            .with_upload_queue_budget(queued.clone());
            let mut reference = if hdr {
                RasterRenderer::new_linear_hdr(&gpu.device)
            } else {
                RasterRenderer::new(&gpu.device, wgpu::TextureFormat::Rgba8UnormSrgb)
            };
            reference.upload(&gpu.device, &gpu.queue, &source).unwrap();
            let base = ViewParameters {
                viewport: [64., 64.],
                zoom: 16.,
                ..Default::default()
            };
            let visible = RasterRenderer::visible_source_region(&source, base)
                .unwrap()
                .unwrap();
            let mut state = ResidentSource::install(
                &mut renderer,
                &gpu.device,
                &gpu.queue,
                &source,
                base,
                None,
                &MemoryBudget::new(0),
            )
            .unwrap()
            .unwrap();
            assert!(state.region[2] > visible[2] && state.region[3] > visible[3]);
            let mut uploads = 0;
            for step in 0..48 {
                let view = ViewParameters {
                    pan: [step as f32 * 0.75, -step as f32 * 0.25],
                    ..base
                };
                uploads += usize::from(
                    state
                        .refresh_deferred(&mut renderer, &gpu.device, &gpu.queue, view)
                        .unwrap(),
                );
                renderer.update_view(&gpu.queue, view);
                reference.update_view(&gpu.queue, view);
                if hdr {
                    assert_eq!(
                        gpu.render_existing_raster_half(&renderer, [64, 64]),
                        gpu.render_existing_raster_half(&reference, [64, 64])
                    );
                } else {
                    assert_eq!(
                        gpu.render_existing_raster(&renderer, [64, 64]).pixels,
                        gpu.render_existing_raster(&reference, [64, 64]).pixels
                    );
                }
                assert_eq!(queued.used(), 0);
                assert!(texture.used() <= 1048576);
            }
            assert!(
                (1..=4).contains(&uploads),
                "48 nearby views should share their source region: {uploads}"
            );
            drop(state);
            drop(renderer);
            assert_eq!((texture.used(), queued.used(), root.used()), (0, 0, 0));
            eprintln!(
                "Metal panning margin: hdr={hdr} shared={shared};48 exact frames;{uploads} replacement uploads; final credits=0"
            );
        }
    }

    #[test]
    #[ignore = "actual Metal viewer region fallback and managed source lifetime"]
    fn viewer_region_fallback_retains_source_reuses_and_retries() {
        let gpu = crate::navigation_metal_readback::qualification_gpu().expect("actual GPU required");
        for raw in [false, true] {
            let source = DecodedRaster::new(
                512,
                320,
                RasterPixels::Rgba32Float(
                    Arc::new(
                        (0..512 * 320)
                            .flat_map(|i| {
                                [
                                    (i % 23) as f32 / 8. - 0.5,
                                    (i / 512) as f32 / 80.,
                                    (i % 7) as f32 / 4.,
                                    [0., 0.25, 0.75, 1.][(i % 4) as usize],
                                ]
                            })
                            .collect::<Vec<_>>(),
                    )
                    .into(),
                ),
                RasterColorSpace::LinearSrgb,
            )
            .unwrap();
            let cpu = MemoryBudget::new(source.pixel_capacity_bytes());
            let source = source.try_manage_pixels(&cpu).unwrap();
            let root = MemoryBudget::new(160000);
            let queue = MemoryBudget::new(160000);
            let mut renderer = RasterRenderer::new_linear_hdr_with_budget(&gpu.device, root.clone())
                .with_upload_queue_budget(queue.clone());
            let curve = raw.then(MonotoneCurve::identity);
            let base = ViewParameters {
                viewport: [64., 64.],
                zoom: 16.,
                ..Default::default()
            };
            let mut state = ResidentSource::install(
                &mut renderer,
                &gpu.device,
                &gpu.queue,
                &source,
                base,
                curve.as_ref(),
                &MemoryBudget::new(0),
            )
            .unwrap()
            .expect("whole texture cannot fit; regional fallback required");
            if let Some(curve) = &curve {
                renderer.set_raw_development(&gpu.queue, curve);
            }
            let mut whole = RasterRenderer::new_linear_hdr(&gpu.device);
            whole.upload(&gpu.device, &gpu.queue, &source).unwrap();
            if let Some(curve) = &curve {
                whole.set_raw_development(&gpu.queue, curve);
            }
            drop(source);
            assert!(cpu.used() > 0, "regional display must retain managed CPU source");
            for (index, view) in [
                base,
                ViewParameters {
                    exposure_stops: 1.25,
                    ..base
                },
                ViewParameters {
                    pan: [20.25, -15.5],
                    ..base
                },
                ViewParameters {
                    pan: [-20.5, 12.25],
                    ..base
                },
            ]
            .into_iter()
            .enumerate()
            {
                let changed = state
                    .refresh(&mut renderer, &gpu.device, &gpu.queue, view)
                    .unwrap();
                assert_eq!(changed, index >= 2);
                renderer.update_view(&gpu.queue, view);
                whole.update_view(&gpu.queue, view);
                assert_eq!(
                    gpu.render_existing_raster_half(&renderer, [64, 64]),
                    gpu.render_existing_raster_half(&whole, [64, 64])
                );
                assert_eq!(queue.used(), 0);
            }
            let old = state.region;
            let pressure = root.try_reserve(root.available_bytes()).unwrap();
            let retry = ViewParameters {
                pan: [30.5, -15.25],
                ..base
            };
            assert!(matches!(
                state.refresh(&mut renderer, &gpu.device, &gpu.queue, retry),
                Err(RasterUploadError::Memory(_))
            ));
            assert_eq!(state.region, old);
            assert_eq!(queue.used(), 0);
            drop(pressure);
            assert!(
                state
                    .refresh(&mut renderer, &gpu.device, &gpu.queue, retry)
                    .unwrap()
            );
            renderer.update_view(&gpu.queue, retry);
            whole.update_view(&gpu.queue, retry);
            assert_eq!(
                gpu.render_existing_raster_half(&renderer, [64, 64]),
                gpu.render_existing_raster_half(&whole, [64, 64])
            );
            let prior_bytes = root.used();
            assert!(
                !state
                    .refresh(
                        &mut renderer,
                        &gpu.device,
                        &gpu.queue,
                        ViewParameters {
                            pan: [10000., 0.],
                            ..base
                        }
                    )
                    .unwrap()
            );
            assert_eq!(root.used(), prior_bytes);
            // A fit requiring the whole source fails honestly; LOD is still separate.
            assert!(matches!(
                state.refresh(
                    &mut renderer,
                    &gpu.device,
                    &gpu.queue,
                    ViewParameters { zoom: 1., ..base }
                ),
                Err(RasterUploadError::Memory(_))
            ));
            drop(state);
            assert_eq!(cpu.used(), 0);
            drop(renderer);
            assert_eq!((root.used(), queue.used()), (0, 0));
            eprintln!(
                "viewer regional fallback: raw={raw}; source retained without copy; pan/exposure/retry frames exact; root/queue/source=0"
            );
        }
    }
    #[test]
    #[ignore = "actual Metal automatic fit HDR LOD and source-region switching"]
    fn viewer_automatically_opens_hdr_lod_switches_to_source_and_reuses_cached_level() {
        let gpu = crate::navigation_metal_readback::qualification_gpu().expect("actual GPU required");
        for hdr in [false, true] {
            for raw in [false, true] {
                let source = DecodedRaster::new(
                    512,
                    320,
                    RasterPixels::Rgba32Float(
                        Arc::new(
                            (0..512 * 320)
                                .flat_map(|i| {
                                    [
                                        (i % 23) as f32 / 8. - 0.5,
                                        (i / 512) as f32 / 80.,
                                        (i % 7) as f32 / 4.,
                                        [0., 0.25, 0.75, 1.][(i % 4) as usize],
                                    ]
                                })
                                .collect::<Vec<_>>(),
                        )
                        .into(),
                    ),
                    RasterColorSpace::LinearSrgb,
                )
                .unwrap();
                let source_bytes = source.pixel_capacity_bytes();
                let cpu = MemoryBudget::new(source_bytes + 100000);
                let source = source.try_manage_pixels(&cpu).unwrap();
                let lod_budget = cpu.child(100000);
                let root = MemoryBudget::new(160000);
                let queued = MemoryBudget::new(160000);
                let mut renderer = if hdr {
                    RasterRenderer::new_linear_hdr_with_budget(&gpu.device, root.clone())
                } else {
                    RasterRenderer::new_with_budget(
                        &gpu.device,
                        wgpu::TextureFormat::Rgba8UnormSrgb,
                        root.clone(),
                    )
                }
                .with_upload_queue_budget(queued.clone());
                let mut reference = if hdr {
                    RasterRenderer::new_linear_hdr(&gpu.device)
                } else {
                    RasterRenderer::new(&gpu.device, wgpu::TextureFormat::Rgba8UnormSrgb)
                };
                let curve = raw.then(MonotoneCurve::identity);
                let fit = ViewParameters {
                    viewport: [64., 64.],
                    ..Default::default()
                };
                let mut state = ResidentSource::install(
                    &mut renderer,
                    &gpu.device,
                    &gpu.queue,
                    &source,
                    fit,
                    curve.as_ref(),
                    &lod_budget,
                )
                .unwrap()
                .expect("full image requires LOD");
                assert!(state.lod_active);
                let lod = state.lod.as_ref().unwrap().clone();
                assert_eq!((lod.width(), lod.height()), (64, 40));
                assert_eq!(lod_budget.used(), 40960);
                if let Some(curve) = &curve {
                    renderer.set_raw_development(&gpu.queue, curve);
                }
                for (view, expect_upload, expect_lod) in [
                    (fit, false, true),
                    (
                        ViewParameters {
                            exposure_stops: 1.25,
                            ..fit
                        },
                        false,
                        true,
                    ),
                    (ViewParameters { zoom: 16., ..fit }, true, false),
                    (
                        ViewParameters {
                            zoom: 16.,
                            pan: [20.25, -15.5],
                            ..fit
                        },
                        true,
                        false,
                    ),
                    (fit, true, true),
                    (fit, false, true),
                ] {
                    assert_eq!(
                        state
                            .refresh(&mut renderer, &gpu.device, &gpu.queue, view)
                            .unwrap(),
                        expect_upload
                    );
                    assert_eq!(state.lod_active, expect_lod);
                    renderer.update_view(&gpu.queue, view);
                    reference
                        .upload(&gpu.device, &gpu.queue, if expect_lod { &lod } else { &source })
                        .unwrap();
                    if let Some(curve) = &curve {
                        reference.set_raw_development(&gpu.queue, curve);
                    }
                    reference.update_view(&gpu.queue, view);
                    if hdr {
                        assert_eq!(
                            gpu.render_existing_raster_half(&renderer, [64, 64]),
                            gpu.render_existing_raster_half(&reference, [64, 64])
                        );
                    } else {
                        assert_eq!(
                            gpu.render_existing_raster(&renderer, [64, 64]).pixels,
                            gpu.render_existing_raster(&reference, [64, 64]).pixels
                        );
                    }
                    assert_eq!(queued.used(), 0);
                    assert_eq!(lod_budget.used(), 40960);
                }
                // The cached level can itself exceed headroom during region replacement.
                // Build and admit a smaller level without dropping the old frame/cache first.
                let detail = ViewParameters { zoom: 16., ..fit };
                assert!(
                    state
                        .refresh(&mut renderer, &gpu.device, &gpu.queue, detail)
                        .unwrap()
                );
                renderer.update_view(&gpu.queue, detail);
                if hdr {
                    gpu.render_existing_raster_half(&renderer, [64, 64]);
                } else {
                    gpu.render_existing_raster(&renderer, [64, 64]);
                }
                let pressure = root.try_reserve(root.available_bytes() - 20000).unwrap();
                assert!(
                    state
                        .refresh(&mut renderer, &gpu.device, &gpu.queue, fit)
                        .unwrap()
                );
                assert!(state.lod_active);
                let smaller = state.lod.as_ref().unwrap();
                assert_eq!((smaller.width(), smaller.height()), (32, 20));
                renderer.update_view(&gpu.queue, fit);
                reference.upload(&gpu.device, &gpu.queue, smaller).unwrap();
                if let Some(curve) = &curve {
                    reference.set_raw_development(&gpu.queue, curve);
                }
                reference.update_view(&gpu.queue, fit);
                if hdr {
                    assert_eq!(
                        gpu.render_existing_raster_half(&renderer, [64, 64]),
                        gpu.render_existing_raster_half(&reference, [64, 64])
                    );
                } else {
                    assert_eq!(
                        gpu.render_existing_raster(&renderer, [64, 64]).pixels,
                        gpu.render_existing_raster(&reference, [64, 64]).pixels
                    );
                }
                assert_eq!(queued.used(), 0);
                assert_eq!(
                    lod_budget.used(),
                    40960 + 10240,
                    "external old LOD owner remains charged"
                );
                drop(pressure);
                drop(source);
                drop(lod);
                drop(state);
                assert_eq!(cpu.used(), 0);
                drop(renderer);
                assert_eq!((root.used(), queued.used()), (0, 0));
                eprintln!(
                    "viewer auto LOD: hdr={hdr} raw={raw}; fit->source->fit exact; cached40960 bytes reused; source/LOD/texture/queue=0"
                );
            }
        }
    }
    #[test]
    #[ignore = "actual Metal stale-generation LOD filtering and queued upload cancellation"]
    fn cancelled_generation_lod_filter_and_upload_preserve_old_frame_and_release_credit() {
        use std::sync::atomic::{AtomicU64, Ordering};
        let gpu = crate::navigation_metal_readback::qualification_gpu().expect("actual GPU required");
        let source = DecodedRaster::new(
            512,
            320,
            RasterPixels::Rgba32Float(Arc::new([2., -0.5, 0.25, 0.5].repeat(512 * 320)).into()),
            RasterColorSpace::LinearSrgb,
        )
        .unwrap();
        let old = DecodedRaster::new(
            1,
            1,
            RasterPixels::Rgba32Float(Arc::new(vec![0.25, 2., 0.5, 1.]).into()),
            RasterColorSpace::LinearSrgb,
        )
        .unwrap();
        let texture = MemoryBudget::new(160000);
        let queued = MemoryBudget::new(160000);
        let lod = MemoryBudget::new(100000);
        let mut renderer = RasterRenderer::new_linear_hdr_with_budget(&gpu.device, texture.clone())
            .with_upload_queue_budget(queued.clone());
        renderer.upload(&gpu.device, &gpu.queue, &old).unwrap();
        let fit = ViewParameters {
            viewport: [64., 64.],
            zoom: 1.,
            ..Default::default()
        };
        renderer.update_view(&gpu.queue, fit);
        let before = gpu.render_existing_raster_half(&renderer, [64, 64]);
        let generation = Arc::new(AtomicU64::new(7));
        let token = rrrah_decode::GenerationToken::new(generation.clone(), 7);
        let mut filter_checks = 0;
        let result = ResidentSource::install_with_cancel(
            &mut renderer,
            &gpu.device,
            &gpu.queue,
            &source,
            fit,
            None,
            &lod,
            || {
                if lod.used() > 0 {
                    filter_checks += 1;
                    if filter_checks >= 6 {
                        generation.store(8, Ordering::Release);
                    }
                }
                token.is_cancelled()
            },
        );
        assert!(matches!(result, Err(RasterUploadError::Cancelled)));
        assert_eq!(filter_checks, 6);
        assert_eq!((lod.used(), queued.used(), texture.used()), (0, 0, 16));
        assert_eq!(gpu.render_existing_raster_half(&renderer, [64, 64]), before);
        let mut upload_checks = 0;
        let result = ResidentSource::install_with_cancel(
            &mut renderer,
            &gpu.device,
            &gpu.queue,
            &source,
            fit,
            None,
            &lod,
            || {
                if queued.used() > 0 {
                    upload_checks += 1;
                }
                upload_checks >= 3
            },
        );
        assert!(matches!(result, Err(RasterUploadError::Cancelled)));
        assert_eq!(upload_checks, 3);
        assert_eq!(lod.used(), 0);
        gpu.device.poll(wgpu::PollType::wait_indefinitely()).unwrap();
        assert_eq!((lod.used(), queued.used(), texture.used()), (0, 0, 16));
        assert_eq!(gpu.render_existing_raster_half(&renderer, [64, 64]), before);
        let fresh = rrrah_decode::GenerationToken::new(generation, 8);
        let mut state = ResidentSource::install_with_cancel(
            &mut renderer,
            &gpu.device,
            &gpu.queue,
            &source,
            fit,
            None,
            &lod,
            || fresh.is_cancelled(),
        )
        .unwrap()
        .unwrap();
        assert!(state.lod_active);
        renderer.update_view(&gpu.queue, fit);
        let after = gpu.render_existing_raster_half(&renderer, [64, 64]);
        assert_ne!(after, before);
        let detail = ViewParameters { zoom: 16., ..fit };
        assert!(
            state
                .refresh(&mut renderer, &gpu.device, &gpu.queue, detail)
                .unwrap()
        );
        renderer.update_view(&gpu.queue, detail);
        let old_detail = gpu.render_existing_raster_half(&renderer, [64, 64]);
        let old_region = state.region;
        let old_lod_bytes = lod.used();
        let pressure = texture.try_reserve(texture.available_bytes() - 20000).unwrap();
        let mut replacement_checks = 0;
        let result = state.refresh_with_cancel(&mut renderer, &gpu.device, &gpu.queue, fit, || {
            if lod.used() > old_lod_bytes {
                replacement_checks += 1;
            }
            replacement_checks >= 6
        });
        assert!(matches!(result, Err(RasterUploadError::Cancelled)));
        assert_eq!(replacement_checks, 6);
        assert_eq!(lod.used(), old_lod_bytes);
        assert_eq!(state.region, old_region);
        assert!(!state.lod_active);
        assert_eq!(gpu.render_existing_raster_half(&renderer, [64, 64]), old_detail);
        drop(pressure);
        assert!(
            state
                .refresh(&mut renderer, &gpu.device, &gpu.queue, fit)
                .unwrap()
        );
        renderer.update_view(&gpu.queue, fit);
        gpu.render_existing_raster_half(&renderer, [64, 64]);
        drop(state);
        assert_eq!(lod.used(), 0);
        drop(renderer);
        assert_eq!((texture.used(), queued.used()), (0, 0));
        eprintln!(
            "LOD cancellation: stale generation during filter and after queued write preserve old HDR frame; fresh generation succeeds; LOD/texture/queue=0"
        );
    }
    #[test]
    #[ignore = "actual Metal worker-prepared managed HDR level publication"]
    fn worker_prepared_level_installs_without_filtering_or_scanning_full_source_on_ui() {
        use std::sync::atomic::AtomicU64;
        let gpu = crate::navigation_metal_readback::qualification_gpu().expect("actual GPU required");
        let source = DecodedRaster::new(
            512,
            320,
            RasterPixels::Rgba32Float(Arc::new([2., -0.5, 0.25, 0.5].repeat(512 * 320)).into()),
            RasterColorSpace::LinearSrgb,
        )
        .unwrap();
        let cpu = MemoryBudget::new(source.pixel_capacity_bytes() + 100000);
        let source = source.try_manage_pixels(&cpu).unwrap();
        let lod_budget = cpu.child(100000);
        let texture = MemoryBudget::new(160000);
        let queued = MemoryBudget::new(160000);
        let mut renderer = RasterRenderer::new_linear_hdr_with_budget(&gpu.device, texture.clone())
            .with_upload_queue_budget(queued.clone());
        let size = ResidentSource::plan_lod_size(&source, &lod_budget, &renderer, &gpu.device).unwrap();
        assert_eq!(size, [64, 40]);
        let generation = Arc::new(AtomicU64::new(7));
        let token = rrrah_decode::GenerationToken::new(generation, 7);
        let (wake_tx, wake_rx) = crossbeam_channel::bounded(1);
        let mut worker = crate::raster_lod_worker::LodWorker::default();
        worker.set_waker(move || {
            let _ = wake_tx.try_send(std::thread::current().name().unwrap_or("").to_string());
        });
        let id = worker
            .submit(source.clone(), size, lod_budget.clone(), 7, token.clone())
            .unwrap();
        assert_eq!(
            wake_rx.recv_timeout(std::time::Duration::from_secs(10)).unwrap(),
            "rrrah-hdr-lod"
        );
        let completed = worker.try_take().unwrap();
        assert_eq!((completed.id, completed.generation), (id, 7));
        let lod = completed.into_frame().unwrap();
        assert_eq!(lod_budget.used(), 40960);
        let expected = lod.clone();
        let fit = ViewParameters {
            viewport: [64., 64.],
            ..Default::default()
        };
        let mut callbacks = 0;
        let mut state = ResidentSource::install_with_prepared_lod(
            &mut renderer,
            &gpu.device,
            &gpu.queue,
            &source,
            fit,
            None,
            &lod_budget,
            Some(lod),
            || {
                callbacks += 1;
                token.is_cancelled()
            },
        )
        .unwrap()
        .unwrap();
        assert!(
            callbacks < 100,
            "must not scan320 source rows on UI thread: {callbacks}"
        );
        assert!(state.lod_active);
        renderer.update_view(&gpu.queue, fit);
        assert_eq!(
            gpu.render_existing_raster_half(&renderer, [64, 64]),
            gpu.render_raster_half(&expected, fit, [64, 64])
        );
        let detail = ViewParameters { zoom: 16., ..fit };
        state
            .refresh_deferred(&mut renderer, &gpu.device, &gpu.queue, detail)
            .unwrap();
        renderer.update_view(&gpu.queue, detail);
        let old_frame = gpu.render_existing_raster_half(&renderer, [64, 64]);
        state.lod = None;
        drop(expected);
        assert_eq!(lod_budget.used(), 0);
        state.lod_budget = cpu.child(40960);
        let (pixels, size, budget) = state.refresh_job(&renderer, &gpu.device, fit).unwrap().unwrap();
        assert!(matches!(
            state.refresh_deferred(&mut renderer, &gpu.device, &gpu.queue, fit),
            Err(RasterUploadError::LodUnavailable)
        ));
        assert_eq!(lod_budget.used(), 0, "UI must not filter a replacement");
        assert_eq!(gpu.render_existing_raster_half(&renderer, [64, 64]), old_frame);
        worker.submit(pixels, size, budget, 7, token).unwrap();
        wake_rx.recv_timeout(std::time::Duration::from_secs(10)).unwrap();
        let lod = worker.try_take().unwrap().into_frame().unwrap();
        assert_eq!(state.lod_budget.available_bytes(), 0);
        assert!(state.needs_lod(&renderer, &gpu.device, fit).unwrap());
        let expected = lod.clone();
        let old_region = state.region;
        let pressure = texture.try_reserve(texture.available_bytes()).unwrap();
        assert!(matches!(
            state.publish_lod(&mut renderer, &gpu.device, &gpu.queue, lod.clone()),
            Err(RasterUploadError::Memory(_))
        ));
        assert_eq!(state.region, old_region);
        assert!(!state.lod_active);
        assert!(state.lod.is_none());
        assert_eq!(gpu.render_existing_raster_half(&renderer, [64, 64]), old_frame);
        assert_eq!(state.lod_budget.used(), 40960);
        assert_eq!(queued.used(), 0);
        drop(pressure);
        let pressure = queued.try_reserve(queued.available_bytes()).unwrap();
        let texture_before = texture.used();
        assert!(matches!(
            state.publish_lod(&mut renderer, &gpu.device, &gpu.queue, lod.clone()),
            Err(RasterUploadError::Memory(_))
        ));
        assert_eq!(
            texture.used(),
            texture_before,
            "failed queue admission releases new texture credit"
        );
        assert_eq!(state.region, old_region);
        assert!(!state.lod_active);
        assert!(state.lod.is_none());
        assert_eq!(gpu.render_existing_raster_half(&renderer, [64, 64]), old_frame);
        drop(pressure);
        state
            .publish_lod(&mut renderer, &gpu.device, &gpu.queue, lod)
            .unwrap();
        renderer.update_view(&gpu.queue, fit);
        assert_eq!(
            gpu.render_existing_raster_half(&renderer, [64, 64]),
            gpu.render_raster_half(&expected, fit, [64, 64])
        );
        drop(source);
        drop(expected);
        drop(state);
        drop(worker);
        assert_eq!(cpu.used(), 0);
        drop(renderer);
        assert_eq!((texture.used(), queued.used()), (0, 0));
        eprintln!(
            "worker HDR level: real rrrah-hdr-lod thread; prepared publication skips full source scan; exact Metal HDR; source/LOD/texture/queue=0"
        );
    }
}
