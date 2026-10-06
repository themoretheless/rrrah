//! IFF ILBM/PBM saved bitmap, palette, mask and ByteRun1 rows.
use crate::{
    DecodeRequest,
    raster::{MAX_RASTER_BYTES, RasterDecodeError},
};
use rrrah_core::{DecodedRaster, RasterColorSpace, RasterPixels};
use std::sync::Arc;
fn bad(s: &'static str) -> RasterDecodeError {
    RasterDecodeError::InvalidIff(s)
}
pub(crate) fn has_magic(b: &[u8]) -> bool {
    b.starts_with(b"FORM") && matches!(b.get(8..12), Some(b"ILBM" | b"PBM "))
}
struct Input<'a> {
    b: &'a [u8],
    at: usize,
}
impl<'a> Input<'a> {
    fn take(&mut self, n: usize) -> Result<&'a [u8], RasterDecodeError> {
        let end = self.at.checked_add(n).ok_or_else(|| bad("length overflow"))?;
        let out = self
            .b
            .get(self.at..end)
            .ok_or_else(|| bad("truncated chunk or row"))?;
        self.at = end;
        Ok(out)
    }
}
fn row(input: &mut Input<'_>, out: &mut [u8], compression: u8) -> Result<(), RasterDecodeError> {
    if compression == 0 {
        out.copy_from_slice(input.take(out.len())?);
        return Ok(());
    }
    let mut at = 0;
    while at < out.len() {
        let command = input.take(1)?[0] as i8;
        let n = if command >= 0 {
            command as usize + 1
        } else if command == -128 {
            0
        } else {
            (1 - i16::from(command)) as usize
        };
        if at + n > out.len() {
            return Err(bad("ByteRun1 exceeds row"));
        }
        if command >= 0 {
            out[at..at + n].copy_from_slice(input.take(n)?);
        } else if n > 0 {
            out[at..at + n].fill(input.take(1)?[0]);
        }
        at += n;
    }
    Ok(())
}
pub(crate) fn decode(bytes: &[u8], request: &DecodeRequest) -> Result<DecodedRaster, RasterDecodeError> {
    request.check_cancelled()?;
    if !has_magic(bytes) {
        return Err(bad("unsupported FORM"));
    }
    let len = u32::from_be_bytes(bytes[4..8].try_into().unwrap()) as usize;
    if len < 4 || len.checked_add(8) != Some(bytes.len()) {
        return Err(bad("FORM length mismatch"));
    }
    let planar = &bytes[8..12] == b"ILBM";
    let mut input = Input {
        b: &bytes[12..],
        at: 0,
    };
    let (mut bmhd, mut cmap, mut body, mut camg) = (None, None, None, None);
    let mut count = 0;
    while input.at < input.b.len() {
        count += 1;
        if count > 65536 {
            return Err(bad("too many chunks"));
        }
        let kind = input.take(4)?;
        let size = u32::from_be_bytes(input.take(4)?.try_into().unwrap()) as usize;
        let data = input.take(size)?;
        if size % 2 != 0 {
            input.take(1)?;
        }
        let slot = match kind {
            b"BMHD" => Some(&mut bmhd),
            b"CMAP" => Some(&mut cmap),
            b"BODY" => Some(&mut body),
            b"CAMG" => Some(&mut camg),
            b"PCHG" | b"SHAM" | b"CTBL" | b"CLUT" | b"DCOL" => {
                return Err(bad("dynamic palette or direct color extension pending"));
            }
            _ => None,
        };
        if let Some(slot) = slot {
            if slot.replace(data).is_some() {
                return Err(bad("duplicate structural chunk"));
            }
        }
    }
    let header = bmhd.ok_or_else(|| bad("missing BMHD"))?;
    if header.len() != 20 {
        return Err(bad("BMHD size"));
    }
    let word = |at| u16::from_be_bytes([header[at], header[at + 1]]);
    let (width, height) = (u32::from(word(0)), u32::from(word(2)));
    let planes = usize::from(header[8]);
    let masking = header[9];
    let compression = header[10];
    let transparent = usize::from(word(12));
    if width == 0
        || height == 0
        || compression > 1
        || masking > 2
        || !(1..=8).contains(&planes) && !(planar && planes == 24)
    {
        return Err(bad("dimensions, planes, compression or masking"));
    }
    if !planar && (planes != 8 || masking == 1) {
        return Err(bad("PBM requires eight bit indices without mask plane"));
    }
    let mode = if let Some(data) = camg {
        if data.len() != 4 {
            return Err(bad("CAMG size"));
        }
        u32::from_be_bytes(data.try_into().unwrap())
    } else {
        0
    };
    let ham = mode & 0x800 != 0;
    if ham && (!planar || planes != 6) {
        return Err(bad("HAM8 or invalid HAM layout pending"));
    }
    let ehb = mode & 0x80 != 0;
    if ehb && (!planar || planes != 6 || ham) {
        return Err(bad("invalid EHB mode"));
    }
    let palette = cmap.unwrap_or(&[]);
    if palette.len() % 3 != 0 || palette.len() > 768 || planes != 24 && palette.is_empty() {
        return Err(bad("invalid or absent palette"));
    }
    let mut palette: Vec<[u8; 3]> = palette.chunks_exact(3).map(|v| v.try_into().unwrap()).collect();
    if ehb {
        if palette.len() < 32 {
            return Err(bad("short EHB palette"));
        }
        palette.truncate(32);
        for i in 0..32 {
            palette.push(palette[i].map(|v| v / 2));
        }
    }
    if ham && palette.len() < 16 {
        return Err(bad("short HAM6 palette"));
    }
    let output = u64::from(width) * u64::from(height) * 4;
    let stride = if planar {
        width.div_ceil(16) as usize * 2
    } else {
        width.div_ceil(2) as usize * 2
    };
    let stored_planes = if planar {
        planes + usize::from(masking == 1)
    } else {
        1
    };
    if output + (stride * stored_planes) as u64 > MAX_RASTER_BYTES {
        return Err(RasterDecodeError::OutputTooLarge);
    }
    let mut pixels = Vec::with_capacity(output as usize);
    let mut rows = vec![0u8; stride * stored_planes];
    let mut body = Input {
        b: body.ok_or_else(|| bad("missing BODY"))?,
        at: 0,
    };
    for _ in 0..height {
        request.check_cancelled()?;
        for buffer in rows.chunks_exact_mut(stride) {
            row(&mut body, buffer, compression)?;
        }
        let mut held = palette.first().copied().unwrap_or([0; 3]);
        for x in 0..width as usize {
            let value = if planar {
                (0..planes).fold(0usize, |v, p| {
                    v | (((rows[p * stride + x / 8] >> (7 - x % 8)) & 1) as usize) << p
                })
            } else {
                rows[x] as usize
            };
            let rgb = if ham {
                let component = ((value & 15) * 17) as u8;
                match value >> 4 {
                    0 => {
                        held = *palette
                            .get(value & 15)
                            .ok_or_else(|| bad("HAM palette index outside CMAP"))?
                    }
                    1 => held[2] = component,
                    2 => held[0] = component,
                    3 => held[1] = component,
                    _ => unreachable!(),
                }
                held
            } else if planes == 24 {
                [value as u8, (value >> 8) as u8, (value >> 16) as u8]
            } else {
                *palette
                    .get(value)
                    .ok_or_else(|| bad("palette index outside CMAP"))?
            };
            let visible = match masking {
                1 => rows[planes * stride + x / 8] & (1 << (7 - x % 8)) != 0,
                2 => value != transparent,
                _ => true,
            };
            pixels.extend_from_slice(&[rgb[0], rgb[1], rgb[2], if visible { 255 } else { 0 }]);
        }
    }
    if body.at != body.b.len() {
        return Err(bad("trailing BODY data"));
    }
    Ok(DecodedRaster::new(
        width,
        height,
        RasterPixels::Rgba8(Arc::new(pixels).into()),
        RasterColorSpace::AssumedSrgb,
    )?)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn byterun1_repeats_noops_and_rejects_row_overflow() {
        let mut input = Input {
            b: &[128, 254, 17],
            at: 0,
        };
        let mut out = [0u8; 3];
        row(&mut input, &mut out, 1).unwrap();
        assert_eq!(out, [17; 3]);
        assert!(row(&mut Input { b: &[254, 17], at: 0 }, &mut [0; 2], 1).is_err());
        assert!(row(&mut Input { b: &[1, 17], at: 0 }, &mut [0; 2], 1).is_err());
    }
    #[test]
    fn invalid_chunk_length_and_duplicate_header_are_rejected() {
        let bytes = include_bytes!("../../../tests/fixtures/raster/ilbm-0-0.iff");
        let mut b = bytes.to_vec();
        b[16..20].copy_from_slice(&u32::MAX.to_be_bytes());
        assert!(decode(&b, &DecodeRequest::new("x.iff")).is_err());
        let mut b = bytes.to_vec();
        b.extend_from_slice(&bytes[12..40]);
        let len = (b.len() - 8) as u32;
        b[4..8].copy_from_slice(&len.to_be_bytes());
        assert!(decode(&b, &DecodeRequest::new("x.iff")).is_err());
    }
    #[test]
    fn iff_identity_overrides_raw_suffix() {
        let path = std::env::temp_dir().join(format!("rrrah-iff-magic-{}.cr3", std::process::id()));
        std::fs::write(
            &path,
            include_bytes!("../../../tests/fixtures/raster/ilbm-0-0.iff"),
        )
        .unwrap();
        let result = crate::decode_image_file(&path);
        std::fs::remove_file(path).unwrap();
        assert!(matches!(result, Ok(crate::DecodedImage::Raster(_))));
    }
}
