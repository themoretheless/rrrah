//! Bounded Phase One format-3 row entropy. Container/color integration is separate.
use super::camera_error;
use crate::DecodeError;
fn error(message: &str) -> DecodeError {
    camera_error("IIQ", message)
}
struct Bits<'a> {
    bytes: &'a [u8],
    position: usize,
    buffer: u64,
    available: u32,
}
impl Bits<'_> {
    fn read(&mut self, n: u32) -> Result<u32, DecodeError> {
        if n == 0 || n > 16 {
            return Err(error("invalid entropy length"));
        }
        if self.available < n {
            let end = self
                .position
                .checked_add(4)
                .ok_or(DecodeError::DimensionOverflow)?;
            let word = self
                .bytes
                .get(self.position..end)
                .ok_or_else(|| error("truncated entropy word"))?;
            self.buffer = (self.buffer << 32) | u64::from(u32::from_le_bytes(word.try_into().unwrap()));
            self.position = end;
            self.available += 32;
        }
        self.available -= n;
        let value = ((self.buffer >> self.available) & ((1u64 << n) - 1)) as u32;
        self.buffer &= (1u64 << self.available) - 1;
        Ok(value)
    }
}
pub(super) fn decode(
    bytes: &[u8],
    offsets: &[u32],
    width: usize,
    height: usize,
    cancelled: &dyn Fn() -> bool,
) -> Result<Vec<u16>, DecodeError> {
    let count = width.checked_mul(height).ok_or(DecodeError::DimensionOverflow)?;
    if width == 0 || height == 0 || count > 128 * 1024 * 1024 || offsets.len() != height {
        return Err(error("invalid row geometry"));
    }
    if offsets.first() != Some(&0) || offsets.windows(2).any(|pair| pair[0] >= pair[1]) {
        return Err(error("invalid row offsets"));
    }
    if cancelled() {
        return Err(DecodeError::Cancelled);
    }
    let mut output = Vec::new();
    output
        .try_reserve_exact(count)
        .map_err(|_| error("sensor allocation failed"))?;
    for row in 0..height {
        if cancelled() {
            return Err(DecodeError::Cancelled);
        }
        let start = offsets[row] as usize;
        let end = offsets.get(row + 1).map_or(bytes.len(), |v| *v as usize);
        let data = bytes
            .get(start..end)
            .ok_or_else(|| error("row escapes sensor extent"))?;
        let mut bits = Bits {
            bytes: data,
            position: 0,
            buffer: 0,
            available: 0,
        };
        let mut lengths = [0u32; 2];
        let mut predictors = [0i32; 2];
        for column in 0..width {
            if column >= width & !7 {
                lengths = [14, 14];
            } else if column % 8 == 0 {
                for length in &mut lengths {
                    let mut zeros = 0usize;
                    while zeros < 5 && bits.read(1)? == 0 {
                        zeros += 1;
                    }
                    if zeros != 0 {
                        let selector = bits.read(1)? as usize;
                        *length = [8, 7, 6, 9, 11, 10, 5, 12, 14, 13][(zeros - 1) * 2 + selector];
                    }
                    if *length == 0 {
                        return Err(error("predictor length reused before initialization"));
                    }
                }
            }
            let lane = column & 1;
            let length = lengths[lane];
            let value = if length == 14 {
                bits.read(16)? as i32
            } else {
                predictors[lane]
                    .checked_add(bits.read(length)? as i32 + 1 - (1i32 << (length - 1)))
                    .ok_or(DecodeError::DimensionOverflow)?
            };
            if !(0..=65535).contains(&value) {
                return Err(error("sensor predictor out of range"));
            }
            predictors[lane] = value;
            output.push((value as u16).wrapping_shl(2));
        }
    }
    Ok(output)
}
#[cfg(test)]
mod tests {
    use super::*;
    fn encode_bits(bits: &[u8]) -> Vec<u8> {
        bits.chunks(32)
            .flat_map(|chunk| {
                let word = chunk.iter().fold(0u32, |v, bit| (v << 1) | u32::from(*bit));
                (word << (32 - chunk.len())).to_le_bytes()
            })
            .collect()
    }
    fn append_value(bits: &mut Vec<u8>, value: u16, count: usize) {
        bits.extend((0..count).rev().map(|i| ((value >> i) & 1) as u8));
    }
    #[test]
    fn predictor_reuse_signed_steps_and_row_reset() {
        let mut bits = vec![0, 1, 0, 0, 1, 0]; // Two 8-bit predictor lengths.
        for value in [137, 147, 128, 128, 128, 128, 128, 128] {
            append_value(&mut bits, value, 8);
        }
        bits.extend([1, 1]); // Retain both lengths for next block.
        for _ in 0..8 {
            append_value(&mut bits, 126, 8);
        } // -1 on each parity chain.
        let row = encode_bits(&bits);
        let mut data = row.clone();
        data.extend(&row);
        let output = decode(&data, &[0, row.len() as u32], 16, 2, &|| false).unwrap();
        let expected = [10, 20, 11, 21, 12, 22, 13, 23, 12, 22, 11, 21, 10, 20, 9, 19].map(|v| v * 4);
        assert_eq!(&output[..16], &expected);
        assert_eq!(&output[16..], &expected);
        for end in 0..row.len() {
            assert!(decode(&row[..end], &[0], 16, 1, &|| false).is_err());
        }
    }
    #[test]
    fn absolute_tail_and_invalid_geometry() {
        let mut bits = Vec::new();
        for value in [1, 16383, 65535] {
            append_value(&mut bits, value, 16);
        }
        let data = encode_bits(&bits);
        assert_eq!(decode(&data, &[0], 3, 1, &|| false).unwrap(), [4, 65532, 65532]);
        assert!(decode(&data, &[1], 3, 1, &|| false).is_err());
        assert!(decode(&data, &[0, 0], 3, 2, &|| false).is_err());
        assert!(decode(&data, &[0], usize::MAX, 2, &|| false).is_err());
        assert!(decode(&data, &[0], 0, 1, &|| false).is_err());
        assert!(matches!(
            decode(&data, &[0], 3, 1, &|| true),
            Err(DecodeError::Cancelled)
        ));
        // Reusing an uninitialized length or a negative first predictor is invalid.
        assert!(decode(&encode_bits(&[1, 1]), &[0], 8, 1, &|| false).is_err());
        let mut negative = vec![0, 1, 0, 0, 1, 0];
        append_value(&mut negative, 0, 8);
        assert!(decode(&encode_bits(&negative), &[0], 8, 1, &|| false).is_err());
    }
    #[test]
    #[ignore = "requires pinned Phase One P20+ IIQ object 4366 and independent sensor oracle"]
    fn real_format3_matches_full_independent_sensor() {
        let source = std::fs::read(std::env::var("RRRAH_IIQ_SOURCE").unwrap()).unwrap();
        let oracle = std::fs::read(std::env::var("RRRAH_IIQ_ORACLE").unwrap()).unwrap();
        let offsets: Vec<u32> = source[20583476..20583476 + 16512]
            .chunks_exact(4)
            .map(|b| u32::from_le_bytes(b.try_into().unwrap()))
            .collect();
        let sensor = &source[20..20583392];
        let decoded = decode(sensor, &offsets, 4134, 4128, &|| false).unwrap();
        assert_eq!(oracle.len(), decoded.len() * 2);
        for (i, (actual, expected)) in decoded.iter().zip(oracle.chunks_exact(2)).enumerate() {
            assert_eq!(
                *actual,
                u16::from_le_bytes([expected[0], expected[1]]),
                "sample {i}"
            );
        }
        assert!(decode(&sensor[..sensor.len() / 2], &offsets, 4134, 4128, &|| false).is_err());
        assert!(matches!(
            decode(sensor, &offsets, 4134, 4128, &|| true),
            Err(DecodeError::Cancelled)
        ));
    }
}
