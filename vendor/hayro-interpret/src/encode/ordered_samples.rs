//! Admitted sampled patch coverage with source-order ownership.
use super::{MeshMap, MeshSample, MeshSamples, OrderedPixel, PatchParameterIdentity, PixelSourceOwner};
use crate::shading::{
    AdaptivePatchError, AdaptivePatchLimits, AdaptivePatchRef, PatchTriangleSource, Triangle,
    tessellate_patch_mesh_stream_projected_admitted,
};
use kurbo::{Affine, Point, Rect};
type Credit = Box<dyn std::any::Any + Send + Sync>;
// Split dense cells before quadratic partition work consumes the shared quota.
const WORKING_FRAGMENTS: usize = 512;
const MAX_LEAF_FRAGMENTS: usize = 4096;

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
    bounds: Rect,
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
            bounds: Rect::ZERO,
            records,
            _credit: credit,
        });
    }
    let block = blocks.last_mut()?;
    let bounds = Rect::from_points(record.points[0], record.points[1]).union_pt(record.points[2]);
    block.bounds = if block.records.is_empty() {
        bounds
    } else {
        block.bounds.union(bounds)
    };
    block.records.push(record);
    Some(())
}

fn add_integral(sum: &mut [f64; 5], correction: &mut [f64; 5], value: [f64; 5]) {
    for k in 0..5 {
        let y = value[k] - correction[k];
        let next = sum[k] + y;
        correction[k] = (next - sum[k]) - y;
        sum[k] = next;
    }
}
fn integrate_children(
    records: &[Block],
    patches: &[AdaptivePatchRef<'_>],
    pool: &ParamPool,
    identities: &[Option<PatchParameterIdentity>],
    origin: Point,
    side: f64,
    depth: u8,
    fragment_limit: usize,
    work: &mut usize,
    cancelled: &dyn Fn() -> bool,
    admit: &dyn Fn(usize) -> Option<Credit>,
) -> Option<[f64; 5]> {
    if depth >= 8 || cancelled() {
        if std::env::var_os("RRRAH_TRACE_NATIVE_REPLAY_EVENT").is_some() {
            eprintln!("DENSE_CELL_DEPTH origin={origin:?} side={side} depth={depth} work={work}");
        }
        return None;
    }
    let half = side * 0.5;
    let mut sum = [0.; 5];
    let mut correction = [0.; 5];
    for (x, y) in [(0., 0.), (half, 0.), (0., half), (half, half)] {
        let value = integrate_cell(
            records,
            patches,
            pool,
            identities,
            Point::new(origin.x + x, origin.y + y),
            half,
            depth + 1,
            fragment_limit,
            work,
            cancelled,
            admit,
        )?;
        add_integral(&mut sum, &mut correction, value);
    }
    Some(sum)
}
#[derive(Clone, Copy)]
struct Candidate<'a> {
    record: &'a Record,
    rank: [f64; 2],
    ordinal: usize,
}
fn rank_at_center(field: [f64; 3]) -> f64 {
    let value = field[1].mul_add(0.5, field[2].mul_add(0.5, field[0]));
    if value == 0. { 0. } else { value }
}
impl Candidate<'_> {
    fn compare(&self, other: &Self) -> std::cmp::Ordering {
        other
            .record
            .patch_index
            .cmp(&self.record.patch_index)
            .then_with(|| other.rank[0].total_cmp(&self.rank[0]))
            .then_with(|| other.rank[1].total_cmp(&self.rank[1]))
            .then_with(|| self.ordinal.cmp(&other.ordinal))
    }
}
// Allocation-free, fallible heapsort. Each comparison debits the caller's
// continuing work allowance and observes cancellation before doing more work.
fn sort_candidates(
    values: &mut [Candidate<'_>],
    work: &mut usize,
    cancelled: &dyn Fn() -> bool,
) -> Option<()> {
    fn compare(
        a: &Candidate<'_>,
        b: &Candidate<'_>,
        work: &mut usize,
        cancelled: &dyn Fn() -> bool,
    ) -> Option<std::cmp::Ordering> {
        if cancelled() {
            return None;
        }
        *work = work.checked_sub(1)?;
        Some(a.compare(b))
    }
    fn sift(
        values: &mut [Candidate<'_>],
        mut root: usize,
        end: usize,
        work: &mut usize,
        cancelled: &dyn Fn() -> bool,
    ) -> Option<()> {
        while root * 2 + 1 < end {
            let mut child = root * 2 + 1;
            if child + 1 < end && compare(&values[child], &values[child + 1], work, cancelled)?.is_lt() {
                child += 1;
            }
            if !compare(&values[root], &values[child], work, cancelled)?.is_lt() {
                break;
            }
            values.swap(root, child);
            root = child;
        }
        Some(())
    }
    if cancelled() {
        return None;
    }
    *work = work.checked_sub(1)?;
    for start in (0..values.len() / 2).rev() {
        sift(values, start, values.len(), work, cancelled)?;
    }
    for end in (1..values.len()).rev() {
        values.swap(0, end);
        sift(values, 0, end, work, cancelled)?;
    }
    Some(())
}
fn cell_owner(
    record: &Record,
    patches: &[AdaptivePatchRef<'_>],
    pool: &ParamPool,
    identities: &[Option<PatchParameterIdentity>],
    origin: Point,
    side: f64,
) -> Option<PixelSourceOwner> {
    let triangle = record.triangle(patches, pool);
    let mut owner = PixelSourceOwner::from_triangle(&triangle, record.source(pool), origin).ok()?;
    if let Some(identity) = identities[record.patch_index as usize] {
        identity.apply(&mut owner, origin).ok()?;
    }
    for f in [&mut owner.u, &mut owner.v]
        .into_iter()
        .chain(owner.components.iter_mut())
    {
        f[1] *= side;
        f[2] *= side;
    }
    Some(owner)
}

// Opt-in failure diagnostics only; no region is discarded or approximated.
fn trace_dense_regions(
    pixel: &OrderedPixel<'_>,
    origin: Point,
    side: f64,
    candidates: usize,
    processed: usize,
) {
    let mut minimum = f64::INFINITY;
    let mut maximum = 0f64;
    let mut area = 0.;
    let mut bins = [0usize; 4];
    let mut first_patch = usize::MAX;
    let mut last_patch = 0usize;
    let status = pixel.visit(&mut 100_000, &|| false, &mut |weight, _, _, owner| {
        minimum = minimum.min(weight);
        maximum = maximum.max(weight);
        area += weight;
        let bucket = if weight < 1e-12 {
            0
        } else if weight < 1e-9 {
            1
        } else if weight < 1e-6 {
            2
        } else {
            3
        };
        bins[bucket] += 1;
        first_patch = first_patch.min(owner.patch_index);
        last_patch = last_patch.max(owner.patch_index);
        Ok(())
    });
    eprintln!(
        "DENSE_REGION_STATS origin={origin:?} side={side} candidates={candidates} processed={processed} fragments={} patch_range={first_patch}..={last_patch} min_area={minimum} max_area={maximum} sum_area={area} area_bins={bins:?} status={status:?}",
        pixel.fragments()
    );
}

// Integrals are in original device-pixel area. Each local partition owns the
// same source priorities; only its coordinate frame and clipping square change.
fn integrate_cell(
    records: &[Block],
    patches: &[AdaptivePatchRef<'_>],
    pool: &ParamPool,
    identities: &[Option<PatchParameterIdentity>],
    origin: Point,
    side: f64,
    depth: u8,
    fragment_limit: usize,
    work: &mut usize,
    cancelled: &dyn Fn() -> bool,
    admit: &dyn Fn(usize) -> Option<Credit>,
) -> Option<[f64; 5]> {
    let mut pixel = OrderedPixel::new(admit, fragment_limit);
    let rect = Rect::new(origin.x, origin.y, origin.x + side, origin.y + side);
    let mut candidate_credit: Option<Credit> = None;
    let mut candidates: Vec<Candidate<'_>> = Vec::new();
    for block in records {
        if cancelled() {
            return None;
        }
        *work = work.checked_sub(1)?;
        let overlap = block.bounds.intersect(rect);
        if overlap.width() <= 0. || overlap.height() <= 0. {
            continue;
        }
        for record in &block.records {
            if cancelled() {
                return None;
            }
            *work = work.checked_sub(1)?;
            let points = record.points;
            let bbox = Rect::from_points(points[0], points[1])
                .union_pt(points[2])
                .intersect(rect);
            if bbox.width() <= 0. || bbox.height() <= 0. {
                continue;
            }
            let owner = cell_owner(record, patches, pool, identities, origin, side)?;
            let rank = [rank_at_center(owner.v), rank_at_center(owner.u)];
            if candidates.len() == candidates.capacity() {
                let capacity = candidates.capacity().checked_mul(2)?.max(8);
                let next = admit(capacity.checked_mul(size_of::<Candidate<'_>>())?)?;
                candidates.try_reserve_exact(capacity - candidates.len()).ok()?;
                if candidates.capacity() > capacity {
                    return None;
                }
                candidate_credit = Some(next);
            }
            let ordinal = candidates.len();
            candidates.push(Candidate {
                record,
                rank,
                ordinal,
            });
        }
    }
    sort_candidates(&mut candidates, work, cancelled)?;
    // The center key orders work only. Exact UV comparisons still select the
    // owner at each overlap. Identical UV fields share a key, with original
    // ordinal preserving the existing tie behavior (including signed zero).
    for index in 0..candidates.len() {
        if cancelled() {
            return None;
        }
        *work = work.checked_sub(1)?;
        let record = candidates[index].record;
        let points = record.points;
        let owner = cell_owner(record, patches, pool, identities, origin, side)?;
        let local = points.map(|p| Point::new((p.x - origin.x) / side, (p.y - origin.y) / side));
        match pixel.paint_triangle(local, owner, work, cancelled) {
            Ok(()) => {}
            Err(super::PixelPartitionError::Limit) if *work > 0 => {
                if depth >= 8
                    && fragment_limit == MAX_LEAF_FRAGMENTS
                    && std::env::var_os("RRRAH_TRACE_NATIVE_REPLAY_EVENT").is_some()
                {
                    trace_dense_regions(&pixel, origin, side, candidates.len(), index);
                }
                drop(pixel);
                drop(candidates);
                drop(candidate_credit);
                // Preserve the original 4096-region capacity at the deepest
                // cell, after cheap subdivisions have reduced its support.
                if depth >= 8 && fragment_limit < MAX_LEAF_FRAGMENTS {
                    return integrate_cell(
                        records,
                        patches,
                        pool,
                        identities,
                        origin,
                        side,
                        depth,
                        MAX_LEAF_FRAGMENTS,
                        work,
                        cancelled,
                        admit,
                    );
                }
                return integrate_children(
                    records,
                    patches,
                    pool,
                    identities,
                    origin,
                    side,
                    depth,
                    fragment_limit,
                    work,
                    cancelled,
                    admit,
                );
            }
            Err(error) => {
                if std::env::var_os("RRRAH_TRACE_NATIVE_REPLAY_EVENT").is_some() {
                    if depth >= 8 || *work == 0 {
                        trace_dense_regions(&pixel, origin, side, candidates.len(), index);
                    }
                    eprintln!("DENSE_CELL_PAINT origin={origin:?} side={side} refusal={error:?} work={work}");
                }
                return None;
            }
        }
    }
    drop(candidates);
    drop(candidate_credit);
    let mut sum = [0.; 5];
    let mut correction = [0.; 5];
    pixel
        .visit(work, cancelled, &mut |area, center, _, owner| {
            let values = owner
                .evaluate(center)
                .ok_or(super::PixelPartitionError::Invalid)?;
            let area = area * side * side;
            add_integral(
                &mut sum,
                &mut correction,
                [
                    area,
                    area * values[0],
                    area * values[1],
                    area * values[2],
                    area * values[3],
                ],
            );
            Ok(())
        })
        .inspect_err(|error| {
            if std::env::var_os("RRRAH_TRACE_NATIVE_REPLAY_EVENT").is_some() {
                eprintln!("DENSE_CELL_VISIT origin={origin:?} side={side} refusal={error:?} work={work}");
            }
        })
        .ok()?;
    Some(sum)
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
            AdaptivePatchRef::Coons(p) => PatchParameterIdentity::coons(p, index, transform),
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
            max_depth: 16,
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
    .inspect_err(|error| {
        if std::env::var_os("RRRAH_TRACE_NATIVE_REPLAY_EVENT").is_some() {
            eprintln!("ORDERED_SAMPLE_TESSELLATION refusal={error:?}");
        }
    })
    .ok()?;
    let mut guard = None;
    let mut map = MeshMap::default();
    let mut work = 100_000_000usize;
    // The closure keeps every budget-phase return observable in opt-in traces.
    let computed = (|| -> Option<()> {
        let x0 = extent.x0.floor() as u16;
        let x1 = extent.x1.ceil() as u16;
        let y0 = extent.y0.floor() as u16;
        let y1 = extent.y1.ceil() as u16;
        for ty in (y0..y1).step_by(4) {
            for tx in (x0..x1).step_by(4) {
                if cancelled() {
                    return None;
                }
                let mut pixels: [OrderedPixel<'_>; 16] =
                    std::array::from_fn(|_| OrderedPixel::new(admit, WORKING_FRAGMENTS));
                let mut dense = [false; 16];
                let tile = Rect::new(
                    tx as f64,
                    ty as f64,
                    (u32::from(tx) + 4).min(u32::from(x1)) as f64,
                    (u32::from(ty) + 4).min(u32::from(y1)) as f64,
                );
                for block in &records {
                    if cancelled() {
                        return None;
                    }
                    work = work.checked_sub(1)?;
                    let overlap = block.bounds.intersect(tile);
                    if overlap.width() <= 0. || overlap.height() <= 0. {
                        continue;
                    }
                    for record in &block.records {
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
                                let slot = usize::from(y - ty) * 4 + usize::from(x - tx);
                                if dense[slot] {
                                    continue;
                                }
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
                                match pixels[slot].paint_triangle(local, owner, &mut work, cancelled) {
                                    Ok(()) => {}
                                    Err(super::PixelPartitionError::Limit) if work > 0 => {
                                        pixels[slot] = OrderedPixel::new(admit, WORKING_FRAGMENTS);
                                        dense[slot] = true;
                                        if std::env::var_os("RRRAH_TRACE_NATIVE_REPLAY_EVENT").is_some() {
                                            eprintln!("ORDERED_SAMPLE_SUBDIVIDE pixel={x},{y} work={work}");
                                        }
                                    }
                                    Err(_) => return None,
                                }
                            }
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
                        if dense[usize::from(y - ty) * 4 + usize::from(x - tx)] {
                            let value = integrate_children(
                                &records,
                                patches,
                                &parameters_pool,
                                &identities,
                                Point::new(x as f64, y as f64),
                                1.,
                                0,
                                WORKING_FRAGMENTS,
                                &mut work,
                                cancelled,
                                admit,
                            )?;
                            coverage = value[0];
                            sum.copy_from_slice(&value[1..]);
                        }
                        if coverage <= 0. {
                            continue;
                        }
                        if !coverage.is_finite() || coverage > 1. + 1e-10 {
                            if std::env::var_os("RRRAH_TRACE_NATIVE_REPLAY_EVENT").is_some() {
                                eprintln!("ORDERED_COVERAGE_REFUSAL pixel={x},{y} coverage={coverage}");
                            }
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
        Some(())
    })();
    // Keep storage and credits in their original ownership scope. The tracing
    // closure borrows them and cannot reorder their destruction on early exits.
    drop(records);
    drop(record_credit);
    let complete = computed.is_some() && !cancelled();
    if std::env::var_os("RRRAH_TRACE_NATIVE_REPLAY_EVENT").is_some() {
        eprintln!(
            "ORDERED_SAMPLE_OUTCOME patches={} complete={} work_spent={} work_left={work}",
            patches.len(),
            complete,
            100_000_000 - work
        );
    }
    if complete {
        Some(MeshSamples { map, _guard: guard })
    } else {
        None
    }
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
    fn ranked_candidates_match_total_order_preserve_ties_and_cancel_every_comparison() {
        assert_eq!(rank_at_center([-0., -0., -0.]).to_bits(), 0f64.to_bits());
        assert_eq!(rank_at_center([0., 0., 0.]).to_bits(), 0f64.to_bits());
        let records = (0..96)
            .map(|i| Record {
                points: [Point::ZERO; 3],
                parameters: [[0; 2]; 3],
                patch_index: (i % 2) as u32,
            })
            .collect::<Vec<_>>();
        let make = || {
            let mut candidates = records
                .iter()
                .enumerate()
                .map(|(i, record)| Candidate {
                    record,
                    rank: [(i % 3) as f64, (i % 5) as f64],
                    ordinal: i,
                })
                .collect::<Vec<_>>();
            candidates.reverse();
            candidates
        };
        let mut expected = make();
        expected.sort_by(Candidate::compare);
        let expected = expected.iter().map(|c| c.ordinal).collect::<Vec<_>>();
        let checks = Cell::new(0usize);
        let mut candidates = make();
        sort_candidates(&mut candidates, &mut 10000, &|| {
            checks.set(checks.get() + 1);
            false
        })
        .unwrap();
        assert_eq!(candidates.iter().map(|c| c.ordinal).collect::<Vec<_>>(), expected);
        for stop in 1..=checks.get() {
            let at = Cell::new(0usize);
            let mut candidates = make();
            let mut work = 10000;
            assert!(
                sort_candidates(&mut candidates, &mut work, &|| {
                    at.set(at.get() + 1);
                    at.get() == stop
                })
                .is_none()
            );
            assert!(work <= 10000);
        }
        assert!(sort_candidates(&mut make(), &mut 0, &|| false).is_none());
        let mut retry = make();
        sort_candidates(&mut retry, &mut 10000, &|| false).unwrap();
        assert_eq!(retry.iter().map(|c| c.ordinal).collect::<Vec<_>>(), expected);
    }
    #[test]
    fn ranked_cell_keeps_crossing_uv_selection_exact() {
        let admit = |_| Some(Box::new(()) as Credit);
        let mut patch = source(0., 1., [0.; 3]);
        patch.colors =
            [[1., 0., 0.], [1., 0., 0.], [0., 0., 1.], [0., 0., 1.]].map(|c| c.into_iter().collect());
        let refs = [AdaptivePatchRef::Tensor(&patch)];
        let mut pool = ParamPool::new();
        let mut records = Vec::new();
        let mut credit = None;
        let points = [Point::ZERO, Point::new(1., 0.), Point::new(0., 1.)];
        // Two retained source fields over one projected triangle: v=x,u=y/4
        // and v=1-x,u=1-y/4. The center keys favor the second, but v crosses
        // at x=1/2. Exact rational region integrals are red=5/32, blue=11/32.
        for uv in [[[0., 0.], [0., 1.], [0.25, 0.]], [[1., 1.], [1., 0.], [0.75, 1.]]] {
            let mut parameters = [[0; 2]; 3];
            for i in 0..3 {
                parameters[i] = [
                    pool.intern(uv[i][0], &admit).unwrap(),
                    pool.intern(uv[i][1], &admit).unwrap(),
                ];
            }
            push_record(
                &mut records,
                &mut credit,
                Record {
                    points,
                    parameters,
                    patch_index: 0,
                },
                &admit,
            )
            .unwrap();
        }
        let value = integrate_cell(
            &records,
            &refs,
            &pool,
            &[None],
            Point::ZERO,
            1.,
            0,
            32,
            &mut 100_000,
            &|| false,
            &admit,
        )
        .unwrap();
        for (a, b) in value.into_iter().zip([0.5, 5. / 32., 0., 11. / 32., 0.]) {
            assert!((a - b).abs() < 1e-12, "{value:?}");
        }
    }
    #[test]
    fn ranked_dense_occlusion_preserves_integrals_with_less_shared_work() {
        let used = Arc::new(AtomicUsize::new(0));
        let admit = |bytes: usize| -> Option<Credit> {
            let next = used.load(Ordering::SeqCst).checked_add(bytes)?;
            if next > 8 * 1024 * 1024 {
                return None;
            }
            used.store(next, Ordering::SeqCst);
            Some(Box::new(Reservation(used.clone(), bytes)))
        };
        let mut fine = source(0., 1., [0.; 3]);
        fine.colors =
            [[0., 0., 0.], [0., 1., 0.], [1., 1., 1.], [1., 0., 0.]].map(|c| c.into_iter().collect());
        let cover = source(0., 1., [0.25; 3]);
        let refs = [AdaptivePatchRef::Tensor(&fine), AdaptivePatchRef::Tensor(&cover)];
        let mut pool = ParamPool::new();
        let mut records = Vec::new();
        let mut credit = None;
        let mut append = |points: [Point; 3], patch_index: u32| {
            let mut parameters = [[0; 2]; 3];
            for (i, p) in points.iter().enumerate() {
                parameters[i] = [
                    pool.intern(p.x, &admit).unwrap(),
                    pool.intern(p.y, &admit).unwrap(),
                ];
            }
            push_record(
                &mut records,
                &mut credit,
                Record {
                    points,
                    parameters,
                    patch_index,
                },
                &admit,
            )
            .unwrap();
        };
        for y in 0..16 {
            for x in 0..32 {
                let [a, b, c, d] = [(x, y), (x + 1, y), (x, y + 1), (x + 1, y + 1)]
                    .map(|(x, y)| Point::new(x as f64 / 32., y as f64 / 16.));
                append([a, b, c], 0);
                append([b, d, c], 0);
            }
        }
        append([Point::ZERO, Point::new(1., 0.), Point::new(0., 1.)], 1);
        append([Point::new(1., 0.), Point::new(1., 1.), Point::new(0., 1.)], 1);
        let baseline_bytes = used.load(Ordering::SeqCst);
        let mut baseline_work = 100_000_000usize;
        let mut pixel = OrderedPixel::new(&admit, 2048);
        for block in &records {
            baseline_work = baseline_work.checked_sub(1).unwrap();
            for record in &block.records {
                baseline_work = baseline_work.checked_sub(1).unwrap();
                let owner = cell_owner(record, &refs, &pool, &[None, None], Point::ZERO, 1.).unwrap();
                pixel
                    .paint_triangle(record.points, owner, &mut baseline_work, &|| false)
                    .unwrap();
            }
        }
        let mut baseline = [0.; 5];
        let mut correction = [0.; 5];
        pixel
            .visit(&mut baseline_work, &|| false, &mut |area, center, _, owner| {
                let v = owner.evaluate(center).unwrap();
                add_integral(
                    &mut baseline,
                    &mut correction,
                    [area, area * v[0], area * v[1], area * v[2], area * v[3]],
                );
                Ok(())
            })
            .unwrap();
        drop(pixel);
        assert_eq!(used.load(Ordering::SeqCst), baseline_bytes);
        let mut ranked_work = 100_000_000;
        let ranked = integrate_cell(
            &records,
            &refs,
            &pool,
            &[None, None],
            Point::ZERO,
            1.,
            0,
            2048,
            &mut ranked_work,
            &|| false,
            &admit,
        )
        .unwrap();
        assert_eq!(ranked, baseline);
        assert_eq!(ranked, [1., 0.25, 0.25, 0.25, 0.]);
        let source_ticks = 100_000_000 - baseline_work;
        let ranked_ticks = 100_000_000 - ranked_work;
        eprintln!("RANKED_DENSE_WORK source_ticks={source_ticks} ranked_ticks={ranked_ticks}");
        assert!(
            ranked_ticks * 2 < source_ticks,
            "source={source_ticks} ranked={ranked_ticks}"
        );
        assert_eq!(used.load(Ordering::SeqCst), baseline_bytes);
        drop(records);
        drop(credit);
        drop(pool);
        assert_eq!(used.load(Ordering::SeqCst), 0);
    }
    #[test]
    fn dense_subcells_preserve_area_component_integrals_and_release_on_refusal() {
        let used = Arc::new(AtomicUsize::new(0));
        let calls = Cell::new(0usize);
        let fail = Cell::new(usize::MAX);
        let admit = |bytes: usize| -> Option<Credit> {
            calls.set(calls.get() + 1);
            if calls.get() == fail.get() {
                return None;
            }
            let next = used.load(Ordering::SeqCst).checked_add(bytes)?;
            if next > 8 * 1024 * 1024 {
                return None;
            }
            used.store(next, Ordering::SeqCst);
            Some(Box::new(Reservation(used.clone(), bytes)))
        };
        let mut patch = source(0., 1., [0.; 3]);
        patch.colors =
            [[0., 0., 0.], [0., 1., 0.], [1., 1., 1.], [1., 0., 0.]].map(|c| c.into_iter().collect());
        let refs = [AdaptivePatchRef::Tensor(&patch)];
        let mut pool = ParamPool::new();
        let mut records = Vec::new();
        let mut credit = None;
        // A coarse baseline has identical UV fields but different affine color
        // approximations. Later fine triangles must still win exact UV ties.
        for points in [
            [Point::ZERO, Point::new(1., 0.), Point::new(0., 1.)],
            [Point::new(1., 0.), Point::new(1., 1.), Point::new(0., 1.)],
        ] {
            let mut parameters = [[0; 2]; 3];
            for (i, p) in points.iter().enumerate() {
                parameters[i] = [
                    pool.intern(p.x, &admit).unwrap(),
                    pool.intern(p.y, &admit).unwrap(),
                ];
            }
            push_record(
                &mut records,
                &mut credit,
                Record {
                    points,
                    parameters,
                    patch_index: 0,
                },
                &admit,
            )
            .unwrap();
        }
        for y in 0..16 {
            for x in 0..16 {
                let [a, b, c, d] = [(x, y), (x + 1, y), (x, y + 1), (x + 1, y + 1)]
                    .map(|(x, y)| Point::new(x as f64 / 16., y as f64 / 16.));
                for points in [[a, b, c], [b, d, c]] {
                    let mut parameters = [[0; 2]; 3];
                    for (i, p) in points.iter().enumerate() {
                        parameters[i] = [
                            pool.intern(p.x, &admit).unwrap(),
                            pool.intern(p.y, &admit).unwrap(),
                        ];
                    }
                    push_record(
                        &mut records,
                        &mut credit,
                        Record {
                            points,
                            parameters,
                            patch_index: 0,
                        },
                        &admit,
                    )
                    .unwrap();
                }
            }
        }
        let baseline = used.load(Ordering::SeqCst);
        let run = |cancelled: &dyn Fn() -> bool, work: &mut usize| {
            integrate_cell(
                &records,
                &refs,
                &pool,
                &[None],
                Point::ZERO,
                1.,
                0,
                32,
                work,
                cancelled,
                &admit,
            )
        };
        calls.set(0);
        let mut work = 10_000_000;
        let actual = run(&|| false, &mut work).unwrap();
        let reservations = calls.get();
        for (a, b) in actual
            .into_iter()
            .zip([1., 0.5, 0.5, 0.25 - 1. / (12. * 256.), 0.])
        {
            assert!((a - b).abs() < 1e-12, "{actual:?}");
        }
        assert!(reservations > 10 && work < 10_000_000);
        assert_eq!(used.load(Ordering::SeqCst), baseline);
        for stop in [1, reservations / 2, reservations] {
            calls.set(0);
            fail.set(stop);
            assert!(run(&|| false, &mut 10_000_000).is_none());
            assert_eq!(used.load(Ordering::SeqCst), baseline);
        }
        fail.set(usize::MAX);
        assert!(run(&|| false, &mut 0).is_none());
        assert!(run(&|| true, &mut 10_000_000).is_none());
        let checkpoints = Cell::new(0usize);
        run(
            &|| {
                checkpoints.set(checkpoints.get() + 1);
                false
            },
            &mut 10_000_000,
        )
        .unwrap();
        for stop in [checkpoints.get() / 2, checkpoints.get()] {
            let at = Cell::new(0usize);
            assert!(
                run(
                    &|| {
                        at.set(at.get() + 1);
                        at.get() == stop
                    },
                    &mut 10_000_000
                )
                .is_none()
            );
            assert_eq!(used.load(Ordering::SeqCst), baseline);
        }
        run(&|| false, &mut 10_000_000).unwrap();
        // The same geometry in 1/16 of a pixel edge requires more than four
        // recursion levels, while retaining exactly scaled source integrals.
        for block in &mut records {
            for record in &mut block.records {
                record.points = record.points.map(|p| Point::new(p.x / 16., p.y / 16.));
            }
            block.bounds = Rect::new(
                block.bounds.x0 / 16.,
                block.bounds.y0 / 16.,
                block.bounds.x1 / 16.,
                block.bounds.y1 / 16.,
            );
        }
        let scaled = integrate_cell(
            &records,
            &refs,
            &pool,
            &[None],
            Point::ZERO,
            1.,
            0,
            32,
            &mut 10_000_000,
            &|| false,
            &admit,
        )
        .unwrap();
        for (a, b) in scaled.into_iter().zip(actual) {
            assert!((a - b / 256.).abs() < 1e-12);
        }
        assert_eq!(used.load(Ordering::SeqCst), baseline);
        for block in &mut records {
            for record in &mut block.records {
                record.points = record.points.map(|p| Point::new(p.x / 16., p.y / 16.));
            }
            block.bounds = Rect::new(
                block.bounds.x0 / 16.,
                block.bounds.y0 / 16.,
                block.bounds.x1 / 16.,
                block.bounds.y1 / 16.,
            );
        }
        let microscopic = integrate_cell(
            &records,
            &refs,
            &pool,
            &[None],
            Point::ZERO,
            1.,
            0,
            32,
            &mut 10_000_000,
            &|| false,
            &admit,
        )
        .unwrap();
        for (a, b) in microscopic.into_iter().zip(actual) {
            assert!((a - b / 65536.).abs() < 1e-12);
        }
        assert_eq!(used.load(Ordering::SeqCst), baseline);
        drop(records);
        drop(credit);
        drop(pool);
        assert_eq!(used.load(Ordering::SeqCst), 0);
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
