//! Bounded Olympus adaptive residual stream decoder, in full sensor coordinates.
use super::camera_error;
use crate::DecodeError;

struct Bits<'a> {
    bytes: &'a [u8],
    position: usize,
}
impl Bits<'_> {
    fn take(&mut self, count: usize) -> Result<i32, DecodeError> {
        let end = self.position.checked_add(count).ok_or(DecodeError::DimensionOverflow)?;
        if end > self.bytes.len().saturating_mul(8) {
            return Err(camera_error("ORF", "truncated Olympus entropy stream"));
        }
        let mut value = 0;
        while self.position < end {
            value = (value << 1) | i32::from((self.bytes[self.position / 8] >> (7 - self.position % 8)) & 1);
            self.position += 1;
        }
        Ok(value)
    }
    #[inline]
    fn take_mode<const WINDOW: bool>(&mut self, count: usize) -> Result<i32, DecodeError> {
        if WINDOW { return self.take_window(count); }
        self.take(count)
    }
    #[inline]
    fn take_window(&mut self, count: usize) -> Result<i32, DecodeError> {
        let end = self
            .position
            .checked_add(count)
            .ok_or(DecodeError::DimensionOverflow)?;
        if end > self.bytes.len().saturating_mul(8) {
            return Err(camera_error("ORF", "truncated Olympus entropy stream"));
        }
        if count == 0 {
            return Ok(0);
        }
        // Residual widths are at most 13 bits; read the covering bytes once
        // instead of iterating over each individual bit.
        if count > 16 {
            return Err(camera_error("ORF", "invalid Olympus bit width"));
        }
        let offset = self.position % 8;
        let first = self.position / 8;
        if offset + count <= 8 {
            let value = (u32::from(self.bytes[first]) >> (8 - offset - count)) & ((1_u32 << count) - 1);
            self.position = end;
            return Ok(value as i32);
        }
        let covered = (offset + count).div_ceil(8);
        let mut value = u32::from(self.bytes[first]);
        if covered >= 2 {
            value = (value << 8) | u32::from(self.bytes[first + 1]);
        }
        if covered == 3 {
            value = (value << 8) | u32::from(self.bytes[first + 2]);
        }
        value = (value >> (covered * 8 - offset - count)) & ((1_u32 << count) - 1);
        self.position = end;
        Ok(value as i32)
    }
}

pub(super) fn decode(
    bytes: &[u8],
    output: &mut [u16],
    width: usize,
    cancelled: &(dyn Fn() -> bool + Sync),
) -> Result<(), DecodeError> {
    decode_impl::<true, true>(bytes, output, width, cancelled)
}

fn decode_impl<const WINDOW: bool, const PREFIX: bool>(
    bytes: &[u8],
    output: &mut [u16],
    width: usize,
    cancelled: &(dyn Fn() -> bool + Sync),
) -> Result<(), DecodeError> {
    if width == 0 || !width.is_multiple_of(2) || !output.len().is_multiple_of(width) {
        return Err(camera_error("ORF", "invalid Olympus entropy dimensions"));
    }
    let mut bits = Bits {
        bytes: bytes
            .get(7..)
            .ok_or_else(|| camera_error("ORF", "truncated Olympus entropy header"))?,
        position: 0,
    };
    for row in 0..output.len() / width {
        if cancelled() {
            return Err(DecodeError::Cancelled);
        }
        let mut state = [[0_i32; 3]; 2];
        for col in 0..width {
            let carry = &mut state[col % 2];
            let adjustment = if carry[2] < 3 { 2 } else { 0 };
            let active = 32 - i32::try_from(carry[0].leading_zeros()).expect("at most 32 bits");
            let low_bits = (active - adjustment).max(2 + adjustment);
            if !(2..=14).contains(&low_bits) {
                return Err(camera_error("ORF", "invalid Olympus residual state"));
            }
            let (negative, remainder) = if PREFIX {
                let prefix = bits.take_mode::<WINDOW>(3)?;
                (prefix & 4 != 0, prefix & 3)
            } else {
                (bits.take_mode::<WINDOW>(1)? != 0, bits.take_mode::<WINDOW>(2)?)
            };
            let mut high = 0;
            while high < 12 && bits.take_mode::<WINDOW>(1)? == 0 {
                high += 1;
            }
            if high == 12 {
                high = bits.take_mode::<WINDOW>(usize::try_from(15 - low_bits).expect("validated residual width"))?;
                bits.take_mode::<WINDOW>(1)?;
            }
            carry[0] = (high << low_bits)
                | bits.take_mode::<WINDOW>(usize::try_from(low_bits).expect("validated residual width"))?;
            let residual = (if negative { !carry[0] } else { carry[0] }) + carry[1];
            carry[1] = (3 * residual + carry[1]) >> 5;
            carry[2] = if carry[0] > 16 { 0 } else { carry[2] + 1 };
            let at = row * width + col;
            let prediction = match (row >= 2, col >= 2) {
                (false, false) => 0,
                (false, true) => i32::from(output[at - 2]),
                (true, false) => i32::from(output[at - 2 * width]),
                (true, true) => {
                    let left = i32::from(output[at - 2]);
                    let up = i32::from(output[at - 2 * width]);
                    let corner = i32::from(output[at - 2 * width - 2]);
                    let dx = left - corner;
                    let dy = up - corner;
                    if (dx < 0 && dy > 0) || (dx > 0 && dy < 0) {
                        if dx.abs() > 32 || dy.abs() > 32 {
                            left + dy
                        } else {
                            (left + up) >> 1
                        }
                    } else if dx.abs() > dy.abs() {
                        left
                    } else {
                        up
                    }
                }
            };
            output[at] = u16::try_from(prediction + ((residual * 4) | remainder))
                .map_err(|_| camera_error("ORF", "Olympus prediction exceeds sensor range"))?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn byte_window_matches_bit_reference_at_every_alignment_and_tail() {
        let bytes: Vec<u8> = (0..257).map(|i| ((i * 73 + 19) % 256) as u8).collect();
        for length in 0..=bytes.len() {
            let source = &bytes[..length];
            for position in length.saturating_sub(3) * 8..=length * 8 {
                for count in 0..=16 {
                    let mut bits = Bits { bytes: source, position };
                    let result = bits.take_window(count);
                    if position + count > length * 8 {
                        assert!(result.is_err());
                        assert_eq!(bits.position, position);
                    } else {
                        let expected = (position..position + count).fold(0, |v, bit|
                            (v << 1) | i32::from((source[bit / 8] >> (7 - bit % 8)) & 1));
                        assert_eq!(result.unwrap(), expected);
                        assert_eq!(bits.position, position + count);
                    }
                }
            }
        }
    }

    #[test]
    #[ignore = "manual paired bit-reader throughput benchmark"]
    fn paired_bit_reader_throughput() {
        use std::{hint::black_box, time::Instant};
        let bytes: Vec<u8> = (0..1024 * 1024).map(|i| ((i * 73 + 19) % 256) as u8).collect();
        for round in 0..6 {
            let mut results = [(0_u64, 0.0); 2];
            for mode in if round % 2 == 0 { [0, 1] } else { [1, 0] } {
                let started = Instant::now();
                let mut reader = Bits { bytes: black_box(&bytes), position: 0 };
                let mut checksum = 0_u64;
                let mut index = 0;
                while reader.position + 16 <= bytes.len() * 8 {
                    let count = black_box([1, 2, 1, 4, 1, 7, 13][index % 7]);
                    let value = if mode == 0 {
                        reader.take(count).unwrap()
                    } else {
                        reader.take_window(count).unwrap()
                    };
                    checksum = checksum.wrapping_add(black_box(value) as u64);
                    index += 1;
                }
                results[mode] = (checksum, started.elapsed().as_secs_f64() * 1000.0);
            }
            assert_eq!(results[0].0, results[1].0);
            eprintln!("ORF paired bits: round={round}; reference_ms={:.3}; window_ms={:.3}; checksum={}", results[0].1, results[1].1, results[0].0);
        }
    }

    #[test]
    #[ignore = "manual paired real ORF entropy stream benchmark"]
    fn paired_real_orf_entropy() {
        let path = std::env::var("RRRAH_ORF_ENTROPY_STREAM").unwrap();
        let bytes = std::fs::read(path).unwrap();
        let mut baseline = vec![0; 4640 * 3472];
        let mut candidate = vec![0; baseline.len()];
        for round in 0..6 {
            let mut times = [0.0; 2];
            for mode in if round % 2 == 0 { [0, 1] } else { [1, 0] } {
                let start = std::time::Instant::now();
                if mode == 0 { decode_impl::<false, false>(&bytes, &mut baseline, 4640, &|| false).unwrap(); }
                else { decode_impl::<true, true>(&bytes, &mut candidate, 4640, &|| false).unwrap(); }
                times[mode] = start.elapsed().as_secs_f64() * 1000.0;
            }
            assert_eq!(baseline, candidate);
            eprintln!("ORF paired real: round={round}; reference_ms={:.3}; window_ms={:.3}; samples=16110080; all_exact", times[0], times[1]);
        }
    }

    #[test]
    #[ignore = "manual paired real ORF prefix benchmark"]
    fn paired_real_orf_prefix() {
        let bytes = std::fs::read(std::env::var("RRRAH_ORF_ENTROPY_STREAM").unwrap()).unwrap();
        let mut old = vec![0; 4640 * 3472];
        let mut fused = vec![0; old.len()];
        for round in 0..6 {
            let mut times = [0.; 2];
            for mode in if round % 2 == 0 { [0, 1] } else { [1, 0] } {
                let start = std::time::Instant::now();
                if mode == 0 { decode_impl::<true, false>(&bytes, &mut old, 4640, &|| false).unwrap(); }
                else { decode_impl::<true, true>(&bytes, &mut fused, 4640, &|| false).unwrap(); }
                times[mode] = start.elapsed().as_secs_f64()*1000.;
            }
            assert_eq!(old, fused);
            eprintln!("ORF fused prefix: round={round}; old_ms={:.3}; fused_ms={:.3}; samples=16110080; exact", times[0], times[1]);
        }
    }

    #[test]
    fn zero_residuals_predict_full_sensor_plane() {
        // Each sample: positive sign, zero low remainder, unary high=0,
        // four low magnitude bits=0. State resets at each row.
        let mut bytes = vec![0; 7];
        bytes.extend_from_slice(&[0x10; 8]);
        let mut output = [99; 8];
        decode(&bytes, &mut output, 4, &|| false).unwrap();
        assert_eq!(output, [0; 8]);
    }
    #[test]
    fn truncated_stream_and_cancellation_are_errors() {
        for length in 0..8 {
            assert!(decode(&vec![0; length], &mut [0; 4], 2, &|| false).is_err());
        }
        assert!(matches!(
            decode(&[0; 8], &mut [0; 4], 2, &|| true),
            Err(DecodeError::Cancelled)
        ));
        assert!(decode(&[0; 8], &mut [0; 4], 3, &|| false).is_err());
    }
}
