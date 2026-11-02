//! Experimental guarded display evidence under caller-supplied geometry.
use crate::{
    animated::AnimationBudget,
    decode::{CachedError, DecodeSourceSnapshot, FileError, decode_selected_frame_bounded},
    geometry::Transform,
    local_scan::LocalFileError,
    warp::{
        DisplayProjectionEvidence, PhotometricPolicy, verify_photometric_white_anchored_display_projection,
    },
};
use rrrah_core::MemoryBudget;
use rrrah_decode::DecodeRequest;
use std::cell::Cell;

/// Decode and compare selected frames, retaining source/dependency observations
/// until after both directions finish. Does not discover geometry or establish
/// exact identity. The returned strict evidence must remain separate.
pub fn verify_white_anchored_files(
    left: &DecodeRequest,
    right: &DecodeRequest,
    transform: Transform,
    decode: AnimationBudget,
    policy: PhotometricPolicy,
    budget: &MemoryBudget,
    cancel: impl Fn() -> bool,
) -> Result<DisplayProjectionEvidence, LocalFileError> {
    let latch = Cell::new(false);
    let request_cancelled = || {
        [left, right].iter().any(|request| {
            request
                .cancellation
                .as_ref()
                .is_some_and(rrrah_decode::GenerationToken::is_cancelled)
        })
    };
    let cancelled = || {
        let value = latch.get() || cancel() || request_cancelled();
        latch.set(value);
        value
    };
    let result = (|| {
        if cancelled() {
            return Err(LocalFileError::Cancelled);
        }
        crate::warp::validate_photometric_policy(policy)?;
        let determinant = transform.a * transform.a + transform.b * transform.b;
        if !determinant.is_finite()
            || determinant <= 0.0
            || [
                transform.a,
                transform.b,
                transform.translation[0],
                transform.translation[1],
            ]
            .iter()
            .any(|value| !value.is_finite())
        {
            return Err(LocalFileError::InvalidPolicy);
        }
        if decode.max_frames == 0 || decode.max_pixels == 0 || decode.max_file_bytes == 0 {
            return Err(LocalFileError::InvalidPolicy);
        }
        let first = DecodeSourceSnapshot::read(&left.path, decode.max_file_bytes, &cancelled)?;
        let second = DecodeSourceSnapshot::read(&right.path, decode.max_file_bytes, &cancelled)?;
        let a = decode_selected_frame_bounded(left, decode, budget, &cancelled)?;
        let b = decode_selected_frame_bounded(right, decode, budget, &cancelled)?;
        let av = a
            .view(&cancelled)
            .map_err(FileError::from)
            .map_err(CachedError::from)?;
        let bv = b
            .view(&cancelled)
            .map_err(FileError::from)
            .map_err(CachedError::from)?;
        let evidence =
            verify_photometric_white_anchored_display_projection(&av, &bv, transform, policy, &cancelled)?;
        first.verify(&cancelled)?;
        second.verify(&cancelled)?;
        // User callbacks may mutate either input, including on the final call.
        // Validate again after that call with only passive cancellation checks.
        if cancelled() {
            return Err(LocalFileError::Cancelled);
        }
        let passive_cancel = || latch.get() || request_cancelled();
        first.verify(&passive_cancel)?;
        second.verify(&passive_cancel)?;
        Ok(evidence)
    })();
    match result {
        Ok(evidence) if !latch.get() && !request_cancelled() => Ok(evidence),
        Ok(_) => Err(LocalFileError::Cancelled),
        Err(_) if cancelled() => Err(LocalFileError::Cancelled),
        Err(error) => Err(error),
    }
}
