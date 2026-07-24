//! Assembly of independently encoded DNG lossless-JPEG strips and tiles.
//!
//! DNG permits a JPEG frame's internal geometry and component count to differ
//! from the enclosing TIFF segment geometry. The only required relationship is
//! that the decoded sample counts agree, so assembly deliberately treats the
//! JPEG output as one flat TIFF-order sample sequence.
//!
//! Segments are independent coded units (own Huffman tables, predictor reset
//! per segment), so they decode on the bounded worker set from
//! [`super::parallel`]; assembly into the mosaic is bit-identical for any
//! worker count.

use super::{DngError, DngImage, Storage, lossless_jpeg, parallel};

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
    precision: u8,
    rows_per_strip: u32,
    segments: &[super::Segment<'_>],
    cancelled: &(dyn Fn() -> bool + Sync),
    workers: usize,
) -> Result<(), DngError> {
    let width = usize::try_from(width).map_err(|_| DngError::ArithmeticOverflow("JPEG strip width"))?;
    let height = usize::try_from(height).map_err(|_| DngError::ArithmeticOverflow("JPEG strip height"))?;
    let rows_per_strip =
        usize::try_from(rows_per_strip).map_err(|_| DngError::ArithmeticOverflow("JPEG rows per strip"))?;

    let workers = parallel::effective_workers(workers, segments.len(), output.len());
    let units = parallel::split_row_bands(output, width, height, rows_per_strip);
    let process = |index: usize, unit: &mut [u16]| -> Result<(), DngError> {
        let first_row = index
            .checked_mul(rows_per_strip)
            .ok_or(DngError::ArithmeticOverflow("JPEG strip first row"))?;
        if cancelled() {
            return Err(DngError::Cancelled { row: first_row });
        }
        let rows = rows_per_strip.min(height.saturating_sub(first_row));
        let expected = width
            .checked_mul(rows)
            .ok_or(DngError::ArithmeticOverflow("JPEG strip sample count"))?;
        let decoded = decode_segment(segments[index].bytes, cancelled, first_row)?;
        validate_segment(index, precision, expected, &decoded)?;
        unit.copy_from_slice(&decoded.samples);
        Ok(())
    };
    parallel::run_units(units, workers, cancelled, &process)
}

#[allow(clippy::too_many_arguments)]
fn decode_tiles(
    output: &mut [u16],
    width: u32,
    height: u32,
    precision: u8,
    tile_width: u32,
    tile_height: u32,
    segments: &[super::Segment<'_>],
    cancelled: &(dyn Fn() -> bool + Sync),
    workers: usize,
) -> Result<(), DngError> {
    let width = usize::try_from(width).map_err(|_| DngError::ArithmeticOverflow("JPEG tile image width"))?;
    let height =
        usize::try_from(height).map_err(|_| DngError::ArithmeticOverflow("JPEG tile image height"))?;
    let tile_width =
        usize::try_from(tile_width).map_err(|_| DngError::ArithmeticOverflow("JPEG tile width"))?;
    let tile_height =
        usize::try_from(tile_height).map_err(|_| DngError::ArithmeticOverflow("JPEG tile height"))?;
    let tile_columns = width.div_ceil(tile_width);
    let tile_rows = height.div_ceil(tile_height);
    let expected = tile_width
        .checked_mul(tile_height)
        .ok_or(DngError::ArithmeticOverflow("JPEG tile sample count"))?;

    // One parallel unit per tile row: bands are contiguous output regions,
    // tiles inside a band are decoded sequentially in index order.
    let workers = parallel::effective_workers(workers, tile_rows, output.len());
    let units = parallel::split_row_bands(output, width, height, tile_height);
    let process = |band: usize, unit: &mut [u16]| -> Result<(), DngError> {
        let first_y = band
            .checked_mul(tile_height)
            .ok_or(DngError::ArithmeticOverflow("JPEG tile top"))?;
        let first_tile = band
            .checked_mul(tile_columns)
            .ok_or(DngError::ArithmeticOverflow("JPEG tile band"))?;
        let last_tile = first_tile.saturating_add(tile_columns).min(segments.len());
        for (offset, segment) in segments[first_tile..last_tile].iter().enumerate() {
            let index = first_tile + offset;
            let tile_x = index % tile_columns;
            let first_x = tile_x
                .checked_mul(tile_width)
                .ok_or(DngError::ArithmeticOverflow("JPEG tile left"))?;
            if cancelled() {
                return Err(DngError::Cancelled { row: first_y });
            }
            let decoded = decode_segment(segment.bytes, cancelled, first_y)?;
            validate_segment(index, precision, expected, &decoded)?;

            let copy_width = tile_width.min(width.saturating_sub(first_x));
            let copy_height = tile_height.min(height.saturating_sub(first_y));
            for row in 0..copy_height {
                if cancelled() {
                    return Err(DngError::Cancelled { row: first_y + row });
                }
                let source_start = row
                    .checked_mul(tile_width)
                    .ok_or(DngError::ArithmeticOverflow("JPEG tile source row"))?;
                let target_start = row
                    .checked_mul(width)
                    .and_then(|offset| offset.checked_add(first_x))
                    .ok_or(DngError::ArithmeticOverflow("JPEG tile output row"))?;
                unit[target_start..target_start + copy_width]
                    .copy_from_slice(&decoded.samples[source_start..source_start + copy_width]);
            }
        }
        Ok(())
    };
    parallel::run_units(units, workers, cancelled, &process)
}

fn decode_segment(
    bytes: &[u8],
    cancelled: &dyn Fn() -> bool,
    first_row: usize,
) -> Result<lossless_jpeg::LosslessJpegImage, DngError> {
    match lossless_jpeg::decode(bytes, cancelled) {
        Ok(decoded) => Ok(decoded),
        Err(lossless_jpeg::LosslessJpegError::Cancelled { row }) => Err(DngError::Cancelled {
            row: first_row.saturating_add(row),
        }),
        Err(error) => Err(error.into()),
    }
}

fn validate_segment(
    index: usize,
    expected_precision: u8,
    expected_samples: usize,
    decoded: &lossless_jpeg::LosslessJpegImage,
) -> Result<(), DngError> {
    if decoded.precision != expected_precision {
        return Err(DngError::LosslessJpegPrecision {
            index,
            expected: expected_precision,
            actual: decoded.precision,
        });
    }
    if decoded.samples.len() != expected_samples {
        return Err(DngError::LosslessJpegSampleCount {
            index,
            expected: expected_samples,
            actual: decoded.samples.len(),
            jpeg_width: decoded.width,
            jpeg_height: decoded.height,
            components: decoded.component_ids.len(),
        });
    }
    Ok(())
}
