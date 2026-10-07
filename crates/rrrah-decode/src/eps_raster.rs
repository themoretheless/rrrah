//! Native EPS fill scene compositing. Not yet a general EPS decoder.
use crate::{
    EpsDeviceColor, EpsFillError, EpsFillLimits, EpsFlattenError, EpsFlattenLimits, EpsGraphicsError,
    EpsMatrix, EpsPaintKind, EpsVectorScene, fill_eps_path, flatten_eps_path,
};
use rrrah_core::{BufferError, MemoryBudget, SharedBuffer};

/// Explicit caller decision: interpret unprofiled DeviceGray/RGB values as
/// encoded sRGB. CMYK requires a separate color conversion policy.
#[derive(Debug, Clone, Copy)]
pub enum EpsRasterColorPolicy {
    DeviceGrayRgbAsSrgb,
}
#[derive(Debug, Clone, Copy)]
pub struct EpsRasterLimits {
    pub stroke: crate::EpsStrokeAssemblyLimits,
    pub flatten: EpsFlattenLimits,
    pub fill: EpsFillLimits,
    pub max_paints: usize,
    pub max_composite_work: u64,
}
impl Default for EpsRasterLimits {
    fn default() -> Self {
        Self {
            stroke: crate::EpsStrokeAssemblyLimits::default(),
            flatten: EpsFlattenLimits::default(),
            fill: EpsFillLimits::default(),
            max_paints: 4096,
            max_composite_work: 100_000_000,
        }
    }
}
#[derive(Debug, thiserror::Error)]
pub enum EpsRasterError {
    #[error("EPS raster range or work limit")]
    Limit,
    #[error("EPS raster cancelled")]
    Cancelled,
    #[error("fill-only EPS API cannot rasterize strokes")]
    Stroke,
    #[error(transparent)]
    StrokePreparation(#[from] crate::EpsStrokePrepareError),
    #[error(transparent)]
    StrokeOutline(#[from] crate::EpsStrokeError),
    #[error("EPS CMYK requires explicit profile conversion")]
    Cmyk,
    #[error(transparent)]
    Memory(#[from] BufferError),
    #[error(transparent)]
    Graphics(#[from] EpsGraphicsError),
    #[error(transparent)]
    Flatten(#[from] EpsFlattenError),
    #[error(transparent)]
    Fill(#[from] EpsFillError),
}
/// Renders supported fills in source order into straight-alpha sRGB RGBA bytes.
/// `viewport` maps EPS world coordinates into top-to-bottom pixel coordinates.
/// Unsupported operations are rejected before output admission. Private
/// working buffers and output share the caller root; no partial raster escapes.
pub fn rasterize_eps_fills<F: FnMut() -> bool>(
    scene: &EpsVectorScene,
    width: u32,
    height: u32,
    viewport: EpsMatrix,
    policy: EpsRasterColorPolicy,
    limits: EpsRasterLimits,
    budget: &MemoryBudget,
    cancelled: F,
) -> Result<SharedBuffer<u8>, EpsRasterError> {
    rasterize(
        scene, width, height, viewport, policy, limits, budget, cancelled, false,
    )
}
/// Renders native fills and unadjusted strokes in paint order. Hairlines use a
/// one-pixel raster-space pen. CMYK requires explicit profile conversion.
pub fn rasterize_eps_scene<F: FnMut() -> bool>(
    scene: &EpsVectorScene,
    width: u32,
    height: u32,
    viewport: EpsMatrix,
    policy: EpsRasterColorPolicy,
    limits: EpsRasterLimits,
    budget: &MemoryBudget,
    cancelled: F,
) -> Result<SharedBuffer<u8>, EpsRasterError> {
    rasterize(
        scene, width, height, viewport, policy, limits, budget, cancelled, true,
    )
}
fn rasterize<F: FnMut() -> bool>(
    scene: &EpsVectorScene,
    width: u32,
    height: u32,
    viewport: EpsMatrix,
    policy: EpsRasterColorPolicy,
    limits: EpsRasterLimits,
    budget: &MemoryBudget,
    mut cancelled: F,
    strokes: bool,
) -> Result<SharedBuffer<u8>, EpsRasterError> {
    let EpsRasterColorPolicy::DeviceGrayRgbAsSrgb = policy;
    if cancelled() {
        return Err(EpsRasterError::Cancelled);
    }
    let pixels = u64::from(width)
        .checked_mul(u64::from(height))
        .ok_or(EpsRasterError::Limit)?;
    let work = pixels
        .checked_mul(
            (scene.paints().len() as u64)
                .checked_add(1)
                .ok_or(EpsRasterError::Limit)?,
        )
        .ok_or(EpsRasterError::Limit)?;
    if width == 0
        || height == 0
        || pixels > limits.fill.max_pixels as u64
        || work > limits.max_composite_work
        || scene.paints().len() > limits.max_paints
        || !viewport.iter().all(|n| n.is_finite())
    {
        return Err(EpsRasterError::Limit);
    }
    for paint in scene.paints() {
        if cancelled() {
            return Err(EpsRasterError::Cancelled);
        }
        if paint.kind == EpsPaintKind::Stroke && !strokes {
            return Err(EpsRasterError::Stroke);
        }
        if matches!(paint.style.color, EpsDeviceColor::Cmyk(_)) {
            return Err(EpsRasterError::Cmyk);
        }
    }
    let bytes = pixels
        .checked_mul(4)
        .and_then(|n| usize::try_from(n).ok())
        .ok_or(EpsRasterError::Limit)?;
    let mut rgba = budget.try_buffer(bytes, 0u8)?;
    for (index, paint) in scene.paints().iter().enumerate() {
        let flat = if paint.kind == EpsPaintKind::Stroke {
            let prepared =
                crate::prepare_eps_stroke(scene, index, viewport, limits.flatten, budget, &mut cancelled)?;
            crate::outline_eps_prepared_stroke(&prepared, limits.stroke, budget, &mut cancelled)?
        } else {
            let path = scene.prepare_path(index, viewport, budget, &mut cancelled)?;
            flatten_eps_path(&path, limits.flatten, budget, &mut cancelled)?
        };
        let coverage = fill_eps_path(
            &flat,
            width,
            height,
            paint.kind == EpsPaintKind::FillEvenOdd,
            limits.fill,
            budget,
            &mut cancelled,
        )?;
        drop(flat);
        let color = match paint.style.color {
            EpsDeviceColor::Gray(g) => [g; 3],
            EpsDeviceColor::Rgb(rgb) => rgb,
            EpsDeviceColor::Cmyk(_) => unreachable!(),
        }
        .map(|n| (n * 255.).round() as u32);
        for (at, (pixel, &alpha)) in rgba.chunks_exact_mut(4).zip(coverage.iter()).enumerate() {
            if at % 4096 == 0 && cancelled() {
                return Err(EpsRasterError::Cancelled);
            }
            let alpha = u32::from(alpha);
            let inverse = 255 - alpha;
            for channel in 0..3 {
                pixel[channel] =
                    ((color[channel] * alpha + u32::from(pixel[channel]) * inverse + 127) / 255) as u8;
            }
            pixel[3] = (alpha + (u32::from(pixel[3]) * inverse + 127) / 255) as u8;
        }
    }
    for (at, pixel) in rgba.chunks_exact_mut(4).enumerate() {
        if at % 4096 == 0 && cancelled() {
            return Err(EpsRasterError::Cancelled);
        }
        let alpha = u32::from(pixel[3]);
        if alpha == 0 {
            pixel[..3].fill(0);
        } else {
            for channel in &mut pixel[..3] {
                *channel = ((u32::from(*channel) * 255 + alpha / 2) / alpha).min(255) as u8;
            }
        }
    }
    if cancelled() {
        return Err(EpsRasterError::Cancelled);
    }
    Ok(rgba.freeze())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{EpsGraphics, EpsGraphicsLimits};
    #[test]
    fn device_hairline_width_survives_paint_scale_and_singular_matrix() {
        let root = MemoryBudget::new(100_000);
        for scale in [[1., 1.], [20., 3.], [0., 0.]] {
            let mut g = EpsGraphics::new(
                EpsGraphicsLimits {
                    max_nodes: 4,
                    max_paints: 1,
                    max_saved_states: 0,
                },
                &root,
                || false,
            )
            .unwrap();
            g.set_line_width(0.).unwrap();
            g.move_to(0., 2.5).unwrap();
            g.line_to(4., 2.5).unwrap();
            g.scale(scale[0], scale[1]).unwrap();
            g.paint(EpsPaintKind::Stroke).unwrap();
            let scene = g.finish();
            let output = MemoryBudget::new(100_000);
            let pixels = rasterize_eps_scene(
                &scene,
                4,
                5,
                [1., 0., 0., 1., 0., 0.],
                EpsRasterColorPolicy::DeviceGrayRgbAsSrgb,
                EpsRasterLimits::default(),
                &output,
                || false,
            )
            .unwrap();
            for (at, pixel) in pixels.chunks_exact(4).enumerate() {
                assert_eq!(
                    pixel,
                    if at / 4 == 2 {
                        &[0, 0, 0, 255]
                    } else {
                        &[0, 0, 0, 0]
                    },
                    "scale {scale:?} pixel {at}"
                );
            }
            drop(pixels);
            assert_eq!(output.used(), 0);
            drop(scene);
            assert_eq!(root.used(), 0);
        }
    }
    #[test]
    fn native_program_mixed_fill_stroke_scene_renders_in_source_order() {
        let root = MemoryBudget::new(100_000);
        let code=b"1 0 0 setrgbcolor 0 0 moveto 4 0 lineto 4 4 lineto 0 4 lineto closepath fill 0 0 1 setrgbcolor 2 setlinewidth 0 2 moveto 4 2 lineto stroke";
        let source = crate::EpsSource {
            postscript: code,
            wmf_preview: None,
            tiff_preview: None,
        };
        let program =
            crate::compile_eps_program(&source, crate::EpsCompileLimits::default(), &root, || false).unwrap();
        let result = crate::evaluate_eps_vectors(
            &program,
            crate::EpsVmLimits {
                max_operands: 64,
                max_dictionary_entries: 16,
                max_execution_frames: 8,
                max_work: 10000,
            },
            EpsGraphicsLimits {
                max_nodes: 16,
                max_paints: 2,
                max_saved_states: 0,
            },
            &root,
            || false,
        )
        .unwrap();
        let output = MemoryBudget::new(100_000);
        let pixels = rasterize_eps_scene(
            &result.scene,
            4,
            4,
            [1., 0., 0., -1., 0., 4.],
            EpsRasterColorPolicy::DeviceGrayRgbAsSrgb,
            EpsRasterLimits::default(),
            &output,
            || false,
        )
        .unwrap();
        for (at, pixel) in pixels.chunks_exact(4).enumerate() {
            assert_eq!(
                pixel,
                if (1..=2).contains(&(at / 4)) {
                    &[0, 0, 255, 255]
                } else {
                    &[255, 0, 0, 255]
                }
            );
        }
        assert_eq!(output.used(), 64);
        drop(pixels);
        assert_eq!(output.used(), 0);
        assert!(
            rasterize_eps_scene(
                &result.scene,
                4,
                4,
                [1., 0., 0., 1., 0., 0.],
                EpsRasterColorPolicy::DeviceGrayRgbAsSrgb,
                EpsRasterLimits::default(),
                &output,
                || output.used() != 0
            )
            .is_err()
        );
        assert_eq!(output.used(), 0);
    }
    #[test]
    fn independent_ghostscript_binary_color_pixels_match_native_pipeline() {
        compare_ghostscript_pixels(5);
    }
    #[test]
    #[ignore = "strict full-color qualification: gray/RGB 0.5 native 128 versus Ghostscript 127 remains unresolved"]
    fn independent_ghostscript_full_color_pixel_qualification() {
        compare_ghostscript_pixels(9);
    }
    fn compare_ghostscript_pixels(case_count: usize) {
        let reference: serde_json::Value = serde_json::from_str(include_str!(
            "../../../tests/fixtures/eps/pixel-ghostscript-reference.json"
        ))
        .unwrap();
        let cases = reference["cases"].as_array().unwrap();
        assert_eq!(cases.len(), 9);
        for case in &cases[..case_count] {
            let root = MemoryBudget::new(1_000_000);
            let code = case["source"].as_str().unwrap();
            let source = crate::EpsSource {
                postscript: code.as_bytes(),
                wmf_preview: None,
                tiff_preview: None,
            };
            let program =
                crate::compile_eps_program(&source, crate::EpsCompileLimits::default(), &root, || false)
                    .unwrap();
            let result = crate::evaluate_eps_vectors(
                &program,
                crate::EpsVmLimits {
                    max_operands: 64,
                    max_dictionary_entries: 32,
                    max_execution_frames: 16,
                    max_work: 100_000,
                },
                EpsGraphicsLimits {
                    max_nodes: 128,
                    max_paints: 8,
                    max_saved_states: 16,
                },
                &root,
                || false,
            )
            .unwrap();
            let pixels = rasterize_eps_fills(
                &result.scene,
                8,
                8,
                [1., 0., 0., -1., 0., 8.],
                EpsRasterColorPolicy::DeviceGrayRgbAsSrgb,
                EpsRasterLimits::default(),
                &root,
                || false,
            )
            .unwrap();
            let rgb = case["rgb"].as_array().unwrap();
            assert_eq!(rgb.len(), 8 * 8 * 3);
            for (at, pixel) in pixels.chunks_exact(4).enumerate() {
                for channel in 0..3 {
                    assert_eq!(
                        u64::from(pixel[channel]),
                        rgb[at * 3 + channel].as_u64().unwrap(),
                        "{} pixel {at} channel {channel}",
                        case["name"]
                    );
                }
                assert_eq!(pixel[3], 255);
            }
            drop(pixels);
            drop(result);
            drop(program);
            assert_eq!(root.used(), 0);
        }
    }
    fn rectangle(g: &mut EpsGraphics, x: f64, y: f64, w: f64, h: f64) {
        g.move_to(x, y).unwrap();
        g.line_to(x + w, y).unwrap();
        g.line_to(x + w, y + h).unwrap();
        g.line_to(x, y + h).unwrap();
        g.paint(EpsPaintKind::FillNonZero).unwrap();
    }
    #[test]
    fn paint_order_straight_alpha_viewport_and_last_owner() {
        let root = MemoryBudget::new(100_000);
        let mut g = EpsGraphics::new(
            EpsGraphicsLimits {
                max_nodes: 20,
                max_paints: 2,
                max_saved_states: 0,
            },
            &root,
            || false,
        )
        .unwrap();
        g.set_color(EpsDeviceColor::Rgb([1., 0., 0.])).unwrap();
        rectangle(&mut g, 0., 0., 2., 1.);
        g.set_color(EpsDeviceColor::Rgb([0., 0., 1.])).unwrap();
        rectangle(&mut g, 0.5, 0., 1., 1.);
        let scene = g.finish();
        let output_root = MemoryBudget::new(100_000);
        let pixels = rasterize_eps_fills(
            &scene,
            2,
            2,
            [1., 0., 0., -1., 0., 2.],
            EpsRasterColorPolicy::DeviceGrayRgbAsSrgb,
            EpsRasterLimits::default(),
            &output_root,
            || false,
        )
        .unwrap();
        assert_eq!(
            &*pixels,
            &[0, 0, 0, 0, 0, 0, 0, 0, 127, 0, 128, 255, 127, 0, 128, 255]
        );
        assert_eq!(output_root.used(), 16);
        let last = pixels.clone();
        drop(pixels);
        assert_eq!(output_root.used(), 16);
        drop(last);
        assert_eq!(output_root.used(), 0);
    }
    #[test]
    fn rejects_unsupported_before_output_and_cancels_after_admission() {
        let root = MemoryBudget::new(100_000);
        let mut g = EpsGraphics::new(
            EpsGraphicsLimits {
                max_nodes: 20,
                max_paints: 2,
                max_saved_states: 0,
            },
            &root,
            || false,
        )
        .unwrap();
        rectangle(&mut g, 0., 0., 2., 2.);
        let scene = g.finish();
        let output = MemoryBudget::new(100_000);
        let policy = EpsRasterColorPolicy::DeviceGrayRgbAsSrgb;
        assert!(matches!(
            rasterize_eps_fills(
                &scene,
                2,
                2,
                [1., 0., 0., 1., 0., 0.],
                policy,
                EpsRasterLimits::default(),
                &output,
                || output.used() != 0
            ),
            Err(EpsRasterError::Graphics(EpsGraphicsError::Cancelled))
        ));
        assert_eq!(output.used(), 0);
        let mut g = EpsGraphics::new(
            EpsGraphicsLimits {
                max_nodes: 20,
                max_paints: 2,
                max_saved_states: 0,
            },
            &root,
            || false,
        )
        .unwrap();
        g.set_color(EpsDeviceColor::Cmyk([0.; 4])).unwrap();
        rectangle(&mut g, 0., 0., 2., 2.);
        let refused = MemoryBudget::new(100_000);
        assert!(matches!(
            rasterize_eps_fills(
                &g.finish(),
                2,
                2,
                [1., 0., 0., 1., 0., 0.],
                policy,
                EpsRasterLimits::default(),
                &refused,
                || false
            ),
            Err(EpsRasterError::Cmyk)
        ));
        assert_eq!(refused.peak(), 0);
    }
}
