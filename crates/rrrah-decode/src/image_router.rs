//! Common image entrypoint; sensor data and rendered pixels stay distinct.

use crate::{
    DecodeError, DecodeOutput, DecodeRequest, NativeRawDecoder, RasterDecodeError, RawDecoder,
    bounded_io::read_managed,
    decode_raster,
    sniff::{SniffedFormat, read_header, sniff},
};
use rrrah_core::DecodedRaster;
use std::path::Path;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImageSourceKind {
    Sensor,
    Raster,
}

#[derive(Debug, Clone)]
pub enum DecodedImage {
    Sensor(Box<DecodeOutput>),
    Raster(DecodedRaster),
}

/// File-extension candidates for the shipped decoders, not a promise that
/// every storage/color variant can be decoded or displayed.
pub fn is_supported_image_path(path: &Path) -> bool {
    is_supported_raw_path(path)
        || path.extension().and_then(|s| s.to_str()).is_some_and(|s| {
            [
                "dcm", "dicom", "ase", "aseprite", "jxr", "wdp", "hdp", "jls", "jng", "jpg", "jpeg", "png",
                "apng", "gif", "webp", "bmp", "ico", "qoi", "tga", "ppm", "pgm", "pbm", "pnm", "pvr", "fits",
                "fit", "fts", "mrc", "mrcs", "map", "nrrd", "wad", "rla", "pic", "cin", "cineon", "dpx",
                "wal", "mac", "macp", "pntg", "mpnt", "pam", "ff", "exr", "hdr", "pfm", "pcx", "sgi", "rgb",
                "rgba", "bw", "ras", "sun", "sunras", "dds", "avif", "xbm", "xpm", "cur", "svg", "ora",
                "pdf", "ai", "basis", "ktx2", "ktx", "pkm", "iff", "ilbm", "lbm", "pix", "astc", "heic",
                "heif", "hif", "x3f", "pict", "pct", "kra", "xcf", "psd", "psb", "dcx", "mng", "sti", "wmf",
                "emf", "jxl", "jp2", "j2k", "j2c", "jpc",
            ]
            .iter()
            .any(|ext| s.eq_ignore_ascii_case(ext))
        })
}

/// Supported RAW extension candidates. A candidate may still contain an
/// unsupported camera/storage variant; content validation happens at decode.
pub fn is_supported_raw_path(path: &Path) -> bool {
    path.extension().and_then(|s| s.to_str()).is_some_and(|s| {
        [
            "crw", "cr3", "cr2", "dng", "nef", "nrw", "arw", "sr2", "mrw", "erf", "kdc", "srw", "3fr", "fff",
            "dcr", "mos", "iiq", "srf", "dcs", "orf", "pef", "ptx", "rw2", "rwl", "raw", "raf", "tif",
            "tiff",
        ]
        .iter()
        .any(|ext| s.eq_ignore_ascii_case(ext))
    })
}

pub fn image_source_kind(request: &DecodeRequest) -> Result<ImageSourceKind, RasterDecodeError> {
    request.check_cancelled()?;
    // Share one bounded header read between sensor and raster detection.
    let file = std::fs::File::open(&request.path).map_err(|source| DecodeError::Io {
        path: request.path.clone(),
        source,
    })?;
    let (header, length) = read_header(file).map_err(|source| DecodeError::Io {
        path: request.path.clone(),
        source,
    })?;
    let bytes = &header[..length];
    request.check_cancelled()?;
    if bytes.starts_with(b"FOVb") || crate::dicom::has_magic(bytes) {
        return Ok(ImageSourceKind::Raster);
    }
    match sniff(&bytes) {
        SniffedFormat::Crw
        | SniffedFormat::Cr3
        | SniffedFormat::Cr2
        | SniffedFormat::Orf
        | SniffedFormat::Rw2
        | SniffedFormat::Mrw
        | SniffedFormat::Raf => Ok(ImageSourceKind::Sensor),
        SniffedFormat::TiffFamily => {
            // Camera extensions remain sensor requests even for malformed or
            // unsupported RAW variants. Never fall back to a TIFF preview.
            let camera = request
                .path
                .extension()
                .and_then(|s| s.to_str())
                .is_some_and(|s| {
                    [
                        "dng", "nef", "nrw", "arw", "sr2", "mrw", "erf", "kdc", "srw", "3fr", "fff", "dcr",
                        "mos", "iiq", "srf", "dcs", "pef", "ptx", "raw",
                    ]
                    .iter()
                    .any(|ext| s.eq_ignore_ascii_case(ext))
                });
            if camera
                || crate::dng::has_sensor_image(&read_managed(request)?)
                    .map_err(|error| DecodeError::NativeDng(error.to_string()))?
            {
                Ok(ImageSourceKind::Sensor)
            } else {
                Ok(ImageSourceKind::Raster)
            }
        }
        _ => {
            // An actual raster signature wins over a misleading RAW suffix.
            let raster_magic = crate::xcf::has_magic(&bytes)
                || crate::pict::has_magic(&bytes)
                || crate::dicom::has_magic(&bytes)
                || crate::aseprite::has_magic(&bytes)
                || crate::jpegxr::has_magic(&bytes)
                || crate::jpegls::has_magic(&bytes)
                || crate::jng::has_magic(&bytes)
                || crate::mng::has_magic(&bytes)
                || crate::sti::has_magic(&bytes)
                || crate::wmf::has_magic(&bytes)
                || crate::emf::has_magic(&bytes)
                || crate::pvr::has_magic(&bytes)
                || crate::fits::has_magic(&bytes)
                || crate::mrc::has_magic(&bytes)
                || crate::nrrd::has_magic(&bytes)
                || crate::wad::has_magic(&bytes)
                || crate::softimage::has_magic(&bytes)
                || crate::cineon::has_magic(&bytes)
                || crate::dpx::has_magic(&bytes)
                || crate::macpaint::has_wrapper(&bytes)
                || crate::astc::has_magic(&bytes)
                || crate::heif::has_magic(&bytes)
                || crate::pdf::has_magic(&bytes)
                || crate::basis::has_magic(&bytes)
                || crate::ktx2::has_magic(&bytes)
                || crate::ktx::has_magic(&bytes)
                || crate::pkm::has_magic(&bytes)
                || crate::iff::has_magic(&bytes)
                || crate::kra::has_magic(&bytes)
                || crate::psd::has_magic(&bytes)
                || crate::dcx::has_magic(&bytes)
                || crate::jp2::has_magic(&bytes)
                || crate::jxl::has_magic(&bytes)
                || crate::ora::has_magic(&bytes)
                || bytes.starts_with(&[0, 0, 2, 0])
                || bytes.starts_with(b"/* XPM */")
                || bytes.starts_with(b"! XPM2")
                || bytes.starts_with(b"DDS ")
                || bytes.starts_with(&[0x59, 0xa6, 0x6a, 0x95])
                || bytes.starts_with(&[1, 218])
                || image::guess_format(&bytes).is_ok()
                || bytes.starts_with(b"PF\n")
                || bytes.starts_with(b"Pf\n")
                || bytes.starts_with(b"PF\r\n")
                || bytes.starts_with(b"Pf\r\n")
                || bytes.first() == Some(&10);
            if !raster_magic && is_supported_raw_path(&request.path) {
                Ok(ImageSourceKind::Sensor)
            } else {
                Ok(ImageSourceKind::Raster)
            }
        }
    }
}

pub fn decode_image_file(path: impl AsRef<Path>) -> Result<DecodedImage, RasterDecodeError> {
    decode_image(&DecodeRequest::new(path.as_ref()))
}

pub fn decode_image(request: &DecodeRequest) -> Result<DecodedImage, RasterDecodeError> {
    match image_source_kind(request)? {
        ImageSourceKind::Sensor => Ok(DecodedImage::Sensor(Box::new(NativeRawDecoder.decode(request)?))),
        ImageSourceKind::Raster => Ok(DecodedImage::Raster(decode_raster(request)?)),
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn cancelled_image_entrypoints_precede_header_io_and_memory_admission() {
        for extension in ["png", "tiff", "cr3", "pdf", "nrrd", "unknown"] {
            let mut request = crate::DecodeRequest::new(format!("/rrrah-absent-cancelled-image.{extension}"));
            request.cancellation = Some(crate::GenerationToken::new(
                std::sync::Arc::new(std::sync::atomic::AtomicU64::new(2)),
                1,
            ));
            let budget = rrrah_core::MemoryBudget::new(0);
            request.memory_budget = Some(budget.clone());
            assert!(matches!(
                super::image_source_kind(&request),
                Err(crate::RasterDecodeError::Source(crate::DecodeError::Cancelled))
            ));
            assert!(matches!(
                super::decode_image(&request),
                Err(crate::RasterDecodeError::Source(crate::DecodeError::Cancelled))
            ));
            assert!(matches!(
                crate::decode_raster(&request),
                Err(crate::RasterDecodeError::Source(crate::DecodeError::Cancelled))
            ));
            assert!(matches!(
                crate::decode_raster_with_window(&request, crate::ScalarWindow::new(0.0, 1.0).unwrap()),
                Err(crate::RasterDecodeError::Source(crate::DecodeError::Cancelled))
            ));
            assert_eq!(budget.peak(), 0);
        }
    }
    #[test]
    fn pdf_compatible_ai_candidate_routes_selected_pages_but_refuses_postscript() {
        let path = std::env::temp_dir().join(format!("rrrah-pdf-compatible-{}.AI", std::process::id()));
        assert!(is_supported_image_path(&path));
        // This checks extension routing with our own PDF, not Adobe producer fidelity.
        let fixture =
            Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/pdf/red-green-pages.pdf");
        std::fs::copy(fixture, &path).unwrap();
        let budget = rrrah_core::MemoryBudget::new(64 * 1024);
        let mut request = DecodeRequest::new(&path);
        request.memory_budget = Some(budget.clone());
        for index in 0..2 {
            request.image_index = index;
            let image = decode_image(&request).unwrap();
            let DecodedImage::Raster(image) = image else {
                panic!("PDF became sensor data")
            };
            assert_eq!((image.image_index(), image.image_count()), (index, 2));
            let rrrah_core::RasterPixels::Rgba8(pixels) = image.pixels() else {
                panic!("unexpected PDF pixel representation")
            };
            let expected = if index == 0 {
                [255, 0, 0, 255]
            } else {
                [0, 255, 0, 255]
            };
            assert!(pixels.chunks_exact(4).all(|pixel| pixel == expected));
            drop(image);
            assert_eq!(budget.used(), 0);
        }
        request.image_index = 0;
        std::fs::write(&path, b"%!PS-Adobe-3.0\n%%Creator: Adobe Illustrator\nshowpage\n").unwrap();
        assert!(matches!(
            decode_image(&request),
            Err(RasterDecodeError::UnsupportedFormat)
        ));
        assert_eq!(budget.used(), 0);
        std::fs::remove_file(path).unwrap();
    }
    #[test]
    #[ignore = "requires pinned Mamiya ZD source; set RRRAH_MEF_SOURCE"]
    fn unqualified_mef_refuses_public_view_without_preview_fallback_or_leak() {
        let path = std::path::PathBuf::from(std::env::var_os("RRRAH_MEF_SOURCE").expect("MEF source"));
        let budget = rrrah_core::MemoryBudget::new(128 * 1024 * 1024);
        let mut request = DecodeRequest::new(path);
        request.memory_budget = Some(budget.clone());
        assert!(matches!(image_source_kind(&request),
            Err(RasterDecodeError::Source(DecodeError::NativeDng(reason)))
                if reason.contains("unsupported field type 4084")));
        let result = decode_image(&request);
        assert!(
            result.is_err(),
            "unqualified camera must not become a preview or guessed-color image"
        );
        eprintln!("MEF public route refusal: {}", result.unwrap_err());
        assert_eq!(budget.used(), 0);
    }
    use super::*;

    #[test]
    fn camera_candidates_cover_all_native_backends_and_case() {
        for ext in [
            "CR3", "CR2", "DNG", "NEF", "NRW", "nrw", "ARW", "SR2", "sr2", "mrw", "erf", "kdc", "srw", "3fr",
            "fff", "dcr", "mos", "iiq", "srf", "dcs", "ORF", "PEF", "PTX", "ptx", "RW2", "RWL", "RAW", "raw",
            "RAF", "TIFF",
        ] {
            assert!(is_supported_raw_path(Path::new(&format!("image.{ext}"))));
        }
        assert!(!is_supported_raw_path(Path::new("image.jpg")));
    }
    #[test]
    fn camera_tiff_aliases_are_sensor_without_whole_source_admission() {
        for extension in ["NRW", "PTX", "ptx"] {
            let path =
                std::env::temp_dir().join(format!("rrrah-camera-routing-{}.{extension}", std::process::id()));
            std::fs::write(&path, [b'I', b'I', 42, 0, 8, 0, 0, 0]).unwrap();
            let budget = rrrah_core::MemoryBudget::new(0);
            let mut request = DecodeRequest::new(&path);
            request.memory_budget = Some(budget.clone());
            assert!(is_supported_image_path(&path));
            assert_eq!(image_source_kind(&request).unwrap(), ImageSourceKind::Sensor);
            assert_eq!(budget.used(), 0);
            assert!(decode_image(&request).is_err());
            std::fs::remove_file(path).unwrap();
        }
    }

    #[test]
    fn strong_contents_override_misleading_extensions_in_both_directions() {
        use std::io::Cursor;
        let path = std::env::temp_dir().join(format!("rrrah-router-png-{}.cr3", std::process::id()));
        let mut encoded = Cursor::new(Vec::new());
        image::DynamicImage::ImageRgba8(image::RgbaImage::from_pixel(1, 1, image::Rgba([11, 22, 33, 44])))
            .write_to(&mut encoded, image::ImageFormat::Png)
            .unwrap();
        std::fs::write(&path, encoded.into_inner()).unwrap();
        assert!(matches!(decode_image_file(&path), Ok(DecodedImage::Raster(_))));
        let mut request = DecodeRequest::new(&path);
        request.memory_budget = Some(rrrah_core::MemoryBudget::new(0));
        assert_eq!(image_source_kind(&request).unwrap(), ImageSourceKind::Raster);
        assert_eq!(request.memory_budget.as_ref().unwrap().peak(), 0);

        let fixture =
            Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/raster/jxr-uint8-3.jxr");
        std::fs::copy(fixture, &path).unwrap();
        assert!(matches!(decode_image_file(&path), Ok(DecodedImage::Raster(_))));
        let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../tests/fixtures/raster/ase-32-1-2019-1.aseprite");
        std::fs::copy(fixture, &path).unwrap();
        assert!(matches!(decode_image_file(&path), Ok(DecodedImage::Raster(_))));
        std::fs::remove_file(path).unwrap();
        let path = std::env::temp_dir().join(format!("rrrah-router-cr2-{}.jpg", std::process::id()));
        std::fs::write(&path, b"II*\0\x10\0\0\0CR\x02\0\0\0\0\0").unwrap();
        assert!(matches!(
            decode_image_file(&path),
            Err(RasterDecodeError::Source(DecodeError::NativeCamera {
                format: "CR2",
                ..
            }))
        ));
        std::fs::remove_file(path).unwrap();
    }

    #[test]
    fn ordinary_tiff_is_raster_but_sensor_markers_never_fall_back() {
        use std::io::Cursor;
        let mut bytes = Cursor::new(Vec::new());
        image::DynamicImage::ImageRgb8(image::RgbImage::from_pixel(1, 1, image::Rgb([11, 22, 33])))
            .write_to(&mut bytes, image::ImageFormat::Tiff)
            .unwrap();
        let path = std::env::temp_dir().join(format!("rrrah-image-router-{}.tif", std::process::id()));
        std::fs::write(&path, bytes.into_inner()).unwrap();
        assert!(matches!(decode_image_file(&path), Ok(DecodedImage::Raster(_))));
        let budget = rrrah_core::MemoryBudget::new(0);
        let mut request = DecodeRequest::new(&path);
        request.memory_budget = Some(budget.clone());
        assert!(matches!(
            image_source_kind(&request),
            Err(RasterDecodeError::Source(DecodeError::Memory(_)))
        ));
        assert_eq!(budget.used(), 0);
        let source_bytes = std::fs::metadata(&path).unwrap().len();
        let budget = rrrah_core::MemoryBudget::new(source_bytes);
        request.memory_budget = Some(budget.clone());
        assert_eq!(image_source_kind(&request).unwrap(), ImageSourceKind::Raster);
        assert_eq!(budget.peak(), source_bytes);
        assert_eq!(budget.used(), 0);

        // TIFF with DNGVersion but no usable image: must fail in RAW decoder.
        let mut bytes = b"II*\0\x08\0\0\0\x01\0".to_vec();
        bytes.extend_from_slice(&50706_u16.to_le_bytes());
        bytes.extend_from_slice(&[1, 0, 4, 0, 0, 0, 1, 4, 0, 0, 0, 0, 0, 0]);
        std::fs::write(&path, &bytes).unwrap();
        assert_eq!(
            image_source_kind(&DecodeRequest::new(&path)).unwrap(),
            ImageSourceKind::Sensor
        );
        assert!(matches!(
            decode_image_file(&path),
            Err(RasterDecodeError::Source(DecodeError::NativeDng(_)))
        ));
        std::fs::remove_file(path).unwrap();
    }
}
