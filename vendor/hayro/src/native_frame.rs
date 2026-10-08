//! Test-only admitted native group buffers. Profile transforms and interpreter
//! event routing are separate; callers must supply a verified destination key.
use super::{PreciseCoverage, native_blend::Pixel};
use hayro_interpret::BlendMode;
type Credit = Box<dyn std::any::Any + Send + Sync>;
pub(super) struct Buffer {
    pub(super) pixels: Vec<Pixel>,
    _credit: Credit,
}
fn convert_group_pixel(
    pixel: Pixel, source: &hayro_interpret::color::ColorSpace,
    destination: &hayro_interpret::color::ColorSpace,
) -> Option<Pixel> {
    use hayro_interpret::color::BlendingModel;
    use super::native_blend::Model;
    if source.shares_blending_coordinates(destination) { return Some(pixel); }
    let model = match destination.blending_model()? {
        BlendingModel::Gray => Model::Gray, BlendingModel::Rgb => Model::Rgb, BlendingModel::Cmyk => Model::Cmyk,
    };
    if pixel.alpha()==0. { return Pixel::from_native(model,&[0.;4][..model.channels()],0.); }
    let sample = pixel.native_components()?.map(|value| value as f32);
    let input = &sample[..pixel.model().channels()];
    let converted = source.device_sample_to_rgb(destination,input)
        .or_else(|| source.device_sample_to_gray_or_cmyk(destination,input))
        .or_else(|| source.calibrated_sample_to_rgb(destination,input))?;
    let values = converted.map(f64::from);
    Pixel::from_native(model,&values[..model.channels()],pixel.alpha())
}
struct CoverageBuffer {
    values: Vec<PreciseCoverage>,
    _credit: Credit,
}
pub(super) struct Frame {
    initial: Buffer,
    current: Buffer,
    coverage: CoverageBuffer,
    space_key: u128,
    knockout: bool,
    failed: bool,
}
/// Own the verified target space together with the admitted native buffers.
/// The inner key is private: callers cannot tag arbitrary paint as converted.
pub(super) struct PaintFrame {
    frame: Frame,
    space: hayro_interpret::color::ColorSpace,
    _credit: Credit,
}
pub(super) struct PreparedPaint {
    pixel: Pixel,
    space: hayro_interpret::color::ColorSpace,
}
impl PaintFrame {
    // Backdrop components must already be expressed in `space`; cross-space
    // parent-backdrop conversion belongs to the future scene/group router.
    pub(super) fn admitted(
        backdrop: &[Pixel],
        space: &hayro_interpret::color::ColorSpace,
        isolated: bool,
        knockout: bool,
        cancelled: &dyn Fn() -> bool,
        admit: &dyn Fn(usize) -> Option<Credit>,
    ) -> Option<Box<Self>> {
        use super::native_blend::Model;
        use hayro_interpret::color::BlendingModel;
        if cancelled() {
            return None;
        }
        let model = match space.blending_model()? {
            BlendingModel::Gray => Model::Gray,
            BlendingModel::Rgb => Model::Rgb,
            BlendingModel::Cmyk => Model::Cmyk,
        };
        if backdrop.first()?.model() != model {
            return None;
        }
        let credit = admit(size_of::<Self>())?;
        let frame = Frame::admitted(backdrop, 1, isolated, knockout, cancelled, admit)?;
        if cancelled() {
            return None;
        }
        Some(Box::new(Self {
            frame,
            space: space.clone(),
            _credit: credit,
        }))
    }
    pub(super) fn apply_paint(
        &mut self,
        index: usize,
        paint: &super::native_paint::NativePaint,
        shape: f64,
        mode: BlendMode,
        cancelled: &dyn Fn() -> bool,
    ) -> Option<()> {
        self.apply_paint_with_alpha_source(index, paint, shape, false, mode, cancelled)
    }
    pub(super) fn apply_paint_with_alpha_source(
        &mut self,
        index: usize,
        paint: &super::native_paint::NativePaint,
        geometric_shape: f64,
        alpha_is_shape: bool,
        mode: BlendMode,
        cancelled: &dyn Fn() -> bool,
    ) -> Option<()> {
        let prepared = self.prepare_paint(paint, cancelled)?;
        self.apply_prepared(index, &prepared, geometric_shape, alpha_is_shape, mode, cancelled)
    }
    pub(super) fn prepare_native_pixel(
        &mut self, pixel: Pixel, source_space: &hayro_interpret::color::ColorSpace,
        cancelled: &dyn Fn() -> bool,
    ) -> Option<PreparedPaint> {
        let result=(|| {
            if self.frame.failed || cancelled() {return None;}
            let Some(pixel)=convert_group_pixel(pixel,source_space,&self.space) else {
                if std::env::var_os("RRRAH_TRACE_NATIVE_COLOR_REFUSAL").is_some() {
                    eprintln!("NATIVE_RASTER_COLOR_REFUSAL source={:?} destination={:?}",source_space.blending_model(),self.space.blending_model());
                }
                return None;
            };
            if cancelled() {return None;}
            Some(PreparedPaint {pixel,space:self.space.clone()})
        })();
        if result.is_none() {self.frame.failed=true;}
        result
    }
    pub(super) fn prepare_paint(
        &mut self,
        paint: &super::native_paint::NativePaint,
        cancelled: &dyn Fn() -> bool,
    ) -> Option<PreparedPaint> {
        if self.frame.failed || cancelled() {
            self.frame.failed = true;
            return None;
        }
        let Some(pixel) = paint.pixel_in_space(&self.space, 1.0) else {
            if std::env::var_os("RRRAH_TRACE_NATIVE_COLOR_REFUSAL").is_some() {
                eprintln!("NATIVE_COLOR_REFUSAL source={:?} destination={:?}",paint.color_space.blending_model(),self.space.blending_model());
            }
            self.frame.failed = true;
            return None;
        };
        if cancelled() {
            self.frame.failed = true;
            return None;
        }
        Some(PreparedPaint {
            pixel,
            space: self.space.clone(),
        })
    }
    pub(super) fn apply_prepared(
        &mut self,
        index: usize,
        paint: &PreparedPaint,
        geometric_shape: f64,
        alpha_is_shape: bool,
        mode: BlendMode,
        cancelled: &dyn Fn() -> bool,
    ) -> Option<()> {
        self.apply_prepared_masked(
            index,
            paint,
            geometric_shape,
            alpha_is_shape,
            1.0,
            mode,
            cancelled,
        )
    }
    pub(super) fn apply_prepared_masked(
        &mut self,
        index: usize,
        paint: &PreparedPaint,
        geometric_shape: f64,
        alpha_is_shape: bool,
        mask: f64,
        mode: BlendMode,
        cancelled: &dyn Fn() -> bool,
    ) -> Option<()> {
        if self.frame.failed || cancelled() || !self.space.shares_blending_coordinates(&paint.space) {
            self.frame.failed = true;
            return None;
        }
        let Some(source) = paint.pixel.scale_opacity(geometric_shape) else {
            self.frame.failed = true;
            return None;
        };
        let Some(source) = source.scale_opacity(mask) else {
            self.frame.failed = true;
            return None;
        };
        let shape = geometric_shape
            * if alpha_is_shape {
                paint.pixel.alpha() * mask
            } else {
                1.0
            };
        self.frame.apply(index, source, shape, 1, mode, cancelled)
    }
    pub(super) fn child(
        &mut self,
        isolated: bool,
        knockout: bool,
        cancelled: &dyn Fn() -> bool,
        admit: &dyn Fn(usize) -> Option<Credit>,
    ) -> Option<Box<Self>> {
        if self.frame.failed {
            return None;
        }
        let backdrop = if self.frame.knockout {
            &self.frame.initial
        } else {
            &self.frame.current
        };
        let child = Self::admitted(
            &backdrop.pixels,
            &self.space,
            isolated,
            knockout,
            cancelled,
            admit,
        );
        if child.is_none() {
            self.frame.failed = true;
        }
        child
    }
    /// Isolated groups start transparent in their declared coordinates.
    /// The scratch backdrop is admitted before allocation.
    pub(super) fn isolated_child_in_space(
        &mut self, target: &hayro_interpret::color::ColorSpace, knockout: bool,
        cancelled: &dyn Fn() -> bool, admit: &dyn Fn(usize) -> Option<Credit>,
    ) -> Option<Box<Self>> {
        let result = (|| {
            use hayro_interpret::color::BlendingModel;
            use super::native_blend::Model;
            if self.frame.failed || cancelled() { return None; }
            let model = match target.blending_model()? {
                BlendingModel::Gray => Model::Gray, BlendingModel::Rgb => Model::Rgb, BlendingModel::Cmyk => Model::Cmyk,
            };
            let count = self.frame.current.pixels.len();
            let _credit = admit(count.checked_mul(size_of::<Pixel>())?)?;
            let mut backdrop = Vec::new();
            backdrop.try_reserve_exact(count).ok()?;
            let clear = Pixel::from_native(model,&[0.;4][..model.channels()],0.)?;
            for _ in 0..count { if cancelled() { return None; } backdrop.push(clear); }
            Self::admitted(&backdrop,target,true,knockout,cancelled,admit)
        })();
        if result.is_none() { self.frame.failed = true; }
        result
    }
    pub(super) fn apply_group(
        &mut self,
        child: Box<Self>,
        opacity: f64,
        alpha_is_shape: bool,
        mode: BlendMode,
        cancelled: &dyn Fn() -> bool,
    ) -> Option<()> {
        self.apply_group_masked(child, opacity, alpha_is_shape, None, mode, cancelled)
    }
    pub(super) fn apply_group_masked(
        &mut self,
        child: Box<Self>,
        opacity: f64,
        alpha_is_shape: bool,
        mask: Option<&[f64]>,
        mode: BlendMode,
        cancelled: &dyn Fn() -> bool,
    ) -> Option<()> {
        let result = (|| {
            if self.frame.failed
                || child.frame.failed
                || cancelled()
                || !opacity.is_finite()
                || !(0.0..=1.0).contains(&opacity)
                || self.frame.current.pixels.len() != child.frame.current.pixels.len()
                || mask.is_some_and(|values| values.len() != self.frame.current.pixels.len())
            {
                return None;
            }
            if !self.space.shares_blending_coordinates(&child.space) {
                for pixel in &child.frame.initial.pixels {
                    if cancelled() || pixel.alpha() != 0. { return None; }
                }
            }
            for index in 0..self.frame.current.pixels.len() {
                if cancelled() {
                    return None;
                }
                let coverage = child.frame.coverage.values[index];
                let factor = mask.map_or(1.0, |values| values[index]);
                if !factor.is_finite() || !(0.0..=1.0).contains(&factor) {
                    return None;
                }
                let opacity = opacity * factor;
                let mut source = child.frame.current.pixels[index]
                    .remove_backdrop(child.frame.initial.pixels[index], coverage.alpha)?;
                source = convert_group_pixel(source,&child.space,&self.space)?;
                let source = source.scale_opacity(opacity)?;
                let shape = coverage.shape * if alpha_is_shape { opacity } else { 1.0 };
                self.frame.apply(index, source, shape, 1, mode, cancelled)?;
            }
            Some(())
        })();
        if result.is_none() {
            self.frame.failed = true;
        }
        result
    }
    pub(super) fn contribution(self: Box<Self>, cancelled: &dyn Fn() -> bool) -> Option<Buffer> {
        self.frame.contribution(cancelled)
    }
    pub(super) fn finish(self: Box<Self>, cancelled: &dyn Fn() -> bool) -> Option<Buffer> {
        if self.frame.failed || cancelled() {
            return None;
        }
        for pixel in &self.frame.current.pixels {
            if cancelled() || !pixel.valid() {
                return None;
            }
        }
        Some(self.frame.current)
    }
}

impl Frame {
    pub(super) fn admitted(
        backdrop: &[Pixel],
        space_key: u128,
        isolated: bool,
        knockout: bool,
        cancelled: &dyn Fn() -> bool,
        admit: &dyn Fn(usize) -> Option<Credit>,
    ) -> Option<Self> {
        let count = backdrop.len();
        if count == 0 || count > 16_777_216 || cancelled() {
            return None;
        }
        let model = backdrop[0].model();
        for &pixel in backdrop {
            if cancelled() || !pixel.valid() || pixel.model() != model {
                return None;
            }
        }
        let pixel_bytes = count.checked_mul(size_of::<Pixel>())?;
        let coverage_bytes = count.checked_mul(size_of::<PreciseCoverage>())?;
        // All three exact storage credits precede their allocations.
        let initial_credit = admit(pixel_bytes)?;
        let current_credit = admit(pixel_bytes)?;
        let coverage_credit = admit(coverage_bytes)?;
        if cancelled() {
            return None;
        }
        let mut initial = Vec::new();
        initial.try_reserve_exact(count).ok()?;
        let mut current = Vec::new();
        current.try_reserve_exact(count).ok()?;
        let mut coverage = Vec::new();
        coverage.try_reserve_exact(count).ok()?;
        let clear = Pixel::from_native(model, &[0.; 4][..model.channels()], 0.)?;
        for &pixel in backdrop {
            if cancelled() {
                return None;
            }
            let pixel = if isolated { clear } else { pixel };
            initial.push(pixel);
            current.push(pixel);
            coverage.push(PreciseCoverage::default());
        }
        Some(Self {
            initial: Buffer {
                pixels: initial,
                _credit: initial_credit,
            },
            current: Buffer {
                pixels: current,
                _credit: current_credit,
            },
            coverage: CoverageBuffer {
                values: coverage,
                _credit: coverage_credit,
            },
            space_key,
            knockout,
            failed: false,
        })
    }
    pub(super) fn apply(
        &mut self,
        index: usize,
        source: Pixel,
        shape: f64,
        source_space_key: u128,
        mode: BlendMode,
        cancelled: &dyn Fn() -> bool,
    ) -> Option<()> {
        if self.failed
            || cancelled()
            || source_space_key != self.space_key
            || index >= self.current.pixels.len()
        {
            self.failed = true;
            return None;
        }
        let previous = self.current.pixels[index];
        let backdrop = if self.knockout {
            self.initial.pixels[index]
        } else {
            previous
        };
        let next = Pixel::composite(previous, backdrop, source, shape, mode);
        let coverage = self.coverage.values[index].add(shape, source.alpha(), self.knockout);
        match (next, coverage) {
            (Some(next), Some(coverage)) => {
                self.current.pixels[index] = next;
                self.coverage.values[index] = coverage;
                Some(())
            }
            _ => {
                self.failed = true;
                None
            }
        }
    }
    /// Consume the frame so no cancelled/failed operation exposes partial pixels.
    /// The returned current buffer retains only its own credit; initial backdrop
    /// and coverage storage/credits drop before returning.
    pub(super) fn contribution(mut self, cancelled: &dyn Fn() -> bool) -> Option<Buffer> {
        if self.failed || cancelled() {
            return None;
        }
        for index in 0..self.current.pixels.len() {
            if cancelled() {
                return None;
            }
            self.current.pixels[index] = self.current.pixels[index]
                .remove_backdrop(self.initial.pixels[index], self.coverage.values[index].alpha)?;
        }
        Some(self.current)
    }
}

#[test]
fn admitted_cmyk_frame_keeps_shape_and_alpha_separate_and_releases_background() {
    use super::native_blend::Model;
    use std::sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    };
    struct Guard(Arc<AtomicUsize>, usize);
    impl Drop for Guard {
        fn drop(&mut self) {
            self.0.fetch_sub(self.1, Ordering::Relaxed);
        }
    }
    let used = Arc::new(AtomicUsize::new(0));
    let admit = |n| {
        used.fetch_add(n, Ordering::Relaxed);
        Some(Box::new(Guard(used.clone(), n)) as Credit)
    };
    let make = |c: &[f64], a| Pixel::from_native(Model::Cmyk, c, a).unwrap();
    let white = make(&[0.; 4], 1.);
    let mut frame = Frame::admitted(&[white; 2], 42, false, true, &|| false, &admit).unwrap();
    assert_eq!(
        used.load(Ordering::Relaxed),
        2 * (2 * size_of::<Pixel>() + size_of::<PreciseCoverage>())
    );
    let magenta = make(&[0., 1., 0., 0.], 1.);
    for i in 0..2 {
        frame
            .apply(i, magenta, 1., 42, BlendMode::Normal, &|| false)
            .unwrap();
    }
    frame
        .apply(0, make(&[1., 0., 0., 0.], 0.), 1., 42, BlendMode::Normal, &|| {
            false
        })
        .unwrap();
    frame
        .apply(
            1,
            make(&[1., 0., 0., 0.], 0.25),
            0.5,
            42,
            BlendMode::Normal,
            &|| false,
        )
        .unwrap();
    let output = frame.contribution(&|| false).unwrap();
    assert_eq!(output.pixels[0].alpha(), 0.);
    assert_eq!(output.pixels[1].alpha(), 0.75);
    for (v, e) in output.pixels[1]
        .native_components()
        .unwrap()
        .iter()
        .zip([1. / 3., 2. / 3., 0., 0.])
    {
        assert!((v - e).abs() < 1e-14);
    }
    assert_eq!(used.load(Ordering::Relaxed), 2 * size_of::<Pixel>());
    drop(output);
    assert_eq!(used.load(Ordering::Relaxed), 0);
}
#[test]
fn native_frame_all_admission_and_cancel_paths_release_storage_and_latch_failure() {
    use super::native_blend::Model;
    use std::cell::Cell;
    use std::sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    };
    struct Guard(Arc<AtomicUsize>, usize);
    impl Drop for Guard {
        fn drop(&mut self) {
            self.0.fetch_sub(self.1, Ordering::Relaxed);
        }
    }
    let used = Arc::new(AtomicUsize::new(0));
    let admit = |n| {
        used.fetch_add(n, Ordering::Relaxed);
        Some(Box::new(Guard(used.clone(), n)) as Credit)
    };
    let pixel = Pixel::from_native(Model::Rgb, &[0.2, 0.3, 0.4], 1.).unwrap();
    let polls = Cell::new(0);
    let frame = Frame::admitted(
        &[pixel; 8],
        7,
        true,
        false,
        &|| {
            polls.set(polls.get() + 1);
            false
        },
        &admit,
    )
    .unwrap();
    drop(frame);
    for stop in 1..=polls.get() {
        let call = Cell::new(0);
        assert!(
            Frame::admitted(
                &[pixel; 8],
                7,
                true,
                false,
                &|| {
                    call.set(call.get() + 1);
                    call.get() == stop
                },
                &admit
            )
            .is_none()
        );
        assert_eq!(used.load(Ordering::Relaxed), 0);
    }
    for deny in 1..=3 {
        let call = Cell::new(0);
        assert!(
            Frame::admitted(&[pixel; 8], 7, true, false, &|| false, &|n| {
                call.set(call.get() + 1);
                if call.get() == deny { None } else { admit(n) }
            })
            .is_none()
        );
        assert_eq!(used.load(Ordering::Relaxed), 0);
    }
    for stop in 1..=9 {
        let frame = Frame::admitted(&[pixel; 8], 7, true, false, &|| false, &admit).unwrap();
        let call = Cell::new(0);
        assert!(
            frame
                .contribution(&|| {
                    call.set(call.get() + 1);
                    call.get() == stop
                })
                .is_none()
        );
        assert_eq!(used.load(Ordering::Relaxed), 0);
    }
    for (index, key, cancel) in [(0, 8, false), (8, 7, false), (0, 7, true)] {
        let mut frame = Frame::admitted(&[pixel; 8], 7, true, false, &|| false, &admit).unwrap();
        frame
            .apply(0, pixel, 1., 7, BlendMode::Normal, &|| false)
            .unwrap();
        assert!(
            frame
                .apply(index, pixel, 1., key, BlendMode::Normal, &|| cancel)
                .is_none()
        );
        assert!(frame.contribution(&|| false).is_none());
        assert_eq!(used.load(Ordering::Relaxed), 0);
    }
}

#[test]
fn paint_frame_metadata_credit_and_conversion_failure_are_atomic() {
    use super::{native_blend::Model, native_paint::NativePaint};
    use hayro_interpret::color::{AlphaColor, Color};
    use std::cell::Cell;
    use std::sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    };
    struct CreditGuard(Arc<AtomicUsize>, usize);
    impl Drop for CreditGuard {
        fn drop(&mut self) {
            self.0.fetch_sub(self.1, Ordering::Relaxed);
        }
    }
    let used = Arc::new(AtomicUsize::new(0));
    let calls = Cell::new(0);
    let denied = Cell::new(0);
    let admit = |bytes| {
        calls.set(calls.get() + 1);
        if calls.get() == denied.get() {
            return None;
        }
        used.fetch_add(bytes, Ordering::Relaxed);
        Some(Box::new(CreditGuard(used.clone(), bytes)) as Credit)
    };
    let color = Color::from_rgba(AlphaColor::new([0.17, 0.31, 0.53, 0.37]));
    let paint = NativePaint::capture(&color, &|| false, &|_| Some(Box::new(()))).unwrap();
    let clear = Pixel::from_native(Model::Rgb, &[0.; 3], 0.).unwrap();
    for stop in 1..=4 {
        calls.set(0);
        denied.set(stop);
        assert!(
            PaintFrame::admitted(&[clear], color.color_space(), false, true, &|| false, &admit).is_none()
        );
        assert_eq!(used.load(Ordering::Relaxed), 0);
    }
    calls.set(0);
    denied.set(0);
    let mut frame =
        PaintFrame::admitted(&[clear], color.color_space(), false, true, &|| false, &admit).unwrap();
    assert_eq!(calls.get(), 4);
    frame
        .apply_paint(0, &paint, 0.5, BlendMode::Normal, &|| false)
        .unwrap();
    let output = frame.contribution(&|| false).unwrap();
    assert_eq!(output.pixels[0].alpha(), f64::from(color.opacity()) * 0.5);
    assert_eq!(used.load(Ordering::Relaxed), size_of::<Pixel>());
    drop(output);
    assert_eq!(used.load(Ordering::Relaxed), 0);
    for cancel in [false, true] {
        let mut frame =
            PaintFrame::admitted(&[clear], color.color_space(), false, true, &|| false, &admit).unwrap();
        assert!(
            frame
                .apply_paint(
                    0,
                    &paint,
                    if cancel { 0.5 } else { -0.1 },
                    BlendMode::Normal,
                    &|| cancel
                )
                .is_none()
        );
        assert!(frame.contribution(&|| false).is_none());
        assert_eq!(used.load(Ordering::Relaxed), 0);
    }
}

#[test]
fn nested_paint_frames_preserve_backdrop_opacity_shape_and_failure_latch() {
    use super::{native_blend::Model, native_paint::NativePaint};
    use hayro_interpret::color::{AlphaColor, Color};
    let red = Color::from_rgba(AlphaColor::new([1., 0., 0., 1.]));
    let blue = Color::from_rgba(AlphaColor::new([0., 0., 1., 0.5]));
    let admit = |_: usize| Some(Box::new(()) as Credit);
    let red = NativePaint::capture(&red, &|| false, &admit).unwrap();
    let blue = NativePaint::capture(&blue, &|| false, &admit).unwrap();
    let clear = Pixel::from_native(Model::Rgb, &[0.; 3], 0.).unwrap();
    for isolated in [false, true] {
        let mut parent =
            PaintFrame::admitted(&[clear], &red.color_space, false, false, &|| false, &admit).unwrap();
        parent
            .apply_paint(0, &red, 1., BlendMode::Normal, &|| false)
            .unwrap();
        let mut child = parent.child(isolated, false, &|| false, &admit).unwrap();
        child
            .apply_paint(0, &blue, 1., BlendMode::Normal, &|| false)
            .unwrap();
        parent
            .apply_group(child, 0.5, false, BlendMode::Normal, &|| false)
            .unwrap();
        let output = parent.contribution(&|| false).unwrap();
        assert_eq!(output.pixels[0].alpha(), 1.0);
        assert_eq!(
            output.pixels[0].native_components().unwrap()[..3],
            [0.75, 0., 0.25]
        );
    }
    // AIS controls shape independently of source alpha in a knockout parent.
    for (ais, expected_alpha) in [(false, 0.25), (true, 0.75)] {
        let mut parent =
            PaintFrame::admitted(&[clear], &red.color_space, false, true, &|| false, &admit).unwrap();
        parent
            .apply_paint(0, &red, 1., BlendMode::Normal, &|| false)
            .unwrap();
        let mut child = parent.child(true, false, &|| false, &admit).unwrap();
        child
            .apply_paint(0, &blue, 1., BlendMode::Normal, &|| false)
            .unwrap();
        parent
            .apply_group(child, 0.5, ais, BlendMode::Normal, &|| false)
            .unwrap();
        assert_eq!(
            parent.contribution(&|| false).unwrap().pixels[0].alpha(),
            expected_alpha
        );
    }
    let mut parent =
        PaintFrame::admitted(&[clear; 2], &red.color_space, false, false, &|| false, &admit).unwrap();
    let child = parent.child(true, false, &|| false, &admit).unwrap();
    let polls = std::cell::Cell::new(0);
    assert!(
        parent
            .apply_group(child, 1., false, BlendMode::Normal, &|| {
                polls.set(polls.get() + 1);
                polls.get() == 4
            })
            .is_none()
    );
    assert!(parent.contribution(&|| false).is_none());
    let mut parent =
        PaintFrame::admitted(&[clear], &red.color_space, false, false, &|| false, &admit).unwrap();
    assert!(parent.child(false, false, &|| false, &|_| None).is_none());
    assert!(parent.contribution(&|| false).is_none());
}

#[test]
fn prepared_paint_applies_leaf_alpha_source_once() {
    use super::{native_blend::Model, native_paint::NativePaint};
    use hayro_interpret::color::{AlphaColor, Color};
    let admit = |_: usize| Some(Box::new(()) as Credit);
    let red = Color::from_rgba(AlphaColor::new([1., 0., 0., 1.]));
    let blue = Color::from_rgba(AlphaColor::new([0., 0., 1., 0.5]));
    let red = NativePaint::capture(&red, &|| false, &admit).unwrap();
    let blue = NativePaint::capture(&blue, &|| false, &admit).unwrap();
    let clear = Pixel::from_native(Model::Rgb, &[0.; 3], 0.).unwrap();
    for ais in [false, true] {
        let mut frame =
            PaintFrame::admitted(&[clear], &red.color_space, false, true, &|| false, &admit).unwrap();
        frame
            .apply_paint(0, &red, 1., BlendMode::Normal, &|| false)
            .unwrap();
        let prepared = frame.prepare_paint(&blue, &|| false).unwrap();
        frame
            .apply_prepared(0, &prepared, 0.5, ais, BlendMode::Normal, &|| false)
            .unwrap();
        let output = frame.finish(&|| false).unwrap();
        assert_eq!(output.pixels[0].alpha(), if ais { 1.0 } else { 0.75 });
        assert_eq!(
            output.pixels[0].native_components().unwrap()[..3],
            if ais {
                [0.75, 0., 0.25]
            } else {
                [2.0 / 3.0, 0., 1.0 / 3.0]
            }
        );
    }
}
