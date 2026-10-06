//! Bounded Olympus adaptive residual stream decoder, in full sensor coordinates.
use super::camera_error;
use crate::DecodeError;

struct Bits<'a> {
    bytes: &'a [u8],
    position: usize,
}
impl Bits<'_> {
    fn take(&mut self, count: usize) -> Result<i32, DecodeError> {
        let end = self
            .position
            .checked_add(count)
            .ok_or(DecodeError::DimensionOverflow)?;
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
}

pub(super) fn decode(
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
            let negative = bits.take(1)? != 0;
            let remainder = bits.take(2)?;
            let mut high = 0;
            while high < 12 && bits.take(1)? == 0 {
                high += 1;
            }
            if high == 12 {
                high = bits.take(usize::try_from(15 - low_bits).expect("validated residual width"))?;
                bits.take(1)?;
            }
            carry[0] = (high << low_bits)
                | bits.take(usize::try_from(low_bits).expect("validated residual width"))?;
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
