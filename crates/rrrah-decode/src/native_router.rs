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
    Erf,
    Kdc,
    Srw,
    ThreeFr,
    Fff,
    Dcr,
    Dcs,
    Mos,
    Iiq,
    Srf,
    Cr3,
    Crw,
    Dng,
    Cr2,
    Nef,
    Nrw,
    Mrw,
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
            Some("erf") => Ok(Self::Erf),
            Some("kdc") => Ok(Self::Kdc),
            Some("srw") => Ok(Self::Srw),
            Some("3fr") => Ok(Self::ThreeFr),
            Some("fff") => Ok(Self::Fff),
            Some("dcr") => Ok(Self::Dcr),
            Some("dcs") => Ok(Self::Dcs),
            Some("mos") => Ok(Self::Mos),
            Some("iiq") => Ok(Self::Iiq),
            Some("srf") => Ok(Self::Srf),
            Some("crw") => Ok(Self::Crw),
            Some("cr3") => Ok(Self::Cr3),
            Some("dng" | "tif" | "tiff") => Ok(Self::Dng),
            Some("cr2") => Ok(Self::Cr2),
            Some("nef") => Ok(Self::Nef),
            Some("mrw") => Ok(Self::Mrw),
            Some("nrw") => Ok(Self::Nrw),
            Some("arw" | "sr2") => Ok(Self::Arw),
            Some("orf") => Ok(Self::Orf),
            Some("pef" | "ptx") => Ok(Self::Pef),
            Some("rw2" | "rwl") => Ok(Self::Rw2),
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
        let sniffed = sniff_file(path);
        let by_extension = match Self::from_path(path) {
            Ok(format) => format,
            Err(error) => match sniffed {
                Some(
                    SniffedFormat::Cr3
                    | SniffedFormat::Crw
                    | SniffedFormat::Cr2
                    | SniffedFormat::Orf
                    | SniffedFormat::Rw2
                    | SniffedFormat::Mrw
                    | SniffedFormat::Raf
                    | SniffedFormat::TiffFamily,
                ) => Self::Dng,
                _ => return Err(error),
            },
        };
        if by_extension == Self::Dng
            && sniffed == Some(SniffedFormat::TiffFamily)
            && crate::sniff::is_kodak_dcs520(path)
        {
            return Ok(Self::Dcs);
        }
        Ok(Self::refine(by_extension, sniffed))
    }

    fn refine(by_extension: Self, sniffed: Option<SniffedFormat>) -> Self {
        match sniffed {
            Some(SniffedFormat::Crw) => Self::Crw,
            Some(SniffedFormat::Cr3) => Self::Cr3,
            Some(SniffedFormat::Cr2) => Self::Cr2,
            Some(SniffedFormat::Orf) => Self::Orf,
            Some(SniffedFormat::Rw2) => Self::Rw2,
            Some(SniffedFormat::Mrw) => Self::Mrw,
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
            NativeFormat::Erf => NativeCameraDecoder::new(CameraFormat::Erf).$method($request),
            NativeFormat::Kdc => NativeCameraDecoder::new(CameraFormat::Kdc).$method($request),
            NativeFormat::Srw => NativeCameraDecoder::new(CameraFormat::Srw).$method($request),
            NativeFormat::ThreeFr => NativeCameraDecoder::new(CameraFormat::ThreeFr).$method($request),
            NativeFormat::Fff => NativeCameraDecoder::new(CameraFormat::Fff).$method($request),
            NativeFormat::Iiq => NativeCameraDecoder::new(CameraFormat::Iiq).$method($request),
            NativeFormat::Srf => NativeCameraDecoder::new(CameraFormat::Srf).$method($request),
            NativeFormat::Mos => NativeCameraDecoder::new(CameraFormat::Mos).$method($request),
            NativeFormat::Dcs => NativeCameraDecoder::new(CameraFormat::Dcs).$method($request),
            NativeFormat::Dcr => NativeCameraDecoder::new(CameraFormat::Dcr).$method($request),
            NativeFormat::Crw => crate::NativeCrwDecoder.$method($request),
            NativeFormat::Cr3 => NativeCr3Decoder.$method($request),
            NativeFormat::Dng => NativeDngDecoder.$method($request),
            NativeFormat::Cr2 => NativeCameraDecoder::new(CameraFormat::Cr2).$method($request),
            NativeFormat::Nef => NativeCameraDecoder::new(CameraFormat::Nef).$method($request),
            NativeFormat::Mrw => crate::NativeMrwDecoder.$method($request),
            NativeFormat::Nrw => NativeCameraDecoder::new(CameraFormat::Nrw).$method($request),
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
        request.check_cancelled()?;
        if request.image_index != 0 {
            return Err(DecodeError::UnsupportedImageIndex { index: request.image_index });
        }
        dispatch!(NativeFormat::resolve(&request.path)?, mosaic_recipe, request)
    }

    fn decode(&self, request: &DecodeRequest) -> Result<DecodeOutput, DecodeError> {
        request.check_cancelled()?;
        if request.image_index != 0 {
            return Err(DecodeError::UnsupportedImageIndex { index: request.image_index });
        }
        dispatch!(NativeFormat::resolve(&request.path)?, decode, request)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn unsupported_raw_index_precedes_format_resolution_and_source_io() {
        for extension in ["cr3", "tiff", "nef", "mrw", "unknown"] {
            for index in [1, usize::MAX] {
                let mut request = DecodeRequest::new(format!("/rrrah-absent-indexed-source.{extension}"));
                request.image_index = index;
                assert!(matches!(NativeRawDecoder.mosaic_recipe(&request),
                    Err(DecodeError::UnsupportedImageIndex { index: actual }) if actual == index));
                assert!(matches!(NativeRawDecoder.decode(&request),
                    Err(DecodeError::UnsupportedImageIndex { index: actual }) if actual == index));
            }
        }
    }
    #[test]
    fn cancelled_router_refuses_before_opening_or_resolving_source() {
        for extension in ["cr3", "tiff", "nef", "unknown"] {
            let mut request = DecodeRequest::new(format!("/rrrah-absent-cancelled-source.{extension}"));
            request.cancellation = Some(crate::GenerationToken::new(
                std::sync::Arc::new(std::sync::atomic::AtomicU64::new(2)), 1,
            ));
            let budget = rrrah_core::MemoryBudget::new(0);
            request.memory_budget = Some(budget.clone());
            assert!(matches!(NativeRawDecoder.mosaic_recipe(&request), Err(DecodeError::Cancelled)));
            assert!(matches!(NativeRawDecoder.decode(&request), Err(DecodeError::Cancelled)));
            assert_eq!(budget.peak(), 0);
        }
    }

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
            ("a.nrw", NativeFormat::Nrw),
            ("a.NRW", NativeFormat::Nrw),
            ("a.arw", NativeFormat::Arw),
            ("a.ARW", NativeFormat::Arw),
            ("a.mrw", NativeFormat::Mrw),
            ("a.erf", NativeFormat::Erf),
            ("a.kdc", NativeFormat::Kdc),
            ("a.srw", NativeFormat::Srw),
            ("a.SRW", NativeFormat::Srw),
            ("a.3fr", NativeFormat::ThreeFr),
            ("a.3FR", NativeFormat::ThreeFr),
            ("a.KDC", NativeFormat::Kdc),
            ("a.ERF", NativeFormat::Erf),
            ("a.MRW", NativeFormat::Mrw),
            ("a.sr2", NativeFormat::Arw),
            ("a.SR2", NativeFormat::Arw),
            ("a.orf", NativeFormat::Orf),
            ("a.ORF", NativeFormat::Orf),
            ("a.pef", NativeFormat::Pef),
            ("a.PEF", NativeFormat::Pef),
            ("a.ptx", NativeFormat::Pef),
            ("a.PTX", NativeFormat::Pef),
            ("a.rw2", NativeFormat::Rw2),
            ("a.rwl", NativeFormat::Rw2),
            ("a.RWL", NativeFormat::Rw2),
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
            (SniffedFormat::Mrw, NativeFormat::Mrw),
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
