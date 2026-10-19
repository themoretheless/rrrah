//! Admitted sampled patch coverage with source-order ownership.
use super::{MeshMap, MeshSample, MeshSamples, OrderedPixel, PatchParameterIdentity, PixelSourceOwner};
use crate::shading::{
    AdaptivePatchError, AdaptivePatchLimits, AdaptivePatchRef, PatchTriangleSource, Triangle,
    tessellate_patch_mesh_stream_projected_admitted,
};
use kurbo::{Affine, Point, Rect};
type Credit = Box<dyn std::any::Any + Send + Sync>;

struct ParamPool {
    index: hashbrown::HashMap<u64, u16, rustc_hash::FxBuildHasher>,
    values: Vec<f64>,
    index_credit: Option<Credit>,
    values_credit: Option<Credit>,
}
impl ParamPool {
    fn new() -> Self {
        Self {
            index: Default::default(),
            values: Vec::new(),
            index_credit: None,
            values_credit: None,
        }
    }
    fn intern(&mut self, value: f64, admit: &dyn Fn(usize) -> Option<Credit>) -> Option<u16> {
        if let Some(index) = self.index.get(&value.to_bits()) {
            return Some(*index);
        }
        let index = u16::try_from(self.values.len()).ok()?;
        if self.index.len() == self.index.capacity() {
            let capacity = self.index.capacity().checked_mul(2)?.max(3);
            let buckets = if capacity < 4 {
                4
            } else if capacity < 8 {
                8
            } else {
                capacity.checked_mul(8)?.div_ceil(7).checked_next_power_of_two()?
            };
            let bytes = buckets
                .checked_mul(size_of::<(u64, u16)>() + 1)?
                .checked_add(128)?;
            let next = admit(bytes)?;
            self.index.try_reserve(capacity - self.index.len()).ok()?;
            if self.index.allocation_size() > bytes {
                return None;
            }
            self.index_credit = Some(next);
        }
        if self.values.len() == self.values.capacity() {
            let capacity = self.values.capacity().checked_mul(2)?.max(4);
            let next = admit(capacity.checked_mul(size_of::<f64>())?)?;
            self.values.try_reserve_exact(capacity - self.values.len()).ok()?;
            if self.values.capacity() > capacity {
                return None;
            }
            self.values_credit = Some(next);
        }
        self.values.push(value);
        self.index.insert(value.to_bits(), index);
        Some(index)
    }
}
struct Record {
    points: [Point; 3],
    parameters: [[u16; 2]; 3],
    patch_index: u32,
}
impl Record {
    fn source(&self, pool: &ParamPool) -> PatchTriangleSource {
        PatchTriangleSource {
            patch_index: self.patch_index as usize,
            parameters: self
                .parameters
                .map(|p| Point::new(pool.values[p[0] as usize], pool.values[p[1] as usize])),
        }
    }
    fn triangle(&self, patches: &[AdaptivePatchRef<'_>], pool: &ParamPool) -> Triangle {
        let colors = self
            .source(pool)
            .parameters
            .map(|uv| match patches[self.patch_index as usize] {
                AdaptivePatchRef::Tensor(p) => p.interpolate(uv),
                AdaptivePatchRef::Coons(p) => p.interpolate(uv),
            });
        Triangle::from_points_colors(self.points, colors)
    }
}

struct Block {
    records: Vec<Record>,
    _credit: Credit,
}

fn push_record(
    blocks: &mut Vec<Block>,
    outer_credit: &mut Option<Credit>,
    record: Record,
    admit: &dyn Fn(usize) -> Option<Credit>,
) -> Option<()> {
    if blocks
        .last()
        .is_none_or(|b| b.records.len() == b.records.capacity())
    {
        if blocks.len() == blocks.capacity() {
            let capacity = blocks.capacity().checked_mul(2)?.max(4);
            let next = admit(capacity.checked_mul(size_of::<Block>())?)?;
            blocks.try_reserve_exact(capacity - blocks.len()).ok()?;
            if blocks.capacity() > capacity {
                return None;
            }
            *outer_credit = Some(next);
        }
        let credit = admit(128usize.checked_mul(size_of::<Record>())?)?;
        let mut records = Vec::new();
        records.try_reserve_exact(128).ok()?;
        if records.capacity() > 128 {
            return None;
        }
        blocks.push(Block {
            records,
            _credit: credit,
        });
    }
    blocks.last_mut()?.records.push(record);
    Some(())
}

pub(super) fn sample_patches(
    patches: &[AdaptivePatchRef<'_>],
    transform: Affine,
    components: usize,
    bounds: Option<Rect>,
    cancelled: &dyn Fn() -> bool,
    admit: &dyn Fn(usize) -> Option<Credit>,
) -> Option<MeshSamples> {
    if cancelled() || !(1..=4).contains(&components) {
        return None;
    }
    let _identities = admit(
        patches
            .len()
            .checked_mul(size_of::<Option<PatchParameterIdentity>>())?,
    )?;
    let mut identities = Vec::new();
    identities.try_reserve_exact(patches.len()).ok()?;
    if identities.capacity() > patches.len() {
        return None;
    }
    for (index, patch) in patches.iter().enumerate() {
        if cancelled() {
            return None;
        }
        let identity = match patch {
            AdaptivePatchRef::Tensor(p) => PatchParameterIdentity::tensor(p, index, transform),
            AdaptivePatchRef::Coons(p) => {
                PatchParameterIdentity::tensor(&p.adaptive_tensor().ok()?, index, transform)
            }
        };
        identities.push(identity);
    }
    // Store compact provenance once; process ownership in 4x4 tiles so its
    // retained footprint is independent of page area. Credits outlive storage.
    let mut parameters_pool = ParamPool::new();
    let mut record_credit = None;
    let mut records: Vec<Block> = Vec::new();
    let mut extent = Rect::new(f64::INFINITY, f64::INFINITY, f64::NEG_INFINITY, f64::NEG_INFINITY);
    let clip = bounds
        .filter(|r| {
            [r.x0, r.y0, r.x1, r.y1].iter().all(|v| v.is_finite()) && r.width() >= 0. && r.height() >= 0.
        })
        .map(|r| r.inflate(2., 2.));
    tessellate_patch_mesh_stream_projected_admitted(
        patches,
        transform,
        AdaptivePatchLimits {
            pixel_error: 1. / 64.,
            component_error: 1. / 1024.,
            max_depth: 12,
            max_triangles: 1_048_576,
        },
        cancelled,
        admit,
        &mut |triangle, source| {
            if cancelled() {
                return Err(AdaptivePatchError::Cancelled);
            }
            let bbox = triangle.bounding_box();
            let bbox = clip.map_or(bbox, |clip| bbox.intersect(clip));
            if bbox.width() <= 0. || bbox.height() <= 0. {
                return Ok(());
            }
            extent = Rect::new(
                extent.x0.min(bbox.x0),
                extent.y0.min(bbox.y0),
                extent.x1.max(bbox.x1),
                extent.y1.max(bbox.y1),
            );
            let mut parameters = [[0u16; 2]; 3];
            for (i, uv) in source.parameters.iter().enumerate() {
                for (j, value) in [uv.x, uv.y].into_iter().enumerate() {
                    parameters[i][j] = parameters_pool
                        .intern(value, admit)
                        .ok_or(AdaptivePatchError::Admission)?;
                }
            }
            let record = Record {
                points: [triangle.p0.point, triangle.p1.point, triangle.p2.point],
                parameters,
                patch_index: u32::try_from(source.patch_index).map_err(|_| AdaptivePatchError::Invalid)?,
            };
            // Geometry and interned UV values stay bit-exact,
            // and component reconstruction must match the checked stream.
            let rebuilt = record.triangle(patches, &parameters_pool);
            for (a, b) in [&triangle.p0, &triangle.p1, &triangle.p2].into_iter().zip([
                &rebuilt.p0,
                &rebuilt.p1,
                &rebuilt.p2,
            ]) {
                if a.colors
                    .iter()
                    .zip(&b.colors)
                    .any(|(a, b)| a.to_bits() != b.to_bits())
                {
                    return Err(AdaptivePatchError::Invalid);
                }
            }
            push_record(&mut records, &mut record_credit, record, admit)
                .ok_or(AdaptivePatchError::Admission)?;
            Ok(())
        },
    )
    .ok()?;
    let mut guard = None;
    let mut map = MeshMap::default();
    let mut work = 100_000_000usize;
    let x0 = extent.x0.floor() as u16;
    let x1 = extent.x1.ceil() as u16;
    let y0 = extent.y0.floor() as u16;
    let y1 = extent.y1.ceil() as u16;
    for ty in (y0..y1).step_by(4) {
        for tx in (x0..x1).step_by(4) {
            if cancelled() {
                return None;
            }
            let mut pixels: [OrderedPixel<'_>; 16] = std::array::from_fn(|_| OrderedPixel::new(admit, 4096));
            let tile = Rect::new(
                tx as f64,
                ty as f64,
                (u32::from(tx) + 4).min(u32::from(x1)) as f64,
                (u32::from(ty) + 4).min(u32::from(y1)) as f64,
            );
            for record in records.iter().flat_map(|block| &block.records) {
                work = work.checked_sub(1)?;
                if cancelled() {
                    return None;
                }
                let points = record.points;
                let bbox = Rect::from_points(points[0], points[1])
                    .union_pt(points[2])
                    .intersect(tile);
                if bbox.width() <= 0. || bbox.height() <= 0. {
                    continue;
                }
                let triangle = record.triangle(patches, &parameters_pool);
                for y in (bbox.y0.floor() as u16)..(bbox.y1.ceil() as u16) {
                    for x in (bbox.x0.floor() as u16)..(bbox.x1.ceil() as u16) {
                        let origin = Point::new(x as f64, y as f64);
                        let mut owner = PixelSourceOwner::from_triangle(
                            &triangle,
                            record.source(&parameters_pool),
                            origin,
                        )
                        .ok()?;
                        if let Some(identity) = identities[record.patch_index as usize] {
                            identity.apply(&mut owner, origin).ok()?;
                        }
                        let local = points.map(|p| Point::new(p.x - origin.x, p.y - origin.y));
                        pixels[usize::from(y - ty) * 4 + usize::from(x - tx)]
                            .paint_triangle(local, owner, &mut work, cancelled)
                            .ok()?;
                    }
                }
            }
            for y in ty..y1.min(ty.saturating_add(4)) {
                for x in tx..x1.min(tx.saturating_add(4)) {
                    let pixel = &pixels[usize::from(y - ty) * 4 + usize::from(x - tx)];
                    let mut sum = [0.; 4];
                    let mut coverage = 0.;
                    pixel
                        .visit(&mut work, cancelled, &mut |area, center, _, owner| {
                            let values = owner
                                .evaluate(center)
                                .ok_or(super::PixelPartitionError::Invalid)?;
                            for i in 0..components {
                                sum[i] += area * values[i];
                            }
                            coverage += area;
                            Ok(())
                        })
                        .ok()?;
                    if coverage <= 0. {
                        continue;
                    }
                    if !coverage.is_finite() || coverage > 1. + 1e-10 {
                        return None;
                    }
                    if map.len() == map.capacity() {
                        let capacity = map.capacity().checked_mul(2)?.max(3);
                        let bytes = super::mesh_table_bound(capacity, components)?;
                        let next = admit(bytes)?;
                        map.try_reserve(capacity - map.len()).ok()?;
                        if map.allocation_size() > bytes {
                            return None;
                        }
                        guard = Some(next);
                    }
                    map.insert(
                        (x, y),
                        MeshSample {
                            colors: sum[..components].iter().map(|v| (v / coverage) as f32).collect(),
                            coverage: coverage.min(1.) as f32,
                        },
                    );
                }
            }
        }
    }
    drop(records);
    drop(record_credit);
    if cancelled() {
        return None;
    }
    Some(MeshSamples { map, _guard: guard })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::shading::TensorProductPatch;
    use std::cell::Cell;
    use std::sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    };

    struct Reservation(Arc<AtomicUsize>, usize);
    impl Drop for Reservation {
        fn drop(&mut self) {
            self.0.fetch_sub(self.1, Ordering::SeqCst);
        }
    }
    fn source(x0: f64, x1: f64, color: [f32; 3]) -> TensorProductPatch {
        let indices = [[0, 1, 2, 3], [11, 12, 13, 4], [10, 15, 14, 5], [9, 8, 7, 6]];
        let mut control_points = [Point::ZERO; 16];
        for i in 0..4 {
            for j in 0..4 {
                control_points[indices[i][j]] = Point::new(x0 + (x1 - x0) * i as f64 / 3., j as f64 / 3.);
            }
        }
        TensorProductPatch {
            control_points,
            colors: std::array::from_fn(|_| color.into_iter().collect()),
        }
    }
    #[test]
    fn interned_parameters_and_segment_growth_preserve_bits_and_release_credit() {
        let used = Arc::new(AtomicUsize::new(0));
        let calls = Cell::new(0);
        let fail = Cell::new(0);
        let admit = |bytes| {
            calls.set(calls.get() + 1);
            if calls.get() == fail.get() {
                return None;
            }
            used.fetch_add(bytes, Ordering::SeqCst);
            Some(Box::new(Reservation(used.clone(), bytes)) as Credit)
        };
        let run = || -> Option<()> {
            let mut pool = ParamPool::new();
            for value in [-0., 0., f64::from_bits(1), f64::MAX]
                .into_iter()
                .chain((0..257).map(|i| i as f64 / 3.))
            {
                let first = pool.intern(value, &admit)?;
                assert_eq!(pool.values[first as usize].to_bits(), value.to_bits());
                assert_eq!(pool.intern(value, &admit), Some(first));
            }
            let mut credit = None;
            let mut blocks = Vec::new();
            for i in 0..641 {
                let record = Record {
                    points: [Point::new(i as f64, 0.); 3],
                    parameters: [[0, 1]; 3],
                    patch_index: 0,
                };
                push_record(&mut blocks, &mut credit, record, &admit)?;
            }
            assert!(blocks.len() > 4);
            assert_eq!(blocks.iter().map(|b| b.records.len()).sum::<usize>(), 641);
            for (i, record) in blocks.iter().flat_map(|b| &b.records).enumerate() {
                assert_eq!(record.points[0].x, i as f64);
            }
            drop(blocks);
            drop(credit);
            Some(())
        };
        run().unwrap();
        let count = calls.get();
        assert_eq!(used.load(Ordering::SeqCst), 0);
        for stop in 1..=count {
            calls.set(0);
            fail.set(stop);
            assert!(run().is_none(), "allocation {stop}/{count}");
            assert_eq!(used.load(Ordering::SeqCst), 0);
        }
        fail.set(0);
        run().unwrap();
        assert_eq!(used.load(Ordering::SeqCst), 0);
    }
    #[test]
    fn ordered_samples_admission_and_cancellation_are_atomic_and_retryable() {
        let patches = [source(0., 1.5, [1., 0., 0.]), source(0.5, 2., [0., 0., 1.])];
        let refs = patches.iter().map(AdaptivePatchRef::Tensor).collect::<Vec<_>>();
        let used = Arc::new(AtomicUsize::new(0));
        let calls = Cell::new(0);
        let fail = Cell::new(0);
        let admit = |bytes| {
            calls.set(calls.get() + 1);
            if calls.get() == fail.get() {
                return None;
            }
            used.fetch_add(bytes, Ordering::SeqCst);
            Some(Box::new(Reservation(used.clone(), bytes)) as Credit)
        };
        let run = |cancelled: &dyn Fn() -> bool| {
            sample_patches(&refs, Affine::IDENTITY, 3, None, cancelled, &admit)
        };
        let samples = run(&|| false).unwrap();
        let rgba8 = |key| {
            samples[&key]
                .colors
                .iter()
                .map(|v| (v * 255.).round() as u8)
                .collect::<Vec<_>>()
        };
        assert_eq!(rgba8((0, 0)), [128, 0, 128]);
        assert_eq!(rgba8((1, 0)), [0, 0, 255]);
        assert_eq!(samples[&(0, 0)].coverage, 1.);
        let count = calls.get();
        drop(samples);
        assert_eq!(used.load(Ordering::SeqCst), 0);
        for stop in 1..=count {
            calls.set(0);
            fail.set(stop);
            assert!(run(&|| false).is_none(), "allocation {stop}/{count}");
            assert_eq!(used.load(Ordering::SeqCst), 0);
        }
        fail.set(0);
        let checks = Cell::new(0);
        drop(
            run(&|| {
                checks.set(checks.get() + 1);
                false
            })
            .unwrap(),
        );
        for stop in 1..=checks.get() {
            let at = Cell::new(0);
            assert!(
                run(&|| {
                    at.set(at.get() + 1);
                    at.get() == stop
                })
                .is_none(),
                "checkpoint {stop}"
            );
            assert_eq!(used.load(Ordering::SeqCst), 0);
        }
        drop(run(&|| false).unwrap());
        assert_eq!(used.load(Ordering::SeqCst), 0);
    }
}
