//! Bounded Hasselblad predictor-8 entropy subset.
use super::camera_error;
use crate::DecodeError;
fn error(message: &str) -> DecodeError {
    camera_error("FFF", message)
}
struct Bits<'a> {
    data: &'a [u8],
    offset: usize,
    buffer: u64,
    available: u32,
}
impl Bits<'_> {
    fn peek(&mut self, count: u32) -> Result<u32, DecodeError> {
        if count > 16 {
            return Err(error("invalid amplitude length"));
        }
        if count == 0 {
            return Ok(0);
        }
        if self.available < count {
            let end = self.offset.checked_add(4).ok_or(DecodeError::DimensionOverflow)?;
            let word: [u8; 4] = self
                .data
                .get(self.offset..end)
                .ok_or_else(|| error("truncated entropy word"))?
                .try_into()
                .unwrap();
            self.offset = end;
            self.buffer = (self.buffer << 32) | u64::from(u32::from_le_bytes(word));
            self.available += 32;
        }
        Ok(((self.buffer >> (self.available - count)) & ((1u64 << count) - 1)) as u32)
    }
    fn consume(&mut self, count: u32) {
        self.available -= count;
        self.buffer &= (1u64 << self.available) - 1;
    }
    fn read(&mut self, count: u32) -> Result<u32, DecodeError> {
        let result = self.peek(count)?;
        self.consume(count);
        Ok(result)
    }
}
struct Huffman {
    counts: [u8; 16],
    values: Vec<u8>,
    prefixes: [u16; 256],
}
impl Huffman {
    fn new(counts: [u8; 16], values: Vec<u8>) -> Self {
        let mut prefixes = [0u16; 256];
        let mut code = 0usize;
        let mut index = 0usize;
        for (depth, count) in counts.into_iter().enumerate() {
            let length = depth + 1;
            for _ in 0..count {
                if length <= 8 {
                    let start = code << (8 - length);
                    let end = start + (1 << (8 - length));
                    prefixes[start..end].fill(((length as u16) << 8) | u16::from(values[index]));
                }
                index += 1;
                code += 1;
            }
            code <<= 1;
        }
        Self {
            counts,
            values,
            prefixes,
        }
    }
    fn read<const FAST: bool>(&self, bits: &mut Bits<'_>) -> Result<u32, DecodeError> {
        // Preserve valid short tails: do not demand another word only to peek.
        if FAST && (bits.available >= 8 || bits.data.len().saturating_sub(bits.offset) >= 4) {
            let entry = self.prefixes[bits.peek(8)? as usize];
            if entry != 0 {
                bits.consume(u32::from(entry >> 8));
                return Ok(u32::from(entry & 255));
            }
        }
        let mut code = 0u32;
        let mut first = 0u32;
        let mut index = 0usize;
        for count in self.counts {
            code = (code << 1) | bits.read(1)?;
            if code >= first && code - first < u32::from(count) {
                return Ok(u32::from(self.values[index + (code - first) as usize]));
            }
            index += usize::from(count);
            first = (first + u32::from(count)) << 1;
        }
        Err(error("invalid Huffman code"))
    }
}

// The camera backend reserves the output before calling this flat decoder.
pub(crate) fn decode(
    data: &[u8],
    width: usize,
    height: usize,
    cancelled: &(dyn Fn() -> bool + Sync),
) -> Result<Vec<u16>, DecodeError> {
    decode_inner::<true>(data, width, height, cancelled)
}
fn decode_inner<const FAST: bool>(
    data: &[u8],
    width: usize,
    height: usize,
    cancelled: &(dyn Fn() -> bool + Sync),
) -> Result<Vec<u16>, DecodeError> {
    if data.get(..2) != Some(&[255, 216]) || width == 0 || width % 2 != 0 || height == 0 {
        return Err(error("invalid predictor-8 dimensions or JPEG header"));
    }
    let samples = width.checked_mul(height).ok_or(DecodeError::DimensionOverflow)?;
    if samples > 200_000_000 {
        return Err(error("sensor exceeds sample limit"));
    }
    if cancelled() {
        return Err(DecodeError::Cancelled);
    }
    let mut position = 2usize;
    let mut table = None;
    let mut frame = false;
    loop {
        if data.get(position) != Some(&255) {
            return Err(error("invalid JPEG marker"));
        }
        while data.get(position) == Some(&255) {
            position += 1;
        }
        let marker = *data.get(position).ok_or_else(|| error("truncated marker"))?;
        position += 1;
        let length_bytes = data
            .get(position..position + 2)
            .ok_or_else(|| error("truncated segment length"))?;
        let length = usize::from(u16::from_be_bytes(length_bytes.try_into().unwrap()));
        if length < 2 {
            return Err(error("invalid segment length"));
        }
        let end = position
            .checked_add(length)
            .ok_or(DecodeError::DimensionOverflow)?;
        let payload = data
            .get(position + 2..end)
            .ok_or_else(|| error("truncated JPEG segment"))?;
        position = end;
        match marker {
            0xc3 => {
                if frame
                    || payload.len() != 9
                    || payload[0] != 16
                    || usize::from(u16::from_be_bytes([payload[1], payload[2]])) != height
                    || usize::from(u16::from_be_bytes([payload[3], payload[4]])) != width
                    || payload[5..] != [1, 0, 17, 0]
                {
                    return Err(error("unqualified SOF3 frame"));
                }
                frame = true;
            }
            0xc4 => {
                if table.is_some() || payload.len() < 17 || payload[0] != 0 {
                    return Err(error("unqualified Huffman table"));
                }
                let counts: [u8; 16] = payload[1..17].try_into().unwrap();
                let total: usize = counts.iter().map(|x| usize::from(*x)).sum();
                if total == 0
                    || total > 17
                    || payload.len() != 17 + total
                    || payload[17..].iter().any(|x| *x > 16)
                {
                    return Err(error("invalid Huffman symbols"));
                }
                let mut slots = 1i32;
                for count in counts {
                    slots = slots * 2 - i32::from(count);
                    if slots < 0 {
                        return Err(error("oversubscribed Huffman table"));
                    }
                }
                table = Some(Huffman::new(counts, payload[17..].to_vec()));
            }
            0xda => {
                if !frame || payload != [1, 0, 0, 8, 0, 0] {
                    return Err(error("unqualified predictor-8 scan"));
                }
                break;
            }
            _ => return Err(error("unsupported JPEG marker in FFF sensor")),
        }
    }
    let table = table.ok_or_else(|| error("missing Huffman table"))?;
    let mut bits = Bits {
        data: &data[position..],
        offset: 0,
        buffer: 0,
        available: 0,
    };
    let mut out = Vec::new();
    out.try_reserve_exact(samples)
        .map_err(|_| error("sensor allocation failed"))?;
    for _ in 0..height {
        if cancelled() {
            return Err(DecodeError::Cancelled);
        }
        let mut predictor = [32768i32; 2];
        for _ in 0..width / 2 {
            let lengths = [table.read::<FAST>(&mut bits)?, table.read::<FAST>(&mut bits)?];
            for (channel, length) in lengths.into_iter().enumerate() {
                let amplitude = bits.read(length)?;
                let mut difference = amplitude as i32;
                if length > 0 && amplitude & (1 << (length - 1)) == 0 {
                    difference -= (1i32 << length) - 1;
                }
                if difference == 65535 {
                    difference = -32768;
                }
                predictor[channel] = predictor[channel]
                    .checked_add(difference)
                    .ok_or_else(|| error("prediction overflow"))?;
                out.push((predictor[channel] & 65535) as u16);
            }
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    fn fixture(width: u16, height: u16, category: u8, word: u32) -> Vec<u8> {
        let mut data = vec![255, 216];
        let mut segment = |marker, payload: &[u8]| {
            data.extend_from_slice(&[255, marker]);
            data.extend_from_slice(&((payload.len() + 2) as u16).to_be_bytes());
            data.extend_from_slice(payload);
        };
        let [h0, h1] = height.to_be_bytes();
        let [w0, w1] = width.to_be_bytes();
        segment(0xc3, &[16, h0, h1, w0, w1, 1, 0, 17, 0]);
        let mut table = vec![0; 18];
        table[1] = 1;
        table[17] = category;
        segment(0xc4, &table);
        segment(0xda, &[1, 0, 0, 8, 0, 0]);
        data.extend_from_slice(&word.to_le_bytes());
        data
    }
    #[test]
    fn prefix_lookup_preserves_long_codes_and_short_final_tail() {
        let mut counts = [0; 16];
        counts[8] = 1;
        let table = super::Huffman::new(counts, vec![7]);
        let data = [0u8; 4];
        let mut bits = super::Bits {
            data: &data,
            offset: 0,
            buffer: 0,
            available: 0,
        };
        assert_eq!(table.read::<true>(&mut bits).unwrap(), 7);
        assert_eq!(bits.available, 23);
        let mut counts = [0; 16];
        counts[0] = 1;
        let table = super::Huffman::new(counts, vec![0]);
        let mut tail = super::Bits {
            data: &[],
            offset: 0,
            buffer: 0,
            available: 1,
        };
        assert_eq!(table.read::<true>(&mut tail).unwrap(), 0);
        assert_eq!(tail.available, 0);
        assert!(table.read::<true>(&mut tail).is_err());
    }

    #[test]
    fn tiny_predictor8_signed_pairs_and_row_reset() {
        let positive = fixture(4, 2, 1, 0x33330000);
        assert_eq!(
            super::decode(&positive, 4, 2, &|| false).unwrap(),
            [32769, 32769, 32770, 32770, 32769, 32769, 32770, 32770]
        );
        let negative = fixture(4, 2, 1, 0);
        assert_eq!(
            super::decode(&negative, 4, 2, &|| false).unwrap(),
            [32767, 32767, 32766, 32766, 32767, 32767, 32766, 32766]
        );
        let zero = fixture(2, 1, 0, 0);
        assert_eq!(super::decode(&zero, 2, 1, &|| false).unwrap(), [32768, 32768]);
    }
    #[test]
    fn tiny_predictor8_rejects_truncated_words_bad_codes_and_mismatched_dimensions() {
        let data = fixture(2, 1, 0, 0);
        for end in 0..data.len() {
            assert!(
                super::decode(&data[..end], 2, 1, &|| false).is_err(),
                "prefix {end}"
            );
        }
        assert!(super::decode(&data, 4, 1, &|| false).is_err());
        assert!(super::decode(&fixture(2, 1, 0, u32::MAX), 2, 1, &|| false).is_err());
        assert!(super::decode(&fixture(2, 1, 17, 0), 2, 1, &|| false).is_err());
    }
    #[test]
    #[ignore = "requires pinned CC0 FFF source; paired full-sensor performance run"]
    fn paired_prefix_vs_bitwise_full_sensor_timing() {
        let source = std::fs::read(std::env::var("RRRAH_HASSELBLAD_FFF_SOURCE").unwrap()).unwrap();
        let data = &source[9825436..9825436 + 69614078];
        let mut times = [Vec::new(), Vec::new()];
        for pair in 0..12 {
            let mut outputs = [None, None];
            for offset in 0..2 {
                let variant = (pair + offset) % 2;
                let start = std::time::Instant::now();
                let pixels = if variant == 0 {
                    super::decode_inner::<false>(std::hint::black_box(data), 8282, 6240, &|| false).unwrap()
                } else {
                    super::decode_inner::<true>(std::hint::black_box(data), 8282, 6240, &|| false).unwrap()
                };
                let elapsed = start.elapsed().as_secs_f64() * 1000.0;
                if pair >= 4 {
                    times[variant].push(elapsed);
                }
                outputs[variant] = Some(pixels);
            }
            assert_eq!(outputs[0], outputs[1]);
        }
        for (label, values) in ["bitwise", "prefix"].into_iter().zip(times.iter()) {
            let mut sorted = values.clone();
            sorted.sort_by(f64::total_cmp);
            eprintln!(
                "paired FFF {label} times_ms={values:?} p50_ms={} p95_ms={}",
                sorted[4], sorted[7]
            );
        }
        let deltas: Vec<_> = times[0].iter().zip(&times[1]).map(|(a, b)| a - b).collect();
        eprintln!("paired FFF bitwise-minus-prefix deltas_ms={deltas:?}");
    }

    #[test]
    #[ignore = "requires pinned CC0 Hasselblad CFV-50 FFF source and LibRaw dump"]
    fn complete_real_sensor_matches_oracle_and_truncation_cancels() {
        let source = std::fs::read(std::env::var("RRRAH_HASSELBLAD_FFF_SOURCE").unwrap()).unwrap();
        let data = &source[9825436..9825436 + 69614078];
        let expected = std::fs::read(std::env::var("RRRAH_HASSELBLAD_FFF_REFERENCE").unwrap()).unwrap();
        let decoded = super::decode(data, 8282, 6240, &|| false).unwrap();
        assert_eq!(decoded.len() * 2, expected.len());
        assert!(
            decoded
                .iter()
                .zip(expected.chunks_exact(2))
                .all(|(a, b)| *a == u16::from_le_bytes([b[0], b[1]]))
        );
        assert!(super::decode(&data[..data.len() / 2], 8282, 6240, &|| false).is_err());
        assert!(matches!(
            super::decode(data, 8282, 6240, &|| true),
            Err(crate::DecodeError::Cancelled)
        ));
    }
}
