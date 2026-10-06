//! Explicit-layout Casio BAY sensor unpacking, without camera/color inference.
//! Layout reference: https://www.inweb.ch/foto/rawformat.html

/// Full sample storage in headerless files; establish camera independently.
/// QV-5700 includes nine columns beyond its 2576-column active area.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BaySensorLayout {
    Qv2000Ux,
    Qv3000Ex,
    Qv5700,
}
impl BaySensorLayout {
    #[must_use]
    pub fn dimensions(self) -> (u32, u32) {
        match self {
            Self::Qv2000Ux => (1632, 1211),
            Self::Qv3000Ex => (2080, 1547),
            Self::Qv5700 => (2585, 1924),
        }
    }
    fn row_bytes(self) -> usize {
        match self {
            Self::Qv5700 => 3232,
            _ => self.dimensions().0 as usize,
        }
    }
}
#[derive(Debug, thiserror::Error)]
pub enum BaySensorError {
    #[error("BAY sensor byte length differs from the explicit camera layout")]
    Length,
    #[error("BAY sensor unpack cancelled")]
    Cancelled,
    #[error(transparent)]
    Memory(#[from] rrrah_core::BufferError),
}
/// Sensor values only: no CFA, black/white levels, WB, crop or display color.
/// QV-5700 consumes 2585 ten-bit samples per 3232-byte row; six padding bits
/// remain after the final sample. There is no auto-detect or implicit crop.
pub fn unpack_bay_sensor(
    bytes: &[u8],
    layout: BaySensorLayout,
    budget: &rrrah_core::MemoryBudget,
    mut cancelled: impl FnMut() -> bool,
) -> Result<rrrah_core::PixelBuffer<u16>, BaySensorError> {
    if cancelled() {
        return Err(BaySensorError::Cancelled);
    }
    let (width, height) = layout.dimensions();
    let width = width as usize;
    let height = height as usize;
    if bytes.len() != layout.row_bytes() * height {
        return Err(BaySensorError::Length);
    }
    let mut output = budget.try_buffer(width * height, 0u16)?;
    for (row, pixels) in bytes
        .chunks_exact(layout.row_bytes())
        .zip(output.chunks_exact_mut(width))
    {
        if cancelled() {
            return Err(BaySensorError::Cancelled);
        }
        if layout == BaySensorLayout::Qv5700 {
            for (group, samples) in row[..width * 10 / 8]
                .chunks_exact(5)
                .zip(pixels.chunks_exact_mut(4))
            {
                let packed = u64::from_be_bytes([0, 0, 0, group[0], group[1], group[2], group[3], group[4]]);
                for (index, sample) in samples.iter_mut().enumerate() {
                    *sample = ((packed >> (30 - index * 10)) & 1023) as u16;
                }
            }
            // 2584 samples occupy 3230 bytes; the last sample is the high ten
            // bits of the remaining two bytes, independent of six padding bits.
            pixels[width - 1] = u16::from_be_bytes([row[3230], row[3231]]) >> 6;
        } else {
            for (sample, &value) in pixels.iter_mut().zip(row) {
                *sample = u16::from(value);
            }
        }
    }
    if cancelled() {
        return Err(BaySensorError::Cancelled);
    }
    Ok(output.freeze().into())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn all_ten_bit_values_storage_columns_padding_and_managed_ownership() {
        let layout = BaySensorLayout::Qv5700;
        let (width, height) = layout.dimensions();
        let mut source = vec![0xa5; layout.row_bytes() * height as usize];
        // Independent MSB-first fixture bit writer; all 1024 values and row
        // boundaries and all nine extra storage columns are exercised.
        for row in 0..height as usize {
            source[row * 3232..(row + 1) * 3232].fill(0);
            source[(row + 1) * 3232 - 1] = 0x3f;
            for column in 0..width as usize {
                let value = (row + column) % 1024;
                for bit in 0..10 {
                    if value & (1 << (9 - bit)) != 0 {
                        let position = column * 10 + bit;
                        source[row * 3232 + position / 8] |= 1 << (7 - position % 8);
                    }
                }
            }
        }
        let budget = rrrah_core::MemoryBudget::new(u64::from(width) * u64::from(height) * 2);
        let pixels = unpack_bay_sensor(&source, layout, &budget, || false).unwrap();
        for (index, &value) in pixels.iter().enumerate() {
            assert_eq!(
                usize::from(value),
                (index / width as usize + index % width as usize) % 1024
            );
        }
        assert_eq!(budget.used(), u64::from(width) * u64::from(height) * 2);
        let shared = pixels.clone();
        drop(pixels);
        assert!(budget.used() > 0);
        drop(shared);
        assert_eq!(budget.used(), 0);
        for size in [source.len() - 1, source.len() + 1] {
            let mut invalid = source.clone();
            invalid.resize(size, 0);
            assert!(matches!(
                unpack_bay_sensor(&invalid, layout, &budget, || false),
                Err(BaySensorError::Length)
            ));
            assert_eq!(budget.used(), 0);
        }
        let tiny = rrrah_core::MemoryBudget::new(1);
        assert!(matches!(
            unpack_bay_sensor(&source, layout, &tiny, || false),
            Err(BaySensorError::Memory(_))
        ));
        assert_eq!(tiny.peak(), 0);
        let mut calls = 0;
        assert!(matches!(
            unpack_bay_sensor(&source, layout, &budget, || {
                calls += 1;
                calls == 3
            }),
            Err(BaySensorError::Cancelled)
        ));
        assert_eq!(budget.used(), 0);
    }
    #[test]
    fn eight_bit_layouts_preserve_all_values_and_exact_storage_dimensions() {
        for layout in [BaySensorLayout::Qv2000Ux, BaySensorLayout::Qv3000Ex] {
            let (width, height) = layout.dimensions();
            let source: Vec<_> = (0..width as usize * height as usize)
                .map(|index| (index % 256) as u8)
                .collect();
            let budget = rrrah_core::MemoryBudget::new(source.len() as u64 * 2);
            let samples = unpack_bay_sensor(&source, layout, &budget, || false).unwrap();
            assert_eq!(samples.len(), source.len());
            for (sample, value) in samples.iter().zip(&source) {
                assert_eq!(*sample, u16::from(*value));
            }
            drop(samples);
            assert_eq!(budget.used(), 0);
            assert!(matches!(
                unpack_bay_sensor(&source, layout, &budget, || true),
                Err(BaySensorError::Cancelled)
            ));
            assert_eq!(budget.used(), 0);
        }
    }
}
