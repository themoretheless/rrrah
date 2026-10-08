//! Bounded experimental painter-order rasterization. Test-only until real
//! source fidelity and speed are measured. Unlike retained polygon fragments,
//! each covered pixel has exactly 64 spatial slots regardless of overlap count.
use super::*;
const SIDE: usize = 8;
const SAMPLES: usize = SIDE * SIDE;
struct Pixel {
    colors: Vec<f32>,
    covered: u64,
}
type Pixels = hashbrown::HashMap<(u16, u16), Pixel, FxBuildHasher>;

fn table_bound(capacity: usize, components: usize) -> Option<usize> {
    let buckets = if capacity < 4 {
        4
    } else if capacity < 8 {
        8
    } else {
        capacity.checked_mul(8)?.div_ceil(7).checked_next_power_of_two()?
    };
    let slots = components.checked_mul(SAMPLES)?.checked_mul(size_of::<f32>())?;
    buckets
        .checked_mul(size_of::<((u16, u16), Pixel)>() + 1 + slots)?
        .checked_add(128)
}

fn sample_ordered(
    triangles: &[Triangle],
    transform: Affine,
    bounds: kurbo::Rect,
    cancelled: &dyn Fn() -> bool,
    admit: &dyn Fn(usize) -> Option<Box<dyn std::any::Any + Send + Sync>>,
) -> Option<MeshSamples> {
    if cancelled() {
        return None;
    }
    let components = triangles.first()?.p0.colors.len();
    if components == 0 {
        return None;
    }
    let spill = color_spill_bound(components)?;
    let _scratch = if spill == 0 {
        None
    } else {
        Some(admit(spill.checked_mul(8)?)?)
    };
    let mut pixels = Pixels::default();
    let mut guard = None;
    for source in triangles {
        if cancelled() {
            return None;
        }
        let vertices = [&source.p0, &source.p1, &source.p2];
        if vertices
            .iter()
            .any(|v| v.colors.len() != components || v.colors.iter().any(|c| !c.is_finite()))
        {
            return None;
        }
        let transformed: [crate::shading::TriangleVertex; 3] = std::array::from_fn(|i| {
            let mut vertex = vertices[i].clone();
            vertex.point = transform * vertex.point;
            vertex
        });
        if transformed
            .iter()
            .any(|v| !v.point.x.is_finite() || !v.point.y.is_finite())
        {
            return None;
        }
        let triangle = Triangle::new(
            transformed[0].clone(),
            transformed[1].clone(),
            transformed[2].clone(),
        );
        let bbox = triangle.bounding_box().intersect(bounds);
        for y in bbox.y0.floor() as u16..bbox.y1.ceil() as u16 {
            if cancelled() {
                return None;
            }
            for x in bbox.x0.floor() as u16..bbox.x1.ceil() as u16 {
                if cancelled() {
                    return None;
                }
                for sy in 0..SIDE {
                    for sx in 0..SIDE {
                        let point = Point::new(
                            f64::from(x) + (sx as f64 + 0.5) / SIDE as f64,
                            f64::from(y) + (sy as f64 + 0.5) / SIDE as f64,
                        );
                        if !triangle.contains_point(point) {
                            continue;
                        }
                        if pixels.len() == pixels.capacity() && !pixels.contains_key(&(x, y)) {
                            let capacity = pixels.capacity().checked_mul(2)?.max(3);
                            let bytes = table_bound(capacity, components)?;
                            let next = admit(bytes)?;
                            if cancelled() {
                                return None;
                            }
                            pixels.reserve(capacity - pixels.len());
                            assert!(pixels.allocation_size() <= bytes);
                            guard = Some(next);
                        }
                        let color = triangle.interpolate(point);
                        if color.iter().any(|c| !c.is_finite()) {
                            return None;
                        }
                        let pixel = pixels.entry((x, y)).or_insert_with(|| Pixel {
                            colors: vec![0.0; SAMPLES * components],
                            covered: 0,
                        });
                        let slot = sy * SIDE + sx;
                        // Later geometry replaces earlier geometry at this sample.
                        pixel.colors[slot * components..(slot + 1) * components].copy_from_slice(&color);
                        pixel.covered |= 1_u64 << slot;
                    }
                }
            }
        }
    }
    let _ = guard.as_ref(); // Keep temporary table/slot credit until consumption ends.
    let bytes = mesh_table_bound(pixels.len(), components)?;
    let output_guard = admit(bytes)?;
    if cancelled() {
        return None;
    }
    let mut map = MeshMap::with_capacity_and_hasher(pixels.len(), FxBuildHasher::default());
    assert!(map.allocation_size() <= bytes);
    for (index, (key, pixel)) in pixels.into_iter().enumerate() {
        if index % 64 == 0 && cancelled() {
            return None;
        }
        let count = pixel.covered.count_ones();
        let mut colors: ColorComponents = (0..components).map(|_| 0.0).collect();
        for slot in 0..SAMPLES {
            if pixel.covered & (1_u64 << slot) != 0 {
                for c in 0..components {
                    colors[c] += pixel.colors[slot * components + c] / count as f32;
                }
            }
        }
        map.insert(
            key,
            MeshSample {
                colors,
                coverage: count as f32 / SAMPLES as f32,
            },
        );
    }
    if cancelled() {
        return None;
    }
    Some(MeshSamples {
        map,
        _guard: Some(output_guard),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn folded_patch_keeps_the_larger_v_branch() {
        let mut triangles = Vec::new();
        crate::shading::generate_ordered_patch_grid(
            |p| Point::new(p.x, 4.0 * p.y * (1.0 - p.y)),
            |p| [p.y as f32, 0.0, 0.0].into_iter().collect(),
            &mut triangles,
        );
        let result = sample_ordered(
            &triangles,
            Affine::IDENTITY,
            kurbo::Rect::new(0.0, 0.0, 1.0, 1.0),
            &|| false,
            &|_| Some(Box::new(())),
        )
        .unwrap();
        // The two inverse branches are (1 +/- sqrt(1-y))/2. A painter
        // traversal must select the plus branch at every covered sample.
        let expected: f32 = (0..SIDE)
            .map(|sy| {
                let y = (sy as f64 + 0.5) / SIDE as f64;
                ((1.0 + (1.0 - y).sqrt()) / 2.0) as f32
            })
            .sum::<f32>()
            / SIDE as f32;
        assert!((result[&(0, 0)].colors[0] - expected).abs() < 0.005);
        assert_eq!(result[&(0, 0)].coverage, 1.0);
    }
    fn rectangle(x0: f64, x1: f64, color: [f32; 3]) -> [Triangle; 2] {
        let mut seed = Vec::new();
        crate::shading::TensorProductPatch {
            control_points: [Point::ZERO; 16],
            colors: std::array::from_fn(|_| color.into_iter().collect()),
        }
        .to_triangles(&mut seed);
        let prototype = seed[0].p0.clone();
        let vertex = |x, y| {
            let mut vertex = prototype.clone();
            vertex.point = Point::new(x, y);
            vertex
        };
        [
            Triangle::new(vertex(x0, 0.0), vertex(x1, 0.0), vertex(x0, 1.0)),
            Triangle::new(vertex(x1, 0.0), vertex(x1, 1.0), vertex(x0, 1.0)),
        ]
    }
    #[test]
    fn later_geometry_replaces_overlap_without_averaging_hidden_color() {
        let mut triangles = rectangle(0.0, 1.0, [1.0, 0.0, 0.0]).to_vec();
        triangles.extend(rectangle(0.0, 1.0, [0.0, 0.0, 1.0]));
        let samples = sample_ordered(
            &triangles,
            Affine::IDENTITY,
            kurbo::Rect::new(0.0, 0.0, 1.0, 1.0),
            &|| false,
            &|_| Some(Box::new(())),
        )
        .unwrap();
        assert_eq!(samples[&(0, 0)].colors.as_slice(), &[0.0, 0.0, 1.0]);
        assert_eq!(samples[&(0, 0)].coverage, 1.0);
    }
    #[test]
    fn partial_overlap_preserves_visible_portion_of_earlier_patch() {
        let mut triangles = rectangle(0.0, 1.0, [1.0, 0.0, 0.0]).to_vec();
        triangles.extend(rectangle(0.5, 1.0, [0.0, 0.0, 1.0]));
        let samples = sample_ordered(
            &triangles,
            Affine::IDENTITY,
            kurbo::Rect::new(0.0, 0.0, 1.0, 1.0),
            &|| false,
            &|_| Some(Box::new(())),
        )
        .unwrap();
        assert_eq!(samples[&(0, 0)].colors.as_slice(), &[0.5, 0.0, 0.5]);
        assert_eq!(samples[&(0, 0)].coverage, 1.0);
    }
    #[test]
    fn cancellation_after_storage_admission_releases_every_credit() {
        use std::sync::{
            Arc,
            atomic::{AtomicUsize, Ordering},
        };
        struct Credit(Arc<AtomicUsize>);
        impl Drop for Credit {
            fn drop(&mut self) {
                self.0.fetch_sub(1, Ordering::Relaxed);
            }
        }
        let live = Arc::new(AtomicUsize::new(0));
        let admissions = AtomicUsize::new(0);
        let triangles = rectangle(0.0, 16.0, [1.0, 0.0, 0.0]);
        let result = sample_ordered(
            &triangles,
            Affine::IDENTITY,
            kurbo::Rect::new(0.0, 0.0, 16.0, 1.0),
            &|| admissions.load(Ordering::Relaxed) > 0,
            &|_| {
                admissions.fetch_add(1, Ordering::Relaxed);
                live.fetch_add(1, Ordering::Relaxed);
                Some(Box::new(Credit(live.clone())))
            },
        );
        assert!(result.is_none());
        assert_eq!(admissions.load(Ordering::Relaxed), 1);
        assert_eq!(live.load(Ordering::Relaxed), 0);
    }
    #[test]
    fn spatial_credit_is_bounded_independently_of_overlap_count_and_released() {
        use std::sync::{
            Arc,
            atomic::{AtomicUsize, Ordering},
        };
        struct Credit {
            used: Arc<AtomicUsize>,
            bytes: usize,
        }
        impl Drop for Credit {
            fn drop(&mut self) {
                self.used.fetch_sub(self.bytes, Ordering::Relaxed);
            }
        }
        let mut peaks = Vec::new();
        for repetitions in [1, 1024] {
            let used = Arc::new(AtomicUsize::new(0));
            let peak = AtomicUsize::new(0);
            let admit = |bytes| -> Option<Box<dyn std::any::Any + Send + Sync>> {
                let current = used.fetch_add(bytes, Ordering::Relaxed) + bytes;
                if current > 4096 {
                    used.fetch_sub(bytes, Ordering::Relaxed);
                    return None;
                }
                peak.fetch_max(current, Ordering::Relaxed);
                Some(Box::new(Credit {
                    used: used.clone(),
                    bytes,
                }))
            };
            let triangles: Vec<_> = (0..repetitions)
                .flat_map(|_| rectangle(0.0, 1.0, [0.0, 0.0, 1.0]))
                .collect();
            let result = sample_ordered(
                &triangles,
                Affine::IDENTITY,
                kurbo::Rect::new(0.0, 0.0, 1.0, 1.0),
                &|| false,
                &admit,
            )
            .unwrap();
            assert!(used.load(Ordering::Relaxed) > 0);
            assert_eq!(result[&(0, 0)].colors.as_slice(), &[0.0, 0.0, 1.0]);
            peaks.push(peak.load(Ordering::Relaxed));
            drop(result);
            assert_eq!(used.load(Ordering::Relaxed), 0);
        }
        assert_eq!(peaks[0], peaks[1]);
    }

    #[test]
    fn admission_refusal_and_cancellation_produce_no_partial_result() {
        let triangles = rectangle(0.0, 1.0, [1.0, 0.0, 0.0]);
        let bounds = kurbo::Rect::new(0.0, 0.0, 1.0, 1.0);
        assert!(sample_ordered(&triangles, Affine::IDENTITY, bounds, &|| false, &|_| None).is_none());
        let polls = std::sync::atomic::AtomicUsize::new(0);
        assert!(
            sample_ordered(
                &triangles,
                Affine::IDENTITY,
                bounds,
                &|| polls.fetch_add(1, std::sync::atomic::Ordering::Relaxed) > 3,
                &|_| Some(Box::new(()))
            )
            .is_none()
        );
    }
}
