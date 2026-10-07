//! Versioned streaming raster payload; checksum/publication belong to the swap store.
use rrrah_core::{DecodedRaster, MemoryBudget, PixelBuffer, RasterColorSpace, RasterPixels};
use std::io::{Read, Write};
use thiserror::Error;
const HEADER: usize = 64;
const MAX_PIXELS: u64 = 512 * 1024 * 1024;
const MAX_PROFILE: usize = 1024 * 1024;
#[derive(Debug, Error)]
pub enum RasterPayloadError {
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    Memory(#[from] rrrah_core::BufferError),
    #[error(transparent)]
    Frame(#[from] rrrah_core::RasterError),
    #[error("invalid raster payload: {0}")]
    Invalid(&'static str),
}
fn descriptor(frame: &DecodedRaster) -> Result<([u8; HEADER], &[u8], u64), RasterPayloadError> {
    let (kind, size) = match frame.pixels() {
        RasterPixels::Rgba8(_) => (1, 1),
        RasterPixels::Rgba16(_) => (2, 2),
        RasterPixels::Rgba32Float(_) => (3, 4),
    };
    let (color, profile): (u8, &[u8]) = match frame.color_space() {
        RasterColorSpace::Srgb => (0, &[]),
        RasterColorSpace::AssumedSrgb => (1, &[]),
        RasterColorSpace::LinearSrgb => (2, &[]),
        RasterColorSpace::LinearRgbUnspecified => (3, &[]),
        RasterColorSpace::Icc(p) => (4, p),
        RasterColorSpace::Unspecified => (5, &[]),
        RasterColorSpace::Bt709 => (6, &[]),
        RasterColorSpace::Cicp { .. } => (7, &[]),
    };
    let pixel_bytes = u64::from(frame.width())
        .checked_mul(u64::from(frame.height()))
        .and_then(|value| value.checked_mul(4))
        .and_then(|value| value.checked_mul(size))
        .ok_or(RasterPayloadError::Invalid("dimensions"))?;
    if pixel_bytes > MAX_PIXELS || profile.len() > MAX_PROFILE {
        return Err(RasterPayloadError::Invalid("size limit"));
    }
    let mut h = [0u8; HEADER];
    h[..8].copy_from_slice(b"RRRAST1\0");
    h[8..12].copy_from_slice(&frame.width().to_le_bytes());
    h[12..16].copy_from_slice(&frame.height().to_le_bytes());
    h[16] = kind;
    h[17] = color;
    if let RasterColorSpace::Cicp {
        primaries,
        transfer,
        matrix,
        full_range,
    } = frame.color_space()
    {
        h[52..54].copy_from_slice(&primaries.to_le_bytes());
        h[54..56].copy_from_slice(&transfer.to_le_bytes());
        h[56..58].copy_from_slice(&matrix.to_le_bytes());
        h[58] = u8::from(*full_range);
    }
    h[20..24].copy_from_slice(&frame.sample_scale().to_bits().to_le_bytes());
    if let Some((x, y)) = frame.hotspot() {
        h[18] = 1;
        h[24..28].copy_from_slice(&x.to_le_bytes());
        h[28..32].copy_from_slice(&y.to_le_bytes());
    }
    h[32..40].copy_from_slice(&(frame.image_index() as u64).to_le_bytes());
    h[40..48].copy_from_slice(&(frame.image_count() as u64).to_le_bytes());
    h[48..52].copy_from_slice(&(profile.len() as u32).to_le_bytes());
    Ok((h, profile, HEADER as u64 + profile.len() as u64 + pixel_bytes))
}
pub fn raster_payload_len(frame: &DecodedRaster) -> Result<u64, RasterPayloadError> {
    Ok(descriptor(frame)?.2)
}
pub fn write_raster_payload(
    writer: &mut impl Write,
    frame: &DecodedRaster,
) -> Result<(), RasterPayloadError> {
    let (h, profile, _) = descriptor(frame)?;
    writer.write_all(&h)?;
    writer.write_all(profile)?;
    #[cfg(target_endian = "big")]
    let mut block = [0u8; 16384];
    match frame.pixels() {
        RasterPixels::Rgba8(p) => {
            for chunk in p.chunks(16384) {
                writer.write_all(chunk)?;
            }
        }
        RasterPixels::Rgba16(p) => {
            #[cfg(target_endian = "little")]
            for chunk in p.chunks(8192) {
                writer.write_all(bytemuck::cast_slice(chunk))?;
            }
            #[cfg(target_endian = "big")]
            for chunk in p.chunks(8192) {
                for (v, b) in chunk.iter().zip(block.chunks_exact_mut(2)) {
                    b.copy_from_slice(&v.to_le_bytes());
                }
                writer.write_all(&block[..chunk.len() * 2])?;
            }
        }
        RasterPixels::Rgba32Float(p) => {
            #[cfg(target_endian = "little")]
            for chunk in p.chunks(4096) {
                writer.write_all(bytemuck::cast_slice(chunk))?;
            }
            #[cfg(target_endian = "big")]
            for chunk in p.chunks(4096) {
                for (v, b) in chunk.iter().zip(block.chunks_exact_mut(4)) {
                    b.copy_from_slice(&v.to_bits().to_le_bytes());
                }
                writer.write_all(&block[..chunk.len() * 4])?;
            }
        }
    }
    Ok(())
}
fn read_samples<T: Copy + bytemuck::Pod>(
    reader: &mut impl Read,
    count: usize,
    size: usize,
    budget: &MemoryBudget,
    sample: impl Fn(&[u8]) -> T,
) -> Result<PixelBuffer<T>, RasterPayloadError> {
    let mut values = budget.try_buffer(count, sample(&[0u8; 4][..size]))?;
    // All supported sample types are Pod: arbitrary wire bits can be read into
    // initialized exclusive storage without a staging copy or per-sample loop.
    debug_assert_eq!(size, std::mem::size_of::<T>());
    for chunk in values.chunks_mut(16384 / size) {
        reader.read_exact(bytemuck::cast_slice_mut(chunk))?;
        #[cfg(target_endian = "big")]
        for value in chunk {
            let converted = sample(bytemuck::bytes_of(value));
            *value = converted;
        }
    }
    Ok(values.freeze().into())
}
pub fn read_raster_payload(
    reader: &mut impl Read,
    budget: &MemoryBudget,
) -> Result<DecodedRaster, RasterPayloadError> {
    read_raster_payload_impl(reader, budget, None)
}
/// Reject descriptor/object-length disagreement before allocating profile or pixels.
pub fn read_raster_payload_with_length(
    reader: &mut impl Read,
    bytes: u64,
    budget: &MemoryBudget,
) -> Result<DecodedRaster, RasterPayloadError> {
    if bytes < HEADER as u64 {
        return Err(RasterPayloadError::Invalid("payload length"));
    }
    read_raster_payload_impl(reader, budget, Some(bytes))
}
fn read_raster_payload_impl(
    reader: &mut impl Read,
    budget: &MemoryBudget,
    bytes: Option<u64>,
) -> Result<DecodedRaster, RasterPayloadError> {
    let mut h = [0u8; HEADER];
    reader.read_exact(&mut h)?;
    let invalid_color_extension = if h[17] == 7 {
        h[58] > 1 || h[59..].iter().any(|&b| b != 0)
    } else {
        h[52..].iter().any(|&b| b != 0)
    };
    if &h[..8] != b"RRRAST1\0" || h[19] != 0 || invalid_color_extension || h[18] > 1 {
        return Err(RasterPayloadError::Invalid("header"));
    }
    let u32_at = |i| u32::from_le_bytes(h[i..i + 4].try_into().unwrap());
    let u64_at = |i| u64::from_le_bytes(h[i..i + 8].try_into().unwrap());
    let width = u32_at(8);
    let height = u32_at(12);
    let scale = f32::from_bits(u32_at(20));
    let index = usize::try_from(u64_at(32)).map_err(|_| RasterPayloadError::Invalid("image index"))?;
    let images = usize::try_from(u64_at(40)).map_err(|_| RasterPayloadError::Invalid("image count"))?;
    let profile_len = u32_at(48) as usize;
    let size = match h[16] {
        1 => 1,
        2 => 2,
        3 => 4,
        _ => return Err(RasterPayloadError::Invalid("sample type")),
    };
    let count = u64::from(width)
        .checked_mul(u64::from(height))
        .and_then(|v| v.checked_mul(4))
        .ok_or(RasterPayloadError::Invalid("dimensions"))?;
    if width == 0
        || height == 0
        || count > MAX_PIXELS / size
        || profile_len > MAX_PROFILE
        || h[17] > 7
        || (h[17] != 4 && profile_len != 0)
        || !scale.is_finite()
        || scale <= 0.
        || index >= images
    {
        return Err(RasterPayloadError::Invalid("descriptor"));
    }
    let hotspot = if h[18] == 1 {
        let x = u32_at(24);
        let y = u32_at(28);
        if x >= width || y >= height {
            return Err(RasterPayloadError::Invalid("hotspot"));
        }
        Some((x, y))
    } else {
        if u32_at(24) != 0 || u32_at(28) != 0 {
            return Err(RasterPayloadError::Invalid("unused hotspot"));
        }
        None
    };
    let expected = HEADER as u64 + profile_len as u64 + count * size;
    if bytes.is_some_and(|bytes| bytes != expected) {
        return Err(RasterPayloadError::Invalid("payload length"));
    }
    let mut profile_reservation = budget.try_reserve(profile_len as u64)?;
    let mut profile = Vec::new();
    profile
        .try_reserve_exact(profile_len)
        .map_err(rrrah_core::BufferError::Allocate)?;
    profile_reservation.ensure_bytes(profile.capacity() as u64)?;
    profile.resize(profile_len, 0);
    reader.read_exact(&mut profile)?;
    let color = match h[17] {
        0 => RasterColorSpace::Srgb,
        1 => RasterColorSpace::AssumedSrgb,
        2 => RasterColorSpace::LinearSrgb,
        3 => RasterColorSpace::LinearRgbUnspecified,
        4 => RasterColorSpace::Icc(profile),
        6 => RasterColorSpace::Bt709,
        7 => RasterColorSpace::Cicp {
            primaries: u16::from_le_bytes(h[52..54].try_into().unwrap()),
            transfer: u16::from_le_bytes(h[54..56].try_into().unwrap()),
            matrix: u16::from_le_bytes(h[56..58].try_into().unwrap()),
            full_range: h[58] != 0,
        },
        _ => RasterColorSpace::Unspecified,
    };
    let count = usize::try_from(count).map_err(|_| RasterPayloadError::Invalid("sample count"))?;
    let pixels = match h[16] {
        1 => RasterPixels::Rgba8(read_samples(reader, count, 1, budget, |b| b[0])?),
        2 => RasterPixels::Rgba16(read_samples(reader, count, 2, budget, |b| {
            u16::from_le_bytes(b.try_into().unwrap())
        })?),
        _ => RasterPixels::Rgba32Float(read_samples(reader, count, 4, budget, |b| {
            f32::from_bits(u32::from_le_bytes(b.try_into().unwrap()))
        })?),
    };
    let mut tail = [0u8; 1];
    if reader.read(&mut tail)? != 0 {
        return Err(RasterPayloadError::Invalid("trailing bytes"));
    }
    Ok(DecodedRaster::new(width, height, pixels, color)?
        .with_color_profile_reservation(profile_reservation)?
        .with_sample_scale(scale)?
        .with_hotspot(hotspot)?
        .with_image_selection(index, images)?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{io::Cursor, sync::Arc};
    #[test]
    fn object_length_disagreement_is_invalid_before_any_reservation() {
        let frame = DecodedRaster::new(
            1,
            1,
            RasterPixels::Rgba32Float(Arc::new(vec![4., 2., -0., 1.]).into()),
            RasterColorSpace::Icc(vec![1, 2, 3]),
        )
        .unwrap();
        let mut encoded = Vec::new();
        write_raster_payload(&mut encoded, &frame).unwrap();
        for bytes in [0, 63, encoded.len() as u64 - 1, encoded.len() as u64 + 1] {
            let root = MemoryBudget::new(0);
            assert!(matches!(
                read_raster_payload_with_length(&mut Cursor::new(&encoded), bytes, &root),
                Err(RasterPayloadError::Invalid("payload length"))
            ));
            assert_eq!(root.peak(), 0);
        }
        let mut oversized = encoded.clone();
        oversized[8..12].copy_from_slice(&100000u32.to_le_bytes());
        let root = MemoryBudget::new(0);
        assert!(matches!(
            read_raster_payload_with_length(&mut Cursor::new(&oversized), encoded.len() as u64, &root),
            Err(RasterPayloadError::Invalid("payload length"))
        ));
        assert_eq!(root.peak(), 0);
        assert!(matches!(
            <DecodedRaster as crate::SwapPayload>::read_payload(
                &mut Cursor::new(&oversized),
                encoded.len() as u64,
                &root
            ),
            Err(crate::SwapPayloadError::Invalid(_))
        ));
        let root = MemoryBudget::new(19);
        let restored =
            read_raster_payload_with_length(&mut Cursor::new(&encoded), encoded.len() as u64, &root).unwrap();
        let RasterPixels::Rgba32Float(samples) = restored.pixels() else {
            panic!()
        };
        assert_eq!(
            samples.iter().map(|v| v.to_bits()).collect::<Vec<_>>(),
            [4.0f32, 2., -0., 1.].map(f32::to_bits)
        );
        drop(restored);
        assert_eq!(root.used(), 0);
    }
    #[test]
    fn icc_profile_is_admitted_before_read_and_shared_until_last_owner() {
        let frame = DecodedRaster::new(
            1,
            1,
            RasterPixels::Rgba32Float(Arc::new(vec![4.0, 2.0, -0.5, 1.0]).into()),
            RasterColorSpace::Icc(vec![1, 2, 3]),
        )
        .unwrap();
        let mut encoded = Vec::new();
        write_raster_payload(&mut encoded, &frame).unwrap();
        for limit in [2, 18] {
            let budget = MemoryBudget::new(limit);
            let mut reader = Cursor::new(&encoded);
            assert!(matches!(
                read_raster_payload(&mut reader, &budget),
                Err(RasterPayloadError::Memory(_))
            ));
            if limit == 2 {
                assert_eq!(reader.position(), HEADER as u64);
            }
            assert_eq!(budget.used(), 0);
        }
        let budget = MemoryBudget::new(19);
        let restored = read_raster_payload(&mut Cursor::new(&encoded), &budget).unwrap();
        let shared = restored.clone();
        let (RasterColorSpace::Icc(first), RasterColorSpace::Icc(second)) =
            (restored.color_space(), shared.color_space())
        else {
            panic!("ICC lost")
        };
        assert_eq!(first.as_ptr(), second.as_ptr(), "cloning must not copy ICC bytes");
        assert_eq!(budget.used(), 19);
        drop(restored);
        assert_eq!(budget.used(), 19);
        drop(shared);
        assert_eq!(budget.used(), 0);
    }
    #[test]
    fn all_precisions_roundtrip_bits_color_and_selection_without_output_copy() {
        for pixels in [
            RasterPixels::Rgba8(Arc::new(vec![1u8, 2, 3, 4]).into()),
            RasterPixels::Rgba16(Arc::new(vec![1u16, 257, 32769, 65535]).into()),
            RasterPixels::Rgba32Float(Arc::new(vec![4.0f32, -0.0, f32::from_bits(0x7fc00001), 1.0]).into()),
        ] {
            let frame = DecodedRaster::new(1, 1, pixels, RasterColorSpace::Icc(vec![1, 2, 3]))
                .unwrap()
                .with_hotspot(Some((0, 0)))
                .unwrap()
                .with_sample_scale(2.5)
                .unwrap()
                .with_image_selection(2, 4)
                .unwrap();
            let mut encoded = Vec::new();
            write_raster_payload(&mut encoded, &frame).unwrap();
            assert_eq!(encoded.len() as u64, raster_payload_len(&frame).unwrap());
            let budget = MemoryBudget::new(19);
            let restored = read_raster_payload(&mut Cursor::new(&encoded), &budget).unwrap();
            assert_eq!(restored.color_space(), frame.color_space());
            assert_eq!(restored.hotspot(), frame.hotspot());
            assert_eq!(restored.sample_scale().to_bits(), frame.sample_scale().to_bits());
            assert_eq!((restored.image_index(), restored.image_count()), (2, 4));
            match (frame.pixels(), restored.pixels()) {
                (RasterPixels::Rgba8(a), RasterPixels::Rgba8(b)) => {
                    assert!(b.is_managed());
                    assert_eq!(a.as_slice(), b.as_slice());
                    assert_eq!(&encoded[67..], a.as_slice());
                }
                (RasterPixels::Rgba16(a), RasterPixels::Rgba16(b)) => {
                    assert!(b.is_managed());
                    assert_eq!(a.as_slice(), b.as_slice());
                    assert_eq!(
                        &encoded[67..],
                        a.iter().flat_map(|v| v.to_le_bytes()).collect::<Vec<_>>()
                    );
                }
                (RasterPixels::Rgba32Float(a), RasterPixels::Rgba32Float(b)) => {
                    assert!(b.is_managed());
                    assert_eq!(
                        a.iter().map(|v| v.to_bits()).collect::<Vec<_>>(),
                        b.iter().map(|v| v.to_bits()).collect::<Vec<_>>()
                    );
                    assert_eq!(
                        &encoded[67..],
                        a.iter()
                            .flat_map(|v| v.to_bits().to_le_bytes())
                            .collect::<Vec<_>>()
                    );
                }
                _ => panic!("precision changed"),
            }
            let retained = restored.clone();
            let bytes = restored.capacity_bytes();
            drop(restored);
            assert_eq!(budget.used(), bytes);
            drop(retained);
            assert_eq!(budget.used(), 0);
        }
    }
    #[test]
    fn cicp_extension_rejects_invalid_range_reserved_bytes_and_color_aliasing() {
        let frame = DecodedRaster::new(
            1,
            1,
            RasterPixels::Rgba16(Arc::new(vec![123, 456, 789, 65535]).into()),
            RasterColorSpace::Cicp {
                primaries: 9,
                transfer: 16,
                matrix: 9,
                full_range: true,
            },
        )
        .unwrap();
        assert!(frame.to_linear_srgb().is_err());
        let mut bytes = Vec::new();
        write_raster_payload(&mut bytes, &frame).unwrap();
        for (offset, value) in [(58, 2), (59, 1), (17, 0)] {
            let mut malformed = bytes.clone();
            malformed[offset] = value;
            let budget = MemoryBudget::new(8);
            assert!(read_raster_payload(&mut Cursor::new(malformed), &budget).is_err());
            assert_eq!(budget.used(), 0);
        }
        let budget = MemoryBudget::new(8);
        let restored = read_raster_payload(&mut Cursor::new(bytes), &budget).unwrap();
        assert_eq!(restored.color_space(), frame.color_space());
        assert!(restored.to_linear_srgb().is_err());
        assert_eq!(budget.used(), 8);
        drop(restored);
        assert_eq!(budget.used(), 0);
    }
    #[test]
    fn every_color_declaration_roundtrips_default_metadata() {
        for color in [
            RasterColorSpace::Srgb,
            RasterColorSpace::AssumedSrgb,
            RasterColorSpace::LinearSrgb,
            RasterColorSpace::LinearRgbUnspecified,
            RasterColorSpace::Icc(vec![1, 2, 3]),
            RasterColorSpace::Unspecified,
            RasterColorSpace::Bt709,
            RasterColorSpace::Cicp {
                primaries: 9,
                transfer: 16,
                matrix: 9,
                full_range: true,
            },
            RasterColorSpace::Cicp {
                primaries: 9,
                transfer: 18,
                matrix: 9,
                full_range: false,
            },
        ] {
            let frame = DecodedRaster::new(
                1,
                1,
                RasterPixels::Rgba8(Arc::new(vec![1, 2, 3, 4]).into()),
                color,
            )
            .unwrap();
            let mut encoded = Vec::new();
            write_raster_payload(&mut encoded, &frame).unwrap();
            let budget = MemoryBudget::new(frame.capacity_bytes());
            let result = read_raster_payload(&mut Cursor::new(encoded), &budget).unwrap();
            assert_eq!(result.color_space(), frame.color_space());
            assert_eq!(result.hotspot(), None);
            assert_eq!((result.image_index(), result.image_count()), (0, 1));
            assert_eq!(result.sample_scale(), 1.0);
            drop(result);
            assert_eq!(budget.used(), 0);
        }
    }
    #[test]
    fn malformed_truncated_trailing_and_admission_errors_release_reservations() {
        let frame = DecodedRaster::new(
            1,
            1,
            RasterPixels::Rgba32Float(Arc::new(vec![4f32, 2., -0.5, 1.]).into()),
            RasterColorSpace::LinearSrgb,
        )
        .unwrap();
        let mut encoded = Vec::new();
        write_raster_payload(&mut encoded, &frame).unwrap();
        let budget = MemoryBudget::new(16);
        for end in 0..encoded.len() {
            assert!(read_raster_payload(&mut Cursor::new(&encoded[..end]), &budget).is_err());
            assert_eq!(budget.used(), 0);
        }
        for offset in [0, 16, 17, 18, 19, 52] {
            let mut invalid = encoded.clone();
            invalid[offset] = 255;
            assert!(read_raster_payload(&mut Cursor::new(invalid), &budget).is_err());
            assert_eq!(budget.used(), 0);
        }
        let mut trailing = encoded.clone();
        trailing.push(0);
        assert!(read_raster_payload(&mut Cursor::new(trailing), &budget).is_err());
        assert_eq!(budget.used(), 0);
        let tight = MemoryBudget::new(15);
        assert!(matches!(
            read_raster_payload(&mut Cursor::new(&encoded), &tight),
            Err(RasterPayloadError::Memory(_))
        ));
        assert_eq!(tight.used(), 0);
        assert!(read_raster_payload(&mut Cursor::new(encoded), &budget).is_ok());
        assert_eq!(budget.used(), 0);
    }
}

#[cfg(test)]
mod block_boundary_tests {
    use super::*;
    use std::sync::Arc;
    #[test]
    fn direct_io_preserves_wire_bits_across_blocks_and_partial_tail() {
        for pixels in [
            RasterPixels::Rgba8(Arc::new((0..4097 * 4).map(|n| n as u8).collect::<Vec<_>>()).into()),
            RasterPixels::Rgba16(
                Arc::new(
                    (0..4097 * 4)
                        .map(|n| (n as u16).wrapping_mul(17))
                        .collect::<Vec<_>>(),
                )
                .into(),
            ),
            RasterPixels::Rgba32Float(
                Arc::new(
                    (0..4097 * 4)
                        .map(|n| f32::from_bits([0x80000000, 0x7fc01234, 0x41800000, 0x3f800000][n % 4]))
                        .collect::<Vec<_>>(),
                )
                .into(),
            ),
        ] {
            let expected: Vec<u8> = match &pixels {
                RasterPixels::Rgba8(p) => p.to_vec(),
                RasterPixels::Rgba16(p) => p.iter().flat_map(|v| v.to_le_bytes()).collect(),
                RasterPixels::Rgba32Float(p) => p.iter().flat_map(|v| v.to_bits().to_le_bytes()).collect(),
            };
            let frame = DecodedRaster::new(4097, 1, pixels, RasterColorSpace::Unspecified).unwrap();
            let mut encoded = Vec::new();
            write_raster_payload(&mut encoded, &frame).unwrap();
            assert_eq!(&encoded[HEADER..], &expected);
            let budget = MemoryBudget::new(expected.len() as u64);
            let restored = read_raster_payload(&mut encoded.as_slice(), &budget).unwrap();
            let mut again = Vec::new();
            write_raster_payload(&mut again, &restored).unwrap();
            assert_eq!(again, encoded);
            assert_eq!(budget.used(), expected.len() as u64);
            drop(restored);
            assert_eq!(budget.used(), 0);
        }
    }
}
