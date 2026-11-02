//! Bounded TIFF Group3/4 through application-owned row decoding.
use crate::raster::{self, RasterDecodeError, tiff_profile};
use crate::{DecodeError, DecodeRequest};
use rrrah_core::{DecodedRaster, RasterColorSpace, RasterPixels};
#[path = "tiff_fax_mixed.rs"]
mod mixed;
fn bad(reason: &'static str) -> RasterDecodeError {
    RasterDecodeError::InvalidTiffDirectory(reason)
}
struct Directory<'a> {
    bytes: &'a [u8],
    little: bool,
    directory: usize,
    count: usize,
    pointer: usize,
    count_size: usize,
    entry_size: usize,
    value_at: usize,
}
impl<'a> Directory<'a> {
    fn number(&self, at: usize, size: usize) -> Result<u64, RasterDecodeError> {
        let data = self
            .bytes
            .get(at..at.checked_add(size).ok_or_else(|| bad("fax offset overflow"))?)
            .ok_or_else(|| bad("truncated fax directory"))?;
        Ok(if self.little {
            data.iter().rev().fold(0, |n, b| (n << 8) | u64::from(*b))
        } else {
            data.iter().fold(0, |n, b| (n << 8) | u64::from(*b))
        })
    }
    fn new(bytes: &'a [u8]) -> Result<Self, RasterDecodeError> {
        let mut d = Self {
            bytes,
            little: bytes.starts_with(b"II"),
            directory: 0,
            count: 0,
            pointer: 4,
            count_size: 2,
            entry_size: 12,
            value_at: 8,
        };
        let offset = match d.number(2, 2)? {
            42 => 4,
            43 if d.number(4, 2)? == 8 && d.number(6, 2)? == 0 => {
                d.pointer = 8;
                d.count_size = 8;
                d.entry_size = 20;
                d.value_at = 12;
                8
            }
            _ => return Err(bad("fax TIFF version")),
        };
        d.directory =
            usize::try_from(d.number(offset, d.pointer)?).map_err(|_| bad("fax directory offset"))?;
        d.count =
            usize::try_from(d.number(d.directory, d.count_size)?).map_err(|_| bad("fax directory count"))?;
        if d.count > 4096 {
            return Err(bad("excessive fax directory entries"));
        }
        let end = d
            .directory
            .checked_add(d.count_size)
            .and_then(|n| n.checked_add(d.count.checked_mul(d.entry_size)?))
            .ok_or_else(|| bad("fax directory overflow"))?;
        d.number(end, d.pointer)?;
        Ok(d)
    }
    fn values(
        &self,
        tag: u64,
        request: &DecodeRequest,
    ) -> Result<Option<(usize, usize, usize)>, RasterDecodeError> {
        let mut result = None;
        for i in 0..self.count {
            request.check_cancelled()?;
            let at = self.directory + self.count_size + i * self.entry_size;
            if self.number(at, 2)? != tag {
                continue;
            }
            if result.is_some() {
                return Err(bad("duplicate fax field"));
            }
            let size = match self.number(at + 2, 2)? {
                3 => 2,
                4 => 4,
                16 => 8,
                _ => return Err(bad("fax field must be unsigned integer")),
            };
            let count =
                usize::try_from(self.number(at + 4, self.pointer)?).map_err(|_| bad("fax field count"))?;
            let bytes = count
                .checked_mul(size)
                .ok_or_else(|| bad("fax field length overflow"))?;
            let start = if bytes <= self.pointer {
                at + self.value_at
            } else {
                usize::try_from(self.number(at + self.value_at, self.pointer)?)
                    .map_err(|_| bad("fax field offset"))?
            };
            let end = start
                .checked_add(bytes)
                .ok_or_else(|| bad("fax field extent overflow"))?;
            self.bytes
                .get(start..end)
                .ok_or_else(|| bad("truncated fax field"))?;
            result = Some((start, size, count));
        }
        Ok(result)
    }
    fn strip(
        &self,
        offsets: (usize, usize, usize),
        lengths: (usize, usize, usize),
        index: usize,
    ) -> Result<&'a [u8], RasterDecodeError> {
        let at = usize::try_from(self.number(offsets.0 + index * offsets.1, offsets.1)?)
            .map_err(|_| bad("fax strip offset"))?;
        let count = usize::try_from(self.number(lengths.0 + index * lengths.1, lengths.1)?)
            .map_err(|_| bad("fax strip length"))?;
        let end = at
            .checked_add(count)
            .ok_or_else(|| bad("fax strip extent overflow"))?;
        self.bytes.get(at..end).ok_or_else(|| bad("truncated fax strip"))
    }
    fn scalar(
        &self,
        tag: u64,
        default: Option<u64>,
        request: &DecodeRequest,
    ) -> Result<u64, RasterDecodeError> {
        match self.values(tag, request)? {
            Some((at, size, 1)) => self.number(at, size),
            None => default.ok_or_else(|| bad("missing fax scalar")),
            _ => Err(bad("fax scalar count")),
        }
    }
}

pub(super) fn decode(
    bytes: &[u8],
    request: &DecodeRequest,
) -> Result<Option<DecodedRaster>, RasterDecodeError> {
    decode_with_cursor::<true>(bytes, request)
}
fn decode_with_cursor<const CURSOR: bool>(
    bytes: &[u8],
    request: &DecodeRequest,
) -> Result<Option<DecodedRaster>, RasterDecodeError> {
    request.check_cancelled()?;
    let d = Directory::new(bytes)?;
    let compression = d.scalar(259, Some(1), request)?;
    if !matches!(compression, 2 | 3 | 4 | 32771) {
        return Ok(None);
    }
    let width =
        u16::try_from(d.scalar(256, None, request)?).map_err(|_| bad("fax width exceeds row codec"))?;
    let height = u32::try_from(d.scalar(257, None, request)?).map_err(|_| bad("fax height"))?;
    let photo = d.scalar(262, None, request)?;
    let fill = d.scalar(266, Some(1), request)?;
    let options = if matches!(compression, 2 | 32771) {
        0
    } else {
        d.scalar(if compression == 3 { 292 } else { 293 }, Some(0), request)?
    };
    if width == 0
        || height == 0
        || photo > 1
        || !matches!(fill, 1 | 2)
        || d.scalar(258, Some(1), request)? != 1
        || d.scalar(277, Some(1), request)? != 1
        || d.scalar(284, Some(1), request)? != 1
        || d.scalar(339, Some(1), request)? != 1
        || d.values(338, request)?.is_some()
    {
        return Err(bad("unsupported fax sample layout"));
    }
    if compression == 3 && options & !5 != 0 {
        return Err(bad("Fax3 uncompressed mode or unknown options unsupported"));
    }
    if compression == 4 && options != 0 {
        return Err(bad("Fax4 uncompressed mode or unknown options unsupported"));
    }
    let orientation = d.scalar(274, Some(1), request)?;
    if !(1..=8).contains(&orientation) {
        return Err(bad("fax orientation"));
    }
    let rows_per_strip = d.scalar(278, Some(u32::MAX as u64), request)?;
    if rows_per_strip == 0 {
        return Err(bad("zero fax rows per strip"));
    }
    let strips = u64::from(height).div_ceil(rows_per_strip);
    let offsets = d
        .values(273, request)?
        .ok_or_else(|| bad("missing fax strip offsets"))?;
    let lengths = d
        .values(279, request)?
        .ok_or_else(|| bad("missing fax strip lengths"))?;
    if offsets.2 as u64 != strips || lengths.2 as u64 != strips {
        return Err(bad("fax strip count"));
    }
    let row_bytes = usize::from(width) * 4 + 512;
    let mut maximum_row_input = 0usize;
    for strip in 0..strips as usize {
        request.check_cancelled()?;
        let source = d.strip(offsets, lengths, strip)?;
        if source.is_empty() {
            return Err(bad("empty fax strip"));
        }
        maximum_row_input = maximum_row_input.max(source.len().min(row_bytes));
        if compression != 3 {
            continue;
        }
        let first_one = source.iter().take(3).enumerate().find_map(|(byte_at, byte)| {
            let value = if fill == 2 { byte.reverse_bits() } else { *byte };
            (value != 0).then(|| byte_at * 8 + value.leading_zeros() as usize)
        });
        if !first_one.is_some_and(|n| (11..=18).contains(&n)) {
            return Err(bad("invalid Fax3 initial EOL"));
        }
    }
    let profile = tiff_profile::profile(bytes, request)?;
    let size = u64::from(width) * u64::from(height) * 4;
    if size > raster::MAX_RASTER_BYTES {
        return Err(RasterDecodeError::OutputTooLarge);
    }
    let output_credit = request
        .memory_budget
        .as_ref()
        .map(|b| b.try_reserve(size))
        .transpose()
        .map_err(DecodeError::Memory)?;
    // A row can consume at most this many new input bytes. Every transition
    // requires at least one bit; doubling Vec growth and u16 storage need at
    // most 32 bytes per allowed input byte for live transition storage.
    // Double that quota covers realloc overlap; margin covers cached bits.
    let scratch_bytes = maximum_row_input as u64 * 64
        + 1024
        + if compression == 4 || options & 1 != 0 {
            u64::from(width) * 32 + 1024
        } else {
            0
        };
    let _scratch = request
        .memory_budget
        .as_ref()
        .map(|b| b.try_reserve(scratch_bytes))
        .transpose()
        .map_err(DecodeError::Memory)?;
    let mut output = Vec::new();
    output
        .try_reserve_exact(size as usize)
        .map_err(|_| RasterDecodeError::OutputTooLarge)?;
    output.resize(size as usize, 0);
    let (out_width, out_height) = if orientation >= 5 {
        (height, u32::from(width))
    } else {
        (u32::from(width), height)
    };
    let mut y = 0u32;
    for strip in 0..strips as usize {
        request.check_cancelled()?;
        let source = d.strip(offsets, lengths, strip)?;
        let mut decoder = if matches!(compression, 2 | 32771) {
            mixed::Mixed::new_rle(source, fill, width, request, compression == 32771)
        } else if compression == 4 {
            mixed::Mixed::new_group4(source, fill, width, request)
        } else {
            mixed::Mixed::new(source, fill, width, request, options & 1 != 0)
        };
        let rows = rows_per_strip.min(u64::from(height - y)) as u32;
        for _row in 0..rows {
            request.check_cancelled()?;
            decoder.advance::<CURSOR>()?;
            let transitions = decoder.transitions();
            if transitions.last() != Some(&width) {
                return Err(bad("fax row length mismatch"));
            }
            for (x, color) in fax::decoder::pels(transitions, width).enumerate() {
                if x % 4096 == 0 {
                    request.check_cancelled()?;
                }
                let x = x as u32;
                let w = u32::from(width);
                let (dx, dy) = match orientation {
                    1 => (x, y),
                    2 => (w - 1 - x, y),
                    3 => (w - 1 - x, height - 1 - y),
                    4 => (x, height - 1 - y),
                    5 => (y, x),
                    6 => (height - 1 - y, x),
                    7 => (height - 1 - y, w - 1 - x),
                    8 => (y, w - 1 - x),
                    _ => unreachable!(),
                };
                let value = if (color == fax::Color::White) == (photo == 0) {
                    255
                } else {
                    0
                };
                let at = (dy as usize * out_width as usize + dx as usize) * 4;
                output[at..at + 4].copy_from_slice(&[value, value, value, 255]);
            }
            y += 1;
        }
    }
    request.check_cancelled()?;
    drop(_scratch); // All per-strip decoder vectors have been destroyed.
    let (space, profile_credit) = if let Some(profile) = profile {
        let credit = request
            .memory_budget
            .as_ref()
            .map(|b| b.try_reserve(profile.len() as u64))
            .transpose()
            .map_err(DecodeError::Memory)?;
        let mut owned = Vec::new();
        owned
            .try_reserve_exact(profile.len())
            .map_err(|_| RasterDecodeError::OutputTooLarge)?;
        owned.extend_from_slice(profile);
        (RasterColorSpace::Icc(owned), credit)
    } else {
        (
            if request.assume_untagged_srgb {
                RasterColorSpace::AssumedSrgb
            } else {
                RasterColorSpace::Unspecified
            },
            None,
        )
    };
    let pixels = raster::adopt_raster_output(output, output_credit)?;
    let mut raster = DecodedRaster::new(out_width, out_height, RasterPixels::Rgba8(pixels), space)?;
    if let Some(credit) = profile_credit {
        raster = raster
            .with_color_profile_reservation(credit)
            .map_err(DecodeError::Memory)?;
    }
    Ok(Some(raster))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn original_go_fax3_matches_pillow_with_budget_and_rejection() {
        let root = rrrah_core::MemoryBudget::new(2 * 1024 * 1024);
        let mut request = DecodeRequest::new(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../../tests/fixtures/raster/go-bw-gopher-fax3.tiff"),
        );
        request.memory_budget = Some(root.clone());
        let frame = raster::decode_raster(&request).unwrap();
        assert_eq!((frame.width(), frame.height()), (153, 55));
        let RasterPixels::Rgba8(pixels) = frame.pixels() else {
            panic!()
        };
        assert_eq!(
            &pixels[..],
            include_bytes!("../../../tests/fixtures/raster/go-bw-gopher-fax3.rgba8")
        );
        assert_eq!(root.used(), 153 * 55 * 4);
        drop(frame);
        assert_eq!(root.used(), 0);
    }
}

#[cfg(test)]
mod reference_tests {
    use super::*;
    #[test]
    fn libtiff_fax3_multistrip_fill_endian_and_orientation_match_oracles() {
        for manifest in [
            include_str!("../../../tests/fixtures/raster/fax3-libtiff-reference.json"),
            include_str!("../../../tests/fixtures/raster/fax3-long-libtiff-reference.json"),
            include_str!("../../../tests/fixtures/raster/fax4-libtiff-reference.json"),
            include_str!("../../../tests/fixtures/raster/fax4-long-libtiff-reference.json"),
            include_str!("../../../tests/fixtures/raster/fax4-go-reference.json"),
            include_str!("../../../tests/fixtures/raster/fax4-dense-libtiff-reference.json"),
            include_str!("../../../tests/fixtures/raster/fax2-libtiff-reference.json"),
            include_str!("../../../tests/fixtures/raster/fax2-long-libtiff-reference.json"),
            include_str!("../../../tests/fixtures/raster/fax2w-libtiff-reference.json"),
            include_str!("../../../tests/fixtures/raster/fax2w-long-libtiff-reference.json"),
            include_str!("../../../tests/fixtures/raster/fax2w-single-libtiff-reference.json"),
        ] {
            let reference: serde_json::Value = serde_json::from_str(manifest).unwrap();
            for row in reference["rows"].as_array().unwrap() {
                let name = row["name"].as_str().unwrap();
                let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                    .join("../../tests/fixtures/raster")
                    .join(name);
                let expected = std::fs::read(row["rgba8_file"].as_str().map_or_else(
                    || path.with_extension("rgba8"),
                    |name| path.parent().unwrap().join(name),
                ))
                .unwrap();
                let root = rrrah_core::MemoryBudget::new(2 * 1024 * 1024);
                let mut request = DecodeRequest::new(path);
                request.memory_budget = Some(root.clone());
                let frame = raster::decode_raster(&request).unwrap_or_else(|e| panic!("{name}: {e}"));
                assert_eq!(
                    (frame.width(), frame.height()),
                    (
                        row["width"].as_u64().unwrap() as u32,
                        row["height"].as_u64().unwrap() as u32
                    )
                );
                let RasterPixels::Rgba8(pixels) = frame.pixels() else {
                    panic!()
                };
                assert_eq!(&pixels[..], &expected, "{name}");
                drop(frame);
                assert_eq!(root.used(), 0, "{name}");
            }
        }
    }
}

#[cfg(test)]
mod refusal_tests {
    use super::*;
    fn set(bytes: &mut [u8], tag: u64, value: u64) {
        let d = Directory::new(bytes).unwrap();
        let little = d.little;
        let (at, size, count) = d
            .values(tag, &DecodeRequest::new("control.tiff"))
            .unwrap()
            .unwrap();
        assert_eq!(count, 1);
        for i in 0..size {
            bytes[at + if little { i } else { size - 1 - i }] = (value >> (i * 8)) as u8;
        }
    }
    #[test]
    fn fax3_refusals_preserve_budget_and_no_missing_rows_are_synthesized() {
        let original = include_bytes!("../../../tests/fixtures/raster/go-bw-gopher-fax3.tiff");
        let root = rrrah_core::MemoryBudget::new(2 * 1024 * 1024);
        let mut request = DecodeRequest::new("go.tiff");
        request.memory_budget = Some(root.clone());
        let exact = rrrah_core::MemoryBudget::new(59260);
        request.memory_budget = Some(exact.clone());
        let frame = decode(original, &request).unwrap().unwrap();
        assert_eq!(exact.peak(), 59260);
        assert_eq!(exact.used(), 33660);
        drop(frame);
        assert_eq!(exact.used(), 0);
        let short = rrrah_core::MemoryBudget::new(59259);
        request.memory_budget = Some(short.clone());
        assert!(matches!(
            decode(original, &request),
            Err(RasterDecodeError::Source(DecodeError::Memory(_)))
        ));
        assert_eq!(short.used(), 0);
        request.memory_budget = Some(root.clone());
        let mut inverted = original.to_vec();
        set(&mut inverted, 262, 1);
        let inverse = decode(&inverted, &request).unwrap().unwrap();
        let RasterPixels::Rgba8(values) = inverse.pixels() else {
            panic!()
        };
        let expected = include_bytes!("../../../tests/fixtures/raster/go-bw-gopher-fax3.rgba8");
        for (index, (a, b)) in values.iter().zip(expected).enumerate() {
            assert_eq!(*a, if index % 4 == 3 { 255 } else { 255 - *b });
        }
        drop(inverse);
        assert_eq!(root.used(), 0);
        let mut truncated = original.to_vec();
        set(&mut truncated, 279, 192);
        assert!(decode(&truncated, &request).is_err());
        assert_eq!(root.used(), 0);
        let mut bits = String::from("000000000001");
        bits.push_str(&"001101010000110111".repeat(400));
        bits.push_str("000000000001");
        while bits.len() % 8 != 0 {
            bits.push('0');
        }
        let payload = bits
            .as_bytes()
            .chunks_exact(8)
            .map(|chunk| chunk.iter().fold(0u8, |v, b| (v << 1) | u8::from(*b == b'1')))
            .collect::<Vec<_>>();
        let mut excessive = original.to_vec();
        let offset = excessive.len();
        excessive.extend_from_slice(&payload);
        for (tag, value) in [
            (256, 1),
            (257, 1),
            (278, 1),
            (273, offset as u64),
            (279, payload.len() as u64),
        ] {
            set(&mut excessive, tag, value);
        }
        assert!(decode(&excessive, &request).is_err());
        assert_eq!(root.used(), 0);
        let zero = rrrah_core::MemoryBudget::new(0);
        request.memory_budget = Some(zero.clone());
        let mut outside = original.to_vec();
        set(&mut outside, 273, u32::MAX as u64);
        assert!(matches!(
            decode(&outside, &request),
            Err(RasterDecodeError::InvalidTiffDirectory(_))
        ));
        assert_eq!(zero.peak(), 0);
        let generation = std::sync::Arc::new(std::sync::atomic::AtomicU64::new(1));
        request.cancellation = Some(crate::GenerationToken::new(generation, 0));
        assert!(matches!(
            decode(original, &request),
            Err(RasterDecodeError::Source(DecodeError::Cancelled))
        ));
        assert_eq!(zero.used(), 0);
    }
}

#[cfg(test)]
mod mixed_admission_tests {
    use super::*;
    #[test]
    fn mixed_source_tight_budget_cancel_and_uncompressed_refusal_release_all_credit() {
        let source = include_bytes!("../../../tests/fixtures/raster/fax3-wl-fill1-opt1-ori1.tiff");
        let mut request = DecodeRequest::new("mixed-control.tiff");
        let root = rrrah_core::MemoryBudget::new(1024 * 1024);
        request.memory_budget = Some(root.clone());
        let frame = decode(source, &request).unwrap().unwrap();
        let peak = root.peak();
        assert_eq!(root.used(), 13 * 7 * 4);
        drop(frame);
        assert_eq!(root.used(), 0);
        let exact = rrrah_core::MemoryBudget::new(peak);
        request.memory_budget = Some(exact.clone());
        drop(decode(source, &request).unwrap());
        assert_eq!(exact.used(), 0);
        let short = rrrah_core::MemoryBudget::new(peak - 1);
        request.memory_budget = Some(short.clone());
        assert!(matches!(
            decode(source, &request),
            Err(RasterDecodeError::Source(DecodeError::Memory(_)))
        ));
        assert_eq!(short.used(), 0);
        let mut unsupported = source.to_vec();
        let d = Directory::new(source).unwrap();
        let (at, _, _) = d.values(292, &request).unwrap().unwrap();
        unsupported[at] = 3;
        assert!(matches!(
            decode(&unsupported, &request),
            Err(RasterDecodeError::InvalidTiffDirectory(
                "Fax3 uncompressed mode or unknown options unsupported"
            ))
        ));
        assert_eq!(short.used(), 0);
        let generation = std::sync::Arc::new(std::sync::atomic::AtomicU64::new(1));
        request.cancellation = Some(crate::GenerationToken::new(generation, 0));
        assert!(matches!(
            decode(source, &request),
            Err(RasterDecodeError::Source(DecodeError::Cancelled))
        ));
        assert_eq!(short.used(), 0);
    }
}

#[cfg(test)]
mod group4_refusal_tests {
    use super::*;
    #[test]
    fn group4_premature_end_options_and_tight_admission_do_not_publish_partial_pixels() {
        let source = include_bytes!("../../../tests/fixtures/raster/fax4-wl-fill1-opt0-ori1.tiff");
        let mut request = DecodeRequest::new("group4-control.tiff");
        let root = rrrah_core::MemoryBudget::new(2 * 1024 * 1024);
        request.memory_budget = Some(root.clone());
        let frame = decode(source, &request).unwrap().unwrap();
        let peak = root.peak();
        drop(frame);
        assert_eq!(root.used(), 0);
        let exact = rrrah_core::MemoryBudget::new(peak);
        request.memory_budget = Some(exact.clone());
        drop(decode(source, &request).unwrap());
        assert_eq!(exact.used(), 0);
        let short = rrrah_core::MemoryBudget::new(peak - 1);
        request.memory_budget = Some(short.clone());
        assert!(matches!(
            decode(source, &request),
            Err(RasterDecodeError::Source(DecodeError::Memory(_)))
        ));
        assert_eq!(short.used(), 0);
        let d = Directory::new(source).unwrap();
        let (options, _, _) = d.values(293, &request).unwrap().unwrap();
        let mut unsupported = source.to_vec();
        unsupported[options] = 2;
        let zero = rrrah_core::MemoryBudget::new(0);
        request.memory_budget = Some(zero.clone());
        assert!(matches!(
            decode(&unsupported, &request),
            Err(RasterDecodeError::InvalidTiffDirectory(
                "Fax4 uncompressed mode or unknown options unsupported"
            ))
        ));
        assert_eq!(zero.peak(), 0);
        request.memory_budget = Some(root.clone());
        let (offset, size, _) = d.values(273, &request).unwrap().unwrap();
        let start = d.number(offset, size).unwrap() as usize;
        let mut early_end = source.to_vec();
        early_end[start..start + 3].copy_from_slice(&[0, 0x10, 1]); // T.6 EOFB before the first row.
        assert!(decode(&early_end, &request).is_err());
        assert_eq!(root.used(), 0);
        let generation = std::sync::Arc::new(std::sync::atomic::AtomicU64::new(1));
        request.cancellation = Some(crate::GenerationToken::new(generation, 0));
        assert!(matches!(
            decode(source, &request),
            Err(RasterDecodeError::Source(DecodeError::Cancelled))
        ));
        assert_eq!(root.used(), 0);
    }
}

#[cfg(test)]
mod cursor_decode_benchmark {
    use super::*;
    #[test]
    #[ignore = "paired complete borrowed-source TIFF decode benchmark; excludes file I/O and display"]
    fn native_group4_cursor_alternating_benchmark() {
        use std::{hint::black_box, time::Instant};
        let source = include_bytes!("../../../tests/fixtures/raster/fax4-dense-wl-fill1-opt0-ori1.tiff");
        let root = rrrah_core::MemoryBudget::new(8 * 1024 * 1024);
        let mut request = DecodeRequest::new("dense-native-bench.tiff");
        request.memory_budget = Some(root.clone());
        let mut expected = None;
        for pair in 0..8 {
            for cursor in if pair % 2 == 0 {
                [false, true]
            } else {
                [true, false]
            } {
                let start = Instant::now();
                let mut checksum = 0u64;
                for _ in 0..16 {
                    let raster = if cursor {
                        decode_with_cursor::<true>(black_box(source), &request)
                    } else {
                        decode_with_cursor::<false>(black_box(source), &request)
                    }
                    .unwrap()
                    .unwrap();
                    let RasterPixels::Rgba8(pixels) = raster.pixels() else {
                        panic!()
                    };
                    checksum = checksum.wrapping_add(u64::from(pixels[0]) + pixels.len() as u64);
                    black_box(&raster);
                }
                assert_eq!(*expected.get_or_insert(checksum), checksum);
                assert_eq!(root.used(), 0);
                eprintln!(
                    "FAX_NATIVE pair={pair} cursor={cursor} ns={} decodes=16 source=1025x129 checksum={checksum} final=0",
                    start.elapsed().as_nanos()
                );
            }
        }
    }
}

#[cfg(test)]
mod rle_admission_tests {
    use super::*;
    #[test]
    fn rle_budget_inverse_photometric_and_invalid_strips_preserve_all_credit() {
        for source in [
            include_bytes!("../../../tests/fixtures/raster/fax2-wl-fill1-opt0-ori1.tiff").as_slice(),
            include_bytes!("../../../tests/fixtures/raster/fax2w-wl-fill1-opt0-ori1.tiff").as_slice(),
        ] {
            let mut request = DecodeRequest::new("rle-control.tiff");
            let root = rrrah_core::MemoryBudget::new(2 * 1024 * 1024);
            request.memory_budget = Some(root.clone());
            let original = decode(source, &request).unwrap().unwrap();
            let peak = root.peak();
            drop(original);
            assert_eq!(root.used(), 0);
            let exact = rrrah_core::MemoryBudget::new(peak);
            request.memory_budget = Some(exact.clone());
            drop(decode(source, &request).unwrap());
            assert_eq!(exact.used(), 0);
            let short = rrrah_core::MemoryBudget::new(peak - 1);
            request.memory_budget = Some(short.clone());
            assert!(matches!(
                decode(source, &request),
                Err(RasterDecodeError::Source(DecodeError::Memory(_)))
            ));
            assert_eq!(short.used(), 0);
            request.memory_budget = Some(root.clone());
            let d = Directory::new(source).unwrap();
            let mut inverse = source.to_vec();
            let (at, _, _) = d.values(262, &request).unwrap().unwrap();
            inverse[at] = 1;
            let inverse = decode(&inverse, &request).unwrap().unwrap();
            let RasterPixels::Rgba8(pixels) = inverse.pixels() else {
                panic!()
            };
            for (i, (&value, &expected)) in pixels
                .iter()
                .zip(include_bytes!(
                    "../../../tests/fixtures/raster/fax3-wl-fill1-opt0-ori1.rgba8"
                ))
                .enumerate()
            {
                assert_eq!(value, if i % 4 == 3 { 255 } else { 255 - expected });
            }
            drop(inverse);
            assert_eq!(root.used(), 0);
            let (at, size, _) = d.values(279, &request).unwrap().unwrap();
            let mut truncated = source.to_vec();
            truncated[at..at + size].fill(0);
            truncated[at] = 1;
            assert!(decode(&truncated, &request).is_err());
            assert_eq!(root.used(), 0);
            let mut empty = source.to_vec();
            empty[at..at + size].fill(0);
            let zero = rrrah_core::MemoryBudget::new(0);
            request.memory_budget = Some(zero.clone());
            assert!(matches!(
                decode(&empty, &request),
                Err(RasterDecodeError::InvalidTiffDirectory("empty fax strip"))
            ));
            assert_eq!(zero.peak(), 0);
            let generation = std::sync::Arc::new(std::sync::atomic::AtomicU64::new(1));
            request.cancellation = Some(crate::GenerationToken::new(generation, 0));
            assert!(matches!(
                decode(source, &request),
                Err(RasterDecodeError::Source(DecodeError::Cancelled))
            ));
            assert_eq!(zero.used(), 0);
        }
    }
}
