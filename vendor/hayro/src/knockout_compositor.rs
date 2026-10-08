//! Normal-blend knockout algebra prototype. Integration requires an independently
//! rendered object shape, the initial group backdrop, and admitted buffers.
//! Alpha alone cannot stand in for shape. This does not replace production PDF
//! group rendering; interpreter replay is limited to authored test fixtures.

#[path = "bounded_stroke.rs"]
mod bounded_stroke;
#[path = "bounded_stroke_outline.rs"]
mod bounded_stroke_outline;
#[path = "knockout_interpreter.rs"]
mod interpreter_tests;
#[path = "polygon_coverage.rs"]
mod polygon_coverage;

#[path = "native_blend.rs"]
mod native_blend;

#[path = "native_paint.rs"]
mod native_paint;

#[path = "native_frame.rs"]
mod native_frame;

struct CompositeBuffer {
    pixels: Vec<[u8; 4]>,
    // Buffer storage drops before its memory admission credit.
    _credit: Box<dyn std::any::Any + Send + Sync>,
}

/// Reduce premultiplied spatial samples only after group composition. The
/// caller owns admitted input storage; this retains output credit until drop.
fn reduce_spatial_samples(
    input: &[[u8; 4]],
    width: usize,
    height: usize,
    factor: usize,
    cancelled: &dyn Fn() -> bool,
    admit: &dyn Fn(usize) -> Option<Box<dyn std::any::Any + Send + Sync>>,
) -> Option<CompositeBuffer> {
    if cancelled() || width == 0 || height == 0 || factor == 0 {
        return None;
    }
    let samples = factor.checked_mul(factor)?;
    let stride = width.checked_mul(factor)?;
    let count = width.checked_mul(height)?;
    if input.len() != count.checked_mul(samples)? {
        return None;
    }
    let divisor = u64::try_from(samples).ok()?;
    // Maximum channel sum plus rounding must fit the accumulator.
    divisor.checked_mul(255)?.checked_add(divisor / 2)?;
    let credit = admit(count.checked_mul(size_of::<[u8; 4]>())?)?;
    if cancelled() {
        return None;
    }
    let mut pixels = Vec::with_capacity(count);
    let mut polls = 0usize;
    for y in 0..height {
        for x in 0..width {
            let mut sum = [0u64; 4];
            for sy in 0..factor {
                for sx in 0..factor {
                    if polls % 256 == 0 && cancelled() {
                        return None;
                    }
                    polls += 1;
                    let sample = input[(y * factor + sy) * stride + x * factor + sx];
                    if sample[..3].iter().any(|&c| c > sample[3]) {
                        return None;
                    }
                    for channel in 0..4 {
                        sum[channel] += u64::from(sample[channel]);
                    }
                }
            }
            pixels.push(sum.map(|v| ((v + divisor / 2) / divisor) as u8));
        }
    }
    if cancelled() {
        return None;
    }
    Some(CompositeBuffer {
        pixels,
        _credit: credit,
    })
}

#[test]
fn spatial_reduction_preserves_premultiplication_and_checks_admission() {
    let input = [[0, 0, 0, 0], [128, 0, 0, 128], [0, 64, 0, 64], [0, 0, 255, 255]];
    let output = reduce_spatial_samples(&input, 1, 1, 2, &|| false, &|bytes| {
        assert_eq!(bytes, 4);
        Some(Box::new(()))
    })
    .unwrap();
    assert_eq!(output.pixels, [[32, 16, 64, 112]]);
    assert!(reduce_spatial_samples(&input, 1, 1, 2, &|| false, &|_| None).is_none());
    for factor in [0, usize::MAX] {
        assert!(
            reduce_spatial_samples(&input, 1, 1, factor, &|| false, &|_| panic!(
                "invalid dimensions admitted"
            ))
            .is_none()
        );
    }
}

#[test]
fn spatial_reduction_cancellation_and_invalid_samples_release_credit() {
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
    let used = Arc::new(AtomicUsize::new(0));
    let admit = |_| -> Option<Box<dyn std::any::Any + Send + Sync>> {
        used.fetch_add(1, Ordering::Relaxed);
        Some(Box::new(Credit(used.clone())))
    };
    let input = vec![[255; 4]; 1024];
    for stop in 1..=7 {
        let calls = AtomicUsize::new(0);
        let output = reduce_spatial_samples(
            &input,
            16,
            16,
            2,
            &|| calls.fetch_add(1, Ordering::Relaxed) + 1 >= stop,
            &admit,
        );
        assert!(output.is_none());
        assert_eq!(used.load(Ordering::Relaxed), 0);
    }
    let output = reduce_spatial_samples(&input, 16, 16, 2, &|| false, &admit).unwrap();
    assert_eq!(used.load(Ordering::Relaxed), 1);
    drop(output);
    assert_eq!(used.load(Ordering::Relaxed), 0);
    let mut invalid = input;
    invalid[1023] = [1, 0, 0, 0];
    assert!(reduce_spatial_samples(&invalid, 16, 16, 2, &|| false, &admit).is_none());
    assert_eq!(used.load(Ordering::Relaxed), 0);
}

/// One output buffer plus one spatial tile; no supersampled full-page buffer.
fn composite_spatial_tiles(
    width: usize,
    height: usize,
    factor: usize,
    edge: usize,
    cancelled: &dyn Fn() -> bool,
    admit: &dyn Fn(usize) -> Option<Box<dyn std::any::Any + Send + Sync>>,
    render: &mut dyn FnMut(usize, usize, usize, usize) -> Option<CompositeBuffer>,
) -> Option<CompositeBuffer> {
    if cancelled() || width == 0 || height == 0 || factor == 0 || edge == 0 {
        return None;
    }
    // Validate worst-case tile before allocating output or invoking producer.
    let max_tw = edge.min(width).checked_mul(factor)?;
    let max_th = edge.min(height).checked_mul(factor)?;
    max_tw.checked_mul(max_th)?.checked_mul(size_of::<[u8; 4]>())?;
    let count = width.checked_mul(height)?;
    let credit = admit(count.checked_mul(size_of::<[u8; 4]>())?)?;
    if cancelled() {
        return None;
    }
    let mut pixels = vec![[0; 4]; count];
    for y in (0..height).step_by(edge) {
        for x in (0..width).step_by(edge) {
            if cancelled() {
                return None;
            }
            let tw = edge.min(width - x);
            let th = edge.min(height - y);
            let spatial = render(x, y, tw.checked_mul(factor)?, th.checked_mul(factor)?)?;
            let tile = reduce_spatial_samples(&spatial.pixels, tw, th, factor, cancelled, admit)?;
            for row in 0..th {
                if cancelled() {
                    return None;
                }
                pixels[(y + row) * width + x..(y + row) * width + x + tw]
                    .copy_from_slice(&tile.pixels[row * tw..(row + 1) * tw]);
            }
        }
    }
    if cancelled() {
        return None;
    }
    Some(CompositeBuffer {
        pixels,
        _credit: credit,
    })
}

struct ShapeMask {
    values: Vec<u8>,
    _credit: Box<dyn std::any::Any + Send + Sync>,
}

#[test]
fn nested_clips_intersect_and_evenodd_clip_hole_remains_uncovered() {
    use kurbo::Shape;
    let path = kurbo::Rect::new(0.0, 0.0, 32.0, 16.0).to_path(0.1);
    let narrow = kurbo::Rect::new(4.0, 0.0, 20.0, 16.0).to_path(0.1);
    let mut hole = path.clone();
    hole.extend(
        kurbo::Rect::new(10.0, 4.0, 14.0, 12.0)
            .to_path(0.1)
            .elements()
            .iter()
            .copied(),
    );
    let mask = rasterize_path_shape_with_clips(
        &path,
        kurbo::Affine::IDENTITY,
        [
            (&narrow, vello_cpu::peniko::Fill::NonZero),
            (&hole, vello_cpu::peniko::Fill::EvenOdd),
        ],
        ShapeDrawMode::Fill(vello_cpu::peniko::Fill::NonZero),
        32,
        16,
        &|| false,
        &|_| Some(Box::new(())),
    )
    .unwrap();
    assert_eq!(mask.values[8 * 32 + 6], 255);
    for x in [2, 12, 24] {
        assert_eq!(mask.values[8 * 32 + x], 0);
    }
}

enum ShapeDrawMode<'a> {
    Fill(vello_cpu::peniko::Fill),
    Stroke(&'a kurbo::Stroke),
}

/// Engine experiment for path coverage. Pixel storage is admitted here;
/// backend scene/resource allocations and in-render cancellation need separate
/// treatment before this can become a production shape renderer.
fn rasterize_fill_shape(
    path: &kurbo::BezPath,
    transform: kurbo::Affine,
    clip: Option<&kurbo::BezPath>,
    width: u16,
    height: u16,
    cancelled: &dyn Fn() -> bool,
    admit: &dyn Fn(usize) -> Option<Box<dyn std::any::Any + Send + Sync>>,
) -> Option<ShapeMask> {
    rasterize_path_shape(
        path,
        transform,
        clip,
        ShapeDrawMode::Fill(vello_cpu::peniko::Fill::NonZero),
        width,
        height,
        cancelled,
        admit,
    )
}

fn rasterize_path_shape(
    path: &kurbo::BezPath,
    transform: kurbo::Affine,
    clip: Option<&kurbo::BezPath>,
    mode: ShapeDrawMode<'_>,
    width: u16,
    height: u16,
    cancelled: &dyn Fn() -> bool,
    admit: &dyn Fn(usize) -> Option<Box<dyn std::any::Any + Send + Sync>>,
) -> Option<ShapeMask> {
    rasterize_path_shape_with_clips(
        path,
        transform,
        clip.into_iter()
            .map(|path| (path, vello_cpu::peniko::Fill::NonZero)),
        mode,
        width,
        height,
        cancelled,
        admit,
    )
}

fn rasterize_path_shape_with_clips<'a>(
    path: &kurbo::BezPath,
    transform: kurbo::Affine,
    clips: impl IntoIterator<Item = (&'a kurbo::BezPath, vello_cpu::peniko::Fill)>,
    mode: ShapeDrawMode<'_>,
    width: u16,
    height: u16,
    cancelled: &dyn Fn() -> bool,
    admit: &dyn Fn(usize) -> Option<Box<dyn std::any::Any + Send + Sync>>,
) -> Option<ShapeMask> {
    rasterize_path_shape_with_transformed_clips(
        path,
        transform,
        clips,
        kurbo::Affine::IDENTITY,
        mode,
        width,
        height,
        cancelled,
        admit,
    )
}

fn rasterize_path_shape_with_transformed_clips<'a>(
    path: &kurbo::BezPath,
    transform: kurbo::Affine,
    clips: impl IntoIterator<Item = (&'a kurbo::BezPath, vello_cpu::peniko::Fill)>,
    clip_transform: kurbo::Affine,
    mode: ShapeDrawMode<'_>,
    width: u16,
    height: u16,
    cancelled: &dyn Fn() -> bool,
    admit: &dyn Fn(usize) -> Option<Box<dyn std::any::Any + Send + Sync>>,
) -> Option<ShapeMask> {
    use vello_cpu::{Pixmap, RenderContext, Resources};
    let count = usize::from(width).checked_mul(usize::from(height))?;
    if width == 0 || height == 0 || cancelled() {
        return None;
    }
    let credit = admit(count.checked_mul(5)?)?; // RGBA raster + extracted alpha.
    if cancelled() {
        return None;
    }
    let mut context = RenderContext::new(width, height);
    context.set_transform(clip_transform);
    let mut clip_count = 0;
    for (clip, rule) in clips {
        if cancelled() {
            return None;
        }
        context.set_fill_rule(rule);
        context.push_clip_path(clip);
        clip_count += 1;
    }
    context.set_transform(transform);
    context.set_paint(vello_cpu::color::AlphaColor::<vello_cpu::color::Srgb>::new(
        [1.0; 4],
    ));
    match mode {
        ShapeDrawMode::Fill(rule) => {
            context.set_fill_rule(rule);
            context.fill_path(path);
        }
        ShapeDrawMode::Stroke(stroke) => {
            context.set_stroke(stroke.clone());
            context.stroke_path(path);
        }
    }
    for _ in 0..clip_count {
        context.pop_clip_path();
    }
    if cancelled() {
        return None;
    }
    let mut raster = Pixmap::new(width, height);
    context.flush();
    context.render_to_pixmap(&mut Resources::default(), &mut raster);
    if cancelled() {
        return None;
    }
    let mut values = Vec::new();
    values.try_reserve_exact(count).ok()?;
    for (index, pixel) in raster.data_as_u8_slice().chunks_exact(4).enumerate() {
        if index % 256 == 0 && cancelled() {
            return None;
        }
        values.push(pixel[3]);
    }
    if cancelled() {
        return None;
    }
    Some(ShapeMask {
        values,
        _credit: credit,
    })
}

#[test]
fn engine_shape_preserves_transformed_fractional_edge_and_clip() {
    use kurbo::Shape;
    let path = kurbo::Rect::new(0.0, 0.0, 23.0, 15.0).to_path(0.1);
    let clip = kurbo::Rect::new(0.0, 0.0, 20.0, 16.0).to_path(0.1);
    let mask = rasterize_fill_shape(
        &path,
        kurbo::Affine::translate((8.5, 0.5)),
        Some(&clip),
        32,
        16,
        &|| false,
        &|bytes| {
            assert_eq!(bytes, 2560);
            Some(Box::new(()))
        },
    )
    .unwrap();
    let edge = mask.values[8 * 32 + 8];
    assert!(edge > 0 && edge < 255);
    assert_eq!(mask.values[8 * 32 + 9], 255);
    assert_eq!(mask.values[8 * 32 + 4], 0);
    assert_eq!(mask.values[8 * 32 + 20], 0);
}

#[test]
fn engine_shape_refuses_pixel_storage_before_backend_work() {
    let path = kurbo::BezPath::new();
    assert!(
        rasterize_fill_shape(&path, kurbo::Affine::IDENTITY, None, 32, 16, &|| false, &|_| None).is_none()
    );
    assert!(
        rasterize_fill_shape(
            &path,
            kurbo::Affine::IDENTITY,
            None,
            32,
            16,
            &|| true,
            &|_| panic!("cancelled before admission")
        )
        .is_none()
    );
}

#[test]
fn engine_shape_respects_evenodd_hole_without_erasing_its_backdrop() {
    use kurbo::Shape;
    let mut path = kurbo::Rect::new(2.0, 2.0, 30.0, 14.0).to_path(0.1);
    path.extend(
        kurbo::Rect::new(10.0, 5.0, 22.0, 11.0)
            .to_path(0.1)
            .elements()
            .iter()
            .copied(),
    );
    let evenodd = rasterize_path_shape(
        &path,
        kurbo::Affine::IDENTITY,
        None,
        ShapeDrawMode::Fill(vello_cpu::peniko::Fill::EvenOdd),
        32,
        16,
        &|| false,
        &|_| Some(Box::new(())),
    )
    .unwrap();
    let nonzero = rasterize_fill_shape(&path, kurbo::Affine::IDENTITY, None, 32, 16, &|| false, &|_| {
        Some(Box::new(()))
    })
    .unwrap();
    assert_eq!(evenodd.values[8 * 32 + 16], 0);
    assert_eq!(nonzero.values[8 * 32 + 16], 255);
    assert_eq!(evenodd.values[8 * 32 + 4], 255);
    let previous = [255, 0, 0, 255];
    assert_eq!(
        composite_pixel(previous, [255; 4], [0; 4], evenodd.values[8 * 32 + 16]),
        Some(previous)
    );
}

#[test]
fn engine_shape_stroke_keeps_width_and_dash_gaps() {
    let mut path = kurbo::BezPath::new();
    path.move_to((8.0, 8.0));
    path.line_to((28.0, 8.0));
    let mut stroke = kurbo::Stroke::new(4.0);
    stroke.start_cap = kurbo::Cap::Butt;
    stroke.end_cap = kurbo::Cap::Butt;
    stroke.dash_pattern = [4.0, 4.0].into_iter().collect();
    let mask = rasterize_path_shape(
        &path,
        kurbo::Affine::IDENTITY,
        None,
        ShapeDrawMode::Stroke(&stroke),
        32,
        16,
        &|| false,
        &|_| Some(Box::new(())),
    )
    .unwrap();
    assert_eq!(mask.values[8 * 32 + 10], 255);
    assert_eq!(mask.values[8 * 32 + 14], 0);
    assert_eq!(mask.values[2 * 32 + 10], 0);
    stroke.start_cap = kurbo::Cap::Round;
    stroke.end_cap = kurbo::Cap::Round;
    let round = rasterize_path_shape(
        &path,
        kurbo::Affine::IDENTITY,
        None,
        ShapeDrawMode::Stroke(&stroke),
        32,
        16,
        &|| false,
        &|_| Some(Box::new(())),
    )
    .unwrap();
    assert!(round.values[8 * 32 + 14] > 0);
}

#[test]
fn engine_shapes_and_buffer_compositor_reproduce_full_overlap_fixture() {
    use kurbo::Shape;
    let red_path = kurbo::Rect::new(0.0, 0.0, 24.0, 16.0).to_path(0.1);
    let blue_path = kurbo::Rect::new(8.0, 0.0, 32.0, 16.0).to_path(0.1);
    let admit = |_: usize| -> Option<Box<dyn std::any::Any + Send + Sync>> { Some(Box::new(())) };
    let red_shape = rasterize_fill_shape(
        &red_path,
        kurbo::Affine::IDENTITY,
        None,
        32,
        16,
        &|| false,
        &admit,
    )
    .unwrap();
    let blue_shape = rasterize_fill_shape(
        &blue_path,
        kurbo::Affine::IDENTITY,
        None,
        32,
        16,
        &|| false,
        &admit,
    )
    .unwrap();
    for background in [[255; 4], [255, 255, 0, 255]] {
        let backdrop = vec![background; 512];
        let red: Vec<_> = red_shape.values.iter().map(|&a| [a, 0, 0, a]).collect();
        let first =
            composite_buffer(&backdrop, &backdrop, &red, &red_shape.values, &|| false, &admit).unwrap();
        let blue: Vec<_> = blue_shape
            .values
            .iter()
            .map(|&shape| {
                let a = ((u32::from(shape) * 128 + 127) / 255) as u8;
                [0, 0, a, a]
            })
            .collect();
        let result = composite_buffer(
            &first.pixels,
            &backdrop,
            &blue,
            &blue_shape.values,
            &|| false,
            &admit,
        )
        .unwrap();
        for (index, &pixel) in result.pixels.iter().enumerate() {
            let x = index % 32;
            let expected = if x < 8 {
                [255, 0, 0, 255]
            } else if background[2] == 255 {
                [127, 127, 255, 255]
            } else {
                [127, 127, 127, 255]
            };
            for channel in 0..4 {
                assert!(
                    pixel[channel].abs_diff(expected[channel]) <= 1,
                    "pixel {index}: {pixel:?} vs {expected:?}"
                );
            }
        }
        let transparent = vec![[0; 4]; 512];
        let cleared = composite_buffer(
            &first.pixels,
            &backdrop,
            &transparent,
            &blue_shape.values,
            &|| false,
            &admit,
        )
        .unwrap();
        for (index, &pixel) in cleared.pixels.iter().enumerate() {
            assert_eq!(
                pixel,
                if index % 32 < 8 {
                    [255, 0, 0, 255]
                } else {
                    background
                }
            );
        }
    }
}

/// Group-only contributions, excluding the initial backdrop's alpha.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
struct GroupCoverage {
    shape: u8,
    alpha: u8,
}

impl GroupCoverage {
    fn add(self, source_shape: u8, source_alpha: u8, knockout: bool) -> Option<Self> {
        if self.alpha > self.shape || source_alpha > source_shape {
            return None;
        }
        let union = |previous: u8, source: u8| -> u8 {
            (u32::from(source) + ((255 - u32::from(source)) * u32::from(previous) + 127) / 255) as u8
        };
        let shape = union(self.shape, source_shape);
        let alpha = if knockout {
            (u32::from(source_alpha) + ((255 - u32::from(source_shape)) * u32::from(self.alpha) + 127) / 255)
                as u8
        } else {
            union(self.alpha, source_alpha)
        };
        Some(Self { shape, alpha })
    }

    fn apply_constant_alpha(self, factor: u8, alpha_is_shape: bool) -> Self {
        let scale = |value: u8| ((u32::from(value) * u32::from(factor) + 127) / 255) as u8;
        Self {
            shape: if alpha_is_shape {
                scale(self.shape)
            } else {
                self.shape
            },
            alpha: scale(self.alpha),
        }
    }
}

#[test]
fn knockout_tracks_zero_opacity_shape_independently() {
    let previous = GroupCoverage::default().add(255, 128, false).unwrap();
    assert_eq!(
        previous.add(255, 0, true),
        Some(GroupCoverage { shape: 255, alpha: 0 })
    );
    assert_eq!(previous.add(255, 0, false), Some(previous));
    assert_eq!(
        previous.add(128, 0, true),
        Some(GroupCoverage {
            shape: 255,
            alpha: 64
        })
    );
}

#[test]
fn group_constant_opacity_preserves_shape_unless_alpha_is_shape() {
    let group = GroupCoverage {
        shape: 128,
        alpha: 64,
    };
    assert_eq!(
        group.apply_constant_alpha(0, false),
        GroupCoverage { shape: 128, alpha: 0 }
    );
    assert_eq!(
        group.apply_constant_alpha(128, false),
        GroupCoverage {
            shape: 128,
            alpha: 32
        }
    );
    assert_eq!(
        group.apply_constant_alpha(128, true),
        GroupCoverage { shape: 64, alpha: 32 }
    );
}

#[test]
fn coverage_invariant_survives_all_source_shape_and_alpha_pairs() {
    for source_shape in 0..=255_u8 {
        for source_alpha in 0..=source_shape {
            for previous in [
                GroupCoverage::default(),
                GroupCoverage {
                    shape: 128,
                    alpha: 64,
                },
                GroupCoverage {
                    shape: 255,
                    alpha: 255,
                },
            ] {
                for knockout in [false, true] {
                    let next = previous.add(source_shape, source_alpha, knockout).unwrap();
                    assert!(next.alpha <= next.shape);
                }
            }
        }
    }
    assert!(GroupCoverage::default().add(64, 128, true).is_none());
    assert!(GroupCoverage { shape: 0, alpha: 1 }.add(0, 0, false).is_none());
}

/// Remove the initial backdrop contribution after rendering a non-isolated
/// group. The group's independently accumulated alpha must be supplied: the
/// composited alpha cannot recover it when the initial backdrop is opaque.
fn remove_initial_backdrop(result: [u8; 4], initial: [u8; 4], group_alpha: u8) -> Option<[u8; 4]> {
    let valid = |p: [u8; 4]| p[..3].iter().all(|&c| c <= p[3]);
    if !valid(result) || !valid(initial) {
        return None;
    }
    let retained = 255 - u32::from(group_alpha);
    let expected_alpha = u32::from(group_alpha) + (retained * u32::from(initial[3]) + 127) / 255;
    if u32::from(result[3]).abs_diff(expected_alpha) > 1 {
        return None;
    }
    let mut source = [0; 4];
    for channel in 0..3 {
        let background = ((retained * u32::from(initial[channel]) + 127) / 255) as i32;
        let contribution = i32::from(result[channel]) - background;
        // At most one code may be lost by intermediate RGBA8 rounding.
        if contribution < -1 || contribution > i32::from(group_alpha) + 1 {
            return None;
        }
        source[channel] = contribution.clamp(0, i32::from(group_alpha)) as u8;
    }
    source[3] = group_alpha;
    Some(source)
}

#[test]
fn opaque_backdrop_does_not_hide_group_alpha_or_color_contribution() {
    assert_eq!(
        remove_initial_backdrop([255, 127, 127, 255], [255; 4], 128),
        Some([128, 0, 0, 128])
    );
    assert_eq!(
        remove_initial_backdrop([127, 127, 128, 255], [255, 255, 0, 255], 128),
        Some([0, 0, 128, 128])
    );
    assert_eq!(remove_initial_backdrop([255; 4], [255; 4], 0), Some([0; 4]));
}

#[test]
fn partially_transparent_and_isolated_backdrops_are_removed_consistently() {
    assert_eq!(
        remove_initial_backdrop([32, 0, 128, 160], [64, 0, 0, 64], 128),
        Some([0, 0, 128, 128])
    );
    assert_eq!(
        remove_initial_backdrop([0, 0, 128, 128], [0; 4], 128),
        Some([0, 0, 128, 128])
    );
}

#[test]
fn inconsistent_backdrop_result_is_rejected_instead_of_hiding_invalid_color() {
    assert!(remove_initial_backdrop([0, 0, 0, 255], [255; 4], 0).is_none());
    assert!(remove_initial_backdrop([0, 0, 0, 32], [0; 4], 128).is_none());
    assert!(remove_initial_backdrop([255, 0, 0, 1], [0; 4], 1).is_none());
}

/// Select the child's initial backdrop before evaluating its contents. A child
/// of a knockout group inherits that group's initial backdrop, not its current
/// accumulated result (PDF 32000-1:2008, 11.4.6, note 6).
fn child_initial_backdrop(
    parent_initial: [u8; 4],
    parent_current: [u8; 4],
    parent_knockout: bool,
    child_isolated: bool,
) -> [u8; 4] {
    if child_isolated {
        [0; 4]
    } else if parent_knockout {
        parent_initial
    } else {
        parent_current
    }
}

#[test]
fn nested_group_backdrop_selection_keeps_initial_and_current_distinct() {
    let initial = [255, 255, 0, 255];
    let current = [255, 0, 0, 255];
    assert_eq!(child_initial_backdrop(initial, current, true, false), initial);
    assert_eq!(child_initial_backdrop(initial, current, false, false), current);
    assert_eq!(child_initial_backdrop(initial, current, true, true), [0; 4]);
    assert_eq!(child_initial_backdrop(initial, current, false, true), [0; 4]);
}

fn composite_buffer(
    previous: &[[u8; 4]],
    backdrop: &[[u8; 4]],
    source: &[[u8; 4]],
    shape: &[u8],
    cancelled: &dyn Fn() -> bool,
    admit: &dyn Fn(usize) -> Option<Box<dyn std::any::Any + Send + Sync>>,
) -> Option<CompositeBuffer> {
    let count = previous.len();
    if backdrop.len() != count || source.len() != count || shape.len() != count || cancelled() {
        return None;
    }
    let bytes = count.checked_mul(size_of::<[u8; 4]>())?;
    let credit = admit(bytes)?;
    if cancelled() {
        return None;
    }
    let mut pixels = Vec::new();
    pixels.try_reserve_exact(count).ok()?;
    for index in 0..count {
        if index % 256 == 0 && cancelled() {
            return None;
        }
        pixels.push(composite_pixel(
            previous[index],
            backdrop[index],
            source[index],
            shape[index],
        )?);
    }
    if cancelled() {
        return None;
    }
    Some(CompositeBuffer {
        pixels,
        _credit: credit,
    })
}

/// Inputs and output use premultiplied RGBA8. Shape is geometric coverage, before
/// constant opacity. The prior group result is replaced only inside that shape.
fn composite_pixel(previous: [u8; 4], backdrop: [u8; 4], source: [u8; 4], shape: u8) -> Option<[u8; 4]> {
    let valid = |p: [u8; 4]| p[..3].iter().all(|&c| c <= p[3]);
    if !valid(previous) || !valid(backdrop) || !valid(source) || source[3] > shape {
        return None;
    }
    let alpha = u32::from(source[3]);
    let shape = u32::from(shape);
    Some(std::array::from_fn(|c| {
        ((255 * u32::from(source[c])
            + (shape - alpha) * u32::from(backdrop[c])
            + (255 - shape) * u32::from(previous[c])
            + 127)
            / 255) as u8
    }))
}

/// PDF separable blend on unassociated colors, followed by group knockout
/// weighting. Normal retains the integer kernel for byte-stable existing output.
fn composite_blended_pixel(
    previous: [u8; 4],
    backdrop: [u8; 4],
    source: [u8; 4],
    shape: u8,
    mode: hayro_interpret::BlendMode,
) -> Option<[u8; 4]> {
    use hayro_interpret::BlendMode;
    let mut result = composite_pixel(previous, backdrop, source, shape)?;
    if mode == BlendMode::Normal {
        return Some(result);
    }
    if !matches!(
        mode,
        BlendMode::Multiply | BlendMode::Screen | BlendMode::HardLight
    ) {
        return None;
    }
    let a = f64::from(source[3]) / 255.0;
    let b = f64::from(backdrop[3]) / 255.0;
    let f = f64::from(shape) / 255.0;
    for channel in 0..3 {
        let cs = if a == 0.0 {
            0.0
        } else {
            f64::from(source[channel]) / 255.0 / a
        };
        let cb = if b == 0.0 {
            0.0
        } else {
            f64::from(backdrop[channel]) / 255.0 / b
        };
        let blend = match mode {
            BlendMode::Multiply => cb * cs,
            BlendMode::Screen => cb + cs - cb * cs,
            BlendMode::HardLight if cs <= 0.5 => 2.0 * cb * cs,
            BlendMode::HardLight => 1.0 - 2.0 * (1.0 - cb) * (1.0 - cs),
            _ => unreachable!(),
        };
        let color = a * ((1.0 - b) * cs + b * blend)
            + (f - a) * f64::from(backdrop[channel]) / 255.0
            + (1.0 - f) * f64::from(previous[channel]) / 255.0;
        result[channel] = (color * 255.0).round().clamp(0.0, f64::from(result[3])) as u8;
    }
    Some(result)
}

#[test]
fn separable_blends_use_initial_backdrop_and_keep_shape_independent() {
    use hayro_interpret::BlendMode;
    let red = [255, 0, 0, 255];
    let yellow = [255, 255, 0, 255];
    let blue = [0, 0, 255, 255];
    assert_eq!(
        composite_blended_pixel(red, yellow, blue, 255, BlendMode::Multiply),
        Some([0, 0, 0, 255])
    );
    assert_eq!(
        composite_blended_pixel(red, yellow, blue, 255, BlendMode::Screen),
        Some([255; 4])
    );
    assert_eq!(
        composite_blended_pixel(red, yellow, blue, 255, BlendMode::HardLight),
        Some(blue)
    );
    assert_eq!(
        composite_blended_pixel(red, yellow, [0, 0, 128, 128], 255, BlendMode::Multiply),
        Some([127, 127, 0, 255])
    );
    assert_eq!(
        composite_blended_pixel(red, yellow, [0, 0, 128, 128], 128, BlendMode::Multiply),
        Some([127, 0, 0, 255])
    );
    for mode in [BlendMode::Multiply, BlendMode::Screen, BlendMode::HardLight] {
        assert_eq!(
            composite_blended_pixel(red, yellow, [0; 4], 255, mode),
            Some(yellow)
        );
        assert_eq!(composite_blended_pixel(red, yellow, [0; 4], 0, mode), Some(red));
        assert_eq!(composite_blended_pixel(red, [0; 4], blue, 255, mode), Some(blue));
    }
}

#[test]
fn half_opacity_child_uses_initial_backdrop() {
    assert_eq!(
        composite_pixel([255, 0, 0, 255], [255; 4], [0, 0, 128, 128], 255),
        Some([127, 127, 255, 255])
    );
    // Independent authored PDF oracle with a yellow initial page backdrop.
    assert_eq!(
        composite_pixel([255, 0, 0, 255], [255, 255, 0, 255], [0, 0, 128, 128], 255),
        Some([127, 127, 128, 255])
    );
}

#[test]
fn transparent_child_replaces_prior_inside_its_shape() {
    assert_eq!(
        composite_pixel([255, 0, 0, 255], [255; 4], [0; 4], 255),
        Some([255; 4])
    );
    assert_eq!(
        composite_pixel([255, 0, 0, 255], [0; 4], [0; 4], 255),
        Some([0; 4])
    );
}

#[test]
fn fractional_shape_preserves_uncovered_previous_result() {
    assert_eq!(
        composite_pixel([255, 0, 0, 255], [255; 4], [0, 0, 32, 32], 64),
        Some([223, 32, 64, 255])
    );
    assert_eq!(
        composite_pixel([64, 0, 0, 64], [0; 4], [0, 0, 32, 32], 64),
        Some([48, 0, 32, 80])
    );
}

#[test]
fn absent_shape_preserves_previous_result() {
    assert_eq!(
        composite_pixel([12, 34, 56, 128], [255; 4], [0; 4], 0),
        Some([12, 34, 56, 128])
    );
}

#[test]
fn invalid_premultiplication_or_alpha_above_shape_is_rejected() {
    assert!(composite_pixel([255; 4], [255; 4], [0, 0, 128, 128], 64).is_none());
    assert!(composite_pixel([255, 0, 0, 0], [255; 4], [0; 4], 255).is_none());
    assert!(composite_pixel([255; 4], [255, 0, 0, 0], [0; 4], 255).is_none());
}

#[test]
fn output_credit_is_retained_until_buffer_drop_and_refusal_preserves_input() {
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
    let previous = [[255, 0, 0, 255]; 1024];
    let backdrop = [[255; 4]; 1024];
    let source = [[0, 0, 128, 128]; 1024];
    let shape = [255; 1024];
    let used = Arc::new(AtomicUsize::new(0));
    let admit = |bytes| -> Option<Box<dyn std::any::Any + Send + Sync>> {
        assert_eq!(bytes, 4096);
        used.fetch_add(bytes, Ordering::Relaxed);
        Some(Box::new(Credit(used.clone(), bytes)))
    };
    let output = composite_buffer(&previous, &backdrop, &source, &shape, &|| false, &admit).unwrap();
    assert_eq!(used.load(Ordering::Relaxed), 4096);
    assert!(output.pixels.iter().all(|p| *p == [127, 127, 255, 255]));
    drop(output);
    assert_eq!(used.load(Ordering::Relaxed), 0);
    assert!(composite_buffer(&previous, &backdrop, &source, &shape, &|| false, &|_| None).is_none());
    assert!(previous.iter().all(|p| *p == [255, 0, 0, 255]));
}

#[test]
fn cancelled_or_invalid_buffer_releases_admission_without_partial_output() {
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
    let previous = [[255, 0, 0, 255]; 1024];
    let backdrop = [[255; 4]; 1024];
    let mut source = [[0, 0, 128, 128]; 1024];
    let shape = [255; 1024];
    let used = Arc::new(AtomicUsize::new(0));
    let admit = |_: usize| -> Option<Box<dyn std::any::Any + Send + Sync>> {
        used.fetch_add(1, Ordering::Relaxed);
        Some(Box::new(Credit(used.clone())))
    };
    let polls = AtomicUsize::new(0);
    assert!(
        composite_buffer(
            &previous,
            &backdrop,
            &source,
            &shape,
            &|| polls.fetch_add(1, Ordering::Relaxed) >= 4,
            &admit
        )
        .is_none()
    );
    assert_eq!(used.load(Ordering::Relaxed), 0);
    source[900] = [255, 0, 0, 0];
    assert!(composite_buffer(&previous, &backdrop, &source, &shape, &|| false, &admit).is_none());
    assert_eq!(used.load(Ordering::Relaxed), 0);
    assert!(
        composite_buffer(
            &previous,
            &backdrop[..100],
            &source,
            &shape,
            &|| false,
            &|_| panic!("mismatched buffers must fail before admission")
        )
        .is_none()
    );
    assert!(previous.iter().all(|p| *p == [255, 0, 0, 255]));
}

#[test]
fn spatial_tiles_release_all_credit_at_every_failure_and_cancellation() {
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
    let run = |deny: usize, stop: usize| {
        let used = Arc::new(AtomicUsize::new(0));
        let attempts = Cell::new(0);
        let polls = Cell::new(0);
        let admit = |bytes| -> Option<Box<dyn std::any::Any + Send + Sync>> {
            attempts.set(attempts.get() + 1);
            if attempts.get() == deny {
                return None;
            }
            let now = used.fetch_add(bytes, Ordering::Relaxed) + bytes;
            assert!(now <= 32 * 16 * 4 + 7 * 7 * 4 * 4 + 7 * 7 * 4);
            Some(Box::new(Credit(used.clone(), bytes)))
        };
        let cancelled = || {
            polls.set(polls.get() + 1);
            polls.get() == stop
        };
        let output = composite_spatial_tiles(32, 16, 2, 7, &cancelled, &admit, &mut |_, _, w, h| {
            let credit = admit(w * h * 4)?;
            if cancelled() {
                return None;
            }
            Some(CompositeBuffer {
                pixels: vec![[255; 4]; w * h],
                _credit: credit,
            })
        });
        if deny == 0 && stop == 0 {
            assert!(output.is_some());
            assert_eq!(used.load(Ordering::Relaxed), 32 * 16 * 4);
        } else {
            assert!(output.is_none(), "deny {deny}, stop {stop}");
        }
        drop(output);
        assert_eq!(used.load(Ordering::Relaxed), 0);
        (attempts.get(), polls.get())
    };
    let (attempts, polls) = run(0, 0);
    assert!(attempts > 20 && polls > 50);
    for deny in 1..=attempts {
        assert_eq!(run(deny, 0).0, deny);
    }
    for stop in 1..=polls {
        assert_eq!(run(0, stop).1, stop);
    }
    assert!(
        composite_spatial_tiles(
            32,
            16,
            usize::MAX,
            7,
            &|| false,
            &|_| panic!("overflow allocated output"),
            &mut |_, _, _, _| panic!("overflow invoked producer")
        )
        .is_none()
    );
}

/// Convert an admitted premultiplied mask-group result to opacity without
/// unpremultiplying: transparent colored pixels must contribute no luminosity.
fn soft_mask_values(
    pixels: &[[u8; 4]],
    luminosity: bool,
    transfer: &dyn Fn(f32) -> f32,
    cancelled: &dyn Fn() -> bool,
    admit: &dyn Fn(usize) -> Option<Box<dyn std::any::Any + Send + Sync>>,
) -> Option<ShapeMask> {
    if cancelled() || pixels.is_empty() {
        return None;
    }
    let credit = admit(pixels.len())?;
    if cancelled() {
        return None;
    }
    let mut values = Vec::new();
    values.try_reserve_exact(pixels.len()).ok()?;
    for (index, pixel) in pixels.iter().enumerate() {
        if index % 256 == 0 && cancelled() {
            return None;
        }
        if pixel[..3].iter().any(|&c| c > pixel[3]) {
            return None;
        }
        let value = if luminosity {
            (0.30 * f32::from(pixel[0]) + 0.59 * f32::from(pixel[1]) + 0.11 * f32::from(pixel[2])) / 255.0
        } else {
            f32::from(pixel[3]) / 255.0
        };
        let mapped = transfer(value);
        if !mapped.is_finite() {
            return None;
        }
        values.push((mapped.clamp(0.0, 1.0) * 255.0).round() as u8);
    }
    if cancelled() {
        return None;
    }
    Some(ShapeMask {
        values,
        _credit: credit,
    })
}

#[test]
fn soft_mask_alpha_and_device_luminosity_preserve_premultiplication() {
    let pixels = [[0, 0, 0, 0], [128, 0, 0, 128], [0, 255, 0, 255], [0, 0, 255, 255]];
    let alpha = soft_mask_values(&pixels, false, &|v| v, &|| false, &|_| Some(Box::new(()))).unwrap();
    assert_eq!(alpha.values, [0, 128, 255, 255]);
    let luminance = soft_mask_values(&pixels, true, &|v| v, &|| false, &|_| Some(Box::new(()))).unwrap();
    assert_eq!(luminance.values, [0, 38, 150, 28]);
    let inverted =
        soft_mask_values(&pixels, false, &|v| 1.0 - v, &|| false, &|_| Some(Box::new(()))).unwrap();
    assert_eq!(inverted.values, [255, 127, 0, 0]);
    assert!(
        soft_mask_values(&[[255, 0, 0, 0]], true, &|v| v, &|| false, &|_| Some(
            Box::new(())
        ))
        .is_none()
    );
    assert!(soft_mask_values(&pixels, false, &|_| f32::NAN, &|| false, &|_| Some(Box::new(()))).is_none());
}

#[test]
fn soft_mask_conversion_cancellation_releases_credit() {
    use std::cell::Cell;
    use std::sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    };
    struct Credit(Arc<AtomicUsize>);
    impl Drop for Credit {
        fn drop(&mut self) {
            self.0.store(0, Ordering::Relaxed);
        }
    }
    let pixels = vec![[64, 64, 64, 128]; 1025];
    let polls = Cell::new(0);
    let complete = soft_mask_values(
        &pixels,
        true,
        &|v| v,
        &|| {
            polls.set(polls.get() + 1);
            false
        },
        &|_| Some(Box::new(())),
    )
    .unwrap();
    assert_eq!(complete.values.len(), 1025);
    for stop in 1..=polls.get() {
        let used = Arc::new(AtomicUsize::new(0));
        let current = Cell::new(0);
        assert!(
            soft_mask_values(
                &pixels,
                true,
                &|v| v,
                &|| {
                    current.set(current.get() + 1);
                    current.get() >= stop
                },
                &|bytes| {
                    used.store(bytes, Ordering::Relaxed);
                    Some(Box::new(Credit(used.clone())))
                }
            )
            .is_none()
        );
        assert_eq!(current.get(), stop);
        assert_eq!(used.load(Ordering::Relaxed), 0);
    }
    assert!(soft_mask_values(&pixels, false, &|v| v, &|| false, &|_| None).is_none());
}

/// AIS=true multiplies geometric shape as well as premultiplied RGBA.
/// Mutation is internal scratch only; cancellation must discard the whole replay.
fn scale_mask_shape(shape: &mut [u8], mask: &[u8], cancelled: &dyn Fn() -> bool) -> Option<()> {
    if shape.len() != mask.len() || shape.is_empty() || cancelled() {
        return None;
    }
    for (index, (value, factor)) in shape.iter_mut().zip(mask).enumerate() {
        if index % 256 == 0 && cancelled() {
            return None;
        }
        *value = ((u32::from(*value) * u32::from(*factor) + 127) / 255) as u8;
    }
    if cancelled() { None } else { Some(()) }
}

#[test]
fn shape_mask_preserves_previous_knockout_child_when_mask_is_zero() {
    let source = [[0, 0, 255, 255]; 3];
    let mask = [0, 128, 255];
    let mut shape = [255; 3];
    scale_mask_shape(&mut shape, &mask, &|| false).unwrap();
    let masked = apply_opacity_mask(&source, &mask, &|| false, &|_| Some(Box::new(()))).unwrap();
    let previous = [255, 0, 0, 255];
    let initial = [255; 4];
    assert_eq!(
        composite_pixel(previous, initial, masked.pixels[0], shape[0]),
        Some(previous)
    );
    assert_eq!(
        composite_pixel(previous, initial, masked.pixels[1], shape[1]),
        Some([127, 0, 128, 255])
    );
    assert_eq!(
        composite_pixel(previous, initial, masked.pixels[2], shape[2]),
        Some([0, 0, 255, 255])
    );
    assert!(scale_mask_shape(&mut shape, &[0], &|| false).is_none());
    assert!(scale_mask_shape(&mut shape, &mask, &|| true).is_none());
}

#[test]
fn shape_mask_stops_at_every_cancellation_checkpoint() {
    use std::cell::Cell;
    let polls = Cell::new(0);
    let mask = [128; 1025];
    scale_mask_shape(&mut [255; 1025], &mask, &|| {
        polls.set(polls.get() + 1);
        false
    })
    .unwrap();
    for stop in 1..=polls.get() {
        let current = Cell::new(0);
        assert!(
            scale_mask_shape(&mut [255; 1025], &mask, &|| {
                current.set(current.get() + 1);
                current.get() >= stop
            })
            .is_none()
        );
        assert_eq!(current.get(), stop);
    }
}

/// A PDF opacity mask scales premultiplied color and alpha. Object shape is
/// deliberately borrowed separately by the compositor and remains unchanged.
fn apply_opacity_mask(
    source: &[[u8; 4]],
    mask: &[u8],
    cancelled: &dyn Fn() -> bool,
    admit: &dyn Fn(usize) -> Option<Box<dyn std::any::Any + Send + Sync>>,
) -> Option<CompositeBuffer> {
    if cancelled() || source.is_empty() || source.len() != mask.len() {
        return None;
    }
    let credit = admit(source.len().checked_mul(size_of::<[u8; 4]>())?)?;
    if cancelled() {
        return None;
    }
    let mut pixels = Vec::new();
    pixels.try_reserve_exact(source.len()).ok()?;
    for (index, (pixel, &factor)) in source.iter().zip(mask).enumerate() {
        if index % 256 == 0 && cancelled() {
            return None;
        }
        if pixel[..3].iter().any(|&channel| channel > pixel[3]) {
            return None;
        }
        pixels.push(pixel.map(|channel| ((u32::from(channel) * u32::from(factor) + 127) / 255) as u8));
    }
    if cancelled() {
        return None;
    }
    Some(CompositeBuffer {
        pixels,
        _credit: credit,
    })
}

#[test]
fn opacity_mask_changes_alpha_without_erasing_knockout_shape() {
    let source = [[0, 0, 255, 255]; 3];
    let masked = apply_opacity_mask(&source, &[0, 128, 255], &|| false, &|_| Some(Box::new(()))).unwrap();
    assert_eq!(masked.pixels, [[0; 4], [0, 0, 128, 128], [0, 0, 255, 255]]);
    let previous = [255, 0, 0, 255];
    let initial = [255; 4];
    assert_eq!(
        composite_pixel(previous, initial, masked.pixels[0], 255),
        Some(initial)
    );
    assert_eq!(
        composite_pixel(previous, initial, masked.pixels[1], 255),
        Some([127, 127, 255, 255])
    );
    // A genuinely absent shape preserves the preceding red child.
    assert_eq!(
        composite_pixel(previous, initial, masked.pixels[0], 0),
        Some(previous)
    );
    assert!(
        apply_opacity_mask(&source, &[255], &|| false, &|_| panic!(
            "invalid lengths admitted"
        ))
        .is_none()
    );
}

#[test]
fn opacity_mask_releases_credit_at_every_cancel_checkpoint() {
    use std::cell::Cell;
    use std::sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    };
    struct Credit(Arc<AtomicUsize>);
    impl Drop for Credit {
        fn drop(&mut self) {
            self.0.store(0, Ordering::Relaxed);
        }
    }
    let source = vec![[32, 64, 128, 128]; 1025];
    let mask = vec![127; 1025];
    let polls = Cell::new(0);
    let result = apply_opacity_mask(
        &source,
        &mask,
        &|| {
            polls.set(polls.get() + 1);
            false
        },
        &|_| Some(Box::new(())),
    )
    .unwrap();
    assert!(result.pixels.iter().all(|p| *p == [16, 32, 64, 64]));
    for stop in 1..=polls.get() {
        let used = Arc::new(AtomicUsize::new(0));
        let current = Cell::new(0);
        assert!(
            apply_opacity_mask(
                &source,
                &mask,
                &|| {
                    current.set(current.get() + 1);
                    current.get() >= stop
                },
                &|bytes| {
                    used.store(bytes, Ordering::Relaxed);
                    Some(Box::new(Credit(used.clone())))
                }
            )
            .is_none()
        );
        assert_eq!(used.load(Ordering::Relaxed), 0);
        assert_eq!(current.get(), stop);
    }
    assert!(apply_opacity_mask(&source, &mask, &|| false, &|_| None).is_none());
}

/// Group intermediates retain precision until final image quantization.
#[derive(Debug, Clone, Copy, Default)]
struct PreciseCoverage {
    shape: f64,
    alpha: f64,
}

fn valid_precise_pixel(pixel: [f64; 4]) -> bool {
    pixel.iter().all(|v| v.is_finite())
        && pixel[3] >= -1e-12
        && pixel[3] <= 1.0 + 1e-12
        && pixel[..3].iter().all(|&v| v >= -1e-12 && v <= pixel[3] + 1e-12)
}

fn composite_precise_pixel(
    previous: [f64; 4],
    backdrop: [f64; 4],
    source: [f64; 4],
    shape: f64,
    mode: hayro_interpret::BlendMode,
) -> Option<[f64; 4]> {
    use hayro_interpret::BlendMode;
    if !valid_precise_pixel(previous)
        || !valid_precise_pixel(backdrop)
        || !valid_precise_pixel(source)
        || !shape.is_finite()
        || shape < 0.0
        || shape > 1.0 + 1e-12
        || source[3] > shape + 1e-12
    {
        return None;
    }
    if !matches!(
        mode,
        BlendMode::Normal | BlendMode::Multiply | BlendMode::Screen | BlendMode::HardLight
    ) {
        return None;
    }
    let a = source[3];
    let b = backdrop[3];
    let mut result = [0.0; 4];
    result[3] = a + (shape - a) * b + (1.0 - shape) * previous[3];
    for channel in 0..3 {
        let cs = if a == 0.0 { 0.0 } else { source[channel] / a };
        let cb = if b == 0.0 { 0.0 } else { backdrop[channel] / b };
        let blend = match mode {
            BlendMode::Normal => cs,
            BlendMode::Multiply => cb * cs,
            BlendMode::Screen => cb + cs - cb * cs,
            BlendMode::HardLight if cs <= 0.5 => 2.0 * cb * cs,
            BlendMode::HardLight => 1.0 - 2.0 * (1.0 - cb) * (1.0 - cs),
            _ => unreachable!(),
        };
        result[channel] = a * ((1.0 - b) * cs + b * blend)
            + (shape - a) * backdrop[channel]
            + (1.0 - shape) * previous[channel];
    }
    valid_precise_pixel(result).then_some(result)
}

impl PreciseCoverage {
    fn add(self, shape: f64, alpha: f64, knockout: bool) -> Option<Self> {
        if !shape.is_finite()
            || !alpha.is_finite()
            || shape < 0.0
            || shape > 1.0 + 1e-12
            || alpha < 0.0
            || alpha > shape + 1e-12
        {
            return None;
        }
        Some(Self {
            shape: shape + (1.0 - shape) * self.shape,
            alpha: alpha + (1.0 - if knockout { shape } else { alpha }) * self.alpha,
        })
    }
}

fn remove_precise_backdrop(result: [f64; 4], initial: [f64; 4], alpha: f64) -> Option<[f64; 4]> {
    if !valid_precise_pixel(result)
        || !valid_precise_pixel(initial)
        || !alpha.is_finite()
        || !(0.0..=1.0 + 1e-12).contains(&alpha)
    {
        return None;
    }
    if (result[3] - (alpha + (1.0 - alpha) * initial[3])).abs() > 1e-12 {
        return None;
    }
    let mut source = [0.0; 4];
    source[3] = alpha;
    for c in 0..3 {
        let value = result[c] - (1.0 - alpha) * initial[c];
        // Only floating-point arithmetic noise is rounded, never RGBA8 errors.
        if value < -1e-12 || value > alpha + 1e-12 {
            return None;
        }
        source[c] = value.clamp(0.0, alpha);
    }
    Some(source)
}

#[test]
fn precise_groups_avoid_false_negative_backdrop_contribution() {
    use hayro_interpret::BlendMode;
    let initial = [166, 154, 95, 255];
    let precise = initial.map(|v| f64::from(v) / 255.0);
    let mut rounded = initial;
    let mut rounded_coverage = GroupCoverage::default();
    let mut current = precise;
    let mut coverage = PreciseCoverage::default();
    for (shape, alpha) in [
        (193, 45),
        (220, 209),
        (186, 24),
        (189, 184),
        (39, 5),
        (146, 73),
        (83, 58),
        (91, 67),
        (34, 2),
    ] {
        rounded = composite_pixel(rounded, initial, [0, 0, 0, alpha], shape).unwrap();
        rounded_coverage = rounded_coverage.add(shape, alpha, true).unwrap();
        current = composite_precise_pixel(
            current,
            precise,
            [0.0, 0.0, 0.0, f64::from(alpha) / 255.0],
            f64::from(shape) / 255.0,
            BlendMode::Normal,
        )
        .unwrap();
        coverage = coverage
            .add(f64::from(shape) / 255.0, f64::from(alpha) / 255.0, true)
            .unwrap();
    }
    assert_eq!(rounded[0], 68);
    assert_eq!(rounded_coverage.alpha, 148);
    assert!(remove_initial_backdrop(rounded, initial, rounded_coverage.alpha).is_none());
    let source = remove_precise_backdrop(current, precise, coverage.alpha).unwrap();
    assert!(source[..3].iter().all(|v| v.abs() < 1e-12));
    assert!((source[3] - coverage.alpha).abs() < 1e-12);
    assert!(remove_precise_backdrop([0.0, 0.0, 0.0, 1.0], [1.0; 4], 0.0).is_none());
}

struct PreciseBuffer {
    pixels: Vec<[f64; 4]>,
    _credit: Box<dyn std::any::Any + Send + Sync>,
}

/// Average spatial samples before the only RGBA8 quantization step.
fn reduce_precise_samples(
    input: &[[f64; 4]],
    width: usize,
    height: usize,
    factor: usize,
    cancelled: &dyn Fn() -> bool,
    admit: &dyn Fn(usize) -> Option<Box<dyn std::any::Any + Send + Sync>>,
) -> Option<CompositeBuffer> {
    if width == 0 || height == 0 || factor == 0 || cancelled() {
        return None;
    }
    let count = width.checked_mul(height)?;
    let samples = factor.checked_mul(factor)?;
    if input.len() != count.checked_mul(samples)? {
        return None;
    }
    let stride = width.checked_mul(factor)?;
    let credit = admit(count.checked_mul(size_of::<[u8; 4]>())?)?;
    if cancelled() {
        return None;
    }
    let mut pixels = Vec::new();
    pixels.try_reserve_exact(count).ok()?;
    let mut polls = 0usize;
    for y in 0..height {
        for x in 0..width {
            let mut sum = [0.0; 4];
            for sy in 0..factor {
                for sx in 0..factor {
                    if polls % 256 == 0 && cancelled() {
                        return None;
                    }
                    polls += 1;
                    let sample = input[(y * factor + sy) * stride + x * factor + sx];
                    if !valid_precise_pixel(sample) {
                        return None;
                    }
                    for c in 0..4 {
                        sum[c] += sample[c];
                    }
                }
            }
            pixels.push(sum.map(|v| (v / (samples as f64) * 255.0).round().clamp(0.0, 255.0) as u8));
        }
    }
    if cancelled() {
        return None;
    }
    Some(CompositeBuffer {
        pixels,
        _credit: credit,
    })
}

#[test]
fn precise_spatial_reduction_avoids_double_rounding_and_rejects_nonfinite_samples() {
    let input = [[1.0; 4], [0.5, 0.5, 1.0, 1.0], [1.0; 4], [0.5, 0.5, 1.0, 1.0]];
    let result = reduce_precise_samples(&input, 1, 1, 2, &|| false, &|bytes| {
        assert_eq!(bytes, 4);
        Some(Box::new(()))
    })
    .unwrap();
    assert_eq!(result.pixels, [[191, 191, 255, 255]]);
    assert!(reduce_precise_samples(&input, 1, 1, 2, &|| false, &|_| None).is_none());
    for factor in [0, usize::MAX] {
        assert!(
            reduce_precise_samples(&input, 1, 1, factor, &|| false, &|_| panic!(
                "invalid dimensions admitted"
            ))
            .is_none()
        );
    }
    let mut invalid = input;
    invalid[3][0] = f64::NAN;
    assert!(reduce_precise_samples(&invalid, 1, 1, 2, &|| false, &|_| Some(Box::new(()))).is_none());
    use std::cell::Cell;
    let polls = Cell::new(0);
    reduce_precise_samples(
        &input,
        1,
        1,
        2,
        &|| {
            polls.set(polls.get() + 1);
            false
        },
        &|_| Some(Box::new(())),
    )
    .unwrap();
    for stop in 1..=polls.get() {
        let current = Cell::new(0);
        assert!(
            reduce_precise_samples(
                &input,
                1,
                1,
                2,
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

fn composite_precise_spatial_tiles(
    width: usize,
    height: usize,
    factor: usize,
    edge: usize,
    cancelled: &dyn Fn() -> bool,
    admit: &dyn Fn(usize) -> Option<Box<dyn std::any::Any + Send + Sync>>,
    render: &mut dyn FnMut(usize, usize, usize, usize) -> Option<PreciseBuffer>,
) -> Option<CompositeBuffer> {
    if cancelled() || width == 0 || height == 0 || factor == 0 || edge == 0 {
        return None;
    }
    // Validate worst-case tile before allocating output or invoking producer.
    let max_tw = edge.min(width).checked_mul(factor)?;
    let max_th = edge.min(height).checked_mul(factor)?;
    max_tw.checked_mul(max_th)?.checked_mul(size_of::<[f64; 4]>())?;
    let count = width.checked_mul(height)?;
    let credit = admit(count.checked_mul(size_of::<[u8; 4]>())?)?;
    if cancelled() {
        return None;
    }
    let mut pixels = vec![[0; 4]; count];
    for y in (0..height).step_by(edge) {
        for x in (0..width).step_by(edge) {
            if cancelled() {
                return None;
            }
            let tw = edge.min(width - x);
            let th = edge.min(height - y);
            let spatial = render(x, y, tw.checked_mul(factor)?, th.checked_mul(factor)?)?;
            let tile = reduce_precise_samples(&spatial.pixels, tw, th, factor, cancelled, admit)?;
            for row in 0..th {
                if cancelled() {
                    return None;
                }
                pixels[(y + row) * width + x..(y + row) * width + x + tw]
                    .copy_from_slice(&tile.pixels[row * tw..(row + 1) * tw]);
            }
        }
    }
    if cancelled() {
        return None;
    }
    Some(CompositeBuffer {
        pixels,
        _credit: credit,
    })
}

#[test]
fn precise_tiles_release_credit_at_every_refusal_and_cancellation() {
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
    let run = |deny: usize, stop: usize| {
        let used = Arc::new(AtomicUsize::new(0));
        let peak = Cell::new(0);
        let attempts = Cell::new(0);
        let polls = Cell::new(0);
        let cancelled = || {
            polls.set(polls.get() + 1);
            stop > 0 && polls.get() >= stop
        };
        let admit = |bytes: usize| -> Option<Box<dyn std::any::Any + Send + Sync>> {
            attempts.set(attempts.get() + 1);
            if attempts.get() == deny {
                return None;
            }
            let next = used.fetch_add(bytes, Ordering::Relaxed) + bytes;
            assert!(
                next <= 13 * 7 * 4 + 6 * 6 * 32 + 3 * 3 * 4,
                "tile scratch exceeded bound"
            );
            peak.set(peak.get().max(next));
            Some(Box::new(Credit(used.clone(), bytes)))
        };
        let result = composite_precise_spatial_tiles(13, 7, 2, 3, &cancelled, &admit, &mut |_, _, w, h| {
            if cancelled() {
                return None;
            }
            let credit = admit(w.checked_mul(h)?.checked_mul(size_of::<[f64; 4]>())?)?;
            if cancelled() {
                return None;
            }
            Some(PreciseBuffer {
                pixels: vec![[0.5, 0.5, 1.0, 1.0]; w * h],
                _credit: credit,
            })
        });
        if deny == 0 && stop == 0 {
            assert!(
                result
                    .as_ref()
                    .unwrap()
                    .pixels
                    .iter()
                    .all(|p| *p == [128, 128, 255, 255])
            );
            assert_eq!(used.load(Ordering::Relaxed), 13 * 7 * 4);
        } else {
            assert!(result.is_none());
        }
        drop(result);
        assert_eq!(used.load(Ordering::Relaxed), 0);
        (attempts.get(), polls.get(), peak.get())
    };
    let (attempts, polls, peak) = run(0, 0);
    assert!(peak < 13 * 7 * 2 * 2 * 32);
    for deny in 1..=attempts {
        let observed = run(deny, 0);
        assert_eq!(observed.0, deny);
    }
    for stop in 1..=polls {
        let observed = run(0, stop);
        assert_eq!(observed.1, stop);
    }
    assert!(
        composite_precise_spatial_tiles(
            13,
            7,
            usize::MAX,
            3,
            &|| false,
            &|_| panic!("overflow admitted"),
            &mut |_, _, _, _| panic!("overflow rendered")
        )
        .is_none()
    );
}

#[test]
#[ignore = "explicit backend translation diagnostic requires fresh report path"]
fn authored_curve_tile_translation_reproducer() {
    use kurbo::{Affine, Circle, Shape};
    use std::io::Write;
    let destination = std::env::var_os("RRRAH_CURVE_TRANSLATION_REPORT").expect("fresh report required");
    let mut output = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(destination)
        .unwrap();
    for attempt in 0..32 {
        let cx = 320.125 + f64::from(attempt) * 0.03125;
        let cy = 220.375 + f64::from(attempt) * 0.0625;
        let radius = 99.1875 + f64::from(attempt) * 0.015625;
        let path = Circle::new((cx, cy), radius).to_path(0.1);
        let whole = rasterize_fill_shape(&path, Affine::IDENTITY, None, 512, 384, &|| false, &|_| {
            Some(Box::new(()))
        })
        .unwrap();
        for oy in (64..384).step_by(64) {
            for ox in (192..512).step_by(64) {
                let tile = rasterize_fill_shape(
                    &path,
                    Affine::translate((-(ox as f64), -(oy as f64))),
                    None,
                    64,
                    64,
                    &|| false,
                    &|_| Some(Box::new(())),
                )
                .unwrap();
                for y in 0..64 {
                    for x in 0..64 {
                        let a = whole.values[(oy + y) * 512 + ox + x];
                        let b = tile.values[y * 64 + x];
                        if a != b {
                            let shifted = rasterize_fill_shape(
                                &path,
                                Affine::translate((64.0, 64.0)),
                                None,
                                576,
                                448,
                                &|| false,
                                &|_| Some(Box::new(())),
                            )
                            .unwrap();
                            let translated_full = shifted.values[(oy + y + 64) * 576 + ox + x + 64];
                            let mut controls = Vec::new();
                            for (dx, dy) in [(64usize, 64usize), (192, 64), (128, 96)] {
                                assert!(cx - radius > dx as f64 && cy - radius > dy as f64);
                                let moved = rasterize_fill_shape(
                                    &path,
                                    Affine::translate((-(dx as f64), -(dy as f64))),
                                    None,
                                    512,
                                    384,
                                    &|| false,
                                    &|_| Some(Box::new(())),
                                )
                                .unwrap();
                                let value = moved.values[(oy + y - dy) * 512 + ox + x - dx];
                                controls
                                    .push(format!("{{\"translation\":[-{dx},-{dy}],\"coverage\":{value}}}"));
                            }
                            let large_view = rasterize_fill_shape(
                                &path,
                                Affine::translate((-(ox as f64), -(oy as f64))),
                                None,
                                512,
                                384,
                                &|| false,
                                &|_| Some(Box::new(())),
                            )
                            .unwrap();
                            let same_offset_large_view = large_view.values[y * 512 + x];
                            let mut precise_controls = Vec::new();
                            for tolerance in [0.01, 0.001, 0.0001] {
                                let mut polygon = kurbo::BezPath::new();
                                kurbo::flatten(path.elements().iter().copied(), tolerance, |element| {
                                    polygon.push(element)
                                });
                                let exact_whole = rasterize_fill_shape(
                                    &polygon,
                                    Affine::IDENTITY,
                                    None,
                                    512,
                                    384,
                                    &|| false,
                                    &|_| Some(Box::new(())),
                                )
                                .unwrap();
                                let exact_tile = rasterize_fill_shape(
                                    &polygon,
                                    Affine::translate((-(ox as f64), -(oy as f64))),
                                    None,
                                    64,
                                    64,
                                    &|| false,
                                    &|_| Some(Box::new(())),
                                )
                                .unwrap();
                                let pw = exact_whole.values[(oy + y) * 512 + ox + x];
                                let pt = exact_tile.values[y * 64 + x];
                                let vertices = polygon.elements().len();
                                precise_controls.push(format!("{{\"tolerance\":{tolerance},\"vertices\":{vertices},\"whole\":{pw},\"tile\":{pt}}}"));
                            }
                            let precise_controls = precise_controls.join(",");
                            let controls = controls.join(",");
                            writeln!(output,"{{\"kind\":\"authored circle cubic path\",\"attempt\":{attempt},\"center\":[{cx},{cy}],\"radius\":{radius},\"path_tolerance\":0.1,\"viewport\":[512,384],\"tile_origin\":[{ox},{oy}],\"tile_size\":[64,64],\"pixel\":[{},{}],\"whole_coverage\":{a},\"tile_coverage\":{b},\"translated_full_coverage\":{translated_full},\"negative_uncropped_controls\":[{controls}],\"same_offset_large_view_coverage\":{same_offset_large_view},\"preflatten_controls\":[{precise_controls}]}}",ox+x,oy+y).unwrap();
                            return;
                        }
                    }
                }
            }
        }
    }
    panic!("no reproduction found in fixed authored search");
}

#[test]
fn global_curve_preflatten_matches_independent_cubic_area_at_reproducer() {
    use kurbo::{Affine, Circle, Shape};
    let source = Circle::new((320.125, 220.375), 99.1875).to_path(0.1);
    // Independent 60-digit cubic area is 0.248925510581..., quantized alpha63.
    // See pdf-authored-cubic-area-oracle-2026-10-08.json; no backend-derived golden.
    for tolerance in [0.001, 0.0001] {
        let mut polygon = kurbo::BezPath::new();
        kurbo::flatten(source.elements().iter().copied(), tolerance, |element| {
            polygon.push(element)
        });
        let whole = rasterize_fill_shape(&polygon, Affine::IDENTITY, None, 512, 384, &|| false, &|_| {
            Some(Box::new(()))
        })
        .unwrap();
        let tile = rasterize_fill_shape(
            &polygon,
            Affine::translate((-320.0, -256.0)),
            None,
            64,
            64,
            &|| false,
            &|_| Some(Box::new(())),
        )
        .unwrap();
        assert_eq!(whole.values[317 * 512 + 341], 63, "tolerance={tolerance}");
        assert_eq!(tile.values[61 * 64 + 21], 63, "tolerance={tolerance}");
    }
}

struct AdmittedContour {
    path: kurbo::BezPath,
    _credit: Box<dyn std::any::Any + Send + Sync>,
}

/// Two passes use bounded call-stack subdivision; no hidden kurbo scratch Vec.
/// Device-space tolerance is independent of the subsequently chosen tile.
fn admitted_global_contour(
    path: &kurbo::BezPath,
    transform: kurbo::Affine,
    tolerance: f64,
    max_vertices: usize,
    cancelled: &dyn Fn() -> bool,
    admit: &dyn Fn(usize) -> Option<Box<dyn std::any::Any + Send + Sync>>,
) -> Option<AdmittedContour> {
    use kurbo::{PathEl, Point};
    if cancelled() || !tolerance.is_finite() || tolerance <= 0.0 || max_vertices == 0 {
        return None;
    }
    fn cubic(
        points: [Point; 4],
        tol: f64,
        depth: usize,
        cancel: &dyn Fn() -> bool,
        emit: &mut dyn FnMut(PathEl) -> Option<()>,
    ) -> Option<()> {
        if cancel() || points.iter().any(|p| !p.x.is_finite() || !p.y.is_finite()) {
            return None;
        }
        let line = points[3] - points[0];
        let length = line.hypot();
        if !length.is_finite() {
            return None;
        }
        let distance = |p: Point| {
            let delta = p - points[0];
            if length == 0.0 {
                delta.hypot()
            } else {
                let direction = line / length;
                let t = delta.dot(direction).clamp(0.0, length);
                (delta - direction * t).hypot()
            }
        };
        // Convex-hull distance bounds the whole curve's deviation from chord.
        if distance(points[1]).max(distance(points[2])) <= tol {
            return emit(PathEl::LineTo(points[3]));
        }
        if depth >= 32 {
            return None;
        }
        let midpoint = |a: Point, b: Point| Point::new(a.x * 0.5 + b.x * 0.5, a.y * 0.5 + b.y * 0.5);
        let a = midpoint(points[0], points[1]);
        let b = midpoint(points[1], points[2]);
        let c = midpoint(points[2], points[3]);
        let d = midpoint(a, b);
        let e = midpoint(b, c);
        let m = midpoint(d, e);
        cubic([points[0], a, d, m], tol, depth + 1, cancel, emit)?;
        cubic([m, e, c, points[3]], tol, depth + 1, cancel, emit)
    }
    let walk = |emit: &mut dyn FnMut(PathEl) -> Option<()>| -> Option<()> {
        let mut current = None;
        let mut start = None;
        for element in path.elements() {
            if cancelled() {
                return None;
            }
            match transform * *element {
                PathEl::MoveTo(p) => {
                    if !p.x.is_finite() || !p.y.is_finite() {
                        return None;
                    }
                    emit(PathEl::MoveTo(p))?;
                    current = Some(p);
                    start = Some(p);
                }
                PathEl::LineTo(p) => {
                    current?;
                    if !p.x.is_finite() || !p.y.is_finite() {
                        return None;
                    }
                    emit(PathEl::LineTo(p))?;
                    current = Some(p);
                }
                PathEl::QuadTo(p1, p2) => {
                    let p0 = current?;
                    cubic(
                        [p0, p0 + (p1 - p0) * (2.0 / 3.0), p2 + (p1 - p2) * (2.0 / 3.0), p2],
                        tolerance,
                        0,
                        cancelled,
                        emit,
                    )?;
                    current = Some(p2);
                }
                PathEl::CurveTo(p1, p2, p3) => {
                    cubic([current?, p1, p2, p3], tolerance, 0, cancelled, emit)?;
                    current = Some(p3);
                }
                PathEl::ClosePath => {
                    current = Some(start?);
                    emit(PathEl::ClosePath)?;
                }
            }
        }
        Some(())
    };
    let mut count = 0usize;
    walk(&mut |_| {
        count = count.checked_add(1)?;
        if count > max_vertices { None } else { Some(()) }
    })?;
    if cancelled() || count == 0 {
        return None;
    }
    let credit = admit(count.checked_mul(size_of::<PathEl>())?)?;
    if cancelled() {
        return None;
    }
    let mut elements = Vec::new();
    elements.try_reserve_exact(count).ok()?;
    walk(&mut |element| {
        if elements.len() >= count {
            return None;
        }
        elements.push(element);
        Some(())
    })?;
    if cancelled() || elements.len() != count {
        return None;
    }
    Some(AdmittedContour {
        path: kurbo::BezPath::from_vec(elements),
        _credit: credit,
    })
}

#[test]
fn admitted_global_contour_preserves_cubic_area_and_refuses_vertex_exhaustion() {
    use kurbo::{Affine, Circle, Shape};
    let input = Circle::new((320.125, 220.375), 99.1875).to_path(0.1);
    let contour = admitted_global_contour(&input, Affine::IDENTITY, 0.0001, 10000, &|| false, &|bytes| {
        assert!(bytes <= 10000 * size_of::<kurbo::PathEl>());
        Some(Box::new(()))
    })
    .unwrap();
    assert!(contour.path.elements().iter().all(|e| matches!(
        e,
        kurbo::PathEl::MoveTo(_) | kurbo::PathEl::LineTo(_) | kurbo::PathEl::ClosePath
    )));
    let whole = rasterize_fill_shape(
        &contour.path,
        Affine::IDENTITY,
        None,
        512,
        384,
        &|| false,
        &|_| Some(Box::new(())),
    )
    .unwrap();
    let tile = rasterize_fill_shape(
        &contour.path,
        Affine::translate((-320.0, -256.0)),
        None,
        64,
        64,
        &|| false,
        &|_| Some(Box::new(())),
    )
    .unwrap();
    assert_eq!(whole.values[317 * 512 + 341], 63);
    assert_eq!(tile.values[61 * 64 + 21], 63);
    assert!(
        admitted_global_contour(&input, Affine::IDENTITY, 0.0001, 8, &|| false, &|_| panic!(
            "vertex overflow admitted"
        ))
        .is_none()
    );
    assert!(admitted_global_contour(&input, Affine::IDENTITY, 0.0001, 10000, &|| false, &|_| None).is_none());
}

#[test]
fn admitted_global_contour_releases_storage_at_every_cancel_checkpoint() {
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
    let mut input = kurbo::BezPath::new();
    input.move_to((0.0, 0.0));
    input.quad_to((1.0, 1.0), (2.0, 0.0));
    input.close_path();
    let polls = Cell::new(0);
    let baseline = admitted_global_contour(
        &input,
        kurbo::Affine::IDENTITY,
        0.02,
        128,
        &|| {
            polls.set(polls.get() + 1);
            false
        },
        &|_| Some(Box::new(())),
    )
    .unwrap();
    assert!(baseline.path.elements().len() > 4);
    drop(baseline);
    for stop in 1..=polls.get() {
        let current = Cell::new(0);
        let used = Arc::new(AtomicUsize::new(0));
        let result = admitted_global_contour(
            &input,
            kurbo::Affine::IDENTITY,
            0.02,
            128,
            &|| {
                current.set(current.get() + 1);
                current.get() >= stop
            },
            &|bytes| {
                used.fetch_add(bytes, Ordering::Relaxed);
                Some(Box::new(Credit(used.clone(), bytes)))
            },
        );
        assert!(result.is_none());
        assert_eq!(current.get(), stop);
        assert_eq!(used.load(Ordering::Relaxed), 0);
    }
    let mut invalid = kurbo::BezPath::new();
    invalid.move_to((0.0, 0.0));
    invalid.curve_to((f64::NAN, 1.0), (2.0, 2.0), (3.0, 3.0));
    assert!(
        admitted_global_contour(
            &invalid,
            kurbo::Affine::IDENTITY,
            0.01,
            128,
            &|| false,
            &|_| panic!("invalid geometry admitted")
        )
        .is_none()
    );
}

/// Experimental native coverage for globally prepared polygon paths and an
/// integer-translation viewport. Refuses other transforms rather than silently
/// routing them through the old coverage backend.
fn rasterize_global_polygon_shape<'a>(
    path: &'a kurbo::BezPath,
    rule: vello_cpu::peniko::Fill,
    clips: impl IntoIterator<Item = (&'a kurbo::BezPath, vello_cpu::peniko::Fill)>,
    origin: (i32, i32),
    width: u16,
    height: u16,
    cancelled: &dyn Fn() -> bool,
    admit: &dyn Fn(usize) -> Option<Box<dyn std::any::Any + Send + Sync>>,
) -> Option<ShapeMask> {
    use kurbo::{PathEl, Rect};
    if width == 0 || height == 0 || cancelled() {
        return None;
    }
    for (start, extent) in [(origin.0, width), (origin.1, height)] {
        if start.abs_diff(0) > 1_048_576 || start.checked_add(i32::from(extent) - 1)?.abs_diff(0) > 1_048_576
        {
            return None;
        }
    }
    let mut regions = [(path, rule); 64];
    let mut bounds = [Rect::ZERO; 64];
    let mut len = 1;
    for clip in clips {
        if cancelled() || len == 64 {
            return None;
        }
        regions[len] = clip;
        len += 1;
    }
    let mut operators = 0usize;
    for (r, (path, _)) in regions[..len].iter().enumerate() {
        let mut bbox = None;
        let mut has_current = false;
        for element in path.elements() {
            operators = operators.checked_add(1)?;
            if operators > 1_000_000 || cancelled() {
                return None;
            }
            match element {
                PathEl::MoveTo(_) => has_current = true,
                PathEl::LineTo(_) | PathEl::ClosePath if !has_current => return None,
                _ => {}
            }
            let point = match element {
                PathEl::MoveTo(p) | PathEl::LineTo(p) => Some(p),
                PathEl::ClosePath => None,
                _ => return None,
            };
            if let Some(p) = point {
                if !p.x.is_finite() || !p.y.is_finite() || p.x.abs() > 1_048_576. || p.y.abs() > 1_048_576. {
                    return None;
                }
                bbox = Some(bbox.map_or(Rect::new(p.x, p.y, p.x, p.y), |b: Rect| {
                    Rect::new(b.x0.min(p.x), b.y0.min(p.y), b.x1.max(p.x), b.y1.max(p.y))
                }));
            }
        }
        bounds[r] = bbox.unwrap_or(Rect::ZERO);
    }
    origin.0.checked_add(i32::from(width))?;
    origin.1.checked_add(i32::from(height))?;
    let count = usize::from(width).checked_mul(usize::from(height))?;
    let credit = admit(count)?;
    if cancelled() {
        return None;
    }
    let mut values = Vec::new();
    values.try_reserve_exact(count).ok()?;
    for y in 0..i32::from(height) {
        let gy = origin.1 + y;
        let outside_row = bounds[..len].iter().any(|b| {
            b.y1 <= f64::from(gy) || b.y0 >= f64::from(gy) + 1.
        });
        let row = if outside_row {
            None
        } else {
            Some(polygon_coverage::admitted_polygon_row(
                &regions[..len], gy, 20_000_000, cancelled, admit,
            )?)
        };
        for x in 0..i32::from(width) {
            if cancelled() {
                return None;
            }
            let (gx, gy) = (origin.0 + x, origin.1 + y);
            let outside = bounds[..len].iter().any(|b| {
                b.x1 <= f64::from(gx)
                    || b.x0 >= f64::from(gx) + 1.
                    || b.y1 <= f64::from(gy)
                    || b.y0 >= f64::from(gy) + 1.
            });
            let area = if outside {
                0.
            } else {
                let Some(area) = row.as_ref()?.pixel_area(
                    &regions[..len],
                    gx,
                    gy,
                    20_000_000,
                    cancelled,
                    admit,
                ) else {
                    if std::env::var_os("RRRAH_TRACE_NATIVE_AREA_REFUSAL").is_some() {
                        eprintln!(
                            "NATIVE_AREA_REFUSAL x={gx} y={gy} regions={len} path_elements={operators}"
                        );
                    }
                    return None;
                };
                area
            };
            values.push((area * 255.).round() as u8);
        }
    }
    Some(ShapeMask {
        values,
        _credit: credit,
    })
}

#[test]
fn native_global_polygon_mask_intersects_regions_before_area_quantization() {
    use kurbo::Shape;
    let path = kurbo::Rect::new(0., 0.5, 4., 2.5).to_path(0.1);
    let clip = kurbo::Rect::new(1.5, 0., 2.5, 3.).to_path(0.1);
    let render = |origin, width| {
        rasterize_global_polygon_shape(
            &path,
            vello_cpu::peniko::Fill::NonZero,
            [(&clip, vello_cpu::peniko::Fill::NonZero)],
            origin,
            width,
            3,
            &|| false,
            &|_| Some(Box::new(())),
        )
        .unwrap()
    };
    let whole = render((0, 0), 4);
    assert_eq!(whole.values, [0, 64, 64, 0, 0, 128, 128, 0, 0, 64, 64, 0]);
    let tile = render((1, 0), 2);
    assert_eq!(tile.values, [64, 64, 128, 128, 64, 64]);
}

#[test]
fn native_global_polygon_mask_retains_output_credit_and_releases_every_refusal() {
    use kurbo::Shape;
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
    let path = kurbo::Rect::new(0., 0., 2., 0.5).to_path(0.1);
    let attempts = Cell::new(0);
    let checks = Cell::new(0);
    let used = Arc::new(AtomicUsize::new(0));
    let output = rasterize_global_polygon_shape(
        &path,
        vello_cpu::peniko::Fill::NonZero,
        [],
        (0, 0),
        2,
        1,
        &|| {
            checks.set(checks.get() + 1);
            false
        },
        &|bytes| {
            attempts.set(attempts.get() + 1);
            used.fetch_add(bytes, Ordering::Relaxed);
            Some(Box::new(Credit(used.clone(), bytes)))
        },
    )
    .unwrap();
    assert_eq!(output.values, [128, 128]);
    assert_eq!(used.load(Ordering::Relaxed), 2);
    drop(output);
    assert_eq!(used.load(Ordering::Relaxed), 0);
    for deny in 1..=attempts.get() {
        let calls = Cell::new(0);
        assert!(
            rasterize_global_polygon_shape(
                &path,
                vello_cpu::peniko::Fill::NonZero,
                [],
                (0, 0),
                2,
                1,
                &|| false,
                &|bytes| {
                    calls.set(calls.get() + 1);
                    if calls.get() == deny {
                        return None;
                    }
                    used.fetch_add(bytes, Ordering::Relaxed);
                    Some(Box::new(Credit(used.clone(), bytes)))
                }
            )
            .is_none()
        );
        assert_eq!(used.load(Ordering::Relaxed), 0);
    }
    for stop in 1..=checks.get() {
        let calls = Cell::new(0);
        assert!(
            rasterize_global_polygon_shape(
                &path,
                vello_cpu::peniko::Fill::NonZero,
                [],
                (0, 0),
                2,
                1,
                &|| {
                    calls.set(calls.get() + 1);
                    calls.get() >= stop
                },
                &|bytes| {
                    used.fetch_add(bytes, Ordering::Relaxed);
                    Some(Box::new(Credit(used.clone(), bytes)))
                }
            )
            .is_none()
        );
        assert_eq!(used.load(Ordering::Relaxed), 0);
    }
}
