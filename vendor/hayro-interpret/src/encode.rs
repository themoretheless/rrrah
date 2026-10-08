//! Encoding shading patterns for easy sampling.

mod native_source;
pub use native_source::{NativeShadingError, NativeShadingSample, NativeShadingSource};

use crate::color::{AlphaColor, ColorComponents, ColorSpace};
use crate::interpret::state::ActiveTransferFunction;
use crate::pattern::ShadingPattern;
use crate::shading::{ShadingFunction, ShadingType, Triangle};
use kurbo::{Affine, Point};
use rustc_hash::{FxBuildHasher, FxHashSet};
use smallvec::{ToSmallVec, smallvec};

/// A shading pattern that was encoded so it can be sampled.
#[derive(Debug)]
pub struct EncodedShadingPattern {
    /// The base transform of the shading pattern.
    pub base_transform: Affine,
    pub(crate) color_space: ColorSpace,
    pub(crate) background_color: AlphaColor,
    pub(crate) shading_type: EncodedShadingType,
    pub(crate) opacity: f32,
    pub(crate) transfer_function: Option<ActiveTransferFunction>,
}

impl EncodedShadingPattern {
    /// Sample the shading at the given position.
    #[inline]
    pub fn sample(&self, pos: Point) -> [f32; 4] {
        self.shading_type
            .eval(pos, self.background_color, &self.color_space)
            .map(|v| {
                let mut components = v.components();
                components[3] *= self.opacity;

                if let Some(tf) = &self.transfer_function {
                    return tf.apply(&AlphaColor::new(components)).components();
                }

                components
            })
            .unwrap_or([0.0, 0.0, 0.0, 0.0])
    }
}

impl ShadingPattern {
    /// Encode the shading pattern.
    pub fn encode(&self) -> EncodedShadingPattern {
        self.encode_with_sample_bounds(None)
    }

    /// Bound sampled mesh work to the device-space pixels requested by the consumer.
    /// Analytic shadings are unaffected. Bounds are expanded conservatively for
    /// fractional texture origins and partially covered final texture pixels.
    pub fn encode_with_sample_bounds(&self, bounds: Option<kurbo::Rect>) -> EncodedShadingPattern {
        self.encode_with_sample_bounds_and_cancel(bounds, &|| false).unwrap()
    }

    /// Encode with cooperative cancellation during patch construction and sampling.
    pub fn encode_with_sample_bounds_and_cancel(&self, bounds: Option<kurbo::Rect>, cancelled: &dyn Fn() -> bool) -> Option<EncodedShadingPattern> {
        self.encode_with_sample_bounds_cancel_and_admission(bounds, cancelled, &|_| Some(Box::new(())))
    }

    /// Admit temporary patch triangle storage before materializing it.
    pub fn encode_with_sample_bounds_cancel_and_admission(&self, bounds: Option<kurbo::Rect>, cancelled: &dyn Fn() -> bool, admit: &dyn Fn(usize) -> Option<Box<dyn std::any::Any + Send + Sync>>) -> Option<EncodedShadingPattern> {
        if cancelled() { return None; }
        let base_transform;

        let shading_type = match self.shading.shading_type.as_ref() {
            ShadingType::FunctionBased {
                domain,
                matrix,
                function,
            } => {
                base_transform = (self.matrix * *matrix).inverse();
                encode_function_shading(domain, function)
            }
            ShadingType::RadialAxial {
                coords,
                domain,
                function,
                extend,
                axial,
            } => {
                let (encoded, initial_transform) =
                    encode_axial_shading(*coords, *domain, function, *extend, *axial);

                base_transform = initial_transform * self.matrix.inverse();

                encoded
            }
            ShadingType::TriangleMesh { triangles, function } => {
                let full_transform = self.matrix;
                let samples = sample_triangles_with_admission(triangles, full_transform, bounds, cancelled, admit)?;

                base_transform = Affine::IDENTITY;

                EncodedShadingType::Sampled {
                    samples,
                    function: function.clone(),
                }
            }
            ShadingType::CoonsPatchMesh { patches, function } => {
                let visible = visible_patch_geometry(patches.iter().map(|patch| &patch.control_points));
                // Iterator handoff can briefly retain the exhausted previous allocation.
                let count: usize = if visible.iter().any(|visible| *visible) { 2 * 722 } else { 0 };
                let components = patches.iter().zip(&visible).filter(|(_, visible)| **visible)
                    .map(|(patch, _)| patch.colors[0].len()).max().unwrap_or(0);
                let spill = color_spill_bound(components)?;
                let bytes = count.checked_mul(size_of::<Triangle>())?
                    .checked_add(count.checked_mul(3)?.checked_add(64)?.checked_mul(spill)?)?;
                let _triangle_guard = admit(bytes)?;
                if cancelled() { return None; }
                let triangles = stream_patch_triangles(
                    patches.iter().zip(visible).filter(|(_, visible)| *visible).map(|(patch, _)| patch),
                    cancelled, |patch, triangles| patch.to_triangles_with_cancel(triangles, cancelled),
                );

                let full_transform = self.matrix;
                let samples = sample_triangle_iter_with_admission(triangles, components, full_transform, bounds, cancelled, admit)?;

                base_transform = Affine::IDENTITY;

                EncodedShadingType::Sampled {
                    samples,
                    function: function.clone(),
                }
            }
            ShadingType::TensorProductPatchMesh { patches, function } => {
                let visible = visible_patch_geometry(patches.iter().map(|patch| &patch.control_points));
                // Iterator handoff can briefly retain the exhausted previous allocation.
                let count: usize = if visible.iter().any(|visible| *visible) { 2 * 722 } else { 0 };
                let components = patches.iter().zip(&visible).filter(|(_, visible)| **visible)
                    .map(|(patch, _)| patch.colors[0].len()).max().unwrap_or(0);
                let spill = color_spill_bound(components)?;
                let bytes = count.checked_mul(size_of::<Triangle>())?
                    .checked_add(count.checked_mul(3)?.checked_add(64)?.checked_mul(spill)?)?;
                let _triangle_guard = admit(bytes)?;
                if cancelled() { return None; }
                let triangles = stream_patch_triangles(
                    patches.iter().zip(visible).filter(|(_, visible)| *visible).map(|(patch, _)| patch),
                    cancelled, |patch, triangles| patch.to_triangles_with_cancel(triangles, cancelled),
                );

                let full_transform = self.matrix;
                let samples = sample_triangle_iter_with_admission(triangles, components, full_transform, bounds, cancelled, admit)?;

                base_transform = Affine::IDENTITY;

                EncodedShadingType::Sampled {
                    samples,
                    function: function.clone(),
                }
            }
            ShadingType::Dummy => {
                base_transform = Affine::IDENTITY;

                EncodedShadingType::Dummy
            }
        };

        let color_space = self.shading.color_space.clone();

        let background_color = self
            .shading
            .background
            .as_ref()
            .filter(|_| self.paint_background)
            .map(|b| color_space.to_rgba(b, 1.0, false))
            .unwrap_or(AlphaColor::TRANSPARENT);

        Some(EncodedShadingPattern {
            color_space,
            background_color,
            shading_type,
            base_transform,
            opacity: self.opacity,
            transfer_function: self.transfer_function.clone(),
        })
    }
}

fn encode_axial_shading(
    coords: [f32; 6],
    domain: [f32; 2],
    function: &ShadingFunction,
    extend: [bool; 2],
    is_axial: bool,
) -> (EncodedShadingType, Affine) {
    let initial_transform;

    let params = if is_axial {
        let [x_0, y_0, x_1, y_1, _, _] = coords;

        initial_transform = ts_from_line_to_line(
            Point::new(x_0 as f64, y_0 as f64),
            Point::new(x_1 as f64, y_1 as f64),
            Point::ZERO,
            Point::new(1.0, 0.0),
        );

        RadialAxialParams::Axial
    } else {
        let [x_0, y_0, r0, x_1, y_1, r_1] = coords;

        initial_transform = Affine::translate((-x_0 as f64, -y_0 as f64));
        let new_x1 = x_1 - x_0;
        let new_y1 = y_1 - y_0;

        let p1 = Point::new(new_x1 as f64, new_y1 as f64);
        let r = Point::new(r0 as f64, r_1 as f64);

        RadialAxialParams::Radial { p1, r }
    };

    (
        EncodedShadingType::RadialAxial {
            function: function.clone(),
            params,
            domain,
            extend,
        },
        initial_transform,
    )
}

// A later patch with identical control geometry covers the earlier patch at
// every location, including fractional edge pixels. Retain source order for all
// other patches; this does not solve arbitrary partial-overlap integration.
fn visible_patch_geometry<'a, const N: usize>(patches: impl DoubleEndedIterator<Item = &'a [Point; N]> + ExactSizeIterator) -> Vec<bool> {
    let mut seen = FxHashSet::default();
    let mut visible = Vec::with_capacity(patches.len());
    for points in patches.rev() {
        if points.iter().any(|point| !point.x.is_finite() || !point.y.is_finite()) {
            visible.push(true);
            continue;
        }
        let key: Vec<_> = points.iter().map(|point| (
            if point.x == 0.0 { 0 } else { point.x.to_bits() },
            if point.y == 0.0 { 0 } else { point.y.to_bits() },
        )).collect();
        visible.push(seen.insert(key));
    }
    visible.reverse();
    visible
}

type MeshMap = hashbrown::HashMap<(u16, u16), MeshSample, FxBuildHasher>;

pub(crate) struct MeshSamples {
    map: MeshMap,
    // Declared last: the table must drop before its managed credit.
    _guard: Option<Box<dyn std::any::Any + Send + Sync>>,
}
impl std::fmt::Debug for MeshSamples {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result { self.map.fmt(f) }
}
impl std::ops::Deref for MeshSamples {
    type Target = MeshMap;
    fn deref(&self) -> &MeshMap { &self.map }
}

// Conservative bound for pinned hashbrown 0.16.1 table storage, including
// control bytes, alignment and separately allocated per-entry color components.
fn color_spill_bound(components: usize) -> Option<usize> {
    if components <= 4 { Some(0) }
    else { components.checked_next_power_of_two()?.checked_mul(size_of::<f32>())?.checked_mul(2) }
}

fn mesh_table_bound(capacity: usize, components: usize) -> Option<usize> {
    let buckets = if capacity < 4 { 4 } else if capacity < 8 { 8 }
        else { capacity.checked_mul(8)?.div_ceil(7).checked_next_power_of_two()? };
    buckets.checked_mul(size_of::<((u16,u16),MeshSample)>() + 1)?.checked_add(128)?
        .checked_add(buckets.checked_mul(color_spill_bound(components)?)?)
}

#[derive(Debug)]
pub(crate) struct MeshSample {
    colors: ColorComponents,
    coverage: f32,
}

// A triangle clipped against a rectangle has at most seven vertices.
// Coordinates are local to the pixel to avoid large-coordinate area cancellation.
fn triangle_pixel_area(t: &Triangle, x: u16, y: u16) -> Option<(f32, Point)> {
    let origin = Point::new(f64::from(x), f64::from(y));
    let mut points = [Point::ZERO; 8];
    for (i, p) in [t.p0.point, t.p1.point, t.p2.point].into_iter().enumerate() {
        points[i] = Point::new(p.x - origin.x, p.y - origin.y);
    }
    let mut len = 3;
    for (axis, bound, greater) in [(0, 0.0, true), (0, 1.0, false), (1, 0.0, true), (1, 1.0, false)] {
        if len == 0 {
            return None;
        }
        let mut out = [Point::ZERO; 8];
        let mut count = 0;
        let coordinate = |p: Point| if axis == 0 { p.x } else { p.y };
        let inside = |p: Point| {
            if greater {
                coordinate(p) >= bound
            } else {
                coordinate(p) <= bound
            }
        };
        let mut previous = points[len - 1];
        for current in points[..len].iter().copied() {
            if inside(previous) != inside(current) {
                let ratio = (bound - coordinate(previous)) / (coordinate(current) - coordinate(previous));
                out[count] = previous + (current - previous) * ratio;
                count += 1;
            }
            if inside(current) {
                out[count] = current;
                count += 1;
            }
            previous = current;
        }
        points = out;
        len = count;
    }
    let mut cross_sum = 0.0;
    let mut cx = 0.0;
    let mut cy = 0.0;
    for i in 0..len {
        let a = points[i];
        let b = points[(i + 1) % len];
        let cross = a.x * b.y - b.x * a.y;
        cross_sum += cross;
        cx += (a.x + b.x) * cross;
        cy += (a.y + b.y) * cross;
    }
    if cross_sum.abs() <= 1e-15 {
        return None;
    }
    Some((
        (cross_sum.abs() * 0.5) as f32,
        Point::new(
            origin.x + cx / (3.0 * cross_sum),
            origin.y + cy / (3.0 * cross_sum),
        ),
    ))
}

#[cfg(test)]
fn sample_triangles(triangles: &[Triangle], transform: Affine, bounds: Option<kurbo::Rect>) -> MeshSamples {
    sample_triangles_with_cancel(triangles, transform, bounds, &|| false).unwrap()
}

#[cfg(test)]
fn sample_triangles_with_cancel(triangles: &[Triangle], transform: Affine, bounds: Option<kurbo::Rect>, cancelled: &dyn Fn() -> bool) -> Option<MeshSamples> {
    sample_triangles_with_admission(triangles, transform, bounds, cancelled, &|_| Some(Box::new(())))
}

fn sample_triangles_with_admission(triangles: &[Triangle], transform: Affine, bounds: Option<kurbo::Rect>, cancelled: &dyn Fn() -> bool, admit: &dyn Fn(usize) -> Option<Box<dyn std::any::Any + Send + Sync>>) -> Option<MeshSamples> {
    if cancelled() { return None; }
    let components = triangles.iter().flat_map(|t| [&t.p0, &t.p1, &t.p2])
        .map(|vertex| vertex.colors.len()).max().unwrap_or(0);
    sample_triangle_iter_with_admission(triangles.iter().cloned().map(Ok), components, transform, bounds, cancelled, admit)
}

// A cancellation is yielded explicitly once: consumers cannot accidentally
// accept a partial stream if a predicate changes back after the checkpoint.
fn stream_patch_triangles<'a, P: 'a>(
    mut patches: impl Iterator<Item = &'a P> + 'a,
    cancelled: &'a dyn Fn() -> bool,
    generate: impl Fn(&P, &mut Vec<Triangle>) -> Result<(), ()> + 'a,
) -> impl Iterator<Item = Result<Triangle, ()>> + 'a {
    let mut current = Vec::new().into_iter();
    let mut stopped = false;
    std::iter::from_fn(move || {
        if stopped { return None; }
        loop {
            if cancelled() { stopped = true; return Some(Err(())); }
            if let Some(triangle) = current.next() { return Some(Ok(triangle)); }
            let patch = patches.next()?;
            if cancelled() { stopped = true; return Some(Err(())); }
            let mut triangles = Vec::with_capacity(722);
            if generate(patch, &mut triangles).is_err() { stopped = true; return Some(Err(())); }
            current = triangles.into_iter();
        }
    })
}

fn sample_triangle_iter_with_admission<I: IntoIterator<Item = Result<Triangle, ()>>>(triangles: I, components: usize, transform: Affine, bounds: Option<kurbo::Rect>, cancelled: &dyn Fn() -> bool, admit: &dyn Fn(usize) -> Option<Box<dyn std::any::Any + Send + Sync>>) -> Option<MeshSamples> {
    if cancelled() { return None; }
    let spill = color_spill_bound(components)?;
    // Input and transformed vertex clones plus one interpolated color buffer. The
    // bound includes overlap during SmallVec growth.
    let _scratch_guard = if spill == 0 { None } else { Some(admit(spill.checked_mul(8)?)?) };
    let mut guard = None;
    let mut map: MeshMap = MeshMap::default();

    for t in triangles {
        let t = t.ok()?;
        if cancelled() { return None; }
        let t = {
            let p0 = transform * t.p0.point;
            let p1 = transform * t.p1.point;
            let p2 = transform * t.p2.point;

            let mut v0 = t.p0.clone();
            v0.point = p0;
            let mut v1 = t.p1.clone();
            v1.point = p1;
            let mut v2 = t.p2.clone();
            v2.point = p2;

            Triangle::new(v0, v1, v2)
        };

        let bbox = t.bounding_box();
        let bbox = if let Some(bounds) = bounds.filter(|r| [r.x0,r.y0,r.x1,r.y1].iter().all(|v| v.is_finite()) && r.width() >= 0.0 && r.height() >= 0.0) {
            bbox.intersect(bounds.inflate(2.0, 2.0))
        } else { bbox };

        for y in (bbox.y0.floor() as u16)..(bbox.y1.ceil() as u16) {
            if cancelled() { return None; }
            for x in (bbox.x0.floor() as u16)..(bbox.x1.ceil() as u16) {
                if x % 64 == 0 && cancelled() { return None; }
                if let Some((area, point)) = triangle_pixel_area(&t, x, y) {
                    let colors = t.interpolate(point);
                    if map.len() == map.capacity() && !map.contains_key(&(x,y)) {
                        let capacity = map.capacity().checked_mul(2)?.max(3);
                        let bytes = mesh_table_bound(capacity, components)?;
                        // Both old and new tables exist during rehashing.
                        let next_guard = admit(bytes)?;
                        if cancelled() { return None; }
                        map.reserve(capacity - map.len());
                        assert!(map.allocation_size() <= bytes, "mesh table admission bound");
                        guard = Some(next_guard);
                    }
                    let sample = map.entry((x, y)).or_insert_with(|| MeshSample {
                        colors: colors.iter().map(|_| 0.0).collect(),
                        coverage: 0.0,
                    });
                    for (total, color) in sample.colors.iter_mut().zip(colors) {
                        *total += color * area;
                    }
                    sample.coverage += area;
                }
            }
        }
    }

    for (index, sample) in map.values_mut().enumerate() {
        if index % 64 == 0 && cancelled() { return None; }
        for color in &mut sample.colors {
            *color /= sample.coverage;
        }
        sample.coverage = sample.coverage.min(1.0);
    }
    if cancelled() { return None; }
    Some(MeshSamples { map, _guard: guard })
}

#[cfg(test)]
mod sample_bounds_tests {
    use super::*;
    use crate::shading::TensorProductPatch;

    #[test]
    fn patch_internal_cancellation_rolls_back_every_checkpoint() {
        use crate::shading::CoonsPatch;
        use std::cell::Cell;
        let tensor = TensorProductPatch {
            control_points: std::array::from_fn(|i| Point::new(i as f64 / 3., (i * i % 7) as f64)),
            colors: std::array::from_fn(|_| (0..9).map(|i| i as f32 / 10.).collect()),
        };
        let coons = CoonsPatch {
            control_points: std::array::from_fn(|i| tensor.control_points[i]),
            colors: tensor.colors.clone(),
        };
        let mut seed = Vec::new();
        tensor.to_triangles(&mut seed);
        for kind in 0..2 {
            let build = |output: &mut Vec<Triangle>, cancelled: &dyn Fn() -> bool| {
                if kind == 0 { tensor.to_triangles_with_cancel(output, cancelled) }
                else { coons.to_triangles_with_cancel(output, cancelled) }
            };
            let mut expected = vec![seed[0].clone()];
            if kind == 0 { tensor.to_triangles(&mut expected); }
            else { coons.to_triangles(&mut expected); }
            let checkpoints = Cell::new(0);
            let mut actual = vec![seed[0].clone()];
            build(&mut actual, &|| { checkpoints.set(checkpoints.get() + 1); false }).unwrap();
            assert_eq!(actual.len(), expected.len());
            for (a, b) in actual.iter().zip(&expected) {
                for (a, b) in [&a.p0, &a.p1, &a.p2].into_iter().zip([&b.p0, &b.p1, &b.p2]) {
                    assert_eq!(a.point.x.to_bits(), b.point.x.to_bits());
                    assert_eq!(a.point.y.to_bits(), b.point.y.to_bits());
                    assert_eq!(a.colors, b.colors);
                }
            }
            let count = checkpoints.get();
            assert!(count > 700);
            for stop in 1..=count {
                let current = Cell::new(0);
                let mut output = vec![seed[0].clone()];
                assert!(build(&mut output, &|| {
                    current.set(current.get() + 1);
                    current.get() == stop
                }).is_err(), "kind {kind}, checkpoint {stop}/{count}");
                assert_eq!(current.get(), stop);
                assert_eq!(output.len(), 1);
                assert_eq!(output[0].p0.point, seed[0].p0.point);
                assert_eq!(output[0].p0.colors, seed[0].p0.colors);
            }
        }
    }

    #[test]
    fn patch_stream_cancellation_is_explicit_and_stops_generation() {
        use std::cell::Cell;
        use std::sync::{Arc, atomic::{AtomicUsize, Ordering}};
        struct Credit { used: Arc<AtomicUsize>, bytes: usize }
        impl Drop for Credit {
            fn drop(&mut self) { self.used.fetch_sub(self.bytes, Ordering::Relaxed); }
        }
        let used = Arc::new(AtomicUsize::new(0));
        let admit = |bytes| {
            used.fetch_add(bytes, Ordering::Relaxed);
            Some(Box::new(Credit { used: used.clone(), bytes }) as Box<dyn std::any::Any + Send + Sync>)
        };
        let mut seed = Vec::new();
        TensorProductPatch {
            control_points: [Point::ZERO; 16],
            colors: std::array::from_fn(|_| (0..9).map(|i| i as f32 / 10.).collect()),
        }.to_triangles(&mut seed);
        let patches = [0u8, 1, 2];
        let calls = Cell::new(0);
        let generated = Cell::new(0);
        let cancelled = || { calls.set(calls.get() + 1); false };
        let stream = stream_patch_triangles(patches.iter(), &cancelled, |_, output| {
            generated.set(generated.get() + 1);
            output.extend(seed[..2].iter().cloned());
            Ok(())
        });
        assert!(sample_triangle_iter_with_admission(stream, 9, Affine::IDENTITY, None, &cancelled, &admit).is_some());
        assert_eq!(generated.get(), patches.len());
        assert_eq!(used.load(Ordering::Relaxed), 0);
        let count = calls.get();
        assert!(count > 10);
        for stop in 1..=count {
            calls.set(0);
            generated.set(0);
            // Return true once only; an explicit iterator error must survive
            // the next false predicate value and prevent partial acceptance.
            let cancelled = || { calls.set(calls.get() + 1); calls.get() == stop };
            let stream = stream_patch_triangles(patches.iter(), &cancelled, |_, output| {
                generated.set(generated.get() + 1);
                output.extend(seed[..2].iter().cloned());
                Ok(())
            });
            assert!(sample_triangle_iter_with_admission(stream, 9, Affine::IDENTITY, None, &cancelled, &admit).is_none(), "checkpoint {stop}/{count}");
            assert_eq!(calls.get(), stop);
            assert_eq!(used.load(Ordering::Relaxed), 0);
            if stop <= 3 { assert_eq!(generated.get(), 0); }
        }
        let stream = stream_patch_triangles(patches.iter(), &|| true, |_, _| panic!("cancelled generation"));
        let mut stream = stream;
        assert!(matches!(stream.next(), Some(Err(()))));
        assert!(stream.next().is_none());
    }

    #[test]
    fn table_growth_and_multicomponent_spills_release_credit() {
        use std::sync::{Arc, atomic::{AtomicUsize, Ordering}};
        struct Credit { used: Arc<AtomicUsize>, bytes: usize }
        impl Drop for Credit {
            fn drop(&mut self) { self.used.fetch_sub(self.bytes, Ordering::Relaxed); }
        }
        let mut seed = Vec::new();
        TensorProductPatch {
            control_points: [Point::ZERO; 16],
            colors: std::array::from_fn(|_| (0..9).map(|i| i as f32 / 10.).collect()),
        }.to_triangles(&mut seed);
        let mut a = seed[0].p0.clone();
        let mut b = a.clone();
        let mut c = a.clone();
        a.point = Point::new(0., 0.);
        b.point = Point::new(40., 0.);
        c.point = Point::new(0., 20.);
        let triangles = [Triangle::new(a, b, c)];
        let used = Arc::new(AtomicUsize::new(0));
        let calls = AtomicUsize::new(0);
        let result = sample_triangles_with_admission(&triangles, Affine::IDENTITY, None, &|| false, &|bytes| {
            calls.fetch_add(1, Ordering::Relaxed);
            used.fetch_add(bytes, Ordering::Relaxed);
            Some(Box::new(Credit { used: used.clone(), bytes }))
        }).unwrap();
        let color_bytes: usize = result.map.values().map(|sample| {
            assert_eq!(sample.colors.len(), 9);
            assert!(sample.colors.spilled());
            sample.colors.capacity() * size_of::<f32>()
        }).sum();
        assert!(used.load(Ordering::Relaxed) >= result.map.allocation_size() + color_bytes);
        let baseline = sample_triangles(&triangles, Affine::IDENTITY, None);
        assert_eq!(baseline.len(), result.len());
        for (key, actual) in &result.map {
            let expected = baseline.get(key).unwrap();
            assert_eq!(actual.coverage.to_bits(), expected.coverage.to_bits());
            for (a, b) in actual.colors.iter().zip(&expected.colors) {
                assert_eq!(a.to_bits(), b.to_bits());
            }
        }
        let count = calls.load(Ordering::Relaxed);
        assert!(count > 5);
        drop(result);
        assert_eq!(used.load(Ordering::Relaxed), 0);
        for stop in 1..=count {
            calls.store(0, Ordering::Relaxed);
            assert!(sample_triangles_with_admission(&triangles, Affine::IDENTITY, None, &|| false, &|bytes| {
                if calls.fetch_add(1, Ordering::Relaxed) + 1 == stop { return None; }
                used.fetch_add(bytes, Ordering::Relaxed);
                Some(Box::new(Credit { used: used.clone(), bytes }))
            }).is_none());
            assert_eq!(calls.load(Ordering::Relaxed), stop);
            assert_eq!(used.load(Ordering::Relaxed), 0);
        }
    }

    #[test]
    fn mesh_sampling_cancels_at_every_checkpoint_without_partial_result() {
        use std::cell::Cell;
        let mut seed = Vec::new();
        TensorProductPatch {
            control_points: [Point::ZERO; 16],
            colors: std::array::from_fn(|_| smallvec![0.1, 0.2, 0.3]),
        }.to_triangles(&mut seed);
        let mut a = seed[0].p0.clone();
        let mut b = a.clone();
        let mut c = a.clone();
        a.point = Point::new(0.0, 0.0);
        b.point = Point::new(140.0, 0.0);
        c.point = Point::new(0.0, 8.0);
        let triangles = [Triangle::new(a, b, c)];
        let checkpoints = Cell::new(0);
        let complete = sample_triangles_with_cancel(&triangles, Affine::IDENTITY, None, &|| {
            checkpoints.set(checkpoints.get() + 1);
            false
        }).unwrap();
        assert!(!complete.is_empty());
        let count = checkpoints.get();
        assert!(count > 30);
        for stop in 1..=count {
            let current = Cell::new(0);
            assert!(sample_triangles_with_cancel(&triangles, Affine::IDENTITY, None, &|| {
                current.set(current.get() + 1);
                current.get() == stop
            }).is_none(), "checkpoint {stop}/{count}");
            assert_eq!(current.get(), stop);
        }
    }

    #[test]
    fn fractional_texture_bounds_preserve_every_consumed_sample() {
        let mut seed = Vec::new();
        TensorProductPatch {
            control_points: [Point::ZERO; 16],
            colors: std::array::from_fn(|_| smallvec![0.0, 0.0, 0.0]),
        }.to_triangles(&mut seed);
        let template = seed[0].p0.clone();
        let vertex = |x, y, color| {
            let mut vertex = template.clone();
            vertex.point = Point::new(x, y);
            vertex.colors = smallvec![color, 1.0 - color, 0.25];
            vertex
        };
        let triangles = [
            Triangle::new(vertex(0., 0., 0.), vertex(50., 0., 1.), vertex(0., 50., 0.5)),
            Triangle::new(vertex(50., 0., 1.), vertex(50., 50., 0.), vertex(0., 50., 0.5)),
            Triangle::new(vertex(80., 80., 0.), vertex(100., 80., 1.), vertex(80., 100., 0.5)),
        ];
        for transform in [Affine::IDENTITY, Affine::new([1.1, 0.2, -0.1, 0.9, 0.3, 0.7])] {
            let complete = sample_triangles(&triangles, transform, None);
            for origin in [0.0, 0.01, 0.49, 0.51, 0.99] {
                for extent in [0.0_f64, 0.1, 1.0, 1.01, 9.75] {
                    let bounds = kurbo::Rect::new(3. + origin, 4. + origin, 3. + origin + extent, 4. + origin + extent);
                    let bounded = sample_triangles(&triangles, transform, Some(bounds));
                    assert!(bounded.len() < complete.len());
                    let size = extent.max(1.0).ceil() as usize;
                    for y in 0..size {
                        for x in 0..size {
                            let key = ((bounds.x0 + 0.5 + x as f64) as u16, (bounds.y0 + 0.5 + y as f64) as u16);
                            match (complete.get(&key), bounded.get(&key)) {
                                (Some(a), Some(b)) => {
                                    assert_eq!(a.coverage.to_bits(), b.coverage.to_bits());
                                    assert_eq!(a.colors.len(), b.colors.len());
                                    for (a, b) in a.colors.iter().zip(&b.colors) { assert_eq!(a.to_bits(), b.to_bits()); }
                                }
                                (None, None) => {}
                                _ => panic!("missing consumed sample {key:?}"),
                            }
                        }
                    }
                }
            }
        }
    }
}

fn encode_function_shading(domain: &[f32; 4], function: &ShadingFunction) -> EncodedShadingType {
    let domain = kurbo::Rect::new(
        domain[0] as f64,
        domain[2] as f64,
        domain[1] as f64,
        domain[3] as f64,
    );

    EncodedShadingType::FunctionBased {
        domain,
        function: function.clone(),
    }
}

#[derive(Debug)]
pub(crate) enum RadialAxialParams {
    Axial,
    Radial { p1: Point, r: Point },
}

#[derive(Debug)]
pub(crate) enum EncodedShadingType {
    FunctionBased {
        domain: kurbo::Rect,
        function: ShadingFunction,
    },
    RadialAxial {
        function: ShadingFunction,
        params: RadialAxialParams,
        domain: [f32; 2],
        extend: [bool; 2],
    },
    Sampled {
        samples: MeshSamples,
        function: Option<ShadingFunction>,
    },
    Dummy,
}

impl EncodedShadingType {
    pub(crate) fn eval(
        &self,
        pos: Point,
        bg_color: AlphaColor,
        color_space: &ColorSpace,
    ) -> Option<AlphaColor> {
        match self {
            Self::FunctionBased { domain, function } => {
                if !domain.contains(pos) {
                    Some(bg_color)
                } else {
                    let out = function.eval(&smallvec![pos.x as f32, pos.y as f32])?;
                    // TODO: Clamp out-of-range values.
                    Some(color_space.to_rgba(&out, 1.0, false))
                }
            }
            Self::RadialAxial {
                function,
                params,
                domain,
                extend,
            } => {
                let (t0, t1) = (domain[0], domain[1]);

                let mut t = match params {
                    RadialAxialParams::Axial => pos.x as f32,
                    RadialAxialParams::Radial { p1, r } => {
                        radial_pos(&pos, p1, *r, extend[0], extend[1]).unwrap_or(f32::MIN)
                    }
                };

                if t == f32::MIN {
                    return Some(bg_color);
                }

                if t < 0.0 {
                    if extend[0] {
                        t = 0.0;
                    } else {
                        return Some(bg_color);
                    }
                } else if t > 1.0 {
                    if extend[1] {
                        t = 1.0;
                    } else {
                        return Some(bg_color);
                    }
                }

                let t = t0 + (t1 - t0) * t;

                let val = function.eval(&smallvec![t])?;

                Some(color_space.to_rgba(&val, 1.0, false))
            }
            Self::Sampled { samples, function } => {
                let sample_point = (pos.x as u16, pos.y as u16);

                if let Some(sample) = samples.get(&sample_point) {
                    let color = &sample.colors;
                    let foreground = if let Some(function) = function {
                        let val = function.eval(&color.to_smallvec())?;
                        color_space.to_rgba(&val, sample.coverage, false)
                    } else {
                        color_space.to_rgba(color, sample.coverage, false)
                    };
                    let mut combined = foreground.premultiplied();
                    let background = bg_color.premultiplied();
                    for c in 0..4 {
                        combined[c] += background[c] * (1.0 - sample.coverage);
                    }
                    if combined[3] > 0.0 {
                        for c in 0..3 {
                            combined[c] /= combined[3];
                        }
                    }
                    Some(AlphaColor::new(combined))
                } else {
                    Some(bg_color)
                }
            }
            Self::Dummy => Some(AlphaColor::TRANSPARENT),
        }
    }
}

fn ts_from_line_to_line(src1: Point, src2: Point, dst1: Point, dst2: Point) -> Affine {
    let unit_to_line1 = unit_to_line(src1, src2);
    let line1_to_unit = unit_to_line1.inverse();
    let unit_to_line2 = unit_to_line(dst1, dst2);

    unit_to_line2 * line1_to_unit
}

fn unit_to_line(p0: Point, p1: Point) -> Affine {
    Affine::new([p1.y - p0.y, p0.x - p1.x, p1.x - p0.x, p1.y - p0.y, p0.x, p0.y])
}

fn radial_pos(pos: &Point, p1: &Point, r: Point, min_extend: bool, max_extend: bool) -> Option<f32> {
    let r0 = r.x as f32;
    let dx = p1.x as f32;
    let dy = p1.y as f32;
    let dr = r.y as f32 - r0;

    let px = pos.x as f32;
    let py = pos.y as f32;

    let a = dx * dx + dy * dy - dr * dr;
    let b = -2.0 * (px * dx + py * dy + r0 * dr);
    let c = px * px + py * py - r0 * r0;

    let discriminant = b * b - 4.0 * a * c;

    // No solution available.
    if discriminant < 0.0 {
        return None;
    }

    if a.abs() < 1e-6 {
        if b.abs() < 1e-6 {
            return None;
        }

        let t = -c / b;

        if (!min_extend && t < 0.0) || (!max_extend && t > 1.0) {
            return None;
        }

        let r_t = r0 + dr * t;
        if r_t < 0.0 {
            return None;
        }

        return Some(t);
    }

    let sqrt_d = discriminant.sqrt();
    let t1 = (-b - sqrt_d) / (2.0 * a);
    let t2 = (-b + sqrt_d) / (2.0 * a);

    let max = t1.max(t2);
    let mut take_max = Some(max);
    let min = t1.min(t2);
    let mut take_min = Some(min);

    if (!min_extend && min < 0.0) || r0 + dr * min < 0.0 {
        take_min = None;
    }

    if (!max_extend && max > 1.0) || r0 + dr * max < 0.0 {
        take_max = None;
    }

    match (take_min, take_max) {
        (Some(_), Some(max)) => Some(max),
        (Some(min), None) => Some(min),
        (None, Some(max)) => Some(max),
        (None, None) => None,
    }
}

#[cfg(test)]
mod ordered_grid;
