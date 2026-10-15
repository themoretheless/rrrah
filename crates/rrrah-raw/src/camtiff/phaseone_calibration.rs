//! Native integer Phase One black and flat-field stages, before demosaicing.
use super::camera_error;
use crate::DecodeError;
fn error(message: &str) -> DecodeError {
    camera_error("IIQ", message)
}
pub(super) fn subtract_black(
    samples: &mut [u16],
    width: usize,
    black: u16,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), DecodeError> {
    if width == 0 || !samples.len().is_multiple_of(width) {
        return Err(error("invalid black-stage geometry"));
    }
    for row in samples.chunks_exact_mut(width) {
        if cancelled() {
            return Err(DecodeError::Cancelled);
        }
        for value in row {
            *value = value.saturating_sub(black);
        }
    }
    Ok(())
}
/// Integer gain grids: one luma coefficient or separate red/blue coefficients.
/// This subset uses the qualified full-sensor GBRG pattern.
pub(super) fn flat_field(
    samples: &mut [u16],
    width: usize,
    height: usize,
    bytes: &[u8],
    channels: usize,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), DecodeError> {
    match channels {
        2 => flat_field_impl::<2>(samples, width, height, bytes, cancelled),
        4 => flat_field_impl::<4>(samples, width, height, bytes, cancelled),
        _ => Err(error("invalid flat-field geometry")),
    }
}
fn flat_field_impl<const CHANNELS: usize>(
    samples: &mut [u16],
    width: usize,
    height: usize,
    bytes: &[u8],
    cancelled: &dyn Fn() -> bool,
) -> Result<(), DecodeError> {
    if !matches!(CHANNELS, 2 | 4)
        || width.checked_mul(height) != Some(samples.len())
        || width == 0
        || height == 0
    {
        return Err(error("invalid flat-field geometry"));
    }
    let header = bytes
        .get(..16)
        .ok_or_else(|| error("truncated flat-field header"))?;
    let mut h = [0usize; 8];
    for (v, b) in h.iter_mut().zip(header.chunks_exact(2)) {
        *v = u16::from_le_bytes(b.try_into().unwrap()) as usize;
    }
    if h[2] == 0
        || h[3] == 0
        || h[4] == 0
        || h[5] == 0
        || h[4] > h[2]
        || h[5] > h[3]
        || h[6] != 0
        || h[7] != 0
    {
        return Err(error("invalid flat-field grid"));
    }
    let columns = h[2].div_ceil(h[4]);
    let rows = h[3].div_ceil(h[5]);
    let values = columns
        .checked_mul(rows)
        .and_then(|n| n.checked_mul(CHANNELS / 2))
        .ok_or(DecodeError::DimensionOverflow)?;
    if values > 1024 * 1024 || bytes.len() != 16 + values * 2 {
        return Err(error("flat-field length mismatch"));
    }
    if cancelled() {
        return Err(DecodeError::Cancelled);
    }
    let mut state = vec![0f32; CHANNELS * columns];
    let mut position = 16;
    for gy in 0..rows {
        if cancelled() {
            return Err(DecodeError::Cancelled);
        }
        for x in 0..columns {
            for c in (0..CHANNELS).step_by(2) {
                let gain =
                    u16::from_le_bytes(bytes[position..position + 2].try_into().unwrap()) as f32 / 32768.;
                position += 2;
                let index = c * columns + x;
                if gy == 0 {
                    state[index] = gain;
                } else {
                    state[index + columns] = (gain - state[index]) / h[5] as f32;
                }
            }
        }
        if gy == 0 {
            continue;
        }
        let y0 = h[1] + (gy - 1) * h[5];
        let y1 = (h[1] + gy * h[5]).min(height).min(h[1] + h[3] - h[5]);
        for y in y0..y1 {
            if cancelled() {
                return Err(DecodeError::Cancelled);
            }
            for gx in 1..columns {
                let mut gain = [0f32; 4];
                for c in (0..CHANNELS).step_by(2) {
                    gain[c] = state[c * columns + gx - 1];
                    gain[c + 1] = (state[c * columns + gx] - gain[c]) / h[4] as f32;
                }
                let x0 = h[0] + (gx - 1) * h[4];
                let x1 = (h[0] + gx * h[4]).min(width).min(h[0] + h[2] - h[4]);
                for x in x0..x1 {
                    let c = if CHANNELS == 2 {
                        0
                    } else {
                        match (y & 1, x & 1) {
                            (0, 1) => 2,
                            (1, 0) => 0,
                            _ => 1,
                        }
                    };
                    if c % 2 == 0 {
                        let i = y * width + x;
                        samples[i] = (samples[i] as f32 * gain[c]).clamp(0., 65535.) as u16;
                    }
                    for c in (0..CHANNELS).step_by(2) {
                        gain[c] += gain[c + 1];
                    }
                }
            }
            for x in 0..columns {
                for c in (0..CHANNELS).step_by(2) {
                    let i = c * columns + x;
                    state[i] += state[i + columns];
                }
            }
        }
    }
    Ok(())
}
#[cfg(test)]
fn flat_field_baseline(
    samples: &mut [u16],
    width: usize,
    height: usize,
    bytes: &[u8],
    channels: usize,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), DecodeError> {
    if !matches!(channels, 2 | 4)
        || width.checked_mul(height) != Some(samples.len())
        || width == 0
        || height == 0
    {
        return Err(error("invalid flat-field geometry"));
    }
    let header = bytes
        .get(..16)
        .ok_or_else(|| error("truncated flat-field header"))?;
    let mut h = [0usize; 8];
    for (v, b) in h.iter_mut().zip(header.chunks_exact(2)) {
        *v = u16::from_le_bytes(b.try_into().unwrap()) as usize;
    }
    if h[2] == 0
        || h[3] == 0
        || h[4] == 0
        || h[5] == 0
        || h[4] > h[2]
        || h[5] > h[3]
        || h[6] != 0
        || h[7] != 0
    {
        return Err(error("invalid flat-field grid"));
    }
    let columns = h[2].div_ceil(h[4]);
    let rows = h[3].div_ceil(h[5]);
    let values = columns
        .checked_mul(rows)
        .and_then(|n| n.checked_mul(channels / 2))
        .ok_or(DecodeError::DimensionOverflow)?;
    if values > 1024 * 1024 || bytes.len() != 16 + values * 2 {
        return Err(error("flat-field length mismatch"));
    }
    if cancelled() {
        return Err(DecodeError::Cancelled);
    }
    let mut state = vec![0f32; channels * columns];
    let mut position = 16;
    for gy in 0..rows {
        if cancelled() {
            return Err(DecodeError::Cancelled);
        }
        for x in 0..columns {
            for c in (0..channels).step_by(2) {
                let gain =
                    u16::from_le_bytes(bytes[position..position + 2].try_into().unwrap()) as f32 / 32768.;
                position += 2;
                let index = c * columns + x;
                if gy == 0 {
                    state[index] = gain;
                } else {
                    state[index + columns] = (gain - state[index]) / h[5] as f32;
                }
            }
        }
        if gy == 0 {
            continue;
        }
        let y0 = h[1] + (gy - 1) * h[5];
        let y1 = (h[1] + gy * h[5]).min(height).min(h[1] + h[3] - h[5]);
        for y in y0..y1 {
            if cancelled() {
                return Err(DecodeError::Cancelled);
            }
            for gx in 1..columns {
                let mut gain = [0f32; 4];
                for c in (0..channels).step_by(2) {
                    gain[c] = state[c * columns + gx - 1];
                    gain[c + 1] = (state[c * columns + gx] - gain[c]) / h[4] as f32;
                }
                let x0 = h[0] + (gx - 1) * h[4];
                let x1 = (h[0] + gx * h[4]).min(width).min(h[0] + h[2] - h[4]);
                for x in x0..x1 {
                    let c = if channels == 2 {
                        0
                    } else {
                        match (y & 1, x & 1) {
                            (0, 1) => 2,
                            (1, 0) => 0,
                            _ => 1,
                        }
                    };
                    if c % 2 == 0 {
                        let i = y * width + x;
                        samples[i] = (samples[i] as f32 * gain[c]).clamp(0., 65535.) as u16;
                    }
                    for c in (0..channels).step_by(2) {
                        gain[c] += gain[c + 1];
                    }
                }
            }
            for x in 0..columns {
                for c in (0..channels).step_by(2) {
                    let i = c * columns + x;
                    state[i] += state[i + columns];
                }
            }
        }
    }
    Ok(())
}
/// Repair individual defects before flat fields. Column defects are handled later.
pub(super) fn repair_pixels(
    samples: &mut [u16],
    width: usize,
    height: usize,
    defects: &[u8],
    cancelled: &dyn Fn() -> bool,
) -> Result<(), DecodeError> {
    if width.checked_mul(height) != Some(samples.len())
        || width == 0
        || height == 0
        || !defects.len().is_multiple_of(8)
        || defects.len() > 65536 * 8
    {
        return Err(error("invalid defect table geometry"));
    }
    for record in defects.chunks_exact(8) {
        if cancelled() {
            return Err(DecodeError::Cancelled);
        }
        let word = |i| u16::from_le_bytes([record[i], record[i + 1]]) as usize;
        let (x, y, kind) = (word(0), word(2), word(4));
        if word(6) != 0 || !matches!(kind, 129 | 131 | 137) {
            return Err(error("unqualified defect record"));
        }
        if x >= width || (kind == 129 && y >= height) {
            return Err(error("defect outside sensor"));
        }
        if kind != 129 {
            continue;
        }
        let offsets: [(isize, isize); 8] = if y % 2 == 1 && x % 2 == 1 {
            [
                (-1, -1),
                (-1, 1),
                (1, -1),
                (1, 1),
                (-2, 0),
                (0, -2),
                (0, 2),
                (2, 0),
            ]
        } else {
            [
                (-2, 0),
                (0, -2),
                (0, 2),
                (2, 0),
                (-2, -2),
                (-2, 2),
                (2, -2),
                (2, 2),
            ]
        };
        let mut sum = 0u32;
        let mut count = 0u32;
        for (dy, dx) in offsets {
            if let (Some(row), Some(column)) = (y.checked_add_signed(dy), x.checked_add_signed(dx)) {
                if row < height && column < width {
                    sum += u32::from(samples[row * width + column]);
                    count += 1;
                }
            }
        }
        if count == 0 {
            return Err(error("defect has no valid neighbours"));
        }
        samples[y * width + x] = ((sum + count / 2) / count) as u16;
    }
    Ok(())
}

/// Correct defective columns after gain grids, using directional interpolation
/// for isolated columns and same-color neighbour averages for clustered columns.
pub(super) fn repair_columns(
    samples: &mut [u16],
    width: usize,
    height: usize,
    defects: &[u8],
    cancelled: &dyn Fn() -> bool,
) -> Result<(), DecodeError> {
    if width == 0
        || height == 0
        || width.checked_mul(height) != Some(samples.len())
        || !defects.len().is_multiple_of(8)
        || defects.len() > 65536 * 8
    {
        return Err(error("invalid column defect geometry"));
    }
    if cancelled() {
        return Err(DecodeError::Cancelled);
    }
    let mut columns = Vec::new();
    for record in defects.chunks_exact(8) {
        let word = |i| u16::from_le_bytes([record[i], record[i + 1]]) as usize;
        let (x, y, kind) = (word(0), word(2), word(4));
        if word(6) != 0 || !matches!(kind, 129 | 131 | 137) || x >= width || (kind == 129 && y >= height) {
            return Err(error("unqualified column defect record"));
        }
        if kind != 129 {
            columns.push(x);
        }
    }
    columns.sort_unstable();
    if columns.windows(2).any(|pair| pair[1] == pair[0]) {
        return Err(error("duplicate defective column"));
    }
    // Four directional neighbourhoods, with the other three obtained by
    // reflection. Each direction contains six sample pairs for edge confidence.
    const DIRECTIONS: [[(isize, isize); 12]; 4] = [
        [
            (-4, -2),
            (4, 2),
            (-3, -1),
            (1, 1),
            (-1, -1),
            (3, 1),
            (-4, -1),
            (0, 1),
            (-2, -1),
            (2, 1),
            (0, -1),
            (4, 1),
        ],
        [
            (-2, -2),
            (2, 2),
            (-3, -1),
            (-1, 1),
            (-1, -1),
            (1, 1),
            (1, -1),
            (3, 1),
            (-2, -1),
            (0, 1),
            (0, -1),
            (2, 1),
        ],
        [
            (-2, -4),
            (2, 4),
            (-1, -3),
            (1, 1),
            (-1, -1),
            (1, 3),
            (-2, -1),
            (0, 3),
            (-1, -2),
            (1, 2),
            (0, -3),
            (2, 1),
        ],
        [
            (0, -2),
            (0, 2),
            (-1, -1),
            (-1, 1),
            (1, -1),
            (1, 1),
            (-1, -2),
            (-1, 2),
            (0, -1),
            (0, -1),
            (1, -2),
            (1, 2),
        ],
    ];
    for (index, &x) in columns.iter().enumerate() {
        let isolated = (index == 0 || x - columns[index - 1] > 4)
            && (index + 1 == columns.len() || columns[index + 1] - x > 4);
        for y in 0..height {
            if cancelled() {
                return Err(DecodeError::Cancelled);
            }
            if !isolated {
                // Search increasingly distant diagonals, counting only in-bounds
                // sites. Preserve column order because previous repairs participate.
                for radius in 0..3 {
                    let mut sum = 0u32;
                    let mut count = 0u32;
                    for dy in [-4isize, -2, 2, 4] {
                        for dx in [-4isize, -2, 2, 4] {
                            let selected = match radius {
                                0 => dy.abs() == 2 && dx.abs() == 2,
                                1 => dy.abs() != dx.abs(),
                                _ => dy.abs() == 4 && dx.abs() == 4,
                            };
                            if selected {
                                if let (Some(r), Some(c)) =
                                    (y.checked_add_signed(dy), x.checked_add_signed(dx))
                                {
                                    if r < height && c < width {
                                        sum += samples[r * width + c] as u32;
                                        count += 1;
                                    }
                                }
                            }
                        }
                    }
                    if count != 0 {
                        samples[y * width + x] = ((sum + count / 2) / count) as u16;
                        break;
                    }
                }
                continue;
            }
            let sample = |dy: isize, dx: isize| -> u32 {
                match (y.checked_add_signed(dy), x.checked_add_signed(dx)) {
                    (Some(r), Some(c)) if r < height && c < width => samples[r * width + c] as u32,
                    _ => 0,
                }
            };
            let bounds = [sample(0, -2), sample(0, 2)];
            let mut scores = [(0u32, 0u32); 7];
            for (direction, score) in scores.iter_mut().enumerate() {
                let (base, sign) = if direction <= 3 {
                    (direction, 1)
                } else {
                    (6 - direction, -1)
                };
                for (pair_index, pair) in DIRECTIONS[base].chunks_exact(2).enumerate() {
                    let a = sample(pair[0].0, pair[0].1 * sign);
                    let b = sample(pair[1].0, pair[1].1 * sign);
                    if pair_index == 0 {
                        score.0 = a + b;
                    }
                    score.1 += a.abs_diff(b);
                }
            }
            let limit = scores.iter().map(|s| s.1).min().unwrap() * 3 / 2;
            let mut sum = 0;
            let mut count = 0;
            for (estimate, gradient) in scores {
                if gradient <= limit {
                    sum += estimate;
                    count += 2;
                }
            }
            samples[y * width + x] =
                ((sum + count / 2) / count).clamp(bounds[0].min(bounds[1]), bounds[0].max(bounds[1])) as u16;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    #[ignore = "P20+ external source and stage oracles; paired informational benchmark"]
    fn real_flat_field_specialization_paired() {
        use std::time::Instant;
        let source = std::fs::read(std::env::var("RRRAH_IIQ_SOURCE").unwrap()).unwrap();
        let input = std::fs::read(std::env::var("RRRAH_IIQ_ORACLE").unwrap()).unwrap();
        let mut base: Vec<u16> = input
            .chunks_exact(2)
            .map(|b| u16::from_le_bytes(b.try_into().unwrap()))
            .collect();
        subtract_black(&mut base, 4134, 1024, &|| false).unwrap();
        repair_pixels(
            &mut base,
            4134,
            4128,
            &source[20608740..20608740 + 14920],
            &|| false,
        )
        .unwrap();
        let reference = std::fs::read(std::env::var("RRRAH_IIQ_CORRECTED_ORACLE").unwrap()).unwrap();
        let expected: Vec<u16> = reference
            .chunks_exact(2)
            .map(|b| u16::from_le_bytes(b.try_into().unwrap()))
            .collect();
        let mut timings = [Vec::new(), Vec::new()];
        for iteration in 0..10 {
            let mut results = [0.; 2];
            for order in 0..2 {
                let variant = (iteration + order) % 2;
                let mut pixels = base.clone();
                let started = Instant::now();
                for (offset, length, channels) in [(15240, 66322, 2), (81564, 526354, 2), (607920, 132628, 4)]
                {
                    let field = &source[20608420 + offset..20608420 + offset + length];
                    if variant == 0 {
                        flat_field_baseline(&mut pixels, 4134, 4128, field, channels, &|| false).unwrap();
                    } else {
                        flat_field(&mut pixels, 4134, 4128, field, channels, &|| false).unwrap();
                    }
                }
                results[variant] = started.elapsed().as_secs_f64() * 1000.;
                repair_columns(
                    &mut pixels,
                    4134,
                    4128,
                    &source[20608740..20608740 + 14920],
                    &|| false,
                )
                .unwrap();
                assert_eq!(pixels, expected, "iteration {iteration} variant {variant}");
            }
            if iteration >= 2 {
                eprintln!(
                    "iiq_flat_pair,{},baseline_ms={:.6},specialized_ms={:.6},saved_ms={:.6}",
                    iteration - 1,
                    results[0],
                    results[1],
                    results[0] - results[1]
                );
                for (samples, value) in timings.iter_mut().zip(results) {
                    samples.push(value);
                }
            }
        }
        for (name, mut times) in ["baseline", "specialized"].into_iter().zip(timings) {
            times.sort_by(f64::total_cmp);
            eprintln!(
                "iiq_flat_summary,{name},p50_ms={:.6},p95_ms={:.6}",
                times[4], times[7]
            );
        }
    }
    #[test]
    fn adjacent_columns_use_valid_diagonal_neighbours_at_sensor_edges() {
        let mut sensor = vec![100u16; 121];
        for row in 0..11 {
            sensor[row * 11] = 60000;
            sensor[row * 11 + 1] = 60000;
        }
        let records: Vec<u8> = [0u16, 0, 131, 0, 1, 0, 137, 0]
            .into_iter()
            .flat_map(u16::to_le_bytes)
            .collect();
        repair_columns(&mut sensor, 11, 11, &records, &|| false).unwrap();
        assert_eq!(sensor, vec![100; 121]);
        let mut tiny = [777u16; 4];
        repair_columns(&mut tiny, 2, 2, &records, &|| false).unwrap();
        assert_eq!(tiny, [777; 4]); // No valid same-color neighbours: leave untouched.
    }
    #[test]
    fn isolated_columns_preserve_constant_sensor_and_validate_before_mutation() {
        let record: Vec<u8> = [5u16, 0, 131, 0].into_iter().flat_map(u16::to_le_bytes).collect();
        let mut sensor = vec![100u16; 121];
        for y in 0..11 {
            sensor[y * 11 + 5] = 60000;
        }
        repair_columns(&mut sensor, 11, 11, &record, &|| false).unwrap();
        assert_eq!(sensor, vec![100; 121]);
        let original = sensor.clone();
        let mut duplicate = record.clone();
        duplicate.extend([5u16, 0, 137, 0].into_iter().flat_map(u16::to_le_bytes));
        assert!(repair_columns(&mut sensor, 11, 11, &duplicate, &|| false).is_err());
        assert_eq!(sensor, original);
        for end in 1..8 {
            assert!(repair_columns(&mut sensor, 11, 11, &record[..end], &|| false).is_err());
        }
        assert!(matches!(
            repair_columns(&mut sensor, 11, 11, &record, &|| true),
            Err(DecodeError::Cancelled)
        ));
        assert_eq!(sensor, original);
    }
    #[test]
    fn pixel_defects_use_valid_neighbours_and_reject_malformed_records() {
        let mut sensor = vec![100u16; 9];
        sensor[4] = 60000;
        let record: Vec<u8> = [1u16, 1, 129, 0].into_iter().flat_map(u16::to_le_bytes).collect();
        repair_pixels(&mut sensor, 3, 3, &record, &|| false).unwrap();
        assert_eq!(sensor, [100; 9]);
        for length in 1..record.len() {
            assert!(repair_pixels(&mut sensor, 3, 3, &record[..length], &|| false).is_err());
        }
        assert!(matches!(
            repair_pixels(&mut sensor, 3, 3, &record, &|| true),
            Err(DecodeError::Cancelled)
        ));
        let mut invalid = record.clone();
        invalid[0..2].copy_from_slice(&3u16.to_le_bytes());
        assert!(repair_pixels(&mut sensor, 3, 3, &invalid, &|| false).is_err());
        invalid = record;
        invalid[4..6].copy_from_slice(&128u16.to_le_bytes());
        assert!(repair_pixels(&mut sensor, 3, 3, &invalid, &|| false).is_err());
    }
    #[test]
    #[ignore = "requires P20+ sensor and independent corrected oracle with column entries disabled"]
    fn real_pixel_repair_and_flat_fields_match_independent_sensor() {
        let source = std::fs::read(std::env::var("RRRAH_IIQ_SOURCE").unwrap()).unwrap();
        let input = std::fs::read(std::env::var("RRRAH_IIQ_ORACLE").unwrap()).unwrap();
        let mut samples: Vec<u16> = input
            .chunks_exact(2)
            .map(|b| u16::from_le_bytes(b.try_into().unwrap()))
            .collect();
        subtract_black(&mut samples, 4134, 1024, &|| false).unwrap();
        repair_pixels(
            &mut samples,
            4134,
            4128,
            &source[20608740..20608740 + 14920],
            &|| false,
        )
        .unwrap();
        for (offset, length, channels) in [(15240, 66322, 2), (81564, 526354, 2), (607920, 132628, 4)] {
            flat_field(
                &mut samples,
                4134,
                4128,
                &source[20608420 + offset..20608420 + offset + length],
                channels,
                &|| false,
            )
            .unwrap();
        }
        let oracle_path = if let Ok(path) = std::env::var("RRRAH_IIQ_CORRECTED_ORACLE") {
            repair_columns(
                &mut samples,
                4134,
                4128,
                &source[20608740..20608740 + 14920],
                &|| false,
            )
            .unwrap();
            path
        } else {
            std::env::var("RRRAH_IIQ_PIXELS_ORACLE").unwrap()
        };
        let oracle = std::fs::read(oracle_path).unwrap();
        assert_eq!(oracle.len(), samples.len() * 2);
        for (i, (a, b)) in samples.iter().zip(oracle.chunks_exact(2)).enumerate() {
            assert_eq!(*a, u16::from_le_bytes(b.try_into().unwrap()), "sample {i}");
        }
    }
    #[test]
    fn black_saturation_chroma_and_field_bounds() {
        let mut data = vec![0, 1023, 1024, 1025];
        subtract_black(&mut data, 2, 1024, &|| false).unwrap();
        assert_eq!(data, [0, 0, 0, 1]);
        let mut field: Vec<u8> = [0u16, 0, 4, 4, 2, 2, 0, 0]
            .into_iter()
            .flat_map(u16::to_le_bytes)
            .collect();
        for _ in 0..4 {
            field.extend(65535u16.to_le_bytes());
            field.extend(16384u16.to_le_bytes());
        }
        let mut sensor = vec![100u16; 16];
        flat_field(&mut sensor, 4, 4, &field, 4, &|| false).unwrap();
        assert_eq!(&sensor[..8], &[100, 50, 100, 100, 199, 100, 100, 100]);
        assert!(sensor[8..].iter().all(|v| *v == 100));
        for end in 0..field.len() {
            assert!(flat_field(&mut sensor, 4, 4, &field[..end], 4, &|| false).is_err());
        }
        assert!(matches!(
            flat_field(&mut sensor, 4, 4, &field, 4, &|| true),
            Err(DecodeError::Cancelled)
        ));
        assert!(flat_field(&mut sensor, 4, 4, &field, 3, &|| false).is_err());
        field[8..10].fill(0);
        assert!(flat_field(&mut sensor, 4, 4, &field, 4, &|| false).is_err());
    }
    #[test]
    #[ignore = "requires P20+ native sensor and independent black/flat-field stage oracles"]
    fn real_native_black_and_flat_fields_match_stage_oracles() {
        let source = std::fs::read(std::env::var("RRRAH_IIQ_SOURCE").unwrap()).unwrap();
        let bytes = std::fs::read(std::env::var("RRRAH_IIQ_ORACLE").unwrap()).unwrap();
        let mut samples: Vec<u16> = bytes
            .chunks_exact(2)
            .map(|b| u16::from_le_bytes(b.try_into().unwrap()))
            .collect();
        subtract_black(&mut samples, 4134, 1024, &|| false).unwrap();
        let black = std::fs::read(std::env::var("RRRAH_IIQ_BLACK_ORACLE").unwrap()).unwrap();
        assert_eq!(black.len(), samples.len() * 2);
        assert!(
            samples
                .iter()
                .zip(black.chunks_exact(2))
                .all(|(a, b)| *a == u16::from_le_bytes(b.try_into().unwrap()))
        );
        for (offset, length, channels) in [(15240, 66322, 2), (81564, 526354, 2), (607920, 132628, 4)] {
            flat_field(
                &mut samples,
                4134,
                4128,
                &source[20608420 + offset..20608420 + offset + length],
                channels,
                &|| false,
            )
            .unwrap();
        }
        let oracle = std::fs::read(std::env::var("RRRAH_IIQ_FLAT_ORACLE").unwrap()).unwrap();
        assert_eq!(oracle.len(), samples.len() * 2);
        for (i, (a, b)) in samples.iter().zip(oracle.chunks_exact(2)).enumerate() {
            assert_eq!(*a, u16::from_le_bytes(b.try_into().unwrap()), "sample {i}");
        }
    }
}
