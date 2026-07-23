use super::{ByteOrder, DngError, DngImage, Storage};

pub(super) fn decode(image: &DngImage<'_>, cancelled: &dyn Fn() -> bool) -> Result<Vec<u16>, DngError> {
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
    cancelled: &dyn Fn() -> bool,
) -> Result<(), DngError> {
    let width = usize::try_from(width).map_err(|_| DngError::ArithmeticOverflow("strip width"))?;
    let height = usize::try_from(height).map_err(|_| DngError::ArithmeticOverflow("strip height"))?;
    let rows_per_strip =
        usize::try_from(rows_per_strip).map_err(|_| DngError::ArithmeticOverflow("rows per strip"))?;
    let row_bytes = packed_row_bytes(width, bits_per_sample)?;

    for (index, segment) in segments.iter().enumerate() {
        if cancelled() {
            return Err(DngError::Cancelled {
                row: index.saturating_mul(rows_per_strip),
            });
        }
        let first_row = index
            .checked_mul(rows_per_strip)
            .ok_or(DngError::ArithmeticOverflow("strip first row"))?;
        let rows = rows_per_strip.min(height.saturating_sub(first_row));
        let expected = row_bytes
            .checked_mul(rows)
            .ok_or(DngError::ArithmeticOverflow("strip byte length"))?;
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
            let target_start = (first_row + row) * width;
            decode_row(
                &segment.bytes[source_start..source_start + row_bytes],
                &mut output[target_start..target_start + width],
                bits_per_sample,
                byte_order,
            )?;
        }
    }
    Ok(())
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
    cancelled: &dyn Fn() -> bool,
) -> Result<(), DngError> {
    let width = usize::try_from(width).map_err(|_| DngError::ArithmeticOverflow("tile image width"))?;
    let height = usize::try_from(height).map_err(|_| DngError::ArithmeticOverflow("tile image height"))?;
    let tile_width = usize::try_from(tile_width).map_err(|_| DngError::ArithmeticOverflow("tile width"))?;
    let tile_height =
        usize::try_from(tile_height).map_err(|_| DngError::ArithmeticOverflow("tile height"))?;
    let tile_columns = width.div_ceil(tile_width);
    let row_bytes = packed_row_bytes(tile_width, bits_per_sample)?;
    let expected = row_bytes
        .checked_mul(tile_height)
        .ok_or(DngError::ArithmeticOverflow("tile byte length"))?;
    let mut decoded_row = Vec::new();
    decoded_row
        .try_reserve_exact(tile_width)
        .map_err(|_| DngError::AllocationFailed { elements: tile_width })?;
    decoded_row.resize(tile_width, 0);

    for (index, segment) in segments.iter().enumerate() {
        if cancelled() {
            return Err(DngError::Cancelled {
                row: (index / tile_columns).saturating_mul(tile_height),
            });
        }
        if segment.bytes.len() != expected {
            return Err(DngError::SegmentLength {
                index,
                expected,
                actual: segment.bytes.len(),
            });
        }
        let tile_y = index / tile_columns;
        let tile_x = index % tile_columns;
        let first_y = tile_y * tile_height;
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
            let target_start = (first_y + row) * width + first_x;
            output[target_start..target_start + copy_width].copy_from_slice(&decoded_row[..copy_width]);
        }
    }
    Ok(())
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

fn decode_msb_packed(encoded: &[u8], output: &mut [u16], bits_per_sample: u8) -> Result<(), DngError> {
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
        assert_eq!(decode(&image, &|| false).unwrap(), [1, 0xabc, 3, 4, 5, 0xfff]);
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
        assert_eq!(decode(&image, &|| false).unwrap(), [1, 2, 3, 4, 5, 6]);
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
        let mut decoded = decode(&image, &|| false).unwrap();
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
                as_shot_neutral: None,
            },
            storage,
        }
    }
}
