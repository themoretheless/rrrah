//! Extension-level dispatch between independent native RAW backends, refined
//! by content sniffing when the file can be opened.

use std::path::Path;

use rrrah_core::MosaicRecipeManifest;

use crate::{
    DecodeError, DecodeOutput, DecodeRequest, NativeCr3Decoder, NativeDngDecoder, RawDecoder,
    camtiff::{CameraFormat, NativeCameraDecoder},
    sniff::{SniffedFormat, sniff_file},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum NativeFormat {
    Cr3,
    Dng,
    Cr2,
    Nef,
    Arw,
    Orf,
    Pef,
    Rw2,
    Raf,
}

impl NativeFormat {
    fn from_path(path: &Path) -> Result<Self, DecodeError> {
        match path
            .extension()
            .and_then(|extension| extension.to_str())
            .map(str::to_ascii_lowercase)
            .as_deref()
        {
            Some("cr3") => Ok(Self::Cr3),
            Some("dng" | "tif" | "tiff") => Ok(Self::Dng),
            Some("cr2") => Ok(Self::Cr2),
            Some("nef") => Ok(Self::Nef),
            Some("arw") => Ok(Self::Arw),
            Some("orf") => Ok(Self::Orf),
            Some("pef") => Ok(Self::Pef),
            Some("rw2") => Ok(Self::Rw2),
            Some("raf") => Ok(Self::Raf),
            _ => Err(DecodeError::UnsupportedFormat {
                path: path.to_owned(),
            }),
        }
    }

    /// Resolves the routing format for a request: extension first, refined by
    /// content sniffing when the file can be opened. A strong container magic
    /// (CR3, CR2, ORF, RW2, RAF) always wins over the extension; a generic
    /// TIFF-family or unknown magic keeps the extension's choice.
    fn resolve(path: &Path) -> Result<Self, DecodeError> {
        let by_extension = Self::from_path(path)?;
        Ok(Self::refine(by_extension, sniff_file(path)))
    }

    fn refine(by_extension: Self, sniffed: Option<SniffedFormat>) -> Self {
        match sniffed {
            Some(SniffedFormat::Cr3) => Self::Cr3,
            Some(SniffedFormat::Cr2) => Self::Cr2,
            Some(SniffedFormat::Orf) => Self::Orf,
            Some(SniffedFormat::Rw2) => Self::Rw2,
            Some(SniffedFormat::Raf) => Self::Raf,
            Some(SniffedFormat::TiffFamily | SniffedFormat::Unknown) | None => by_extension,
        }
    }
}

/// Production router for every clean-room RAW backend shipped by Rrrah.
#[derive(Debug, Clone, Copy, Default)]
pub struct NativeRawDecoder;

macro_rules! dispatch {
    ($format:expr, $method:ident, $request:expr) => {
        match $format {
            NativeFormat::Cr3 => NativeCr3Decoder.$method($request),
            NativeFormat::Dng => NativeDngDecoder.$method($request),
            NativeFormat::Cr2 => NativeCameraDecoder::new(CameraFormat::Cr2).$method($request),
            NativeFormat::Nef => NativeCameraDecoder::new(CameraFormat::Nef).$method($request),
            NativeFormat::Arw => NativeCameraDecoder::new(CameraFormat::Arw).$method($request),
            NativeFormat::Orf => NativeCameraDecoder::new(CameraFormat::Orf).$method($request),
            NativeFormat::Pef => NativeCameraDecoder::new(CameraFormat::Pef).$method($request),
            NativeFormat::Rw2 => NativeCameraDecoder::new(CameraFormat::Rw2).$method($request),
            NativeFormat::Raf => NativeCameraDecoder::new(CameraFormat::Raf).$method($request),
        }
    };
}

impl RawDecoder for NativeRawDecoder {
    fn mosaic_recipe(&self, request: &DecodeRequest) -> Result<MosaicRecipeManifest, DecodeError> {
        dispatch!(NativeFormat::resolve(&request.path)?, mosaic_recipe, request)
    }

    fn decode(&self, request: &DecodeRequest) -> Result<DecodeOutput, DecodeError> {
        dispatch!(NativeFormat::resolve(&request.path)?, decode, request)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn routes_extensions_case_insensitively() {
        assert_eq!(
            NativeFormat::from_path(Path::new("a.CR3")).unwrap(),
            NativeFormat::Cr3
        );
        for path in ["a.dng", "a.DNG", "a.tif", "a.TIFF"] {
            assert_eq!(
                NativeFormat::from_path(Path::new(path)).unwrap(),
                NativeFormat::Dng
            );
        }
        assert!(matches!(
            NativeFormat::from_path(Path::new("a.jpg")),
            Err(DecodeError::UnsupportedFormat { .. })
        ));
    }

    #[test]
    fn maps_every_camera_extension() {
        let cases = [
            ("a.cr2", NativeFormat::Cr2),
            ("a.CR2", NativeFormat::Cr2),
            ("a.nef", NativeFormat::Nef),
            ("a.NEF", NativeFormat::Nef),
            ("a.arw", NativeFormat::Arw),
            ("a.ARW", NativeFormat::Arw),
            ("a.orf", NativeFormat::Orf),
            ("a.ORF", NativeFormat::Orf),
            ("a.pef", NativeFormat::Pef),
            ("a.PEF", NativeFormat::Pef),
            ("a.rw2", NativeFormat::Rw2),
            ("a.RW2", NativeFormat::Rw2),
            ("a.raf", NativeFormat::Raf),
            ("a.RAF", NativeFormat::Raf),
        ];
        for (path, expected) in cases {
            assert_eq!(
                NativeFormat::from_path(Path::new(path)).unwrap(),
                expected,
                "{path}"
            );
        }
    }

    #[test]
    fn strong_magic_overrides_the_extension() {
        for (sniffed, expected) in [
            (SniffedFormat::Cr3, NativeFormat::Cr3),
            (SniffedFormat::Cr2, NativeFormat::Cr2),
            (SniffedFormat::Orf, NativeFormat::Orf),
            (SniffedFormat::Rw2, NativeFormat::Rw2),
            (SniffedFormat::Raf, NativeFormat::Raf),
        ] {
            for extension_format in [
                NativeFormat::Cr3,
                NativeFormat::Dng,
                NativeFormat::Cr2,
                NativeFormat::Nef,
                NativeFormat::Arw,
                NativeFormat::Orf,
                NativeFormat::Pef,
                NativeFormat::Rw2,
                NativeFormat::Raf,
            ] {
                assert_eq!(
                    NativeFormat::refine(extension_format, Some(sniffed)),
                    expected,
                    "{sniffed:?} must win over {extension_format:?}"
                );
            }
        }
    }

    #[test]
    fn tiff_family_and_unreadable_files_keep_the_extension() {
        for sniffed in [
            Some(SniffedFormat::TiffFamily),
            Some(SniffedFormat::Unknown),
            None,
        ] {
            assert_eq!(
                NativeFormat::refine(NativeFormat::Nef, sniffed),
                NativeFormat::Nef,
                "{sniffed:?} must keep the extension format"
            );
            assert_eq!(
                NativeFormat::refine(NativeFormat::Dng, sniffed),
                NativeFormat::Dng,
                "{sniffed:?} must keep the extension format"
            );
        }
    }

    #[test]
    fn unreadable_file_falls_back_to_extension_routing() {
        // A missing .cr2 cannot be sniffed; routing still reaches the CR2
        // backend, which then reports the I/O failure.
        let request = DecodeRequest::new("definitely-missing-file.cr2");
        assert!(matches!(
            NativeRawDecoder.decode(&request),
            Err(DecodeError::Io { .. })
        ));
    }

    #[test]
    fn mismatched_extension_is_corrected_by_magic() {
        // A .nef whose bytes carry the CR2 header must route to the CR2
        // backend (which is still a placeholder and reports "CR2").
        let mut header = b"II*\0".to_vec();
        header.extend_from_slice(&16_u32.to_le_bytes());
        header.extend_from_slice(b"CR\x02\0");
        header.extend_from_slice(&0_u32.to_le_bytes());
        let path = std::env::temp_dir().join(format!("rrrah-router-test-{}-magic.nef", std::process::id()));
        std::fs::write(&path, &header).unwrap();
        let result = NativeRawDecoder.decode(&DecodeRequest::new(&path));
        let _ = std::fs::remove_file(&path);
        assert!(
            matches!(result, Err(DecodeError::NativeCamera { format: "CR2", .. })),
            "CR2 magic must win over the .nef extension, got {result:?}"
        );
    }

    #[test]
    fn camera_placeholder_backends_fail_typed_end_to_end() {
        // A .rw2 with a matching RW2 magic reaches the RW2 placeholder; the
        // TIFF parse fails because 0x55 is not the classic TIFF magic — but
        // the error must be typed NativeCamera("RW2"), not a routing error.
        let path = std::env::temp_dir().join(format!("rrrah-router-test-{}-typed.rw2", std::process::id()));
        std::fs::write(&path, b"IIU\0\x08\0\0\0").unwrap();
        let result = NativeRawDecoder.decode(&DecodeRequest::new(&path));
        let _ = std::fs::remove_file(&path);
        assert!(
            matches!(result, Err(DecodeError::NativeCamera { format: "RW2", .. })),
            "RW2 routing must produce a typed camera error, got {result:?}"
        );
    }
}
