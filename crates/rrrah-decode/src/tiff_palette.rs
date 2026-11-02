//! Expand native 1/2/4/8/16-bit TIFF indices through the original 16-bit ColorMap.
//! Compression, strips/tiles, orientation and ICC extraction use the existing
//! grayscale codec, with only the selected directory's photometric field
//! temporarily overlaid. No source copy or palette quantization is required.
use crate::raster::{self, RasterDecodeError};
use crate::{DecodeError, DecodeRequest};
use rrrah_core::{DecodedRaster, RasterPixels};

fn bad(reason: &'static str) -> RasterDecodeError {
    RasterDecodeError::InvalidTiffDirectory(reason)
}

struct Palette {
    photometric: usize,
    little: bool,
    channels: [u16; 768],
    entries: usize,
    map_start: usize,
}
impl Palette {
    fn color(&self, bytes: &[u8], index: usize) -> [u16; 4] {
        let channel = |c: usize| {
            if self.entries <= 256 {
                return self.channels[c * self.entries + index];
            }
            // The entire table extent was checked before entering the codec.
            let at = self.map_start + (c * self.entries + index) * 2;
            let value = [bytes[at], bytes[at + 1]];
            if self.little {
                u16::from_le_bytes(value)
            } else {
                u16::from_be_bytes(value)
            }
        };
        [channel(0), channel(1), channel(2), 65535]
    }
}

fn palette(bytes: &[u8], request: &DecodeRequest) -> Result<Option<Palette>, RasterDecodeError> {
    let little = bytes.starts_with(b"II");
    let number = |at: usize, size: usize| -> Result<u64, RasterDecodeError> {
        let data = bytes
            .get(
                at..at
                    .checked_add(size)
                    .ok_or_else(|| bad("palette offset overflow"))?,
            )
            .ok_or_else(|| bad("truncated palette directory"))?;
        Ok(if little {
            data.iter().rev().fold(0, |n, b| (n << 8) | u64::from(*b))
        } else {
            data.iter().fold(0, |n, b| (n << 8) | u64::from(*b))
        })
    };
    let (offset_at, pointer, count_size, entry_size, value_at) = match number(2, 2)? {
        42 => (4, 4, 2, 12, 8),
        43 if number(4, 2)? == 8 && number(6, 2)? == 0 => (8, 8, 8, 20, 12),
        _ => return Err(bad("palette TIFF version")),
    };
    let directory =
        usize::try_from(number(offset_at, pointer)?).map_err(|_| bad("palette directory offset"))?;
    let count = usize::try_from(number(directory, count_size)?).map_err(|_| bad("palette entry count"))?;
    if count > 4096 {
        return Err(bad("excessive palette directory entries"));
    }
    let end = directory
        .checked_add(count_size)
        .and_then(|n| n.checked_add(count.checked_mul(entry_size)?))
        .ok_or_else(|| bad("palette directory overflow"))?;
    number(end, pointer)?;
    let entry = |tag: u64| -> Result<Option<usize>, RasterDecodeError> {
        let mut found = None;
        for i in 0..count {
            request.check_cancelled()?;
            let at = directory + count_size + i * entry_size;
            if number(at, 2)? == tag {
                if found.replace(at).is_some() {
                    return Err(bad("duplicate palette field"));
                }
            }
        }
        Ok(found)
    };
    let scalar = |at: usize| -> Result<u64, RasterDecodeError> {
        if number(at + 2, 2)? != 3 || number(at + 4, pointer)? != 1 {
            return Err(bad("palette scalar must be one SHORT"));
        }
        number(at + value_at, 2)
    };
    let Some(photo) = entry(262)? else { return Ok(None) };
    if scalar(photo)? != 3 {
        return Ok(None);
    }
    let bits = scalar(entry(258)?.ok_or_else(|| bad("missing palette bit depth"))?)?;
    if !matches!(bits, 1 | 2 | 4 | 8 | 16) {
        return Err(bad("palette TIFF currently requires 1/2/4/8/16-bit indices"));
    }
    let entries = 1usize << bits;
    for tag in [277, 339] {
        if let Some(at) = entry(tag)? {
            if scalar(at)? != 1 {
                return Err(bad("palette requires one unsigned sample"));
            }
        }
    }
    if entry(338)?.is_some() {
        return Err(bad("palette ExtraSamples unsupported"));
    }
    let map = entry(320)?.ok_or_else(|| bad("missing TIFF ColorMap"))?;
    if number(map + 2, 2)? != 3 || number(map + 4, pointer)? != (entries * 3) as u64 {
        return Err(bad("TIFF ColorMap length differs from palette bit depth"));
    }
    let start = usize::try_from(number(map + value_at, pointer)?).map_err(|_| bad("ColorMap offset"))?;
    let map_end = start
        .checked_add(entries * 6)
        .ok_or_else(|| bad("ColorMap extent overflow"))?;
    if start < offset_at + pointer || (start < end + pointer && directory < map_end) {
        return Err(bad("ColorMap overlaps TIFF directory"));
    }
    if map_end > bytes.len() {
        return Err(bad("truncated TIFF ColorMap"));
    }
    let mut channels = [0; 768];
    if entries <= 256 {
        for (i, value) in channels[..entries * 3].iter_mut().enumerate() {
            *value = number(start + i * 2, 2)? as u16;
        }
    }
    Ok(Some(Palette {
        photometric: photo + value_at,
        little,
        channels,
        entries,
        map_start: start,
    }))
}

pub(super) fn decode(bytes: &mut [u8], request: &DecodeRequest) -> Result<DecodedRaster, RasterDecodeError> {
    request.check_cancelled()?;
    let Some(palette) = palette(bytes, request)? else {
        return raster::decode_raster_bytes_inner(bytes, request);
    };
    let at = palette.photometric;
    let original: [u8; 2] = bytes[at..at + 2].try_into().unwrap();
    bytes[at..at + 2].copy_from_slice(&if palette.little {
        1u16.to_le_bytes()
    } else {
        1u16.to_be_bytes()
    });
    let decoded = raster::decode_raster_bytes_inner(&mut *bytes, request);
    bytes[at..at + 2].copy_from_slice(&original);
    let mut raster = decoded?;
    if palette.entries == 65536 {
        let values = raster
            .rgba16_mut()
            .ok_or_else(|| bad("unexpected 16-bit palette index ownership"))?;
        for (i, pixel) in values.chunks_exact_mut(4).enumerate() {
            if i % 4096 == 0 {
                request.check_cancelled()?;
            }
            if pixel[1] != pixel[0] || pixel[2] != pixel[0] || pixel[3] != 65535 {
                return Err(bad("invalid 16-bit palette index raster"));
            }
            pixel.copy_from_slice(&palette.color(bytes, usize::from(pixel[0])));
        }
        request.check_cancelled()?;
        return Ok(raster);
    }
    let RasterPixels::Rgba8(indices) = raster.pixels() else {
        return Err(bad("unexpected palette index storage"));
    };
    let length = indices.len();
    let output_bytes = u64::try_from(length)
        .ok()
        .and_then(|n| n.checked_mul(2))
        .ok_or(RasterDecodeError::OutputTooLarge)?;
    if output_bytes > raster::MAX_RASTER_BYTES {
        return Err(RasterDecodeError::OutputTooLarge);
    }
    let reservation = request
        .memory_budget
        .as_ref()
        .map(|budget| budget.try_reserve(output_bytes))
        .transpose()
        .map_err(DecodeError::Memory)?;
    let mut output = Vec::new();
    output
        .try_reserve_exact(length)
        .map_err(|_| RasterDecodeError::OutputTooLarge)?;
    for (i, pixel) in indices.chunks_exact(4).enumerate() {
        if i % 4096 == 0 {
            request.check_cancelled()?;
        }
        let step = (255 / (palette.entries - 1)) as u8;
        let index = usize::from(pixel[0] / step);
        if pixel[0] % step != 0 || pixel[1] != pixel[0] || pixel[2] != pixel[0] || pixel[3] != 255 {
            return Err(bad("invalid palette index raster"));
        }
        output.extend_from_slice(&palette.color(bytes, index));
    }
    request.check_cancelled()?;
    let pixels = raster::adopt_raster_output(output, reservation)?;
    Ok(raster.with_replaced_pixels(RasterPixels::Rgba16(pixels))?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::raster::decode_raster;
    use rrrah_core::MemoryBudget;

    #[test]
    fn original_go_deflate_palette_matches_independent_pillow_all_pixels() {
        let root = MemoryBudget::new(2 * 1024 * 1024);
        let mut request = DecodeRequest::new(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../../tests/fixtures/raster/go-video-001-paletted.tiff"),
        );
        request.memory_budget = Some(root.clone());
        let frame = decode_raster(&request).unwrap();
        assert_eq!((frame.width(), frame.height()), (150, 103));
        let RasterPixels::Rgba16(values) = frame.pixels() else {
            panic!()
        };
        let oracle = include_bytes!("../../../tests/fixtures/raster/go-video-001-paletted.rgba16le");
        assert_eq!(values.len() * 2, oracle.len());
        for (a, b) in values.iter().zip(oracle.chunks_exact(2)) {
            assert_eq!(*a, u16::from_le_bytes([b[0], b[1]]));
        }
        drop(frame);
        assert_eq!(root.used(), 0);
        request.path.set_file_name("go-invalid-palette-ref.tiff");
        assert!(decode_raster(&request).is_err());
        assert_eq!(root.used(), 0);
    }

    #[test]
    fn palette_preserves_16bit_values_both_endians_and_managed_overlap() {
        for original in [
            include_bytes!("../../../tests/fixtures/raster/palette16-le.tiff").as_slice(),
            include_bytes!("../../../tests/fixtures/raster/palette16-be.tiff").as_slice(),
            include_bytes!("../../../tests/fixtures/raster/palette16-big-le.tiff").as_slice(),
            include_bytes!("../../../tests/fixtures/raster/palette16-big-be.tiff").as_slice(),
        ] {
            let root = MemoryBudget::new(96);
            let mut request = DecodeRequest::new("palette.tiff");
            request.memory_budget = Some(root.clone());
            let mut bytes = original.to_vec();
            let frame = decode(&mut bytes, &request).unwrap();
            assert_eq!(bytes, original);
            let RasterPixels::Rgba16(values) = frame.pixels() else {
                panic!()
            };
            for (pixel, i) in values.chunks_exact(4).zip([0u32, 1, 127, 255, 17, 31, 64, 128]) {
                assert_eq!(
                    pixel,
                    &[
                        (i * 251) as u16,
                        (65535 - i * 251) as u16,
                        ((i * i * 31) % 65536) as u16,
                        65535
                    ]
                );
            }
            assert_eq!(root.used(), 64);
            assert_eq!(root.peak(), 96);
            drop(frame);
            assert_eq!(root.used(), 0);
            let short = MemoryBudget::new(95);
            request.memory_budget = Some(short.clone());
            assert!(decode(&mut bytes, &request).is_err());
            assert_eq!(bytes, original);
            assert_eq!(short.used(), 0);
        }
    }

    #[test]
    fn palette_decoder_failure_restores_source_overlay() {
        let mut bytes = include_bytes!("../../../tests/fixtures/raster/palette16-le.tiff").to_vec();
        bytes[54..56].copy_from_slice(&65000u16.to_le_bytes());
        let expected = bytes.clone();
        assert!(decode(&mut bytes, &DecodeRequest::new("bad.tiff")).is_err());
        assert_eq!(bytes, expected);
    }
}

#[cfg(test)]
mod packed_tests {
    use super::*;
    #[test]
    fn packed_palette_rows_keep_indices_and_ignore_padding() {
        for bits in [1, 2, 4] {
            for suffix in ["le", "be", "big-le", "big-be"] {
                let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(format!(
                    "../../tests/fixtures/raster/palette-packed{bits}-{suffix}.tiff"
                ));
                let root = rrrah_core::MemoryBudget::new(4096);
                let mut request = DecodeRequest::new(path);
                request.memory_budget = Some(root.clone());
                let frame = raster::decode_raster(&request).unwrap();
                assert_eq!((frame.width(), frame.height()), (5, 3));
                let RasterPixels::Rgba16(values) = frame.pixels() else {
                    panic!()
                };
                for (position, pixel) in values.chunks_exact(4).enumerate() {
                    let i = ((position % 5 + position / 5 * 3) % (1 << bits)) as u32;
                    assert_eq!(
                        pixel,
                        &[
                            (i * 3911) as u16,
                            (65535 - i * 3911) as u16,
                            ((i * i * 997) % 65536) as u16,
                            65535
                        ]
                    );
                }
                drop(frame);
                assert_eq!(root.used(), 0);
                // The codec extension must also handle an ordinary gray image,
                // not only the palette adapter's temporary grayscale view.
                let mut bytes = std::fs::read(&request.path).unwrap();
                let photo = if suffix.starts_with("big-") { 116 } else { 66 };
                bytes[photo..photo + 2].copy_from_slice(&if suffix.ends_with("le") {
                    1u16.to_le_bytes()
                } else {
                    1u16.to_be_bytes()
                });
                let gray = decode(&mut bytes, &request).unwrap();
                let RasterPixels::Rgba8(values) = gray.pixels() else {
                    panic!()
                };
                for (position, pixel) in values.chunks_exact(4).enumerate() {
                    let i = (position % 5 + position / 5 * 3) % (1 << bits);
                    let value = (i * 255 / ((1 << bits) - 1)) as u8;
                    assert_eq!(pixel, &[value, value, value, 255]);
                }
                drop(gray);
                assert_eq!(root.used(), 0);
            }
        }
    }
}

#[cfg(test)]
mod wide_tests {
    use super::*;
    #[test]
    fn full_u16_palette_indices_preserve_high_bits_and_managed_storage() {
        for suffix in ["le", "be", "big-le", "big-be"] {
            let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(format!(
                "../../tests/fixtures/raster/palette-wide16-{suffix}.tiff"
            ));
            let original = std::fs::read(path).unwrap();
            let mut bytes = original.clone();
            let root = rrrah_core::MemoryBudget::new(100);
            let mut request = DecodeRequest::new("wide.tiff");
            request.memory_budget = Some(root.clone());
            let frame = decode(&mut bytes, &request).unwrap();
            let RasterPixels::Rgba16(values) = frame.pixels() else {
                panic!()
            };
            for (pixel, i) in values
                .chunks_exact(4)
                .zip([0u32, 1, 255, 256, 4095, 32767, 32768, 40000, 65534, 65535])
            {
                assert_eq!(
                    pixel,
                    &[
                        ((i * 251) % 65536) as u16,
                        (65535 - i) as u16,
                        ((i * 73 + 19) % 65536) as u16,
                        65535
                    ]
                );
            }
            assert_eq!(bytes, original);
            assert_eq!(root.used(), 80);
            assert_eq!(root.peak(), 100);
            drop(frame);
            assert_eq!(root.used(), 0);
            let short = rrrah_core::MemoryBudget::new(99);
            request.memory_budget = Some(short.clone());
            assert!(decode(&mut bytes, &request).is_err());
            assert_eq!(short.used(), 0);
            assert_eq!(bytes, original);
            let zero = rrrah_core::MemoryBudget::new(0);
            request.memory_budget = Some(zero.clone());
            let mut truncated = original[..original.len() - 21].to_vec();
            assert!(matches!(
                decode(&mut truncated, &request),
                Err(RasterDecodeError::InvalidTiffDirectory("truncated TIFF ColorMap"))
            ));
            let mut outside = original.clone();
            let (offset, size) = if suffix.starts_with("big-") {
                (216, 8)
            } else {
                (126, 4)
            };
            outside[offset..offset + size].fill(255);
            assert!(matches!(
                decode(&mut outside, &request),
                Err(RasterDecodeError::InvalidTiffDirectory(_))
            ));
            assert_eq!(zero.peak(), 0);
            assert_eq!(zero.used(), 0);
        }
    }
    #[test]
    fn dense_u16_palette_checks_every_index_without_second_rgba_buffer() {
        for suffix in ["le", "be", "big-le", "big-be"] {
            let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(format!(
                "../../tests/fixtures/raster/palette-wide16-{suffix}.tiff"
            ));
            let original = std::fs::read(path).unwrap();
            let mut bytes = original[..original.len() - 20].to_vec();
            let little = suffix.ends_with("le");
            let (first_value, stride) = if suffix.starts_with("big-") {
                (36, 20)
            } else {
                (18, 12)
            };
            for (entry, value) in [(0, 256u32), (1, 256), (7, 256), (8, 131072)] {
                let at = first_value + entry * stride;
                bytes[at..at + 4].copy_from_slice(&if little {
                    value.to_le_bytes()
                } else {
                    value.to_be_bytes()
                });
            }
            for i in 0..=65535u16 {
                bytes.extend_from_slice(&if little { i.to_le_bytes() } else { i.to_be_bytes() });
            }
            let root = rrrah_core::MemoryBudget::new(655360);
            let mut request = DecodeRequest::new("dense.tiff");
            request.memory_budget = Some(root.clone());
            let frame = decode(&mut bytes, &request).unwrap();
            assert_eq!((frame.width(), frame.height()), (256, 256));
            let RasterPixels::Rgba16(values) = frame.pixels() else {
                panic!()
            };
            for (i, pixel) in values.chunks_exact(4).enumerate() {
                let i = i as u32;
                assert_eq!(
                    pixel,
                    &[
                        ((i * 251) % 65536) as u16,
                        (65535 - i) as u16,
                        ((i * 73 + 19) % 65536) as u16,
                        65535
                    ]
                );
            }
            assert_eq!(root.used(), 524288);
            assert_eq!(root.peak(), 655360);
            drop(frame);
            assert_eq!(root.used(), 0);
        }
    }
}
