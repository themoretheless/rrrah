//! Bounded Kodak 65000 single-plane block decoder (identity response curve).
use super::camera_error;
use crate::DecodeError;
fn error(message: &str) -> DecodeError {
    camera_error("DCR", message)
}
struct Reader<'a> {
    data: &'a [u8],
    position: usize,
}
impl Reader<'_> {
    fn take(&mut self, n: usize) -> Result<&[u8], DecodeError> {
        let end = self
            .position
            .checked_add(n)
            .ok_or(DecodeError::DimensionOverflow)?;
        let out = self
            .data
            .get(self.position..end)
            .ok_or_else(|| error("truncated Kodak block"))?;
        self.position = end;
        Ok(out)
    }
    fn block(&mut self, count: usize, output: &mut Vec<u16>) -> Result<(), DecodeError> {
        let rounded = (count + 3) & !3;
        let saved = self.position;
        let header = self.take(rounded / 2)?;
        let mut lengths = [0u8; 256];
        for (i, byte) in header.iter().enumerate() {
            lengths[2 * i] = byte & 15;
            lengths[2 * i + 1] = byte >> 4;
        }
        if lengths[..rounded].iter().any(|v| *v > 12) {
            self.position = saved;
            let mut values = [0u16; 256];
            for group in (0..rounded).step_by(8) {
                let b = self.take(12)?;
                let mut words = [0u16; 6];
                for (w, pair) in words.iter_mut().zip(b.chunks_exact(2)) {
                    *w = u16::from_le_bytes([pair[0], pair[1]]);
                }
                values[group] = (words[0] >> 12) << 8 | (words[2] >> 12) << 4 | words[4] >> 12;
                values[group + 1] = (words[1] >> 12) << 8 | (words[3] >> 12) << 4 | words[5] >> 12;
                for (j, w) in words.into_iter().enumerate() {
                    if group + 2 + j < rounded {
                        values[group + 2 + j] = w & 4095;
                    }
                }
            }
            output.extend_from_slice(&values[..count]);
            return Ok(());
        }
        let mut buffer = 0u64;
        let mut available = 0u32;
        if rounded & 7 == 4 {
            let b = self.take(2)?;
            buffer = u64::from(u16::from_be_bytes([b[0], b[1]]));
            available = 16;
        }
        let mut predictors = [0i32; 2];
        for (index, length) in lengths[..rounded].iter().copied().enumerate() {
            let length = u32::from(length);
            if available < length {
                let b = self.take(4)?;
                let word = u32::from_le_bytes([b[1], b[0], b[3], b[2]]);
                buffer |= u64::from(word) << available;
                available += 32;
            }
            let amplitude = (buffer & ((1u64 << length) - 1)) as i32;
            buffer >>= length;
            available -= length;
            let diff = if length > 0 && amplitude & (1 << (length - 1)) == 0 {
                amplitude - ((1 << length) - 1)
            } else {
                amplitude
            };
            if index < count {
                predictors[index & 1] += diff;
                let value = predictors[index & 1];
                if !(0..=4095).contains(&value) {
                    return Err(error("Kodak sample outside 12-bit range"));
                }
                output.push(value as u16);
            }
        }
        Ok(())
    }
}
// Output admission belongs to the native camera backend before allocation.
pub(crate) fn decode(
    data: &[u8],
    width: usize,
    height: usize,
    cancelled: &(dyn Fn() -> bool + Sync),
) -> Result<Vec<u16>, DecodeError> {
    let samples = width.checked_mul(height).ok_or(DecodeError::DimensionOverflow)?;
    if width == 0 || height == 0 || samples > 200_000_000 {
        return Err(error("invalid sensor dimensions"));
    }
    if cancelled() {
        return Err(DecodeError::Cancelled);
    }
    let mut out = Vec::new();
    out.try_reserve_exact(samples)
        .map_err(|_| error("sensor allocation failed"))?;
    let mut reader = Reader { data, position: 0 };
    for _ in 0..height {
        if cancelled() {
            return Err(DecodeError::Cancelled);
        }
        for col in (0..width).step_by(256) {
            reader.block((width - col).min(256), &mut out)?;
        }
    }
    Ok(out)
}
#[cfg(test)]
mod tests {
    #[test]
    fn synthetic_prediction_and_partial_block_are_exact() {
        // Four 1-bit +1 residuals, with two independent parity predictors.
        assert_eq!(
            super::decode(&[0x11, 0x11, 0xff, 0xff], 4, 1, &|| false).unwrap(),
            [1, 1, 2, 2]
        );
        // A one-sample tail still carries the rounded four-sample block header.
        assert_eq!(super::decode(&[0, 0, 0, 0], 1, 1, &|| false).unwrap(), [0]);
        // Predictor resets at each row/block.
        assert_eq!(
            super::decode(&[0x11, 0x11, 0xff, 0xff, 0x11, 0x11, 0xff, 0xff], 4, 2, &|| false).unwrap(),
            [1, 1, 2, 2, 1, 1, 2, 2]
        );
    }
    #[test]
    fn synthetic_packed_fallback_retains_all_twelve_bits() {
        let words = [0x1fffu16, 0x4009, 0x200a, 0x500b, 0x300c, 0x600d];
        let bytes: Vec<_> = words.into_iter().flat_map(u16::to_le_bytes).collect();
        assert_eq!(
            super::decode(&bytes, 8, 1, &|| false).unwrap(),
            [0x123, 0x456, 0xfff, 9, 10, 11, 12, 13]
        );
        for n in 0..bytes.len() {
            assert!(super::decode(&bytes[..n], 8, 1, &|| false).is_err(), "prefix {n}");
        }
    }
    #[test]
    fn synthetic_invalid_prediction_dimensions_and_cancel_reject() {
        assert!(super::decode(&[0x11, 0x11, 0, 0], 4, 1, &|| false).is_err());
        assert!(super::decode(&[], 0, 1, &|| false).is_err());
        assert!(super::decode(&[], usize::MAX, 2, &|| false).is_err());
        assert!(matches!(
            super::decode(&[], 4, 1, &|| true),
            Err(crate::DecodeError::Cancelled)
        ));
    }
    #[test]
    #[ignore = "requires pinned Kodak DCS760C DCR object 1347 and LibRaw sensor dump"]
    fn real_full_sensor_matches_libraw() {
        let source = std::fs::read(std::env::var("RRRAH_KODAK_DCR_SOURCE").unwrap()).unwrap();
        let data = &source[804352..804352 + 5703212];
        let reference = std::fs::read(std::env::var("RRRAH_KODAK_DCR_REFERENCE").unwrap()).unwrap();
        let out = super::decode(data, 3040, 2016, &|| false).unwrap();
        assert_eq!(out.len() * 2, reference.len());
        assert!(
            out.iter()
                .zip(reference.chunks_exact(2))
                .all(|(a, b)| *a == u16::from_le_bytes([b[0], b[1]]))
        );
        assert!(super::decode(&data[..data.len() / 2], 3040, 2016, &|| false).is_err());
    }
}
