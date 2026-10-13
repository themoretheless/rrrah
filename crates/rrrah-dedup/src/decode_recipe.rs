//! Canonical, versioned identity for caller-supplied pixel interpretation.
use crate::cache::CacheError;
use rrrah_core::RasterColorSpace;
use rrrah_decode::{DecodeRequest, RlaAlphaMode, RlaColorSpace, RlaFloatByteOrder};

fn alpha(mode: RlaAlphaMode) -> u8 {
    match mode {
        RlaAlphaMode::Straight => 0,
        RlaAlphaMode::Premultiplied => 1,
    }
}

fn bytes(hash: &mut blake3::Hasher, value: &[u8]) -> Result<(), CacheError> {
    let length = u64::try_from(value.len()).map_err(|_| CacheError::Invalid)?;
    hash.update(&length.to_le_bytes());
    hash.update(value);
    Ok(())
}

fn color(hash: &mut blake3::Hasher, value: &RasterColorSpace) -> Result<(), CacheError> {
    match value {
        RasterColorSpace::Srgb => {
            hash.update(&[0]);
        }
        RasterColorSpace::AssumedSrgb => {
            hash.update(&[1]);
        }
        RasterColorSpace::Bt709 => {
            hash.update(&[2]);
        }
        RasterColorSpace::LinearSrgb => {
            hash.update(&[3]);
        }
        RasterColorSpace::LinearRgbUnspecified => {
            hash.update(&[4]);
        }
        RasterColorSpace::LinearRgb(coordinates) => {
            hash.update(&[5]);
            bytes(hash, coordinates.color_metadata_bytes())?;
        }
        RasterColorSpace::Icc(profile) => {
            hash.update(&[6]);
            bytes(hash, profile)?;
        }
        RasterColorSpace::Cicp {
            primaries,
            transfer,
            matrix,
            full_range,
        } => {
            hash.update(&[7]);
            for field in [primaries, transfer, matrix] {
                hash.update(&field.to_le_bytes());
            }
            hash.update(&[u8::from(*full_range)]);
        }
        RasterColorSpace::Unspecified => {
            hash.update(&[8]);
        }
    }
    Ok(())
}

pub(crate) fn update_interpretation(
    hash: &mut blake3::Hasher,
    request: &DecodeRequest,
) -> Result<(), CacheError> {
    // Exhaustive destructuring makes new request fields require an explicit
    // decision here. Path/selected frame have separate recipe/key identities;
    // cancellation and admission are live guards rather than pixel settings.
    let DecodeRequest {
        path: _,
        image_index: _,
        rpf_interpretation,
        rla_alpha_mode,
        rla_float_byte_order,
        rla_color_space,
        pq_reference_white_nits,
        hlg_display,
        assume_untagged_srgb,
        assume_untagged_linear_srgb,
        pfm_top_down,
        cancellation: _,
        memory_budget: _,
    } = request;
    hash.update(b"caller-pixel-interpretation-v1");
    match rpf_interpretation {
        None => {
            hash.update(&[0]);
        }
        Some(value) => {
            hash.update(&[1]);
            for channel in value.rgb {
                hash.update(&channel.to_le_bytes());
            }
            match value.matte {
                None => {
                    hash.update(&[0]);
                }
                Some(channel) => {
                    hash.update(&[1]);
                    hash.update(&channel.to_le_bytes());
                }
            }
            hash.update(&[alpha(value.alpha_mode)]);
            color(hash, &value.color_space)?;
        }
    }
    hash.update(&[match rla_alpha_mode {
        None => 0,
        Some(value) => 1 + alpha(*value),
    }]);
    hash.update(&[match rla_float_byte_order {
        None => 0,
        Some(RlaFloatByteOrder::Little) => 1,
        Some(RlaFloatByteOrder::Big) => 2,
    }]);
    hash.update(&[match rla_color_space {
        None => 0,
        Some(RlaColorSpace::Srgb) => 1,
        Some(RlaColorSpace::LinearSrgb) => 2,
    }]);
    match pq_reference_white_nits {
        None => {
            hash.update(&[0]);
        }
        Some(value) => {
            hash.update(&[1]);
            hash.update(&value.to_bits().to_le_bytes());
        }
    }
    match hlg_display {
        None => {
            hash.update(&[0]);
        }
        Some((white, peak, gamma)) => {
            hash.update(&[1]);
            for value in [white, peak, gamma] {
                hash.update(&value.to_bits().to_le_bytes());
            }
        }
    }
    hash.update(&[
        u8::from(*assume_untagged_srgb),
        u8::from(*assume_untagged_linear_srgb),
        u8::from(*pfm_top_down),
    ]);
    Ok(())
}
