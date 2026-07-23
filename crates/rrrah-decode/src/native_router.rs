//! Extension-level dispatch between independent native RAW backends.

use std::path::Path;

use rrrah_core::MosaicRecipeManifest;

use crate::{DecodeError, DecodeOutput, DecodeRequest, NativeCr3Decoder, NativeDngDecoder, RawDecoder};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum NativeFormat {
    Cr3,
    Dng,
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
            _ => Err(DecodeError::UnsupportedFormat {
                path: path.to_owned(),
            }),
        }
    }
}

/// Production router for every clean-room RAW backend shipped by Rrrah.
#[derive(Debug, Clone, Copy, Default)]
pub struct NativeRawDecoder;

impl RawDecoder for NativeRawDecoder {
    fn mosaic_recipe(&self, request: &DecodeRequest) -> Result<MosaicRecipeManifest, DecodeError> {
        match NativeFormat::from_path(&request.path)? {
            NativeFormat::Cr3 => NativeCr3Decoder.mosaic_recipe(request),
            NativeFormat::Dng => NativeDngDecoder.mosaic_recipe(request),
        }
    }

    fn decode(&self, request: &DecodeRequest) -> Result<DecodeOutput, DecodeError> {
        match NativeFormat::from_path(&request.path)? {
            NativeFormat::Cr3 => NativeCr3Decoder.decode(request),
            NativeFormat::Dng => NativeDngDecoder.decode(request),
        }
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
}
