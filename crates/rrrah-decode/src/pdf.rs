//! Selected PDF pages at one pixel per point; managed major buffers, not parser scratch.
//! DeviceCMYK currently uses Hayro's fixed CGATS001Compat-v2-micro ICC profile.
//! Marking the rendered buffer sRGB describes the output encoding; it does not
//! qualify source color conversion. The external AI and CMYK patch comparisons
//! in docs/research record remaining color differences against Poppler.
use crate::{DecodeError, DecodeRequest, RasterDecodeError};
use rrrah_core::{DecodedRaster, MemoryBudget, RasterColorSpace, RasterPixels};
use std::sync::{
    Arc,
    atomic::{AtomicU8, Ordering},
};
pub(crate) fn has_magic(bytes: &[u8]) -> bool {
    bytes.starts_with(b"%PDF-")
}
fn bad(reason: &'static str) -> RasterDecodeError {
    RasterDecodeError::InvalidPdf(reason)
}
// Hayro 0.7 silently drops unknown font subtypes. Follow invoked forms with
// matching resource inheritance; depth and total operations bound cycles/work.
fn preflight_fonts(
    mut operations: hayro::hayro_syntax::content::TypedIter<'_>,
    resources: &hayro::hayro_syntax::page::Resources<'_>,
    request: &DecodeRequest,
    depth: usize,
    processed: &mut usize,
) -> Result<(), RasterDecodeError> {
    use hayro::hayro_syntax::{
        content::{TypedIter, ops::TypedInstruction},
        object::{Dict, Name},
        page::Resources,
    };
    if depth > 32 {
        return Err(bad("form nesting limit"));
    }
    while let Some(operation) = operations.next() {
        request.check_cancelled()?;
        *processed += 1;
        if *processed > 1_000_000 {
            return Err(bad("content operation limit"));
        }
        match operation {
            TypedInstruction::TextFont(font) => {
                if let Some(dictionary) = resources.get_font(font.0) {
                    let subtype = dictionary
                        .get::<Name<'_>>(b"Subtype")
                        .ok_or_else(|| bad("unsupported font"))?;
                    if ![
                        b"Type1".as_slice(),
                        b"MMType1",
                        b"TrueType",
                        b"OpenType",
                        b"Type0",
                        b"Type3",
                    ]
                    .contains(&subtype.as_ref())
                    {
                        return Err(bad("unsupported font"));
                    }
                }
            }
            TypedInstruction::XObject(object) => {
                let stream = resources
                    .get_x_object(object.0)
                    .ok_or_else(|| bad("missing XObject resource"))?;
                let dictionary = stream.dict();
                let subtype = dictionary
                    .get::<Name<'_>>(b"Subtype")
                    .ok_or_else(|| bad("unsupported XObject subtype"))?;
                if ![b"Form".as_slice(), b"Image"].contains(&subtype.as_ref()) {
                    return Err(bad("unsupported XObject subtype"));
                }
                if subtype.as_ref() == b"Form" {
                    let bounds = dictionary
                        .get::<[f32; 4]>(b"BBox")
                        .ok_or_else(|| bad("invalid form geometry"))?;
                    if !bounds.iter().all(|value| value.is_finite()) {
                        return Err(bad("invalid form geometry"));
                    }
                    let decoded = stream.decoded().map_err(|_| bad("form stream decode failure"))?;
                    let child = Resources::from_parent(
                        dictionary.get::<Dict<'_>>(b"Resources").unwrap_or_default(),
                        resources.clone(),
                    );
                    preflight_fonts(TypedIter::new(&decoded), &child, request, depth + 1, processed)?;
                }
            }
            _ => {}
        }
    }
    Ok(())
}
pub(crate) fn decode(bytes: &[u8], request: &DecodeRequest) -> Result<DecodedRaster, RasterDecodeError> {
    request.check_cancelled()?;
    if !has_magic(bytes) {
        return Err(bad("signature"));
    }
    let budget = request
        .memory_budget
        .clone()
        .unwrap_or_else(|| MemoryBudget::new(crate::raster::MAX_RASTER_BYTES));
    // The PDF parser owns its source. Admit that copy before allocating it.
    let mut source_owner = budget
        .try_reserve(bytes.len() as u64)
        .map_err(DecodeError::from)?;
    let mut source = Vec::new();
    source
        .try_reserve_exact(bytes.len())
        .map_err(|error| DecodeError::from(rrrah_core::BufferError::Allocate(error)))?;
    source_owner
        .ensure_bytes(source.capacity() as u64)
        .map_err(DecodeError::from)?;
    source.extend_from_slice(bytes);
    let pdf = hayro::hayro_syntax::Pdf::new(source).map_err(|_| bad("parse or encryption"))?;
    let count = pdf.pages().len();
    if count == 0 || count > 4096 {
        return Err(bad("page count"));
    }
    let page = pdf
        .pages()
        .get(request.image_index)
        .ok_or(DecodeError::UnsupportedImageIndex {
            index: request.image_index,
        })?;
    preflight_fonts(page.typed_operations(), page.resources(), request, 0, &mut 0)?;
    let unit = if page.raw().contains_key(b"UserUnit") {
        page.raw()
            .get::<f32>(b"UserUnit")
            .ok_or_else(|| bad("invalid UserUnit"))?
    } else {
        1.
    };
    if !unit.is_finite() || unit <= 0. {
        return Err(bad("invalid UserUnit"));
    }
    let (w, h) = page.render_dimensions();
    let (w, h) = (w * unit, h * unit);
    // Leave room for the renderer's u16 tile-edge arithmetic (vello_common).
    if !w.is_finite() || !h.is_finite() || w <= 0. || h <= 0. || w > 65500. || h > 65500. {
        return Err(bad("page geometry"));
    }
    // Include the partially covered edge pixel without rescaling page artwork.
    let (width, height) = (w.ceil() as u16, h.ceil() as u16);
    let length = u64::from(width) * u64::from(height) * 4;
    if length > crate::raster::MAX_RASTER_BYTES {
        return Err(RasterDecodeError::OutputTooLarge);
    }
    let mut final_owner = budget.try_reserve(length).map_err(DecodeError::from)?;
    let _pixmap = budget.try_reserve(length).map_err(DecodeError::from)?;
    request.check_cancelled()?;
    let render_warnings = Arc::new(AtomicU8::new(0));
    let warnings = render_warnings.clone();
    let settings = hayro::hayro_interpret::InterpreterSettings {
        warning_sink: Arc::new(move |warning| {
            use hayro::hayro_interpret::InterpreterWarning;
            let flag = match warning {
                InterpreterWarning::UnsupportedFont => 1,
                InterpreterWarning::ImageDecodeFailure => 2,
            };
            warnings.fetch_or(flag, Ordering::Relaxed);
        }),
        ..Default::default()
    };
    let pixmap = hayro::render(
        page,
        &hayro::RenderCache::new(),
        &settings,
        &hayro::RenderSettings {
            width: Some(width),
            height: Some(height),
            x_scale: unit,
            y_scale: unit,
            ..Default::default()
        },
    );
    request.check_cancelled()?;
    let warnings = render_warnings.load(Ordering::Relaxed);
    if warnings & 2 != 0 {
        return Err(bad("embedded image decode failure"));
    }
    if warnings & 1 != 0 {
        return Err(bad("unsupported font"));
    }
    let rendered = pixmap.data_as_u8_slice();
    if rendered.len() as u64 != length {
        return Err(bad("render extent"));
    }
    let mut values = Vec::new();
    values
        .try_reserve_exact(rendered.len())
        .map_err(|error| DecodeError::from(rrrah_core::BufferError::Allocate(error)))?;
    final_owner
        .ensure_bytes(values.capacity() as u64)
        .map_err(DecodeError::from)?;
    values.extend_from_slice(rendered);
    for (index, pixel) in values.chunks_exact_mut(4).enumerate() {
        if index % 16384 == 0 {
            request.check_cancelled()?;
        }
        let alpha = u32::from(pixel[3]);
        for channel in &mut pixel[..3] {
            *channel = if alpha == 0 {
                0
            } else {
                ((u32::from(*channel) * 255 + alpha / 2) / alpha).min(255) as u8
            };
        }
    }
    let values = final_owner.try_adopt(values).map_err(DecodeError::from)?;
    Ok(DecodedRaster::new(
        u32::from(width),
        u32::from(height),
        RasterPixels::Rgba8(values.into()),
        RasterColorSpace::Srgb,
    )?
    .with_image_selection(request.image_index, count)?)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn unsupported_invoked_object_kind_refuses_without_rendering() {
        let original = std::str::from_utf8(include_bytes!(
            "../../../tests/fixtures/pdf/nested-inherited-font.pdf"
        ))
        .unwrap();
        for replacement in ["", "/Subtype /Unknown", "/Subtype /PS", "/Subtype 42"] {
            let bytes = original.replace("/Subtype /Form", replacement);
            let budget = MemoryBudget::new(65536);
            let mut request = DecodeRequest::new("unknown-object.pdf");
            request.memory_budget = Some(budget.clone());
            assert!(matches!(
                decode(bytes.as_bytes(), &request),
                Err(RasterDecodeError::InvalidPdf("unsupported XObject subtype"))
            ));
            assert_eq!(budget.peak(), bytes.len() as u64);
            assert_eq!(budget.used(), 0);
        }
    }

    #[test]
    fn malformed_invoked_form_geometry_refuses_without_output_reservation() {
        let original = std::str::from_utf8(include_bytes!(
            "../../../tests/fixtures/pdf/nested-inherited-font.pdf"
        ))
        .unwrap();
        for replacement in ["", "/BBox /Invalid", "/BBox [0 0 100]"] {
            let bytes = original.replace("/BBox [0 0 100 20]", replacement);
            let budget = MemoryBudget::new(65536);
            let mut request = DecodeRequest::new("invalid-form-geometry.pdf");
            request.memory_budget = Some(budget.clone());
            assert!(matches!(
                decode(bytes.as_bytes(), &request),
                Err(RasterDecodeError::InvalidPdf("invalid form geometry"))
            ));
            assert_eq!(budget.peak(), bytes.len() as u64);
            assert_eq!(budget.used(), 0);
        }
    }

    #[test]
    fn missing_invoked_object_refuses_in_page_and_inherited_form() {
        let original = std::str::from_utf8(include_bytes!(
            "../../../tests/fixtures/pdf/nested-inherited-font.pdf"
        ))
        .unwrap();
        for bytes in [
            original.replace("/Fm0 Do", "/Gone Do"),
            original.replace("BT /F0 12 Tf 1 1 Td (Visible text) Tj ET", "/Gone Do"),
        ] {
            let budget = MemoryBudget::new(65536);
            let mut request = DecodeRequest::new("missing-object.pdf");
            request.memory_budget = Some(budget.clone());
            assert!(matches!(
                decode(bytes.as_bytes(), &request),
                Err(RasterDecodeError::InvalidPdf("missing XObject resource"))
            ));
            assert_eq!(budget.peak(), bytes.len() as u64);
            assert_eq!(budget.used(), 0);
        }
    }

    #[test]
    fn nested_form_fonts_inherit_resources_and_cycles_refuse() {
        for (bytes, failure) in [
            (
                include_bytes!("../../../tests/fixtures/pdf/nested-unsupported-font.pdf").as_slice(),
                Some("unsupported font"),
            ),
            (
                include_bytes!("../../../tests/fixtures/pdf/cyclic-form.pdf").as_slice(),
                Some("form nesting limit"),
            ),
            (
                include_bytes!("../../../tests/fixtures/pdf/nested-inherited-font.pdf").as_slice(),
                None,
            ),
        ] {
            let budget = MemoryBudget::new(65536);
            let mut request = DecodeRequest::new("nested-form.pdf");
            request.memory_budget = Some(budget.clone());
            let result = decode(bytes, &request);
            if let Some(reason) = failure {
                assert!(matches!(result, Err(RasterDecodeError::InvalidPdf(found)) if found == reason));
                assert_eq!(budget.peak(), bytes.len() as u64);
            } else {
                let image = result.unwrap();
                let RasterPixels::Rgba8(pixels) = image.pixels() else {
                    panic!()
                };
                assert!(pixels.chunks_exact(4).any(|pixel| pixel[3] != 0));
            }
            assert_eq!(budget.used(), 0);
        }
    }

    #[test]
    fn unsupported_selected_font_refuses_before_render_allocation() {
        let bytes = include_bytes!("../../../tests/fixtures/pdf/unsupported-font.pdf");
        let budget = MemoryBudget::new(16384);
        let mut request = DecodeRequest::new("unsupported-font.pdf");
        request.memory_budget = Some(budget.clone());
        assert!(matches!(
            decode(bytes, &request),
            Err(RasterDecodeError::InvalidPdf("unsupported font"))
        ));
        assert_eq!(budget.peak(), bytes.len() as u64);
        assert_eq!(budget.used(), 0);
    }

    #[test]
    fn corrupt_embedded_image_refuses_incomplete_render_and_releases_credit() {
        let bytes = include_bytes!("../../../tests/fixtures/pdf/corrupt-embedded-jpeg.pdf");
        let budget = MemoryBudget::new(16384);
        let mut request = DecodeRequest::new("corrupt-image.pdf");
        request.memory_budget = Some(budget.clone());
        assert!(matches!(
            decode(bytes, &request),
            Err(RasterDecodeError::InvalidPdf("embedded image decode failure"))
        ));
        assert!(budget.peak() >= bytes.len() as u64 + 800);
        assert_eq!(budget.used(), 0);
    }

    #[test]
    fn renderer_overflow_extents_are_rejected_before_output_allocation() {
        let original =
            std::str::from_utf8(include_bytes!("../../../tests/fixtures/pdf/geometry-0.pdf")).unwrap();
        for extent in ["65534.5 0.5", "0.5 65534.5"] {
            let bytes = original.replace("/MediaBox [0 0 10 20]", &format!("/MediaBox [0 0 {extent}]"));
            let budget = MemoryBudget::new(1024 * 1024);
            let mut request = DecodeRequest::new("excessive-extent.pdf");
            request.memory_budget = Some(budget.clone());
            assert!(matches!(
                decode(bytes.as_bytes(), &request),
                Err(RasterDecodeError::InvalidPdf("page geometry"))
            ));
            assert_eq!(budget.peak(), bytes.len() as u64);
            assert_eq!(budget.used(), 0);
        }
    }

    #[test]
    fn fractional_page_extents_keep_partial_edge_pixels() {
        let original =
            std::str::from_utf8(include_bytes!("../../../tests/fixtures/pdf/geometry-0.pdf")).unwrap();
        for (extent, expected) in [
            ("9.25 19.5", (10, 20)),
            ("0.25 0.5", (1, 1)),
            ("65499.5 0.5", (65500, 1)),
        ] {
            let bytes = original.replace("/MediaBox [0 0 10 20]", &format!("/MediaBox [0 0 {extent}]"));
            let budget = MemoryBudget::new(1024 * 1024);
            let mut request = DecodeRequest::new("fractional.pdf");
            request.memory_budget = Some(budget.clone());
            let image = decode(bytes.as_bytes(), &request).unwrap();
            assert_eq!((image.width(), image.height()), expected);
            let RasterPixels::Rgba8(pixels) = image.pixels() else {
                panic!()
            };
            assert_eq!(pixels.len(), expected.0 as usize * expected.1 as usize * 4);
            if expected == (10, 20) {
                assert!(pixels[(9 * 4) + 3] > 0, "fractional right edge lost");
                assert!(pixels[(19 * 10 * 4) + 3] > 0, "fractional bottom edge lost");
            }
            drop(image);
            assert_eq!(budget.used(), 0);
        }
    }

    #[test]
    fn invalid_and_excessive_user_units_refuse_without_retained_credit() {
        let original = include_bytes!("../../../tests/fixtures/pdf/geometry-unit.pdf");
        let text = std::str::from_utf8(original).unwrap();
        for value in ["0", "-1", "/Invalid", "100000"] {
            // Hayro repairs xref offsets; the page object itself remains intact.
            let bytes = text.replace("/UserUnit 2", &format!("/UserUnit {value}"));
            let budget = MemoryBudget::new(16384);
            let mut request = DecodeRequest::new("invalid-unit.pdf");
            request.memory_budget = Some(budget.clone());
            let expected = if value == "100000" {
                "page geometry"
            } else {
                "invalid UserUnit"
            };
            assert!(
                matches!(decode(bytes.as_bytes(), &request),
                Err(RasterDecodeError::InvalidPdf(reason)) if reason == expected),
                "{value}"
            );
            assert_eq!(budget.used(), 0, "{value}");
            assert_eq!(
                budget.peak(),
                bytes.len() as u64,
                "render output reserved for {value}"
            );
        }
    }

    #[test]
    fn page_user_unit_scales_geometry_and_pixels() {
        let bytes = include_bytes!("../../../tests/fixtures/pdf/geometry-unit.pdf");
        let mut request = DecodeRequest::new("unit.pdf");
        let budget = MemoryBudget::new(16384);
        request.memory_budget = Some(budget.clone());
        let image = decode(bytes, &request).unwrap();
        assert_eq!((image.width(), image.height()), (20, 40));
        let RasterPixels::Rgba8(values) = image.pixels() else {
            panic!()
        };
        for (i, p) in values.chunks_exact(4).enumerate() {
            assert_eq!(
                p,
                if i / 20 < 20 {
                    &[0, 255, 0, 255]
                } else {
                    &[255, 0, 0, 255]
                }
            );
        }
        drop(image);
        assert_eq!(budget.used(), 0);
    }

    #[test]
    fn page_rotation_preserves_extent_and_matches_poppler() {
        let bytes = include_bytes!("../../../tests/fixtures/pdf/rotate90.pdf");
        let expected = include_bytes!("../../../tests/fixtures/pdf/rotate90.rgba");
        let budget = MemoryBudget::new(16384);
        let mut request = DecodeRequest::new("rotated.pdf");
        request.memory_budget = Some(budget.clone());
        let image = decode(bytes, &request).unwrap();
        assert_eq!((image.width(), image.height()), (20, 10));
        let RasterPixels::Rgba8(values) = image.pixels() else {
            panic!()
        };
        assert_eq!(&**values, expected.as_slice());
        drop(image);
        assert_eq!(budget.used(), 0);
    }

    #[test]
    fn malformed_source_and_source_admission_release_credit() {
        let bytes = b"%PDF-1.4\nnot a valid PDF object graph\n";
        let budget = MemoryBudget::new(4096);
        let mut request = DecodeRequest::new("malformed.pdf");
        request.memory_budget = Some(budget.clone());
        assert!(matches!(
            decode(bytes, &request),
            Err(RasterDecodeError::InvalidPdf(_))
        ));
        assert!(budget.peak() >= bytes.len() as u64);
        assert_eq!(budget.used(), 0);
        let insufficient = MemoryBudget::new(bytes.len() as u64 - 1);
        request.memory_budget = Some(insufficient.clone());
        assert!(matches!(
            decode(bytes, &request),
            Err(RasterDecodeError::Source(DecodeError::Memory(_)))
        ));
        assert_eq!(insufficient.peak(), 0);
        assert_eq!(insufficient.used(), 0);
    }

    #[test]
    fn half_alpha_red_is_straight_and_matches_poppler_white_composite() {
        let bytes = include_bytes!("../../../tests/fixtures/pdf/red-alpha.pdf");
        let budget = MemoryBudget::new(16384);
        let mut request = DecodeRequest::new("alpha.pdf");
        request.memory_budget = Some(budget.clone());
        let image = decode(bytes, &request).unwrap();
        let RasterPixels::Rgba8(values) = image.pixels() else {
            panic!()
        };
        assert!(values.chunks_exact(4).all(|p| p == [255, 0, 0, 128]));
        // Independent Poppler full-plane RGB on white: [255,127,127].
        for p in values.chunks_exact(4) {
            let alpha = u32::from(p[3]);
            let rgb =
                [p[0], p[1], p[2]].map(|c| ((u32::from(c) * alpha + 255 * (255 - alpha) + 127) / 255) as u8);
            assert_eq!(rgb, [255, 127, 127]);
        }
        drop(image);
        assert_eq!(budget.used(), 0);
    }

    #[test]
    fn selected_pages_match_authored_samples_and_release_budget() {
        let bytes = include_bytes!("../../../tests/fixtures/pdf/red-green-pages.pdf");
        let budget = MemoryBudget::new(16384);
        let mut request = DecodeRequest::new(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../../tests/fixtures/pdf/red-green-pages.pdf"),
        );
        request.memory_budget = Some(budget.clone());
        for (index, color) in [[255, 0, 0, 255], [0, 255, 0, 255]].into_iter().enumerate() {
            request.image_index = index;
            let crate::DecodedImage::Raster(image) = crate::decode_image(&request).unwrap() else {
                panic!()
            };
            assert_eq!(
                (
                    image.width(),
                    image.height(),
                    image.image_index(),
                    image.image_count()
                ),
                (10, 10, index, 2)
            );
            let RasterPixels::Rgba8(values) = image.pixels() else {
                panic!()
            };
            assert!(values.chunks_exact(4).all(|p| p == color));
            assert_eq!(budget.used(), 400);
            drop(image);
            assert_eq!(budget.used(), 0);
        }
        request.image_index = 2;
        assert!(decode(bytes, &request).is_err());
        assert_eq!(budget.used(), 0);
        request.image_index = 0;
        request.memory_budget = Some(MemoryBudget::new(bytes.len() as u64 + 799));
        assert!(decode(bytes, &request).is_err());
        assert_eq!(request.memory_budget.unwrap().used(), 0);
    }
}

#[cfg(test)]
mod soft_mask_isolation_tests {
    use super::*;
    #[test]
    fn luminosity_soft_mask_bands_preserve_alpha_and_managed_ownership() {
        let budget = MemoryBudget::new(1024 * 1024);
        let mut request = DecodeRequest::new("mask.pdf");
        request.memory_budget = Some(budget.clone());
        let image = decode(
            include_bytes!("../../../tests/fixtures/pdf/soft-mask-luminosity.pdf"),
            &request,
        )
        .unwrap();
        assert_eq!((image.width(), image.height()), (12, 8));
        let RasterPixels::Rgba8(pixels) = image.pixels() else {
            panic!()
        };
        for row in pixels.chunks_exact(48) {
            for (x, pixel) in row.chunks_exact(4).enumerate() {
                assert_eq!(
                    pixel,
                    match x {
                        0..=3 => &[0, 0, 0, 0],
                        4..=7 => &[0, 0, 255, 128],
                        _ => &[0, 0, 255, 255],
                    }
                );
            }
        }
        drop(image);
        assert_eq!(budget.used(), 0);
    }
}
