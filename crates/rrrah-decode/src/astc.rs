//! ASTC 2D surfaces using Arm's reference decoder, with no HDR quantization.
use crate::{
    DecodeRequest,
    raster::{MAX_RASTER_BYTES, RasterDecodeError},
};
use rrrah_core::{DecodedRaster, RasterColorSpace, RasterPixels};
use std::sync::Arc;

fn bad(message: impl ToString) -> RasterDecodeError {
    RasterDecodeError::InvalidAstc(message.to_string())
}
pub(crate) fn has_magic(bytes: &[u8]) -> bool {
    bytes.starts_with(&[0x13, 0xab, 0xa1, 0x5c])
}
pub(crate) fn decode(bytes: &[u8], request: &DecodeRequest) -> Result<DecodedRaster, RasterDecodeError> {
    request.check_cancelled()?;
    let header = bytes.get(..16).ok_or_else(|| bad("truncated header"))?;
    if !has_magic(header) {
        return Err(bad("signature"));
    }
    let (bx, by) = (header[4], header[5]);
    if !matches!(
        (bx, by),
        (4, 4)
            | (5, 4)
            | (5, 5)
            | (6, 5)
            | (6, 6)
            | (8, 5)
            | (8, 6)
            | (8, 8)
            | (10, 5)
            | (10, 6)
            | (10, 8)
            | (10, 10)
            | (12, 10)
            | (12, 12)
    ) {
        return Err(bad("unsupported block footprint"));
    }
    let dimension =
        |at: usize| u32::from(header[at]) | u32::from(header[at + 1]) << 8 | u32::from(header[at + 2]) << 16;
    let (width, height, depth) = (dimension(7), dimension(10), dimension(13));
    if header[6] != 1 || depth != 1 {
        return Err(bad("3D textures require explicit slice selection"));
    }
    if width == 0 || height == 0 {
        return Err(bad("empty surface"));
    }
    // Bound output for the widest possible native representation (RGBA32F).
    if width > 65536 || height > 65536 || u64::from(width) * u64::from(height) * 16 > MAX_RASTER_BYTES {
        return Err(RasterDecodeError::OutputTooLarge);
    }
    let count = u64::from(width.div_ceil(u32::from(bx))) * u64::from(height.div_ceil(u32::from(by)));
    if bytes.len() as u64 != 16 + count * 16 {
        return Err(bad("payload size mismatch"));
    }
    let (pixels, color) = native::decode_surface(&bytes[16..], width, height, bx, by, request)?;
    Ok(DecodedRaster::new(width, height, pixels, color)?)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn constant(hdr: bool) -> Vec<u8> {
        let mut bytes = vec![0x13, 0xab, 0xa1, 0x5c, 4, 4, 1, 3, 0, 0, 2, 0, 0, 1, 0, 0];
        bytes.extend([
            0xfc,
            if hdr { 0xff } else { 0xfd },
            0xff,
            0xff,
            0xff,
            0xff,
            0xff,
            0xff,
        ]);
        for value in if hdr {
            [0x4000u16, 0x3800, 0x3400, 0x3c00]
        } else {
            [65535u16, 32768, 0, 16384]
        } {
            bytes.extend(value.to_le_bytes());
        }
        bytes
    }
    #[test]
    fn ldr_constant_crop_preserves_straight_alpha() {
        let frame = decode(&constant(false), &DecodeRequest::new("x.astc")).unwrap();
        let RasterPixels::Rgba8(p) = frame.pixels() else {
            panic!()
        };
        assert_eq!((frame.width(), frame.height()), (3, 2));
        for pixel in p.chunks_exact(4) {
            assert_eq!(pixel, &[255, 128, 0, 64]);
        }
    }
    #[test]
    fn hdr_values_above_one_are_preserved() {
        let frame = decode(&constant(true), &DecodeRequest::new("x.astc")).unwrap();
        let RasterPixels::Rgba32Float(p) = frame.pixels() else {
            panic!("HDR quantized")
        };
        for pixel in p.chunks_exact(4) {
            assert_eq!(pixel, &[2.0, 0.5, 0.25, 1.0]);
        }
        assert_eq!(frame.color_space(), &RasterColorSpace::LinearRgbUnspecified);
    }
    #[test]
    fn compressed_hdr_matches_reference_float_oracle() {
        let frame = decode(
            include_bytes!("../../../tests/fixtures/raster/hdr-compressed.astc"),
            &DecodeRequest::new("hdr.astc"),
        )
        .unwrap();
        let RasterPixels::Rgba32Float(pixels) = frame.pixels() else {
            panic!("compressed HDR was quantized")
        };
        let oracle = include_bytes!("../../../tests/fixtures/raster/hdr-compressed.astc.rgba32f");
        assert_eq!(pixels.len() * 4, oracle.len());
        for (index, (actual, expected)) in pixels.iter().zip(oracle.chunks_exact(4)).enumerate() {
            let expected = f32::from_le_bytes(expected.try_into().unwrap());
            assert_eq!(*actual, expected, "sample {index}");
        }
        assert!(pixels.iter().any(|value| *value > 1.0));
    }
    #[test]
    fn malformed_blocks_payloads_and_volumes_are_rejected() {
        let original = constant(false);
        let mut invalid = original.clone();
        invalid[16..].fill(0);
        assert!(decode(&invalid, &DecodeRequest::new("bad.astc")).is_err());
        let mut invalid = original.clone();
        invalid.push(0);
        assert!(decode(&invalid, &DecodeRequest::new("bad.astc")).is_err());
        let mut invalid = original;
        invalid[6] = 4;
        assert!(decode(&invalid, &DecodeRequest::new("bad.astc")).is_err());
    }
}

// Only this bridge owns raw pointers. Sizes and footprints have already been
// validated by the container reader; native contexts are never shared.
#[allow(unsafe_code)]
mod native {
    use super::*;
    use astcenc_sys as ffi;
    use std::{ffi::c_void, mem::MaybeUninit, ptr::NonNull};

    struct Context(NonNull<ffi::astcenc_context>);
    impl Drop for Context {
        fn drop(&mut self) {
            // SAFETY: the pointer was allocated by context_alloc and is owned
            // exclusively by this object, freed exactly once.
            unsafe {
                ffi::astcenc_context_free(self.0.as_ptr());
            }
        }
    }
    fn check(code: ffi::astcenc_error) -> Result<(), RasterDecodeError> {
        if code == ffi::astcenc_error_ASTCENC_SUCCESS {
            Ok(())
        } else {
            Err(bad(format!("native decoder error {code}")))
        }
    }
    impl Context {
        fn new(bx: u8, by: u8, hdr: bool) -> Result<Self, RasterDecodeError> {
            let mut config = MaybeUninit::uninit();
            let profile = if hdr {
                ffi::astcenc_profile_ASTCENC_PRF_HDR
            } else {
                ffi::astcenc_profile_ASTCENC_PRF_LDR_SRGB
            };
            // SAFETY: writable config storage; valid dimensions and profile.
            check(unsafe {
                ffi::astcenc_config_init(
                    profile,
                    u32::from(bx),
                    u32::from(by),
                    1,
                    0.0,
                    ffi::ASTCENC_FLG_DECOMPRESS_ONLY,
                    config.as_mut_ptr(),
                )
            })?;
            // SAFETY: successful config_init initializes every config field.
            let config = unsafe { config.assume_init() };
            let mut pointer = std::ptr::null_mut();
            // SAFETY: config lives through allocation, the writable output
            // pointer is initialized only on successful allocation.
            check(unsafe { ffi::astcenc_context_alloc(&config, 1, &mut pointer) })?;
            Ok(Self(
                NonNull::new(pointer).ok_or_else(|| bad("null decoder context"))?,
            ))
        }
        fn inspect(&mut self, block: &[u8; 16]) -> Result<bool, RasterDecodeError> {
            let mut info = MaybeUninit::uninit();
            // SAFETY: exactly one readable 16-byte block and writable info.
            check(unsafe {
                ffi::astcenc_get_block_info(self.0.as_ptr(), block.as_ptr(), info.as_mut_ptr())
            })?;
            // SAFETY: get_block_info initializes info when returning success.
            let info = unsafe { info.assume_init() };
            if info.is_error_block {
                return Err(bad("invalid ASTC block"));
            }
            // Arm returns early for constant blocks without setting is_hdr_block.
            // For validated void-extent blocks, bit 9 selects FP16 storage.
            Ok(info.is_hdr_block || (info.is_constant_block && block[1] & 2 != 0))
        }
        fn decode(
            &mut self,
            payload: &[u8],
            width: u32,
            height: u32,
            output: *mut c_void,
            kind: ffi::astcenc_type,
        ) -> Result<(), RasterDecodeError> {
            let mut pointers = [output];
            let mut image = ffi::astcenc_image {
                dim_x: width,
                dim_y: height,
                dim_z: 1,
                data_type: kind,
                data: pointers.as_mut_ptr(),
            };
            let swizzle = ffi::astcenc_swizzle {
                r: ffi::astcenc_swz_ASTCENC_SWZ_R,
                g: ffi::astcenc_swz_ASTCENC_SWZ_G,
                b: ffi::astcenc_swz_ASTCENC_SWZ_B,
                a: ffi::astcenc_swz_ASTCENC_SWZ_A,
            };
            // SAFETY: the caller supplies width*height*4 initialized samples
            // of kind's type; payload has the exact validated block count.
            // The image, pointers and buffers all outlive the synchronous call.
            check(unsafe {
                ffi::astcenc_decompress_image(
                    self.0.as_ptr(),
                    payload.as_ptr(),
                    payload.len(),
                    &mut image,
                    &swizzle,
                    0,
                )
            })
        }
    }
    pub(super) fn decode_surface(
        payload: &[u8],
        width: u32,
        height: u32,
        bx: u8,
        by: u8,
        request: &DecodeRequest,
    ) -> Result<(RasterPixels, RasterColorSpace), RasterDecodeError> {
        let mut context = Context::new(bx, by, true)?;
        let mut hdr = false;
        for block in payload.chunks_exact(16) {
            request.check_cancelled()?;
            hdr |= context.inspect(block.try_into().unwrap())?;
        }
        let count = width as usize * height as usize * 4;
        let result = if hdr {
            let mut out = vec![0.0f32; count];
            context.decode(
                payload,
                width,
                height,
                out.as_mut_ptr().cast(),
                ffi::astcenc_type_ASTCENC_TYPE_F32,
            )?;
            if out.iter().any(|v| !v.is_finite()) {
                return Err(bad("non-finite HDR sample"));
            }
            (
                RasterPixels::Rgba32Float(Arc::new(out).into()),
                RasterColorSpace::LinearRgbUnspecified,
            )
        } else {
            drop(context);
            let mut context = Context::new(bx, by, false)?;
            let mut out = vec![0u8; count];
            context.decode(
                payload,
                width,
                height,
                out.as_mut_ptr().cast(),
                ffi::astcenc_type_ASTCENC_TYPE_U8,
            )?;
            (
                RasterPixels::Rgba8(Arc::new(out).into()),
                RasterColorSpace::AssumedSrgb,
            )
        };
        request.check_cancelled()?;
        Ok(result)
    }
}
