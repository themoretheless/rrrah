use crate::{
    DecodeRequest,
    raster::{MAX_RASTER_BYTES, RasterDecodeError, decode_raster_bytes},
};
use rrrah_core::DecodedRaster;
use std::io::{Cursor, Read};
pub(crate) fn has_magic(bytes: &[u8]) -> bool {
    zip_mime_magic(bytes, b"image/openraster")
}
pub(crate) fn zip_mime_magic(bytes: &[u8], mime: &[u8]) -> bool {
    if bytes.len() < 30 || !bytes.starts_with(b"PK\x03\x04") || bytes[8..10] != [0, 0] {
        return false;
    }
    let name = usize::from(u16::from_le_bytes([bytes[26], bytes[27]]));
    let extra = usize::from(u16::from_le_bytes([bytes[28], bytes[29]]));
    name == 8
        && bytes.get(30..38) == Some(b"mimetype")
        && bytes.get(38 + extra..38 + extra + mime.len()) == Some(mime)
}
fn invalid(s: impl ToString) -> RasterDecodeError {
    RasterDecodeError::InvalidOra(s.to_string())
}
pub(crate) fn read_entry(
    archive: &mut zip::ZipArchive<Cursor<&[u8]>>,
    name: &str,
    limit: u64,
    request: &DecodeRequest,
) -> Result<crate::bounded_io::SourceBuffer, RasterDecodeError> {
    request.check_cancelled()?;
    let mut entry = archive.by_name(name).map_err(invalid)?;
    if !matches!(
        entry.compression(),
        zip::CompressionMethod::Stored | zip::CompressionMethod::Deflated
    ) {
        return Err(invalid("unsupported ZIP compression"));
    }
    if entry.size() > limit {
        return Err(RasterDecodeError::OutputTooLarge);
    }
    if let Some(budget) = &request.memory_budget {
        let length = usize::try_from(entry.size()).map_err(|_| RasterDecodeError::OutputTooLarge)?;
        let mut out = budget
            .try_buffer(length, 0_u8)
            .map_err(crate::DecodeError::from)?;
        for chunk in out.chunks_mut(65536) {
            request.check_cancelled()?;
            entry.read_exact(chunk).map_err(invalid)?;
        }
        request.check_cancelled()?;
        let mut extra = [0_u8; 1];
        if entry.read(&mut extra).map_err(invalid)? != 0 {
            return Err(invalid("ZIP entry length mismatch"));
        }
        return Ok(crate::bounded_io::SourceBuffer::Managed(out));
    }
    let mut out = Vec::new();
    let mut buffer = [0; 65536];
    loop {
        request.check_cancelled()?;
        let n = entry.read(&mut buffer).map_err(invalid)?;
        if n == 0 {
            break;
        }
        if out.len() as u64 + n as u64 > limit {
            return Err(RasterDecodeError::OutputTooLarge);
        }
        out.try_reserve(n)
            .map_err(|_| RasterDecodeError::OutputTooLarge)?;
        out.extend_from_slice(&buffer[..n]);
    }
    Ok(crate::bounded_io::SourceBuffer::Legacy(out))
}
pub(crate) fn decode(bytes: &[u8], request: &DecodeRequest) -> Result<DecodedRaster, RasterDecodeError> {
    let mut archive = zip::ZipArchive::new(Cursor::new(bytes)).map_err(invalid)?;
    if archive.len() > 65536 {
        return Err(invalid("too many ZIP entries"));
    }
    {
        let first = archive.by_index(0).map_err(invalid)?;
        if first.name() != "mimetype" || first.compression() != zip::CompressionMethod::Stored {
            return Err(invalid("first entry must be stored mimetype"));
        }
    }
    let mut names = std::collections::HashSet::new();
    for i in 0..archive.len() {
        let entry = archive.by_index(i).map_err(invalid)?;
        if !names.insert(entry.name().to_owned()) {
            return Err(invalid("duplicate ZIP entry"));
        }
    }
    if read_entry(&mut archive, "mimetype", 64, request)?.as_ref() != b"image/openraster" {
        return Err(invalid("incorrect mimetype"));
    }
    let stack = read_entry(&mut archive, "stack.xml", 4 * 1024 * 1024, request)?;
    let stack = std::str::from_utf8(&stack).map_err(invalid)?;
    let doc = resvg::usvg::roxmltree::Document::parse(stack).map_err(invalid)?;
    let image = doc.root_element();
    if image.tag_name().name() != "image" {
        return Err(invalid("stack root is not image"));
    }
    let dimension = |name| {
        image
            .attribute(name)
            .and_then(|v| v.parse::<u32>().ok())
            .filter(|v| *v > 0)
            .ok_or_else(|| invalid("invalid stack canvas dimensions"))
    };
    let (width, height) = (dimension("w")?, dimension("h")?);
    if u64::from(width) * u64::from(height) > MAX_RASTER_BYTES / 8 {
        return Err(RasterDecodeError::OutputTooLarge);
    }
    let merged = read_entry(&mut archive, "mergedimage.png", MAX_RASTER_BYTES, request)?;
    if !merged.starts_with(b"\x89PNG\r\n\x1a\n") {
        return Err(invalid("merged image must be PNG"));
    }
    let frame = decode_raster_bytes(merged, request)?;
    if (frame.width(), frame.height()) != (width, height) {
        return Err(invalid("merged image dimensions disagree with canvas"));
    }
    Ok(frame)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    fn archive(mime: &str, width: u32, include_merged: bool) -> Vec<u8> {
        let mut writer = zip::ZipWriter::new(Cursor::new(Vec::new()));
        let options = zip::write::SimpleFileOptions::default();
        writer.start_file("mimetype", options).unwrap();
        writer.write_all(mime.as_bytes()).unwrap();
        writer.start_file("stack.xml", options).unwrap();
        writer
            .write_all(format!("<image version='0.0.6' w='{width}' h='16'><stack/></image>").as_bytes())
            .unwrap();
        if include_merged {
            writer.start_file("mergedimage.png", options).unwrap();
            writer
                .write_all(include_bytes!("../../../tests/fixtures/raster/pattern.png"))
                .unwrap();
        }
        writer.finish().unwrap().into_inner()
    }
    #[test]
    fn ora_and_kra_managed_source_and_expansion_preserve_pixels_and_profiles() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/raster");
        for name in [
            "pattern.png.ora",
            "pattern.png.kra",
            "pattern.profiled.png.ora",
            "pattern.profiled.png.kra",
        ] {
            let path = root.join(name);
            let reference = crate::decode_raster_file(&path).unwrap();
            let source_bytes = std::fs::metadata(&path).unwrap().len();
            let budget = rrrah_core::MemoryBudget::new(1024 * 1024);
            let mut request = DecodeRequest::new(path);
            request.memory_budget = Some(budget.clone());
            let actual = crate::decode_raster(&request).unwrap();
            assert_eq!(
                (actual.width(), actual.height()),
                (reference.width(), reference.height())
            );
            assert_eq!(actual.color_space(), reference.color_space());
            let (rrrah_core::RasterPixels::Rgba8(a), rrrah_core::RasterPixels::Rgba8(b)) =
                (actual.pixels(), reference.pixels())
            else {
                panic!("archive pixel precision changed");
            };
            assert_eq!(a, b);
            assert!(budget.peak() > source_bytes);
            assert!(a.is_managed());
            assert_eq!(budget.used(), actual.capacity_bytes());
            drop(actual);
            assert_eq!(budget.used(), 0);
            let tight = rrrah_core::MemoryBudget::new(source_bytes);
            request.memory_budget = Some(tight.clone());
            assert!(matches!(
                crate::decode_raster(&request),
                Err(RasterDecodeError::Source(crate::DecodeError::Memory(_)))
            ));
            assert_eq!(tight.used(), 0);
        }
    }

    #[test]
    fn expanded_member_holds_budget_until_drop_and_rejects_overcommit() {
        let bytes = archive("image/openraster", 16, true);
        let mut zip = zip::ZipArchive::new(Cursor::new(bytes.as_slice())).unwrap();
        let expected = include_bytes!("../../../tests/fixtures/raster/pattern.png");
        let budget = rrrah_core::MemoryBudget::new(expected.len() as u64);
        let mut request = DecodeRequest::new("unused.ora");
        request.memory_budget = Some(budget.clone());
        let member = read_entry(&mut zip, "mergedimage.png", MAX_RASTER_BYTES, &request).unwrap();
        assert_eq!(member.as_ref(), expected);
        assert_eq!(budget.used(), expected.len() as u64);
        assert!(matches!(
            read_entry(&mut zip, "mergedimage.png", MAX_RASTER_BYTES, &request),
            Err(RasterDecodeError::Source(crate::DecodeError::Memory(_)))
        ));
        assert_eq!(budget.used(), expected.len() as u64);
        drop(member);
        assert_eq!(budget.used(), 0);
        drop(read_entry(&mut zip, "mergedimage.png", MAX_RASTER_BYTES, &request).unwrap());
        assert_eq!(budget.used(), 0);
    }

    #[test]
    fn invalid_mime_canvas_and_missing_composition_are_rejected() {
        for bytes in [
            archive("image/openraster\n", 16, true),
            archive("image/openraster", 15, true),
            archive("image/openraster", 16, false),
        ] {
            assert!(matches!(
                decode(&bytes, &DecodeRequest::new("unused.ora")),
                Err(RasterDecodeError::InvalidOra(_))
            ));
        }
    }
    #[test]
    fn decompression_limit_is_enforced_before_read() {
        let bytes = archive("image/openraster", 16, true);
        let mut archive = zip::ZipArchive::new(Cursor::new(bytes.as_slice())).unwrap();
        assert!(matches!(
            read_entry(&mut archive, "mergedimage.png", 8, &DecodeRequest::new("unused")),
            Err(RasterDecodeError::OutputTooLarge)
        ));
    }
    #[test]
    fn embedded_icc_is_preserved_for_display() {
        let frame = decode(
            include_bytes!("../../../tests/fixtures/raster/pattern.profiled.png.ora"),
            &DecodeRequest::new("unused.ora"),
        )
        .unwrap();
        assert!(matches!(
            frame.color_space(),
            rrrah_core::RasterColorSpace::Icc(_)
        ));
        assert!(crate::prepare_raster_for_display(&frame).is_ok());
    }
}
