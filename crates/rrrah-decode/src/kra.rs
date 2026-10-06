//! Krita saved composition. Native layers and animation remain separate contracts.
use crate::{
    DecodeRequest,
    raster::{MAX_RASTER_BYTES, RasterDecodeError, decode_raster_bytes},
};
use rrrah_core::DecodedRaster;
use std::io::Cursor;
fn bad(s: impl ToString) -> RasterDecodeError {
    RasterDecodeError::InvalidKra(s.to_string())
}
pub(crate) fn has_magic(bytes: &[u8]) -> bool {
    crate::ora::zip_mime_magic(bytes, b"application/x-krita")
        || crate::ora::zip_mime_magic(bytes, b"application/x-kra")
}
pub(crate) fn decode(bytes: &[u8], request: &DecodeRequest) -> Result<DecodedRaster, RasterDecodeError> {
    request.check_cancelled()?;
    let mut archive = zip::ZipArchive::new(Cursor::new(bytes)).map_err(bad)?;
    if archive.len() > 65536 {
        return Err(bad("too many entries"));
    }
    let mut names = std::collections::HashSet::new();
    for i in 0..archive.len() {
        let entry = archive.by_index(i).map_err(bad)?;
        if !names.insert(entry.name().to_owned()) {
            return Err(bad("duplicate entry"));
        }
    }
    let mut read = |name, limit| {
        crate::ora::read_entry(&mut archive, name, limit, request).map_err(|e| match e {
            RasterDecodeError::InvalidOra(s) => bad(s),
            other => other,
        })
    };
    let mime = read("mimetype", 64)?;
    if mime.as_ref() != b"application/x-krita" && mime.as_ref() != b"application/x-kra" {
        return Err(bad("incorrect mimetype"));
    }
    let xml = read(
        if names.contains("maindoc.xml") {
            "maindoc.xml"
        } else {
            "root"
        },
        4 * 1024 * 1024,
    )?;
    let xml = std::str::from_utf8(&xml).map_err(bad)?;
    let doc = resvg::usvg::roxmltree::Document::parse_with_options(
        xml,
        resvg::usvg::roxmltree::ParsingOptions {
            allow_dtd: true,
            nodes_limit: 100000,
        },
    )
    .map_err(bad)?;
    let mut images = doc
        .descendants()
        .filter(|n| n.is_element() && n.tag_name().name() == "IMAGE");
    let image = images.next().ok_or_else(|| bad("missing IMAGE"))?;
    if images.next().is_some() {
        return Err(bad("multiple canvases"));
    }
    if image.attribute("mime") != Some("application/x-kra") {
        return Err(bad("incorrect document mime"));
    }
    let dimension = |name| {
        image
            .attribute(name)
            .and_then(|v| v.parse::<u32>().ok())
            .filter(|v| *v > 0 && *v <= 65536)
            .ok_or_else(|| bad("invalid dimensions"))
    };
    let (width, height) = (dimension("width")?, dimension("height")?);
    if u64::from(width) * u64::from(height) > MAX_RASTER_BYTES / 8 {
        return Err(RasterDecodeError::OutputTooLarge);
    }
    let merged = read("mergedimage.png", MAX_RASTER_BYTES)?;
    if !merged.starts_with(b"\x89PNG\r\n\x1a\n") {
        return Err(bad("composition must be PNG"));
    }
    let frame = decode_raster_bytes(merged, request)?;
    if (frame.width(), frame.height()) != (width, height) {
        return Err(bad("composition canvas mismatch"));
    }
    Ok(frame)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    fn fixture(mime: &str, width: u32, merged: bool) -> Vec<u8> {
        let mut writer = zip::ZipWriter::new(Cursor::new(Vec::new()));
        let options = zip::write::SimpleFileOptions::default();
        writer.start_file("mimetype", options).unwrap();
        writer.write_all(mime.as_bytes()).unwrap();
        writer.start_file("root", options).unwrap();
        writer
            .write_all(
                format!("<DOC><IMAGE mime='application/x-kra' width='{width}' height='16'/></DOC>")
                    .as_bytes(),
            )
            .unwrap();
        if merged {
            writer.start_file("mergedimage.png", options).unwrap();
            writer
                .write_all(include_bytes!("../../../tests/fixtures/raster/pattern.png"))
                .unwrap();
        }
        writer.finish().unwrap().into_inner()
    }
    #[test]
    fn legacy_root_and_native_mime_are_read() {
        assert!(
            decode(
                &fixture("application/x-kra", 16, true),
                &DecodeRequest::new("x.kra")
            )
            .is_ok()
        );
    }
    #[test]
    fn invalid_identity_canvas_and_missing_merged_are_rejected() {
        for b in [
            fixture("image/openraster", 16, true),
            fixture("application/x-krita", 15, true),
            fixture("application/x-krita", 16, false),
        ] {
            assert!(matches!(
                decode(&b, &DecodeRequest::new("x.kra")),
                Err(RasterDecodeError::InvalidKra(_))
            ));
        }
    }
    #[test]
    fn rgb_icc_composition_reaches_display() {
        let f = decode(
            include_bytes!("../../../tests/fixtures/raster/pattern.profiled.png.kra"),
            &DecodeRequest::new("x.kra"),
        )
        .unwrap();
        assert!(matches!(f.color_space(), rrrah_core::RasterColorSpace::Icc(_)));
        assert!(crate::prepare_raster_for_display(&f).is_ok());
    }
    #[test]
    fn container_identity_overrides_raw_suffix() {
        let path = std::env::temp_dir().join(format!("rrrah-kra-magic-{}.cr3", std::process::id()));
        std::fs::write(
            &path,
            include_bytes!("../../../tests/fixtures/raster/pattern.png.kra"),
        )
        .unwrap();
        let result = crate::decode_image_file(&path);
        std::fs::remove_file(path).unwrap();
        assert!(matches!(result, Ok(crate::DecodedImage::Raster(_))));
    }
}
