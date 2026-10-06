//! Borrowed, bounded VC-5 tag/chunk reader. Pixel reconstruction is separate.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Vc5Record<'a> {
    pub offset: usize,
    pub tag: u16,
    pub optional: bool,
    pub value: u16,
    pub payload: Option<&'a [u8]>,
}

impl<'a> Vc5Record<'a> {
    /// Enter a structural section, never entropy-coded blocks or vendor data.
    pub fn section_records(&self) -> Option<Vc5Records<'a>> {
        if !matches!(
            self.tag & 0xff00,
            0x2000 | 0x2100 | 0x2400 | 0x2500 | 0x2600 | 0x2700
        ) {
            return None;
        }
        self.payload.map(|bytes| Vc5Records {
            bytes,
            offset: 0,
            failed: false,
        })
    }
}

/// Unpack the explicitly 16-bit lowpass representation without allocation.
/// Width/height must be derived from the admitted channel and wavelet geometry.
pub fn lowpass_16(
    payload: &[u8],
    width: usize,
    height: usize,
) -> Result<impl ExactSizeIterator<Item = u16> + '_, &'static str> {
    let size = width
        .checked_mul(height)
        .and_then(|n| n.checked_mul(2))
        .ok_or("VC-5 lowpass extent")?;
    if width == 0 || height == 0 || payload.len() != size {
        return Err("VC-5 lowpass size mismatch");
    }
    Ok(payload.chunks_exact(2).map(|v| u16::from_be_bytes([v[0], v[1]])))
}

#[derive(Debug)]
pub struct Vc5Records<'a> {
    bytes: &'a [u8],
    offset: usize,
    failed: bool,
}
impl<'a> Vc5Records<'a> {
    pub fn new(bytes: &'a [u8]) -> Result<Self, &'static str> {
        if bytes.get(..4) != Some(b"VC-5") {
            return Err("VC-5 signature");
        }
        Ok(Self {
            bytes,
            offset: 4,
            failed: false,
        })
    }
}
impl<'a> Iterator for Vc5Records<'a> {
    type Item = Result<Vc5Record<'a>, &'static str>;
    fn next(&mut self) -> Option<Self::Item> {
        if self.failed || self.offset == self.bytes.len() {
            return None;
        }
        let offset = self.offset;
        let result = (|| {
            let header = self
                .bytes
                .get(offset..offset.checked_add(4).ok_or("VC-5 offset")?)
                .ok_or("truncated VC-5 tag")?;
            let signed = i16::from_be_bytes([header[0], header[1]]);
            let tag = signed.unsigned_abs();
            let value = u16::from_be_bytes([header[2], header[3]]);
            let start = offset + 4;
            let payload = if tag & 0x6000 != 0 {
                let segments = if tag & 0x2000 != 0 {
                    (usize::from(tag & 255) << 16) | usize::from(value)
                } else {
                    usize::from(value)
                };
                let end = start
                    .checked_add(segments.checked_mul(4).ok_or("VC-5 chunk size")?)
                    .ok_or("VC-5 chunk extent")?;
                let payload = self.bytes.get(start..end).ok_or("truncated VC-5 chunk")?;
                self.offset = end;
                Some(payload)
            } else {
                self.offset = start;
                None
            };
            Ok(Vc5Record {
                offset,
                tag,
                optional: signed < 0,
                value,
                payload,
            })
        })();
        if result.is_err() {
            self.failed = true;
        }
        Some(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn bayer_assembly_refuses_bad_geometry_and_cancels_before_writing() {
        let component = [2048i16; 1];
        let mut output = [999u16; 4];
        assert!(
            assemble_bayer(
                [&component; 4],
                2,
                2,
                14,
                BayerOrder::Rggb,
                &mut output,
                &mut || true
            )
            .is_err()
        );
        assert_eq!(output, [999; 4]);
        assert!(
            assemble_bayer(
                [&component; 4],
                3,
                2,
                14,
                BayerOrder::Rggb,
                &mut output,
                &mut || false
            )
            .is_err()
        );
        assert_eq!(output, [999; 4]);
    }

    #[test]
    fn component_to_bayer_cells_match_independent_gopro_all_output_precisions() {
        let bytes = include_bytes!("../../../tests/fixtures/vc5/bayer-cell-gopro.u16le");
        for record in bytes.chunks_exact(18) {
            let words: [u16; 9] =
                std::array::from_fn(|i| u16::from_le_bytes([record[i * 2], record[i * 2 + 1]]));
            let components = std::array::from_fn(|i| words[i + 1] as i16);
            let actual = components_to_bayer_cell(components, words[0] as u8).unwrap();
            assert_eq!(actual, words[5..9]);
        }
        assert_eq!(bytes.len(), 768 * 18);
    }

    #[test]
    fn inverse_log_matches_every_independent_gopro_code() {
        let bytes = include_bytes!("../../../tests/fixtures/vc5/logcurve-gopro.u16le");
        for (code, b) in bytes.chunks_exact(2).enumerate() {
            assert_eq!(
                inverse_log12(code as u16).unwrap(),
                u16::from_le_bytes([b[0], b[1]]),
                "log code {code}"
            );
        }
        assert!(inverse_log12(4096).is_err());
        assert!(components_to_bayer_cell([0; 4], 10).is_err());
    }

    #[test]
    fn managed_entropy_band_reserves_before_decode_and_releases_failed_input() {
        for limit in [1, 2] {
            let root = rrrah_core::MemoryBudget::new(limit);
            let mut request = crate::DecodeRequest::new("unused.gpr");
            request.memory_budget = Some(root.clone());
            let result = decode_highpass_managed(&[98, 41, 116, 96], 1, &request);
            if limit == 1 {
                assert!(matches!(result, Err(crate::DecodeError::Memory(_))));
            } else {
                let output = result.unwrap();
                assert_eq!(&*output, &[0]);
                assert_eq!(root.used(), 2);
                drop(output);
            }
            assert_eq!(root.used(), 0);
            if limit == 2 {
                assert!(decode_highpass_managed(&[], 1, &request).is_err());
                assert_eq!(root.used(), 0);
            }
        }
    }

    #[test]
    fn managed_reconstruction_admits_overlapping_output_scratch_and_releases_on_refusal() {
        let low = [512i16; 9];
        let high = [0i16; 9];
        for limit in [95, 96] {
            let root = rrrah_core::MemoryBudget::new(limit);
            let mut request = crate::DecodeRequest::new("unused.gpr");
            request.memory_budget = Some(root.child(limit));
            let result = reconstruct_managed(
                [&low, &high, &high, &high],
                [1, 12, 24, 96],
                3,
                3,
                6,
                6,
                0,
                &request,
            );
            if limit == 95 {
                assert!(matches!(result, Err(crate::DecodeError::Memory(_))));
            } else {
                let output = result.unwrap();
                assert!(output.iter().all(|v| *v == 128));
                assert_eq!(root.used(), 72);
                assert_eq!(root.peak(), 96);
                drop(output);
            }
            assert_eq!(root.used(), 0);
        }
    }

    #[test]
    fn reconstruction_cancels_between_rows_and_returns_partial_output_to_caller() {
        let low = [512i16; 9];
        let high = [0i16; 9];
        let mut output = [-1i16; 36];
        let mut scratch = [0i16; 12];
        let mut rows = 0;
        let result = inverse_spatial_cancellable(
            [&low, &high, &high, &high],
            [1, 12, 24, 96],
            3,
            3,
            6,
            6,
            &mut output,
            &mut scratch,
            0,
            &mut || {
                rows += 1;
                rows == 2
            },
        );
        assert_eq!(result, Err("VC-5 reconstruction cancelled"));
        assert!(output[..12].iter().all(|v| *v == 128));
        assert!(output[12..].iter().all(|v| *v == -1));
    }

    #[test]
    fn spatial_inverse_matches_independent_gopro_odd_extents_and_borders() {
        let bytes = include_bytes!("../../../tests/fixtures/vc5/spatial-gopro.i16le");
        let mut words = bytes.chunks_exact(2).map(|b| i16::from_le_bytes([b[0], b[1]]));
        for _ in 0..32 {
            let width = words.next().unwrap() as usize;
            let height = words.next().unwrap() as usize;
            let ow = words.next().unwrap() as usize;
            let oh = words.next().unwrap() as usize;
            let bands: [Vec<_>; 4] = std::array::from_fn(|_| words.by_ref().take(width * height).collect());
            let expected: Vec<_> = words.by_ref().take(ow * oh).collect();
            let mut output = vec![0; ow * oh];
            let mut scratch = vec![0; width * 4];
            inverse_spatial(
                bands.each_ref().map(|b| b.as_slice()),
                [1, 12, 24, 96],
                width,
                height,
                ow,
                oh,
                &mut output,
                &mut scratch,
            )
            .unwrap();
            assert_eq!(output, expected, "{width}x{height} -> {ow}x{oh}");
        }
        assert!(words.next().is_none());
    }

    #[test]
    fn spatial_prescaled_inverse_matches_independent_gopro_odd_extents_and_borders() {
        let bytes = include_bytes!("../../../tests/fixtures/vc5/spatial-descaled-gopro.i16le");
        let mut words = bytes.chunks_exact(2).map(|b| i16::from_le_bytes([b[0], b[1]]));
        for _ in 0..32 {
            let width = words.next().unwrap() as usize;
            let height = words.next().unwrap() as usize;
            let ow = words.next().unwrap() as usize;
            let oh = words.next().unwrap() as usize;
            let bands: [Vec<_>; 4] = std::array::from_fn(|_| words.by_ref().take(width * height).collect());
            let expected: Vec<_> = words.by_ref().take(ow * oh).collect();
            let mut output = vec![0; ow * oh];
            let mut scratch = vec![0; width * 4];
            inverse_spatial_prescaled(
                bands.each_ref().map(|b| b.as_slice()),
                [1, 12, 24, 96],
                width,
                height,
                ow,
                oh,
                &mut output,
                &mut scratch,
                2,
            )
            .unwrap();
            assert_eq!(output, expected, "{width}x{height} -> {ow}x{oh}");
        }
        assert!(words.next().is_none());
    }

    #[test]
    fn horizontal_inverse_matches_independent_gopro_edges_odd_widths_and_saturation() {
        let bytes = include_bytes!("../../../tests/fixtures/vc5/horizontal-gopro.i16le");
        let mut words = bytes.chunks_exact(2).map(|b| i16::from_le_bytes([b[0], b[1]]));
        for _ in 0..256 {
            let width = words.next().unwrap() as usize;
            let output_width = words.next().unwrap() as usize;
            let low: Vec<_> = words.by_ref().take(width).collect();
            let high: Vec<_> = words.by_ref().take(width).collect();
            let expected: Vec<_> = words.by_ref().take(output_width).collect();
            let mut output = vec![0; output_width];
            inverse_horizontal(&low, &high, &mut output).unwrap();
            assert_eq!(output, expected);
        }
        assert!(words.next().is_none());
        let mut untouched = [123; 4];
        assert!(inverse_horizontal(&[1, 2], &[1, 2], &mut untouched).is_err());
        assert_eq!(untouched, [123; 4]);
    }

    #[test]
    fn horizontal_prescaled_inverse_matches_independent_gopro_edges_odd_widths_and_saturation() {
        let bytes = include_bytes!("../../../tests/fixtures/vc5/horizontal-descaled-gopro.i16le");
        let mut words = bytes.chunks_exact(2).map(|b| i16::from_le_bytes([b[0], b[1]]));
        for _ in 0..256 {
            let width = words.next().unwrap() as usize;
            let output_width = words.next().unwrap() as usize;
            let low: Vec<_> = words.by_ref().take(width).collect();
            let high: Vec<_> = words.by_ref().take(width).collect();
            let expected: Vec<_> = words.by_ref().take(output_width).collect();
            let mut output = vec![0; output_width];
            inverse_horizontal_prescaled(&low, &high, &mut output, 2).unwrap();
            assert_eq!(output, expected);
        }
        assert!(words.next().is_none());
        let mut untouched = [123; 4];
        assert!(inverse_horizontal_prescaled(&[1, 2], &[1, 2], &mut untouched, 2).is_err());
        assert_eq!(untouched, [123; 4]);
    }

    #[test]
    fn inverse_companding_and_quantization_match_independent_gopro_oracle() {
        let reference = include_bytes!("../../../tests/fixtures/vc5/dequantize-gopro.i16le");
        let mut samples = reference.chunks_exact(2);
        for factor in [0, 1, 12, 24, 48, 96, 144, 65535] {
            for value in -255..=255 {
                let bytes = samples.next().unwrap();
                let expected = i16::from_le_bytes([bytes[0], bytes[1]]);
                assert_eq!(
                    dequantize(value, factor).unwrap(),
                    expected,
                    "value {value}, factor {factor}"
                );
            }
        }
        assert!(samples.next().is_none());
        assert!(dequantize(i32::MIN, 1).is_err());
        assert!(dequantize(256, 1).is_err());
    }

    #[test]
    fn header_and_borrowed_chunks_have_exact_extents() {
        let bytes = b"VC-5\0\x14\x15\xc0\xc0\0\0\x01abcd";
        let rows: Vec<_> = Vc5Records::new(bytes).unwrap().collect::<Result<_, _>>().unwrap();
        assert_eq!(rows[0].value, 5568);
        assert_eq!(rows[1].tag, 0x4000);
        assert!(rows[1].optional);
        assert_eq!(rows[1].payload.unwrap(), b"abcd");
        assert_eq!(rows[1].payload.unwrap().as_ptr(), bytes[12..].as_ptr());
    }
    #[test]
    fn truncation_fails_once_without_allocation_or_resynchronization() {
        for bytes in [b"VC-5\0".as_slice(), b"VC-5\x60\xff\xff\xff".as_slice()] {
            let mut records = Vc5Records::new(bytes).unwrap();
            assert!(records.next().unwrap().is_err());
            assert!(records.next().is_none());
        }
        assert!(Vc5Records::new(b"JPEG").is_err());
    }
    #[test]
    fn lowpass_unpack_has_exact_geometry_and_byte_order() {
        let bytes = [0x09, 0xbb, 0x09, 0x96, 0x0a, 0xcd, 0x0b, 0xca];
        assert_eq!(
            lowpass_16(&bytes, 2, 2).unwrap().collect::<Vec<_>>(),
            [2491, 2454, 2765, 3018]
        );
        assert!(lowpass_16(&bytes, 1, 2).is_err());
        assert!(lowpass_16(&bytes, usize::MAX, 2).is_err());
        assert!(lowpass_16(&[], 0, 0).is_err());
    }

    #[test]
    fn entropy_sign_runs_band_end_and_truncation() {
        let pack = |bits: &str| {
            let mut output = vec![0u8; bits.len().div_ceil(8)];
            for (i, bit) in bits.bytes().enumerate() {
                if bit == b'1' {
                    output[i / 8] |= 1 << (7 - i % 8);
                }
            }
            output
        };
        // zero, +1, -2, twelve zeros, then the codebook-17 end marker.
        let bits = format!("010011111101001{:026b}", 0x03114ba3u32);
        let bytes = pack(&bits);
        assert_eq!(
            Vc5Runs::new(&bytes, 15).collect::<Result<Vec<_>, _>>().unwrap(),
            [(1, 0), (1, 1), (1, -2), (12, 0)]
        );
        assert!(Vc5Runs::new(&pack("1101001"), 11).next().unwrap().is_err());
        let mut truncated = Vc5Runs::new(&[], 1);
        assert!(truncated.next().unwrap().is_err());
        assert!(truncated.next().is_none());
        assert!(
            Vc5Runs::new(&pack("0"), 1)
                .collect::<Result<Vec<_>, _>>()
                .is_err()
        );
    }

    #[test]
    #[ignore = "requires pinned official HERO9 GPR; set RRRAH_GPR_SOURCE"]
    fn pinned_hero9_has_four_bounded_channel_sections() {
        let bytes = std::fs::read(std::env::var_os("RRRAH_GPR_SOURCE").unwrap()).unwrap();
        assert_eq!(bytes.len(), 6763778);
        let indexed = index_raw(&bytes[14622..]).unwrap();
        assert_eq!(
            (indexed.width, indexed.height, indexed.packed_prescale),
            (5568, 4176, 10240)
        );
        assert_eq!(indexed.bands[0][0].payload.len(), 181656);
        assert_eq!(indexed.bands[0][1].payload.len(), 47692);
        assert_eq!(indexed.bands[0][1].quantization, 24);
        assert_eq!(indexed.bands[0][3].quantization, 12);
        assert!(index_raw(&bytes[14622..bytes.len() - 4]).is_err());
        let rows: Vec<_> = Vc5Records::new(&bytes[14622..])
            .unwrap()
            .collect::<Result<_, _>>()
            .unwrap();
        assert_eq!(rows.iter().find(|r| r.tag == 20).unwrap().value, 5568);
        assert_eq!(rows.iter().find(|r| r.tag == 21).unwrap().value, 4176);
        let sections: Vec<_> = rows.iter().filter(|r| r.tag & 0xff00 == 0x2400).collect();
        assert_eq!(sections.len(), 4);
        let mut wavelets = 0;
        let mut subbands = 0;
        let mut blocks = 0;
        let mut first_highpass_checked = false;
        for channel in &sections {
            for wavelet in channel.section_records().unwrap() {
                let wavelet = wavelet.unwrap();
                let Some(records) = wavelet.section_records() else {
                    continue;
                };
                wavelets += 1;
                for band in records {
                    let band = band.unwrap();
                    let Some(records) = band.section_records() else {
                        continue;
                    };
                    subbands += 1;
                    for block in records {
                        let block = block.unwrap();
                        if block.tag & 0xff00 == 0x6000 {
                            blocks += 1;
                            assert!(block.section_records().is_none());
                            if block.payload.unwrap().len() == 181656 {
                                let samples = lowpass_16(block.payload.unwrap(), 348, 261).unwrap();
                                assert_eq!(samples.len(), 90828);
                            } else if !first_highpass_checked {
                                let mut coefficients = 0;
                                for run in Vc5Runs::new(block.payload.unwrap(), 90828) {
                                    coefficients += run.unwrap().0;
                                }
                                assert_eq!(coefficients, 90828);
                                first_highpass_checked = true;
                            }
                        }
                    }
                }
            }
        }
        assert_eq!((wavelets, subbands, blocks), (12, 40, 40));
        assert!(first_highpass_checked);
        assert_eq!(
            sections
                .iter()
                .map(|r| r.payload.unwrap().len())
                .collect::<Vec<_>>(),
            [2126056, 1454472, 1375580, 1792872]
        );
    }
}

#[path = "vc5_codebook.rs"]
mod codebook;

/// Signed quantized coefficient runs; inverse companding is a later stage.
#[derive(Debug)]
pub struct Vc5Runs<'a> {
    bytes: &'a [u8],
    bit: usize,
    remaining: usize,
    finished: bool,
}
impl<'a> Vc5Runs<'a> {
    pub fn new(bytes: &'a [u8], coefficients: usize) -> Self {
        Self {
            bytes,
            bit: 0,
            remaining: coefficients,
            finished: false,
        }
    }
    fn read_bit(&mut self) -> Result<u32, &'static str> {
        let byte = self.bytes.get(self.bit / 8).ok_or("truncated VC-5 entropy")?;
        let value = u32::from((byte >> (7 - self.bit % 8)) & 1);
        self.bit += 1;
        Ok(value)
    }
    fn magnitude(&mut self) -> Result<(usize, i32), &'static str> {
        let mut bits = 0;
        for length in 1..=26 {
            bits = (bits << 1) | self.read_bit()?;
            if let Some((_, _, count, value)) = codebook::CODES
                .iter()
                .find(|(size, code, _, _)| *size == length && *code == bits)
            {
                return Ok((*count, *value));
            }
        }
        Err("invalid VC-5 entropy code")
    }
}
impl Iterator for Vc5Runs<'_> {
    type Item = Result<(usize, i32), &'static str>;
    fn next(&mut self) -> Option<Self::Item> {
        if self.finished {
            return None;
        }
        let result = (|| {
            let (count, magnitude) = self.magnitude()?;
            if self.remaining == 0 {
                if (count, magnitude) != (0, 1) {
                    return Err("VC-5 band end marker");
                }
                self.finished = true;
                return Ok(None);
            }
            if count == 0 || count > self.remaining {
                return Err("VC-5 run exceeds band");
            }
            let value = if magnitude != 0 && self.read_bit()? != 0 {
                -magnitude
            } else {
                magnitude
            };
            self.remaining -= count;
            Ok(Some((count, value)))
        })();
        match result {
            Ok(Some(run)) => Some(Ok(run)),
            Ok(None) => None,
            Err(error) => {
                self.finished = true;
                Some(Err(error))
            }
        }
    }
}

/// Invert codebook-17 cubic companding and quantization with signed saturation.
/// Input must be a coefficient, not a band-end marker.
pub fn dequantize(value: i32, factor: u16) -> Result<i16, &'static str> {
    let magnitude = u64::from(value.unsigned_abs());
    if magnitude > 255 {
        return Err("VC-5 magnitude exceeds codebook");
    }
    let expanded = magnitude + magnitude * magnitude * magnitude * 768 / (255 * 255 * 255);
    let scaled = (expanded * u64::from(factor)) as i64;
    let signed = if value < 0 { -scaled } else { scaled };
    Ok(signed.clamp(i64::from(i16::MIN), i64::from(i16::MAX)) as i16)
}

/// Horizontal 2–6 inverse filter without prescale, writing an admitted row.
/// The rightmost odd sample may be omitted for odd output image widths.
pub fn inverse_horizontal(low: &[i16], high: &[i16], output: &mut [i16]) -> Result<(), &'static str> {
    inverse_horizontal_prescaled(low, high, output, 0)
}

/// Horizontal inverse filter for admitted VC-5 prescale 0 or 2.
pub fn inverse_horizontal_prescaled(
    low: &[i16],
    high: &[i16],
    output: &mut [i16],
    prescale: u8,
) -> Result<(), &'static str> {
    if !matches!(prescale, 0 | 2) {
        return Err("VC-5 horizontal prescale");
    }
    let width = low.len();
    let doubled = width.checked_mul(2).ok_or("VC-5 horizontal extent")?;
    if width < 3 || high.len() != width || !matches!(output.len(), n if n == doubled || n == doubled - 1) {
        return Err("VC-5 horizontal geometry");
    }
    for x in 0..width {
        let value = i32::from(low[x]);
        let (even, odd) = if x == 0 {
            let next = i32::from(low[1]);
            let after = i32::from(low[2]);
            (
                (11 * value - 4 * next + after + 4) >> 3,
                (5 * value + 4 * next - after + 4) >> 3,
            )
        } else if x == width - 1 {
            let previous = i32::from(low[x - 1]);
            let before = i32::from(low[x - 2]);
            (
                (5 * value + 4 * previous - before + 4) >> 3,
                (11 * value - 4 * previous + before + 4) >> 3,
            )
        } else {
            let previous = i32::from(low[x - 1]);
            let next = i32::from(low[x + 1]);
            (
                value + ((previous - next + 4) >> 3),
                value + ((-previous + next + 4) >> 3),
            )
        };
        let correction = i32::from(high[x]);
        let finish = |value: i32| {
            if prescale == 2 {
                (value * 2).clamp(i32::from(i16::MIN), i32::from(i16::MAX)) as i16
            } else {
                (value >> 1).clamp(0, 16383) as i16
            }
        };
        output[x * 2] = finish(even + correction);
        if x * 2 + 1 < output.len() {
            output[x * 2 + 1] = finish(odd - correction);
        }
    }
    Ok(())
}

/// Reconstruct four quantized bands using caller-provided output and four-row scratch.
/// Bands are LL, LH, HL, HH; this currently admits only the no-prescale transform.
pub fn inverse_spatial(
    bands: [&[i16]; 4],
    factors: [u16; 4],
    width: usize,
    height: usize,
    output_width: usize,
    output_height: usize,
    output: &mut [i16],
    scratch: &mut [i16],
) -> Result<(), &'static str> {
    inverse_spatial_prescaled(
        bands,
        factors,
        width,
        height,
        output_width,
        output_height,
        output,
        scratch,
        0,
    )
}

/// Spatial inverse with admitted VC-5 prescale 0 or 2.
pub fn inverse_spatial_prescaled(
    bands: [&[i16]; 4],
    factors: [u16; 4],
    width: usize,
    height: usize,
    output_width: usize,
    output_height: usize,
    output: &mut [i16],
    scratch: &mut [i16],
    prescale: u8,
) -> Result<(), &'static str> {
    inverse_spatial_cancellable(
        bands,
        factors,
        width,
        height,
        output_width,
        output_height,
        output,
        scratch,
        prescale,
        &mut || false,
    )
}

/// Cancel between reconstructed source rows; the caller discards partial output.
pub fn inverse_spatial_cancellable(
    bands: [&[i16]; 4],
    factors: [u16; 4],
    width: usize,
    height: usize,
    output_width: usize,
    output_height: usize,
    output: &mut [i16],
    scratch: &mut [i16],
    prescale: u8,
    cancelled: &mut impl FnMut() -> bool,
) -> Result<(), &'static str> {
    if !matches!(prescale, 0 | 2) {
        return Err("VC-5 spatial prescale");
    }
    let count = width.checked_mul(height).ok_or("VC-5 spatial extent")?;
    let target = output_width
        .checked_mul(output_height)
        .ok_or("VC-5 spatial output extent")?;
    let twice_width = width.checked_mul(2).ok_or("VC-5 spatial width")?;
    let twice_height = height.checked_mul(2).ok_or("VC-5 spatial height")?;
    if width < 3
        || height < 3
        || bands.iter().any(|b| b.len() != count)
        || ![twice_width, twice_width - 1].contains(&output_width)
        || ![twice_height, twice_height - 1].contains(&output_height)
        || output.len() != target
        || scratch.len() != width.checked_mul(4).ok_or("VC-5 scratch extent")?
    {
        return Err("VC-5 spatial geometry");
    }
    if bands[1..]
        .iter()
        .any(|b| b.iter().any(|v| v.unsigned_abs() > 255))
    {
        return Err("VC-5 magnitude exceeds codebook");
    }
    let (even_low, rest) = scratch.split_at_mut(width);
    let (even_high, rest) = rest.split_at_mut(width);
    let (odd_low, odd_high) = rest.split_at_mut(width);
    for y in 0..height {
        if cancelled() {
            return Err("VC-5 reconstruction cancelled");
        }
        let rows = if y == 0 {
            [0, 1, 2]
        } else if y == height - 1 {
            [y - 2, y - 1, y]
        } else {
            [y - 1, y, y + 1]
        };
        for x in 0..width {
            let filter = |low: [i32; 3], correction: i32| {
                let (even, odd) = if y == 0 {
                    (
                        (11 * low[0] - 4 * low[1] + low[2] + 4) >> 3,
                        (5 * low[0] + 4 * low[1] - low[2] + 4) >> 3,
                    )
                } else if y == height - 1 {
                    (
                        (5 * low[2] + 4 * low[1] - low[0] + 4) >> 3,
                        (11 * low[2] - 4 * low[1] + low[0] + 4) >> 3,
                    )
                } else {
                    (
                        low[1] + ((low[0] - low[2] + 4) >> 3),
                        low[1] + ((-low[0] + low[2] + 4) >> 3),
                    )
                };
                (
                    ((even + correction) >> 1).clamp(-32768, 32767) as i16,
                    ((odd - correction) >> 1).clamp(-32768, 32767) as i16,
                )
            };
            let low = rows.map(|row| i32::from(bands[0][row * width + x]));
            let high = i32::from(dequantize(i32::from(bands[2][y * width + x]), factors[2])?);
            (even_low[x], odd_low[x]) = filter(low, high);
            let mut low = [0; 3];
            for (i, row) in rows.into_iter().enumerate() {
                low[i] = i32::from(dequantize(i32::from(bands[1][row * width + x]), factors[1])?);
            }
            let high = i32::from(dequantize(i32::from(bands[3][y * width + x]), factors[3])?);
            (even_high[x], odd_high[x]) = filter(low, high);
        }
        let start = y * 2 * output_width;
        inverse_horizontal_prescaled(
            even_low,
            even_high,
            &mut output[start..start + output_width],
            prescale,
        )?;
        if y * 2 + 1 < output_height {
            inverse_horizontal_prescaled(
                odd_low,
                odd_high,
                &mut output[start + output_width..start + output_width * 2],
                prescale,
            )?;
        }
    }
    Ok(())
}

/// Admit output and scratch to the request's shared budget before reconstruction.
pub fn reconstruct_managed(
    bands: [&[i16]; 4],
    factors: [u16; 4],
    width: usize,
    height: usize,
    output_width: usize,
    output_height: usize,
    prescale: u8,
    request: &crate::DecodeRequest,
) -> Result<rrrah_core::SharedBuffer<i16>, crate::DecodeError> {
    request.check_cancelled()?;
    let count = output_width
        .checked_mul(output_height)
        .ok_or(crate::DecodeError::DimensionOverflow)?;
    let scratch_count = width
        .checked_mul(4)
        .ok_or(crate::DecodeError::DimensionOverflow)?;
    let budget = request
        .memory_budget
        .clone()
        .unwrap_or_else(|| rrrah_core::MemoryBudget::new(crate::raster::MAX_RASTER_BYTES));
    let mut output = budget.try_buffer(count, 0i16)?;
    let mut scratch = budget.try_buffer(scratch_count, 0i16)?;
    let result = inverse_spatial_cancellable(
        bands,
        factors,
        width,
        height,
        output_width,
        output_height,
        &mut output,
        &mut scratch,
        prescale,
        &mut || request.check_cancelled().is_err(),
    );
    if let Err(message) = result {
        request.check_cancelled()?;
        return Err(crate::DecodeError::NativeCamera {
            format: "VC-5",
            message: message.into(),
        });
    }
    request.check_cancelled()?;
    Ok(output.freeze())
}

/// Decode an entropy band into an admitted shared buffer; keep quantized values.
pub fn decode_highpass_managed(
    bytes: &[u8],
    coefficients: usize,
    request: &crate::DecodeRequest,
) -> Result<rrrah_core::SharedBuffer<i16>, crate::DecodeError> {
    request.check_cancelled()?;
    let budget = request
        .memory_budget
        .clone()
        .unwrap_or_else(|| rrrah_core::MemoryBudget::new(crate::raster::MAX_RASTER_BYTES));
    let mut output = budget.try_buffer(coefficients, 0i16)?;
    let mut offset = 0;
    for run in Vc5Runs::new(bytes, coefficients) {
        request.check_cancelled()?;
        let (count, value) = run.map_err(|message| crate::DecodeError::NativeCamera {
            format: "VC-5",
            message: message.into(),
        })?;
        output[offset..offset + count].fill(value as i16);
        offset += count;
    }
    request.check_cancelled()?;
    Ok(output.freeze())
}

/// Borrowed coefficient block with its stream-selected reconstruction parameters.
#[derive(Debug, Clone, Copy)]
pub struct Vc5Band<'a> {
    pub payload: &'a [u8],
    pub quantization: u16,
    pub lowpass_precision: u16,
}

/// Baseline four-channel, ten-band RAW representation. No pixels are decoded here.
#[derive(Debug)]
pub struct Vc5Index<'a> {
    pub width: u16,
    pub height: u16,
    pub packed_prescale: u16,
    pub bands: [[Vc5Band<'a>; 10]; 4],
}

/// Index baseline RAW sections without allocating or copying compressed blocks.
pub fn index_raw(bytes: &[u8]) -> Result<Vc5Index<'_>, &'static str> {
    #[derive(Default)]
    struct State {
        channel: usize,
        band: usize,
        quantization: u16,
        precision: u16,
        width: u16,
        height: u16,
        channels: u16,
        format: u16,
        pattern_width: u16,
        pattern_height: u16,
        components: u16,
        prescale: u16,
    }
    fn walk<'a>(
        records: Vc5Records<'a>,
        depth: usize,
        state: &mut State,
        bands: &mut [[Option<Vc5Band<'a>>; 10]; 4],
    ) -> Result<(), &'static str> {
        if depth > 4 {
            return Err("VC-5 section depth");
        }
        for record in records {
            let record = record?;
            if let Some(records) = record.section_records() {
                walk(records, depth + 1, state, bands)?;
            } else if record.tag & 0xff00 == 0x6000 {
                let slot = bands
                    .get_mut(state.channel)
                    .and_then(|c| c.get_mut(state.band))
                    .ok_or("VC-5 channel/subband index")?;
                if slot.is_some() {
                    return Err("duplicate VC-5 coefficient block");
                }
                *slot = Some(Vc5Band {
                    payload: record.payload.ok_or("VC-5 block payload")?,
                    quantization: state.quantization,
                    lowpass_precision: state.precision,
                });
            } else {
                match record.tag {
                    12 => state.channels = record.value,
                    20 => state.width = record.value,
                    21 => state.height = record.value,
                    84 => state.format = record.value,
                    106 => state.pattern_width = record.value,
                    107 => state.pattern_height = record.value,
                    108 => state.components = record.value,
                    109 => state.prescale = record.value,
                    62 => state.channel = usize::from(record.value),
                    48 => state.band = usize::from(record.value),
                    53 => state.quantization = record.value,
                    35 => state.precision = record.value,
                    102 if record.value <= 16 => {}
                    14 if record.value == 10 => {}
                    _ if record.optional => {}
                    _ => return Err("unsupported required VC-5 tag"),
                }
            }
        }
        Ok(())
    }
    let mut state = State::default();
    let mut bands = [[None; 10]; 4];
    walk(Vc5Records::new(bytes)?, 0, &mut state, &mut bands)?;
    if state.channels != 4
        || state.format != 4
        || state.pattern_width != 2
        || state.pattern_height != 2
        || state.components != 1
        || state.width == 0
        || state.height == 0
    {
        return Err("unsupported VC-5 RAW representation");
    }
    if bands.iter().flatten().any(Option::is_none) {
        return Err("missing VC-5 coefficient block");
    }
    let bands = bands.map(|c| c.map(|b| b.expect("checked complete band table")));
    for channel in &bands {
        if channel[0].lowpass_precision != 16 {
            return Err("unsupported VC-5 lowpass precision");
        }
        if channel[1..].iter().any(|b| b.quantization == 0) {
            return Err("invalid VC-5 highpass quantization");
        }
    }
    Ok(Vc5Index {
        width: state.width,
        height: state.height,
        packed_prescale: state.prescale,
        bands,
    })
}

/// Restore one baseline RAW component through all three wavelet levels.
/// Compressed source ownership is retained by the caller; each decoded band is managed.
pub fn decode_channel(
    index: &Vc5Index<'_>,
    channel: usize,
    request: &crate::DecodeRequest,
) -> Result<rrrah_core::SharedBuffer<i16>, crate::DecodeError> {
    let bad = |message: &'static str| crate::DecodeError::NativeCamera {
        format: "VC-5",
        message: message.into(),
    };
    request.check_cancelled()?;
    let bands = index
        .bands
        .get(channel)
        .ok_or_else(|| bad("VC-5 channel index"))?;
    let channel_width = usize::from(index.width).div_ceil(2);
    let channel_height = usize::from(index.height).div_ceil(2);
    let dimensions: [(usize, usize); 3] = std::array::from_fn(|level| {
        let divisor = 1usize << (3 - level);
        (channel_width.div_ceil(divisor), channel_height.div_ceil(divisor))
    });
    let budget = request
        .memory_budget
        .clone()
        .unwrap_or_else(|| rrrah_core::MemoryBudget::new(crate::raster::MAX_RASTER_BYTES));
    let mut managed_request = request.clone();
    managed_request.memory_budget = Some(budget.clone());
    let (width, height) = dimensions[0];
    let source = lowpass_16(bands[0].payload, width, height).map_err(bad)?;
    let mut low = budget.try_buffer(source.len(), 0i16)?;
    for (i, (out, value)) in low.iter_mut().zip(source).enumerate() {
        if i % 4096 == 0 {
            request.check_cancelled()?;
        }
        *out = value as i16;
    }
    let mut low = low.freeze();
    for level in 0..3 {
        request.check_cancelled()?;
        let (width, height) = dimensions[level];
        let first = if level == 0 { 1 } else { 1 + level * 3 };
        let count = width
            .checked_mul(height)
            .ok_or(crate::DecodeError::DimensionOverflow)?;
        let a = decode_highpass_managed(bands[first].payload, count, &managed_request)?;
        let b = decode_highpass_managed(bands[first + 1].payload, count, &managed_request)?;
        let c = decode_highpass_managed(bands[first + 2].payload, count, &managed_request)?;
        let (ow, oh) = if level == 2 {
            (channel_width, channel_height)
        } else {
            dimensions[level + 1]
        };
        let prescale = ((index.packed_prescale >> (14 - (2 - level) * 2)) & 3) as u8;
        let output = reconstruct_managed(
            [&low, &a, &b, &c],
            [
                1,
                bands[first].quantization,
                bands[first + 1].quantization,
                bands[first + 2].quantization,
            ],
            width,
            height,
            ow,
            oh,
            prescale,
            &managed_request,
        )?;
        low = output;
    }
    Ok(low)
}

static INVERSE_LOG: std::sync::LazyLock<[u16; 4096]> = std::sync::LazyLock::new(|| {
    std::array::from_fn(|code| {
        // The reference rounds the exponent's result to f32 before truncation.
        let value = (65535.0 * (113.0f64.powf(code as f64 / 4095.0) - 1.0) / 112.0) as f32;
        value as u16
    })
});

/// Inverse Protune log curve for the baseline 12-bit component representation.
pub fn inverse_log12(code: u16) -> Result<u16, &'static str> {
    INVERSE_LOG
        .get(usize::from(code))
        .copied()
        .ok_or("VC-5 log code exceeds 12 bits")
}

/// Convert one GS/RG/BG/GD component sample to R/G1/G2/B linear sensor codes.
pub fn components_to_bayer_cell(components: [i16; 4], output_bits: u8) -> Result<[u16; 4], &'static str> {
    if !matches!(output_bits, 12 | 14 | 16) {
        return Err("VC-5 sensor output precision");
    }
    let [gs, rg, bg, gd] = components.map(i32::from);
    let rg = rg - 2048;
    let bg = bg - 2048;
    let gd = gd - 2048;
    let log = [2 * rg + gs, gs + gd, gs - gd, 2 * bg + gs].map(|v| v.clamp(0, 4095) as u16);
    Ok(log.map(|v| INVERSE_LOG[usize::from(v)] >> (16 - output_bits)))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BayerOrder {
    Rggb,
    Gbrg,
}

/// Assemble admitted linear sensor storage from four baseline component arrays.
pub fn assemble_bayer(
    components: [&[i16]; 4],
    width: usize,
    height: usize,
    output_bits: u8,
    order: BayerOrder,
    output: &mut [u16],
    cancelled: &mut impl FnMut() -> bool,
) -> Result<(), &'static str> {
    let cw = width.div_ceil(2);
    let ch = height.div_ceil(2);
    let cells = cw.checked_mul(ch).ok_or("VC-5 Bayer extent")?;
    let count = width.checked_mul(height).ok_or("VC-5 Bayer extent")?;
    if width == 0
        || height == 0
        || output.len() != count
        || components.iter().any(|c| c.len() != cells)
        || !matches!(output_bits, 12 | 14 | 16)
    {
        return Err("VC-5 Bayer geometry or precision");
    }
    for y in 0..ch {
        if cancelled() {
            return Err("VC-5 Bayer assembly cancelled");
        }
        for x in 0..cw {
            let index = y * cw + x;
            let [r, g1, g2, b] = components_to_bayer_cell(components.map(|c| c[index]), output_bits)?;
            let values = match order {
                BayerOrder::Rggb => [r, g1, g2, b],
                BayerOrder::Gbrg => [g1, b, r, g2],
            };
            for dy in 0..2 {
                for dx in 0..2 {
                    let ox = x * 2 + dx;
                    let oy = y * 2 + dy;
                    if ox < width && oy < height {
                        output[oy * width + ox] = values[dy * 2 + dx];
                    }
                }
            }
        }
    }
    Ok(())
}

/// Decode baseline VC-5 to managed linear Bayer codes; DNG metadata admission is separate.
pub fn decode_bayer(
    index: &Vc5Index<'_>,
    output_bits: u8,
    order: BayerOrder,
    request: &crate::DecodeRequest,
) -> Result<rrrah_core::SharedBuffer<u16>, crate::DecodeError> {
    if !matches!(output_bits, 12 | 14 | 16) {
        return Err(crate::DecodeError::NativeCamera {
            format: "VC-5",
            message: "VC-5 sensor output precision".into(),
        });
    }
    request.check_cancelled()?;
    let budget = request
        .memory_budget
        .clone()
        .unwrap_or_else(|| rrrah_core::MemoryBudget::new(crate::raster::MAX_RASTER_BYTES));
    let mut managed_request = request.clone();
    managed_request.memory_budget = Some(budget.clone());
    let width = usize::from(index.width);
    let height = usize::from(index.height);
    let count = width
        .checked_mul(height)
        .ok_or(crate::DecodeError::DimensionOverflow)?;
    let mut output = budget.try_buffer(count, 0u16)?;
    let a = decode_channel(index, 0, &managed_request)?;
    let b = decode_channel(index, 1, &managed_request)?;
    let c = decode_channel(index, 2, &managed_request)?;
    let d = decode_channel(index, 3, &managed_request)?;
    if let Err(message) = assemble_bayer(
        [&a, &b, &c, &d],
        width,
        height,
        output_bits,
        order,
        &mut output,
        &mut || request.check_cancelled().is_err(),
    ) {
        request.check_cancelled()?;
        return Err(crate::DecodeError::NativeCamera {
            format: "VC-5",
            message: message.into(),
        });
    }
    request.check_cancelled()?;
    Ok(output.freeze())
}
