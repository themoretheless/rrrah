//! GPU and game texture format decoders (DDS, KTX, KTX2, PVR/PVRTC, ASTC, Basis Universal, PKM/ETC1, Quake WAD3/WAL, STI, PCX/DCX).
#![allow(
    clippy::pedantic,
    clippy::chunks_exact_to_as_chunks,
    clippy::collapsible_if,
    clippy::missing_errors_doc,
    clippy::cast_precision_loss,
    clippy::manual_is_multiple_of,
    clippy::manual_checked_ops,
    clippy::too_many_arguments,
    clippy::needless_range_loop
)]

pub use rrrah_raw::{DecodeError, DecodeRequest, GenerationToken, bounded_io};
pub use rrrah_scientific::sample;

pub mod astc;
pub mod basis;
pub mod dcx;
pub mod dds;
pub(crate) mod deflate;
mod eac;
mod etc1;
pub mod ktx;
pub mod ktx2;
pub mod pcx;
pub mod pkm;
pub mod pvr;
pub mod sti;
pub mod wad;
pub mod wal;

pub use pvr::{PvrImage, PvrMetadata, decode_pvr};
pub use wal::{decode_wal_with_palette, wal_palette_path};

pub mod raster {
    pub const MAX_RASTER_BYTES: u64 = 512 * 1024 * 1024;
    pub use super::TextureDecodeError as RasterDecodeError;
}

#[derive(Debug, thiserror::Error)]
pub enum TextureDecodeError {
    #[error(transparent)]
    Source(#[from] DecodeError),
    #[error(transparent)]
    Frame(#[from] rrrah_core::RasterError),
    #[error("raster output exceeds the {}-byte limit", raster::MAX_RASTER_BYTES)]
    OutputTooLarge,
    #[error("invalid or unsupported Basis: {0}")]
    InvalidBasis(&'static str),
    #[error("invalid STI: {0}")]
    InvalidSti(&'static str),
    #[error("invalid DCX: {0}")]
    InvalidDcx(&'static str),
    #[error("WAL requires an external palette; use decode_wal_with_palette")]
    WalPaletteRequired,
    #[error("invalid WAL texture: {0}")]
    InvalidWal(&'static str),
    #[error("invalid or unsupported PVR: {0}")]
    InvalidPvr(&'static str),
    #[error("invalid or unsupported WAD: {0}")]
    InvalidWad(&'static str),
    #[error("invalid or unsupported PKM texture: {0}")]
    InvalidPkm(&'static str),
    #[error("invalid or unsupported ASTC texture: {0}")]
    InvalidAstc(String),
    #[error("invalid or unsupported KTX2 texture: {0}")]
    InvalidKtx2(&'static str),
    #[error("invalid or unsupported KTX texture: {0}")]
    InvalidKtx(&'static str),
    #[error("invalid or unsupported DDS image: {0}")]
    InvalidDds(&'static str),
    #[error("invalid or unsupported PCX image: {0}")]
    InvalidPcx(&'static str),
}

pub type RasterDecodeError = TextureDecodeError;
pub(crate) fn check_single_image(request: &DecodeRequest) -> Result<(), TextureDecodeError> {
    request.check_cancelled()?;
    if request.image_index != 0 {
        return Err(DecodeError::UnsupportedImageIndex {
            index: request.image_index,
        }
        .into());
    }
    Ok(())
}

#[cfg(test)]
mod single_image_tests {
    use super::*;
    #[test]
    fn direct_single_image_decoders_reject_selection_and_cancel_before_admission() {
        type Decoder = fn(&[u8], &DecodeRequest) -> Result<rrrah_core::DecodedRaster, TextureDecodeError>;
        let decoders: [Decoder; 3] = [pcx::decode, pkm::decode, astc::decode];
        let budget = rrrah_core::MemoryBudget::new(0);
        let mut request = DecodeRequest::new("unused");
        request.memory_budget = Some(budget.clone());
        for decode in decoders {
            for index in [1, 4, usize::MAX] {
                request.image_index = index;
                assert!(
                    matches!(decode(&[], &request), Err(TextureDecodeError::Source(DecodeError::UnsupportedImageIndex { index: actual })) if actual == index)
                );
                assert_eq!((budget.used(), budget.peak()), (0, 0));
            }
        }
        request.cancellation = Some(GenerationToken::new(
            std::sync::Arc::new(std::sync::atomic::AtomicU64::new(2)),
            1,
        ));
        for decode in decoders {
            assert!(matches!(
                decode(&[], &request),
                Err(TextureDecodeError::Source(DecodeError::Cancelled))
            ));
        }
        assert_eq!((budget.used(), budget.peak()), (0, 0));
    }
}
