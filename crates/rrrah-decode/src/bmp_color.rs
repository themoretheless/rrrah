//! BMP V4/V5 declarations. Profile offsets are relative to the DIB, not the file.
use crate::{DecodeRequest, raster::RasterDecodeError};
use rrrah_core::RasterColorSpace;

pub(super) struct Metadata<'a> {
    pub profile: Option<&'a [u8]>,
    pub pixel_end: usize,
    pub color: RasterColorSpace,
}
fn bad(reason: &'static str) -> RasterDecodeError {
    RasterDecodeError::InvalidBmpColor(reason)
}
pub(super) fn parse<'a>(bytes: &'a [u8], request: &DecodeRequest) -> Result<Metadata<'a>, RasterDecodeError> {
    request.check_cancelled()?;
    let number = |at: usize| -> Result<u32, RasterDecodeError> {
        Ok(u32::from_le_bytes(
            bytes
                .get(at..at + 4)
                .ok_or_else(|| bad("truncated DIB header"))?
                .try_into()
                .unwrap(),
        ))
    };
    let size = number(14)? as usize;
    let mut result = Metadata {
        profile: None,
        pixel_end: bytes.len(),
        color: RasterColorSpace::Srgb,
    };
    if !matches!(size, 108 | 124) {
        return Ok(result); // Other header layouts remain the pixel decoder's responsibility.
    }
    bytes
        .get(..14 + size)
        .ok_or_else(|| bad("truncated V4/V5 header"))?;
    match number(14 + 56)? {
        0x7352_4742 | 0x5769_6e20 => {} // sRGB / Windows default sRGB.
        0x4c49_4e4b => return Err(bad("linked profiles require explicit external-resource support")),
        0x4d42_4544 => {
            if size != 124 {
                return Err(bad("embedded profile requires a V5 header"));
            }
            let start = 14usize
                .checked_add(number(14 + 112)? as usize)
                .ok_or_else(|| bad("profile offset overflow"))?;
            let length = number(14 + 116)? as usize;
            let end = start
                .checked_add(length)
                .ok_or_else(|| bad("profile extent overflow"))?;
            let pixel_start = number(10)? as usize;
            if start < 14 + size || start < pixel_start {
                return Err(bad("profile precedes bitmap pixel data"));
            }
            let profile = bytes
                .get(start..end)
                .ok_or_else(|| bad("truncated embedded profile"))?;
            if profile.len() < 128 || profile.get(36..40) != Some(b"acsp") {
                return Err(bad("invalid embedded ICC header"));
            }
            let declared = u32::from_be_bytes(profile[..4].try_into().unwrap()) as usize;
            if declared < 128 || declared > profile.len() {
                return Err(bad("invalid embedded ICC size"));
            }
            result.profile = Some(profile);
            // Truncated bitmap data must never be completed with bytes from its profile.
            result.pixel_end = start;
        }
        // Preserve the samples, but do not turn unimplemented calibrated/unknown
        // declarations into an sRGB assertion. Their display transform remains open.
        _ => result.color = RasterColorSpace::Unspecified,
    }
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    const ICC: &[u8] = include_bytes!("../../../tests/fixtures/raster/animation-linear-float.icc");
    fn bitmap() -> Vec<u8> {
        let mut b = vec![0; 142];
        b[..2].copy_from_slice(b"BM");
        b[10..14].copy_from_slice(&138u32.to_le_bytes());
        b[14..18].copy_from_slice(&124u32.to_le_bytes());
        b[18..22].copy_from_slice(&1i32.to_le_bytes());
        b[22..26].copy_from_slice(&1i32.to_le_bytes());
        b[26..28].copy_from_slice(&1u16.to_le_bytes());
        b[28..30].copy_from_slice(&24u16.to_le_bytes());
        b[70..74].copy_from_slice(&0x4d42_4544u32.to_le_bytes());
        b[126..130].copy_from_slice(&128u32.to_le_bytes());
        b[130..134].copy_from_slice(&(ICC.len() as u32).to_le_bytes());
        b[138..141].copy_from_slice(&[192, 128, 64]);
        b.extend_from_slice(ICC);
        let len = b.len() as u32;
        b[2..6].copy_from_slice(&len.to_le_bytes());
        b
    }
    #[test]
    fn v5_icc_reaches_public_color_preparation_and_shared_managed_ownership() {
        let root = rrrah_core::MemoryBudget::new(ICC.len() as u64 + 7);
        let mut request = DecodeRequest::new("profile.bmp");
        request.memory_budget = Some(root.clone());
        let raster = crate::raster::decode_raster_bytes(bitmap(), &request).unwrap();
        assert_eq!(raster.color_space(), &RasterColorSpace::Icc(ICC.to_vec()));
        let rrrah_core::RasterPixels::Rgba8(values) = raster.pixels() else {
            panic!()
        };
        assert_eq!(&values[..], &[64, 128, 192, 255]);
        assert_eq!(root.used(), ICC.len() as u64 + 4);
        assert_eq!(root.peak(), ICC.len() as u64 + 7);
        let reference = rrrah_core::DecodedRaster::new(
            1,
            1,
            rrrah_core::RasterPixels::Rgba8(std::sync::Arc::new(vec![64, 128, 192, 255]).into()),
            RasterColorSpace::Icc(ICC.to_vec()),
        )
        .unwrap();
        let actual = crate::prepare_raster_for_display(&raster).unwrap();
        let expected = crate::prepare_raster_for_display(&reference).unwrap();
        let (rrrah_core::RasterPixels::Rgba32Float(a), rrrah_core::RasterPixels::Rgba32Float(e)) =
            (actual.pixels(), expected.pixels())
        else {
            panic!()
        };
        assert_eq!(&a[..], &e[..]);
        let clone = raster.clone();
        drop((raster, actual));
        assert_eq!(root.used(), ICC.len() as u64 + 4);
        drop(clone);
        assert_eq!(root.used(), 0);
    }
    #[test]
    fn profile_and_output_credit_failures_release_every_reservation() {
        for limit in [ICC.len() as u64 - 1, ICC.len() as u64 + 6] {
            let root = rrrah_core::MemoryBudget::new(limit);
            let mut request = DecodeRequest::new("profile.bmp");
            request.memory_budget = Some(root.clone());
            assert!(crate::raster::decode_raster_bytes(bitmap(), &request).is_err());
            assert_eq!(root.used(), 0);
        }
    }
    #[test]
    fn malformed_and_linked_profiles_refuse_without_pixel_admission() {
        let request = DecodeRequest::new("profile.bmp");
        for (at, value) in [
            (126, 0),
            (126, u32::MAX),
            (130, 0),
            (130, u32::MAX),
            (70, 0x4c49_4e4b),
        ] {
            let mut bytes = bitmap();
            bytes[at..at + 4].copy_from_slice(&value.to_le_bytes());
            assert!(parse(&bytes, &request).is_err(), "field {at}={value}");
        }
        let mut bytes = bitmap();
        bytes.truncate(bytes.len() - 1);
        assert!(parse(&bytes, &request).is_err());
        let mut bytes = bitmap();
        bytes[142 + 36] = 0;
        assert!(parse(&bytes, &request).is_err());
    }
    #[test]
    fn calibration_is_not_srgb_and_srgb_ignores_unused_profile_fields() {
        let request = DecodeRequest::new("profile.bmp");
        let mut bytes = bitmap();
        bytes[70..74].fill(0);
        assert_eq!(
            parse(&bytes, &request).unwrap().color,
            RasterColorSpace::Unspecified
        );
        bytes[70..74].copy_from_slice(&0x7352_4742u32.to_le_bytes());
        bytes[126..134].fill(255);
        let metadata = parse(&bytes, &request).unwrap();
        assert!(metadata.profile.is_none());
        assert_eq!(metadata.color, RasterColorSpace::Srgb);
    }
    #[test]
    fn truncated_pixel_payload_cannot_borrow_icc_bytes() {
        let mut bytes = bitmap();
        bytes[126..130].copy_from_slice(&124u32.to_le_bytes());
        bytes.drain(138..142);
        assert!(crate::raster::decode_raster_bytes(bytes, &DecodeRequest::new("profile.bmp")).is_err());
    }
}
