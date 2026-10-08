//! Test-only PDF callback replay for authored normal-blend path fixtures.
//! Scene capture and frame allocations are not production-admitted yet.
use super::*;
use hayro_interpret::{
    BlendMode, ClipPath, Device, Paint, PathDrawMode, SoftMask, TransparencyGroupProperties,
};
use kurbo::{Affine, BezPath};

enum RecordedPaint {
    Color([u8; 4]),
    NativeColor(Box<super::native_paint::NativePaint>),
    Shading(Box<RecordedShading>),
}

struct RecordedShading {
    encoded: Box<hayro_interpret::encode::EncodedShadingPattern>,
    native: Option<hayro_interpret::encode::NativeShadingSource>,
    refusal: Option<hayro_interpret::encode::NativeShadingError>,
}
impl std::ops::Deref for RecordedShading {
    type Target = hayro_interpret::encode::EncodedShadingPattern;
    fn deref(&self) -> &Self::Target { &self.encoded }
}

struct SharedClip {
    path: std::rc::Rc<BezPath>,
    fill: hayro_interpret::FillRule,
}

enum Event {
    Blend(BlendMode),
    AlphaSource(bool),
    Mask(Option<(Box<Recorder>, bool, [u8; 4], [f32; 256], Option<std::rc::Rc<ShapeMask>>)>),
    Push(f32, TransparencyGroupProperties, BlendMode),
    Pop,
    Raster(
        hayro_interpret::ImageData,
        Option<hayro_interpret::LumaData>,
        Affine,
        Vec<ClipPath>,
        Option<u8>, // Stencil paint opacity; mask alpha represents shape.
    ),
    Path(BezPath, Affine, RecordedPaint, Vec<SharedClip>, PathDrawMode),
}

#[derive(Default)]
struct Recorder {
    blend_mode: BlendMode,
    alpha_is_shape: bool,
    width: u16,
    height: u16,
    mask_probe: bool,
    global_contour: bool,
    canonical_f32: bool,
    native_polygon: bool,
    admitted_strokes: bool,
    prepared_clips: Vec<(BezPath, std::rc::Rc<BezPath>)>,
    page_group_space: hayro_interpret::TransparencyGroupColorSpace,
    group_spaces: Vec<(usize, hayro_interpret::TransparencyGroupColorSpace)>,
    raster_spaces: Vec<(usize, Option<hayro_interpret::color::ColorSpace>)>,
    capture_native_rasters: bool,
    capture_native_shadings: bool,
    native_rasters: Vec<(usize, Option<hayro_interpret::NativeRasterComponents>, Option<hayro_interpret::NativeRasterComponentsError>)>,
    mask_results: Vec<ShapeMask>,
    native_mask_transfers: Vec<(usize, Option<hayro_interpret::TransferFunction>, Box<super::native_paint::NativePaint>, bool)>,
    events: Vec<Event>,
    clips: Vec<ClipPath>,
    failed: bool,
    operator_checks: usize,
    type3_calls: usize,
    cancelled: Option<std::sync::Arc<dyn Fn() -> bool + Send + Sync>>,
    admission: Option<std::rc::Rc<dyn Fn(usize) -> Option<Box<dyn std::any::Any + Send + Sync>>>>,
    // Scene storage drops before its retained admission guards.
    credits: Vec<Box<dyn std::any::Any + Send + Sync>>,
}

struct PathTiming {
    start: std::time::Instant,
    shape_ms: Option<f64>,
    event: usize,
    width: u16,
    height: u16,
    elements: usize,
    clips: usize,
    complete: bool,
}
impl Drop for PathTiming {
    fn drop(&mut self) {
        let total_ms = self.start.elapsed().as_secs_f64() * 1000.;
        if total_ms >= 5. || !self.complete {
            eprintln!("PROFILE_PATH {{\"event\":{},\"width\":{},\"height\":{},\"elements\":{},\"clips\":{},\"shape_ms\":{},\"total_ms\":{},\"complete\":{}}}",
                self.event, self.width, self.height, self.elements, self.clips,
                self.shape_ms.map_or("null".to_owned(), |v| v.to_string()), total_ms, self.complete);
        }
    }
}

fn cached_mask_crop_bounds(
    recorder: &Recorder,
    width: u16,
    height: u16,
    transform: Affine,
) -> Option<(usize, usize)> {
    let c = transform.as_coeffs();
    if c[..4] != [1., 0., 0., 1.]
        || c[4..].iter().any(|v| !v.is_finite() || v.fract() != 0. || *v > 0.) {
        return None;
    }
    let (x, y) = (-c[4], -c[5]);
    if x + f64::from(width) > f64::from(recorder.width)
        || y + f64::from(height) > f64::from(recorder.height) {
        return None;
    }
    Some((x as usize, y as usize))
}

impl Recorder {
    fn ready(&mut self) -> bool {
        if self.failed {
            return false;
        }
        if self.cancelled.as_ref().is_some_and(|cancelled| cancelled()) {
            self.failed = true;
            return false;
        }
        true
    }
    fn capture(&mut self, bytes: Option<usize>) -> bool {
        if !self.ready() {
            return false;
        }
        let Some(bytes) = bytes else {
            self.failed = true;
            return false;
        };
        if let Some(admit) = &self.admission {
            let Some(bytes) = bytes.checked_add(size_of::<Box<dyn std::any::Any + Send + Sync>>()) else {
                self.failed = true;
                return false;
            };
            let Some(credit) = admit(bytes) else {
                self.failed = true;
                return false;
            };
            if !self.ready() {
                return false;
            }
            if self.credits.try_reserve_exact(1).is_err() {
                self.failed = true;
                return false;
            }
            self.credits.push(credit);
        }
        true
    }
    fn prepared_contour(&mut self, path: &BezPath, transform: Affine) -> Option<BezPath> {
        if !self.capture(Some(size_of::<Box<dyn std::any::Any + Send + Sync>>())) {
            return None;
        }
        if self.credits.try_reserve_exact(1).is_err() {
            self.failed = true;
            return None;
        }
        let cancelled = self.cancelled.clone();
        let admission = self.admission.clone();
        let result = admitted_global_contour(
            path,
            transform,
            0.0001,
            1_000_000,
            &|| cancelled.as_ref().is_some_and(|check| check()),
            &|bytes| {
                admission.as_ref().map_or_else(
                    || Some(Box::new(()) as Box<dyn std::any::Any + Send + Sync>),
                    |admit| admit(bytes),
                )
            },
        );
        let Some(mut result) = result else {
            self.failed = true;
            return None;
        };
        if !self.ready() {
            return None;
        }
        if self.canonical_f32 {
            for element in result.path.elements_mut() {
                if !self.ready() {
                    return None;
                }
                let point = match element {
                    kurbo::PathEl::MoveTo(point) | kurbo::PathEl::LineTo(point) => point,
                    kurbo::PathEl::ClosePath => continue,
                    _ => {
                        self.failed = true;
                        return None;
                    }
                };
                point.x = f64::from(point.x as f32);
                point.y = f64::from(point.y as f32);
                if !point.x.is_finite() || !point.y.is_finite() {
                    self.failed = true;
                    return None;
                }
            }
        }
        let AdmittedContour { path, _credit } = result;
        self.credits.push(_credit);
        Some(path)
    }

    fn prepared_stroke(&mut self, path: &BezPath, transform: Affine, mode: &PathDrawMode) -> Option<BezPath> {
        let PathDrawMode::Stroke(props) = mode else {
            return None;
        };
        let bytes = props
            .dash_array
            .len()
            .checked_mul(size_of::<f64>())?
            .checked_add(size_of::<Box<dyn std::any::Any + Send + Sync>>())?;
        if !self.capture(Some(bytes)) {
            return None;
        }
        if self.credits.try_reserve_exact(1).is_err() {
            self.failed = true;
            return None;
        }
        let stroke = kurbo::Stroke {
            width: f64::from(props.line_width),
            join: props.line_join,
            miter_limit: f64::from(props.miter_limit),
            start_cap: props.line_cap,
            end_cap: props.line_cap,
            dash_pattern: props.dash_array.iter().map(|&v| f64::from(v)).collect(),
            dash_offset: f64::from(props.dash_offset),
        };
        let cancelled = self.cancelled.clone();
        let admission = self.admission.clone();
        let cancel = || cancelled.as_ref().is_some_and(|check| check());
        let admit = |bytes| {
            admission.as_ref().map_or_else(
                || Some(Box::new(()) as Box<dyn std::any::Any + Send + Sync>),
                |admit| admit(bytes),
            )
        };
        let Some(plan) = bounded_stroke::admitted_stroke_runs(
            path, &stroke, transform, 0.0001, 1_000_000, 1_000_000, 20_000_000, &cancel, &admit,
        ) else {
            self.failed = true;
            return None;
        };
        let Some(outline) = bounded_stroke_outline::admitted_stroke_outline(
            &plan, &stroke, transform, 0.0001, 1_000_000, 20_000_000, &cancel, &admit,
        ) else {
            self.failed = true;
            return None;
        };
        drop(plan);
        if !self.ready() {
            return None;
        }
        let bounded_stroke_outline::StrokeOutline { path, _credit } = outline;
        self.credits.push(_credit);
        Some(path)
    }

    fn shared_clip(&mut self, clip: ClipPath, prepare: bool) -> Option<SharedClip> {
        if !self.ready() {
            return None;
        }
        if prepare {
            // Compare complete geometry, never accept a hash collision as identity.
            if let Some((_, path)) = self
                .prepared_clips
                .iter()
                .find(|(original, _)| original.elements() == clip.path.elements())
            {
                return Some(SharedClip {
                    path: path.clone(),
                    fill: clip.fill,
                });
            }
            let bytes = clip
                .path
                .elements()
                .len()
                .checked_mul(size_of::<kurbo::PathEl>())
                .and_then(|n| n.checked_add(size_of::<(BezPath, std::rc::Rc<BezPath>)>()))
                .and_then(|n| n.checked_add(size_of::<BezPath>() + 2 * size_of::<usize>()));
            if !self.capture(bytes) {
                return None;
            }
            if self.prepared_clips.try_reserve_exact(1).is_err() {
                self.failed = true;
                return None;
            }
            let prepared = self.prepared_contour(&clip.path, Affine::IDENTITY)?;
            let path = std::rc::Rc::new(prepared);
            self.prepared_clips.push((clip.path, path.clone()));
            Some(SharedClip {
                path,
                fill: clip.fill,
            })
        } else {
            if !self.capture(Some(size_of::<BezPath>() + 2 * size_of::<usize>())) {
                return None;
            }
            Some(SharedClip {
                path: std::rc::Rc::new(clip.path),
                fill: clip.fill,
            })
        }
    }

    fn event(&mut self, extra: usize) -> bool {
        if !self.capture(size_of::<Event>().checked_add(extra)) {
            return false;
        }
        if self.events.try_reserve_exact(1).is_err() {
            self.failed = true;
            return false;
        }
        true
    }
}

impl<'a> Device<'a> for Recorder {
    fn should_stop(&mut self) -> bool {
        self.operator_checks += 1;
        !self.ready()
    }

    fn set_alpha_source(&mut self, alpha_is_shape: bool) {
        if self.alpha_is_shape != alpha_is_shape && self.event(0) {
            self.alpha_is_shape = alpha_is_shape;
            self.events.push(Event::AlphaSource(alpha_is_shape));
        }
    }

    fn set_soft_mask(&mut self, mask: Option<SoftMask<'a>>) {
        if mask.is_none() && !self.mask_probe {
            return;
        }
        if !self.ready() {
            return;
        }
        if let Some(mask) = mask {
            assert!(self.mask_probe, "mask application unsupported by experiment");
            if !self.event(size_of::<Recorder>() + size_of::<[f32; 256]>() + size_of::<ShapeMask>()) {
                return;
            }
            let mut scene = Recorder {
                width: self.width,
                height: self.height,
                mask_probe: self.mask_probe,
                global_contour: self.global_contour,
                canonical_f32: self.canonical_f32,
                native_polygon: self.native_polygon,
                admitted_strokes: self.admitted_strokes,
                capture_native_shadings: self.capture_native_shadings,
                admission: self.admission.clone(),
                cancelled: self.cancelled.clone(),
                ..Recorder::default()
            };
            mask.interpret(&mut scene);
            if scene.failed || !self.ready() {
                self.failed = true;
                return;
            }
            let backdrop = if mask.mask_type() == hayro_interpret::MaskType::Luminosity {
                mask.background_color().to_rgba().to_rgba8()
            } else {
                [0; 4]
            };
            let cancelled = self.cancelled.clone();
            let admission = self.admission.clone();
            let cancel = || cancelled.as_ref().is_some_and(|check| check());
            let admit = |bytes| {
                admission.as_ref().map_or_else(
                    || Some(Box::new(()) as Box<dyn std::any::Any + Send + Sync>),
                    |f| f(bytes),
                )
            };
            let Some(pixels) = replay_scene_with_backdrop(
                &scene,
                self.width,
                self.height,
                Affine::IDENTITY,
                backdrop,
                &cancel,
                &admit,
            ) else {
                self.failed = true;
                return;
            };
            let Some(values) = soft_mask_values(
                &pixels.pixels,
                mask.mask_type() == hayro_interpret::MaskType::Luminosity,
                &|v| mask.transfer_function().map_or(v, |f| f.apply(v)),
                &cancel,
                &admit,
            ) else {
                self.failed = true;
                return;
            };
            if self.mask_results.try_reserve_exact(1).is_err() {
                self.failed = true;
                return;
            }
            self.mask_results.push(values);
            let mut transfer = [0.0; 256];
            for (i, v) in transfer.iter_mut().enumerate() {
                if i % 16 == 0 && !self.ready() {
                    return;
                }
                let sample = i as f32 / 255.0;
                *v = mask.transfer_function().map_or(sample, |f| f.apply(sample));
            }
            if !self.ready() {
                return;
            }
            let cached = if self.native_polygon {
                if !self.capture(Some(size_of::<ShapeMask>() + 2 * size_of::<usize>())) {
                    return;
                }
                let Some(values) = soft_mask_values(
                    &pixels.pixels,
                    mask.mask_type() == hayro_interpret::MaskType::Luminosity,
                    &|v| transfer[(v.clamp(0.0, 1.0) * 255.0).round() as usize],
                    &cancel,
                    &admit,
                ) else {
                    self.failed = true;
                    return;
                };
                Some(std::rc::Rc::new(values))
            } else {
                None
            };
            if !self.capture(Some(size_of::<(usize, Option<hayro_interpret::TransferFunction>, Box<super::native_paint::NativePaint>, bool)>())) { return; }
            if self.native_mask_transfers.try_reserve_exact(1).is_err() { self.failed = true; return; }
            let Some(native_backdrop) = super::native_paint::NativePaint::capture(&mask.background_color(),&cancel,&admit) else {
                self.failed = true; return;
            };
            self.native_mask_transfers.push((self.events.len(),mask.transfer_function().cloned(),native_backdrop,mask.uses_device_luminosity()));
            self.events.push(Event::Mask(Some((
                Box::new(scene),
                mask.mask_type() == hayro_interpret::MaskType::Luminosity,
                backdrop,
                transfer,
                cached,
            ))));
        } else if self.mask_probe && self.event(0) {
            self.events.push(Event::Mask(None));
        }
    }
    fn set_blend_mode(&mut self, mode: BlendMode) {
        if self.blend_mode != mode && self.event(0) {
            self.blend_mode = mode;
            self.events.push(Event::Blend(mode));
        }
    }
    fn push_clip_path(&mut self, clip: &ClipPath) {
        let bytes = clip
            .path
            .elements()
            .len()
            .checked_mul(size_of::<kurbo::PathEl>())
            .and_then(|n| n.checked_add(size_of::<ClipPath>()));
        if !self.capture(bytes) {
            return;
        }
        if self.clips.try_reserve_exact(1).is_err() {
            self.failed = true;
            return;
        }
        self.clips.push(clip.clone());
    }
    fn pop_clip_path(&mut self) {
        if !self.failed {
            self.clips.pop().unwrap();
        }
    }
    fn push_transparency_group(&mut self, opacity: f32, mask: Option<SoftMask<'a>>, mode: BlendMode) {
        self.push_transparency_group_with_properties(
            opacity,
            mask,
            mode,
            TransparencyGroupProperties::default(),
        );
    }
    fn set_page_group_color_space(&mut self, space: &hayro_interpret::TransparencyGroupColorSpace) {
        if matches!(space, hayro_interpret::TransparencyGroupColorSpace::Inherited) {
            self.page_group_space = space.clone();
        } else if self.capture(Some(size_of::<hayro_interpret::TransparencyGroupColorSpace>())) {
            self.page_group_space = space.clone();
        }
    }
    fn push_transparency_group_with_color_space(
        &mut self, opacity: f32, mask: Option<SoftMask<'a>>, mode: BlendMode,
        properties: TransparencyGroupProperties,
        space: &hayro_interpret::TransparencyGroupColorSpace,
    ) {
        if !self.capture(Some(size_of::<(usize, hayro_interpret::TransparencyGroupColorSpace)>())) { return; }
        if self.group_spaces.try_reserve_exact(1).is_err() { self.failed = true; return; }
        self.push_transparency_group_with_properties(opacity, mask, mode, properties);
        if self.failed { return; }
        let index = self.events.len().checked_sub(1).unwrap();
        if !matches!(self.events[index], Event::Push(_, _, _)) { self.failed = true; return; }
        self.group_spaces.push((index, space.clone()));
    }
    fn push_transparency_group_with_properties(
        &mut self,
        opacity: f32,
        mask: Option<SoftMask<'a>>,
        mode: BlendMode,
        properties: TransparencyGroupProperties,
    ) {
        self.set_soft_mask(mask);
        if self.event(0) {
            self.events.push(Event::Push(opacity, properties, mode));
        }
    }
    fn pop_transparency_group(&mut self) {
        if self.event(0) {
            self.events.push(Event::Pop);
        }
    }
    fn draw_path(&mut self, path: &BezPath, transform: Affine, paint: &Paint<'a>, mode: &PathDrawMode) {
        if !self.ready() {
            return;
        }
        let clip_bytes = self.clips.iter().try_fold(0usize, |total, clip| {
            clip.path
                .elements()
                .len()
                .checked_mul(size_of::<kurbo::PathEl>())?
                .checked_add(size_of::<ClipPath>())?
                .checked_add(total)
        });
        let dash_bytes = match mode {
            PathDrawMode::Stroke(props) => props.dash_array.len().checked_mul(size_of::<f32>()),
            _ => Some(0),
        };
        let bytes = path
            .elements()
            .len()
            .checked_mul(size_of::<kurbo::PathEl>())
            .and_then(|n| n.checked_add(clip_bytes?))
            .and_then(|n| n.checked_add(dash_bytes?));
        let Some(bytes) = bytes else {
            self.failed = true;
            return;
        };
        if !self.event(bytes) {
            return;
        }
        let mut clips = self.clips.clone();
        let paint = match paint {
            Paint::Color(color) => {
                let cancelled = self.cancelled.clone();
                let admission = self.admission.clone();
                let Some(paint) = super::native_paint::NativePaint::capture(
                    color, &|| cancelled.as_ref().is_some_and(|f| f()),
                    &|bytes| admission.as_ref().map_or_else(
                        || Some(Box::new(()) as Box<dyn std::any::Any + Send + Sync>),
                        |admit| admit(bytes),
                    ),
                ) else { self.failed = true; return; };
                RecordedPaint::NativeColor(paint)
            },
            Paint::Pattern(pattern) => {
                let hayro_interpret::pattern::Pattern::Shading(shading) = pattern.as_ref() else {
                    panic!("tiling paint unsupported by experiment")
                };
                // get_paint has already transformed the shading BBox into device space.
                if let Some(path) = &shading.shading.clip_path {
                    let bytes = path
                        .elements()
                        .len()
                        .checked_mul(size_of::<kurbo::PathEl>())
                        .and_then(|n| n.checked_add(size_of::<ClipPath>()));
                    if !self.capture(bytes) {
                        return;
                    }
                    clips.push(ClipPath {
                        path: path.clone(),
                        fill: hayro_interpret::FillRule::NonZero,
                    });
                }
                if !self.capture(Some(size_of::<hayro_interpret::encode::EncodedShadingPattern>() + size_of::<RecordedShading>())) {
                    return;
                }
                let mut native = None;
                let mut refusal = None;
                if self.capture_native_shadings {
                    if std::env::var_os("RRRAH_TRACE_NATIVE_REPLAY_EVENT").is_some() {
                        let (kind,function_kind,function_count)=match shading.shading.shading_type.as_ref() {
                            hayro_interpret::shading::ShadingType::RadialAxial {axial,function,..}=> {
                                let (function_kind,count)=match function {
                                    hayro_interpret::shading::ShadingFunction::Single(f)=>(Some(f.source_type_number()),1),
                                    hayro_interpret::shading::ShadingFunction::Multiple(f)=>(None,f.len()),
                                };
                                (if *axial {"axial"} else {"radial"},function_kind,count)
                            },
                            hayro_interpret::shading::ShadingType::FunctionBased {..}=>("function-based",None,0),
                            hayro_interpret::shading::ShadingType::TriangleMesh {..}=>("triangle-mesh",None,0),
                            hayro_interpret::shading::ShadingType::CoonsPatchMesh {..}=>("coons-mesh",None,0),
                            hayro_interpret::shading::ShadingType::TensorProductPatchMesh {..}=>("tensor-mesh",None,0),
                            hayro_interpret::shading::ShadingType::Dummy=>("dummy",None,0),
                        };
                        eprintln!("NATIVE_SHADING_SOURCE kind={kind} model={:?} device={} transfer={} function_kind={function_kind:?} function_count={function_count}",
                            shading.shading.color_space.blending_model(),shading.shading.color_space.native_device_retained_capacity().is_some(),shading.transfer_function.is_some());
                    }
                    let result = hayro_interpret::encode::NativeShadingSource::capture(shading,
                        &|| self.cancelled.as_ref().is_some_and(|check| check()),
                        &|bytes| self.admission.as_ref().map_or_else(
                            || Some(Box::new(()) as Box<dyn std::any::Any + Send + Sync>), |admit| admit(bytes)));
                    match result {
                        Ok(source) => native = Some(source),
                        Err(error @ (hayro_interpret::encode::NativeShadingError::Cancelled | hayro_interpret::encode::NativeShadingError::Admission)) => {
                            let _ = error; self.failed = true; return;
                        }
                        Err(error) => refusal = Some(error),
                    }
                }
                let cancelled = self.cancelled.clone();
                let encoded = shading.encode_with_sample_bounds_cancel_and_admission(
                    Some(kurbo::Rect::new(
                        0.0,
                        0.0,
                        f64::from(self.width),
                        f64::from(self.height),
                    )),
                    &|| cancelled.as_ref().is_some_and(|check| check()),
                    &|bytes| {
                        self.admission.as_ref().map_or_else(
                            || Some(Box::new(()) as Box<dyn std::any::Any + Send + Sync>),
                            |admit| admit(bytes),
                        )
                    },
                );
                let Some(encoded) = encoded else {
                    self.failed = true;
                    return;
                };
                RecordedPaint::Shading(Box::new(RecordedShading { encoded:Box::new(encoded), native, refusal }))
            }
        };
        let (path, transform, mode) = if self.global_contour && matches!(mode, PathDrawMode::Fill(_)) {
            let Some(path) = self.prepared_contour(path, transform) else {
                return;
            };
            (path, Affine::IDENTITY, mode.clone())
        } else if self.global_contour && self.admitted_strokes && matches!(mode, PathDrawMode::Stroke(_)) {
            let Some(path) = self.prepared_stroke(path, transform, mode) else {
                return;
            };
            (
                path,
                Affine::IDENTITY,
                PathDrawMode::Fill(hayro_interpret::FillRule::NonZero),
            )
        } else {
            (path.clone(), transform, mode.clone())
        };
        let prepare = self.global_contour && matches!(mode, PathDrawMode::Fill(_));
        let Some(clips) = clips
            .into_iter()
            .map(|clip| self.shared_clip(clip, prepare))
            .collect::<Option<Vec<_>>>()
        else {
            return;
        };
        self.events
            .push(Event::Path(path, transform, paint, clips, mode.clone()));
    }
    fn draw_glyph(
        &mut self,
        glyph: &hayro_interpret::font::Glyph<'a>,
        transform: Affine,
        glyph_transform: Affine,
        paint: &Paint<'a>,
        mode: &hayro_interpret::GlyphDrawMode,
    ) {
        if !self.ready() || matches!(mode, hayro_interpret::GlyphDrawMode::Invisible) {
            return;
        }
        let glyph = match glyph {
            hayro_interpret::font::Glyph::Outline(glyph) => glyph,
            hayro_interpret::font::Glyph::Type3(glyph) => {
                self.type3_calls += 1;
                glyph.interpret(self, transform, glyph_transform, paint);
                return;
            }
        };
        // Outline extraction itself still needs bounded font admission before
        // production use; draw_path admits the retained recorded copy.
        let path = glyph.outline();
        match mode {
            hayro_interpret::GlyphDrawMode::Fill => self.draw_path(
                &path,
                transform * glyph_transform,
                paint,
                &PathDrawMode::Fill(hayro_interpret::FillRule::NonZero),
            ),
            hayro_interpret::GlyphDrawMode::Stroke(props) => self.draw_path(
                &(glyph_transform * path),
                transform,
                paint,
                &PathDrawMode::Stroke(props.clone()),
            ),
            hayro_interpret::GlyphDrawMode::Invisible => unreachable!(),
        }
    }
    fn draw_image(&mut self, image: hayro_interpret::Image<'a, '_>, transform: Affine) {
        if !self.ready() {
            return;
        }
        let Some(count) = (image.width() as usize).checked_mul(image.height() as usize) else {
            self.failed = true;
            return;
        };
        let clips = self.clips.iter().try_fold(0usize, |sum, clip| {
            clip.path
                .elements()
                .len()
                .checked_mul(size_of::<kurbo::PathEl>())?
                .checked_add(size_of::<ClipPath>())?
                .checked_add(sum)
        });
        let Some(bytes) = count.checked_mul(4).and_then(|n| n.checked_add(clips?)) else {
            self.failed = true;
            return;
        };
        if !self.event(bytes) {
            return;
        }
        let cancelled = self.cancelled.clone();
        let mut published = false;
        match image {
            hayro_interpret::Image::Raster(raster) => {
                let admission = self.admission.clone();
                let (native,refusal) = if self.capture_native_rasters {
                    match raster.native_components_checked(&|| cancelled.as_ref().is_some_and(|check| check()),
                        &|bytes| admission.as_ref().map_or_else(|| Some(Box::new(()) as Box<dyn std::any::Any+Send+Sync>),|admit| admit(bytes))) {
                        Ok(data) => (Some(data),None), Err(reason) => (None,Some(reason)),
                    }
                } else { (None,None) };
                if refusal==Some(hayro_interpret::NativeRasterComponentsError::Cancelled) {
                    self.failed=true;
                    return;
                }
                raster.with_rgba_and_cancel(
                |image, alpha| {
                    if !self.ready() {
                        return;
                    }
                    if (image.width() as usize).checked_mul(image.height() as usize) != Some(count)
                        || alpha
                            .as_ref()
                            .is_some_and(|a| (a.width as usize).checked_mul(a.height as usize) != Some(count))
                    {
                        self.failed = true;
                        return;
                    }
                    if !self.capture(Some(size_of::<(usize,Option<hayro_interpret::color::ColorSpace>)>())) { return; }
                    if self.raster_spaces.try_reserve_exact(1).is_err() { self.failed=true; return; }
                    self.raster_spaces.push((self.events.len(),raster.color_space().cloned()));
                    if self.capture_native_rasters {
                        if !self.capture(Some(size_of::<(usize,Option<hayro_interpret::NativeRasterComponents>,Option<hayro_interpret::NativeRasterComponentsError>)>())) { return; }
                        if self.native_rasters.try_reserve_exact(1).is_err() { self.failed=true; return; }
                        self.native_rasters.push((self.events.len(),native,refusal));
                    }
                    self.events
                        .push(Event::Raster(image, alpha, transform, self.clips.clone(), None));
                    published = true;
                },
                None,
                &|| cancelled.as_ref().is_some_and(|check| check()),
            )
            },
            hayro_interpret::Image::Stencil(stencil) => stencil.with_stencil_and_cancel(
                |mask, paint| {
                    if !self.ready() {
                        return;
                    }
                    let Paint::Color(color) = paint else {
                        self.failed = true;
                        return;
                    };
                    let color = color.to_rgba().to_rgba8();
                    if mask.data.len() != count {
                        self.failed = true;
                        return;
                    }
                    let mut rgb = Vec::with_capacity(count * 3);
                    for index in 0..count {
                        if index % 256 == 0 && !self.ready() {
                            return;
                        }
                        rgb.extend_from_slice(&color[..3]);
                    }
                    let image = hayro_interpret::ImageData::Rgb(hayro_interpret::RgbData {
                        data: rgb,
                        width: mask.width,
                        height: mask.height,
                        interpolate: mask.interpolate,
                        scale_factors: mask.scale_factors,
                    });
                    self.events.push(Event::Raster(
                        image,
                        Some(mask),
                        transform,
                        self.clips.clone(),
                        Some(color[3]),
                    ));
                    published = true;
                },
                None,
                &|| cancelled.as_ref().is_some_and(|check| check()),
            ),
        }
        if !published {
            self.failed = true;
        }
    }
}

fn authored_pdf(knockout: bool, blue_alpha: f32, yellow: bool) -> Vec<u8> {
    authored_pdf_with_content(
        knockout,
        blue_alpha,
        if yellow {
            "1 1 0 rg 0 0 32 16 re f\n/form Do"
        } else {
            "/form Do"
        },
    )
}

fn authored_pdf_with_content(knockout: bool, blue_alpha: f32, content: &str) -> Vec<u8> {
    authored_pdf_with_blue(knockout, blue_alpha, content, "0 0 1 rg 8 0 24 16 re f")
}

fn authored_pdf_with_blue(knockout: bool, blue_alpha: f32, content: &str, blue: &str) -> Vec<u8> {
    authored_pdf_with_paths(knockout, blue_alpha, content, "1 0 0 rg 0 0 24 16 re f", blue)
}

fn authored_pdf_with_paths(knockout: bool, blue_alpha: f32, content: &str, red: &str, blue: &str) -> Vec<u8> {
    authored_pdf_with_font(
        knockout,
        blue_alpha,
        content,
        red,
        blue,
        "<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>",
        None,
    )
}

fn authored_pdf_with_font(
    knockout: bool,
    blue_alpha: f32,
    content: &str,
    red: &str,
    blue: &str,
    font: &str,
    glyph: Option<&str>,
) -> Vec<u8> {
    authored_pdf_with_font_alpha_source(knockout, blue_alpha, content, red, blue, font, glyph, false)
}

fn authored_pdf_with_font_alpha_source(
    knockout: bool,
    blue_alpha: f32,
    content: &str,
    red: &str,
    blue: &str,
    font: &str,
    glyph: Option<&str>,
    alpha_is_shape: bool,
) -> Vec<u8> {
    authored_pdf_with_font_blend(
        knockout,
        blue_alpha,
        content,
        red,
        blue,
        font,
        glyph,
        alpha_is_shape,
        "Normal",
    )
}

fn authored_pdf_with_font_blend(
    knockout: bool,
    blue_alpha: f32,
    content: &str,
    red: &str,
    blue: &str,
    font: &str,
    glyph: Option<&str>,
    alpha_is_shape: bool,
    blend_mode: &str,
) -> Vec<u8> {
    authored_pdf_with_font_blend_and_blue_space(knockout,blue_alpha,content,red,blue,font,glyph,alpha_is_shape,blend_mode,None)
}

fn authored_pdf_with_font_blend_and_blue_space(
    knockout: bool, blue_alpha: f32, content: &str, red: &str, blue: &str,
    font: &str, glyph: Option<&str>, alpha_is_shape: bool, blend_mode: &str,
    blue_space: Option<(&str,bool)>,
) -> Vec<u8> {
    let declared = blue_space.map_or(String::new(), |(space,_)| format!(" /CS {space}"));
    let blue_isolated = blue_space.map_or(true, |(_,isolated)| isolated);
    let font_resources=if font.starts_with("<< /Type /XObject /Subtype /Image") { "/XObject << /image 9 0 R >>" } else { "/Font << /F1 9 0 R >>" };
    let stream = |dict: &str, content: &str| {
        format!(
            "<< {dict} /Length {} >>\nstream\n{content}\nendstream",
            content.len()
        )
    };
    let mut objects = vec![
        "<< /Type /Catalog /Pages 2 0 R >>".to_owned(),
        "<< /Type /Pages /Kids [3 0 R] /Count 1 >>".to_owned(),
        "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 32 16] /Resources << /XObject << /form 6 0 R >> >> /Contents 4 0 R >>".to_owned(),
        stream("", content),
        format!("<< /Type /ExtGState /ca {blue_alpha} /AIS {alpha_is_shape} /BM /{blend_mode} >>"),
        stream(&format!("/Type /XObject /Subtype /Form /BBox [0 0 32 16] /Group << /S /Transparency /I false /K {knockout} >> /Resources << /ExtGState << /half 5 0 R >> /XObject << /red 7 0 R /blue 8 0 R >> >>"), "q /red Do Q\nq /half gs /blue Do Q"),
        stream(&format!("/Type /XObject /Subtype /Form /BBox [0 0 32 16] /Group << /S /Transparency /I true /K false >> /Resources << {font_resources} /ExtGState << /half 5 0 R >> /Shading << /S1 10 0 R >> >>"), red),
        stream(&format!("/Type /XObject /Subtype /Form /BBox [0 0 32 16] /Group << /S /Transparency /I {blue_isolated} /K false{declared} >> /Resources << {font_resources} /ExtGState << /half 5 0 R >> /Shading << /S1 10 0 R >> >>"), blue),
        font.to_owned(),
    ];
    if let Some(glyph) = glyph {
        objects.push(if glyph.starts_with("<< /ShadingType") || glyph.starts_with("<< /FunctionType") || glyph.starts_with("<< /Type /XObject") {
            glyph.to_owned()
        } else {
            stream("", glyph)
        });
    }
    let mut pdf = b"%PDF-1.4\n".to_vec();
    let mut offsets = vec![0];
    for (index, object) in objects.iter().enumerate() {
        offsets.push(pdf.len());
        pdf.extend_from_slice(format!("{} 0 obj\n{object}\nendobj\n", index + 1).as_bytes());
    }
    let xref = pdf.len();
    pdf.extend_from_slice(format!("xref\n0 {}\n0000000000 65535 f \n", offsets.len()).as_bytes());
    for offset in &offsets[1..] {
        pdf.extend_from_slice(format!("{offset:010} 00000 n \n").as_bytes());
    }
    pdf.extend_from_slice(
        format!(
            "trailer\n<< /Size {} /Root 1 0 R >>\nstartxref\n{xref}\n%%EOF\n",
            offsets.len()
        )
        .as_bytes(),
    );
    pdf
}

#[test]
fn device_conversion_functions_follow_precedence_and_restore_after_form() {
    use hayro_interpret::color::DeviceConversionFunction;
    let scene = record(authored_pdf_with_font_blend(
        false, 1.0, "/form Do 0 0 0 rg 0 0 1 1 re f",
        "1 0 0 rg 0 0 4 4 re f", "0 0 1 rg 0 0 4 4 re f",
        "<< >>", None, false,
        "Normal /BG << /FunctionType 2 /Domain [0 1] /C0 [0] /C1 [1] /N 1 >> /BG2 << /FunctionType 2 /Domain [0 1] /C0 [0.1] /C1 [0.3] /N 1 >> /UCR null /UCR2 /Default",
    ));
    assert!(!scene.failed);
    let paints: Vec<_> = scene.events.iter().filter_map(|event| match event {
        Event::Path(_, _, RecordedPaint::NativeColor(paint), _, _) => Some(paint),
        _ => None,
    }).collect();
    assert_eq!(paints.len(), 3);
    assert!(matches!(paints[0].conversion_functions.black_generation, DeviceConversionFunction::Default));
    let DeviceConversionFunction::Function(function) = &paints[1].conversion_functions.black_generation else { panic!("BG2 missing") };
    let output = function.eval([0.5].into_iter().collect()).unwrap();
    assert_eq!(output.len(), 1);
    assert!((output[0] - 0.2).abs() < 1e-7);
    assert!(matches!(paints[1].conversion_functions.undercolor_removal, DeviceConversionFunction::Default));
    assert!(matches!(paints[2].conversion_functions.black_generation, DeviceConversionFunction::Default));
    let invalid = record(authored_pdf_with_font_blend(
        false, 1.0, "/form Do", "1 0 0 rg 0 0 4 4 re f", "0 0 1 rg 0 0 4 4 re f",
        "<< >>", None, false, "Normal /BG2 /Identity /UCR2 null",
    ));
    assert!(!invalid.failed);
    let paints: Vec<_> = invalid.events.iter().filter_map(|event| match event {
        Event::Path(_, _, RecordedPaint::NativeColor(paint), _, _) => Some(paint),
        _ => None,
    }).collect();
    assert_eq!(paints.len(), 2);
    assert!(matches!(paints[1].conversion_functions.black_generation, DeviceConversionFunction::Unresolved));
    assert!(matches!(paints[1].conversion_functions.undercolor_removal, DeviceConversionFunction::Unresolved));
    let explicit = record(authored_pdf_with_font_blend(
        false, 1.0, "/form Do", "0 0 0 0 k 0 0 4 4 re f", "0.2 0.4 0.6 rg 0 0 4 4 re f",
        "<< >>", None, false,
        "Normal /BG2 << /FunctionType 3 /Domain [0 1] /Functions [ << /FunctionType 2 /Domain [0 1] /C0 [0.1] /C1 [0.2] /N 1 >> << /FunctionType 2 /Domain [0 1] /C0 [0.2] /C1 [0.3] /N 1 >> ] /Bounds [0.5] /Encode [0 1 0 1] >> /UCR2 << /FunctionType 2 /Domain [0 1] /C0 [0] /C1 [1] /N 1 >>",
    ));
    assert!(!explicit.failed);
    let paints: Vec<_> = explicit.events.iter().filter_map(|event| match event {
        Event::Path(_, _, RecordedPaint::NativeColor(paint), _, _) => Some(paint),
        _ => None,
    }).collect();
    assert_eq!(paints.len(), 2);
    let pixel = paints[1].pixel_in_space(&paints[0].color_space, 0.5).unwrap();
    assert_eq!(pixel.alpha(), 0.5);
    for (actual, expected) in pixel.native_components().unwrap().into_iter().zip([0.4,0.2,0.0,0.18]) {
        assert!((actual-expected).abs() < 1e-7);
    }
    let sampled = record(authored_pdf_with_font_blend(
        false, 1.0, "/form Do", "0 0 0 0 k 0 0 4 4 re f", "0.2 0.4 0.6 rg 0 0 4 4 re f",
        "<< >>", Some("<< /FunctionType 0 /Domain [0 1] /Range [0 1] /Size [2] /BitsPerSample 8 /Order 1 /Filter /ASCIIHexDecode /Length 5 >>\nstream\n00ff>\nendstream"), false,
        "Normal /BG2 10 0 R /UCR2 << /FunctionType 2 /Domain [0 1] /C0 [0] /C1 [1] /N 1 >>",
    ));
    assert!(!sampled.failed);
    let paints: Vec<_> = sampled.events.iter().filter_map(|event| match event {
        Event::Path(_, _, RecordedPaint::NativeColor(paint), _, _) => Some(paint),
        _ => None,
    }).collect();
    assert_eq!(paints.len(), 2);
    let pixel = paints[1].pixel_in_space(&paints[0].color_space, 0.5).unwrap();
    assert_eq!(pixel.alpha(), 0.5);
    for (actual, expected) in pixel.native_components().unwrap().into_iter().zip([0.4,0.2,0.0,0.4]) {
        assert!((actual-expected).abs() < 1e-7);
    }

    let calculator = record(authored_pdf_with_font_blend(
        false, 1.0, "/form Do", "0 0 0 0 k 0 0 4 4 re f", "0.2 0.4 0.6 rg 0 0 4 4 re f",
        "<< >>", Some("<< /FunctionType 4 /Domain [0 1] /Range [0 1] /Length 19 >>\nstream\n{ 0.2 mul 0.1 add }\nendstream"), false,
        "Normal /BG2 10 0 R /UCR2 << /FunctionType 2 /Domain [0 1] /C0 [0] /C1 [1] /N 1 >>",
    ));
    assert!(!calculator.failed);
    let paints: Vec<_> = calculator.events.iter().filter_map(|event| match event {
        Event::Path(_, _, RecordedPaint::NativeColor(paint), _, _) => Some(paint),
        _ => None,
    }).collect();
    assert_eq!(paints.len(), 2);
    let pixel = paints[1].pixel_in_space(&paints[0].color_space, 0.5).unwrap();
    assert_eq!(pixel.alpha(),0.5);
    for (actual, expected) in pixel.native_components().unwrap().into_iter().zip([0.4,0.2,0.0,0.18]) {
        assert!((actual-expected).abs() < 1e-7);
    }

}

fn record(bytes: Vec<u8>) -> Recorder {
    record_at_size(bytes, 32, 16)
}

fn record_at_size(bytes: Vec<u8>, width: u16, height: u16) -> Recorder {
    record_at_transform(
        bytes,
        width,
        height,
        Affine::scale_non_uniform(f64::from(width) / 32.0, f64::from(height) / 16.0),
    )
}

fn record_at_transform(bytes: Vec<u8>, width: u16, height: u16, transform: Affine) -> Recorder {
    record_into(bytes, width, height, transform, Recorder::default())
}

fn record_into(
    bytes: Vec<u8>,
    width: u16,
    height: u16,
    transform: Affine,
    mut recorder: Recorder,
) -> Recorder {
    if !recorder.ready() {
        return recorder;
    }
    let pdf = hayro_interpret::hayro_syntax::Pdf::new(bytes).unwrap();
    let page = &pdf.pages()[0];
    let cache = crate::RenderCache::new();
    let mut context = hayro_interpret::Context::new(
        transform,
        kurbo::Rect::new(0.0, 0.0, f64::from(width), f64::from(height)),
        &cache.interpreter_cache,
        page.xref(),
        hayro_interpret::InterpreterSettings::default(),
    );
    recorder.width = width;
    recorder.height = height;
    hayro_interpret::interpret_page(page, &mut context, &mut recorder);
    recorder.ready();
    assert!(recorder.failed || recorder.clips.is_empty());
    recorder
}

struct Frame {
    blend_mode: BlendMode,
    alpha_is_shape: bool,
    mask: Option<ShapeMask>,
    initial: Vec<[f64; 4]>,
    current: Vec<[f64; 4]>,
    coverage: Vec<PreciseCoverage>,
    properties: TransparencyGroupProperties,
    opacity: f64,
    _credit: Box<dyn std::any::Any + Send + Sync>,
}

/// Test-only texture sampling in premultiplied space to avoid transparent
/// color fringes. PDF mask-specific interpolation still needs qualification.
fn sample_raster(
    image: &hayro_interpret::ImageData,
    alpha: Option<&hayro_interpret::LumaData>,
    point: kurbo::Point,
) -> Option<[u8; 4]> {
    let width = image.width() as usize;
    let height = image.height() as usize;
    if width == 0 || height == 0 || !point.x.is_finite() || !point.y.is_finite() {
        return None;
    }
    if alpha.is_some_and(|a| a.width != image.width() || a.height != image.height()) {
        return None;
    }
    let at = |x: f64, y: f64| -> [f64; 4] {
        let x = x.clamp(0.0, (width - 1) as f64) as usize;
        let y = y.clamp(0.0, (height - 1) as f64) as usize;
        let index = y * width + x;
        let rgb = match image {
            hayro_interpret::ImageData::Rgb(data) => [
                data.data[index * 3],
                data.data[index * 3 + 1],
                data.data[index * 3 + 2],
            ],
            hayro_interpret::ImageData::Luma(data) => [data.data[index]; 3],
        };
        let a = f64::from(alpha.map_or(255, |data| data.data[index]));
        [
            f64::from(rgb[0]) * a / 255.0,
            f64::from(rgb[1]) * a / 255.0,
            f64::from(rgb[2]) * a / 255.0,
            a,
        ]
    };
    let interpolate = match image {
        hayro_interpret::ImageData::Rgb(data) => data.interpolate,
        hayro_interpret::ImageData::Luma(data) => data.interpolate,
    };
    let pixel = if interpolate {
        let x = point.x - 0.5;
        let y = point.y - 0.5;
        let tx = x - x.floor();
        let ty = y - y.floor();
        let p = [
            at(x.floor(), y.floor()),
            at(x.floor() + 1.0, y.floor()),
            at(x.floor(), y.floor() + 1.0),
            at(x.floor() + 1.0, y.floor() + 1.0),
        ];
        std::array::from_fn(|c| {
            p[0][c] * (1.0 - tx) * (1.0 - ty)
                + p[1][c] * tx * (1.0 - ty)
                + p[2][c] * (1.0 - tx) * ty
                + p[3][c] * tx * ty
        })
    } else {
        at(point.x.floor(), point.y.floor())
    };
    Some(pixel.map(|c| c.round().clamp(0.0, 255.0) as u8))
}

fn apply(
    frame: &mut Frame,
    source: &[[u8; 4]],
    shape: &[u8],
    cancelled: &dyn Fn() -> bool,
    mode: BlendMode,
) -> Option<()> {
    if source.len() != frame.current.len() || shape.len() != source.len() {
        return None;
    }
    if !matches!(mode, BlendMode::Normal | BlendMode::Multiply | BlendMode::Screen | BlendMode::HardLight) {
        return None;
    }
    for index in 0..source.len() {
        if index % 256 == 0 && cancelled() {
            return None;
        }
        // A sample with neither shape nor premultiplied contribution cannot
        // change backdrop or accumulated group coverage, including knockout.
        if shape[index] == 0 && source[index] == [0; 4] {
            continue;
        }
        apply_precise_sample(
            frame,
            index,
            source[index].map(|v| f64::from(v) / 255.0),
            f64::from(shape[index]) / 255.0,
            mode,
        )?;
    }
    Some(())
}

fn apply_precise_sample(
    frame: &mut Frame,
    index: usize,
    source: [f64; 4],
    shape: f64,
    mode: BlendMode,
) -> Option<()> {
    let backdrop = if frame.properties.knockout {
        frame.initial[index]
    } else {
        frame.current[index]
    };
    frame.current[index] = composite_precise_pixel(frame.current[index], backdrop, source, shape, mode)?;
    frame.coverage[index] = frame.coverage[index].add(shape, source[3], frame.properties.knockout)?;
    Some(())
}

fn replay(recorder: Recorder) -> Vec<[u8; 4]> {
    replay_with_cancel(recorder, &|| false).unwrap()
}

fn replay_with_cancel(recorder: Recorder, cancelled: &dyn Fn() -> bool) -> Option<Vec<[u8; 4]>> {
    Some(replay_with_controls(recorder, cancelled, &|_| Some(Box::new(())))?.pixels)
}

fn replay_with_controls(
    recorder: Recorder,
    cancelled: &dyn Fn() -> bool,
    admit: &dyn Fn(usize) -> Option<Box<dyn std::any::Any + Send + Sync>>,
) -> Option<CompositeBuffer> {
    replay_scene(
        &recorder,
        recorder.width,
        recorder.height,
        Affine::IDENTITY,
        cancelled,
        admit,
    )
}

fn replay_scene(
    recorder: &Recorder,
    width: u16,
    height: u16,
    tile_transform: Affine,
    cancelled: &dyn Fn() -> bool,
    admit: &dyn Fn(usize) -> Option<Box<dyn std::any::Any + Send + Sync>>,
) -> Option<CompositeBuffer> {
    replay_scene_with_backdrop(
        recorder,
        width,
        height,
        tile_transform,
        [255; 4],
        cancelled,
        admit,
    )
}

fn replay_scene_with_backdrop(
    recorder: &Recorder,
    width: u16,
    height: u16,
    tile_transform: Affine,
    backdrop: [u8; 4],
    cancelled: &dyn Fn() -> bool,
    admit: &dyn Fn(usize) -> Option<Box<dyn std::any::Any + Send + Sync>>,
) -> Option<CompositeBuffer> {
    let image = replay_scene_precise_with_backdrop(
        recorder,
        width,
        height,
        tile_transform,
        backdrop,
        cancelled,
        admit,
    )?;
    reduce_precise_samples(
        &image.pixels,
        usize::from(width),
        usize::from(height),
        1,
        cancelled,
        admit,
    )
}

fn replay_scene_precise_with_backdrop(
    recorder: &Recorder,
    width: u16,
    height: u16,
    tile_transform: Affine,
    backdrop: [u8; 4],
    cancelled: &dyn Fn() -> bool,
    admit: &dyn Fn(usize) -> Option<Box<dyn std::any::Any + Send + Sync>>,
) -> Option<PreciseBuffer> {
    if recorder.failed || cancelled() {
        return None;
    }
    let count = usize::from(width).checked_mul(usize::from(height))?;
    if count == 0 {
        return None;
    }
    let capacity = recorder
        .events
        .iter()
        .filter(|event| matches!(event, Event::Push(_, _, _)))
        .count()
        .checked_add(1)?;
    let _stack_credit = admit(capacity.checked_mul(size_of::<Frame>())?)?;
    if cancelled() {
        return None;
    }
    let mut stack = Vec::with_capacity(capacity);
    let frame_bytes = count.checked_mul(2 * size_of::<[f64; 4]>() + size_of::<PreciseCoverage>())?;
    let root_credit = admit(frame_bytes)?;
    if cancelled() {
        return None;
    }
    stack.push(Frame {
        blend_mode: BlendMode::Normal,
        alpha_is_shape: false,
        mask: None,
        initial: vec![backdrop.map(|v| f64::from(v) / 255.0); count],
        current: vec![backdrop.map(|v| f64::from(v) / 255.0); count],
        coverage: vec![PreciseCoverage::default(); count],
        properties: TransparencyGroupProperties::default(),
        opacity: 1.0,
        _credit: root_credit,
    });
    let mut active_mask: Option<ShapeMask> = None;
    let mut alpha_is_shape = false;
    let mut blend_mode = BlendMode::Normal;
    let profile_paths = std::env::var_os("RRRAH_PROFILE_NATIVE_PATHS").is_some();
    for (event_index, event) in recorder.events.iter().enumerate() {
        if cancelled() {
            return None;
        }
        match event {
            Event::Blend(mode) => blend_mode = *mode,
            Event::AlphaSource(value) => alpha_is_shape = *value,
            Event::Mask(mask) => {
                active_mask = if let Some((scene, luminosity, backdrop, transfer, cached)) = mask {
                    let crop = cached.as_ref().and_then(|mask| {
                        cached_mask_crop_bounds(recorder, width, height, tile_transform)
                            .map(|origin| (mask, origin))
                    });
                    if let Some((mask, (x, y))) = crop {
                        let credit = admit(count)?;
                        if cancelled() { return None; }
                        let mut values = Vec::new();
                        values.try_reserve_exact(count).ok()?;
                        for row in y..y + usize::from(height) {
                            if cancelled() { return None; }
                            let start = row * usize::from(recorder.width) + x;
                            values.extend_from_slice(&mask.values[start..start + usize::from(width)]);
                        }
                        Some(ShapeMask { values, _credit: credit })
                    } else {
                    let pixels = replay_scene_with_backdrop(
                        scene,
                        width,
                        height,
                        tile_transform,
                        *backdrop,
                        cancelled,
                        admit,
                    )?;
                    Some(soft_mask_values(
                        &pixels.pixels,
                        *luminosity,
                        &|v| transfer[(v.clamp(0.0, 1.0) * 255.0).round() as usize],
                        cancelled,
                        admit,
                    )?)
                    }
                } else {
                    None
                };
            }
            Event::Raster(image, alpha, transform, clips, stencil) => {
                use kurbo::Shape;
                if image.width() == 0 || image.height() == 0 {
                    return None;
                }
                let (sx, sy) = image.scale_factors();
                let transform =
                    tile_transform * *transform * Affine::scale_non_uniform(f64::from(sx), f64::from(sy));
                if !transform.determinant().is_finite() || transform.determinant().abs() < 1e-12 {
                    return None;
                }
                let inverse = transform.inverse();
                let path = kurbo::Rect::new(0.0, 0.0, f64::from(image.width()), f64::from(image.height()))
                    .to_path(0.1);
                let mut shape = rasterize_path_shape_with_transformed_clips(
                    &path,
                    transform,
                    clips.iter().map(|clip| {
                        (
                            &clip.path,
                            match clip.fill {
                                hayro_interpret::FillRule::NonZero => vello_cpu::peniko::Fill::NonZero,
                                hayro_interpret::FillRule::EvenOdd => vello_cpu::peniko::Fill::EvenOdd,
                            },
                        )
                    }),
                    tile_transform,
                    ShapeDrawMode::Fill(vello_cpu::peniko::Fill::NonZero),
                    width,
                    height,
                    cancelled,
                    admit,
                )?;
                let _credit = admit(count.checked_mul(4)?)?;
                if cancelled() {
                    return None;
                }
                let mut source = Vec::with_capacity(count);
                for index in 0..count {
                    if index % 256 == 0 && cancelled() {
                        return None;
                    }
                    let point = inverse
                        * kurbo::Point::new(
                            (index % usize::from(width)) as f64 + 0.5,
                            (index / usize::from(width)) as f64 + 0.5,
                        );
                    let pixel = sample_raster(image, alpha.as_ref(), point)?;
                    let covered =
                        pixel.map(|c| ((u32::from(c) * u32::from(shape.values[index]) + 127) / 255) as u8);
                    let opacity = stencil.unwrap_or(255);
                    source.push(covered.map(|c| ((u32::from(c) * u32::from(opacity) + 127) / 255) as u8));
                    if alpha_is_shape {
                        shape.values[index] = source.last().unwrap()[3];
                    } else if stencil.is_some() {
                        shape.values[index] =
                            ((u32::from(shape.values[index]) * u32::from(pixel[3]) + 127) / 255) as u8;
                    }
                }
                if let Some(mask) = &active_mask {
                    let masked = apply_opacity_mask(&source, &mask.values, cancelled, admit)?;
                    if alpha_is_shape {
                        scale_mask_shape(&mut shape.values, &mask.values, cancelled)?;
                    }
                    apply(
                        stack.last_mut().unwrap(),
                        &masked.pixels,
                        &shape.values,
                        cancelled,
                        blend_mode,
                    )?;
                } else {
                    apply(
                        stack.last_mut().unwrap(),
                        &source,
                        &shape.values,
                        cancelled,
                        blend_mode,
                    )?;
                }
            }
            Event::Push(opacity, properties, mode) => {
                if !opacity.is_finite() || !(0.0..=1.0).contains(opacity) {
                    return None;
                }
                let credit = admit(frame_bytes)?;
                if cancelled() {
                    return None;
                }
                let parent = stack.last().unwrap();
                let initial: Vec<_> = (0..count)
                    .map(|index| {
                        if properties.isolated {
                            [0.0; 4]
                        } else if parent.properties.knockout {
                            parent.initial[index]
                        } else {
                            parent.current[index]
                        }
                    })
                    .collect();
                stack.push(Frame {
                    blend_mode: *mode,
                    alpha_is_shape,
                    mask: active_mask.take(),
                    current: initial.clone(),
                    initial,
                    coverage: vec![PreciseCoverage::default(); count],
                    properties: *properties,
                    opacity: f64::from(*opacity),
                    _credit: credit,
                });
            }
            Event::Pop => {
                assert!(stack.len() > 1);
                let child = stack.pop().unwrap();
                let parent = stack.last_mut().unwrap();
                for index in 0..count {
                    if index % 256 == 0 && cancelled() {
                        return None;
                    }
                    let contribution = remove_precise_backdrop(
                        child.current[index],
                        child.initial[index],
                        child.coverage[index].alpha,
                    )?;
                    let opacity = child.opacity;
                    let mask = child
                        .mask
                        .as_ref()
                        .map_or(1.0, |mask| f64::from(mask.values[index]) / 255.0);
                    let source = contribution.map(|v| v * opacity * mask);
                    let shape =
                        child.coverage[index].shape * if child.alpha_is_shape { opacity * mask } else { 1.0 };
                    apply_precise_sample(parent, index, source, shape, child.blend_mode)?;
                }
            }
            Event::Path(path, transform, paint, clips, mode) => {
                let mut timing = profile_paths.then(|| PathTiming {
                    start: std::time::Instant::now(), shape_ms: None,
                    event: event_index, width, height, elements: path.elements().len(),
                    clips: clips.len(), complete: false,
                });
                // Keep shape independent of paint/group opacity for knockout.
                let _dash_credit = match &mode {
                    PathDrawMode::Stroke(props) => {
                        let bytes = props.dash_array.len().checked_mul(size_of::<f64>())?;
                        let credit = admit(bytes)?;
                        if cancelled() {
                            return None;
                        }
                        Some(credit)
                    }
                    _ => None,
                };
                let stroke = match &mode {
                    PathDrawMode::Stroke(props) => Some(kurbo::Stroke {
                        width: f64::from(props.line_width),
                        join: props.line_join,
                        miter_limit: f64::from(props.miter_limit),
                        start_cap: props.line_cap,
                        end_cap: props.line_cap,
                        dash_pattern: props.dash_array.iter().map(|&v| f64::from(v)).collect(),
                        dash_offset: f64::from(props.dash_offset),
                    }),
                    _ => None,
                };
                let draw_mode = match mode {
                    PathDrawMode::Fill(hayro_interpret::FillRule::NonZero) => {
                        ShapeDrawMode::Fill(vello_cpu::peniko::Fill::NonZero)
                    }
                    PathDrawMode::Fill(hayro_interpret::FillRule::EvenOdd) => {
                        ShapeDrawMode::Fill(vello_cpu::peniko::Fill::EvenOdd)
                    }
                    PathDrawMode::Stroke(_) => ShapeDrawMode::Stroke(stroke.as_ref().unwrap()),
                };
                let mut shape = if recorder.native_polygon {
                    if *transform != Affine::IDENTITY {
                        return None;
                    }
                    let coefficients = tile_transform.as_coeffs();
                    if coefficients[..4] != [1.0, 0.0, 0.0, 1.0]
                        || coefficients[4..]
                            .iter()
                            .any(|v| !v.is_finite() || v.fract() != 0. || v.abs() > 1_048_576.)
                    {
                        return None;
                    }
                    let rule = match draw_mode {
                        ShapeDrawMode::Fill(rule) => rule,
                        _ => return None,
                    };
                    rasterize_global_polygon_shape(
                        path,
                        rule,
                        clips.iter().map(|clip| {
                            (
                                clip.path.as_ref(),
                                match clip.fill {
                                    hayro_interpret::FillRule::NonZero => vello_cpu::peniko::Fill::NonZero,
                                    hayro_interpret::FillRule::EvenOdd => vello_cpu::peniko::Fill::EvenOdd,
                                },
                            )
                        }),
                        (-(coefficients[4] as i32), -(coefficients[5] as i32)),
                        width,
                        height,
                        cancelled,
                        admit,
                    )?
                } else {
                    rasterize_path_shape_with_transformed_clips(
                        path,
                        tile_transform * *transform,
                        clips.iter().map(|clip| {
                            (
                                clip.path.as_ref(),
                                match clip.fill {
                                    hayro_interpret::FillRule::NonZero => vello_cpu::peniko::Fill::NonZero,
                                    hayro_interpret::FillRule::EvenOdd => vello_cpu::peniko::Fill::EvenOdd,
                                },
                            )
                        }),
                        tile_transform,
                        draw_mode,
                        width,
                        height,
                        cancelled,
                        admit,
                    )?
                };
                if let Some(timing) = &mut timing {
                    timing.shape_ms = Some(timing.start.elapsed().as_secs_f64() * 1000.);
                }
                // An empty clipped path has neither shape nor color contribution.
                // Keep cancellation bounded while avoiding paint sampling/storage.
                let mut has_shape = false;
                for chunk in shape.values.chunks(256) {
                    if cancelled() {
                        return None;
                    }
                    if chunk.iter().any(|&value| value != 0) {
                        has_shape = true;
                        break;
                    }
                }
                if !has_shape && matches!(
                    blend_mode,
                    BlendMode::Normal | BlendMode::Multiply | BlendMode::Screen | BlendMode::HardLight
                ) {
                    if let Some(timing) = &mut timing { timing.complete = true; }
                    continue;
                }
                let _source_credit = admit(count * size_of::<[u8; 4]>())?;
                if cancelled() {
                    return None;
                }
                let inverse_tile = tile_transform.inverse();
                let source: Option<Vec<_>> = shape
                    .values
                    .iter()
                    .enumerate()
                    .map(|(index, &shape)| {
                        if index % 256 == 0 && cancelled() {
                            return None;
                        }
                        if shape == 0 {
                            return Some([0; 4]);
                        }
                        let color = match paint {
                            RecordedPaint::Color(color) => *color,
                            RecordedPaint::NativeColor(color) => color.display,
                            RecordedPaint::Shading(shading) => {
                                let point = inverse_tile
                                    * kurbo::Point::new(
                                        (index % usize::from(width)) as f64 + 0.5,
                                        (index / usize::from(width)) as f64 + 0.5,
                                    );
                                shading
                                    .sample(shading.base_transform * point)
                                    .map(|v| (v.clamp(0.0, 1.0) * 255.0).round() as u8)
                            }
                        };
                        let alpha = ((u32::from(shape) * u32::from(color[3]) + 127) / 255) as u8;
                        Some([
                            ((u32::from(color[0]) * u32::from(alpha) + 127) / 255) as u8,
                            ((u32::from(color[1]) * u32::from(alpha) + 127) / 255) as u8,
                            ((u32::from(color[2]) * u32::from(alpha) + 127) / 255) as u8,
                            alpha,
                        ])
                    })
                    .collect();
                let source = source?;
                if alpha_is_shape {
                    for (index, pixel) in source.iter().enumerate() {
                        if index % 256 == 0 && cancelled() {
                            return None;
                        }
                        shape.values[index] = pixel[3];
                    }
                }
                if let Some(mask) = &active_mask {
                    let masked = apply_opacity_mask(&source, &mask.values, cancelled, admit)?;
                    if alpha_is_shape {
                        scale_mask_shape(&mut shape.values, &mask.values, cancelled)?;
                    }
                    apply(
                        stack.last_mut().unwrap(),
                        &masked.pixels,
                        &shape.values,
                        cancelled,
                        blend_mode,
                    )?;
                } else {
                    apply(
                        stack.last_mut().unwrap(),
                        &source,
                        &shape.values,
                        cancelled,
                        blend_mode,
                    )?;
                }
                if let Some(timing) = &mut timing { timing.complete = true; }
            }
        }
    }
    assert_eq!(stack.len(), 1);
    if cancelled() {
        return None;
    }
    let Frame { current, _credit, .. } = stack.pop().unwrap();
    Some(PreciseBuffer {
        pixels: current,
        _credit,
    })
}

#[test]
fn every_pdf_replay_checkpoint_returns_no_partial_image_on_cancellation() {
    use std::cell::Cell;
    use std::sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    };
    struct Credit(Arc<AtomicUsize>, usize);
    impl Drop for Credit {
        fn drop(&mut self) {
            self.0.fetch_sub(self.1, Ordering::Relaxed);
        }
    }
    for pdf in replay_fault_fixtures() {
        let checkpoints = Cell::new(0);
        assert!(
            replay_with_cancel(record(pdf.clone()), &|| {
                checkpoints.set(checkpoints.get() + 1);
                false
            })
            .is_some()
        );
        assert!(checkpoints.get() > 20);
        for stop in 1..=checkpoints.get() {
            let polls = Cell::new(0);
            let used = Arc::new(AtomicUsize::new(0));
            let result = replay_with_controls(
                record(pdf.clone()),
                &|| {
                    polls.set(polls.get() + 1);
                    polls.get() >= stop
                },
                &|bytes| {
                    used.fetch_add(bytes, Ordering::Relaxed);
                    Some(Box::new(Credit(used.clone(), bytes)))
                },
            );
            assert!(result.is_none(), "checkpoint {stop}");
            assert_eq!(polls.get(), stop);
            assert_eq!(used.load(Ordering::Relaxed), 0, "checkpoint {stop}");
        }
    }
}

#[test]
fn interpreter_preserves_nested_knockout_group_properties() {
    let recorder = record(authored_pdf(true, 0.5, false));
    let groups: Vec<_> = recorder
        .events
        .iter()
        .filter_map(|event| {
            if let Event::Push(a, p, _) = event {
                Some((*a, *p))
            } else {
                None
            }
        })
        .collect();
    assert_eq!(groups.len(), 3);
    assert_eq!(
        groups[0],
        (
            1.0,
            TransparencyGroupProperties {
                isolated: false,
                knockout: true
            }
        )
    );
    assert_eq!(
        groups[2],
        (
            0.5,
            TransparencyGroupProperties {
                isolated: true,
                knockout: false
            }
        )
    );
}

#[test]
fn precise_group_refuses_original_small_budget_without_retaining_credit() {
    use std::sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    };
    struct Credit(Arc<AtomicUsize>, usize);
    impl Drop for Credit {
        fn drop(&mut self) {
            self.0.fetch_sub(self.1, Ordering::Relaxed);
        }
    }
    let used = Arc::new(AtomicUsize::new(0));
    let refusals = AtomicUsize::new(0);
    let result = replay_with_controls(record(authored_pdf(true, 0.5, false)), &|| false, &|bytes| {
        let current = used.load(Ordering::Relaxed);
        if current.checked_add(bytes).is_none_or(|next| next > 32 * 1024) {
            refusals.fetch_add(1, Ordering::Relaxed);
            return None;
        }
        used.fetch_add(bytes, Ordering::Relaxed);
        Some(Box::new(Credit(used.clone(), bytes)))
    });
    assert!(result.is_none());
    assert_eq!(refusals.load(Ordering::Relaxed), 1);
    assert_eq!(used.load(Ordering::Relaxed), 0);
}

#[test]
fn every_replay_admission_refusal_releases_frames_and_scratch() {
    use std::sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    };
    struct Credit(Arc<AtomicUsize>, usize);
    impl Drop for Credit {
        fn drop(&mut self) {
            self.0.fetch_sub(self.1, Ordering::Relaxed);
        }
    }
    for pdf in replay_fault_fixtures() {
        let mut requests = 0;
        for fail_at in 0..=32 {
            if fail_at > requests && fail_at != 0 {
                break;
            }
            let used = Arc::new(AtomicUsize::new(0));
            let peak = AtomicUsize::new(0);
            let attempts = AtomicUsize::new(0);
            let admit = |bytes| -> Option<Box<dyn std::any::Any + Send + Sync>> {
                let attempt = attempts.fetch_add(1, Ordering::Relaxed) + 1;
                if attempt == fail_at {
                    return None;
                }
                let current = used.fetch_add(bytes, Ordering::Relaxed) + bytes;
                assert!(
                    current <= 192 * 1024,
                    "precise group pixel/frame credit exceeded test limit"
                );
                peak.fetch_max(current, Ordering::Relaxed);
                Some(Box::new(Credit(used.clone(), bytes)))
            };
            let output = replay_with_controls(record(pdf.clone()), &|| false, &admit);
            if fail_at == 0 {
                requests = attempts.load(Ordering::Relaxed);
                assert!(requests >= 3, "stack, root and output storage require admission");
                assert!(requests <= 32, "expand refusal sweep for additional admissions");
                assert!(output.is_some());
                assert!(used.load(Ordering::Relaxed) >= 512 * 4);
                assert!(peak.load(Ordering::Relaxed) > used.load(Ordering::Relaxed));
            } else {
                assert!(output.is_none(), "admission {fail_at}");
                assert_eq!(attempts.load(Ordering::Relaxed), fail_at);
            }
            drop(output);
            assert_eq!(used.load(Ordering::Relaxed), 0, "admission {fail_at}");
        }
    }
}

#[test]
fn actual_pdf_callbacks_replay_knockout_and_normal_overlap() {
    for (knockout, opacity, yellow) in [
        (true, 0.5, false),
        (true, 0.0, false),
        (true, 0.5, true),
        (false, 0.5, false),
    ] {
        let pixels = replay(record(authored_pdf(knockout, opacity, yellow)));
        for (index, pixel) in pixels.iter().enumerate() {
            let x = index % 32;
            let expected = if x < 8 {
                [255, 0, 0, 255]
            } else if opacity == 0.0 {
                [255; 4]
            } else if yellow {
                [127, 127, 127, 255]
            } else if !knockout && x < 24 {
                [127, 0, 128, 255]
            } else {
                [127, 127, 255, 255]
            };
            for channel in 0..4 {
                assert!(
                    pixel[channel].abs_diff(expected[channel]) <= 1,
                    "pixel {index}: {pixel:?} != {expected:?}"
                );
            }
        }
    }
}

#[test]
fn actual_pdf_alpha_source_group_opacity_preserves_knockout_backdrop_and_restores_state() {
    for knockout in [false, true] {
        for opacity in [0.0, 0.5, 1.0] {
            let scene = record(authored_pdf_with_font_alpha_source(
                knockout,
                opacity,
                "q /form Do Q",
                "1 0 0 rg 0 0 24 16 re f",
                "0 0 1 rg 8 0 24 16 re f",
                "<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>",
                None,
                true,
            ));
            assert!(
                scene
                    .events
                    .iter()
                    .any(|event| matches!(event, Event::AlphaSource(true)))
            );
            assert!(!scene.alpha_is_shape, "Q must restore the default opacity source");
            let result = replay_scene(&scene, 32, 16, Affine::IDENTITY, &|| false, &|_| {
                Some(Box::new(()))
            })
            .unwrap();
            let alpha = (opacity * 255.0).round() as u8;
            let retained = ((1.0 - opacity) * 255.0).round() as u8;
            for (index, pixel) in result.pixels.iter().enumerate() {
                let x = index % 32;
                let expected = if x < 8 {
                    [255, 0, 0, 255]
                } else if x < 24 {
                    [retained, 0, alpha, 255]
                } else {
                    [retained, retained, 255, 255]
                };
                assert_eq!(
                    *pixel, expected,
                    "KO={knockout}, opacity={opacity}, pixel={index}"
                );
            }
            for edge in [3, 7] {
                let tiled = composite_spatial_tiles(
                    32,
                    16,
                    1,
                    edge,
                    &|| false,
                    &|_| Some(Box::new(())),
                    &mut |x, y, w, h| {
                        replay_scene(
                            &scene,
                            w as u16,
                            h as u16,
                            Affine::translate((-(x as f64), -(y as f64))),
                            &|| false,
                            &|_| Some(Box::new(())),
                        )
                    },
                )
                .unwrap();
                assert_eq!(tiled.pixels, result.pixels);
            }
        }
    }
}

#[test]
fn actual_pdf_alpha_source_path_and_group_constants_each_apply_once() {
    for alpha_is_shape in [false, true] {
        let scene = record(authored_pdf_with_font_alpha_source(
            true,
            0.5,
            "q /form Do Q",
            "1 0 0 rg 0 0 24 16 re f",
            "/half gs 0 0 1 rg 8 0 24 16 re f",
            "<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>",
            None,
            alpha_is_shape,
        ));
        let pixels = replay_scene(&scene, 32, 16, Affine::IDENTITY, &|| false, &|_| {
            Some(Box::new(()))
        })
        .unwrap();
        assert_eq!(pixels.pixels[8 * 32 + 4], [255, 0, 0, 255]);
        assert_eq!(
            pixels.pixels[8 * 32 + 12],
            if alpha_is_shape {
                [191, 0, 64, 255]
            } else {
                [191, 191, 255, 255]
            }
        );
        assert_eq!(pixels.pixels[8 * 32 + 28], [191, 191, 255, 255]);
        assert!(!scene.alpha_is_shape);
    }
}

#[test]
fn actual_pdf_alpha_source_mask_changes_group_shape_once() {
    for alpha_is_shape in [false, true] {
        let stream = |dict: &str, body: &str| {
            format!("<< {dict} /Length {} >>\nstream\n{body}\nendstream", body.len())
        };
        let objects = [
            "<< /Type /Catalog /Pages 2 0 R >>".to_owned(),
            "<< /Type /Pages /Kids [3 0 R] /Count 1 >>".to_owned(),
            "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 32 16] /Resources << /XObject << /P 5 0 R >> >> /Contents 4 0 R >>".to_owned(),
            stream("", "q /P Do Q 0 1 0 rg 24 0 8 16 re f"),
            stream("/Type /XObject /Subtype /Form /BBox [0 0 32 16] /Group << /S /Transparency /I false /K true >> /Resources << /ExtGState << /M 6 0 R >> /XObject << /G 8 0 R >> >>", "1 0 0 rg 0 0 32 16 re f q /M gs /G Do Q"),
            format!("<< /AIS {alpha_is_shape} /SMask << /S /Alpha /G 7 0 R >> >>"),
            stream("/Type /XObject /Subtype /Form /BBox [0 0 32 16] /Group << /S /Transparency /I true >> /Resources << /ExtGState << /H 9 0 R >> >>", "/H gs 0 g 0 0 16 16 re f"),
            stream("/Type /XObject /Subtype /Form /BBox [0 0 32 16] /Group << /S /Transparency /I false >> /Resources << /ExtGState << /H 9 0 R >> >>", "/H gs 0 g 0 0 32 16 re f 0 0 32 16 re f"),
            "<< /ca 0.5 /AIS false >>".to_owned(),
        ];
        let mut pdf = b"%PDF-1.4\n".to_vec();
        let mut offsets = vec![0];
        for (index, object) in objects.iter().enumerate() {
            offsets.push(pdf.len());
            pdf.extend_from_slice(format!("{} 0 obj\n{object}\nendobj\n", index + 1).as_bytes());
        }
        let xref = pdf.len();
        pdf.extend_from_slice(format!("xref\n0 {}\n0000000000 65535 f \n", offsets.len()).as_bytes());
        for offset in &offsets[1..] {
            pdf.extend_from_slice(format!("{offset:010} 00000 n \n").as_bytes());
        }
        pdf.extend_from_slice(
            format!(
                "trailer\n<< /Size {} /Root 1 0 R >>\nstartxref\n{xref}\n%%EOF\n",
                offsets.len()
            )
            .as_bytes(),
        );
        let scene = record_into(
            pdf.clone(),
            32,
            16,
            Affine::IDENTITY,
            Recorder {
                mask_probe: true,
                ..Recorder::default()
            },
        );
        assert!(!scene.failed);
        assert!(!scene.alpha_is_shape);
        let native = record_into(pdf,32,16,Affine::IDENTITY,Recorder {
            mask_probe:true,global_contour:true,native_polygon:true,admitted_strokes:true,..Recorder::default()
        });
        assert!(!native.failed);
        use hayro_interpret::color::{Color,AlphaColor};
        use super::native_blend::{Pixel,Model};
        let rgb = Color::from_rgba(AlphaColor::new([0.,0.,0.,1.]));
        let white = Pixel::from_native(Model::Rgb,&[1.;3],1.).unwrap();
        let output = replay_native_solid_scene(&native,32,16,rgb.color_space(),white,&|| false,&|_| Some(Box::new(()))).unwrap();
        for (index,pixel) in output.pixels.iter().enumerate() {
            let x = index%32;
            let expected = if x>=24 {[0.,1.,0.]} else if alpha_is_shape && x>=16 {[1.,0.,0.]}
                else if alpha_is_shape {[0.625,0.125,0.125]} else if x>=16 {[1.;3]} else {[0.625;3]};
            assert_eq!(pixel.alpha(),1.0);
            assert_eq!(pixel.native_components().unwrap()[..3],expected,"native AIS={alpha_is_shape} pixel={index}");
        }

        let result = replay_scene(&scene, 32, 16, Affine::IDENTITY, &|| false, &|_| {
            Some(Box::new(()))
        })
        .unwrap();
        for (index, pixel) in result.pixels.iter().enumerate() {
            let x = index % 32;
            let expected = if x >= 24 {
                [0, 255, 0, 255]
            } else if alpha_is_shape && x >= 16 {
                [255, 0, 0, 255]
            } else if alpha_is_shape {
                [159, 32, 32, 255]
            } else if x >= 16 {
                [255; 4]
            } else {
                [159, 159, 159, 255]
            };
            assert_eq!(*pixel, expected, "AIS={alpha_is_shape}, pixel={index}");
        }
        for edge in [3, 7] {
            let tiled = composite_spatial_tiles(
                32,
                16,
                1,
                edge,
                &|| false,
                &|_| Some(Box::new(())),
                &mut |x, y, w, h| {
                    replay_scene(
                        &scene,
                        w as u16,
                        h as u16,
                        Affine::translate((-(x as f64), -(y as f64))),
                        &|| false,
                        &|_| Some(Box::new(())),
                    )
                },
            )
            .unwrap();
            assert_eq!(tiled.pixels, result.pixels);
        }
    }
}

#[test]
fn actual_pdf_separable_group_blends_use_colored_knockout_initial_backdrop() {
    for mode in ["Multiply", "Screen", "HardLight"] {
        for opacity in [0.5, 1.0] {
            let scene = record(authored_pdf_with_font_blend(
                true,
                opacity,
                "1 1 0 rg 0 0 32 16 re f q /form Do Q",
                "1 0 0 rg 0 0 24 16 re f",
                "0 0 1 rg 8 0 24 16 re f",
                "<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>",
                None,
                false,
                mode,
            ));
            assert_eq!(scene.blend_mode, BlendMode::Normal);
            let result = replay_scene(&scene, 32, 16, Affine::IDENTITY, &|| false, &|_| {
                Some(Box::new(()))
            })
            .unwrap();
            let expected = match (mode, opacity == 1.0) {
                ("Multiply", false) => [128, 128, 0, 255],
                ("Multiply", true) => [0, 0, 0, 255],
                ("Screen", false) => [255, 255, 128, 255],
                ("Screen", true) => [255; 4],
                ("HardLight", false) => [128, 128, 128, 255],
                ("HardLight", true) => [0, 0, 255, 255],
                _ => unreachable!(),
            };
            for (index, pixel) in result.pixels.iter().enumerate() {
                assert_eq!(
                    *pixel,
                    if index % 32 < 8 {
                        [255, 0, 0, 255]
                    } else {
                        expected
                    },
                    "mode={mode}, opacity={opacity}, pixel={index}"
                );
            }
            for edge in [3, 7] {
                let tiled = composite_spatial_tiles(
                    32,
                    16,
                    1,
                    edge,
                    &|| false,
                    &|_| Some(Box::new(())),
                    &mut |x, y, w, h| {
                        replay_scene(
                            &scene,
                            w as u16,
                            h as u16,
                            Affine::translate((-(x as f64), -(y as f64))),
                            &|| false,
                            &|_| Some(Box::new(())),
                        )
                    },
                )
                .unwrap();
                assert_eq!(tiled.pixels, result.pixels);
            }
        }
    }
}

#[test]
fn actual_pdf_clip_is_restored_after_knockout_group() {
    for opacity in [0.5, 0.0] {
        let pixels = replay(record(authored_pdf_with_content(
            true,
            opacity,
            "q 0 0 16 16 re W n /form Do Q\n0 1 0 rg 24 0 8 16 re f",
        )));
        for (index, pixel) in pixels.iter().enumerate() {
            let x = index % 32;
            let expected = if x < 8 {
                [255, 0, 0, 255]
            } else if x < 16 && opacity != 0.0 {
                [127, 127, 255, 255]
            } else if x >= 24 {
                [0, 255, 0, 255]
            } else {
                [255; 4]
            };
            for channel in 0..4 {
                assert!(
                    pixel[channel].abs_diff(expected[channel]) <= 1,
                    "opacity {opacity}, pixel {index}: {pixel:?} != {expected:?}"
                );
            }
        }
    }
}

#[test]
fn actual_pdf_evenodd_hole_preserves_previous_knockout_content() {
    // Two equally oriented rectangles: only f* leaves the inner one uncovered.
    let pdf = authored_pdf_with_blue(true, 0.5, "/form Do", "0 0 1 rg 0 0 32 16 re 10 4 4 8 re f*");
    let pixels = replay(record(pdf));
    for (index, pixel) in pixels.iter().enumerate() {
        let x = index % 32;
        let y = index / 32;
        let expected = if (10..14).contains(&x) && (4..12).contains(&y) {
            [255, 0, 0, 255]
        } else {
            [127, 127, 255, 255]
        };
        for channel in 0..4 {
            assert!(
                pixel[channel].abs_diff(expected[channel]) <= 1,
                "pixel {index}: {pixel:?} != {expected:?}"
            );
        }
    }
}

#[test]
fn actual_pdf_dashed_stroke_keeps_knockout_shape_separate_from_alpha() {
    for opacity in [0.5, 0.0] {
        let pdf = authored_pdf_with_blue(
            true,
            opacity,
            "/form Do",
            "0 0 1 RG 4 w 0 J [4 4] 0 d 0 8 m 32 8 l S",
        );
        let pixels = replay(record(pdf));
        for (index, pixel) in pixels.iter().enumerate() {
            let x = index % 32;
            let y = index / 32;
            let stroke = (6..10).contains(&y) && x % 8 < 4;
            let expected = if stroke && opacity == 0.5 {
                [127, 127, 255, 255]
            } else if stroke || x >= 24 {
                [255; 4]
            } else {
                [255, 0, 0, 255]
            };
            for channel in 0..4 {
                assert!(
                    pixel[channel].abs_diff(expected[channel]) <= 1,
                    "opacity {opacity}, pixel {index}: {pixel:?} != {expected:?}"
                );
            }
        }
    }
}

fn replay_fault_fixtures() -> [Vec<u8>; 4] {
    [
        authored_pdf_with_font(
            true,
            0.5,
            "/form Do",
            "1 0 0 rg 0 0 32 16 re f",
            "/S1 sh",
            "<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>",
            Some(
                "<< /ShadingType 2 /ColorSpace /DeviceRGB /BBox [4.5 2.5 27.5 13.5] /Coords [0 0 32 0] /Function << /FunctionType 2 /Domain [0 1] /C0 [0 0 0] /C1 [1 1 1] /N 1 >> /Extend [true true] >>",
            ),
        ),
        authored_pdf(true, 0.5, true),
        authored_pdf_with_blue(true, 0.5, "/form Do", "0 0 1 rg 0 0 32 16 re 10 4 4 8 re f*"),
        authored_pdf_with_blue(
            true,
            0.0,
            "q 0 0 16 16 re W n /form Do Q",
            "0 0 1 RG 4 w 0 J [4 4] 0 d 0 8 m 32 8 l S",
        ),
    ]
}

#[test]
fn actual_pdf_replay_scales_geometry_and_admitted_frames() {
    use std::cell::Cell;
    for (width, height) in [(64, 32), (96, 48)] {
        let largest = Cell::new(0);
        let output = replay_with_controls(
            record_at_size(authored_pdf(true, 0.5, false), width, height),
            &|| false,
            &|bytes| {
                largest.set(largest.get().max(bytes));
                Some(Box::new(()))
            },
        )
        .unwrap();
        let count = usize::from(width) * usize::from(height);
        assert_eq!(output.pixels.len(), count);
        assert_eq!(
            largest.get(),
            count * (2 * size_of::<[f64; 4]>() + size_of::<PreciseCoverage>())
        );
        for (index, pixel) in output.pixels.iter().enumerate() {
            let expected = if index % usize::from(width) < usize::from(width) / 4 {
                [255, 0, 0, 255]
            } else {
                [127, 127, 255, 255]
            };
            for channel in 0..4 {
                assert!(
                    pixel[channel].abs_diff(expected[channel]) <= 1,
                    "{width}x{height}, pixel {index}: {pixel:?} != {expected:?}"
                );
            }
        }
    }
    let empty = Recorder::default();
    assert!(replay_with_controls(empty, &|| false, &|_| panic!("empty surface admitted")).is_none());
}

#[test]
#[ignore = "requires explicit authored PDF fixture directory and fresh diagnostic output path"]
fn external_authored_pdf_replay_diagnostic() {
    let root = std::env::var_os("RRRAH_KNOCKOUT_DIAGNOSTIC_ROOT").unwrap();
    let root = std::path::PathBuf::from(root);
    let output = std::path::PathBuf::from(std::env::var_os("RRRAH_KNOCKOUT_DIAGNOSTIC_OUTPUT").unwrap());
    std::fs::create_dir(&output).unwrap();
    for name in [
        "pdf-normal-overlap",
        "pdf-knockout-overlap",
        "pdf-knockout-transparent-child",
        "pdf-knockout-colored-backdrop",
        "pdf-knockout-fractional-isolated",
        "pdf-knockout-fractional-nonisolated",
    ] {
        let pixels = replay(record(std::fs::read(root.join(format!("{name}.pdf"))).unwrap()));
        assert_eq!(pixels.len(), 512);
        assert!(pixels.iter().all(|pixel| pixel[3] == 255));
        let bytes: Vec<_> = pixels.into_iter().flatten().collect();
        std::fs::write(output.join(format!("{name}.rgba")), bytes).unwrap();
    }
}

#[test]
fn correlated_fractional_edges_require_compositing_before_area_reduction() {
    let pdf = authored_pdf_with_paths(
        true,
        0.5,
        "/form Do",
        "1 0 0 rg 8.5 0 16 16 re f",
        "0 0 1 rg 8.5 0 16 16 re f",
    );
    let averaged_first = replay(record(pdf.clone()));
    let scene = record_at_size(pdf, 64, 32);
    let spatial =
        replay_scene_precise_with_backdrop(&scene, 64, 32, Affine::IDENTITY, [255; 4], &|| false, &|_| {
            Some(Box::new(()))
        })
        .unwrap();
    // Exact geometry: both objects cover the right half of pixel x=8.
    // Composite each half first: left white, right half-blue on white.
    let reduced_image =
        reduce_precise_samples(&spatial.pixels, 32, 16, 2, &|| false, &|_| Some(Box::new(()))).unwrap();
    let reduced = reduced_image.pixels[8 * 32 + 8];
    assert_eq!(reduced, [191, 191, 255, 255]);
    for edge in [3, 7, 16] {
        let tiled = composite_precise_spatial_tiles(
            32,
            16,
            2,
            edge,
            &|| false,
            &|_| Some(Box::new(())),
            &mut |x, y, w, h| {
                replay_scene_precise_with_backdrop(
                    &scene,
                    w as u16,
                    h as u16,
                    Affine::translate((-(x as f64) * 2.0, -(y as f64) * 2.0)),
                    [255; 4],
                    &|| false,
                    &|_| Some(Box::new(())),
                )
            },
        )
        .unwrap();
        assert_eq!(tiled.pixels, reduced_image.pixels, "precise edge={edge}");
    }

    // Diagnostic of the current pixel-average architecture, not an accepted
    // rendering requirement. Keep its discrepancy explicit before integration.
    assert_ne!(averaged_first[8 * 32 + 8], reduced);
    eprintln!(
        "correlated edge: averaged-first {:?}, spatial {:?}",
        averaged_first[8 * 32 + 8],
        reduced
    );
}

#[test]
fn spatial_tiles_match_whole_pdf_composition_with_fractional_edges() {
    use std::cell::Cell;
    let pdf = authored_pdf_with_paths(
        true,
        0.5,
        "/form Do",
        "1 0 0 rg 8.5 0 16 16 re f",
        "0 0 1 rg 8.5 0 16 16 re f",
    );
    let clipped = authored_pdf_with_blue(
        true,
        0.5,
        "q 2.5 1.5 27 13 re W n 0 0 32 16 re 10.5 4.5 4 7 re W* n /form Do Q\n0 1 0 rg 28 0 4 16 re f",
        "0 0 1 RG 3 w 1 J [4 3] 1 d 0 8 m 32 8 l S",
    );
    let hole = authored_pdf_with_blue(
        true,
        0.0,
        "/form Do",
        "0 0 1 rg 0.5 0.5 31 15 re 10.5 4.5 4 7 re f*",
    );
    for pdf in [pdf, clipped, hole] {
        let spatial = replay(record_at_size(pdf.clone(), 64, 32));
        let expected =
            reduce_spatial_samples(&spatial, 32, 16, 2, &|| false, &|_| Some(Box::new(()))).unwrap();
        let scene = record(pdf);
        for edge in [3, 7, 16] {
            let largest_tile = Cell::new(0);
            let output = composite_spatial_tiles(
                32,
                16,
                2,
                edge,
                &|| false,
                &|_| Some(Box::new(())),
                &mut |x, y, w, h| {
                    largest_tile.set(largest_tile.get().max(w * h));
                    let transform =
                        Affine::translate((-(x as f64) * 2.0, -(y as f64) * 2.0)) * Affine::scale(2.0);
                    replay_scene(&scene, w as u16, h as u16, transform, &|| false, &|_| {
                        Some(Box::new(()))
                    })
                },
            )
            .unwrap();
            assert_eq!(output.pixels, expected.pixels, "tile edge {edge}");
            assert!(largest_tile.get() <= edge * edge * 4);
        }
    }
}

#[test]
#[ignore = "explicit prototype benchmark with fresh report destination required"]
fn spatial_tile_scene_reuse_benchmark() {
    let report = std::env::var_os("RRRAH_TILE_BENCH_REPORT").unwrap();
    let pdf = authored_pdf_with_blue(
        true,
        0.5,
        "q 2.5 1.5 27 13 re W n /form Do Q",
        "0 0 1 RG 3 w 1 J [4 3] 1 d 0 8 m 32 8 l S",
    );
    let mut rows = Vec::new();
    for edge in [3, 7, 16] {
        let mut reused = Vec::new();
        let mut reparsed = Vec::new();
        for iteration in 0..6 {
            let mut results = [None, None];
            for reuse in if iteration % 2 == 0 {
                [true, false]
            } else {
                [false, true]
            } {
                let start = std::time::Instant::now();
                let scene = if reuse { Some(record(pdf.clone())) } else { None };
                let output = composite_spatial_tiles(
                    32,
                    16,
                    2,
                    edge,
                    &|| false,
                    &|_| Some(Box::new(())),
                    &mut |x, y, w, h| {
                        let transform =
                            Affine::translate((-(x as f64) * 2.0, -(y as f64) * 2.0)) * Affine::scale(2.0);
                        if let Some(scene) = &scene {
                            replay_scene(scene, w as u16, h as u16, transform, &|| false, &|_| {
                                Some(Box::new(()))
                            })
                        } else {
                            replay_with_controls(
                                record_at_transform(pdf.clone(), w as u16, h as u16, transform),
                                &|| false,
                                &|_| Some(Box::new(())),
                            )
                        }
                    },
                )
                .unwrap();
                let elapsed = start.elapsed().as_micros();
                if reuse {
                    reused.push(elapsed);
                } else {
                    reparsed.push(elapsed);
                }
                results[usize::from(reuse)] = Some(output.pixels);
            }
            assert_eq!(results[0], results[1]);
        }
        rows.push(format!(
            "{{\"edge\":{edge},\"reuse_us\":{reused:?},\"reparse_us\":{reparsed:?}}}"
        ));
    }
    let json = format!(
        "{{\"scope\":\"Test-only 32x16 authored clipped dashed PDF, 2x spatial sampling; includes initial parser work; alternating six runs per path. No production or real artwork throughput claim.\",\"pixel_comparison\":\"exact for every paired run\",\"results\":[{}]}}\n",
        rows.join(",")
    );
    use std::io::Write;
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(report)
        .unwrap();
    file.write_all(json.as_bytes()).unwrap();
}

#[test]
fn recorded_scene_admission_refusal_prevents_partial_replay_and_releases_credit() {
    use std::sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    };
    struct Credit(Arc<AtomicUsize>, usize);
    impl Drop for Credit {
        fn drop(&mut self) {
            self.0.fetch_sub(self.1, Ordering::Relaxed);
        }
    }
    let mut requests = 0;
    for deny in 0..100 {
        if deny > requests && deny != 0 {
            break;
        }
        let used = Arc::new(AtomicUsize::new(0));
        let attempts = Arc::new(AtomicUsize::new(0));
        let counter = attempts.clone();
        let budget = used.clone();
        let recorder = Recorder {
            admission: Some(std::rc::Rc::new(move |bytes| {
                if counter.fetch_add(1, Ordering::Relaxed) + 1 == deny {
                    return None;
                }
                budget.fetch_add(bytes, Ordering::Relaxed);
                Some(Box::new(Credit(budget.clone(), bytes)))
            })),
            ..Recorder::default()
        };
        let scene = record_into(
            authored_pdf_with_blue(
                true,
                0.5,
                "q 2 2 28 12 re W n /form Do Q",
                "0 0 1 RG 3 w [4 3] 1 d 0 8 m 32 8 l S",
            ),
            32,
            16,
            Affine::IDENTITY,
            recorder,
        );
        if deny == 0 {
            requests = attempts.load(Ordering::Relaxed);
            assert!(requests > 8);
            assert!(!scene.failed);
            assert!(used.load(Ordering::Relaxed) > 0);
        } else {
            assert!(scene.failed);
            assert_eq!(attempts.load(Ordering::Relaxed), deny);
            assert!(
                replay_scene(&scene, 32, 16, Affine::IDENTITY, &|| false, &|_| panic!(
                    "partial scene admitted frame"
                ))
                .is_none()
            );
        }
        drop(scene);
        assert_eq!(used.load(Ordering::Relaxed), 0);
    }
}

#[test]
fn recorded_scene_cancellation_stops_capture_and_prevents_partial_replay() {
    use std::sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    };
    struct Credit(Arc<AtomicUsize>, usize);
    impl Drop for Credit {
        fn drop(&mut self) {
            self.0.fetch_sub(self.1, Ordering::Relaxed);
        }
    }
    let run = |stop| {
        let used = Arc::new(AtomicUsize::new(0));
        let polls = Arc::new(AtomicUsize::new(0));
        let counter = polls.clone();
        let budget = used.clone();
        let recorder = Recorder {
            cancelled: Some(Arc::new(move || {
                counter.fetch_add(1, Ordering::Relaxed) + 1 == stop
            })),
            admission: Some(std::rc::Rc::new(move |bytes| {
                budget.fetch_add(bytes, Ordering::Relaxed);
                Some(Box::new(Credit(budget.clone(), bytes)))
            })),
            ..Recorder::default()
        };
        let scene = record_into(
            authored_pdf_with_blue(
                true,
                0.5,
                "q 2 2 28 12 re W n /form Do Q",
                "0 0 1 RG 3 w [4 3] 1 d 0 8 m 32 8 l S",
            ),
            32,
            16,
            Affine::IDENTITY,
            recorder,
        );
        if stop == 0 {
            assert!(!scene.failed);
        } else {
            assert!(scene.failed);
            assert_eq!(polls.load(Ordering::Relaxed), stop);
            assert!(
                replay_scene(&scene, 32, 16, Affine::IDENTITY, &|| false, &|_| panic!(
                    "cancelled scene admitted frame"
                ))
                .is_none()
            );
        }
        drop(scene);
        assert_eq!(used.load(Ordering::Relaxed), 0);
        polls.load(Ordering::Relaxed)
    };
    let polls = run(0);
    assert!(polls > 20);
    for stop in 1..=polls {
        run(stop);
    }
}

#[test]
fn interpreter_stops_operator_only_stream_before_any_scene_capture() {
    use std::sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    };
    // No drawing callbacks among twenty thousand save/restore operators.
    let content = "q Q\n".repeat(10_000);
    let pdf = authored_pdf_with_content(true, 0.5, &content);
    for stop in [2, 3, 1002] {
        let polls = Arc::new(AtomicUsize::new(0));
        let counter = polls.clone();
        let recorder = Recorder {
            cancelled: Some(Arc::new(move || {
                counter.fetch_add(1, Ordering::Relaxed) + 1 == stop
            })),
            admission: Some(std::rc::Rc::new(|_| {
                panic!("operator-only stream captured scene")
            })),
            ..Recorder::default()
        };
        let scene = record_into(pdf.clone(), 32, 16, Affine::IDENTITY, recorder);
        assert!(scene.failed);
        assert!(scene.events.is_empty());
        assert_eq!(polls.load(Ordering::Relaxed), stop);
        // Page-space preparation now has an early stop guard. It can return
        // before the final page guard; cancellation still consumes exactly
        // the requested polls and never captures this operator-only stream.
        assert!((stop - 1..=stop).contains(&scene.operator_checks));
    }
}

#[test]
fn actual_pdf_outline_text_records_fill_stroke_and_ignores_invisible_mode() {
    for mode in [0, 1, 3] {
        let red = format!("1 0 0 rg 1 0 0 RG BT /F1 12 Tf {mode} Tr 3 3 Td (H) Tj ET");
        let blue = format!("0 0 1 rg 0 0 1 RG BT /F1 12 Tf {mode} Tr 3 3 Td (H) Tj ET");
        let pdf = authored_pdf_with_paths(true, 0.5, "/form Do", &red, &blue);
        let scene = record(pdf);
        let paths = scene
            .events
            .iter()
            .filter(|e| matches!(e, Event::Path(..)))
            .count();
        assert_eq!(paths, if mode == 3 { 0 } else { 2 });
        let output = replay_scene(&scene, 64, 32, Affine::scale(2.0), &|| false, &|_| {
            Some(Box::new(()))
        })
        .unwrap();
        if mode == 3 {
            assert!(output.pixels.iter().all(|&p| p == [255; 4]));
        } else {
            assert!(
                output
                    .pixels
                    .iter()
                    .any(|p| p[2] == 255 && p[0].abs_diff(127) <= 1 && p[1].abs_diff(127) <= 1)
            );
        }
    }
}

#[test]
fn actual_pdf_type3_shape_glyph_replays_through_knockout_tiles() {
    let pdf = authored_pdf_with_font(
        true,
        0.5,
        "/form Do",
        "1 0 0 rg BT /F1 10 Tf 4 2 Td (H) Tj ET",
        "0 0 1 rg BT /F1 10 Tf 4 2 Td (H) Tj ET",
        "<< /Type /Font /Subtype /Type3 /FontBBox [0 0 800 800] /FontMatrix [.001 0 0 .001 0 0] /CharProcs << /H 10 0 R >> /Encoding << /Type /Encoding /Differences [72 /H] >> /FirstChar 72 /LastChar 72 /Widths [800] /Resources << >> >>",
        Some("800 0 0 0 800 800 d1 0 0 800 800 re f"),
    );
    let scene = record(pdf);
    assert_eq!(
        scene
            .events
            .iter()
            .filter(|e| matches!(e, Event::Path(..)))
            .count(),
        2
    );
    let whole = replay_scene(&scene, 64, 32, Affine::scale(2.0), &|| false, &|_| {
        Some(Box::new(()))
    })
    .unwrap();
    let expected =
        reduce_spatial_samples(&whole.pixels, 32, 16, 2, &|| false, &|_| Some(Box::new(()))).unwrap();
    let output = composite_spatial_tiles(
        32,
        16,
        2,
        3,
        &|| false,
        &|_| Some(Box::new(())),
        &mut |x, y, w, h| {
            replay_scene(
                &scene,
                w as u16,
                h as u16,
                Affine::translate((-(x as f64) * 2.0, -(y as f64) * 2.0)) * Affine::scale(2.0),
                &|| false,
                &|_| Some(Box::new(())),
            )
        },
    )
    .unwrap();
    assert_eq!(output.pixels, expected.pixels);
    assert!(
        output
            .pixels
            .iter()
            .any(|p| p[2] == 255 && p[0].abs_diff(127) <= 1 && p[1].abs_diff(127) <= 1)
    );
}

#[test]
fn type3_cancellation_stops_classification_and_shape_program_without_drawing() {
    use std::sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    };
    let operators = "q Q\n".repeat(10_000);
    for before_marker in [true, false] {
        let glyph = if before_marker {
            format!("{operators}800 0 0 0 800 800 d1 0 0 800 800 re f")
        } else {
            format!("800 0 0 0 800 800 d1 {operators}0 0 800 800 re f")
        };
        let pdf = authored_pdf_with_font(
            true,
            0.5,
            "/form Do",
            "1 0 0 rg BT /F1 10 Tf 4 2 Td (H) Tj ET",
            "",
            "<< /Type /Font /Subtype /Type3 /FontBBox [0 0 800 800] /FontMatrix [.001 0 0 .001 0 0] /CharProcs << /H 10 0 R >> /Encoding << /Type /Encoding /Differences [72 /H] >> /FirstChar 72 /LastChar 72 /Widths [800] /Resources << >> >>",
            Some(&glyph),
        );
        let polls = Arc::new(AtomicUsize::new(0));
        let counter = polls.clone();
        let scene = record_into(
            pdf,
            32,
            16,
            Affine::IDENTITY,
            Recorder {
                cancelled: Some(Arc::new(move || {
                    counter.fetch_add(1, Ordering::Relaxed) + 1 == 50
                })),
                ..Recorder::default()
            },
        );
        assert!(scene.failed);
        assert_eq!(scene.type3_calls, 1);
        assert_eq!(polls.load(Ordering::Relaxed), 50);
        assert!(scene.operator_checks <= 50);
        assert!(!scene.events.iter().any(|e| matches!(e, Event::Path(..))));
    }
}

#[test]
fn actual_pdf_rgb_image_records_owned_pixels_and_transform() {
    let pdf = authored_pdf_with_paths(
        true,
        0.5,
        "/form Do",
        "",
        "q 16 0 0 16 0 0 cm BI /W 1 /H 1 /BPC 8 /CS /RGB /F /AHx ID 0000FF> EI Q",
    );
    let scene = record(pdf);
    assert!(!scene.failed);
    let rasters: Vec<_> = scene
        .events
        .iter()
        .filter_map(|event| {
            if let Event::Raster(image, alpha, transform, clips, _) = event {
                Some((image, alpha, transform, clips))
            } else {
                None
            }
        })
        .collect();
    assert_eq!(rasters.len(), 1);
    let (image, alpha, transform, clips) = rasters[0];
    let hayro_interpret::ImageData::Rgb(image) = image else {
        panic!("expected RGB");
    };
    assert_eq!(image.data, [0, 0, 255]);
    assert_eq!((image.width, image.height), (1, 1));
    assert!(alpha.is_none());
    assert!(transform.determinant().abs() > 0.0);
    assert!(!clips.is_empty());
}

#[test]
fn actual_pdf_nearest_rgb_image_composes_knockout_in_tiles() {
    let pdf = authored_pdf_with_paths(
        true,
        0.5,
        "/form Do",
        "1 0 0 rg 0 0 16 16 re f",
        "q 16 0 0 16 0 0 cm BI /W 1 /H 1 /BPC 8 /CS /RGB /F /AHx ID 0000FF> EI Q",
    );
    let scene = record(pdf);
    let whole = replay_scene(&scene, 64, 32, Affine::scale(2.0), &|| false, &|_| {
        Some(Box::new(()))
    })
    .unwrap();
    let expected =
        reduce_spatial_samples(&whole.pixels, 32, 16, 2, &|| false, &|_| Some(Box::new(()))).unwrap();
    let output = composite_spatial_tiles(
        32,
        16,
        2,
        7,
        &|| false,
        &|_| Some(Box::new(())),
        &mut |x, y, w, h| {
            replay_scene(
                &scene,
                w as u16,
                h as u16,
                Affine::translate((-(x as f64) * 2.0, -(y as f64) * 2.0)) * Affine::scale(2.0),
                &|| false,
                &|_| Some(Box::new(())),
            )
        },
    )
    .unwrap();
    assert_eq!(output.pixels, expected.pixels);
    for (index, pixel) in output.pixels.iter().enumerate() {
        let expected = if index % 32 < 16 {
            [127, 127, 255, 255]
        } else {
            [255; 4]
        };
        for channel in 0..4 {
            assert!(pixel[channel].abs_diff(expected[channel]) <= 1);
        }
    }
}

#[test]
fn raster_interpolation_preserves_premultiplied_transparent_edges() {
    let image = hayro_interpret::ImageData::Rgb(hayro_interpret::RgbData {
        data: vec![255, 0, 0, 0, 0, 255],
        width: 2,
        height: 1,
        interpolate: true,
        scale_factors: (1.0, 1.0),
    });
    let alpha = hayro_interpret::LumaData {
        data: vec![0, 255],
        width: 2,
        height: 1,
        interpolate: true,
        scale_factors: (1.0, 1.0),
    };
    assert_eq!(
        sample_raster(&image, Some(&alpha), kurbo::Point::new(1.0, 0.5)),
        Some([0, 0, 128, 128])
    );
    assert_eq!(
        sample_raster(&image, None, kurbo::Point::new(1.0, 0.5)),
        Some([128, 0, 128, 255])
    );
    assert_eq!(
        sample_raster(&image, Some(&alpha), kurbo::Point::new(-2.0, 0.5)),
        Some([0; 4])
    );
    assert_eq!(
        sample_raster(&image, Some(&alpha), kurbo::Point::new(9.0, 0.5)),
        Some([0, 0, 255, 255])
    );
    assert!(sample_raster(&image, None, kurbo::Point::new(f64::NAN, 0.5)).is_none());
}

#[test]
#[ignore = "requires fresh raster oracle diagnostic output directory"]
fn actual_pdf_image_interpolation_oracle_dump() {
    let root = std::path::PathBuf::from(std::env::var_os("RRRAH_IMAGE_ORACLE_OUTPUT").unwrap());
    std::fs::create_dir(&root).unwrap();
    for interpolate in [false, true] {
        let image = format!(
            "q 16 0 0 16 0 0 cm BI /W 2 /H 1 /BPC 8 /CS /RGB /I {interpolate} /F /AHx ID 000000FFFFFF> EI Q"
        );
        let pdf = authored_pdf_with_paths(true, 0.5, "/form Do", "1 0 0 rg 0 0 16 16 re f", &image);
        let scene = record(pdf.clone());
        assert!(scene.events.iter().any(|event| {
            matches!(event, Event::Raster(hayro_interpret::ImageData::Rgb(image),_,_,_,_) if image.interpolate==interpolate)
        }));
        let output = replay_scene(&scene, 32, 16, Affine::IDENTITY, &|| false, &|_| {
            Some(Box::new(()))
        })
        .unwrap();
        let tiled = composite_spatial_tiles(
            32,
            16,
            1,
            7,
            &|| false,
            &|_| Some(Box::new(())),
            &mut |x, y, w, h| {
                replay_scene(
                    &scene,
                    w as u16,
                    h as u16,
                    Affine::translate((-(x as f64), -(y as f64))),
                    &|| false,
                    &|_| Some(Box::new(())),
                )
            },
        )
        .unwrap();
        assert_eq!(output.pixels, tiled.pixels);
        assert!(
            output
                .pixels
                .iter()
                .all(|p| p[0] == p[1] && p[1] == p[2] && p[3] == 255)
        );
        let name = if interpolate { "bilinear" } else { "nearest" };
        std::fs::write(root.join(format!("{name}.pdf")), pdf).unwrap();
        std::fs::write(
            root.join(format!("{name}.rgba")),
            output.pixels.into_iter().flatten().collect::<Vec<_>>(),
        )
        .unwrap();
    }
}

#[test]
fn actual_pdf_stencil_holes_preserve_previous_knockout_content() {
    for opacity in [0.5, 0.0] {
        let pdf = authored_pdf_with_paths(
            true,
            opacity,
            "/form Do",
            "1 0 0 rg 0 0 16 16 re f",
            "0 0 1 rg q 16 0 0 16 0 0 cm BI /W 2 /H 1 /BPC 1 /IM true /F /AHx ID 40> EI Q",
        );
        let pixels = replay(record(pdf));
        for (index, pixel) in pixels.iter().enumerate() {
            let x = index % 32;
            let expected = if x < 8 {
                if opacity == 0.0 {
                    [255; 4]
                } else {
                    [127, 127, 255, 255]
                }
            } else if x < 16 {
                [255, 0, 0, 255]
            } else {
                [255; 4]
            };
            for channel in 0..4 {
                assert!(
                    pixel[channel].abs_diff(expected[channel]) <= 1,
                    "opacity {opacity} pixel {index}: {pixel:?} != {expected:?}"
                );
            }
        }
    }
}

#[test]
fn stencil_paint_opacity_does_not_replace_mask_shape() {
    for opacity in [0.5, 0.0] {
        let pdf = authored_pdf_with_paths(
            true,
            opacity,
            "/form Do",
            "1 0 0 rg 0 0 16 16 re f",
            "/half gs 0 0 1 rg q 16 0 0 16 0 0 cm BI /W 2 /H 1 /BPC 1 /IM true /F /AHx ID 40> EI Q",
        );
        let pixels = replay(record(pdf));
        for (index, pixel) in pixels.iter().enumerate() {
            let x = index % 32;
            let expected = if x < 8 {
                if opacity == 0.0 {
                    [255; 4]
                } else {
                    [191, 191, 255, 255]
                }
            } else if x < 16 {
                [255, 0, 0, 255]
            } else {
                [255; 4]
            };
            for channel in 0..4 {
                assert!(
                    pixel[channel].abs_diff(expected[channel]) <= 1,
                    "opacity {opacity}, pixel {index}: {pixel:?} != {expected:?}"
                );
            }
        }
    }
}

#[test]
fn production_stencil_opacity_is_applied_once_at_image_group() {
    let bytes = authored_pdf_with_paths(
        false,
        0.5,
        "/form Do",
        "",
        "/half gs 0 0 1 rg q 16 0 0 16 0 0 cm BI /W 2 /H 1 /BPC 1 /IM true /F /AHx ID 40> EI Q",
    );
    let pdf = hayro_interpret::hayro_syntax::Pdf::new(bytes).unwrap();
    let cache = crate::RenderCache::new();
    let output = crate::render(
        &pdf.pages()[0],
        &cache,
        &hayro_interpret::InterpreterSettings::default(),
        &crate::RenderSettings::default(),
    );
    let data = output.data_as_u8_slice();
    let pixel = &data[(8 * 32 + 4) * 4..(8 * 32 + 4) * 4 + 4];
    // Parent child alpha .5 times image alpha .5 = .25, not .125.
    let white: Vec<_> = pixel[..3]
        .iter()
        .map(|&c| u16::from(c) + 255 - u16::from(pixel[3]))
        .collect();
    assert!(white[0].abs_diff(191) <= 1 && white[1].abs_diff(191) <= 1);
    assert_eq!(white[2], 255);
}

#[test]
#[ignore = "requires fresh stencil oracle diagnostic output directory"]
fn stencil_opacity_external_oracle_dump() {
    let root = std::path::PathBuf::from(std::env::var_os("RRRAH_STENCIL_ORACLE_OUTPUT").unwrap());
    std::fs::create_dir(&root).unwrap();
    for opacity in [0.5, 0.0] {
        let bytes = authored_pdf_with_paths(
            false,
            opacity,
            "/form Do",
            "",
            "/half gs 0 0 1 rg q 16 0 0 16 0 0 cm BI /W 2 /H 1 /BPC 1 /IM true /F /AHx ID 40> EI Q",
        );
        let prototype = replay(record(bytes.clone()));
        let pdf = hayro_interpret::hayro_syntax::Pdf::new(bytes.clone()).unwrap();
        let cache = crate::RenderCache::new();
        let output = crate::render(
            &pdf.pages()[0],
            &cache,
            &hayro_interpret::InterpreterSettings::default(),
            &crate::RenderSettings::default(),
        );
        let production: Vec<_> = output
            .data_as_u8_slice()
            .chunks_exact(4)
            .flat_map(|p| [p[0] + (255 - p[3]), p[1] + (255 - p[3]), p[2] + (255 - p[3]), 255])
            .collect();
        let prototype: Vec<_> = prototype.into_iter().flatten().collect();
        assert_eq!(production, prototype);
        let name = if opacity == 0.5 {
            "stencil-quarter-opacity"
        } else {
            "stencil-zero-opacity"
        };
        std::fs::write(root.join(format!("{name}.pdf")), bytes).unwrap();
        std::fs::write(root.join(format!("{name}.rgba")), production).unwrap();
    }
}

#[test]
fn actual_pdf_axial_shading_records_and_replays_in_knockout_tiles() {
    let pdf = authored_pdf_with_font(
        true,
        0.5,
        "/form Do",
        "1 0 0 rg 0 0 32 16 re f",
        "/S1 sh",
        "<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>",
        Some(
            "<< /ShadingType 2 /ColorSpace /DeviceRGB /Coords [0 0 32 0] /Function << /FunctionType 2 /Domain [0 1] /C0 [0 0 0] /C1 [1 1 1] /N 1 >> /Extend [true true] >>",
        ),
    );
    let scene = record(pdf);
    assert!(
        scene
            .events
            .iter()
            .any(|e| matches!(e, Event::Path(_, _, RecordedPaint::Shading(_), _, _)))
    );
    let whole = replay_scene(&scene, 64, 32, Affine::scale(2.0), &|| false, &|_| {
        Some(Box::new(()))
    })
    .unwrap();
    let expected =
        reduce_spatial_samples(&whole.pixels, 32, 16, 2, &|| false, &|_| Some(Box::new(()))).unwrap();
    let tiled = composite_spatial_tiles(
        32,
        16,
        2,
        7,
        &|| false,
        &|_| Some(Box::new(())),
        &mut |x, y, w, h| {
            replay_scene(
                &scene,
                w as u16,
                h as u16,
                Affine::translate((-(x as f64) * 2.0, -(y as f64) * 2.0)) * Affine::scale(2.0),
                &|| false,
                &|_| Some(Box::new(())),
            )
        },
    )
    .unwrap();
    assert_eq!(tiled.pixels, expected.pixels);
    for (index, pixel) in tiled.pixels.iter().enumerate() {
        let x = index % 32;
        let gray = (127.5 + 127.5 * (x as f64 + 0.5) / 32.0).round() as u8;
        for channel in 0..3 {
            assert!(
                pixel[channel].abs_diff(gray) <= 1,
                "pixel {index}: {pixel:?} != {gray}"
            );
        }
    }
}

#[test]
fn actual_pdf_axial_shading_bbox_clips_across_tiles() {
    let pdf = authored_pdf_with_font(
        true,
        0.5,
        "/form Do",
        "1 0 0 rg 0 0 32 16 re f",
        "/S1 sh",
        "<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>",
        Some(
            "<< /ShadingType 2 /ColorSpace /DeviceRGB /BBox [4.5 2.5 27.5 13.5] /Coords [0 0 32 0] /Function << /FunctionType 2 /Domain [0 1] /C0 [0 0 0] /C1 [1 1 1] /N 1 >> /Extend [true true] >>",
        ),
    );
    let scene = record(pdf);
    let whole = replay_scene(&scene, 64, 32, Affine::scale(2.0), &|| false, &|_| {
        Some(Box::new(()))
    })
    .unwrap();
    let expected =
        reduce_spatial_samples(&whole.pixels, 32, 16, 2, &|| false, &|_| Some(Box::new(()))).unwrap();
    for edge in [3, 7] {
        let tiled = composite_spatial_tiles(
            32,
            16,
            2,
            edge,
            &|| false,
            &|_| Some(Box::new(())),
            &mut |x, y, w, h| {
                replay_scene(
                    &scene,
                    w as u16,
                    h as u16,
                    Affine::translate((-(x as f64) * 2.0, -(y as f64) * 2.0)) * Affine::scale(2.0),
                    &|| false,
                    &|_| Some(Box::new(())),
                )
            },
        )
        .unwrap();
        assert_eq!(tiled.pixels, expected.pixels);
    }
    for y in 0..16 {
        for x in 0..32 {
            let pixel = expected.pixels[y * 32 + x];
            if x < 4 || x >= 28 || y < 2 || y >= 14 {
                assert_eq!(pixel, [255, 0, 0, 255], "outside BBox at {x},{y}");
            } else if (5..27).contains(&x) && (3..13).contains(&y) {
                let gray = (127.5 + 127.5 * (x as f64 + 0.5) / 32.0).round() as u8;
                assert!(pixel[..3].iter().all(|c| c.abs_diff(gray) <= 1));
            }
        }
    }
}

#[test]
fn shading_bbox_capture_and_replay_transforms_agree() {
    let pdf = authored_pdf_with_font(
        true,
        0.5,
        "/form Do",
        "1 0 0 rg 0 0 32 16 re f",
        "/S1 sh",
        "<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>",
        Some(
            "<< /ShadingType 2 /ColorSpace /DeviceRGB /BBox [4.5 2.5 27.5 10.5] /Coords [0 0 32 0] /Function << /FunctionType 2 /Domain [0 1] /C0 [0 0 0] /C1 [1 1 1] /N 1 >> /Extend [true true] >>",
        ),
    );
    let transform = Affine::translate((4.0, 2.0)) * Affine::scale_non_uniform(2.0, 2.5);
    let original = record(pdf.clone());
    let transformed = record_at_transform(pdf, 96, 48, transform);
    let reference = replay_scene(&original, 96, 48, transform, &|| false, &|_| Some(Box::new(()))).unwrap();
    let actual = replay_scene(&transformed, 96, 48, Affine::IDENTITY, &|| false, &|_| {
        Some(Box::new(()))
    })
    .unwrap();
    assert_eq!(actual.pixels, reference.pixels);
    // The asymmetric BBox is transformed exactly once at capture.
    let clips = transformed
        .events
        .iter()
        .find_map(|event| match event {
            Event::Path(_, _, RecordedPaint::Shading(_), clips, _) => Some(clips),
            _ => None,
        })
        .unwrap();
    let bbox = kurbo::Shape::bounding_box(clips.last().unwrap().path.as_ref());
    assert_eq!(
        bbox,
        transform.transform_rect_bbox(kurbo::Rect::new(4.5, 2.5, 27.5, 10.5))
    );
}

#[test]
fn actual_pdf_radial_shading_matches_concentric_radius() {
    let pdf = authored_pdf_with_font(
        true,
        0.5,
        "/form Do",
        "1 0 0 rg 0 0 32 16 re f",
        "/S1 sh",
        "<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>",
        Some(
            "<< /ShadingType 3 /ColorSpace /DeviceRGB /Coords [16 8 0 16 8 8] /Function << /FunctionType 2 /Domain [0 1] /C0 [0 0 0] /C1 [1 1 1] /N 1 >> /Extend [true true] >>",
        ),
    );
    let scene = record(pdf);
    let pixels = replay_scene(&scene, 32, 16, Affine::IDENTITY, &|| false, &|_| {
        Some(Box::new(()))
    })
    .unwrap();
    for (index, pixel) in pixels.pixels.iter().enumerate() {
        let dx = (index % 32) as f64 + 0.5 - 16.0;
        let dy = (index / 32) as f64 + 0.5 - 8.0;
        let t = (dx.hypot(dy) / 8.0).min(1.0);
        let expected = (127.5 + 127.5 * t).round() as u8;
        assert!(
            pixel[..3].iter().all(|c| c.abs_diff(expected) <= 1),
            "radial pixel {index}: {pixel:?} expected {expected}"
        );
    }
    let tiled = composite_spatial_tiles(
        32,
        16,
        1,
        7,
        &|| false,
        &|_| Some(Box::new(())),
        &mut |x, y, w, h| {
            replay_scene(
                &scene,
                w as u16,
                h as u16,
                Affine::translate((-(x as f64), -(y as f64))),
                &|| false,
                &|_| Some(Box::new(())),
            )
        },
    )
    .unwrap();
    assert_eq!(pixels.pixels, tiled.pixels);
}

#[test]
fn actual_pdf_soft_mask_group_is_captured_on_transparent_backdrop() {
    let oracle_output = std::env::var_os("RRRAH_MASK_ORACLE_OUTPUT").map(std::path::PathBuf::from);
    if let Some(root) = &oracle_output {
        std::fs::create_dir(root).unwrap();
    }

    for knockout in [false, true] {
        for colored in [false, true] {
            if knockout && !colored {
                continue;
            }
            for nested in [false, true] {
                for restored in [false, true] {
                    if colored && !restored {
                        continue;
                    }
                    for (kind, opacity, invert, white_background) in [
                        ("Alpha", 1.0, false, false),
                        ("Luminosity", 1.0, false, false),
                        ("Alpha", 0.5, false, false),
                        ("Luminosity", 0.5, false, false),
                        ("Alpha", 0.5, true, false),
                        ("Luminosity", 0.5, true, false),
                        ("Alpha", 0.5, false, true),
                        ("Luminosity", 0.5, false, true),
                        ("Luminosity", 0.5, true, true),
                    ] {
                        let content = if nested {
                            "/N gs /P gs 0.5 g 0 0 16 16 re f"
                        } else {
                            "/P gs 0.5 g 0 0 16 16 re f"
                        };
                        let transfer = if invert {
                            "/TR << /FunctionType 2 /Domain [0 1] /C0 [1] /C1 [0] /N 1 >>"
                        } else {
                            ""
                        };
                        let background = if white_background { "/BC [1 1 1]" } else { "" };
                        let page_content = if knockout {
                            "/Parent Do 0 1 0 rg 24 0 8 16 re f"
                        } else if colored {
                            "1 0 0 rg 0 0 32 16 re f q 0 g /M gs /G Do Q 0 1 0 rg 24 0 8 16 re f"
                        } else if restored {
                            "q /M gs /G Do Q 0 1 0 rg 24 0 8 16 re f"
                        } else {
                            "/M gs 0 0 32 16 re f"
                        };
                        let mut objects = vec![
            "<< /Type /Catalog /Pages 2 0 R >>".to_string(),
            "<< /Type /Pages /Kids [3 0 R] /Count 1 >>".to_string(),
            "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 32 16] /Resources << /ExtGState << /M 5 0 R >> /XObject << /G 11 0 R /Parent 13 0 R >> >> /Contents 4 0 R >>".to_string(),
            format!("<< /Length {} >>\nstream\n{page_content}\nendstream",page_content.len()),
            format!("<< /SMask << /S /{kind} /G 6 0 R {transfer} {background} >> >>"),
            format!("<< /Type /XObject /Subtype /Form /BBox [0 0 32 16] /Group << /S /Transparency /I true /CS /DeviceRGB >> /Resources << /ExtGState << /P 7 0 R /N 8 0 R >> >> /Length {} >>\nstream\n{content}\nendstream",content.len()),
            format!("<< /ca {opacity} >>"),
        ];
                        if nested {
                            let inner = "/H gs 1 g 0 0 32 16 re f";
                            objects.push("<< /SMask << /S /Alpha /G 9 0 R >> >>".to_string());
                            objects.push(format!("<< /Type /XObject /Subtype /Form /BBox [0 0 32 16] /Group << /S /Transparency /I true >> /Resources << /ExtGState << /H 10 0 R >> >> /Length {} >>\nstream\n{inner}\nendstream",inner.len()));
                            objects.push("<< /ca 0.5 >>".to_string());
                        }
                        while objects.len() < 10 {
                            objects.push("null".to_string());
                        }
                        let isolated = !colored;
                        let group_content = if colored {
                            "/T gs 0 g 0 0 32 16 re f 0 0 32 16 re f"
                        } else {
                            "0 g 0 0 32 16 re f 0 0 32 16 re f"
                        };
                        objects.push(format!("<< /Type /XObject /Subtype /Form /BBox [0 0 32 16] /Group << /S /Transparency /I {isolated} >> /Resources << /ExtGState << /T 12 0 R >> >> /Length {} >>\nstream\n{group_content}\nendstream",group_content.len()));
                        objects.push("<< /ca 0.5 >>".to_string());
                        if knockout {
                            let parent = "q /Red Do Q q /M gs /G Do Q";
                            objects.push(format!("<< /Type /XObject /Subtype /Form /BBox [0 0 32 16] /Group << /S /Transparency /I false /K true >> /Resources << /ExtGState << /M 5 0 R >> /XObject << /G 11 0 R /Red 14 0 R >> >> /Length {} >>\nstream\n{parent}\nendstream",parent.len()));
                            let red = "1 0 0 rg 0 0 32 16 re f";
                            objects.push(format!("<< /Type /XObject /Subtype /Form /BBox [0 0 32 16] /Group << /S /Transparency /I true >> /Length {} >>\nstream\n{red}\nendstream",red.len()));
                        }
                        let mut pdf = b"%PDF-1.4\n".to_vec();
                        let mut offsets = vec![0];
                        for (index, object) in objects.iter().enumerate() {
                            offsets.push(pdf.len());
                            pdf.extend_from_slice(
                                format!("{} 0 obj\n{object}\nendobj\n", index + 1).as_bytes(),
                            );
                        }
                        let xref = pdf.len();
                        pdf.extend_from_slice(
                            format!("xref\n0 {}\n0000000000 65535 f \n", offsets.len()).as_bytes(),
                        );
                        for offset in &offsets[1..] {
                            pdf.extend_from_slice(format!("{offset:010} 00000 n \n").as_bytes());
                        }
                        pdf.extend_from_slice(
                            format!(
                                "trailer\n<< /Size {} /Root 1 0 R >>\nstartxref\n{xref}\n%%EOF\n",
                                offsets.len()
                            )
                            .as_bytes(),
                        );
                        let parsed = hayro_interpret::hayro_syntax::Pdf::new(pdf.clone()).unwrap();
                        let production = crate::render(
                            &parsed.pages()[0],
                            &crate::RenderCache::new(),
                            &hayro_interpret::InterpreterSettings::default(),
                            &crate::RenderSettings::default(),
                        );
                        let probe = record_into(
                            pdf.clone(),
                            32,
                            16,
                            Affine::IDENTITY,
                            Recorder {
                                mask_probe: true,
                                global_contour: true,
                                canonical_f32: true,
                                ..Recorder::default()
                            },
                        );
                        assert!(!probe.mask_results.is_empty());
                        let replayed = replay_scene(&probe, 32, 16, Affine::IDENTITY, &|| false, &|_| {
                            Some(Box::new(()))
                        })
                        .unwrap();
                        let native_probe = record_into(
                            pdf.clone(),
                            32,
                            16,
                            Affine::IDENTITY,
                            Recorder {
                                mask_probe: true,
                                global_contour: true,
                                canonical_f32: true,
                                native_polygon: true,
                                ..Recorder::default()
                            },
                        );
                        let native =
                            replay_scene(&native_probe, 32, 16, Affine::IDENTITY, &|| false, &|_| {
                                Some(Box::new(()))
                            })
                            .unwrap();
                        assert_eq!(native.pixels, replayed.pixels, "native masked fixture {kind}");
                        for edge in [3, 7] {
                            let tiled = composite_precise_spatial_tiles(
                                32,
                                16,
                                1,
                                edge,
                                &|| false,
                                &|_| Some(Box::new(())),
                                &mut |x, y, w, h| {
                                    replay_scene_precise_with_backdrop(
                                        &native_probe,
                                        w as u16,
                                        h as u16,
                                        Affine::translate((-(x as f64), -(y as f64))),
                                        [255; 4],
                                        &|| false,
                                        &|_| Some(Box::new(())),
                                    )
                                },
                            )
                            .unwrap();
                            assert_eq!(
                                tiled.pixels, native.pixels,
                                "native masked tiles {kind} edge={edge}"
                            );
                        }
                        if let Some(root) = &oracle_output {
                            let name = format!(
                                "ko-{knockout}-color-{colored}-nested-{nested}-restore-{restored}-{kind}-{opacity}-bc-{white_background}-invert-{invert}"
                            );
                            std::fs::write(root.join(format!("{name}.pdf")), &pdf).unwrap();
                            let raw: Vec<u8> = replayed.pixels.iter().flatten().copied().collect();
                            std::fs::write(root.join(format!("{name}.rgba")), raw).unwrap();
                        }
                        for (index, pixel) in replayed.pixels.iter().enumerate() {
                            if restored && index % 32 >= 24 {
                                assert_eq!(*pixel, [0, 255, 0, 255], "restored mask at {index}");
                                continue;
                            }
                            if knockout {
                                let mask = probe.mask_results.last().unwrap().values[index];
                                let expected = 255 - ((u16::from(mask) * 192 + 127) / 255) as u8;
                                assert!(
                                    pixel[..3].iter().all(|c| c.abs_diff(expected) <= 1),
                                    "knockout masked group {index}: {pixel:?} != {expected}"
                                );
                                continue;
                            }
                            let original = &production.data_as_u8_slice()[index * 4..index * 4 + 4];
                            for channel in 0..3 {
                                let expected = u16::from(original[channel]) + 255 - u16::from(original[3]);
                                assert!(u16::from(pixel[channel]).abs_diff(expected) <= 1);
                            }
                        }
                        let spatial = replay_scene(&probe, 64, 32, Affine::scale(2.0), &|| false, &|_| {
                            Some(Box::new(()))
                        })
                        .unwrap();
                        let whole = reduce_spatial_samples(&spatial.pixels, 32, 16, 2, &|| false, &|_| {
                            Some(Box::new(()))
                        })
                        .unwrap();
                        for edge in [3, 7] {
                            let tiled = composite_spatial_tiles(
                                32,
                                16,
                                2,
                                edge,
                                &|| false,
                                &|_| Some(Box::new(())),
                                &mut |x, y, w, h| {
                                    replay_scene(
                                        &probe,
                                        w as u16,
                                        h as u16,
                                        Affine::translate((-(x as f64) * 2.0, -(y as f64) * 2.0))
                                            * Affine::scale(2.0),
                                        &|| false,
                                        &|_| Some(Box::new(())),
                                    )
                                },
                            )
                            .unwrap();
                            assert_eq!(
                                tiled.pixels, whole.pixels,
                                "masked tiles {kind} opacity={opacity} invert={invert} edge={edge}"
                            );
                        }
                        if kind == "Alpha" && opacity == 0.5 && !invert && !white_background {
                            use std::cell::Cell;
                            let polls = Cell::new(0);
                            assert!(
                                replay_scene(
                                    &probe,
                                    32,
                                    16,
                                    Affine::IDENTITY,
                                    &|| {
                                        polls.set(polls.get() + 1);
                                        false
                                    },
                                    &|_| Some(Box::new(()))
                                )
                                .is_some()
                            );
                            use std::sync::{
                                Arc,
                                atomic::{AtomicUsize, Ordering},
                            };
                            struct Credit(Arc<AtomicUsize>, usize);
                            impl Drop for Credit {
                                fn drop(&mut self) {
                                    self.0.fetch_sub(self.1, Ordering::Relaxed);
                                }
                            }
                            let capture_calls = Arc::new(AtomicUsize::new(0));
                            let counter = capture_calls.clone();
                            let baseline = record_into(
                                pdf.clone(),
                                32,
                                16,
                                Affine::IDENTITY,
                                Recorder {
                                    mask_probe: true,
                                    admission: Some(std::rc::Rc::new(move |_| {
                                        counter.fetch_add(1, Ordering::Relaxed);
                                        Some(Box::new(()))
                                    })),
                                    ..Recorder::default()
                                },
                            );
                            assert!(!baseline.failed);
                            drop(baseline);
                            for deny in 1..=capture_calls.load(Ordering::Relaxed) {
                                let calls = Arc::new(AtomicUsize::new(0));
                                let counter = calls.clone();
                                let used = Arc::new(AtomicUsize::new(0));
                                let credit_used = used.clone();
                                let failed = record_into(
                                    pdf.clone(),
                                    32,
                                    16,
                                    Affine::IDENTITY,
                                    Recorder {
                                        mask_probe: true,
                                        admission: Some(std::rc::Rc::new(move |bytes| {
                                            if counter.fetch_add(1, Ordering::Relaxed) + 1 == deny {
                                                return None;
                                            }
                                            credit_used.fetch_add(bytes, Ordering::Relaxed);
                                            Some(Box::new(Credit(credit_used.clone(), bytes)))
                                        })),
                                        ..Recorder::default()
                                    },
                                );
                                assert!(failed.failed, "mask capture refusal {deny}");
                                drop(failed);
                                assert_eq!(used.load(Ordering::Relaxed), 0);
                            }
                            let capture_polls = Arc::new(AtomicUsize::new(0));
                            let counter = capture_polls.clone();
                            let baseline = record_into(
                                pdf.clone(),
                                32,
                                16,
                                Affine::IDENTITY,
                                Recorder {
                                    mask_probe: true,
                                    cancelled: Some(Arc::new(move || {
                                        counter.fetch_add(1, Ordering::Relaxed);
                                        false
                                    })),
                                    ..Recorder::default()
                                },
                            );
                            assert!(!baseline.failed);
                            drop(baseline);
                            for stop in 1..=capture_polls.load(Ordering::Relaxed) {
                                let calls = Arc::new(AtomicUsize::new(0));
                                let counter = calls.clone();
                                let used = Arc::new(AtomicUsize::new(0));
                                let credit_used = used.clone();
                                let failed = record_into(
                                    pdf.clone(),
                                    32,
                                    16,
                                    Affine::IDENTITY,
                                    Recorder {
                                        mask_probe: true,
                                        cancelled: Some(Arc::new(move || {
                                            counter.fetch_add(1, Ordering::Relaxed) + 1 >= stop
                                        })),
                                        admission: Some(std::rc::Rc::new(move |bytes| {
                                            credit_used.fetch_add(bytes, Ordering::Relaxed);
                                            Some(Box::new(Credit(credit_used.clone(), bytes)))
                                        })),
                                        ..Recorder::default()
                                    },
                                );
                                assert!(failed.failed, "mask capture cancel {stop}");
                                assert!(
                                    replay_scene(&failed, 32, 16, Affine::IDENTITY, &|| false, &|_| panic!(
                                        "partial mask replay admitted"
                                    ))
                                    .is_none()
                                );
                                drop(failed);
                                assert_eq!(used.load(Ordering::Relaxed), 0, "mask capture cancel {stop}");
                            }
                            let admissions = Cell::new(0);
                            drop(replay_scene(&probe, 32, 16, Affine::IDENTITY, &|| false, &|_| {
                                admissions.set(admissions.get() + 1);
                                Some(Box::new(()))
                            }));
                            for deny in 1..=admissions.get() {
                                let used = Arc::new(AtomicUsize::new(0));
                                let current = Cell::new(0);
                                assert!(
                                    replay_scene(&probe, 32, 16, Affine::IDENTITY, &|| false, &|bytes| {
                                        current.set(current.get() + 1);
                                        if current.get() == deny {
                                            return None;
                                        }
                                        used.fetch_add(bytes, Ordering::Relaxed);
                                        Some(Box::new(Credit(used.clone(), bytes)))
                                    })
                                    .is_none()
                                );
                                assert_eq!(used.load(Ordering::Relaxed), 0, "masked admission {deny}");
                            }
                            for stop in 1..=polls.get() {
                                let current = Cell::new(0);
                                assert!(
                                    replay_scene(
                                        &probe,
                                        32,
                                        16,
                                        Affine::IDENTITY,
                                        &|| {
                                            current.set(current.get() + 1);
                                            current.get() >= stop
                                        },
                                        &|_| Some(Box::new(()))
                                    )
                                    .is_none()
                                );
                                assert_eq!(current.get(), stop);
                            }
                        }
                        for mask in probe.mask_results {
                            for (index, &value) in mask.values.iter().enumerate() {
                                let alpha = if opacity == 1.0 { 255u8 } else { 128 };
                                let alpha = if nested {
                                    ((u16::from(alpha) * 128 + 127) / 255) as u8
                                } else {
                                    alpha
                                };
                                let base: u8 = if index % 32 >= 16 {
                                    if kind == "Luminosity" && white_background {
                                        255
                                    } else {
                                        0
                                    }
                                } else if kind == "Alpha" {
                                    alpha
                                } else {
                                    let gray = ((128u16 * u16::from(alpha) + 127) / 255) as u8;
                                    if white_background {
                                        gray + (255 - alpha)
                                    } else {
                                        gray
                                    }
                                };
                                let expected = if invert { 255 - base } else { base };
                                if knockout {
                                    assert!(value.abs_diff(expected) <= 1);
                                    continue;
                                }
                                let pixel = &production.data_as_u8_slice()[index * 4..index * 4 + 4];
                                if restored && index % 32 >= 24 {
                                    assert_eq!(pixel, &[0, 255, 0, 255]);
                                    assert!(value.abs_diff(expected) <= 1);
                                    continue;
                                }
                                if colored {
                                    assert!(
                                        pixel[0]
                                            .abs_diff(255 - ((u16::from(expected) * 192 + 127) / 255) as u8)
                                            <= 1
                                    );
                                    assert_eq!(&pixel[1..], [0, 0, 255]);
                                    assert!(value.abs_diff(expected) <= 1);
                                    continue;
                                }
                                assert_eq!(&pixel[..3], &[0, 0, 0]);
                                assert!(
                                    pixel[3].abs_diff(expected) <= 1,
                                    "production {kind} opacity={opacity} invert={invert} pixel {index}: {pixel:?} != {expected}"
                                );
                                assert!(
                                    value.abs_diff(expected) <= 1,
                                    "{kind} mask pixel {index}: {value} != {expected}"
                                );
                            }
                        }
                    }
                }
            }
        }
    }
}

#[test]
#[ignore = "requires explicit real AI corpus and fresh diagnostic output directory"]
fn real_ai_compositor_capture_and_replay_diagnostic() {
    use hayro_interpret::TransformExt;
    use std::sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    };
    use std::time::{Duration, Instant};
    struct Credit(Arc<AtomicUsize>, usize);
    impl Drop for Credit {
        fn drop(&mut self) {
            self.0.fetch_sub(self.1, Ordering::Relaxed);
        }
    }
    let source = std::path::PathBuf::from(
        std::env::var_os("RRRAH_AI_REPLAY_CORPUS").expect("explicit corpus required"),
    );
    let output =
        std::path::PathBuf::from(std::env::var_os("RRRAH_AI_REPLAY_OUTPUT").expect("fresh output required"));
    std::fs::create_dir(&output).unwrap();
    let mut rows = Vec::new();
    for name in ["VectorApple.ai", "one.ai"] {
        let pdf = hayro_interpret::hayro_syntax::Pdf::new(std::fs::read(source.join(name)).unwrap()).unwrap();
        for (index, page) in pdf.pages().iter().enumerate() {
            let (width, height) = page.render_dimensions();
            assert!(width > 0.0 && width <= 4096.0 && height > 0.0 && height <= 4096.0);
            let (width, height) = (width.ceil() as u16, height.ceil() as u16);
            let used = Arc::new(AtomicUsize::new(0));
            let peak = Arc::new(AtomicUsize::new(0));
            let counter = used.clone();
            let high = peak.clone();
            let admission = std::rc::Rc::new(
                move |bytes: usize| -> Option<Box<dyn std::any::Any + Send + Sync>> {
                    let current = counter.load(Ordering::Relaxed);
                    let next = current.checked_add(bytes)?;
                    if next > 256 * 1024 * 1024 {
                        return None;
                    }
                    counter.store(next, Ordering::Relaxed);
                    high.fetch_max(next, Ordering::Relaxed);
                    Some(Box::new(Credit(counter.clone(), bytes)))
                },
            );
            let start = Instant::now();
            let cancelled = Arc::new(move || start.elapsed() > Duration::from_secs(30));
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                let cache = crate::RenderCache::new();
                let mut context = hayro_interpret::Context::new(
                    page.initial_transform(true).to_kurbo(),
                    kurbo::Rect::new(0.0, 0.0, f64::from(width), f64::from(height)),
                    &cache.interpreter_cache,
                    page.xref(),
                    hayro_interpret::InterpreterSettings::default(),
                );
                let mut scene = Recorder {
                    width,
                    height,
                    mask_probe: true,
                    capture_native_rasters: std::env::var_os("RRRAH_AI_NATIVE_RASTER_CAPTURE").is_some(),
                    capture_native_shadings: std::env::var_os("RRRAH_AI_NATIVE_SHADING_CAPTURE").is_some(),
                    global_contour: std::env::var_os("RRRAH_AI_GLOBAL_CONTOUR").is_some(),
                    canonical_f32: std::env::var_os("RRRAH_AI_CANONICAL_F32").is_some(),
                    native_polygon: std::env::var_os("RRRAH_AI_NATIVE_POLYGON").is_some(),
                    admitted_strokes: std::env::var_os("RRRAH_AI_ADMITTED_STROKES").is_some(),
                    admission: Some(admission.clone()),
                    cancelled: Some(cancelled.clone()),
                    ..Recorder::default()
                };
                hayro_interpret::interpret_page(page, &mut context, &mut scene);
                let capture_ms = start.elapsed().as_secs_f64() * 1000.0;
                let capture_deadline_expired = cancelled();
                let events = scene.events.len();
                let groups = scene
                    .events
                    .iter()
                    .filter(|event| matches!(event,Event::Push(_,props,_) if props.knockout))
                    .count();
                if std::env::var_os("RRRAH_AI_NATIVE_SOLID_DIAGNOSTIC").is_some() {
                    use hayro_interpret::color::{AlphaColor,Color,BlendingModel};
                    use super::native_blend::{Model,Pixel};
                    let rgb = Color::from_rgba(AlphaColor::WHITE);
                    let space = match &scene.page_group_space {
                        hayro_interpret::TransparencyGroupColorSpace::Explicit(space) => space,
                        hayro_interpret::TransparencyGroupColorSpace::Inherited => rgb.color_space(),
                        _ => return "\"status\":\"native_unsupported_page_space\"".to_owned(),
                    };
                    let model = match space.blending_model() {
                        Some(BlendingModel::Gray) => Model::Gray,
                        Some(BlendingModel::Rgb) => Model::Rgb,
                        Some(BlendingModel::Cmyk) => Model::Cmyk,
                        _ => return "\"status\":\"native_unsupported_page_space\"".to_owned(),
                    };
                    let white = if matches!(model,Model::Cmyk) { [0.;4] } else { [1.;4] };
                    let backdrop = Pixel::from_native(model,&white[..model.channels()],1.).unwrap();
                    let rasters = scene.events.iter().filter(|e| matches!(e,Event::Raster(..))).count();
                    let shadings = scene.events.iter().filter(|e| matches!(e,Event::Path(_,_,RecordedPaint::Shading(_),_,_))).count();
                    let native_raster_components: Vec<_> = scene.native_rasters.iter().map(|(index,data,refusal)| format!("event={index},sample_count={:?},refusal={refusal:?}",data.as_ref().map(|data| data.samples().len()))).collect();
                    let raster_models: Vec<_> = scene.raster_spaces.iter().map(|(index,space)| format!("event={index},model={:?}",space.as_ref().and_then(|space| space.blending_model()))).collect();
                    let paints = scene.events.iter().filter(|e| matches!(e,Event::Path(_,_,RecordedPaint::NativeColor(_),_,_))).count();
                    let replay_start = Instant::now();
                    let native = replay_native_solid_scene(&scene,width,height,space,backdrop,&*cancelled,&*admission);
                    let completed = native.is_some();
                    if matches!(model,Model::Rgb) {
                        if let Some(pixels) = &native {
                            use std::io::Write;
                            let mut file = std::fs::File::create(output.join(format!("{name}-page-{index}.native.rgba"))).unwrap();
                            for pixel in &pixels.pixels {
                                assert_eq!(pixel.alpha(),1.0);
                                let c = pixel.native_components().unwrap();
                                file.write_all(&[(c[0]*255.).round() as u8,(c[1]*255.).round() as u8,(c[2]*255.).round() as u8,255]).unwrap();
                            }
                        }
                    }
                    drop(native);
                    return format!("\"status\":\"native_diagnostic\",\"native_completed\":{completed},\"capture_failed\":{},\"capture_ms\":{capture_ms},\"native_replay_ms\":{},\"deadline_expired\":{},\"events\":{events},\"native_paints\":{paints},\"raster_events\":{rasters},\"raster_source_models\":{raster_models:?},\"native_raster_components\":{native_raster_components:?},\"shading_events\":{shadings},\"page_model\":{:?}",scene.failed,replay_start.elapsed().as_secs_f64()*1000.,cancelled(),format!("{model:?}"));
                }
                if std::env::var_os("RRRAH_AI_TRACE_COVERAGE").is_some() && index == 0 {
                    let points: &[(usize, usize)] = if name == "VectorApple.ai" {
                        &[(144, 66), (153, 165)]
                    } else {
                        &[
                            (1150, 249),
                            (453, 391),
                            (453, 403),
                            (1044, 192),
                            (372, 204),
                            (266, 210),
                            (1123, 214),
                            (987, 241),
                            (960, 303),
                            (954, 327),
                            (1179, 330),
                            (449, 346),
                            (1161, 522),
                            (1135, 567),
                            (1010, 580),
                            (1101, 597),
                        ]
                    };
                    let mut traces = Vec::new();
                    for (event_index, event) in scene.events.iter().enumerate() {
                        let Event::Path(path, transform, paint, clips, PathDrawMode::Fill(rule)) = event
                        else {
                            continue;
                        };
                        let fill = match rule {
                            hayro_interpret::FillRule::NonZero => vello_cpu::peniko::Fill::NonZero,
                            hayro_interpret::FillRule::EvenOdd => vello_cpu::peniko::Fill::EvenOdd,
                        };
                        let render = |w, h, offset| {
                            rasterize_path_shape_with_transformed_clips(
                                path,
                                offset * *transform,
                                clips.iter().map(|clip| {
                                    (
                                        clip.path.as_ref(),
                                        match clip.fill {
                                            hayro_interpret::FillRule::NonZero => {
                                                vello_cpu::peniko::Fill::NonZero
                                            }
                                            hayro_interpret::FillRule::EvenOdd => {
                                                vello_cpu::peniko::Fill::EvenOdd
                                            }
                                        },
                                    )
                                }),
                                offset,
                                ShapeDrawMode::Fill(fill),
                                w,
                                h,
                                &*cancelled,
                                &*admission,
                            )
                        };
                        let Some(whole) = render(width, height, Affine::IDENTITY) else {
                            continue;
                        };
                        for &(x, y) in points {
                            let (ox, oy) = (x / 64 * 64, y / 64 * 64);
                            let tw = 64.min(usize::from(width) - ox);
                            let th = 64.min(usize::from(height) - oy);
                            let Some(tile) = render(
                                tw as u16,
                                th as u16,
                                Affine::translate((-(ox as f64), -(oy as f64))),
                            ) else {
                                continue;
                            };
                            let a = whole.values[y * usize::from(width) + x];
                            let b = tile.values[(y - oy) * tw + x - ox];
                            if a != b {
                                let paint = match paint {
                                    RecordedPaint::Color(_) | RecordedPaint::NativeColor(_) => "color",
                                    RecordedPaint::Shading(_) => "shading",
                                };
                                traces.push(format!("{{\"event\":{event_index},\"x\":{x},\"y\":{y},\"whole_shape\":{a},\"tile_shape\":{b},\"paint\":{paint:?}}}"));
                            }
                        }
                    }
                    std::fs::write(
                        output.join(format!("{name}-coverage.json")),
                        format!("[{}]\n", traces.join(",")),
                    )
                    .unwrap();
                }
                let whole_start = Instant::now();
                let result = replay_scene(&scene, width, height, Affine::IDENTITY, &*cancelled, &*admission);
                if let Some(pixels) = result {
                    let whole_ms = whole_start.elapsed().as_secs_f64() * 1000.0;
                    let mut tile_detail = String::new();
                    if let Some(edge) = std::env::var("RRRAH_AI_REPLAY_TILE_EDGE")
                        .ok()
                        .map(|v| v.parse::<usize>().unwrap())
                    {
                        assert!(edge > 0 && edge <= 256);
                        let whole_peak = peak.load(Ordering::Relaxed);
                        let retained_baseline = used.load(Ordering::Relaxed);
                        peak.store(retained_baseline, Ordering::Relaxed);
                        let tile_start = Instant::now();
                        let tiled = composite_precise_spatial_tiles(
                            usize::from(width),
                            usize::from(height),
                            1,
                            edge,
                            &*cancelled,
                            &*admission,
                            &mut |x, y, w, h| {
                                replay_scene_precise_with_backdrop(
                                    &scene,
                                    w as u16,
                                    h as u16,
                                    Affine::translate((-(x as f64), -(y as f64))),
                                    [255; 4],
                                    &*cancelled,
                                    &*admission,
                                )
                            },
                        );
                        let tile_ms = tile_start.elapsed().as_secs_f64() * 1000.0;
                        let tile_peak = peak.load(Ordering::Relaxed);
                        peak.store(whole_peak.max(tile_peak), Ordering::Relaxed);
                        let different = tiled.as_ref().map(|tile| {
                            tile.pixels
                                .iter()
                                .zip(&pixels.pixels)
                                .filter(|(a, b)| a != b)
                                .count()
                        });
                        if let Some(tile) = tiled.as_ref() {
                            std::fs::write(
                                output.join(format!("{name}-page-{index}.tile-{edge}.rgba")),
                                tile.pixels.iter().flatten().copied().collect::<Vec<_>>(),
                            )
                            .unwrap();
                        }
                        let difference = different.map_or("null".to_owned(), |v| v.to_string());
                        tile_detail = format!(
                            ",\"tile_edge\":{edge},\"tile_replay_ms\":{tile_ms},\"tile_different_pixels\":{difference},\"tile_peak_admitted_bytes\":{tile_peak},\"tile_retained_comparison_baseline_bytes\":{retained_baseline}"
                        );
                    }
                    let bytes: Vec<_> = pixels.pixels.iter().flatten().copied().collect();
                    std::fs::write(output.join(format!("{name}-page-{index}.rgba")), bytes).unwrap();
                    format!(
                        "\"status\":\"replayed\",\"events\":{events},\"knockout_groups\":{groups},\"capture_ms\":{capture_ms},\"capture_deadline_expired\":{capture_deadline_expired},\"whole_replay_ms\":{whole_ms}{tile_detail}"
                    )
                } else {
                    format!(
                        "\"status\":\"refused\",\"capture_failed\":{},\"capture_ms\":{capture_ms},\"capture_deadline_expired\":{capture_deadline_expired},\"deadline_expired\":{},\"events\":{events}",
                        scene.failed, cancelled()
                    )
                }
            }));
            let detail = match result {
                Ok(detail) => detail,
                Err(error) => {
                    let message = error
                        .downcast_ref::<&str>()
                        .copied()
                        .or_else(|| error.downcast_ref::<String>().map(String::as_str))
                        .unwrap_or("unknown panic");
                    format!("\"status\":\"unsupported\",\"reason\":{message:?}")
                }
            };
            assert_eq!(
                used.load(Ordering::Relaxed),
                0,
                "all capture/replay credits must be released"
            );
            rows.push(format!("{{\"fixture\":{name:?},\"page\":{index},\"width\":{width},\"height\":{height},\"peak_admitted_bytes\":{},\"final_admitted_bytes\":0,{detail}}}",peak.load(Ordering::Relaxed)));
        }
    }
    std::fs::write(
        output.join("report.json"),
        format!("[\n{}\n]\n", rows.join(",\n")),
    )
    .unwrap();
}

#[test]
fn prepared_fill_capture_refusal_releases_geometry_before_partial_replay() {
    use std::sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    };
    struct Credit(Arc<AtomicUsize>, usize);
    impl Drop for Credit {
        fn drop(&mut self) {
            self.0.fetch_sub(self.1, Ordering::Relaxed);
        }
    }
    let pdf = authored_pdf_with_blue(
        true,
        0.5,
        "/form Do",
        "0 0 1 rg 8 0 m 20 24 32 -8 32 16 c 8 16 l h f",
    );
    let attempts = Arc::new(AtomicUsize::new(0));
    let counted = attempts.clone();
    let baseline = record_into(
        pdf.clone(),
        32,
        16,
        Affine::IDENTITY,
        Recorder {
            global_contour: true,
            admission: Some(std::rc::Rc::new(move |_| {
                counted.fetch_add(1, Ordering::Relaxed);
                Some(Box::new(()))
            })),
            ..Recorder::default()
        },
    );
    assert!(!baseline.failed);
    assert!(
        replay_scene(&baseline, 32, 16, Affine::IDENTITY, &|| false, &|_| Some(
            Box::new(())
        ))
        .is_some()
    );
    drop(baseline);
    for deny in 1..=attempts.load(Ordering::Relaxed) {
        let calls = Arc::new(AtomicUsize::new(0));
        let count = calls.clone();
        let used = Arc::new(AtomicUsize::new(0));
        let balance = used.clone();
        let failed = record_into(
            pdf.clone(),
            32,
            16,
            Affine::IDENTITY,
            Recorder {
                global_contour: true,
                admission: Some(std::rc::Rc::new(move |bytes| {
                    if count.fetch_add(1, Ordering::Relaxed) + 1 == deny {
                        return None;
                    }
                    balance.fetch_add(bytes, Ordering::Relaxed);
                    Some(Box::new(Credit(balance.clone(), bytes)))
                })),
                ..Recorder::default()
            },
        );
        assert!(failed.failed, "capture admission {deny}");
        assert!(
            replay_scene(&failed, 32, 16, Affine::IDENTITY, &|| false, &|_| panic!(
                "partial scene admitted"
            ))
            .is_none()
        );
        drop(failed);
        assert_eq!(used.load(Ordering::Relaxed), 0);
    }
}

#[test]
fn prepared_clips_share_only_identical_geometry_and_preserve_fill_rule() {
    let mut path = BezPath::new();
    path.move_to((0.0, 0.0));
    path.curve_to((0.0, 8.0), (8.0, 8.0), (8.0, 0.0));
    path.close_path();
    let mut recorder = Recorder::default();
    let first = recorder
        .shared_clip(
            ClipPath {
                path: path.clone(),
                fill: hayro_interpret::FillRule::NonZero,
            },
            true,
        )
        .unwrap();
    let second = recorder
        .shared_clip(
            ClipPath {
                path: path.clone(),
                fill: hayro_interpret::FillRule::EvenOdd,
            },
            true,
        )
        .unwrap();
    assert!(std::rc::Rc::ptr_eq(&first.path, &second.path));
    assert_eq!(second.fill, hayro_interpret::FillRule::EvenOdd);
    path.line_to((9.0, 0.0));
    let different = recorder
        .shared_clip(
            ClipPath {
                path,
                fill: hayro_interpret::FillRule::NonZero,
            },
            true,
        )
        .unwrap();
    assert!(!std::rc::Rc::ptr_eq(&first.path, &different.path));
    assert_eq!(recorder.prepared_clips.len(), 2);
}

#[test]
fn canonical_global_vertices_preserve_independent_cubic_area_control() {
    use kurbo::{Circle, Shape};
    let source = Circle::new((320.125, 220.375), 99.1875).to_path(0.1);
    let mut recorder = Recorder {
        canonical_f32: true,
        ..Recorder::default()
    };
    let path = recorder.prepared_contour(&source, Affine::IDENTITY).unwrap();
    for (width, height, transform, index) in [
        (512, 384, Affine::IDENTITY, 317 * 512 + 341),
        (64, 64, Affine::translate((-320.0, -256.0)), 61 * 64 + 21),
    ] {
        let shape = rasterize_fill_shape(&path, transform, None, width, height, &|| false, &|_| {
            Some(Box::new(()))
        })
        .unwrap();
        assert_eq!(shape.values[index], 63); // Independent cubic-area reference.
    }
    let mut invalid = BezPath::new();
    invalid.move_to((f64::from(f32::MAX) * 2.0, 0.0));
    assert!(recorder.prepared_contour(&invalid, Affine::IDENTITY).is_none());
    assert!(recorder.failed);
}

#[test]
fn prepared_global_scene_matches_tiles_at_capture_scales_and_rotations() {
    let cases = prepared_transform_cases(false, 0, true, true);
    assert_eq!(
        cases
            .iter()
            .filter(|(_, _, _, different)| *different != 0)
            .count(),
        0,
        "prepared scene transform/tile gate"
    );
}

fn prepared_transform_cases(
    outline_diagnostic: bool,
    gutter: u16,
    native_polygon: bool,
    admitted_strokes: bool,
) -> Vec<(usize, usize, usize, usize)> {
    let mut cases = Vec::new();
    let fixtures = [
        authored_pdf_with_blue(
            true,
            0.5,
            "/form Do",
            "0 0 1 rg 8 0 m 20 24 32 -8 32 16 c 8 16 l h f",
        ),
        authored_pdf_with_blue(
            true,
            0.5,
            "q 2.5 1.5 27 13 re W n /form Do Q",
            "0 0 1 RG 3 w 1 J [4 3] 1 d 0 8 m 32 8 l S",
        ),
        authored_pdf_with_blue(
            true,
            0.5,
            "/form Do",
            "0 0 1 RG 1.5 w 8 0 m 20 24 32 -8 32 16 c S",
        ),
    ];
    for (fixture, pdf) in fixtures.into_iter().enumerate() {
        for (case, transform) in [
            Affine::IDENTITY,
            Affine::scale(0.75),
            Affine::scale(1.5),
            Affine::translate((12.25, 8.75)) * Affine::rotate(0.17),
            Affine::translate((48.0, 0.0)) * Affine::scale_non_uniform(-1.0, 1.0),
        ]
        .into_iter()
        .enumerate()
        {
            let mut scene = record_into(
                pdf.clone(),
                64,
                48,
                transform,
                Recorder {
                    global_contour: true,
                    canonical_f32: true,
                    native_polygon,
                    admitted_strokes,
                    ..Recorder::default()
                },
            );
            assert!(!scene.failed);
            if outline_diagnostic {
                // Oracle experiment only: kurbo stroke scratch has no allocation hook.
                // This must not be interpreted as an admitted production implementation.
                let events = std::mem::take(&mut scene.events);
                for event in events {
                    let event = match event {
                        Event::Path(path, transform, paint, clips, PathDrawMode::Stroke(props)) => {
                            let stroke = kurbo::Stroke {
                                width: f64::from(props.line_width),
                                join: props.line_join,
                                miter_limit: f64::from(props.miter_limit),
                                start_cap: props.line_cap,
                                end_cap: props.line_cap,
                                dash_pattern: props.dash_array.iter().map(|&v| f64::from(v)).collect(),
                                dash_offset: f64::from(props.dash_offset),
                            };
                            let outline = kurbo::stroke(
                                path.elements().iter().copied(),
                                &stroke,
                                &kurbo::StrokeOpts::default(),
                                0.0001,
                            );
                            let path = scene.prepared_contour(&outline, transform).unwrap();
                            Event::Path(
                                path,
                                Affine::IDENTITY,
                                paint,
                                clips,
                                PathDrawMode::Fill(hayro_interpret::FillRule::NonZero),
                            )
                        }
                        other => other,
                    };
                    scene.events.push(event);
                }
            }
            let whole = replay_scene(&scene, 64, 48, Affine::IDENTITY, &|| false, &|_| {
                Some(Box::new(()))
            })
            .unwrap();
            for edge in [3, 7, 16] {
                let tile = composite_precise_spatial_tiles(
                    64,
                    48,
                    1,
                    edge,
                    &|| false,
                    &|_| Some(Box::new(())),
                    &mut |x, y, w, h| {
                        replay_precise_tile_with_gutter(
                            &scene,
                            w as u16,
                            h as u16,
                            Affine::translate((-(x as f64), -(y as f64))),
                            gutter,
                            &|| false,
                            &|_| Some(Box::new(())),
                        )
                    },
                )
                .unwrap();
                let differences = tile
                    .pixels
                    .iter()
                    .zip(&whole.pixels)
                    .filter(|(a, b)| a != b)
                    .count();
                eprintln!(
                    "TRANSFORM_CASE {{\"fixture\":{fixture},\"transform\":{case},\"edge\":{edge},\"different_pixels\":{differences}}}"
                );
                if differences != 0 {
                    for (index, (a, b)) in tile
                        .pixels
                        .iter()
                        .zip(&whole.pixels)
                        .enumerate()
                        .filter(|(_, (a, b))| a != b)
                        .take(20)
                    {
                        let x = index % 64;
                        let y = index / 64;
                        eprintln!(
                            "TRANSFORM_PIXEL {{\"fixture\":{fixture},\"transform\":{case},\"edge\":{edge},\"x\":{x},\"y\":{y},\"tile\":{a:?},\"whole\":{b:?}}}"
                        );
                        trace_prepared_leaf_shapes(&scene, x, y, edge, gutter);
                    }
                }
                cases.push((fixture, case, edge, differences));
            }
        }
    }
    cases
}

#[test]
#[ignore = "explicit unadmitted stroke-outline oracle experiment; fresh report path required"]
fn global_stroke_outline_transform_diagnostic() {
    let output = std::env::var_os("RRRAH_STROKE_OUTLINE_REPORT").unwrap();
    let output = std::path::PathBuf::from(output);
    assert!(!output.exists());
    let gutter = std::env::var("RRRAH_STROKE_OUTLINE_GUTTER")
        .ok()
        .map(|v| v.parse::<u16>().unwrap())
        .unwrap_or(0);
    let native = std::env::var_os("RRRAH_NATIVE_POLYGON").is_some();
    let cases = prepared_transform_cases(true, gutter, native, false);
    let rows: Vec<_> = cases.iter().map(|(fixture,transform,edge,different)|
        format!("{{\"fixture\":{fixture},\"transform\":{transform},\"edge\":{edge},\"different_pixels\":{different}}}")).collect();
    std::fs::write(output, format!("[{}]\n", rows.join(","))).unwrap();
    assert!(cases.iter().all(|(_, _, _, different)| *different == 0));
}

fn replay_precise_tile_with_gutter(
    scene: &Recorder,
    width: u16,
    height: u16,
    transform: Affine,
    gutter: u16,
    cancelled: &dyn Fn() -> bool,
    admit: &dyn Fn(usize) -> Option<Box<dyn std::any::Any + Send + Sync>>,
) -> Option<PreciseBuffer> {
    if width == 0 || height == 0 || cancelled() {
        return None;
    }
    let double = gutter.checked_mul(2)?;
    let padded_width = width.checked_add(double)?;
    let padded_height = height.checked_add(double)?;
    let mut padded = replay_scene_precise_with_backdrop(
        scene,
        padded_width,
        padded_height,
        Affine::translate((f64::from(gutter), f64::from(gutter))) * transform,
        [255; 4],
        cancelled,
        admit,
    )?;
    for y in 0..usize::from(height) {
        if cancelled() {
            return None;
        }
        let src = (y + usize::from(gutter)) * usize::from(padded_width) + usize::from(gutter);
        padded
            .pixels
            .copy_within(src..src + usize::from(width), y * usize::from(width));
    }
    padded.pixels.truncate(usize::from(width) * usize::from(height));
    // Retain the original padded-frame guard and allocation capacity through output lifetime.
    Some(padded)
}

#[test]
fn padded_tile_refusal_and_cancellation_release_all_credit() {
    use std::cell::Cell;
    use std::sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    };
    struct Credit(Arc<AtomicUsize>, usize);
    impl Drop for Credit {
        fn drop(&mut self) {
            self.0.fetch_sub(self.1, Ordering::Relaxed);
        }
    }
    let scene = record(authored_pdf(true, 0.5, false));
    let admissions = Cell::new(0);
    let checkpoints = Cell::new(0);
    let baseline = replay_precise_tile_with_gutter(
        &scene,
        7,
        5,
        Affine::IDENTITY,
        2,
        &|| {
            checkpoints.set(checkpoints.get() + 1);
            false
        },
        &|_| {
            admissions.set(admissions.get() + 1);
            Some(Box::new(()))
        },
    )
    .unwrap();
    assert_eq!(baseline.pixels.len(), 35);
    drop(baseline);
    for deny in 1..=admissions.get() {
        let calls = Cell::new(0);
        let used = Arc::new(AtomicUsize::new(0));
        let result =
            replay_precise_tile_with_gutter(&scene, 7, 5, Affine::IDENTITY, 2, &|| false, &|bytes| {
                calls.set(calls.get() + 1);
                if calls.get() == deny {
                    return None;
                }
                used.fetch_add(bytes, Ordering::Relaxed);
                Some(Box::new(Credit(used.clone(), bytes)))
            });
        assert!(result.is_none(), "refusal={deny}");
        assert_eq!(used.load(Ordering::Relaxed), 0);
    }
    for stop in 1..=checkpoints.get() {
        let calls = Cell::new(0);
        let used = Arc::new(AtomicUsize::new(0));
        let result = replay_precise_tile_with_gutter(
            &scene,
            7,
            5,
            Affine::IDENTITY,
            2,
            &|| {
                calls.set(calls.get() + 1);
                calls.get() >= stop
            },
            &|bytes| {
                used.fetch_add(bytes, Ordering::Relaxed);
                Some(Box::new(Credit(used.clone(), bytes)))
            },
        );
        assert!(result.is_none(), "checkpoint={stop}");
        assert_eq!(used.load(Ordering::Relaxed), 0);
    }
    assert!(
        replay_precise_tile_with_gutter(&scene, u16::MAX, 5, Affine::IDENTITY, 2, &|| false, &|_| panic!(
            "overflow admitted"
        ))
        .is_none()
    );
}

fn trace_prepared_leaf_shapes(scene: &Recorder, x: usize, y: usize, edge: usize, gutter: u16) {
    let ox = x / edge * edge;
    let oy = y / edge * edge;
    let tw = edge.min(64 - ox) + usize::from(gutter) * 2;
    let th = edge.min(48 - oy) + usize::from(gutter) * 2;
    let offset = Affine::translate((f64::from(gutter) - ox as f64, f64::from(gutter) - oy as f64));
    for (event, event_data) in scene.events.iter().enumerate() {
        let Event::Path(path, transform, _, clips, mode) = event_data else {
            continue;
        };
        let stroke = match mode {
            PathDrawMode::Stroke(props) => Some(kurbo::Stroke {
                width: f64::from(props.line_width),
                join: props.line_join,
                miter_limit: f64::from(props.miter_limit),
                start_cap: props.line_cap,
                end_cap: props.line_cap,
                dash_pattern: props.dash_array.iter().map(|&v| f64::from(v)).collect(),
                dash_offset: f64::from(props.dash_offset),
            }),
            _ => None,
        };
        for clipped in [false, true] {
            let render = |width, height, offset| {
                let draw = match mode {
                    PathDrawMode::Fill(hayro_interpret::FillRule::NonZero) => {
                        ShapeDrawMode::Fill(vello_cpu::peniko::Fill::NonZero)
                    }
                    PathDrawMode::Fill(hayro_interpret::FillRule::EvenOdd) => {
                        ShapeDrawMode::Fill(vello_cpu::peniko::Fill::EvenOdd)
                    }
                    PathDrawMode::Stroke(_) => ShapeDrawMode::Stroke(stroke.as_ref().unwrap()),
                };
                rasterize_path_shape_with_transformed_clips(
                    path,
                    offset * *transform,
                    clips.iter().filter(|_| clipped).map(|clip| {
                        (
                            clip.path.as_ref(),
                            match clip.fill {
                                hayro_interpret::FillRule::NonZero => vello_cpu::peniko::Fill::NonZero,
                                hayro_interpret::FillRule::EvenOdd => vello_cpu::peniko::Fill::EvenOdd,
                            },
                        )
                    }),
                    offset,
                    draw,
                    width,
                    height,
                    &|| false,
                    &|_| Some(Box::new(())),
                )
                .unwrap()
            };
            let whole = render(64, 48, Affine::IDENTITY);
            let tile = render(tw as u16, th as u16, offset);
            let a = whole.values[y * 64 + x];
            let b = tile.values[(y - oy + usize::from(gutter)) * tw + x - ox + usize::from(gutter)];
            eprintln!(
                "LEAF_SHAPE {{\"event\":{event},\"x\":{x},\"y\":{y},\"clipped\":{clipped},\"whole\":{a},\"tile\":{b}}}"
            );
        }
    }
}

#[test]
#[ignore = "explicit independently authored half-area stroke raster diagnostic; fresh report path required"]
fn half_area_dash_outline_raster_diagnostic() {
    use kurbo::Shape;
    let output = std::path::PathBuf::from(std::env::var_os("RRRAH_HALF_AREA_REPORT").unwrap());
    assert!(!output.exists());
    let mut line = BezPath::new();
    line.move_to((0.0, 8.0));
    line.line_to((32.0, 8.0));
    let stroke = kurbo::Stroke {
        width: 3.0,
        start_cap: kurbo::Cap::Round,
        end_cap: kurbo::Cap::Round,
        dash_pattern: [4.0, 3.0].into_iter().collect(),
        dash_offset: 1.0,
        ..kurbo::Stroke::default()
    };
    let outline = kurbo::stroke(
        line.elements().iter().copied(),
        &stroke,
        &kurbo::StrokeOpts::default(),
        0.0001,
    );
    let mut recorder = Recorder {
        canonical_f32: true,
        ..Recorder::default()
    };
    let prepared = recorder.prepared_contour(&outline, Affine::IDENTITY).unwrap();
    // Dash offset1 leaves first length3, then gap3, then dash x=6..10.
    // Pixel [7,8]x[9,10] is entirely away from caps; y=9..9.5 covers area1/2.
    // Exact alpha quantization: round(255/2)=128, independently of the backend.
    let rectangle = kurbo::Rect::new(6.0, 6.5, 10.0, 9.5).to_path(0.1);
    let mut rows = Vec::new();
    for (kind, path) in [("rectangle", &rectangle), ("dash_outline", &prepared)] {
        for (viewport, w, h, offset, index) in [
            ("whole", 64, 48, Affine::IDENTITY, 9 * 64 + 7),
            (
                "translated_tile_with_gutter",
                11,
                11,
                Affine::translate((-5.0, -5.0)),
                4 * 11 + 2,
            ),
        ] {
            let shape =
                rasterize_fill_shape(path, offset, None, w, h, &|| false, &|_| Some(Box::new(()))).unwrap();
            let actual = shape.values[index];
            rows.push(format!(
                "{{\"kind\":{kind:?},\"viewport\":{viewport:?},\"expected\":128,\"actual\":{actual}}}"
            ));
        }
    }
    use std::sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    };
    struct ScratchCredit(Arc<AtomicUsize>, usize);
    impl Drop for ScratchCredit {
        fn drop(&mut self) {
            self.0.fetch_sub(self.1, Ordering::Relaxed);
        }
    }
    let used = Arc::new(AtomicUsize::new(0));
    let peak = Arc::new(AtomicUsize::new(0));
    let first = std::cell::Cell::new(0);
    let polygon_area = polygon_coverage::polygon_pixel_area(
        &[(&prepared, vello_cpu::peniko::Fill::NonZero)],
        7,
        9,
        20_000_000,
        &|| false,
        &|bytes| {
            if first.get() == 0 {
                first.set(bytes);
            }
            let current = used.fetch_add(bytes, Ordering::Relaxed) + bytes;
            peak.fetch_max(current, Ordering::Relaxed);
            Some(Box::new(ScratchCredit(used.clone(), bytes)))
        },
    )
    .unwrap();
    assert_eq!(used.load(Ordering::Relaxed), 0);
    let active_edges = first.get() / size_of::<polygon_coverage::Edge>();
    let previous_worst_case_scratch_bytes = active_edges
        * (size_of::<polygon_coverage::Edge>() + size_of::<(f64, usize)>())
        + (2 + active_edges * 4 + active_edges * (active_edges - 1) / 2) * size_of::<f64>()
        + size_of::<i32>();
    let peak_scratch_bytes = peak.load(Ordering::Relaxed);
    let alpha = (polygon_area * 255.0).round() as u8;
    assert_eq!(polygon_area, 0.5);
    assert_eq!(alpha, 128);
    rows.push(format!("{{\"kind\":\"native_global_polygon\",\"viewport\":\"global_pixel\",\"expected\":128,\"actual\":{alpha},\"area\":{polygon_area}}}"));
    rows.push(format!("{{\"kind\":\"scratch_allocation\",\"active_edges\":{active_edges},\"peak_admitted_bytes\":{peak_scratch_bytes},\"previous_worst_case_scratch_bytes\":{previous_worst_case_scratch_bytes},\"final_admitted_bytes\":0}}"));
    std::fs::write(output, format!("[{}]\n", rows.join(","))).unwrap();
}

#[test]
fn admitted_stroke_capture_refusal_and_cancel_never_replay_partial_geometry() {
    use std::sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    };
    struct Credit(Arc<AtomicUsize>, usize);
    impl Drop for Credit {
        fn drop(&mut self) {
            self.0.fetch_sub(self.1, Ordering::Relaxed);
        }
    }
    let pdf = authored_pdf_with_blue(true, 0.5, "/form Do", "0 0 1 RG 3 w 1 J [4 3] 1 d 0 8 m 32 8 l S");
    let calls = Arc::new(AtomicUsize::new(0));
    let count = calls.clone();
    let polls = Arc::new(AtomicUsize::new(0));
    let polling = polls.clone();
    let baseline = record_into(
        pdf.clone(),
        32,
        16,
        Affine::IDENTITY,
        Recorder {
            global_contour: true,
            native_polygon: true,
            admitted_strokes: true,
            admission: Some(std::rc::Rc::new(move |_| {
                count.fetch_add(1, Ordering::Relaxed);
                Some(Box::new(()))
            })),
            cancelled: Some(Arc::new(move || {
                polling.fetch_add(1, Ordering::Relaxed);
                false
            })),
            ..Recorder::default()
        },
    );
    assert!(!baseline.failed);
    assert!(
        baseline
            .events
            .iter()
            .all(|event| !matches!(event, Event::Path(_, _, _, _, PathDrawMode::Stroke(_))))
    );
    assert!(
        replay_scene(&baseline, 32, 16, Affine::IDENTITY, &|| false, &|_| Some(
            Box::new(())
        ))
        .is_some()
    );
    drop(baseline);
    for deny in 1..=calls.load(Ordering::Relaxed) {
        let attempted = Arc::new(AtomicUsize::new(0));
        let counter = attempted.clone();
        let used = Arc::new(AtomicUsize::new(0));
        let balance = used.clone();
        let scene = record_into(
            pdf.clone(),
            32,
            16,
            Affine::IDENTITY,
            Recorder {
                global_contour: true,
                native_polygon: true,
                admitted_strokes: true,
                admission: Some(std::rc::Rc::new(move |bytes| {
                    if counter.fetch_add(1, Ordering::Relaxed) + 1 == deny {
                        return None;
                    }
                    balance.fetch_add(bytes, Ordering::Relaxed);
                    Some(Box::new(Credit(balance.clone(), bytes)))
                })),
                ..Recorder::default()
            },
        );
        assert!(scene.failed, "capture admission={deny}");
        assert!(
            replay_scene(&scene, 32, 16, Affine::IDENTITY, &|| false, &|_| panic!(
                "partial stroke admitted"
            ))
            .is_none()
        );
        drop(scene);
        assert_eq!(used.load(Ordering::Relaxed), 0);
    }
    for stop in 1..=polls.load(Ordering::Relaxed) {
        let attempted = Arc::new(AtomicUsize::new(0));
        let counter = attempted.clone();
        let used = Arc::new(AtomicUsize::new(0));
        let balance = used.clone();
        let scene = record_into(
            pdf.clone(),
            32,
            16,
            Affine::IDENTITY,
            Recorder {
                global_contour: true,
                native_polygon: true,
                admitted_strokes: true,
                cancelled: Some(Arc::new(move || {
                    counter.fetch_add(1, Ordering::Relaxed) + 1 >= stop
                })),
                admission: Some(std::rc::Rc::new(move |bytes| {
                    balance.fetch_add(bytes, Ordering::Relaxed);
                    Some(Box::new(Credit(balance.clone(), bytes)))
                })),
                ..Recorder::default()
            },
        );
        assert!(scene.failed, "capture cancel={stop}");
        assert!(
            replay_scene(&scene, 32, 16, Affine::IDENTITY, &|| false, &|_| panic!(
                "cancelled stroke admitted"
            ))
            .is_none()
        );
        drop(scene);
        assert_eq!(used.load(Ordering::Relaxed), 0);
    }
}

#[test]
fn empty_native_path_preserves_zero_pixels_and_rejects_unsupported_blend() {
    use kurbo::Shape;
    for mode in [BlendMode::Normal, BlendMode::Difference] {
        let mut scene = Recorder {
            width: 8,
            height: 8,
            native_polygon: true,
            ..Recorder::default()
        };
        scene.events.push(Event::Blend(mode));
        scene.events.push(Event::Path(
            kurbo::Rect::new(100., 100., 101., 101.).to_path(0.1),
            Affine::IDENTITY,
            RecordedPaint::Color([255, 0, 0, 255]),
            Vec::new(),
            PathDrawMode::Fill(hayro_interpret::FillRule::NonZero),
        ));
        let result = replay_scene_with_backdrop(
            &scene, 8, 8, Affine::IDENTITY, [0; 4], &|| false, &|_| Some(Box::new(())),
        );
        if mode == BlendMode::Normal {
            assert!(result.unwrap().pixels.iter().all(|pixel| *pixel == [0; 4]));
        } else {
            assert!(result.is_none());
        }
    }
}

#[test]
fn group_spaces_preserve_page_form_resources_and_unresolved_declarations() {
    use hayro_interpret::TransparencyGroupColorSpace as Space;
    for (page_cs, form_cs, page_channels, form_channels) in [
        (Some("/DeviceCMYK"), None, Some(4), Some(0)),
        (None, Some("/DeviceGray"), Some(0), Some(1)),
        (Some("/DeviceRGB"), Some("/Alias"), Some(3), Some(1)),
        (None, Some("/Missing"), Some(0), None),
        (None, Some("null"), Some(0), None),
        (None, Some("[/Indexed /DeviceRGB 1 <000000ffffff>]"), Some(0), None),
    ] {
        let group = |cs: Option<&str>| cs.map_or(String::new(), |cs| format!(" /CS {cs}"));
        let content = if page_cs == Some("/DeviceCMYK") {
            "0.17 0.31 0.53 0.07 k 0 0 32 16 re f"
        } else {
            "0 0 1 rg 0 0 32 16 re f"
        };
        let objects = [
            "<< /Type /Catalog /Pages 2 0 R >>".to_owned(),
            "<< /Type /Pages /Kids [3 0 R] /Count 1 >>".to_owned(),
            format!("<< /Type /Page /Parent 2 0 R /MediaBox [0 0 32 16] /Group << /S /Transparency{} >> /Resources << /ColorSpace << /Alias /DeviceCMYK >> /XObject << /G 6 0 R >> >> /Contents 4 0 R >>", group(page_cs)),
            "<< /Length 5 >>\nstream\n/G Do\nendstream".to_owned(),
            "<< >>".to_owned(),
            format!("<< /Type /XObject /Subtype /Form /BBox [0 0 32 16] /Group << /S /Transparency /I true{} >> /Resources << /ColorSpace << /Alias /DeviceGray >> >> /Length {} >>\nstream\n{content}\nendstream", group(form_cs), content.len()),
        ];
        let mut pdf = b"%PDF-1.4\n".to_vec();
        let mut offsets = vec![0];
        for (i, object) in objects.iter().enumerate() {
            offsets.push(pdf.len());
            pdf.extend_from_slice(format!("{} 0 obj\n{object}\nendobj\n", i + 1).as_bytes());
        }
        let xref = pdf.len();
        pdf.extend_from_slice(b"xref\n0 7\n0000000000 65535 f \n");
        for offset in &offsets[1..] {
            pdf.extend_from_slice(format!("{offset:010} 00000 n \n").as_bytes());
        }
        pdf.extend_from_slice(format!("trailer\n<< /Size 7 /Root 1 0 R >>\nstartxref\n{xref}\n%%EOF\n").as_bytes());
        let scene = record(pdf);
        assert!(!scene.failed);
        let channels = |space: &Space| match space {
            Space::Inherited => Some(0),
            Space::Explicit(cs) => Some(cs.num_components()),
            Space::Unresolved => None,
        };
        assert_eq!(channels(&scene.page_group_space), page_channels);
        assert_eq!(scene.group_spaces.len(), 1);
        assert!(matches!(scene.events[scene.group_spaces[0].0], Event::Push(_, _, _)));
        assert_eq!(channels(&scene.group_spaces[0].1), form_channels);
        let paint = scene.events.iter().find_map(|event| match event {
            Event::Path(_, _, RecordedPaint::NativeColor(paint), _, _) => Some(paint),
            _ => None,
        }).unwrap();
        assert_eq!(paint.opacity, 1.);
        if page_cs == Some("/DeviceCMYK") {
            assert_eq!(paint.components, [0.17, 0.31, 0.53, 0.07]);
            assert_eq!(paint.color_space.num_components(), 4);
            let hayro_interpret::TransparencyGroupColorSpace::Explicit(target) = &scene.page_group_space else { panic!("missing page space") };
            let clear = super::native_blend::Pixel::from_native(super::native_blend::Model::Cmyk, &[0.;4], 0.).unwrap();
            let mut frame = super::native_frame::PaintFrame::admitted(&[clear], target, false, true,
                &|| false, &|_| Some(Box::new(()))).unwrap();
            frame.apply_paint(0, paint, 0.5, BlendMode::Normal, &|| false).unwrap();
            let output = frame.contribution(&|| false).unwrap();
            assert_eq!(output.pixels[0].alpha(), 0.5);
            for (actual, &expected) in output.pixels[0].native_components().unwrap().iter().zip(&paint.components) {
                assert!((*actual-f64::from(expected)).abs() < 1e-15);
            }
            let rgb = hayro_interpret::color::Color::from_rgba(
                hayro_interpret::color::AlphaColor::new([0., 0., 0., 1.]),
            );
            let pixel = paint.pixel_in_space(rgb.color_space(), 0.5).unwrap();
            assert_eq!(pixel.alpha(), 0.5);
            let components = pixel.native_components().unwrap();
            for i in 0..3 {
                assert_eq!((components[i] * 255.0).round() as u8, paint.display[i]);
            }
        } else {
            // Original RGB paint survives inside a Gray group. Conversion
            // to the group space must occur explicitly in the renderer.
            assert_eq!(paint.components, [0., 0., 1.]);
            assert_eq!(paint.color_space.num_components(), 3);
        }
    }
}

// Test-only native solid event replay with alpha/device luminosity masks.
// Unsupported color boundaries, calibrated luminosity, rasters and shading
// refuse; their native routing remains required before production.
fn replay_native_solid_scene(
    scene: &Recorder, width: u16, height: u16,
    space: &hayro_interpret::color::ColorSpace, backdrop: super::native_blend::Pixel,
    cancelled: &dyn Fn() -> bool,
    admit: &dyn Fn(usize) -> Option<Box<dyn std::any::Any + Send + Sync>>,
) -> Option<super::native_frame::Buffer> {
    replay_native_solid_scene_depth(scene,width,height,space,backdrop,cancelled,admit,0)
}

fn replay_native_solid_scene_depth(
    scene: &Recorder, width: u16, height: u16,
    space: &hayro_interpret::color::ColorSpace, backdrop: super::native_blend::Pixel,
    cancelled: &dyn Fn() -> bool,
    admit: &dyn Fn(usize) -> Option<Box<dyn std::any::Any + Send + Sync>>,
    depth: usize,
) -> Option<super::native_frame::Buffer> {
    use hayro_interpret::TransparencyGroupColorSpace as GroupSpace;
    use super::native_frame::PaintFrame;
    if scene.failed || !scene.native_polygon || cancelled() || depth>=32 { return None; }
    match &scene.page_group_space {
        GroupSpace::Inherited => {},
        GroupSpace::Explicit(page) if page.shares_blending_coordinates(space) => {},
        _ => return None,
    }
    let count = usize::from(width).checked_mul(usize::from(height))?;
    if count == 0 || count > 16_777_216 || scene.events.len() > 1_000_000 { return None; }
    struct NativeMask { values: Vec<f64>, _credit: Box<dyn std::any::Any+Send+Sync> }
    type Entry = (Box<PaintFrame>, f64, bool, BlendMode, Option<NativeMask>);
    let mut capacity = 1usize;
    for event in &scene.events {
        if cancelled() { return None; }
        if matches!(event,Event::Push(_,_,_)) { capacity = capacity.checked_add(1)?; }
    }
    let _stack_credit = admit(capacity.checked_mul(size_of::<Entry>())?)?;
    let mut stack = Vec::<Entry>::new();
    stack.try_reserve_exact(capacity).ok()?;
    let _backdrop_credit = admit(count.checked_mul(size_of::<super::native_blend::Pixel>())?)?;
    let mut pixels = Vec::new();
    pixels.try_reserve_exact(count).ok()?;
    for _ in 0..count { if cancelled() { return None; } pixels.push(backdrop); }
    let root = PaintFrame::admitted(&pixels,space,false,false,cancelled,admit)?;
    drop(pixels);
    drop(_backdrop_credit);
    stack.push((root,1.0,false,BlendMode::Normal,None));
    let mut active_mask: Option<NativeMask> = None;
    let mut mask_cursor = 0;
    let mut blend = BlendMode::Normal;
    let mut ais = false;
    let mut group_cursor = 0;
    let mut raster_cursor = 0;
    for (event_index,event) in scene.events.iter().enumerate() {
        if cancelled() { return None; }
        if std::env::var_os("RRRAH_TRACE_NATIVE_REPLAY_EVENT").is_some() {
            let kind = match event {
                Event::Blend(_) => "blend", Event::AlphaSource(_) => "alpha-source", Event::Mask(_) => "mask",
                Event::Push(..) => "push", Event::Pop => "pop", Event::Raster(..) => "raster",
                Event::Path(_,_,RecordedPaint::Shading(_),_,_) => "shading",
                Event::Path(_,_,RecordedPaint::NativeColor(_),_,_) => "native-paint",
                _ => "unsupported-paint",
            };
            eprintln!("NATIVE_EVENT depth={depth} index={event_index} kind={kind}");
        }
        match event {
            Event::Blend(mode) => {
                if !matches!(mode,BlendMode::Normal | BlendMode::Multiply | BlendMode::Screen | BlendMode::HardLight) { return None; }
                blend = *mode;
            },
            Event::AlphaSource(value) => ais = *value,
            Event::Mask(None) => active_mask = None,
            Event::Mask(Some((mask_scene,luminosity,_,_,_))) => {
                let (index,transfer,native_backdrop,device_luminosity) = scene.native_mask_transfers.get(mask_cursor)?;
                if *luminosity && !device_luminosity { return None; }
                if *index != event_index { return None; }
                mask_cursor += 1;
                let mask_space = match &mask_scene.page_group_space {
                    GroupSpace::Explicit(target) => target,
                    GroupSpace::Inherited => mask_scene.group_spaces.iter().find_map(|(_,state)| match state {
                        GroupSpace::Explicit(target) => Some(target), _ => None,
                    }).unwrap_or(space),
                    _ => return None,
                };
                use hayro_interpret::color::BlendingModel;
                use super::native_blend::{Model,Pixel};
                let model = match mask_space.blending_model()? {
                    BlendingModel::Gray => Model::Gray, BlendingModel::Rgb => Model::Rgb, BlendingModel::Cmyk => Model::Cmyk,
                };
                let background = if *luminosity { native_backdrop.pixel_in_space(mask_space,1.0)? }
                    else { Pixel::from_native(model,&[0.;4][..model.channels()],0.0)? };
                let pixels = replay_native_solid_scene_depth(mask_scene,width,height,mask_space,background,cancelled,admit,depth+1)?;
                let credit = admit(count.checked_mul(size_of::<f64>())?)?;
                let mut values = Vec::new();
                values.try_reserve_exact(count).ok()?;
                for pixel in &pixels.pixels {
                    if cancelled() { return None; }
                    let sample = if *luminosity {
                        // Device-space approximation; calibrated PCS luminosity remains separate.
                        let components = pixel.native_components()?;
                        let gray = match model {
                            Model::Gray => components[0],
                            Model::Rgb => 0.30*components[0]+0.59*components[1]+0.11*components[2],
                            Model::Cmyk => 1.0-(0.30*components[0]+0.59*components[1]+0.11*components[2]+components[3]).min(1.0),
                        };
                        gray*pixel.alpha()
                    } else { pixel.alpha() };
                    let value = if let Some(transfer) = transfer {
                        f64::from(transfer.apply_bounded(sample as f32,cancelled)?)
                    } else { sample };
                    if !value.is_finite() || !(0.0..=1.0).contains(&value) { return None; }
                    values.push(value);
                }
                active_mask = Some(NativeMask {values,_credit:credit});
            },
            Event::Push(opacity,properties,mode) => {
                if !opacity.is_finite() || !(0.0..=1.0).contains(opacity) { return None; }
                let mut declared = None;
                if let Some((index,declaration)) = scene.group_spaces.get(group_cursor) {
                    if *index < event_index { return None; }
                    if *index == event_index {
                        match declaration {
                            _ if !properties.isolated => {},
                            GroupSpace::Inherited => {},
                            GroupSpace::Explicit(target) => declared = Some(target),
                            _ => return None,
                        }
                        group_cursor += 1;
                    }
                }
                let parent = &mut stack.last_mut()?.0;
                let child = if let Some(target) = declared {
                    parent.isolated_child_in_space(target,properties.knockout,cancelled,admit)?
                } else { parent.child(properties.isolated,properties.knockout,cancelled,admit)? };
                stack.push((child,f64::from(*opacity),ais,*mode,active_mask.take()));
            }
            Event::Pop => {
                if stack.len() <= 1 { return None; }
                let (child,opacity,alpha_is_shape,mode,mask) = stack.pop()?;
                stack.last_mut()?.0.apply_group_masked(child,opacity,alpha_is_shape,mask.as_ref().map(|mask| mask.values.as_slice()),mode,cancelled)?;
            }
            Event::Raster(compatibility,_,transform,clips,stencil) => {
                if stencil.is_some() {return None;}
                let (index,data,reason)=scene.native_rasters.get(raster_cursor)?;
                if *index!=event_index || reason.is_some() {return None;}
                raster_cursor+=1;
                let data=data.as_ref()?;
                let (image_width,image_height)=data.dimensions();
                if image_width!=compatibility.width() || image_height!=compatibility.height()
                    || !transform.determinant().is_finite() || transform.determinant().abs()<1e-12 {return None;}
                let inverse=transform.inverse();
                let _path_credit=admit(size_of::<BezPath>().checked_add(5*size_of::<kurbo::PathEl>())?)?;
                let mut elements=Vec::new();elements.try_reserve_exact(5).ok()?;
                let points=[kurbo::Point::new(0.,0.),kurbo::Point::new(f64::from(image_width),0.),kurbo::Point::new(f64::from(image_width),f64::from(image_height)),kurbo::Point::new(0.,f64::from(image_height))];
                for (i,point) in points.into_iter().enumerate() {
                    if cancelled() {return None;}
                    let point=*transform*point;
                    if !point.x.is_finite() || !point.y.is_finite() {return None;}
                    elements.push(if i==0 {kurbo::PathEl::MoveTo(point)}else{kurbo::PathEl::LineTo(point)});
                }
                elements.push(kurbo::PathEl::ClosePath);
                let path=BezPath::from_vec(elements);
                let fill=|rule|match rule {hayro_interpret::FillRule::NonZero=>vello_cpu::peniko::Fill::NonZero,hayro_interpret::FillRule::EvenOdd=>vello_cpu::peniko::Fill::EvenOdd};
                let shape=rasterize_global_polygon_shape(&path,vello_cpu::peniko::Fill::NonZero,clips.iter().map(|clip|(&clip.path,fill(clip.fill))),(0,0),width,height,cancelled,admit)?;
                for (index,&coverage) in shape.values.iter().enumerate() {
                    if cancelled() {return None;}
                    if coverage==0 {continue;}
                    let point=inverse*kurbo::Point::new((index%usize::from(width)) as f64+0.5,(index/usize::from(width)) as f64+0.5);
                    let sample=sample_native_raster(data,point,cancelled)?;
                    use hayro_interpret::color::BlendingModel;
                    use super::native_blend::{Model,Pixel};
                    let model=match sample.space.blending_model()? {BlendingModel::Gray=>Model::Gray,BlendingModel::Rgb=>Model::Rgb,BlendingModel::Cmyk=>Model::Cmyk};
                    let pixel=Pixel::from_native(model,&sample.components[..model.channels()],sample.alpha)?;
                    let frame=&mut stack.last_mut()?.0;
                    let prepared=frame.prepare_native_pixel(pixel,sample.space,cancelled)?;
                    frame.apply_prepared_masked(index,&prepared,f64::from(coverage)/255.,ais,active_mask.as_ref().map_or(1.,|mask|mask.values[index]),blend,cancelled)?;
                }
            },
            Event::Path(path,transform,RecordedPaint::Shading(paint),clips,PathDrawMode::Fill(rule)) => {
                if *transform != Affine::IDENTITY { return None; }
                if std::env::var_os("RRRAH_TRACE_NATIVE_REPLAY_EVENT").is_some() {
                    eprintln!("NATIVE_SHADING event={event_index} captured={} refusal={:?}",paint.native.is_some(),paint.refusal);
                }
                let source=paint.native.as_ref()?;
                let fill=|rule| match rule {
                    hayro_interpret::FillRule::NonZero=>vello_cpu::peniko::Fill::NonZero,
                    hayro_interpret::FillRule::EvenOdd=>vello_cpu::peniko::Fill::EvenOdd,
                };
                let shape=rasterize_global_polygon_shape(path,fill(*rule),clips.iter().map(|clip|
                    (clip.path.as_ref(),fill(clip.fill))),(0,0),width,height,cancelled,admit)?;
                use hayro_interpret::color::BlendingModel;
                use super::native_blend::{Model,Pixel};
                let model=match source.color_space.blending_model()? {
                    BlendingModel::Gray=>Model::Gray,BlendingModel::Rgb=>Model::Rgb,BlendingModel::Cmyk=>Model::Cmyk,
                };
                for (index,&coverage) in shape.values.iter().enumerate() {
                    if cancelled() {return None;}
                    if coverage==0 {continue;}
                    let point=kurbo::Point::new((index%width as usize) as f64+0.5,(index/width as usize) as f64+0.5);
                    let Some(sample)=source.sample(point,cancelled).ok()? else {continue;};
                    if sample.count!=model.channels() {return None;}
                    let components=sample.components.map(f64::from);
                    let pixel=Pixel::from_native(model,&components[..sample.count],f64::from(sample.opacity))?;
                    let frame=&mut stack.last_mut()?.0;
                    let prepared=frame.prepare_native_pixel(pixel,&source.color_space,cancelled)?;
                    frame.apply_prepared_masked(index,&prepared,f64::from(coverage)/255.,ais,
                        active_mask.as_ref().map_or(1.,|mask|mask.values[index]),blend,cancelled)?;
                }
            },
            Event::Path(path,transform,RecordedPaint::NativeColor(paint),clips,PathDrawMode::Fill(rule)) => {
                if *transform != Affine::IDENTITY { return None; }
                let fill = |rule| match rule {
                    hayro_interpret::FillRule::NonZero => vello_cpu::peniko::Fill::NonZero,
                    hayro_interpret::FillRule::EvenOdd => vello_cpu::peniko::Fill::EvenOdd,
                };
                let shape = rasterize_global_polygon_shape(path,fill(*rule),clips.iter().map(|clip|
                    (clip.path.as_ref(),fill(clip.fill))), (0,0),width,height,cancelled,admit)?;
                let prepared = stack.last_mut()?.0.prepare_paint(paint,cancelled)?;
                for (index,&coverage) in shape.values.iter().enumerate() {
                    if cancelled() { return None; }
                    stack.last_mut()?.0.apply_prepared_masked(index,&prepared,f64::from(coverage)/255.0,ais,active_mask.as_ref().map_or(1.0,|mask| mask.values[index]),blend,cancelled)?;
                }
            }
            _ => return None,
        }
    }
    if stack.len() != 1 || group_cursor != scene.group_spaces.len() || mask_cursor != scene.native_mask_transfers.len() || raster_cursor != scene.native_rasters.len() { return None; }
    stack.pop()?.0.finish(cancelled)
}

#[test]
fn native_solid_event_replay_retains_original_color_and_nested_group_math() {
    use hayro_interpret::color::{AlphaColor,Color};
    use super::native_blend::{Pixel,Model};
    let rgb = Color::from_rgba(AlphaColor::new([0.,0.,0.,1.]));
    let clear = Pixel::from_native(Model::Rgb,&[0.;3],0.).unwrap();
    for knockout in [false,true] {
        let scene = record_into(authored_pdf(knockout,0.5,false),32,16,Affine::IDENTITY,
            Recorder { global_contour:true,native_polygon:true,admitted_strokes:true,..Recorder::default() });
        assert!(!scene.failed);
        let output = replay_native_solid_scene(&scene,32,16,rgb.color_space(),clear,&|| false,&|_| Some(Box::new(()))).unwrap();
        // Red and blue overlap at x16,y8; native paint alpha0.5 stays exact.
        let pixel = output.pixels[8*32+16];
        if knockout {
            let yellow = Pixel::from_native(Model::Rgb,&[1.,1.,0.],1.).unwrap();
            let colored = replay_native_solid_scene(&scene,32,16,rgb.color_space(),yellow,&|| false,&|_| Some(Box::new(()))).unwrap();
            let pixel = colored.pixels[8*32+16];
            assert_eq!(pixel.alpha(),1.0);
            assert_eq!(pixel.native_components().unwrap()[..3],[0.5,0.5,0.5]);
        }
        if knockout {
            assert_eq!(pixel.alpha(),0.5);
            assert_eq!(pixel.native_components().unwrap()[..3],[0.,0.,1.]);
        } else {
            assert_eq!(pixel.alpha(),1.0);
            assert_eq!(pixel.native_components().unwrap()[..3],[0.5,0.,0.5]);
        }
    }
}

#[test]
fn native_solid_scene_refusals_and_cancellations_release_all_replay_credit() {
    use hayro_interpret::color::{AlphaColor,Color};
    use super::native_blend::{Pixel,Model};
    use std::sync::{Arc, atomic::{AtomicUsize,Ordering}};
    use std::cell::Cell;
    struct Credit(Arc<AtomicUsize>,usize);
    impl Drop for Credit { fn drop(&mut self) { self.0.fetch_sub(self.1,Ordering::Relaxed); } }
    for document in [authored_pdf(false,0.5,false),authored_native_alpha_transfer_fixture(),authored_native_gray_child_fixture(),authored_native_gray_child_with_isolation(false),authored_native_soft_image_fixture(false),
        authored_native_device_luminosity_fixture("/DeviceGray","0.371234"),
        authored_native_device_luminosity_fixture("/DeviceRGB","0.173123 0.319876 0.537654"),
        authored_native_device_luminosity_fixture("/DeviceCMYK","0.173123 0.319876 0.537654 0.071234"),
    ] {
    let scene = record_into(document,2,1,Affine::scale(1.0/16.0),
        Recorder { capture_native_rasters:true,mask_probe:true,global_contour:true,native_polygon:true,admitted_strokes:true,..Recorder::default() });
    assert!(!scene.failed);
    let rgb = Color::from_rgba(AlphaColor::new([0.,0.,0.,1.]));
    let clear = Pixel::from_native(Model::Rgb,&[0.;3],0.).unwrap();
    let used = Arc::new(AtomicUsize::new(0));
    let admissions = Cell::new(0);
    let deny = Cell::new(0);
    let admit = |bytes| {
        admissions.set(admissions.get()+1);
        if deny.get()==admissions.get() { return None; }
        used.fetch_add(bytes,Ordering::Relaxed);
        Some(Box::new(Credit(used.clone(),bytes)) as Box<dyn std::any::Any+Send+Sync>)
    };
    let polls = Cell::new(0);
    let output = replay_native_solid_scene(&scene,2,1,rgb.color_space(),clear,&|| { polls.set(polls.get()+1); false },&admit).unwrap();
    assert_eq!(used.load(Ordering::Relaxed),2*size_of::<Pixel>());
    drop(output);
    assert_eq!(used.load(Ordering::Relaxed),0);
    let total_admissions = admissions.get();
    for stop in 1..=total_admissions {
        admissions.set(0); deny.set(stop);
        assert!(replay_native_solid_scene(&scene,2,1,rgb.color_space(),clear,&|| false,&admit).is_none());
        assert_eq!(used.load(Ordering::Relaxed),0);
    }
    deny.set(0);
    for stop in 1..=polls.get() {
        let current = Cell::new(0);
        assert!(replay_native_solid_scene(&scene,2,1,rgb.color_space(),clear,&|| { current.set(current.get()+1); current.get()==stop },&admit).is_none());
        assert_eq!(used.load(Ordering::Relaxed),0);
    }
    }
}

#[test]
#[ignore = "diagnostic timing; no performance assertion under shared CPU load"]
fn native_solid_paint_preparation_benchmark() {
    use hayro_interpret::color::{AlphaColor,Color};
    use super::{native_frame::PaintFrame,native_blend::{Pixel,Model}};
    let scene = record(authored_pdf_with_paths(false,1.0,"/form Do",
        "0.17 0.31 0.53 0.07 k 0 0 24 16 re f","0 0 1 rg 8 0 24 16 re f"));
    assert!(!scene.failed);
    let paint = scene.events.iter().find_map(|event| match event {
        Event::Path(_,_,RecordedPaint::NativeColor(paint),_,_) => Some(paint), _ => None,
    }).unwrap();
    assert_eq!(paint.color_space.num_components(),4);
    let rgb = Color::from_rgba(AlphaColor::new([0.,0.,0.,1.]));
    let clear = Pixel::from_native(Model::Rgb,&[0.;3],0.).unwrap();
    let admit = |_: usize| Some(Box::new(()) as Box<dyn std::any::Any+Send+Sync>);
    // Warm lazy color transform before timing.
    paint.pixel_in_space(rgb.color_space(),1.0).unwrap();
    for run in 0..3 {
        let mut repeated = PaintFrame::admitted(&[clear],rgb.color_space(),false,false,&|| false,&admit).unwrap();
        let start = std::time::Instant::now();
        for _ in 0..100_000 {
            repeated.apply_paint(0,std::hint::black_box(paint),0.75,BlendMode::Normal,&|| false).unwrap();
        }
        let repeated_ms = start.elapsed().as_secs_f64()*1000.0;
        let mut prepared = PaintFrame::admitted(&[clear],rgb.color_space(),false,false,&|| false,&admit).unwrap();
        let start = std::time::Instant::now();
        let sample = prepared.prepare_paint(paint,&|| false).unwrap();
        for _ in 0..100_000 {
            prepared.apply_prepared(0,std::hint::black_box(&sample),0.75,false,BlendMode::Normal,&|| false).unwrap();
        }
        let prepared_ms = start.elapsed().as_secs_f64()*1000.0;
        let a = repeated.finish(&|| false).unwrap();
        let b = prepared.finish(&|| false).unwrap();
        assert_eq!(a.pixels[0].alpha(),b.pixels[0].alpha());
        assert_eq!(a.pixels[0].native_components(),b.pixels[0].native_components());
        println!("PREPARED_PAINT_TIMING {{\"run\":{run},\"samples\":100000,\"repeated_ms\":{repeated_ms},\"prepared_ms\":{prepared_ms}}}");
    }
}

fn authored_native_alpha_transfer_fixture() -> Vec<u8> {
    authored_native_alpha_transfer_declaration("<< /FunctionType 2 /Domain [0 1] /C0 [0] /C1 [1] /N 2 >>")
}

fn authored_native_alpha_transfer_declaration(transfer: &str) -> Vec<u8> {
    let content = "/H gs 0 g 0 0 32 16 re f";
    let mask = format!("<< /Type /XObject /Subtype /Form /BBox [0 0 32 16] /Group << /S /Transparency /I true >> /Resources << /ExtGState << /H << /ca 0.5 >> >> >> /Length {} >>\nstream\n{content}\nendstream",content.len());
    authored_pdf_with_font_blend(false,1.0,"/form Do","1 0 0 rg 0 0 24 16 re f","0 0 1 rg 8 0 24 16 re f",
        "<< >>",Some(&mask),false,&format!("Normal /SMask << /S /Alpha /G 10 0 R /TR {transfer} >>"))
}

#[test]
fn native_alpha_mask_uses_original_transfer_without_lookup_quantization() {
    use hayro_interpret::color::{AlphaColor,Color};
    use super::native_blend::{Model,Pixel};
    let scene = record_into(authored_native_alpha_transfer_fixture(),32,16,Affine::IDENTITY,
        Recorder {mask_probe:true,global_contour:true,native_polygon:true,admitted_strokes:true,..Recorder::default()});
    assert!(!scene.failed);
    let rgb = Color::from_rgba(AlphaColor::new([0.,0.,0.,1.]));
    let clear = Pixel::from_native(Model::Rgb,&[0.;3],0.).unwrap();
    let output = replay_native_solid_scene(&scene,32,16,rgb.color_space(),clear,&|| false,&|_| Some(Box::new(()))).unwrap();
    let pixel = output.pixels[8*32+16];
    assert_eq!(pixel.alpha(),1.0);
    assert_eq!(pixel.native_components().unwrap()[..3],[0.75,0.,0.25]);
}

fn authored_native_device_luminosity_fixture(model: &str, background: &str) -> Vec<u8> {
    let black = if model=="/DeviceCMYK" {"0 0 0 1 k"} else {"0 g"};
    let content = format!("/H gs {black} 0 0 16 16 re f");
    let mask = format!("<< /Type /XObject /Subtype /Form /BBox [0 0 32 16] /Group << /S /Transparency /I true /CS {model} >> /Resources << /ExtGState << /H << /ca 0.5 >> >> >> /Length {} >>\nstream\n{content}\nendstream",content.len());
    let state = format!("Normal /SMask << /S /Luminosity /G 10 0 R /BC [{background}] >>");
    authored_pdf_with_font_blend(false,1.0,"/form Do","1 0 0 rg 0 0 24 16 re f","0 0 1 rg 8 0 24 16 re f",
        "<< >>",Some(&mask),false,&state)
}

#[test]
fn native_device_luminosity_masks_preserve_gray_rgb_cmyk_background_components() {
    use hayro_interpret::color::{AlphaColor,Color};
    use super::native_blend::{Model,Pixel};
    let rgb = Color::from_rgba(AlphaColor::new([0.,0.,0.,1.]));
    let clear = Pixel::from_native(Model::Rgb,&[0.;3],0.).unwrap();
    let rgb_lum = 0.3*f64::from(0.173123_f32)+0.59*f64::from(0.319876_f32)+0.11*f64::from(0.537654_f32);
    let cmyk_lum = 1.0-rgb_lum-f64::from(0.071234_f32);
    for (model,background,expected) in [
        ("/DeviceGray","0.371234",f64::from(0.371234_f32)),
        ("/DeviceRGB","0.173123 0.319876 0.537654",rgb_lum),
        ("/DeviceCMYK","0.173123 0.319876 0.537654 0.071234",cmyk_lum),
    ] {
        let scene = record_into(authored_native_device_luminosity_fixture(model,background),32,16,Affine::IDENTITY,
            Recorder {mask_probe:true,global_contour:true,native_polygon:true,admitted_strokes:true,..Recorder::default()});
        assert!(!scene.failed);
        let output = replay_native_solid_scene(&scene,32,16,rgb.color_space(),clear,&|| false,&|_| Some(Box::new(()))).unwrap();
        for (x,mask) in [(12,expected*0.5),(20,expected)] {
            let pixel = output.pixels[8*32+x];
            assert_eq!(pixel.alpha(),1.0);
            for (actual,expected) in pixel.native_components().unwrap()[..3].iter().zip([1.0-mask,0.,mask]) {
                assert!((*actual-expected).abs()<1e-14,"{model} x{x} actual{actual} expected{expected}");
            }
        }
    }
}

#[test]
fn native_invalid_mask_transfer_refuses_instead_of_using_identity_and_releases_credit() {
    use hayro_interpret::color::{Color,AlphaColor};
    use super::native_blend::{Pixel,Model};
    use std::sync::{Arc,atomic::{AtomicUsize,Ordering}};
    struct Credit(Arc<AtomicUsize>,usize);
    impl Drop for Credit { fn drop(&mut self) { self.0.fetch_sub(self.1,Ordering::Relaxed); } }
    let used = Arc::new(AtomicUsize::new(0));
    let admit = |bytes| {
        used.fetch_add(bytes,Ordering::Relaxed);
        Some(Box::new(Credit(used.clone(),bytes)) as Box<dyn std::any::Any+Send+Sync>)
    };
    let rgb = Color::from_rgba(AlphaColor::new([0.,0.,0.,1.]));
    let clear = Pixel::from_native(Model::Rgb,&[0.;3],0.).unwrap();
    for declaration in ["/Bogus","/Default","42","null","<< /FunctionType 2 /N 1 >>",
        "<< /FunctionType 2 /Domain [0 1] /C0 [0 0] /C1 [1 1] /N 1 >>"] {
        let scene = record_into(authored_native_alpha_transfer_declaration(declaration),32,16,Affine::IDENTITY,
            Recorder {mask_probe:true,global_contour:true,native_polygon:true,admitted_strokes:true,..Recorder::default()});
        assert!(!scene.failed,"capture {declaration}");
        assert!(scene.native_mask_transfers.iter().all(|(_,transfer,_,_)| transfer.is_some()));
        assert!(replay_native_solid_scene(&scene,32,16,rgb.color_space(),clear,&|| false,&admit).is_none(),"{declaration}");
        assert_eq!(used.load(Ordering::Relaxed),0,"{declaration}");
    }
    let scene = record_into(authored_native_alpha_transfer_declaration("/Identity"),32,16,Affine::IDENTITY,
        Recorder {mask_probe:true,global_contour:true,native_polygon:true,admitted_strokes:true,..Recorder::default()});
    assert!(!scene.failed);
    assert!(scene.native_mask_transfers.iter().all(|(_,transfer,_,_)| transfer.is_none()));
    let output = replay_native_solid_scene(&scene,32,16,rgb.color_space(),clear,&|| false,&admit).unwrap();
    assert_eq!(output.pixels[8*32+16].native_components().unwrap()[..3],[0.5,0.,0.5]);
    drop(output);
    assert_eq!(used.load(Ordering::Relaxed),0);
}

#[test]
fn isolated_gray_group_converts_to_rgb_without_byte_quantization() {
    use hayro_interpret::color::{AlphaColor, Color};
    use super::native_blend::{Model, Pixel};
    use super::native_frame::PaintFrame;
    let scene = record_into(authored_native_device_luminosity_fixture("/DeviceGray", "0.25"),32,16,Affine::IDENTITY,
        Recorder { mask_probe:true,global_contour:true,native_polygon:true,admitted_strokes:true,..Recorder::default() });
    let (_,_,background,_) = &scene.native_mask_transfers[0];
    let gray = &background.color_space;
    let rgb = Color::from_rgba(AlphaColor::new([1.,0.,0.,1.]));
    let red = Pixel::from_native(Model::Rgb,&[1.,0.,0.],1.).unwrap();
    use std::sync::{Arc, atomic::{AtomicUsize, Ordering}};
    struct Guard(Arc<AtomicUsize>, usize);
    impl Drop for Guard { fn drop(&mut self) { self.0.fetch_sub(self.1,Ordering::Relaxed); } }
    let used = Arc::new(AtomicUsize::new(0));
    let admit = |bytes| {
        used.fetch_add(bytes,Ordering::Relaxed);
        Some(Box::new(Guard(used.clone(),bytes)) as Box<dyn std::any::Any + Send + Sync>)
    };
    let mut parent = PaintFrame::admitted(&[red],rgb.color_space(),false,false,&|| false,&admit).unwrap();
    let mut child = parent.isolated_child_in_space(gray,false,&|| false,&admit).unwrap();
    let prepared = child.prepare_paint(background,&|| false).unwrap();
    child.apply_prepared(0,&prepared,0.5,false,BlendMode::Normal,&|| false).unwrap();
    parent.apply_group(child,1.,false,BlendMode::Normal,&|| false).unwrap();
    let pixel = parent.finish(&|| false).unwrap().pixels[0];
    assert_eq!(pixel.alpha(),1.);
    assert_eq!(&pixel.native_components().unwrap()[..3],&[0.625,0.125,0.125]);
    for stop in 1..=5 {
        let mut parent = PaintFrame::admitted(&[red],rgb.color_space(),false,false,&|| false,&admit).unwrap();
        let calls = std::cell::Cell::new(0);
        let denied = |bytes| { calls.set(calls.get()+1); if calls.get()==stop { None } else { admit(bytes) } };
        assert!(parent.isolated_child_in_space(gray,false,&|| false,&denied).is_none());
        assert!(parent.finish(&|| false).is_none());
        assert_eq!(used.load(Ordering::Relaxed),0);
    }
    let mut parent = PaintFrame::admitted(&[red],rgb.color_space(),false,false,&|| false,&admit).unwrap();
    assert!(parent.isolated_child_in_space(gray,false,&|| true,&admit).is_none());
    assert!(parent.finish(&|| false).is_none());
    assert_eq!(used.load(Ordering::Relaxed),0);
}

fn authored_native_gray_child_fixture() -> Vec<u8> { authored_native_gray_child_with_isolation(true) }
fn authored_native_gray_child_with_isolation(isolated: bool) -> Vec<u8> {
    authored_pdf_with_font_blend_and_blue_space(false,0.5,"/form Do",
        "1 0 0 rg 0 0 24 16 re f","0.25 g 8 0 24 16 re f","<< >>",None,false,"Normal",Some(("/DeviceGray",isolated)))
}

#[test]
fn actual_pdf_isolated_gray_child_routes_native_space_and_preserves_opacity() {
    use hayro_interpret::TransparencyGroupColorSpace as GroupSpace;
    use hayro_interpret::color::{AlphaColor, Color, BlendingModel};
    use super::native_blend::{Model, Pixel};
    for isolated in [true,false] {
    let scene = record_into(authored_native_gray_child_with_isolation(isolated),32,16,Affine::IDENTITY,
        Recorder { global_contour:true,native_polygon:true,admitted_strokes:true,..Recorder::default() });
    assert!(!scene.failed);
    assert!(scene.group_spaces.iter().any(|(_,space)| matches!(space,
        GroupSpace::Explicit(target) if target.blending_model()==Some(BlendingModel::Gray))));
    let rgb = Color::from_rgba(AlphaColor::new([0.,0.,0.,1.]));
    let clear = Pixel::from_native(Model::Rgb,&[0.;3],0.).unwrap();
    let output = replay_native_solid_scene(&scene,32,16,rgb.color_space(),clear,&|| false,&|_| Some(Box::new(()))).unwrap();
    let overlap = output.pixels[8*32+16];
    assert_eq!(overlap.alpha(),1.);
    assert_eq!(&overlap.native_components().unwrap()[..3],&[0.625,0.125,0.125]);
    let gray_only = output.pixels[8*32+28];
    assert_eq!(gray_only.alpha(),0.5);
    assert_eq!(&gray_only.native_components().unwrap()[..3],&[0.25;3]);
    // Integer rectangle endpoint contributes no area to the pixel on its
    // right. Guard this independently of renderer-specific edge filtering.
    let before_edge = output.pixels[8*32+23];
    assert_eq!(before_edge.alpha(),1.);
    assert_eq!(&before_edge.native_components().unwrap()[..3],&[0.625,0.125,0.125]);
    let at_edge = output.pixels[8*32+24];
    assert_eq!(at_edge.alpha(),0.5);
    assert_eq!(&at_edge.native_components().unwrap()[..3],&[0.25;3]);
    }
}

#[test]
#[ignore = "exports authored native Gray-group PDF and RGB pixels for independent oracle"]
fn native_gray_group_external_oracle_export() {
    use hayro_interpret::color::{AlphaColor, Color};
    use super::native_blend::{Model, Pixel};
    let root = std::path::PathBuf::from(std::env::var_os("RRRAH_GRAY_GROUP_ORACLE_OUTPUT").expect("fresh output directory"));
    std::fs::create_dir(&root).unwrap();
    let pdf = authored_native_gray_child_fixture();
    let scene = record_into(pdf.clone(),32,16,Affine::IDENTITY,
        Recorder {global_contour:true,native_polygon:true,admitted_strokes:true,..Recorder::default()});
    let rgb = Color::from_rgba(AlphaColor::WHITE);
    let white = Pixel::from_native(Model::Rgb,&[1.;3],1.).unwrap();
    let output = replay_native_solid_scene(&scene,32,16,rgb.color_space(),white,&|| false,&|_| Some(Box::new(()))).unwrap();
    let mut bytes = Vec::new();
    for pixel in &output.pixels {
        assert_eq!(pixel.alpha(),1.);
        for value in &pixel.native_components().unwrap()[..3] { bytes.push((value*255.).round() as u8); }
    }
    std::fs::write(root.join("gray-group.pdf"),pdf).unwrap();
    std::fs::write(root.join("native.rgb"),bytes).unwrap();
}

#[test]
fn nonisolated_pdf_gray_declaration_inherits_rgb_for_multiply() {
    use hayro_interpret::color::{AlphaColor,Color};
    use super::native_blend::{Pixel,Model};
    let pdf = authored_pdf_with_font_blend_and_blue_space(false,0.5,"/form Do",
        "1 0 0 rg 0 0 24 16 re f","0.25 g 8 0 24 16 re f","<< >>",None,false,"Multiply",Some(("/DeviceGray",false)));
    let scene = record_into(pdf,32,16,Affine::IDENTITY,
        Recorder {global_contour:true,native_polygon:true,admitted_strokes:true,..Recorder::default()});
    let rgb = Color::from_rgba(AlphaColor::WHITE);
    let white = Pixel::from_native(Model::Rgb,&[1.;3],1.).unwrap();
    let output = replay_native_solid_scene(&scene,32,16,rgb.color_space(),white,&|| false,&|_| Some(Box::new(()))).unwrap();
    let pixel = output.pixels[8*32+16];
    assert_eq!(pixel.alpha(),1.);
    assert_eq!(&pixel.native_components().unwrap()[..3],&[0.625,0.,0.]);
}

#[test]
fn raster_capture_retains_original_gray_rgb_cmyk_coordinates() {
    use hayro_interpret::color::BlendingModel;
    for (space,bytes,expected) in [("/G","40",BlendingModel::Gray),("/RGB","4080C0",BlendingModel::Rgb),("/CMYK","4080C020",BlendingModel::Cmyk)] {
        let content = format!("q 16 0 0 16 0 0 cm BI /W 1 /H 1 /BPC 8 /CS {space} /F /AHx ID {bytes}> EI Q");
        let scene = record(authored_pdf_with_paths(false,1.,"/form Do","",&content));
        assert!(!scene.failed);
        assert_eq!(scene.raster_spaces.len(),1);
        let (index,source)=&scene.raster_spaces[0];
        assert!(matches!(scene.events[*index],Event::Raster(..)));
        assert_eq!(source.as_ref().unwrap().blending_model(),Some(expected));
    }
}

#[test]
fn actual_pdf_stream_retains_native_16bit_components_and_decode() {
    use hayro_interpret::color::BlendingModel;
    for (space,bytes,decode,expected,model) in [
        ("/G","1234","[1 0]",vec![60875./65535.],BlendingModel::Gray),
        ("/RGB","123456789ABC","[0 1 0 1 0 1]",vec![4660./65535.,22136./65535.,39612./65535.],BlendingModel::Rgb),
        ("/CMYK","123456789ABCDEF0","[0 1 0 1 0 1 0 1]",vec![4660./65535.,22136./65535.,39612./65535.,57072./65535.],BlendingModel::Cmyk),
    ] {
        let content=format!("q 16 0 0 16 0 0 cm BI /W 1 /H 1 /BPC 16 /CS {space} /D {decode} /F /AHx ID {bytes}> EI Q");
        let scene=record_into(authored_pdf_with_paths(false,1.,"/form Do","",&content),32,16,Affine::IDENTITY,
            Recorder {capture_native_rasters:true,..Recorder::default()});
        assert!(!scene.failed);
        assert_eq!(scene.native_rasters.len(),1);
        let (index,data,refusal)=&scene.native_rasters[0];
        assert_eq!(*refusal,None);
        assert!(matches!(scene.events[*index],Event::Raster(..)));
        let data=data.as_ref().unwrap();
        assert_eq!(data.dimensions(),(1,1));
        assert_eq!(data.color_space().blending_model(),Some(model));
        assert_eq!(data.samples(),expected);
    }
}

#[test]
fn native_image_mask_refusal_preserves_reason_and_compatibility_alpha() {
    let content="q 16 0 0 16 0 0 cm BI /W 1 /H 1 /BPC 8 /CS /G /Mask [0 0] /F /AHx ID 00> EI Q";
    let scene=record_into(authored_pdf_with_paths(false,1.,"/form Do","",content),32,16,Affine::IDENTITY,
        Recorder {capture_native_rasters:true,..Recorder::default()});
    assert!(!scene.failed);
    assert_eq!(scene.native_rasters.len(),1);
    let (index,data,reason)=&scene.native_rasters[0];
    assert!(data.is_none());
    assert_eq!(*reason,Some(hayro_interpret::NativeRasterComponentsError::Mask(hayro_interpret::NativeRasterMaskKind::ColorKey)));
    let Event::Raster(_,Some(alpha),_,_,_)=&scene.events[*index] else { panic!("original alpha must survive compatibility capture") };
    assert_eq!(alpha.data,vec![0]);
}

#[test]
fn every_native_image_capture_cancellation_latches_and_releases_credit() {
    use std::sync::{Arc,atomic::{AtomicUsize,Ordering}};
    struct Guard(Arc<AtomicUsize>,usize);
    impl Drop for Guard { fn drop(&mut self) { self.0.fetch_sub(self.1,Ordering::Relaxed); } }
    let content="q 16 0 0 16 0 0 cm BI /W 1 /H 1 /BPC 16 /CS /RGB /F /AHx ID 123456789ABC> EI Q";
    let pdf=authored_pdf_with_paths(false,1.,"/form Do","",content);
    for pdf in [pdf,authored_native_soft_image_fixture(false)] {
    let run=|stop| {
        let used=Arc::new(AtomicUsize::new(0));
        let ledger=used.clone();
        let admission=std::rc::Rc::new(move |bytes| {
            ledger.fetch_add(bytes,Ordering::Relaxed);
            Some(Box::new(Guard(ledger.clone(),bytes)) as Box<dyn std::any::Any+Send+Sync>)
        });
        let polls=Arc::new(AtomicUsize::new(0));
        let count=polls.clone();
        let scene=record_into(pdf.clone(),32,16,Affine::IDENTITY,Recorder {
            capture_native_rasters:true,admission:Some(admission),
            cancelled:Some(Arc::new(move || count.fetch_add(1,Ordering::Relaxed)+1==stop)),
            ..Recorder::default()
        });
        if stop!=0 { assert!(scene.failed,"cancellation checkpoint {stop} did not latch"); }
        else { assert!(!scene.failed);assert!(scene.native_rasters[0].1.is_some()); }
        drop(scene);
        assert_eq!(used.load(Ordering::Relaxed),0);
        polls.load(Ordering::Relaxed)
    };
    for stop in 1..=run(0) { run(stop); }
    }
}

fn authored_native_soft_image_fixture(matte: bool) -> Vec<u8> {
    let mask_extra=if matte { "/Matte [1 1 1]" } else { "" };
    let bytes="8000>";
    let mask=format!("<< /Type /XObject /Subtype /Image /Width 1 /Height 1 /ColorSpace /DeviceGray /BitsPerComponent 16 /Filter /ASCIIHexDecode {mask_extra} /Length {} >>\nstream\n{bytes}\nendstream",bytes.len());
    let bytes="123456789ABC>";
    let image=format!("<< /Type /XObject /Subtype /Image /Width 1 /Height 1 /ColorSpace /DeviceRGB /BitsPerComponent 16 /SMask 10 0 R /Filter /ASCIIHexDecode /Length {} >>\nstream\n{bytes}\nendstream",bytes.len());
    authored_pdf_with_font_blend(false,1.,"/form Do","","q 16 0 0 16 0 0 cm /image Do Q",
        &image,Some(&mask),false,"Normal")
}

#[test]
fn native_soft_image_alpha_preserves_16bit_precision_and_refuses_matte() {
    for matte in [false,true] {
        let scene=record_into(authored_native_soft_image_fixture(matte),32,16,Affine::IDENTITY,
            Recorder {capture_native_rasters:true,..Recorder::default()});
        assert!(!scene.failed);
        assert_eq!(scene.native_rasters.len(),1);
        let (_,data,reason)=&scene.native_rasters[0];
        if matte {
            assert!(data.is_none());
            assert_eq!(*reason,Some(hayro_interpret::NativeRasterComponentsError::Mask(hayro_interpret::NativeRasterMaskKind::SoftMatte)));
        } else {
            assert_eq!(*reason,None);
            let data=data.as_ref().unwrap();
            assert_eq!(data.samples(),&[4660./65535.,22136./65535.,39612./65535.]);
            let alpha=data.alpha().unwrap();
            assert_eq!(alpha.dimensions(),(1,1));
            assert_eq!(alpha.samples(),&[32768./65535.]);
            let sampled=sample_native_raster(data,kurbo::Point::new(0.5,0.5),&||false).unwrap();
            assert_eq!(sampled.alpha,f64::from(alpha.samples()[0]));
            assert_eq!(&sampled.components[..3],&data.samples().iter().copied().map(f64::from).collect::<Vec<_>>());
        }
    }
}

struct NativeRasterSample<'a> {
    space: &'a hayro_interpret::color::ColorSpace,
    components: [f64;4],
    alpha: f64,
}
fn sample_native_raster<'a>(image: &'a hayro_interpret::NativeRasterComponents,point: kurbo::Point,cancelled: &dyn Fn()->bool) -> Option<NativeRasterSample<'a>> {
    if cancelled() || !point.x.is_finite() || !point.y.is_finite() { return None; }
    let (width,height)=image.dimensions();
    if width==0 || height==0 { return None; }
    let channels=usize::from(image.color_space().num_components());
    if image.samples().len()!=usize::try_from(width).ok()?.checked_mul(usize::try_from(height).ok()?)?.checked_mul(channels)? { return None; }
    let alpha_at=|x:f64,y:f64| -> Option<f64> {
        let Some(alpha)=image.alpha() else {return Some(1.)};
        let (w,h)=alpha.dimensions();if w==0 || h==0 {return None;}
        if alpha.samples().len()!=usize::try_from(w).ok()?.checked_mul(usize::try_from(h).ok()?)? {return None;}
        let x=x/f64::from(width)*f64::from(w);let y=y/f64::from(height)*f64::from(h);
        let at=|x:f64,y:f64| -> Option<f64> {
            if cancelled() {return None;}
            let x=x.clamp(0.,f64::from(w-1)) as usize;let y=y.clamp(0.,f64::from(h-1)) as usize;
            let value=f64::from(alpha.samples()[y*w as usize+x]);
            (value.is_finite() && (0. ..=1.).contains(&value)).then_some(value)
        };
        if !alpha.interpolate() { return at(x.floor(),y.floor()); }
        let x=x-0.5;let y=y-0.5;let tx=x-x.floor();let ty=y-y.floor();
        Some(at(x.floor(),y.floor())?*(1.-tx)*(1.-ty)+at(x.floor()+1.,y.floor())?*tx*(1.-ty)+at(x.floor(),y.floor()+1.)?*(1.-tx)*ty+at(x.floor()+1.,y.floor()+1.)?*tx*ty)
    };
    let at=|x:f64,y:f64,ax:f64,ay:f64| -> Option<NativeRasterSample<'a>> {
        if cancelled() {return None;}
        let x=x.clamp(0.,f64::from(width-1)) as usize;let y=y.clamp(0.,f64::from(height-1)) as usize;
        let index=(y*width as usize+x)*channels;
        let (space,components)=image.color_space().native_sample_coordinates(&image.samples()[index..index+channels])?;
        Some(NativeRasterSample {space,components:components.map(f64::from),alpha:alpha_at(ax,ay)?})
    };
    if !image.interpolate() { return at(point.x.floor(),point.y.floor(),point.x,point.y); }
    let x=point.x-0.5;let y=point.y-0.5;let tx=x-x.floor();let ty=y-y.floor();
    let positions=[(x.floor(),y.floor(),(1.-tx)*(1.-ty)),(x.floor()+1.,y.floor(),tx*(1.-ty)),(x.floor(),y.floor()+1.,(1.-tx)*ty),(x.floor()+1.,y.floor()+1.,tx*ty)];
    let mut result=None;
    let mut color=[0.;4];let mut alpha=0.;
    for (x,y,weight) in positions {
        let pixel=at(x,y,x.clamp(0.,f64::from(width-1))+0.5,y.clamp(0.,f64::from(height-1))+0.5)?;
        if let Some(space)=result { if !pixel.space.shares_blending_coordinates(space) {return None;} } else {result=Some(pixel.space);}
        alpha+=pixel.alpha*weight;
        for (out,value) in color.iter_mut().zip(pixel.components) { *out+=value*pixel.alpha*weight; }
    }
    if alpha>0. { for value in &mut color { *value/=alpha; } }
    if cancelled() {return None;}
    Some(NativeRasterSample {space:result?,components:color,alpha})
}

#[test]
fn native_raster_sampling_expands_palette_before_premultiplied_filtering() {
    for indexed in [false,true] {
        let bytes="0000FFFF>";
        let mask=format!("<< /Type /XObject /Subtype /Image /Width 2 /Height 1 /ColorSpace /DeviceGray /BitsPerComponent 16 /Filter /ASCIIHexDecode /Length {} >>\nstream\n{bytes}\nendstream",bytes.len());
        let (space,bytes)=if indexed {("[/Indexed /DeviceRGB 1 <FF00000000FF>]","0001>")}else{("/DeviceRGB","FF00000000FF>")};
        let image=format!("<< /Type /XObject /Subtype /Image /Width 2 /Height 1 /ColorSpace {space} /BitsPerComponent 8 /Interpolate true /SMask 10 0 R /Filter /ASCIIHexDecode /Length {} >>\nstream\n{bytes}\nendstream",bytes.len());
        let pdf=authored_pdf_with_font_blend(false,1.,"/form Do","","q 16 0 0 16 0 0 cm /image Do Q",&image,Some(&mask),false,"Normal");
        let scene=record_into(pdf,32,16,Affine::IDENTITY,Recorder {capture_native_rasters:true,..Recorder::default()});
        assert!(!scene.failed);
        let data=scene.native_rasters[0].1.as_ref().unwrap();
        let middle=sample_native_raster(data,kurbo::Point::new(1.,0.5),&||false).unwrap();
        assert_eq!(middle.alpha,0.5);
        assert_eq!(middle.components[..3],[0.,0.,1.]);
        let clear=sample_native_raster(data,kurbo::Point::new(0.5,0.5),&||false).unwrap();
        assert_eq!(clear.alpha,0.);
        assert_eq!(clear.components,[0.;4]);
        assert!(sample_native_raster(data,kurbo::Point::new(f64::NAN,0.),&||false).is_none());
        assert!(sample_native_raster(data,kurbo::Point::new(1.,0.5),&||true).is_none());
        let polls=std::cell::Cell::new(0);
        sample_native_raster(data,kurbo::Point::new(1.,0.5),&|| {polls.set(polls.get()+1);false}).unwrap();
        for stop in 1..=polls.get() {
            let now=std::cell::Cell::new(0);
            assert!(sample_native_raster(data,kurbo::Point::new(1.,0.5),&|| {now.set(now.get()+1);now.get()==stop}).is_none());
        }
    }
}

#[test]
fn actual_native_soft_image_replay_preserves_float_color_alpha_and_coordinates() {
    use hayro_interpret::color::{Color,AlphaColor};
    use super::native_blend::{Model,Pixel};
    let scene=record_into(authored_native_soft_image_fixture(false),32,16,Affine::IDENTITY,
        Recorder {capture_native_rasters:true,global_contour:true,native_polygon:true,admitted_strokes:true,..Recorder::default()});
    assert!(!scene.failed);
    let rgb=Color::from_rgba(AlphaColor::WHITE);
    let white=Pixel::from_native(Model::Rgb,&[1.;3],1.).unwrap();
    let output=replay_native_solid_scene(&scene,32,16,rgb.color_space(),white,&||false,&|_|Some(Box::new(()))).unwrap();
    let source=scene.native_rasters[0].1.as_ref().unwrap();
    let alpha=f64::from(source.alpha().unwrap().samples()[0]);
    let pixel=output.pixels[8*32+8];
    assert_eq!(pixel.alpha(),1.);
    let color=pixel.native_components().unwrap();
    for i in 0..3 {assert!((color[i]-(1.-alpha+alpha*f64::from(source.samples()[i]))).abs()<1e-14);}
    assert_eq!(&output.pixels[8*32+24].native_components().unwrap()[..3],&[1.;3]);
}

#[test]
fn actual_native_axial_shading_replay_preserves_original_cmyk() {
    use super::native_blend::{Model,Pixel};
    let pdf=authored_pdf_with_font(true,1.0,"/form Do","0 0 0 0 k 0 0 32 16 re f","/S1 sh",
        "<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>",Some(
        "<< /ShadingType 2 /ColorSpace /DeviceCMYK /Coords [0 0 32 0] /Function << /FunctionType 2 /Domain [0 1] /C0 [0.125 0.25 0.5 0.75] /C1 [0.625 0.75 0 0.25] /N 1 >> /Extend [true true] >>"));
    let scene=record_into(pdf,32,16,Affine::IDENTITY,Recorder {capture_native_shadings:true,global_contour:true,native_polygon:true,..Recorder::default()});
    assert!(!scene.failed);
    assert!(scene.events.iter().any(|event|matches!(event,Event::Path(_,_,RecordedPaint::Shading(paint),_,_) if paint.native.is_some())));
    let space=scene.events.iter().find_map(|event| match event {
        Event::Path(_,_,RecordedPaint::Shading(paint),_,_)=>paint.native.as_ref().map(|source|source.color_space.clone()),
        _=>None,
    }).unwrap();
    let white=Pixel::from_native(Model::Cmyk,&[0.;4],1.).unwrap();
    let output=replay_native_solid_scene(&scene,32,16,&space,white,&||false,&|_|Some(Box::new(()))).unwrap();
    for y in 0..16 {for x in 0..32 {
        let pixel=output.pixels[y*32+x];assert_eq!(pixel.alpha(),1.);
        let t=(x as f64+0.5)/32.;
        let expected=[0.125+0.5*t,0.25+0.5*t,0.5-0.5*t,0.75-0.5*t];
        assert_eq!(pixel.native_components().unwrap(),expected);
    }}
}

#[test]
fn actual_native_triangle_shading_stream_replay_preserves_original_cmyk() {
    use super::native_blend::{Model,Pixel};
    // Two Type4 triangles span the page; cyan varies linearly in x.
    // ASCIIHex carries actual 8-bit flag/coordinate/component records.
    let records = [
        [0,0,0,0,0,0,255], [0,255,0,255,0,0,0], [0,0,255,0,0,0,255],
        [0,255,0,255,0,0,0], [0,255,255,255,0,0,0], [0,0,255,0,0,0,255],
    ];
    let hex = records.iter().flatten().map(|v|format!("{v:02X}")).collect::<String>()+">";
    let shading = format!("<< /ShadingType 4 /ColorSpace /DeviceCMYK /BitsPerCoordinate 8 /BitsPerComponent 8 /BitsPerFlag 8 /Decode [0 32 0 16 0 1 0 1 0 1 0 1] /Filter /ASCIIHexDecode /Length {} >>\nstream\n{hex}\nendstream",hex.len());
    let pdf=authored_pdf_with_font(true,1.0,"/form Do","0 0 0 0 k 0 0 32 16 re f","/S1 sh",
        "<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>",Some(&shading));
    let scene=record_into(pdf,32,16,Affine::IDENTITY,Recorder {capture_native_shadings:true,global_contour:true,native_polygon:true,..Recorder::default()});
    assert!(!scene.failed);
    let space=scene.events.iter().find_map(|event| match event {
        Event::Path(_,_,RecordedPaint::Shading(paint),_,_)=>paint.native.as_ref().map(|source|source.color_space.clone()), _=>None,
    }).expect("actual Type4 stream captured in original CMYK");
    let white=Pixel::from_native(Model::Cmyk,&[0.;4],1.).unwrap();
    let output=replay_native_solid_scene(&scene,32,16,&space,white,&||false,&|_|Some(Box::new(()))).unwrap();
    for y in 0..16 {for x in 0..32 {
        let pixel=output.pixels[y*32+x]; assert_eq!(pixel.alpha(),1.);
        let t=(x as f64+0.5)/32.;
        assert_eq!(pixel.native_components().unwrap(),[t,0.,0.,1.-t],"pixel ({x},{y})");
    }}
}
