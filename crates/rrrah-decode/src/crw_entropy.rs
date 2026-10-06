//! Bounded CRW entropy, sensor reconstruction and qualified CIFF adaptation.
#![allow(dead_code)]

#[derive(Debug, PartialEq, Eq)]
pub(crate) enum EntropyError {
    Truncated,
    InvalidStuffing,
    InvalidWidth,
    InvalidTable,
    InvalidCode,
    InvalidRun,
    InvalidSample,
    Cancelled,
}
/// MSB-first stream with mandatory zero stuffing after every FF byte.
/// Borrows compressed data and never reads beyond the supplied sensor span.
pub(crate) struct Bits<'a> {
    source: &'a [u8],
    cursor: usize,
    remaining: u8,
    byte: u8,
}
impl<'a> Bits<'a> {
    pub fn new(source: &'a [u8]) -> Self {
        Self {
            source,
            cursor: 0,
            remaining: 0,
            byte: 0,
        }
    }
    pub fn read(&mut self, width: u8) -> Result<u16, EntropyError> {
        if width > 16 {
            return Err(EntropyError::InvalidWidth);
        }
        let mut value = 0;
        for _ in 0..width {
            if self.remaining == 0 {
                let byte = *self.source.get(self.cursor).ok_or(EntropyError::Truncated)?;
                let consumed = if byte == 255 {
                    match self.source.get(self.cursor + 1) {
                        Some(0) => 2,
                        Some(_) => return Err(EntropyError::InvalidStuffing),
                        None => return Err(EntropyError::Truncated),
                    }
                } else {
                    1
                };
                self.cursor += consumed;
                self.byte = byte;
                self.remaining = 8;
            }
            self.remaining -= 1;
            value = (value << 1) | u16::from((self.byte >> self.remaining) & 1);
        }
        Ok(value)
    }
    /// JPEG-style signed amplitude used by CRW differences. Width zero is zero.
    pub fn difference(&mut self, width: u8) -> Result<i32, EntropyError> {
        let value = i32::from(self.read(width)?);
        Ok(if width > 0 && value < (1 << (width - 1)) {
            value - ((1 << width) - 1)
        } else {
            value
        })
    }
}
/// Canonical prefix code with fixed-size indexing and borrowed symbols.
/// Incomplete trees are valid, but over-subscribed and empty trees are rejected.
pub(crate) struct Huffman<'a> {
    counts: [u8; 16],
    first: [u32; 16],
    offsets: [usize; 16],
    symbols: &'a [u8],
}
impl<'a> Huffman<'a> {
    pub fn new(counts: [u8; 16], symbols: &'a [u8]) -> Result<Self, EntropyError> {
        let mut first = [0; 16];
        let mut offsets = [0; 16];
        let mut code = 0u32;
        let mut offset = 0usize;
        for (index, count) in counts.iter().copied().enumerate() {
            first[index] = code;
            offsets[index] = offset;
            code += u32::from(count);
            if code > 1 << (index + 1) {
                return Err(EntropyError::InvalidTable);
            }
            offset += usize::from(count);
            code <<= 1;
        }
        if offset == 0 || offset != symbols.len() {
            return Err(EntropyError::InvalidTable);
        }
        Ok(Self {
            counts,
            first,
            offsets,
            symbols,
        })
    }
    pub fn decode(&self, bits: &mut Bits<'_>) -> Result<u8, EntropyError> {
        let mut code = 0u32;
        for index in 0..16 {
            code = (code << 1) | u32::from(bits.read(1)?);
            if let Some(relative) = code.checked_sub(self.first[index])
                && relative < u32::from(self.counts[index])
            {
                return Ok(self.symbols[self.offsets[index] + relative as usize]);
            }
        }
        Err(EntropyError::InvalidCode)
    }
}

/// Decode one 64-sample difference block. The first symbol uses a distinct tree;
/// later zero symbols terminate the block, leaving the remaining differences zero.
pub(crate) fn differences(
    bits: &mut Bits<'_>,
    first: &Huffman<'_>,
    rest: &Huffman<'_>,
) -> Result<[i32; 64], EntropyError> {
    let mut output = [0; 64];
    let mut index = 0usize;
    while index < 64 {
        let leaf = if index == 0 { first } else { rest }.decode(bits)?;
        if leaf == 0 && index != 0 {
            break;
        }
        if leaf != 255 {
            index += usize::from(leaf >> 4);
            let width = leaf & 15;
            if index >= 64 {
                return Err(EntropyError::InvalidRun);
            }
            if width != 0 {
                output[index] = bits.difference(width)?;
            }
        }
        index += 1;
    }
    Ok(output)
}
/// Predictor for the high ten bits. State advances only after a complete,
/// valid block, so errors cannot publish a partly reconstructed block.
pub(crate) struct Predictor {
    width: usize,
    position: usize,
    base: [i32; 2],
    carry: i32,
}
impl Predictor {
    pub fn new(width: usize) -> Result<Self, EntropyError> {
        if width == 0 || !width.is_multiple_of(2) {
            return Err(EntropyError::InvalidWidth);
        }
        Ok(Self {
            width,
            position: 0,
            base: [512; 2],
            carry: 0,
        })
    }
    pub fn block(&mut self, mut differences: [i32; 64]) -> Result<[u16; 64], EntropyError> {
        differences[0] = differences[0]
            .checked_add(self.carry)
            .ok_or(EntropyError::InvalidSample)?;
        let carry = differences[0];
        let mut position = self.position;
        let mut base = self.base;
        let mut output = [0; 64];
        for (index, difference) in differences.into_iter().enumerate() {
            if position.is_multiple_of(self.width) {
                base = [512; 2];
            }
            let phase = position % 2;
            base[phase] = base[phase]
                .checked_add(difference)
                .ok_or(EntropyError::InvalidSample)?;
            if !(0..=1023).contains(&base[phase]) {
                return Err(EntropyError::InvalidSample);
            }
            output[index] = u16::try_from(base[phase]).map_err(|_| EntropyError::InvalidSample)?;
            position = position.checked_add(1).ok_or(EntropyError::InvalidSample)?;
        }
        self.position = position;
        self.base = base;
        self.carry = carry;
        Ok(output)
    }
}

/// Reconstruct the high-bit sensor into caller-owned storage. Cancellation is
/// checked at every 64-sample block. The caller must discard partial output on error.
pub(crate) fn reconstruct_high(
    compressed: &[u8],
    output: &mut [u16],
    width: usize,
    first: &Huffman<'_>,
    rest: &Huffman<'_>,
    mut cancelled: impl FnMut() -> bool,
) -> Result<(), EntropyError> {
    if width == 0
        || output.is_empty()
        || !output.len().is_multiple_of(width)
        || !output.len().is_multiple_of(64)
    {
        return Err(EntropyError::InvalidWidth);
    }
    let mut bits = Bits::new(compressed);
    let mut predictor = Predictor::new(width)?;
    for target in output.as_chunks_mut::<64>().0 {
        if cancelled() {
            return Err(EntropyError::Cancelled);
        }
        target.copy_from_slice(&predictor.block(differences(&mut bits, first, rest)?)?);
    }
    Ok(())
}

/// Complete internal qualified 10D decode; format tables remain caller-owned.
pub(crate) fn decode_10d(
    source: &[u8],
    table_index: u32,
    trees: (&Huffman<'_>, &Huffman<'_>),
    budget: &rrrah_core::MemoryBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<rrrah_core::DecodedMosaic, crate::DecodeError> {
    crate::ciff::eos_10d_preflight(source, cancelled)?;
    let pixels = sensor_from_ciff(source, table_index, trees, true, budget, cancelled)?;
    let metadata = crate::ciff::eos_10d_metadata(source, &pixels, cancelled)?;
    rrrah_core::DecodedMosaic::new(metadata, pixels).map_err(|reason| crate::DecodeError::NativeCamera {
        format: "CRW",
        message: reason.to_string(),
    })
}

/// Internal sensor-only path with explicitly supplied format tables and
/// low-plane policy. No metadata color or public viewer admission is implied.
pub(crate) fn sensor_from_ciff(
    source: &[u8],
    table_index: u32,
    trees: (&Huffman<'_>, &Huffman<'_>),
    low_present: bool,
    budget: &rrrah_core::MemoryBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<rrrah_core::PixelBuffer<u16>, crate::DecodeError> {
    use crate::DecodeError;
    let error = |reason: &str| DecodeError::NativeCamera {
        format: "CRW",
        message: reason.into(),
    };
    if cancelled() {
        return Err(DecodeError::Cancelled);
    }
    // The qualified sensor layout has a 26-byte little-endian header.
    if source.get(..6) != Some(&b"II\x1a\0\0\0"[..]) {
        return Err(error("unsupported CIFF sensor header"));
    }
    let directory = crate::ciff::Directory::file(source).map_err(error)?;
    let lookup = |tag| {
        directory.unique(tag, cancelled).map_err(|reason| {
            if cancelled() {
                DecodeError::Cancelled
            } else {
                error(reason)
            }
        })
    };
    let table = lookup(0x1835)?.payload;
    if table.len() != 16 {
        return Err(error("unsupported decoder table record"));
    }
    let declared = u32::from_le_bytes(table[..4].try_into().unwrap());
    if declared > 2 || declared != table_index {
        return Err(error("decoder table index mismatch"));
    }
    let geometry = lookup(0x1031)?.payload;
    if geometry.len() != 34 || geometry[..2] != 34u16.to_le_bytes() {
        return Err(error("unsupported sensor geometry record"));
    }
    let width = usize::from(u16::from_le_bytes(geometry[2..4].try_into().unwrap()));
    let height = usize::from(u16::from_le_bytes(geometry[4..6].try_into().unwrap()));
    let count = width.checked_mul(height).ok_or(DecodeError::DimensionOverflow)?;
    let payload = lookup(0x2005)?.payload;
    let low_len = if low_present { count / 4 } else { 0 };
    let entropy_offset = 514usize
        .checked_add(low_len)
        .ok_or(DecodeError::DimensionOverflow)?;
    let compressed = payload
        .get(entropy_offset..)
        .ok_or_else(|| error("sensor payload too short"))?;
    let low = if low_present {
        Some(
            payload
                .get(..low_len)
                .ok_or_else(|| error("low plane too short"))?,
        )
    } else {
        None
    };
    reconstruct_managed(
        compressed,
        low,
        (width, height),
        trees.0,
        trees.1,
        budget,
        cancelled,
    )
}

/// Managed sensor output; source/table storage accounting belongs to the caller.
/// Only the explicitly supplied compressed span is read. No partial buffer is
/// returned after cancellation, invalid entropy or failed memory admission.
pub(crate) fn reconstruct_managed(
    compressed: &[u8],
    low: Option<&[u8]>,
    dimensions: (usize, usize),
    first: &Huffman<'_>,
    rest: &Huffman<'_>,
    budget: &rrrah_core::MemoryBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<rrrah_core::PixelBuffer<u16>, crate::DecodeError> {
    use crate::DecodeError;
    let error = |reason: &str| DecodeError::NativeCamera {
        format: "CRW",
        message: reason.into(),
    };
    if cancelled() {
        return Err(DecodeError::Cancelled);
    }
    let (width, height) = dimensions;
    let count = width.checked_mul(height).ok_or(DecodeError::DimensionOverflow)?;
    if count == 0 || !width.is_multiple_of(4) || !count.is_multiple_of(64) {
        return Err(error("invalid sensor geometry"));
    }
    if let Some(plane) = low
        && plane.len() != count / 4
    {
        return Err(error("invalid low-plane length"));
    }
    // Even an all-zero block consumes a first symbol plus an EOB symbol,
    // each at least one bit. Stuffing can only increase source length.
    let minimum_bytes = (count / 64).div_ceil(4);
    if compressed.len() < minimum_bytes {
        return Err(error("entropy span too short for declared sensor"));
    }
    let mut output = budget.try_buffer(count, 0u16)?;
    reconstruct_high(compressed, &mut output, width, first, rest, cancelled).map_err(|e| {
        if e == EntropyError::Cancelled {
            DecodeError::Cancelled
        } else {
            error(&format!("invalid entropy: {e:?}"))
        }
    })?;
    if let Some(plane) = low {
        for (row, low_row) in output.chunks_exact_mut(width).zip(plane.chunks_exact(width / 4)) {
            if cancelled() {
                return Err(DecodeError::Cancelled);
            }
            merge_low_bits(row, low_row, width).map_err(|e| error(&format!("invalid low plane: {e:?}")))?;
        }
    }
    if cancelled() {
        return Err(DecodeError::Cancelled);
    }
    Ok(output.freeze().into())
}

/// Merge the separate low-two-bit plane after high-bit entropy decoding.
/// Validation completes before mutation. Width 2672 has Canon's documented
/// legacy correction for combined values below 512.
pub(crate) fn merge_low_bits(high: &mut [u16], low: &[u8], width: usize) -> Result<(), EntropyError> {
    if width == 0
        || !width.is_multiple_of(4)
        || !high.len().is_multiple_of(width)
        || high.len() / 4 != low.len()
    {
        return Err(EntropyError::InvalidWidth);
    }
    if high.iter().any(|value| *value > 1023) {
        return Err(EntropyError::InvalidSample);
    }
    for (index, value) in high.iter_mut().enumerate() {
        let extra = u16::from((low[index / 4] >> (2 * (index % 4))) & 3);
        let combined = (*value << 2) | extra;
        *value = combined + if width == 2672 && combined < 512 { 2 } else { 0 };
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    #[ignore = "requires CRW source, independent sensor and external camera table data"]
    fn real_full_sensor_entropy_matches_independent_oracle() {
        let source = std::fs::read(std::env::var("RRRAH_CRW_SOURCE").unwrap()).unwrap();
        let oracle = std::fs::read(std::env::var("RRRAH_CRW_ORACLE").unwrap()).unwrap();
        let tables = std::fs::read(std::env::var("RRRAH_CRW_TABLES").unwrap()).unwrap();
        assert_eq!(tables.len(), 209);
        let make = |bytes: &[u8]| {
            let counts: [u8; 16] = bytes[..16].try_into().unwrap();
            let length: usize = counts.iter().map(|v| usize::from(*v)).sum();
            (counts, length)
        };
        let (counts, length) = make(&tables[..29]);
        let first = Huffman::new(counts, &tables[16..16 + length]).unwrap();
        let (counts, length) = make(&tables[29..]);
        let rest = Huffman::new(counts, &tables[45..45 + length]).unwrap();
        let directory = crate::ciff::Directory::file(&source).unwrap();
        let geometry = directory.unique(0x1031, &|| false).unwrap().payload;
        let width = usize::from(u16::from_le_bytes(geometry[2..4].try_into().unwrap()));
        let height = usize::from(u16::from_le_bytes(geometry[4..6].try_into().unwrap()));
        let payload = directory.unique(0x2005, &|| false).unwrap().payload;
        let expected: Vec<u16> = oracle
            .chunks_exact(2)
            .map(|p| u16::from_le_bytes([p[0], p[1]]))
            .collect();
        let mut actual = vec![0; expected.len()];
        assert_eq!(expected.len(), width * height);
        let low_len = expected.len() / 4;
        let compressed = payload.get(514 + low_len..).unwrap();
        let low_plane = payload.get(..low_len).unwrap();
        reconstruct_high(compressed, &mut actual, width, &first, &rest, || false).unwrap();
        merge_low_bits(&mut actual, low_plane, width).unwrap();
        assert_eq!(actual, expected);
        let budget = rrrah_core::MemoryBudget::new((expected.len() * 2) as u64);
        let table_index = u32::from_le_bytes(
            directory.unique(0x1835, &|| false).unwrap().payload[..4]
                .try_into()
                .unwrap(),
        );
        let managed =
            sensor_from_ciff(&source, table_index, (&first, &rest), true, &budget, &|| false).unwrap();
        assert_eq!(&*managed, &expected);
        if width == 3152 {
            let metadata = crate::ciff::eos_10d_metadata(&source, &managed, &|| false).unwrap();
            assert_eq!(
                metadata.effective_crop(),
                rrrah_core::Rect::new(64, 12, 3088, 2056)
            );
            assert_eq!(metadata.white_balance, [1742.0 / 832.0, 1.0, 1241.0 / 832.0, 1.0]);
            let mosaic = rrrah_core::DecodedMosaic::new(metadata, managed.clone()).unwrap();
            assert_eq!(&*mosaic.pixels, &expected);
            let dimensions = mosaic.metadata.thumbnail_dimensions(128);
            let preview_bytes = u64::from(dimensions.0) * u64::from(dimensions.1) * 4;
            let previews = rrrah_core::MemoryBudget::new(preview_bytes);
            let preview = mosaic.thumbnail_rgba8_managed(128, &previews).unwrap();
            assert_eq!(preview.len() as u64, preview_bytes);
            assert!(preview.chunks_exact(4).all(|pixel| pixel[3] == 255));
            assert!(
                preview
                    .chunks_exact(4)
                    .any(|pixel| pixel[0] != pixel[1] || pixel[1] != pixel[2])
            );
            assert_eq!(previews.used(), preview_bytes);
            assert!(mosaic.thumbnail_rgba8_managed(128, &previews).is_err());
            drop(preview);
            assert_eq!(previews.used(), 0);
            assert!(matches!(
                mosaic.thumbnail_rgba8_managed_with_cancel(128, &previews, &|| true),
                Err(rrrah_core::ThumbnailError::Cancelled)
            ));
            assert_eq!(previews.used(), 0);
            drop(mosaic);
            assert_eq!(
                crate::ciff::eos_10d_black(&managed, &|| false).unwrap(),
                [127, 126, 127, 126]
            );
            assert!(matches!(
                crate::ciff::eos_10d_black(&managed, &|| true),
                Err(crate::DecodeError::Cancelled)
            ));
        }
        assert_eq!(budget.used(), (expected.len() * 2) as u64);
        let empty = rrrah_core::MemoryBudget::new(0);
        assert!(
            sensor_from_ciff(
                &source,
                (table_index + 1) % 3,
                (&first, &rest),
                true,
                &empty,
                &|| false
            )
            .is_err()
        );
        assert_eq!(empty.peak(), 0);
        let clone = managed.clone();
        drop(managed);
        assert_eq!(budget.used(), (expected.len() * 2) as u64);
        drop(clone);
        assert_eq!(budget.used(), 0);
        if width == 3152 {
            let mut request = crate::DecodeRequest::new(std::env::var("RRRAH_CRW_SOURCE").unwrap());
            let combined = rrrah_core::MemoryBudget::new((source.len() + expected.len() * 2) as u64);
            request.memory_budget = Some(combined.clone());
            let imported = crate::decode_crw_10d(&request).unwrap();
            assert_eq!(&*imported.pixels, &expected);
            assert_eq!(combined.used(), (expected.len() * 2) as u64);
            assert_eq!(combined.peak(), (source.len() + expected.len() * 2) as u64);
            drop(imported);
            assert_eq!(combined.used(), 0);
            let decoded = decode_10d(&source, table_index, (&first, &rest), &budget, &|| false).unwrap();
            assert_eq!(&*decoded.pixels, &expected);
            drop(decoded);
            assert_eq!(budget.used(), 0);
            let info = directory.unique(0x1810, &|| false).unwrap().payload;
            let angle_offset = info.as_ptr() as usize - source.as_ptr() as usize + 12;
            for (angle, orientation) in [
                (0u32, rrrah_core::Orientation::Normal),
                (90, rrrah_core::Orientation::Rotate90),
                (180, rrrah_core::Orientation::Rotate180),
                (270, rrrah_core::Orientation::Rotate270),
            ] {
                let mut rotated = source.clone();
                rotated[angle_offset..angle_offset + 4].copy_from_slice(&angle.to_le_bytes());
                let metadata = crate::ciff::eos_10d_metadata(&rotated, &expected, &|| false).unwrap();
                assert_eq!(metadata.orientation, orientation);
                assert_eq!(
                    metadata.display_dimensions(),
                    if orientation.swaps_dimensions() {
                        (2056, 3088)
                    } else {
                        (3088, 2056)
                    }
                );
            }
            let color = directory.unique(0x10a9, &|| false).unwrap().payload;
            let offset = color.as_ptr() as usize - source.as_ptr() as usize;
            let mut bad = source.clone();
            bad[offset + 2..offset + 4].fill(0);
            let untouched = rrrah_core::MemoryBudget::new(expected.len() as u64 * 2);
            assert!(decode_10d(&bad, table_index, (&first, &rest), &untouched, &|| false).is_err());
            assert_eq!(untouched.peak(), 0);
        }
    }
    #[test]
    fn impossible_entropy_geometry_refuses_before_reserving_output() {
        let mut counts = [0; 16];
        counts[0] = 1;
        let tree = Huffman::new(counts, &[0]).unwrap();
        let budget = rrrah_core::MemoryBudget::new(u64::MAX);
        for (width, height, source) in [(64, 1, &[][..]), (64, 5, &[0][..]), (65532, 65536, &[0][..])] {
            assert!(
                reconstruct_managed(source, None, (width, height), &tree, &tree, &budget, &|| false).is_err()
            );
            assert_eq!(budget.used(), 0);
            assert_eq!(budget.peak(), 0);
        }
        let output = reconstruct_managed(&[0], None, (64, 4), &tree, &tree, &budget, &|| false).unwrap();
        assert_eq!(&*output, &[512; 256]);
        drop(output);
        assert_eq!(budget.used(), 0);
    }

    #[test]
    fn managed_admission_and_cancel_release_partial_output() {
        let mut counts = [0; 16];
        counts[0] = 1;
        let tree = Huffman::new(counts, &[0]).unwrap();
        let budget = rrrah_core::MemoryBudget::new(256);
        assert!(
            reconstruct_managed(
                &[0],
                None,
                (64, 2),
                &tree,
                &tree,
                &rrrah_core::MemoryBudget::new(255),
                &|| false
            )
            .is_err()
        );
        let checkpoints = std::cell::Cell::new(0);
        let result = reconstruct_managed(&[0], None, (64, 2), &tree, &tree, &budget, &|| {
            checkpoints.set(checkpoints.get() + 1);
            checkpoints.get() == 3
        });
        assert!(matches!(result, Err(crate::DecodeError::Cancelled)));
        assert_eq!(budget.used(), 0);
        assert!(reconstruct_managed(&[], None, (64, 2), &tree, &tree, &budget, &|| false).is_err());
        assert_eq!(budget.used(), 0);
        drop(reconstruct_managed(&[0], None, (64, 2), &tree, &tree, &budget, &|| false).unwrap());
        assert_eq!(budget.used(), 0);
    }
    #[test]
    fn full_high_plane_cancellation_and_truncation() {
        let mut counts = [0; 16];
        counts[0] = 1;
        let tree = Huffman::new(counts, &[0]).unwrap();
        let mut output = [0; 128];
        reconstruct_high(&[0], &mut output, 64, &tree, &tree, || false).unwrap();
        assert_eq!(output, [512; 128]);
        let mut calls = 0;
        assert_eq!(
            reconstruct_high(&[0], &mut output, 64, &tree, &tree, || {
                calls += 1;
                calls == 2
            }),
            Err(EntropyError::Cancelled)
        );
        assert_eq!(
            reconstruct_high(&[], &mut output, 64, &tree, &tree, || false),
            Err(EntropyError::Truncated)
        );
    }

    #[test]
    fn all_twelve_bit_values_and_legacy_low_value_boundary() {
        for high in 0..=1023 {
            let mut output = [high; 4];
            merge_low_bits(&mut output, &[0b11100100], 4).unwrap();
            assert_eq!(output, [high * 4, high * 4 + 1, high * 4 + 2, high * 4 + 3]);
        }
        let mut output = vec![128; 2672];
        output[..4].copy_from_slice(&[0, 127, 128, 1023]);
        let mut low = vec![0; 668];
        low[0] = 0b11100100;
        merge_low_bits(&mut output, &low, 2672).unwrap();
        assert_eq!(&output[..4], &[2, 511, 514, 4095]);
        for (mut output, low, width) in [
            (vec![0; 4], vec![0], 0),
            (vec![0; 4], vec![0], 3),
            (vec![0; 4], vec![], 4),
            (vec![0; 4], vec![0, 0], 4),
            (vec![0, 0, 0, 1024], vec![0], 4),
        ] {
            let before = output.clone();
            assert!(merge_low_bits(&mut output, &low, width).is_err());
            assert_eq!(output, before);
        }
    }
    #[test]
    #[ignore = "requires pinned EOS 10D CRW and independent full sensor oracle"]
    fn real_10d_low_plane_matches_every_independent_sensor_sample() {
        let source = std::fs::read(std::env::var("RRRAH_CRW_SOURCE").unwrap()).unwrap();
        let oracle = std::fs::read(std::env::var("RRRAH_CRW_ORACLE").unwrap()).unwrap();
        assert_eq!(oracle.len(), 3152 * 2068 * 2);
        let expected: Vec<u16> = oracle
            .chunks_exact(2)
            .map(|p| u16::from_le_bytes([p[0], p[1]]))
            .collect();
        let mut high: Vec<u16> = expected.iter().map(|value| value >> 2).collect();
        merge_low_bits(&mut high, &source[26..26 + expected.len() / 4], 3152).unwrap();
        assert_eq!(high, expected);
        // This verifies the independent low-plane layout, not high-bit entropy.
    }

    #[test]
    fn authored_difference_block_and_row_reset_with_carried_dc() {
        let mut counts = [0; 16];
        counts[0] = 2;
        let first = Huffman::new(counts, &[0, 1]).unwrap();
        let rest = Huffman::new(counts, &[0, 0x11]).unwrap();
        // First +1, skip one then -1, end: 1 1 | 1 0 | 0.
        let block = differences(&mut Bits::new(&[0b11100000]), &first, &rest).unwrap();
        assert_eq!(block[0], 1);
        assert_eq!(block[2], -1);
        assert_eq!(block.iter().sum::<i32>(), 0);
        let mut predictor = Predictor::new(8).unwrap();
        let output = predictor.block(block).unwrap();
        assert_eq!(&output[..8], &[513, 512, 512, 512, 512, 512, 512, 512]);
        assert!(output[8..].iter().all(|v| *v == 512));
        let next = predictor.block([0; 64]).unwrap();
        assert_eq!(&next[..8], &[513, 512, 513, 512, 513, 512, 513, 512]);
        assert!(next[8..].iter().all(|v| *v == 512));
    }
    #[test]
    fn predictor_refuses_overflow_without_advancing_state() {
        assert!(matches!(Predictor::new(0), Err(EntropyError::InvalidWidth)));
        assert!(matches!(Predictor::new(3), Err(EntropyError::InvalidWidth)));
        let mut predictor = Predictor::new(64).unwrap();
        for value in [512, -513, i32::MAX, i32::MIN] {
            let mut bad = [0; 64];
            bad[63] = value;
            assert_eq!(predictor.block(bad), Err(EntropyError::InvalidSample));
            assert_eq!(predictor.block([0; 64]), Ok([512; 64]));
        }
        let mut counts = [0; 16];
        counts[0] = 1;
        let tree = Huffman::new(counts, &[0xf1]).unwrap();
        let first = Huffman::new(counts, &[0]).unwrap();
        assert!(matches!(
            differences(&mut Bits::new(&[0; 32]), &first, &tree),
            Err(EntropyError::InvalidRun)
        ));
    }

    #[test]
    fn canonical_prefixes_and_deep_code_match_authored_bit_patterns() {
        // Authored mapping: 0 -> 10, 10 -> 20, 110 -> 30, 111 -> 40.
        let mut counts = [0; 16];
        counts[..3].copy_from_slice(&[1, 1, 2]);
        let tree = Huffman::new(counts, &[10, 20, 30, 40]).unwrap();
        let mut bits = Bits::new(&[0b01011011, 0b10000000]);
        for expected in [10, 20, 30, 40] {
            assert_eq!(tree.decode(&mut bits), Ok(expected));
        }
        let mut counts = [0; 16];
        counts[15] = 1;
        let tree = Huffman::new(counts, &[99]).unwrap();
        assert_eq!(tree.decode(&mut Bits::new(&[0, 0])), Ok(99));
        assert_eq!(tree.decode(&mut Bits::new(&[0])), Err(EntropyError::Truncated));
        assert_eq!(
            tree.decode(&mut Bits::new(&[0, 1])),
            Err(EntropyError::InvalidCode)
        );
    }
    #[test]
    fn impossible_trees_and_symbol_count_mismatches_refuse() {
        assert!(matches!(
            Huffman::new([0; 16], &[]),
            Err(EntropyError::InvalidTable)
        ));
        for depth in 0..16 {
            let mut counts = [0; 16];
            counts[depth] = 1;
            assert!(matches!(
                Huffman::new(counts, &[]),
                Err(EntropyError::InvalidTable)
            ));
            assert!(matches!(
                Huffman::new(counts, &[1, 2]),
                Err(EntropyError::InvalidTable)
            ));
        }
        let mut counts = [0; 16];
        counts[0] = 3;
        assert!(matches!(
            Huffman::new(counts, &[1, 2, 3]),
            Err(EntropyError::InvalidTable)
        ));
        counts[0] = 2;
        counts[15] = 1;
        assert!(matches!(
            Huffman::new(counts, &[1, 2, 3]),
            Err(EntropyError::InvalidTable)
        ));
    }

    #[test]
    fn stuffed_bytes_cross_byte_reads_and_signed_amplitudes() {
        let mut bits = Bits::new(&[0b10101010, 255, 0, 0b01010101]);
        assert_eq!(bits.read(3), Ok(5));
        assert_eq!(bits.read(10), Ok(351));
        assert_eq!(bits.read(11), Ok(1877));
        assert_eq!(bits.read(1), Err(EntropyError::Truncated));
        for width in 0..=16 {
            for value in 0..(1u32 << width) {
                let encoded = (value << (16 - width)).to_be_bytes();
                let mut source = Vec::new();
                for byte in &encoded[2..] {
                    source.push(*byte);
                    if *byte == 255 {
                        source.push(0);
                    }
                }
                let expected = if width > 0 && value < 1 << (width - 1) {
                    value as i32 - ((1 << width) - 1)
                } else {
                    value as i32
                };
                assert_eq!(Bits::new(&source).difference(width), Ok(expected));
            }
        }
    }
    #[test]
    fn empty_truncated_and_unstuffed_streams_refuse() {
        assert_eq!(Bits::new(&[]).read(0), Ok(0));
        assert_eq!(Bits::new(&[]).read(17), Err(EntropyError::InvalidWidth));
        assert_eq!(Bits::new(&[255]).read(1), Err(EntropyError::Truncated));
        for marker in 1..=255 {
            assert_eq!(
                Bits::new(&[255, marker]).read(1),
                Err(EntropyError::InvalidStuffing)
            );
        }
        let source = [0x80, 255, 0, 0x01];
        for end in 0..source.len() {
            let mut bits = Bits::new(&source[..end]);
            assert!(bits.read(16).and_then(|_| bits.read(8)).is_err());
        }
    }
}
