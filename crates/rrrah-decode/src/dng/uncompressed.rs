use super::{ByteOrder, DngError, DngImage, Storage, parallel};

pub(super) fn decode(
    image: &DngImage<'_>,
    cancelled: &(dyn Fn() -> bool + Sync),
    workers: usize,
) -> Result<Vec<u16>, DngError> {
    let mut pixels = Vec::new();
    pixels
        .try_reserve_exact(image.sample_count)
        .map_err(|_| DngError::AllocationFailed {
            elements: image.sample_count,
        })?;
    pixels.resize(image.sample_count, 0);

    match &image.storage {
        Storage::Strips {
            rows_per_strip,
            segments,
        } => decode_strips(
            &mut pixels,
            image.width,
            image.height,
            image.stored_bits_per_sample,
            image.byte_order,
            *rows_per_strip,
            segments,
            cancelled,
            workers,
        )?,
        Storage::Tiles {
            tile_width,
            tile_height,
            segments,
        } => decode_tiles(
            &mut pixels,
            image.width,
            image.height,
            image.stored_bits_per_sample,
            image.byte_order,
            *tile_width,
            *tile_height,
            segments,
            cancelled,
            workers,
        )?,
    }
    Ok(pixels)
}

#[allow(clippy::too_many_arguments)]
fn decode_strips(
    output: &mut [u16],
    width: u32,
    height: u32,
    bits_per_sample: u8,
    byte_order: ByteOrder,
    rows_per_strip: u32,
    segments: &[super::Segment<'_>],
    cancelled: &(dyn Fn() -> bool + Sync),
    workers: usize,
) -> Result<(), DngError> {
    let width = usize::try_from(width).map_err(|_| DngError::ArithmeticOverflow("strip width"))?;
    let height = usize::try_from(height).map_err(|_| DngError::ArithmeticOverflow("strip height"))?;
    let rows_per_strip =
        usize::try_from(rows_per_strip).map_err(|_| DngError::ArithmeticOverflow("rows per strip"))?;
    let row_bytes = packed_row_bytes(width, bits_per_sample)?;

    let workers = parallel::effective_workers(workers, segments.len(), output.len());
    let units = parallel::split_row_bands(output, width, height, rows_per_strip);
    let process = |index: usize, unit: &mut [u16]| -> Result<(), DngError> {
        let first_row = index
            .checked_mul(rows_per_strip)
            .ok_or(DngError::ArithmeticOverflow("strip first row"))?;
        if cancelled() {
            return Err(DngError::Cancelled { row: first_row });
        }
        let rows = rows_per_strip.min(height.saturating_sub(first_row));
        let expected = row_bytes
            .checked_mul(rows)
            .ok_or(DngError::ArithmeticOverflow("strip byte length"))?;
        let segment = &segments[index];
        if segment.bytes.len() != expected {
            return Err(DngError::SegmentLength {
                index,
                expected,
                actual: segment.bytes.len(),
            });
        }
        for row in 0..rows {
            if cancelled() {
                return Err(DngError::Cancelled { row: first_row + row });
            }
            let source_start = row * row_bytes;
            decode_row(
                &segment.bytes[source_start..source_start + row_bytes],
                &mut unit[row * width..(row + 1) * width],
                bits_per_sample,
                byte_order,
            )?;
        }
        Ok(())
    };
    parallel::run_units(units, workers, cancelled, &process)
}

#[allow(clippy::too_many_arguments)]
fn decode_tiles(
    output: &mut [u16],
    width: u32,
    height: u32,
    bits_per_sample: u8,
    byte_order: ByteOrder,
    tile_width: u32,
    tile_height: u32,
    segments: &[super::Segment<'_>],
    cancelled: &(dyn Fn() -> bool + Sync),
    workers: usize,
) -> Result<(), DngError> {
    let width = usize::try_from(width).map_err(|_| DngError::ArithmeticOverflow("tile image width"))?;
    let height = usize::try_from(height).map_err(|_| DngError::ArithmeticOverflow("tile image height"))?;
    let tile_width = usize::try_from(tile_width).map_err(|_| DngError::ArithmeticOverflow("tile width"))?;
    let tile_height =
        usize::try_from(tile_height).map_err(|_| DngError::ArithmeticOverflow("tile height"))?;
    let tile_columns = width.div_ceil(tile_width);
    let tile_rows = height.div_ceil(tile_height);
    let row_bytes = packed_row_bytes(tile_width, bits_per_sample)?;
    let expected = row_bytes
        .checked_mul(tile_height)
        .ok_or(DngError::ArithmeticOverflow("tile byte length"))?;

    // One parallel unit per tile row: bands are contiguous output regions,
    // tiles inside a band are decoded sequentially in index order.
    let workers = parallel::effective_workers(workers, tile_rows, output.len());
    let units = parallel::split_row_bands(output, width, height, tile_height);
    let process = |band: usize, unit: &mut [u16]| -> Result<(), DngError> {
        let first_y = band
            .checked_mul(tile_height)
            .ok_or(DngError::ArithmeticOverflow("tile top"))?;
        let first_tile = band
            .checked_mul(tile_columns)
            .ok_or(DngError::ArithmeticOverflow("tile band"))?;
        let last_tile = first_tile.saturating_add(tile_columns).min(segments.len());
        let mut decoded_row = Vec::new();
        decoded_row
            .try_reserve_exact(tile_width)
            .map_err(|_| DngError::AllocationFailed { elements: tile_width })?;
        decoded_row.resize(tile_width, 0);
        for (offset, segment) in segments[first_tile..last_tile].iter().enumerate() {
            let index = first_tile + offset;
            if cancelled() {
                return Err(DngError::Cancelled { row: first_y });
            }
            if segment.bytes.len() != expected {
                return Err(DngError::SegmentLength {
                    index,
                    expected,
                    actual: segment.bytes.len(),
                });
            }
            let tile_x = index % tile_columns;
            let first_x = tile_x * tile_width;
            let copy_width = tile_width.min(width.saturating_sub(first_x));
            let copy_height = tile_height.min(height.saturating_sub(first_y));
            for row in 0..copy_height {
                if cancelled() {
                    return Err(DngError::Cancelled { row: first_y + row });
                }
                let source_start = row * row_bytes;
                decode_row(
                    &segment.bytes[source_start..source_start + row_bytes],
                    &mut decoded_row,
                    bits_per_sample,
                    byte_order,
                )?;
                let target_start = row * width + first_x;
                unit[target_start..target_start + copy_width]
                    .copy_from_slice(&decoded_row[..copy_width]);
            }
        }
        Ok(())
    };
    parallel::run_units(units, workers, cancelled, &process)
}

fn decode_row(
    encoded: &[u8],
    output: &mut [u16],
    bits_per_sample: u8,
    byte_order: ByteOrder,
) -> Result<(), DngError> {
    let expected = packed_row_bytes(output.len(), bits_per_sample)?;
    if encoded.len() != expected {
        return Err(DngError::TruncatedPackedRow {
            expected,
            actual: encoded.len(),
        });
    }
    match bits_per_sample {
        8 => {
            for (target, source) in output.iter_mut().zip(encoded) {
                *target = u16::from(*source);
            }
        }
        16 => {
            for (target, bytes) in output.iter_mut().zip(encoded.chunks_exact(2)) {
                *target = byte_order.u16(bytes);
            }
        }
        9..=15 => decode_msb_packed(encoded, output, bits_per_sample)?,
        _ => {
            return Err(DngError::UnsupportedBitsPerSample {
                actual: u64::from(bits_per_sample),
            });
        }
    }
    Ok(())
}

pub(crate) fn decode_msb_packed(
    encoded: &[u8],
    output: &mut [u16],
    bits_per_sample: u8,
) -> Result<(), DngError> {
    let mut reservoir = 0_u64;
    let mut reservoir_bits = 0_u8;
    let mut next_byte = 0_usize;
    let mask = (1_u64 << bits_per_sample) - 1;
    for target in output {
        while reservoir_bits < bits_per_sample {
            // Word-wise refill: six bytes per step via one fixed-size 64-bit
            // load. `reservoir_bits < bits_per_sample <= 15` here, so the
            // 48-bit append always fits below the 64-bit limit.
            if let Some(window) = encoded.get(next_byte..).and_then(|tail| tail.get(..8)) {
                let word = u64::from_be_bytes(window.try_into().expect("an eight-byte window"));
                reservoir = (reservoir << 48) | (word >> 16);
                reservoir_bits += 48;
                next_byte += 6;
            } else {
                let byte = encoded
                    .get(next_byte)
                    .copied()
                    .ok_or(DngError::TruncatedPackedRow {
                        expected: next_byte + 1,
                        actual: encoded.len(),
                    })?;
                reservoir = (reservoir << 8) | u64::from(byte);
                reservoir_bits += 8;
                next_byte += 1;
            }
        }
        let shift = reservoir_bits - bits_per_sample;
        let value = (reservoir >> shift) & mask;
        *target = u16::try_from(value).map_err(|_| DngError::ArithmeticOverflow("packed DNG sample"))?;
        reservoir_bits = shift;
        reservoir &= if shift == 0 { 0 } else { (1_u64 << shift) - 1 };
    }
    Ok(())
}

/// Byte-at-a-time reference implementation kept so benchmarks can A/B the
/// word-wise refill in [`decode_msb_packed`] against the original loop inside
/// one process. Bit-identical to it by construction.
#[doc(hidden)]
pub(crate) fn decode_msb_packed_bytewise(
    encoded: &[u8],
    output: &mut [u16],
    bits_per_sample: u8,
) -> Result<(), DngError> {
    let mut reservoir = 0_u64;
    let mut reservoir_bits = 0_u8;
    let mut next_byte = 0_usize;
    let mask = (1_u64 << bits_per_sample) - 1;
    for target in output {
        while reservoir_bits < bits_per_sample {
            let byte = encoded
                .get(next_byte)
                .copied()
                .ok_or(DngError::TruncatedPackedRow {
                    expected: next_byte + 1,
                    actual: encoded.len(),
                })?;
            reservoir = (reservoir << 8) | u64::from(byte);
            reservoir_bits += 8;
            next_byte += 1;
        }
        let shift = reservoir_bits - bits_per_sample;
        let value = (reservoir >> shift) & mask;
        *target = u16::try_from(value).map_err(|_| DngError::ArithmeticOverflow("packed DNG sample"))?;
        reservoir_bits = shift;
        reservoir &= if shift == 0 { 0 } else { (1_u64 << shift) - 1 };
    }
    Ok(())
}

fn packed_row_bytes(width: usize, bits_per_sample: u8) -> Result<usize, DngError> {
    width
        .checked_mul(usize::from(bits_per_sample))
        .and_then(|bits| bits.checked_add(7))
        .map(|bits| bits / 8)
        .ok_or(DngError::ArithmeticOverflow("packed DNG row byte length"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dng::{Compression, DngMetadata};

    #[test]
    fn word_refill_matches_bytewise_across_widths_and_depths() {
        // Odd widths make rows end mid-word and vary the reservoir state at
        // refill boundaries; pseudo-random samples exercise all bit depths.
        let mut state = 0x9e37_79b9_7f4a_7c15_u64;
        let mut next = move || {
            state ^= state >> 12;
            state ^= state << 25;
            state ^= state >> 27;
            state.wrapping_mul(0x2545_f491_4f6c_dd1d)
        };
        for bits_per_sample in 9_u8..=15 {
            for width in [1_usize, 2, 3, 7, 37, 256] {
                let mask = (1_u16 << bits_per_sample) - 1;
                let samples = (0..width)
                    .map(|_| u16::try_from(next() & u64::from(mask)).unwrap())
                    .collect::<Vec<_>>();
                let row_bytes = packed_row_bytes(width, bits_per_sample).unwrap();
                let mut encoded = vec![0_u8; row_bytes];
                let mut bit_position = 0_usize;
                for &sample in &samples {
                    for shift in (0..bits_per_sample).rev() {
                        if (sample >> shift) & 1 == 1 {
                            encoded[bit_position / 8] |= 1 << (7 - (bit_position % 8));
                        }
                        bit_position += 1;
                    }
                }
                let mut word = vec![0_u16; width];
                let mut bytewise = vec![0_u16; width];
                decode_msb_packed(&encoded, &mut word, bits_per_sample).unwrap();
                decode_msb_packed_bytewise(&encoded, &mut bytewise, bits_per_sample).unwrap();
                assert_eq!(word, samples, "{bits_per_sample}-bit width {width}");
                assert_eq!(bytewise, samples, "{bits_per_sample}-bit width {width}");
            }
        }
    }

    #[test]
    fn decodes_row_aligned_twelve_bit_strips() {
        let encoded = [
            0x00, 0x1a, 0xbc, 0x00, 0x30, // 1, abc, 3 plus row padding.
            0x00, 0x40, 0x05, 0xff, 0xf0, // 4, 5, fff plus row padding.
        ];
        let image = image(
            3,
            2,
            12,
            ByteOrder::Little,
            Storage::Strips {
                rows_per_strip: 2,
                segments: vec![super::super::Segment {
                    offset: 0,
                    bytes: &encoded,
                }],
            },
            None,
        );
        assert_eq!(decode(&image, &|| false, 1).unwrap(), [1, 0xabc, 3, 4, 5, 0xfff]);
    }

    #[test]
    fn decodes_big_endian_sixteen_bit_edge_tiles() {
        let left = [
            0, 1, 0, 2, // first row
            0, 4, 0, 5, // second row
        ];
        let right = [
            0, 3, 0xaa, 0xaa, // padded right sample
            0, 6, 0xbb, 0xbb,
        ];
        let image = image(
            3,
            2,
            16,
            ByteOrder::Big,
            Storage::Tiles {
                tile_width: 2,
                tile_height: 2,
                segments: vec![
                    super::super::Segment {
                        offset: 0,
                        bytes: &left,
                    },
                    super::super::Segment {
                        offset: 8,
                        bytes: &right,
                    },
                ],
            },
            None,
        );
        assert_eq!(decode(&image, &|| false, 1).unwrap(), [1, 2, 3, 4, 5, 6]);
    }

    #[test]
    fn applies_linearization_after_unpacking() {
        let encoded = [0_u8, 1, 2, 3];
        let mut table = (0..256_u16).collect::<Vec<_>>();
        table[0] = 100;
        table[1] = 200;
        table[2] = 300;
        table[3] = 400;
        let image = image(
            4,
            1,
            8,
            ByteOrder::Little,
            Storage::Strips {
                rows_per_strip: 1,
                segments: vec![super::super::Segment {
                    offset: 0,
                    bytes: &encoded,
                }],
            },
            Some(table),
        );
        let mut decoded = decode(&image, &|| false, 1).unwrap();
        for sample in &mut decoded {
            *sample = image.metadata.linearization_table.as_ref().unwrap()[usize::from(*sample)];
        }
        assert_eq!(decoded, [100, 200, 300, 400]);
    }

    fn image(
        width: u32,
        height: u32,
        bits: u8,
        byte_order: ByteOrder,
        storage: Storage<'_>,
        table: Option<Vec<u16>>,
    ) -> DngImage<'_> {
        let sample_count = usize::try_from(width * height).unwrap();
        DngImage {
            byte_order,
            width,
            height,
            sample_count,
            stored_bits_per_sample: bits,
            output_bits_per_sample: 16,
            compression: Compression::Uncompressed,
            parse_timings: super::super::DngParseTimings::default(),
            metadata: DngMetadata {
                dng_version: [1, 7, 1, 0],
                backward_version: [1, 1, 0, 0],
                make: "Test".to_owned(),
                model: "test".to_owned(),
                camera_model: "test".to_owned(),
                orientation: super::super::Orientation::Normal,
                cfa: super::super::CfaPattern {
                    rows: 2,
                    columns: 2,
                    cells: vec![
                        super::super::CfaColor::Red,
                        super::super::CfaColor::Green,
                        super::super::CfaColor::Green,
                        super::super::CfaColor::Blue,
                    ],
                    plane_colors: vec![
                        super::super::CfaColor::Red,
                        super::super::CfaColor::Green,
                        super::super::CfaColor::Blue,
                    ],
                },
                black_level: super::super::BlackLevel {
                    repeat_rows: 1,
                    repeat_columns: 1,
                    values: vec![0.0],
                    delta_horizontal: Vec::new(),
                    delta_vertical: Vec::new(),
                },
                white_level: vec![u16::MAX],
                active_area: super::super::Rect {
                    top: 0,
                    left: 0,
                    bottom: height,
                    right: width,
                },
                crop: super::super::Crop {
                    origin_x: 0.0,
                    origin_y: 0.0,
                    width: f64::from(width),
                    height: f64::from(height),
                },
                linearization_table: table,
                color_matrix_1: None,
                color_matrix_2: None,
                calibration_illuminant_1: None,
                calibration_illuminant_2: None,
                as_shot_neutral: None,
            },
            storage,
        }
    }
}
