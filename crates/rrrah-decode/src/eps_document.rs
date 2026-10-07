//! Explicit experimental native EPS document rasterization.
use crate::*;
use rrrah_core::{DecodedRaster, MemoryBudget, RasterColorSpace, RasterError, RasterPixels};
#[derive(Debug, Clone, Copy)]
pub struct EpsDocumentLimits {
    pub max_source_bytes: usize,
    pub compile: EpsCompileLimits,
    pub vm: EpsVmLimits,
    pub graphics: EpsGraphicsLimits,
    pub raster: EpsRasterLimits,
}
impl Default for EpsDocumentLimits {
    fn default() -> Self {
        Self {
            max_source_bytes: 64 * 1024 * 1024,
            compile: EpsCompileLimits::default(),
            vm: EpsVmLimits::default(),
            graphics: EpsGraphicsLimits::default(),
            raster: EpsRasterLimits::default(),
        }
    }
}
#[derive(Debug, thiserror::Error)]
pub enum EpsDocumentError {
    #[error(transparent)]
    File(#[from] DecodeError),
    #[error("invalid EPS density or excessive raster dimensions")]
    Dimensions,
    #[error("EPS document rasterization cancelled")]
    Cancelled,
    #[error(transparent)]
    Source(#[from] EpsInspectError),
    #[error(transparent)]
    Bounds(#[from] EpsBoundsError),
    #[error(transparent)]
    Compile(#[from] EpsCompileError),
    #[error(transparent)]
    Vm(#[from] EpsVmError),
    #[error(transparent)]
    Render(#[from] EpsRasterError),
    #[error(transparent)]
    Raster(#[from] RasterError),
}
impl EpsDocumentError {
    /// Retains typed root-admission failures for foreground cache reclamation.
    pub fn memory_error(&self) -> Option<&rrrah_core::BufferError> {
        match self {
            Self::File(DecodeError::Memory(e))
            | Self::Compile(EpsCompileError::Memory(e))
            | Self::Vm(EpsVmError::Memory(e))
            | Self::Vm(EpsVmError::Graphics(EpsGraphicsError::Memory(e)))
            | Self::Raster(RasterError::Memory(e)) => Some(e),
            Self::Render(error) => match error {
                EpsRasterError::Memory(e)
                | EpsRasterError::Graphics(EpsGraphicsError::Memory(e))
                | EpsRasterError::Flatten(EpsFlattenError::Memory(e))
                | EpsRasterError::Fill(EpsFillError::Memory(e))
                | EpsRasterError::StrokeOutline(EpsStrokeError::Memory(e))
                | EpsRasterError::StrokePreparation(EpsStrokePrepareError::Graphics(
                    EpsGraphicsError::Memory(e),
                ))
                | EpsRasterError::StrokePreparation(EpsStrokePrepareError::Flatten(
                    EpsFlattenError::Memory(e),
                ))
                | EpsRasterError::StrokePreparation(EpsStrokePrepareError::Stroke(EpsStrokeError::Memory(
                    e,
                ))) => Some(e),
                _ => None,
            },
            _ => None,
        }
    }
}
/// Pixels per PostScript point, with transparent backdrop and explicit device
/// color policy. Source bytes belong to the caller; every native heap working
/// buffer and output uses `budget`. Unsupported operators refuse the document.
/// Embedded previews are never substituted for the original artwork.
pub fn decode_eps_document<F: FnMut() -> bool>(
    bytes: &[u8],
    pixels_per_point: f64,
    policy: EpsRasterColorPolicy,
    limits: EpsDocumentLimits,
    budget: &MemoryBudget,
    mut cancelled: F,
) -> Result<DecodedRaster, EpsDocumentError> {
    if cancelled() {
        return Err(EpsDocumentError::Cancelled);
    }
    if !pixels_per_point.is_finite() || pixels_per_point <= 0. {
        return Err(EpsDocumentError::Dimensions);
    }
    let source = inspect_eps_source(bytes, limits.max_source_bytes)?;
    let bounds = inspect_eps_bounds(&source, &mut cancelled)?;
    let dimension = |axis: usize| {
        let size = ((bounds.maximum[axis] - bounds.minimum[axis]) * pixels_per_point).ceil();
        if !size.is_finite() || size < 1. || size > f64::from(u32::MAX) {
            Err(EpsDocumentError::Dimensions)
        } else {
            Ok(size as u32)
        }
    };
    let width = dimension(0)?;
    let height = dimension(1)?;
    if u64::from(width) * u64::from(height) > limits.raster.fill.max_pixels as u64 {
        return Err(EpsDocumentError::Dimensions);
    }
    let viewport = [
        pixels_per_point,
        0.,
        0.,
        -pixels_per_point,
        -bounds.minimum[0] * pixels_per_point,
        bounds.maximum[1] * pixels_per_point,
    ];
    if !viewport.iter().all(|v| v.is_finite()) {
        return Err(EpsDocumentError::Dimensions);
    }
    let program = compile_eps_program(&source, limits.compile, budget, &mut cancelled)?;
    let EpsVectorExecution { evaluation, scene } =
        evaluate_eps_vectors(&program, limits.vm, limits.graphics, budget, &mut cancelled)?;
    drop(evaluation);
    drop(program);
    let pixels = rasterize_eps_scene(
        &scene,
        width,
        height,
        viewport,
        policy,
        limits.raster,
        budget,
        &mut cancelled,
    )?;
    drop(scene);
    if cancelled() {
        return Err(EpsDocumentError::Cancelled);
    }
    Ok(DecodedRaster::new(
        width,
        height,
        RasterPixels::Rgba8(pixels.into()),
        RasterColorSpace::Srgb,
    )?)
}
/// Bounded file entry: source and output share root admission. Nonzero image
/// selection is refused and request cancellation is honored throughout.
pub fn decode_eps_file_document(
    request: &DecodeRequest,
    pixels_per_point: f64,
    policy: EpsRasterColorPolicy,
    limits: EpsDocumentLimits,
    budget: &MemoryBudget,
) -> Result<DecodedRaster, EpsDocumentError> {
    request.check_cancelled()?;
    if request.image_index != 0 {
        return Err(DecodeError::UnsupportedImageIndex {
            index: request.image_index,
        }
        .into());
    }
    let source = crate::bounded_io::read_managed_capped(request, budget, limits.max_source_bytes as u64)?;
    let cancelled = || {
        request
            .cancellation
            .as_ref()
            .is_some_and(GenerationToken::is_cancelled)
    };
    let raster = decode_eps_document(&source, pixels_per_point, policy, limits, budget, cancelled)?;
    drop(source);
    request.check_cancelled()?;
    Ok(raster)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn bounded_file_source_cap_and_retained_output_share_root() {
        let path = std::env::temp_dir().join(format!(
            "rrrah-eps-import-{}-{}.eps",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::write(&path, CODE).unwrap();
        let request = DecodeRequest::new(&path);
        let policy = EpsRasterColorPolicy::DeviceGrayRgbAsSrgb;
        let refused = MemoryBudget::new(100_000);
        let mut small = limits();
        small.max_source_bytes = CODE.len() - 1;
        assert!(matches!(
            decode_eps_file_document(&request, 1., policy, small, &refused),
            Err(EpsDocumentError::File(DecodeError::InputTooLarge { .. }))
        ));
        assert_eq!(refused.peak(), 0);
        let tiny = MemoryBudget::new(CODE.len() as u64 - 1);
        assert!(decode_eps_file_document(&request, 1., policy, limits(), &tiny).is_err());
        assert_eq!(tiny.used(), 0);
        let root = MemoryBudget::new(100_000);
        let raster = decode_eps_file_document(&request, 1., policy, limits(), &root).unwrap();
        assert_eq!(root.used(), 64);
        assert!(root.peak() > CODE.len() as u64 + 64);
        drop(raster);
        assert_eq!(root.used(), 0);
        std::fs::remove_file(path).unwrap();
    }
    fn limits() -> EpsDocumentLimits {
        EpsDocumentLimits {
            vm: EpsVmLimits {
                max_operands: 64,
                max_dictionary_entries: 16,
                max_execution_frames: 8,
                max_work: 10000,
            },
            graphics: EpsGraphicsLimits {
                max_nodes: 16,
                max_paints: 4,
                max_saved_states: 0,
            },
            ..EpsDocumentLimits::default()
        }
    }
    const CODE:&[u8]=b"%!PS-Adobe-3.0 EPSF-3.0\n%%BoundingBox: -2 -3 2 1\n%%EndComments\n1 0 0 setrgbcolor -2 -3 moveto 2 -3 lineto 2 1 lineto -2 1 lineto closepath fill\n%%EOF\n";
    #[test]
    fn native_document_negative_origin_density_and_output_lifetime() {
        let root = MemoryBudget::new(100_000);
        for density in [1., 2.] {
            let raster = decode_eps_document(
                CODE,
                density,
                EpsRasterColorPolicy::DeviceGrayRgbAsSrgb,
                limits(),
                &root,
                || false,
            )
            .unwrap();
            let RasterPixels::Rgba8(pixels) = raster.pixels() else {
                panic!()
            };
            assert!(pixels.chunks_exact(4).all(|p| p == [255, 0, 0, 255]));
            assert_eq!(pixels.len(), (4. * density * 4. * density * 4.) as usize);
            assert_eq!(root.used(), pixels.len() as u64);
            let last = raster.clone();
            drop(raster);
            assert!(root.used() > 0);
            drop(last);
            assert_eq!(root.used(), 0);
        }
    }
    #[test]
    fn computed_numeric_coordinates_match_literal_document_pixels() {
        let computed = b"%!PS-Adobe-3.0 EPSF-3.0\n%%BoundingBox: -2 -3 2 1\n%%EndComments\n1 0 0 setrgbcolor -5 2 idiv -7 4 mod moveto 4 sqrt -3 lineto 2 1.2 floor lineto -2 0.6 round lineto closepath fill\n%%EOF\n";
        let root = MemoryBudget::new(100_000);
        let decode = |code| {
            decode_eps_document(
                code,
                1.,
                EpsRasterColorPolicy::DeviceGrayRgbAsSrgb,
                limits(),
                &root,
                || false,
            )
            .unwrap()
        };
        let expected = decode(CODE);
        let actual = decode(computed);
        let (RasterPixels::Rgba8(expected_pixels), RasterPixels::Rgba8(actual_pixels)) =
            (expected.pixels(), actual.pixels())
        else {
            panic!()
        };
        assert_eq!(&expected_pixels[..], &actual_pixels[..]);
        drop(expected);
        drop(actual);
        assert_eq!(root.used(), 0);
    }
    #[test]
    fn rectangle_operator_pixels_match_explicit_paths_without_save_slots() {
        let root = MemoryBudget::new(100_000);
        for (rect, path) in [
            (
                "-2 -3 4 4 rectfill",
                "-2 -3 moveto 2 -3 lineto 2 1 lineto -2 1 lineto closepath fill",
            ),
            (
                "2 1 -4 -4 rectfill",
                "2 1 moveto -2 1 lineto -2 -3 lineto 2 -3 lineto closepath fill",
            ),
            (
                "-1 -2 2 2 rectstroke",
                "-1 -2 moveto 1 -2 lineto 1 0 lineto -1 0 lineto closepath stroke",
            ),
            (
                "0 0 0 0 rectfill",
                "0 0 moveto 0 0 lineto 0 0 lineto 0 0 lineto closepath fill",
            ),
        ] {
            let decode = |body| {
                let code = format!(
                    "%!PS-Adobe-3.0 EPSF-3.0\n%%BoundingBox: -2 -3 2 1\n%%EndComments\n1 0 0 setrgbcolor 1 setlinewidth {body}\n%%EOF\n"
                );
                decode_eps_document(
                    code.as_bytes(),
                    1.,
                    EpsRasterColorPolicy::DeviceGrayRgbAsSrgb,
                    limits(),
                    &root,
                    || false,
                )
                .unwrap()
            };
            let actual = decode(rect);
            let expected = decode(path);
            let (RasterPixels::Rgba8(a), RasterPixels::Rgba8(e)) = (actual.pixels(), expected.pixels())
            else {
                panic!()
            };
            assert_eq!(&a[..], &e[..], "{rect}");
            drop(actual);
            drop(expected);
            assert_eq!(root.used(), 0);
        }
    }
    #[test]
    fn imported_rectangles_and_showpage_match_independent_ghostscript_pixels() {
        let reference: serde_json::Value = serde_json::from_str(include_str!(
            "../../../tests/fixtures/eps/import-pixel-ghostscript-reference.json"
        ))
        .unwrap();
        let cases = reference["cases"].as_array().unwrap();
        assert_eq!(cases.len(), 2);
        let root = MemoryBudget::new(32 * 1024 * 1024);
        for case in cases {
            let source = format!(
                "%!PS-Adobe-3.0 EPSF-3.0\n%%BoundingBox: 0 0 8 8\n%%EndComments\n{}\n%%EOF\n",
                case["source"].as_str().unwrap()
            );
            let raster = decode_eps_document(
                source.as_bytes(),
                1.,
                EpsRasterColorPolicy::DeviceGrayRgbAsSrgb,
                EpsDocumentLimits::default(),
                &root,
                || false,
            )
            .unwrap();
            let RasterPixels::Rgba8(pixels) = raster.pixels() else {
                panic!()
            };
            let rgb = case["rgb"].as_array().unwrap();
            assert_eq!(pixels.len(), 256);
            assert_eq!(rgb.len(), 192);
            for (i, pixel) in pixels.chunks_exact(4).enumerate() {
                let expected = [
                    rgb[i * 3].as_u64().unwrap() as u8,
                    rgb[i * 3 + 1].as_u64().unwrap() as u8,
                    rgb[i * 3 + 2].as_u64().unwrap() as u8,
                    255,
                ];
                assert_eq!(pixel, expected, "{} pixel {i}", case["name"]);
            }
            drop(raster);
            assert_eq!(root.used(), 0);
        }
    }
    #[test]
    fn imported_showpage_keeps_complete_document_pixels() {
        let root = MemoryBudget::new(100_000);
        let code = std::str::from_utf8(CODE)
            .unwrap()
            .replace("1 0 0 setrgbcolor", "showpage 1 0 0 setrgbcolor")
            .replace("%%EOF", "showpage /flush /showpage load def flush\n%%EOF");
        let decode = |bytes| {
            decode_eps_document(
                bytes,
                1.,
                EpsRasterColorPolicy::DeviceGrayRgbAsSrgb,
                limits(),
                &root,
                || false,
            )
            .unwrap()
        };
        let expected = decode(CODE);
        let actual = decode(code.as_bytes());
        let (RasterPixels::Rgba8(a), RasterPixels::Rgba8(e)) = (actual.pixels(), expected.pixels()) else {
            panic!()
        };
        assert_eq!(&a[..], &e[..]);
        drop(actual);
        drop(expected);
        assert_eq!(root.used(), 0);
    }
    #[test]
    fn document_errors_do_not_return_partial_preview_or_output() {
        let root = MemoryBudget::new(100_000);
        assert!(matches!(
            decode_eps_document(
                CODE,
                0.,
                EpsRasterColorPolicy::DeviceGrayRgbAsSrgb,
                limits(),
                &root,
                || false
            ),
            Err(EpsDocumentError::Dimensions)
        ));
        let mut small = limits();
        small.raster.fill.max_pixels = 15;
        assert!(matches!(
            decode_eps_document(
                CODE,
                1.,
                EpsRasterColorPolicy::DeviceGrayRgbAsSrgb,
                small,
                &root,
                || false
            ),
            Err(EpsDocumentError::Dimensions)
        ));
        assert_eq!(root.peak(), 0);
        let refused = MemoryBudget::new(1);
        assert!(
            decode_eps_document(
                CODE,
                1.,
                EpsRasterColorPolicy::DeviceGrayRgbAsSrgb,
                limits(),
                &refused,
                || false
            )
            .is_err()
        );
        assert_eq!(refused.used(), 0);
        assert!(
            decode_eps_document(
                CODE,
                1.,
                EpsRasterColorPolicy::DeviceGrayRgbAsSrgb,
                limits(),
                &root,
                || root.used() != 0
            )
            .is_err()
        );
        assert_eq!(root.used(), 0);
    }
}
