//! Cineon storage adapter to the shared DPX sample unpacker. Density values
//! remain unspecified-color samples; a film/display transform is not guessed.
use crate::{
    DecodeRequest,
    raster::{MAX_RASTER_BYTES, RasterDecodeError},
};
use rrrah_core::DecodedRaster;
fn bad(s: &'static str) -> RasterDecodeError {
    RasterDecodeError::InvalidCineon(s)
}
pub(crate) fn has_magic(b: &[u8]) -> bool {
    b.starts_with(&[0x80, 0x2a, 0x5f, 0xd7]) || b.starts_with(&[0xd7, 0x5f, 0x2a, 0x80])
}
pub(crate) fn decode(b: &[u8], request: &DecodeRequest) -> Result<DecodedRaster, RasterDecodeError> {
    request.check_cancelled()?;
    if b.len() < 1024 || !has_magic(b) {
        return Err(bad("truncated generic header"));
    }
    let be = b[0] == 0x80;
    let word = |at| {
        let v = b[at..at + 4].try_into().unwrap();
        if be {
            u32::from_be_bytes(v)
        } else {
            u32::from_le_bytes(v)
        }
    };
    if &b[24..28] != b"V4.5" {
        return Err(bad("unsupported header version"));
    }
    let count = b[193] as usize;
    let bits = b[198];
    let (w, h) = (word(200), word(204));
    if !matches!(count, 1 | 3) || w == 0 || h == 0 || w > 65536 || h > 65536 {
        return Err(bad("unsupported channels or dimensions"));
    }
    if b[192] > 7 || b[680] != 0 || b[682] != 0 || b[683] > 1 {
        return Err(bad(
            "unsupported orientation, interleave, signedness or image sense",
        ));
    }
    let packing = match (bits, b[681], count) {
        (8, 1 | 2, _) | (16, 3 | 4, _) => 0u16,
        (10, 5, 3) => 1,
        (10, 6, 3) => 2,
        _ => return Err(bad("unsupported sample packing")),
    };
    let first = b[197];
    if (count == 1 && first != 0) || (count == 3 && !matches!(first, 1 | 4)) {
        return Err(bad("unsupported channel descriptors"));
    }
    for channel in 0..count {
        let at = 196 + channel * 28;
        if b[at] != 0
            || b[at + 1] != first + channel as u8
            || b[at + 2] != bits
            || word(at + 4) != w
            || word(at + 8) != h
        {
            return Err(bad("inconsistent channel descriptors, precision or dimensions"));
        }
    }
    if u64::from(w) * u64::from(h) * 8 > MAX_RASTER_BYTES {
        return Err(RasterDecodeError::OutputTooLarge);
    }
    let start = word(4) as usize;
    let header = word(8)
        .checked_add(word(12))
        .and_then(|v| v.checked_add(word(16)))
        .ok_or_else(|| bad("header size overflow"))? as usize;
    let samples = w as usize * count;
    let row = match bits {
        8 => samples,
        10 => samples.div_ceil(3) * 4,
        _ => samples * 2,
    };
    let line = word(684) as usize;
    let image = word(688) as usize;
    let end = row
        .checked_add(line)
        .and_then(|v| v.checked_mul(h as usize))
        .and_then(|v| v.checked_add(image))
        .and_then(|v| v.checked_add(start))
        .ok_or_else(|| bad("payload size overflow"))?;
    if word(8) < 1024 || start < header || end != b.len() || word(20) as usize != b.len() {
        return Err(bad("inconsistent header, data offset or declared size"));
    }
    // Header adaptation lets both families share identical precision, padding,
    // orientation and cancellation semantics. Original Cineon metadata never
    // masquerades as a DPX transfer declaration.
    if b.len() - start > MAX_RASTER_BYTES as usize {
        return Err(RasterDecodeError::OutputTooLarge);
    }
    let mut envelope = vec![0u8; 2048];
    envelope[..4].copy_from_slice(if be { b"SDPX" } else { b"XPDS" });
    envelope[8..12].copy_from_slice(b"V2.0");
    let put = |dst: &mut [u8], at, value: u32| {
        dst[at..at + 4].copy_from_slice(&if be {
            value.to_be_bytes()
        } else {
            value.to_le_bytes()
        });
    };
    let put16 = |dst: &mut [u8], at, value: u16| {
        dst[at..at + 2].copy_from_slice(&if be {
            value.to_be_bytes()
        } else {
            value.to_le_bytes()
        });
    };
    put(&mut envelope, 4, 2048);
    put(&mut envelope, 24, 1664);
    put(&mut envelope, 28, 384);
    put(&mut envelope, 660, u32::MAX);
    put16(&mut envelope, 768, u16::from(b[192]));
    put16(&mut envelope, 770, 1);
    put(&mut envelope, 772, w);
    put(&mut envelope, 776, h);
    envelope[800..804].copy_from_slice(&[if count == 1 { 6 } else { 50 }, 255, 255, bits]);
    put16(&mut envelope, 804, packing);
    put(&mut envelope, 808, 2048);
    put(&mut envelope, 812, line as u32);
    put(&mut envelope, 816, image as u32);
    envelope.extend_from_slice(&b[start..]);
    let length = envelope.len() as u32;
    put(&mut envelope, 16, length);
    crate::dpx::decode(&envelope, request)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn header_bounds_channel_consistency_and_orientation() {
        let original = include_bytes!("../../../tests/fixtures/raster/cineon-10-be-3-p5.cin");
        let request = DecodeRequest::new("synthetic.cin");
        for at in [
            4, 8, 12, 16, 20, 24, 192, 193, 196, 197, 198, 200, 204, 224, 226, 228, 232, 680, 681, 682, 683,
            684, 688,
        ] {
            let mut b = original.to_vec();
            b[at] = 255;
            assert!(decode(&b, &request).is_err(), "field {at}");
        }
        assert!(decode(&original[..100], &request).is_err());
        assert!(decode(&original[..original.len() - 1], &request).is_err());
        let base = decode(original, &request).unwrap();
        let rrrah_core::RasterPixels::Rgba16(base) = base.pixels() else {
            panic!()
        };
        let first = [0, 4, 10, 14, 0, 10, 4, 14];
        for orientation in 0..8 {
            let mut b = original.to_vec();
            b[192] = orientation;
            let frame = decode(&b, &request).unwrap();
            assert_eq!(
                (frame.width(), frame.height()),
                if orientation < 4 { (5, 3) } else { (3, 5) }
            );
            let rrrah_core::RasterPixels::Rgba16(p) = frame.pixels() else {
                panic!()
            };
            let index = first[orientation as usize] * 4;
            assert_eq!(&p[..4], &base[index..index + 4]);
        }
    }
}
