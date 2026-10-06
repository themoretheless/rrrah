use crate::{
    DecodeRequest,
    raster::{MAX_RASTER_BYTES, RasterDecodeError},
};
use image::ImageDecoder;
use resvg::{tiny_skia, usvg};
use rrrah_core::{DecodedRaster, RasterColorSpace, RasterPixels};
use std::{
    io::Cursor,
    sync::{Arc, OnceLock},
};
struct FontSnapshot {
    database: Arc<usvg::fontdb::Database>,
    identity: [u8; 32],
}
static FONTS: OnceLock<Result<FontSnapshot, String>> = OnceLock::new();

fn fonts() -> Result<&'static FontSnapshot, RasterDecodeError> {
    FONTS
        .get_or_init(|| {
            let mut database = usvg::fontdb::Database::new();
            database.load_system_fonts();
            let faces: Vec<_> = database.faces().cloned().collect();
            let mut hash = blake3::Hasher::new();
            hash.update(b"rrrah-svg-font-snapshot-v1");
            for family in [
                usvg::fontdb::Family::Serif,
                usvg::fontdb::Family::SansSerif,
                usvg::fontdb::Family::Cursive,
                usvg::fontdb::Family::Fantasy,
                usvg::fontdb::Family::Monospace,
            ] {
                let name = database.family_name(&family);
                hash.update(&(name.len() as u64).to_le_bytes());
                hash.update(name.as_bytes());
            }
            let mut retained = std::collections::BTreeMap::<[u8; 32], Arc<Vec<u8>>>::new();
            for mut face in faces {
                let bytes = database
                    .with_face_data(face.id, |data, _| data.to_vec())
                    .ok_or_else(|| "loaded font data became unavailable".to_owned())?;
                let digest = *blake3::hash(&bytes).as_bytes();
                hash.update(&digest);
                hash.update(&face.index.to_le_bytes());
                let metadata = format!(
                    "{:?}|{}|{:?}|{:?}|{:?}|{}",
                    face.families,
                    face.post_script_name,
                    face.style,
                    face.weight,
                    face.stretch,
                    face.monospaced
                );
                hash.update(&(metadata.len() as u64).to_le_bytes());
                hash.update(metadata.as_bytes());
                database.remove_face(face.id);
                face.source = usvg::fontdb::Source::Binary(
                    retained.entry(digest).or_insert_with(|| Arc::new(bytes)).clone(),
                );
                database.push_face_info(face);
            }
            Ok(FontSnapshot {
                database: Arc::new(database),
                identity: *hash.finalize().as_bytes(),
            })
        })
        .as_ref()
        .map_err(|e| RasterDecodeError::InvalidSvg(e.clone()))
}

pub(crate) fn font_identity() -> Result<[u8; 32], RasterDecodeError> {
    Ok(fonts()?.identity)
}
pub(crate) fn decode(bytes: &[u8], request: &DecodeRequest) -> Result<DecodedRaster, RasterDecodeError> {
    request.check_cancelled()?;
    if bytes.len() > 64 * 1024 * 1024 {
        return Err(RasterDecodeError::OutputTooLarge);
    }
    let text =
        std::str::from_utf8(bytes).map_err(|_| RasterDecodeError::InvalidSvg("non-UTF8 source".into()))?;
    let doc =
        usvg::roxmltree::Document::parse(text).map_err(|e| RasterDecodeError::InvalidSvg(e.to_string()))?;
    if doc.root_element().tag_name().name() != "svg" {
        return Err(RasterDecodeError::InvalidSvg("root is not svg".into()));
    }
    let mut embedded_bytes = 0u64;
    for node in doc.descendants().filter(|n| n.is_element()) {
        if matches!(
            node.tag_name().name(),
            "foreignObject" | "script" | "animate" | "animateTransform" | "animateMotion" | "set"
        ) {
            return Err(RasterDecodeError::InvalidSvg(format!(
                "unsupported SVG element {}",
                node.tag_name().name()
            )));
        }
    }
    for node in doc.descendants().filter(|n| n.is_element()) {
        if node.tag_name().name() == "use"
            && node
                .attributes()
                .any(|a| a.name() == "href" && !a.value().starts_with('#'))
        {
            return Err(RasterDecodeError::InvalidSvg(
                "external use reference unsupported".into(),
            ));
        }
        if node.tag_name().name() != "image" {
            continue;
        }
        let href = node
            .attributes()
            .find(|a| a.name() == "href")
            .map(|a| a.value())
            .ok_or_else(|| RasterDecodeError::InvalidSvg("image missing href".into()))?;
        if !href.starts_with("data:image/png;") {
            return Err(RasterDecodeError::InvalidSvg(
                "only embedded PNG images are supported".into(),
            ));
        }
        let url =
            data_url::DataUrl::process(href).map_err(|e| RasterDecodeError::InvalidSvg(e.to_string()))?;
        let (data, _) = url
            .decode_to_vec()
            .map_err(|e| RasterDecodeError::InvalidSvg(e.to_string()))?;
        let declaration = crate::png_color::declaration(&data)?;
        if !matches!(
            declaration.color,
            Some(RasterColorSpace::Srgb | RasterColorSpace::AssumedSrgb)
        ) {
            return Err(RasterDecodeError::InvalidSvg(
                "embedded PNG color requires a transform".into(),
            ));
        }
        let mut reader = image::ImageReader::with_format(Cursor::new(&data), image::ImageFormat::Png);
        let mut limits = image::Limits::default();
        limits.max_alloc = Some(MAX_RASTER_BYTES);
        reader.limits(limits);
        let mut decoder = reader.into_decoder()?;
        let (w, h) = decoder.dimensions();
        embedded_bytes = embedded_bytes.saturating_add(u64::from(w) * u64::from(h) * 8);
        if embedded_bytes > MAX_RASTER_BYTES {
            return Err(RasterDecodeError::OutputTooLarge);
        }
        if decoder.icc_profile()?.is_some() {
            return Err(RasterDecodeError::InvalidSvg(
                "embedded ICC PNG requires color conversion".into(),
            ));
        }
        image::DynamicImage::from_decoder(decoder)?;
        request.check_cancelled()?;
    }
    let mut options = usvg::Options::default();
    options.image_href_resolver.resolve_string = Box::new(|_, _| None);
    options.image_href_resolver.resolve_data = Box::new(|mime, data, _| {
        if mime == "image/png" {
            Some(usvg::ImageKind::PNG(data))
        } else {
            None
        }
    });
    if doc
        .descendants()
        .any(|n| n.is_element() && n.tag_name().name() == "text")
    {
        options.fontdb = fonts()?.database.clone();
    }
    let tree =
        usvg::Tree::from_str(text, &options).map_err(|e| RasterDecodeError::InvalidSvg(e.to_string()))?;
    let size = tree.size().to_int_size();
    let width = size.width();
    let height = size.height();
    if u64::from(width) * u64::from(height) > MAX_RASTER_BYTES / 4 {
        return Err(RasterDecodeError::OutputTooLarge);
    }
    let reservation = request
        .memory_budget
        .as_ref()
        .map(|budget| budget.try_reserve(u64::from(width) * u64::from(height) * 4))
        .transpose()
        .map_err(crate::DecodeError::Memory)?;
    let mut pixmap = tiny_skia::Pixmap::new(width, height).ok_or(RasterDecodeError::OutputTooLarge)?;
    request.check_cancelled()?;
    resvg::render(&tree, tiny_skia::Transform::identity(), &mut pixmap.as_mut());
    request.check_cancelled()?;
    // tiny-skia stores premultiplied sRGB bytes; the shared raster contract is straight alpha.
    let mut pixels = pixmap.take();
    for (index, p) in pixels.chunks_exact_mut(4).enumerate() {
        if index % 16384 == 0 {
            request.check_cancelled()?;
        }
        let alpha = u32::from(p[3]);
        if alpha == 0 {
            p[..3].fill(0);
        } else {
            for c in &mut p[..3] {
                *c = ((u32::from(*c) * 255 + alpha / 2) / alpha).min(255) as u8;
            }
        }
    }
    let pixels = match reservation {
        Some(reservation) => reservation
            .try_adopt(pixels)
            .map_err(crate::DecodeError::Memory)?
            .into(),
        None => Arc::new(pixels).into(),
    };
    Ok(DecodedRaster::new(
        width,
        height,
        RasterPixels::Rgba8(pixels),
        RasterColorSpace::Srgb,
    )?)
}
#[cfg(test)]
mod tests {
    #[test]
    fn output_admission_precedes_pixmap_allocation_and_last_owner_retains_credit() {
        let source = br##"<svg xmlns="http://www.w3.org/2000/svg" width="10" height="10"><rect width="10" height="10" fill="#ff0000"/></svg>"##;
        let refused = rrrah_core::MemoryBudget::new(399);
        let mut request = crate::DecodeRequest::new("bounded.svg");
        request.memory_budget = Some(refused.clone());
        assert!(super::decode(source, &request).is_err());
        assert_eq!((refused.used(), refused.peak()), (0, 0));
        let admitted = rrrah_core::MemoryBudget::new(400);
        request.memory_budget = Some(admitted.clone());
        let raster = super::decode(source, &request).unwrap();
        let rrrah_core::RasterPixels::Rgba8(pixels) = raster.pixels() else {
            panic!()
        };
        assert!(pixels.is_managed());
        assert!(pixels.chunks_exact(4).all(|p| p == [255, 0, 0, 255]));
        let last = raster.clone();
        drop(raster);
        assert_eq!(admitted.used(), 400);
        drop(last);
        assert_eq!(admitted.used(), 0);
    }
    use super::*;
    #[test]
    fn fill_and_straight_alpha() {
        let f=decode(br#"<svg xmlns="http://www.w3.org/2000/svg" width="2" height="1"><rect width="1" height="1" fill="red" fill-opacity="0.5"/></svg>"#,&DecodeRequest::new("unused")).unwrap();
        let RasterPixels::Rgba8(p) = f.pixels() else {
            panic!()
        };
        assert_eq!(&p[..], [255, 0, 0, 128, 0, 0, 0, 0]);
    }
    #[test]
    fn unsupported_content_is_not_silently_dropped() {
        for element in ["image", "foreignObject", "animate"] {
            let text =
                format!("<svg xmlns='http://www.w3.org/2000/svg' width='1' height='1'><{element}/></svg>");
            assert!(decode(text.as_bytes(), &DecodeRequest::new("unused")).is_err());
        }
    }
    #[test]
    fn oversized_viewport() {
        assert!(matches!(
            decode(
                b"<svg xmlns='http://www.w3.org/2000/svg' width='100000' height='100000'/>",
                &DecodeRequest::new("unused")
            ),
            Err(RasterDecodeError::OutputTooLarge)
        ));
    }
}
