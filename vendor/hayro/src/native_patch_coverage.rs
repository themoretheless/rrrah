//! Source-UV ordered patch ownership, followed by path/clip intersection.
use super::SharedClip;
use hayro_interpret::encode::{
    NativeShadingError, NativeShadingSample, NativeShadingSource, OrderedPixel, PixelPartitionError,
};
use kurbo::{BezPath, Point, Rect};

fn native_error(error: PixelPartitionError) -> NativeShadingError {
    match error {
        PixelPartitionError::Cancelled => NativeShadingError::Cancelled,
        PixelPartitionError::Admission => NativeShadingError::Admission,
        PixelPartitionError::Limit => NativeShadingError::WorkLimit,
        PixelPartitionError::Conditioning => NativeShadingError::Unsupported,
        PixelPartitionError::Invalid => NativeShadingError::Invalid,
    }
}

fn add(sum: &mut f64, correction: &mut f64, value: f64) {
    let corrected = value - *correction;
    let next = *sum + corrected;
    *correction = (next - *sum) - corrected;
    *sum = next;
}

fn partition<'a>(
    source: &NativeShadingSource,
    origin: Point,
    rect: Rect,
    remaining: &mut usize,
    dense: &mut bool,
    cancelled: &dyn Fn() -> bool,
    admit: &'a dyn Fn(usize) -> Option<Box<dyn std::any::Any + Send + Sync>>,
) -> Option<(OrderedPixel<'a>, Option<NativeShadingSample>)> {
    if cancelled() || !source.mesh_has_patch_priority() {
        return None;
    }
    let mut pixel = OrderedPixel::new(admit, 4096);
    // The query API has a separate bounded traversal allowance. Prepay its
    // whole quota; callback ownership work uses the remaining shared allowance.
    const QUERY_WORK: usize = 100_000;
    *remaining = remaining.checked_sub(QUERY_WORK)?;
    let background = source.mesh_background_sample(cancelled, remaining).ok()?;
    if let Some(background) = &background {
        let owner = hayro_interpret::encode::PixelSourceOwner {
            patch_index: 0,
            sample_index: usize::MAX,
            u: [0.; 3],
            v: [0.; 3],
            components: background.components.map(|c| [f64::from(c), 0., 0.]),
            component_count: background.count,
        };
        for points in [
            [Point::ZERO, Point::new(1., 0.), Point::new(0., 1.)],
            [Point::new(1., 0.), Point::new(1., 1.), Point::new(0., 1.)],
        ] {
            pixel.paint_triangle(points, owner, remaining, cancelled).ok()?;
        }
    }
    let query = source.visit_mesh_triangles(rect, QUERY_WORK, cancelled, &mut |triangle| {
        let mut owner = source
            .mesh_triangle_patch_owner(triangle.index, origin, cancelled, remaining)?
            .ok_or(NativeShadingError::Unsupported)?;
        owner.patch_index = owner
            .patch_index
            .checked_add(1)
            .ok_or(NativeShadingError::Invalid)?;
        let points = triangle
            .points
            .map(|p| Point::new(p.x - origin.x, p.y - origin.y));
        pixel
            .paint_triangle(points, owner, remaining, cancelled)
            .map_err(native_error)
    });
    if let Err(error) = query {
        *dense = error == NativeShadingError::WorkLimit && *remaining > 0;
        return None;
    }
    Some((pixel, background))
}

pub(super) fn spatial_geometry(
    source: &NativeShadingSource,
    rect: Rect,
    remaining: &mut usize,
    cancelled: &dyn Fn() -> bool,
    admit: &dyn Fn(usize) -> Option<Box<dyn std::any::Any + Send + Sync>>,
) -> Option<super::SpatialMeshGeometry> {
    if rect.width() != 1. || rect.height() != 1. || rect.x0.fract() != 0. || rect.y0.fract() != 0. {
        return None;
    }
    let origin = Point::new(rect.x0, rect.y0);
    let (pixel, background) = partition(source, origin, rect, remaining, &mut false, cancelled, admit)?;
    let count = pixel.fragments();
    if count > 4095 {
        return None;
    }
    let mut vertices = 0usize;
    let mut visit_work = count;
    *remaining = remaining.checked_sub(visit_work)?;
    pixel
        .visit(&mut visit_work, cancelled, &mut |_, _, points, _| {
            *remaining = remaining.checked_sub(1).ok_or(PixelPartitionError::Limit)?;
            vertices = vertices
                .checked_add(points.len() + 1)
                .ok_or(PixelPartitionError::Limit)?;
            Ok(())
        })
        .ok()?;
    let bytes = count
        .checked_mul(size_of::<BezPath>() + size_of::<usize>())?
        .checked_add(vertices.checked_mul(size_of::<kurbo::PathEl>())?)?;
    let credit = admit(bytes)?;
    let mut paths = Vec::new();
    paths.try_reserve_exact(count).ok()?;
    let mut owners = Vec::new();
    owners.try_reserve_exact(count).ok()?;
    if paths.capacity() > count || owners.capacity() > count {
        return None;
    }
    let background_owner = if background.is_some() {
        Some(source.mesh_triangle_count()?)
    } else {
        None
    };
    let mut visit_work = count;
    *remaining = remaining.checked_sub(visit_work)?;
    pixel
        .visit(&mut visit_work, cancelled, &mut |_, _, points, owner| {
            *remaining = remaining
                .checked_sub(points.len() + 1)
                .ok_or(PixelPartitionError::Limit)?;
            let mut elements = Vec::new();
            elements
                .try_reserve_exact(points.len() + 1)
                .map_err(|_| PixelPartitionError::Admission)?;
            if elements.capacity() > points.len() + 1 {
                return Err(PixelPartitionError::Admission);
            }
            for (i, point) in points.iter().enumerate() {
                let p = Point::new(point.x + origin.x, point.y + origin.y);
                elements.push(if i == 0 {
                    kurbo::PathEl::MoveTo(p)
                } else {
                    kurbo::PathEl::LineTo(p)
                });
            }
            elements.push(kurbo::PathEl::ClosePath);
            let index = if owner.sample_index == usize::MAX {
                background_owner.ok_or(PixelPartitionError::Invalid)?
            } else {
                owner.sample_index
            };
            paths.push(BezPath::from_vec(elements));
            owners.push(index);
            Ok(())
        })
        .ok()?;
    if paths.len() != count || cancelled() {
        return None;
    }
    super::SpatialMeshGeometry::admitted(paths, owners, rect, true, background_owner, credit, admit)
}

pub(super) fn sample(
    source: &NativeShadingSource,
    path: &BezPath,
    rule: hayro_interpret::FillRule,
    clips: &[SharedClip],
    x: i32,
    y: i32,
    rect: Rect,
    remaining: &mut usize,
    dense: &mut bool,
    cancelled: &dyn Fn() -> bool,
    admit: &dyn Fn(usize) -> Option<Box<dyn std::any::Any + Send + Sync>>,
) -> Option<Option<(NativeShadingSample, f64)>> {
    if cancelled() || clips.len() >= 64 || !source.mesh_has_patch_priority() {
        return None;
    }
    let origin = Point::new(f64::from(x), f64::from(y));
    let (pixel, background) = partition(source, origin, rect, remaining, dense, cancelled, admit)?;
    if pixel.fragments() == 0 {
        return Some(None);
    }
    let _regions = admit((1 + clips.len()).checked_mul(size_of::<(&BezPath, vello_cpu::peniko::Fill)>())?)?;
    let fill = |rule| match rule {
        hayro_interpret::FillRule::NonZero => vello_cpu::peniko::Fill::NonZero,
        hayro_interpret::FillRule::EvenOdd => vello_cpu::peniko::Fill::EvenOdd,
    };
    let mut regions = Vec::new();
    regions.try_reserve_exact(1 + clips.len()).ok()?;
    if regions.capacity() > 1 + clips.len() {
        return None;
    }
    regions.push((path, fill(rule)));
    regions.extend(clips.iter().map(|clip| (clip.path.as_ref(), fill(clip.fill))));
    let row = super::super::polygon_coverage::admitted_selected_polygon_domain_budget(
        &regions, y, rect.y0, rect.y1, remaining, cancelled, admit,
    )?;
    let mut area = 0.;
    let mut area_correction = 0.;
    let mut alpha = 0.;
    let mut alpha_correction = 0.;
    let mut integral = [0.; 4];
    let mut integral_correction = [0.; 4];
    let mut count = None;
    row.pixel_intersection_rect_fragments_budget(
        &regions,
        x,
        y,
        rect,
        remaining,
        cancelled,
        admit,
        &mut |fragment, work| {
            let clip = fragment
                .vertices
                .map(|p| Point::new(p.x - origin.x, p.y - origin.y));
            pixel
                .visit_convex_clip(&clip, work, cancelled, &mut |weight, center, _, owner, work| {
                    let point = Point::new(center.x + origin.x, center.y + origin.y);
                    let sample = if owner.sample_index == usize::MAX {
                        let value = background.as_ref().ok_or(PixelPartitionError::Invalid)?;
                        NativeShadingSample {
                            components: value.components,
                            count: value.count,
                            opacity: value.opacity,
                        }
                    } else {
                        source
                            .sample_mesh_triangle_with_budget(owner.sample_index, point, cancelled, work)
                            .map_err(|error| match error {
                                NativeShadingError::Cancelled => PixelPartitionError::Cancelled,
                                NativeShadingError::Admission => PixelPartitionError::Admission,
                                NativeShadingError::WorkLimit => PixelPartitionError::Limit,
                                _ => PixelPartitionError::Invalid,
                            })?
                    };
                    if sample.count == 0 || sample.count > 4 || count.is_some_and(|n| n != sample.count) {
                        return Err(PixelPartitionError::Invalid);
                    }
                    count = Some(sample.count);
                    add(&mut area, &mut area_correction, weight);
                    let opacity = weight * f64::from(sample.opacity);
                    add(&mut alpha, &mut alpha_correction, opacity);
                    for ((sum, correction), value) in integral
                        .iter_mut()
                        .zip(&mut integral_correction)
                        .zip(sample.components)
                    {
                        add(sum, correction, opacity * f64::from(value));
                    }
                    Ok(())
                })
                .map_err(|error| {
                    if error == PixelPartitionError::Limit && *work > 0 {
                        *dense = true;
                    }
                })
                .ok()
        },
    )?;
    if !area.is_finite() || !alpha.is_finite() || area > rect.width() * rect.height() + 1e-10 {
        return None;
    }
    if area <= 0. {
        return Some(None);
    }
    let components = if alpha == 0. {
        [0.; 4]
    } else {
        integral.map(|v| (v / alpha) as f32)
    };
    if components.iter().any(|v| !v.is_finite()) {
        return None;
    }
    Some(Some((
        NativeShadingSample {
            components,
            count: count?,
            opacity: (alpha / area) as f32,
        },
        area.min(rect.width() * rect.height()),
    )))
}

#[cfg(test)]
mod tests {
    use super::super::{Event, RecordedPaint, Recorder, record_into, replay_native_page_scene};
    use super::*;
    use kurbo::{Affine, Shape};
    fn record(bytes: &[u8]) -> Recorder {
        let scene = record_into(
            bytes.to_vec(),
            3,
            3,
            Affine::IDENTITY,
            Recorder {
                mask_probe: true,
                capture_native_shadings: true,
                integrate_native_mesh_boundaries: true,
                global_contour: true,
                native_polygon: true,
                ..Recorder::default()
            },
        );
        assert!(!scene.failed);
        scene
    }
    fn source(scene: &Recorder) -> &NativeShadingSource {
        scene
            .events
            .iter()
            .find_map(|event| match event {
                Event::Path(_, _, RecordedPaint::Shading(paint), _, _) => paint.native.as_ref(),
                _ => None,
            })
            .unwrap()
    }
    #[test]
    fn native_ordered_patch_full_pdf_exact_fold_and_partial_overlap() {
        for (bytes, row) in [
            (
                include_bytes!("../../../tests/fixtures/pdf/coons-folded-solid.pdf").as_slice(),
                [0, 0, 0, 255, 0, 0, 0, 128, 0, 0, 0, 0],
            ),
            (
                include_bytes!("../../../tests/fixtures/pdf/tensor-folded-solid.pdf").as_slice(),
                [0, 0, 0, 255, 0, 0, 0, 128, 0, 0, 0, 0],
            ),
            (
                include_bytes!("../../../tests/fixtures/pdf/coons-partial-overlap.pdf").as_slice(),
                [128, 0, 128, 255, 0, 0, 255, 255, 0, 0, 0, 0],
            ),
            (
                include_bytes!("../../../tests/fixtures/pdf/tensor-partial-overlap.pdf").as_slice(),
                [128, 0, 128, 255, 0, 0, 255, 255, 0, 0, 0, 0],
            ),
            (
                include_bytes!("../../../tests/fixtures/pdf/tensor-folded-background.pdf").as_slice(),
                [0, 0, 0, 255, 0, 128, 0, 255, 0, 255, 0, 255],
            ),
        ] {
            let scene = record(bytes);
            let source = source(&scene);
            if row[11] == 255 {
                let background = source
                    .mesh_background_sample(&|| false, &mut 100)
                    .unwrap()
                    .expect("pattern background retained by native source");
                assert_eq!(background.components, [0., 1., 0., 0.]);
            }
            let event = scene
                .events
                .iter()
                .find_map(|event| match event {
                    Event::Path(
                        path,
                        _,
                        RecordedPaint::Shading(_),
                        clips,
                        hayro_interpret::PathDrawMode::Fill(rule),
                    ) => Some((path, clips, *rule)),
                    _ => None,
                })
                .unwrap();
            for y in 0..3 {
                for x in 0..3 {
                    let result = super::super::sample_covered_shading_pixel(
                        source,
                        event.0,
                        event.2,
                        event.1,
                        x,
                        y,
                        &|| false,
                        &|_| Some(Box::new(())),
                    );
                    assert!(
                        result.is_some(),
                        "native patch area refused pixel {x},{y}, expected row={row:?}"
                    );
                    if let Some(Some((_, area))) = result {
                        assert!(area <= 1., "native coverage outside [0,1]: {area} at {x},{y}");
                    }
                }
            }
            let out = replay_native_page_scene(&scene, 3, 3, &source.color_space, &|| false, &|_| {
                Some(Box::new(()))
            })
            .unwrap();
            let mut actual = Vec::new();
            for pixel in &out.pixels {
                let components = if pixel.alpha() == 0. {
                    [0.; 4]
                } else {
                    pixel.native_components().unwrap()
                };
                actual.extend(components[..3].iter().map(|c| (c * 255.).round() as u8));
                actual.push((pixel.alpha() * 255.).round() as u8);
            }
            assert_eq!(actual, row.repeat(3));
        }
    }
    #[test]
    fn native_ordered_patch_sheared_fold_integrates_greatest_v_branch() {
        use super::super::authored_pdf_with_font;
        let order = [
            (0, 0),
            (0, 1),
            (0, 2),
            (0, 3),
            (1, 3),
            (2, 3),
            (3, 3),
            (3, 2),
            (3, 1),
            (3, 0),
            (2, 0),
            (1, 0),
            (1, 1),
            (1, 2),
            (2, 2),
            (2, 1),
        ];
        let mut payload = vec![0u8];
        for (u, v) in order {
            payload.extend([[0, 255, 85, 0][u], 68 * v + 17 * u].map(|v| v as u8));
        }
        for u in [0, 0, 255, 255] {
            payload.extend([u, 0, 0, 0]);
        }
        let hex = payload.iter().map(|b| format!("{b:02X}")).collect::<String>() + ">";
        let shading = format!(
            "<< /ShadingType 7 /ColorSpace /DeviceCMYK /BitsPerCoordinate 8 /BitsPerComponent 8 /BitsPerFlag 8 /Decode [0 3 0 3.75 0 1 0 1 0 1 0 1] /Filter /ASCIIHexDecode /Length {} >>\nstream\n{hex}\nendstream",
            hex.len()
        );
        let pdf = authored_pdf_with_font(true, 1., "/form Do", "", "/S1 sh", "<< >>", Some(&shading));
        let scene = record_into(
            pdf,
            3,
            4,
            Affine::IDENTITY,
            Recorder {
                capture_native_shadings: true,
                native_shadings_only: true,
                integrate_native_mesh_boundaries: true,
                global_contour: true,
                native_polygon: true,
                ..Recorder::default()
            },
        );
        assert!(!scene.failed);
        let source = source(&scene);
        // x(u)=9u-15u²+6u³; y=3v+0.75u. Throughout this pixel,
        // greatest v selects the increasing (smaller-u) branch.
        let mut lo = 0.;
        let mut hi = (5. - 7f64.sqrt()) / 6.;
        for _ in 0..64 {
            let u = (lo + hi) * 0.5;
            if 3. * u * (1. - u) * (3. - 2. * u) < 1. {
                lo = u;
            } else {
                hi = u;
            }
        }
        let u = (lo + hi) * 0.5;
        let expected = 4.5 * u * u - 10. * u * u * u + 4.5 * u * u * u * u;
        let path = Rect::new(0., 1., 1., 2.).to_path(0.1);
        let (value, area) = sample(
            source,
            &path,
            hayro_interpret::FillRule::NonZero,
            &[],
            0,
            1,
            Rect::new(0., 1., 1., 2.),
            &mut 40_000_000,
            &mut false,
            &|| false,
            &|_| Some(Box::new(())),
        )
        .unwrap()
        .unwrap();
        assert!((area - 1.).abs() < 1e-10);
        println!(
            "NATIVE_PATCH_AREA_INTEGRAL actual={} expected={expected:.17} error={:.17} area={area:.17}",
            value.components[0],
            (f64::from(value.components[0]) - expected).abs()
        );
        assert!(
            (f64::from(value.components[0]) - expected).abs() < 1. / 1024.,
            "sample={} expected={expected}",
            value.components[0]
        );
        let output = replay_native_page_scene(&scene, 3, 4, &source.color_space, &|| false, &|_| {
            Some(Box::new(()))
        })
        .unwrap();
        let pixel = output.pixels[3];
        assert!((pixel.alpha() - 1.).abs() < 1e-10);
        assert!((pixel.native_components().unwrap()[0] - expected).abs() < 1. / 1024.);
        use super::super::super::native_blend::{Model, Pixel};
        let clear = Pixel::from_native(Model::Cmyk, &[0.; 4], 0.).unwrap();
        let spatial = super::super::replay_spatial_scene_pixel(
            &scene,
            &source.color_space,
            clear,
            0,
            1,
            &|| false,
            &|_| Some(Box::new(())),
        )
        .expect("complete spatial sheared fold");
        assert!((spatial.alpha() - 1.).abs() < 1e-10);
        assert!(
            (spatial.native_components().unwrap()[0] - expected).abs() < 1. / 1024.,
            "spatial={:?} expected={expected}",
            spatial.native_components()
        );
    }

    #[test]
    fn native_spatial_patch_full_pdf_matches_byte_controls() {
        use super::super::super::native_blend::{Model, Pixel};
        for (bytes, row) in [
            (
                include_bytes!("../../../tests/fixtures/pdf/coons-folded-solid.pdf").as_slice(),
                [0, 0, 0, 255, 0, 0, 0, 128, 0, 0, 0, 0],
            ),
            (
                include_bytes!("../../../tests/fixtures/pdf/tensor-folded-solid.pdf").as_slice(),
                [0, 0, 0, 255, 0, 0, 0, 128, 0, 0, 0, 0],
            ),
            (
                include_bytes!("../../../tests/fixtures/pdf/coons-partial-overlap.pdf").as_slice(),
                [128, 0, 128, 255, 0, 0, 255, 255, 0, 0, 0, 0],
            ),
            (
                include_bytes!("../../../tests/fixtures/pdf/tensor-partial-overlap.pdf").as_slice(),
                [128, 0, 128, 255, 0, 0, 255, 255, 0, 0, 0, 0],
            ),
            (
                include_bytes!("../../../tests/fixtures/pdf/tensor-folded-background.pdf").as_slice(),
                [0, 0, 0, 255, 0, 128, 0, 255, 0, 255, 0, 255],
            ),
        ] {
            let scene = record(bytes);
            let source = source(&scene);
            let clear = Pixel::from_native(Model::Rgb, &[0.; 3], 0.).unwrap();
            let mut actual = Vec::new();
            for y in 0..3 {
                for x in 0..3 {
                    let pixel = super::super::replay_spatial_scene_pixel(
                        &scene,
                        &source.color_space,
                        clear,
                        x,
                        y,
                        &|| false,
                        &|_| Some(Box::new(())),
                    )
                    .unwrap_or_else(|| panic!("complete spatial patch pixel {x},{y} expected={row:?}"));
                    let components = if pixel.alpha() == 0. {
                        [0.; 4]
                    } else {
                        pixel.native_components().unwrap()
                    };
                    actual.extend(components[..3].iter().map(|c| (c * 255.).round() as u8));
                    actual.push((pixel.alpha() * 255.).round() as u8);
                }
            }
            assert_eq!(actual, row.repeat(3));
        }
    }

    #[test]
    fn native_spatial_patch_luminosity_mask_preserves_later_patch_owner() {
        use super::super::super::native_blend::{Model, Pixel};
        use hayro_interpret::color::{AlphaColor, Color};
        let order = [
            (0, 0),
            (0, 1),
            (0, 2),
            (0, 3),
            (1, 3),
            (2, 3),
            (3, 3),
            (3, 2),
            (3, 1),
            (3, 0),
            (2, 0),
            (1, 0),
            (1, 1),
            (1, 2),
            (2, 2),
            (2, 1),
        ];
        let mut payload = Vec::new();
        for (xs, color) in [
            ([0, 51, 102, 153], [255, 0, 0]),
            ([51, 102, 153, 204], [0, 0, 255]),
        ] {
            payload.push(0u8);
            for (u, v) in order {
                payload.extend([xs[u], (85 * v) as u8]);
            }
            for _ in 0..4 {
                payload.extend(color);
            }
        }
        let stream = |header: &[u8], body: &[u8]| {
            let mut value = header.to_vec();
            value.extend(format!(" /Length {} >>\nstream\n", body.len()).as_bytes());
            value.extend(body);
            value.extend(b"\nendstream");
            value
        };
        let objects=vec![
            b"<</Type/Catalog/Pages 2 0 R>>".to_vec(),
            b"<</Type/Pages/Kids[3 0 R]/Count 1>>".to_vec(),
            b"<</Type/Page/Parent 2 0 R/MediaBox[0 0 3 3]/Contents 4 0 R/Resources<</ExtGState<</GS1<</SMask<</S/Luminosity/G 5 0 R>>>>>>>>>>".to_vec(),
            stream(b"<<",b"/GS1 gs 0 1 0 rg 0 0 3 3 re f"),
            stream(b"<</Type/XObject/Subtype/Form/BBox[0 0 3 3]/Group<</S/Transparency/CS/DeviceRGB/I true>>/Resources<</Shading<</Sh1 6 0 R>>>>",b"/Sh1 sh"),
            stream(b"<</ShadingType 7/ColorSpace/DeviceRGB/BitsPerCoordinate 8/BitsPerComponent 8/BitsPerFlag 8/Decode[0 2.5 0 3 0 1 0 1 0 1]",&payload),
        ];
        let mut pdf = b"%PDF-1.7\n".to_vec();
        let mut offsets = Vec::new();
        for (i, object) in objects.iter().enumerate() {
            offsets.push(pdf.len());
            pdf.extend(format!("{} 0 obj\n", i + 1).as_bytes());
            pdf.extend(object);
            pdf.extend(b"\nendobj\n");
        }
        let xref = pdf.len();
        pdf.extend(b"xref\n0 7\n0000000000 65535 f \n");
        for offset in offsets {
            pdf.extend(format!("{offset:010} 00000 n \n").as_bytes());
        }
        pdf.extend(format!("trailer\n<</Size 7/Root 1 0 R>>\nstartxref\n{xref}\n%%EOF\n").as_bytes());
        let scene = record(&pdf);
        let rgb = Color::from_rgba(AlphaColor::WHITE);
        assert!(scene.events.iter().any(|e| matches!(e, Event::Mask(Some(_)))));
        let clear = Pixel::from_native(Model::Rgb, &[0.; 3], 0.).unwrap();
        for x in 0..3 {
            let pixel = super::super::replay_spatial_scene_pixel(
                &scene,
                rgb.color_space(),
                clear,
                x,
                0,
                &|| false,
                &|_| Some(Box::new(())),
            )
            .unwrap();
            let expected = [0.205, 0.11, 0.][x as usize];
            assert!(
                (pixel.alpha() - expected).abs() < 1e-10,
                "mask pixel {x}: {} != {expected}",
                pixel.alpha()
            );
            if expected > 0. {
                assert_eq!(&pixel.native_components().unwrap()[..3], &[0., 1., 0.]);
            }
        }
    }

    #[test]
    #[ignore = "same-process paired cache microbenchmark; not full-page throughput"]
    fn native_spatial_patch_cache_paired_measurement() {
        use super::super::super::native_blend::{Model, Pixel};
        let scene = record(include_bytes!(
            "../../../tests/fixtures/pdf/tensor-partial-overlap.pdf"
        ));
        let source = source(&scene);
        let clear = Pixel::from_native(Model::Rgb, &[0.; 3], 0.).unwrap();
        let admit = |_| Some(Box::new(()) as Box<dyn std::any::Any + Send + Sync>);
        let start = std::time::Instant::now();
        let geometry = std::sync::Arc::new(
            spatial_geometry(
                source,
                Rect::new(0., 0., 1., 1.),
                &mut 4_000_000,
                &|| false,
                &admit,
            )
            .unwrap(),
        );
        let preparation = start.elapsed().as_nanos();
        let event = scene
            .events
            .iter()
            .position(|e| matches!(e, Event::Path(_, _, RecordedPaint::Shading(_), _, _)))
            .unwrap();
        let cache = [(&scene, event, geometry)];
        let run = |cached| {
            let mut work = 4_000_000;
            let pixel = super::super::replay_spatial_scene_impl(
                &scene,
                &source.color_space,
                clear,
                0,
                0,
                None,
                &[],
                if cached { &cache } else { &[] },
                &mut work,
                &|| false,
                &admit,
            )
            .unwrap();
            assert_eq!(pixel.alpha(), 1.);
            assert_eq!(&pixel.native_components().unwrap()[..3], &[0.5, 0., 0.5]);
        };
        for _ in 0..2 {
            run(false);
            run(true);
        }
        println!("SPATIAL_CACHE_PREPARATION_NS={preparation}");
        for pair in 0..12 {
            let mut ns = [0u128; 2];
            for cached in if pair % 2 == 0 {
                [false, true]
            } else {
                [true, false]
            } {
                let start = std::time::Instant::now();
                for _ in 0..32 {
                    run(cached);
                }
                ns[usize::from(cached)] = start.elapsed().as_nanos() / 32;
            }
            println!(
                "SPATIAL_CACHE_PAIR pair={pair} uncached_ns={} cached_ns={}",
                ns[0], ns[1]
            );
        }
    }

    #[test]
    fn spatial_patch_geometry_admission_cancel_and_cache_domain_are_atomic() {
        use super::super::super::native_blend::{Model, Pixel};
        use std::{
            cell::Cell,
            sync::{
                Arc,
                atomic::{AtomicUsize, Ordering},
            },
        };
        let scene = record(include_bytes!(
            "../../../tests/fixtures/pdf/tensor-partial-overlap.pdf"
        ));
        let source = source(&scene);
        let rect = Rect::new(0., 0., 1., 1.);
        let used = Arc::new(AtomicUsize::new(0));
        let calls = Cell::new(0);
        let fail = Cell::new(0);
        struct Credit(Arc<AtomicUsize>, usize);
        impl Drop for Credit {
            fn drop(&mut self) {
                self.0.fetch_sub(self.1, Ordering::Relaxed);
            }
        }
        let admit = |bytes: usize| {
            calls.set(calls.get() + 1);
            if calls.get() == fail.get() {
                return None;
            }
            let next = used.load(Ordering::Relaxed).checked_add(bytes)?;
            if next > 8 * 1024 * 1024 {
                return None;
            }
            used.store(next, Ordering::Relaxed);
            Some(Box::new(Credit(used.clone(), bytes)) as Box<dyn std::any::Any + Send + Sync>)
        };
        let run =
            |cancelled: &dyn Fn() -> bool| spatial_geometry(source, rect, &mut 4_000_000, cancelled, &admit);
        let geometry = run(&|| false).unwrap();
        assert_eq!(geometry.domain, rect);
        assert!(geometry.patch_ordered);
        assert_eq!(geometry.paths.len(), geometry.owners.len());
        assert!(
            geometry
                .owners
                .iter()
                .all(|&i| i < source.mesh_triangle_count().unwrap())
        );
        let count = calls.get();
        drop(geometry);
        assert_eq!(used.load(Ordering::Relaxed), 0);
        for stop in 1..=count {
            calls.set(0);
            fail.set(stop);
            assert!(run(&|| false).is_none(), "allocation {stop}/{count}");
            assert_eq!(used.load(Ordering::Relaxed), 0);
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
                "cancel {stop}"
            );
            assert_eq!(used.load(Ordering::Relaxed), 0);
        }
        let event = scene
            .events
            .iter()
            .position(|e| matches!(e, Event::Path(_, _, RecordedPaint::Shading(_), _, _)))
            .unwrap();
        let geometry = std::sync::Arc::new(run(&|| false).unwrap());
        let cache = [(&scene, event, geometry.clone())];
        let clear = Pixel::from_native(Model::Rgb, &[0.; 3], 0.).unwrap();
        let replay = |x| {
            super::super::replay_spatial_scene_impl(
                &scene,
                &source.color_space,
                clear,
                x,
                0,
                None,
                &[],
                &cache,
                &mut 4_000_000,
                &|| false,
                &admit,
            )
        };
        let cached = replay(0).unwrap();
        assert_eq!(&cached.native_components().unwrap()[..3], &[0.5, 0., 0.5]);
        assert!(
            replay(1).is_none(),
            "pixel-local geometry cannot be reused in another pixel"
        );
        drop(cache);
        drop(geometry);
        assert_eq!(used.load(Ordering::Relaxed), 0);
        drop(run(&|| false).unwrap());
        assert_eq!(used.load(Ordering::Relaxed), 0);
    }

    #[test]
    fn native_ordered_patch_evenodd_clip_and_atomic_failure_retry() {
        use std::{
            cell::Cell,
            sync::{
                Arc,
                atomic::{AtomicUsize, Ordering},
            },
        };
        let scene = record(include_bytes!(
            "../../../tests/fixtures/pdf/tensor-partial-overlap.pdf"
        ));
        let source = source(&scene);
        let unit = Rect::new(0., 0., 1., 1.).to_path(0.1);
        let mut donut = unit.clone();
        donut.extend(
            Rect::new(0.25, 0.25, 0.75, 0.75)
                .to_path(0.1)
                .elements()
                .iter()
                .copied(),
        );
        let clips = [SharedClip {
            path: std::rc::Rc::new(donut),
            fill: hayro_interpret::FillRule::EvenOdd,
        }];
        let used = Arc::new(AtomicUsize::new(0));
        let calls = Cell::new(0);
        let fail = Cell::new(0);
        struct Credit(Arc<AtomicUsize>, usize);
        impl Drop for Credit {
            fn drop(&mut self) {
                self.0.fetch_sub(self.1, Ordering::Relaxed);
            }
        }
        let admit = |n| {
            calls.set(calls.get() + 1);
            if calls.get() == fail.get() {
                return None;
            }
            let next = used.load(Ordering::Relaxed).checked_add(n)?;
            if next > 8 * 1024 * 1024 {
                return None;
            }
            used.store(next, Ordering::Relaxed);
            Some(Box::new(Credit(used.clone(), n)) as Box<dyn std::any::Any + Send + Sync>)
        };
        let run = |work: &mut usize, cancelled: &dyn Fn() -> bool| {
            sample(
                source,
                &unit,
                hayro_interpret::FillRule::NonZero,
                &clips,
                0,
                0,
                Rect::new(0., 0., 1., 1.),
                work,
                &mut false,
                cancelled,
                &admit,
            )
        };
        let mut work = 4_000_000;
        let (value, area) = run(&mut work, &|| false).unwrap().unwrap();
        assert!((area - 0.75).abs() < 1e-12);
        assert_eq!(&value.components[..3], &[0.5, 0., 0.5]);
        assert!(work < 4_000_000);
        assert_eq!(used.load(Ordering::Relaxed), 0);
        let count = calls.get();
        for stop in 1..=count {
            calls.set(0);
            fail.set(stop);
            assert!(
                run(&mut 4_000_000, &|| false).is_none(),
                "allocation {stop}/{count}"
            );
            assert_eq!(used.load(Ordering::Relaxed), 0);
        }
        fail.set(0);
        let checks = Cell::new(0);
        run(&mut 4_000_000, &|| {
            checks.set(checks.get() + 1);
            false
        })
        .unwrap()
        .unwrap();
        for stop in 1..=checks.get() {
            let at = Cell::new(0);
            assert!(
                run(&mut 4_000_000, &|| {
                    at.set(at.get() + 1);
                    at.get() == stop
                })
                .is_none(),
                "cancel {stop}"
            );
            assert_eq!(used.load(Ordering::Relaxed), 0);
        }
        assert!(run(&mut 0, &|| false).is_none());
        run(&mut 4_000_000, &|| false).unwrap().unwrap();
        assert_eq!(used.load(Ordering::Relaxed), 0);
    }
}
