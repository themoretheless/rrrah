//! Explicit Pentax Optio S4 headerless sensor import. The caller must establish
//! camera identity independently; file size is never used for automatic routing.
//! Layout qualified against rawsamples.ch's camera-produced Optio S4 file and
//! an independent full-sensor oracle. No color/WB or crop inference is performed.

pub const OPTIO_S4_SENSOR_SIZE: (u32, u32) = (2346, 1737);
const ROW_BYTES: usize = 3520;
const FILE_BYTES: usize = ROW_BYTES * OPTIO_S4_SENSOR_SIZE.1 as usize;

#[derive(Debug, thiserror::Error)]
pub enum OptioS4SensorError {
    #[error("Optio S4 sensor file length differs from its explicit layout")]
    Length,
    #[error("Optio S4 sensor import cancelled")]
    Cancelled,
    #[error(transparent)]
    Source(#[from] crate::DecodeError),
    #[error(transparent)]
    Memory(#[from] rrrah_core::BufferError),
}

/// Read a caller-selected Optio S4 sensor with source and output charged to one
/// root. This does not supply the missing camera WB or display color profile.
pub fn read_optio_s4_sensor_with_budget(
    request: &crate::DecodeRequest,
    budget: &rrrah_core::MemoryBudget,
) -> Result<rrrah_core::PixelBuffer<u16>, OptioS4SensorError> {
    request.check_cancelled()?;
    if request.image_index != 0 {
        return Err(crate::DecodeError::UnsupportedImageIndex {
            index: request.image_index,
        }
        .into());
    }
    let mut bounded = request.clone();
    bounded.memory_budget = Some(budget.clone());
    let source = crate::bounded_io::read_bounded(&bounded)?;
    unpack_optio_s4_sensor(&source, budget, || {
        request
            .cancellation
            .as_ref()
            .is_some_and(crate::GenerationToken::is_cancelled)
    })
}

/// Full 2346×1737 sensor, 12-bit MSB-first pairs in each 3520-byte row. The
/// final row byte is padding and never enters a sample. No implicit active crop.
pub fn unpack_optio_s4_sensor(
    bytes: &[u8],
    budget: &rrrah_core::MemoryBudget,
    mut cancelled: impl FnMut() -> bool,
) -> Result<rrrah_core::PixelBuffer<u16>, OptioS4SensorError> {
    if cancelled() {
        return Err(OptioS4SensorError::Cancelled);
    }
    if bytes.len() != FILE_BYTES {
        return Err(OptioS4SensorError::Length);
    }
    let width = OPTIO_S4_SENSOR_SIZE.0 as usize;
    let mut output = budget.try_buffer(width * OPTIO_S4_SENSOR_SIZE.1 as usize, 0u16)?;
    for (row, samples) in bytes.chunks_exact(ROW_BYTES).zip(output.chunks_exact_mut(width)) {
        if cancelled() {
            return Err(OptioS4SensorError::Cancelled);
        }
        for (group, pair) in row[..ROW_BYTES - 1]
            .chunks_exact(3)
            .zip(samples.chunks_exact_mut(2))
        {
            pair[0] = u16::from(group[0]) << 4 | u16::from(group[1] >> 4);
            pair[1] = u16::from(group[1] & 15) << 8 | u16::from(group[2]);
        }
    }
    if cancelled() {
        return Err(OptioS4SensorError::Cancelled);
    }
    Ok(output.freeze().into())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn row_padding_admission_and_mid_import_cancellation() {
        let mut source = vec![0; FILE_BYTES];
        for (y, row) in source.chunks_exact_mut(ROW_BYTES).enumerate() {
            for (x, group) in row[..ROW_BYTES - 1].chunks_exact_mut(3).enumerate() {
                let a = (y * 17 + x * 2) as u16 & 4095;
                let b = (y * 17 + x * 2 + 1) as u16 & 4095;
                group.copy_from_slice(&[(a >> 4) as u8, ((a & 15) << 4 | b >> 8) as u8, b as u8]);
            }
            row[ROW_BYTES - 1] = 255;
        }
        let output_bytes = u64::from(OPTIO_S4_SENSOR_SIZE.0) * u64::from(OPTIO_S4_SENSOR_SIZE.1) * 2;
        let root = rrrah_core::MemoryBudget::new(output_bytes);
        let pixels = unpack_optio_s4_sensor(&source, &root, || false).unwrap();
        for (y, row) in pixels.chunks_exact(OPTIO_S4_SENSOR_SIZE.0 as usize).enumerate() {
            assert!(
                row.iter()
                    .enumerate()
                    .all(|(x, &v)| v == (y * 17 + x) as u16 & 4095)
            );
        }
        assert_eq!(root.used(), output_bytes);
        let clone = pixels.clone();
        drop(pixels);
        assert_eq!(root.used(), output_bytes);
        drop(clone);
        assert_eq!(root.used(), 0);
        let tight = rrrah_core::MemoryBudget::new(output_bytes - 1);
        assert!(matches!(
            unpack_optio_s4_sensor(&source, &tight, || false),
            Err(OptioS4SensorError::Memory(_))
        ));
        assert_eq!(tight.peak(), 0);
        let mut polls = 0;
        assert!(matches!(
            unpack_optio_s4_sensor(&source, &root, || {
                polls += 1;
                polls == 3
            }),
            Err(OptioS4SensorError::Cancelled)
        ));
        assert_eq!(root.used(), 0);
        let empty = rrrah_core::MemoryBudget::new(0);
        assert!(matches!(
            unpack_optio_s4_sensor(&source[..FILE_BYTES - 1], &empty, || false),
            Err(OptioS4SensorError::Length)
        ));
        assert!(matches!(
            unpack_optio_s4_sensor(&[], &empty, || true),
            Err(OptioS4SensorError::Cancelled)
        ));
        assert_eq!(empty.peak(), 0);
    }

    #[test]
    #[ignore = "manual camera-produced Optio S4 corpus qualification"]
    fn camera_file_matches_complete_independent_sensor_and_exact_root_boundary() {
        let path = std::env::var("RRRAH_OPTIO_S4_SOURCE").expect("real source required");
        let oracle = std::env::var("RRRAH_OPTIO_S4_ORACLE").expect("independent u16le oracle required");
        assert_eq!(
            blake3::hash(&std::fs::read(&path).unwrap()).to_hex().as_str(),
            "d8c9d03d4f4a5b1adab596794505b15b00e381428d0d79c1ce90de6accbc5aa0",
            "camera corpus source differs from the qualified file"
        );
        let request = crate::DecodeRequest::new(path);
        let sensor_bytes = u64::from(OPTIO_S4_SENSOR_SIZE.0) * u64::from(OPTIO_S4_SENSOR_SIZE.1) * 2;
        let limit = FILE_BYTES as u64 + sensor_bytes;
        let root = rrrah_core::MemoryBudget::new(limit);
        let sensor = read_optio_s4_sensor_with_budget(&request, &root).unwrap();
        let reference = std::fs::read(oracle).unwrap();
        assert_eq!(
            blake3::hash(&reference).to_hex().as_str(),
            "fde158b5e8927a1dfbe10398bcabb09239f5aa72e9085d8cdec452766b55990d",
            "independent sensor oracle differs from the pinned reference"
        );
        assert_eq!(reference.len() as u64, sensor_bytes);
        assert!(
            sensor
                .iter()
                .zip(reference.chunks_exact(2))
                .all(|(&value, pair)| value == u16::from_le_bytes([pair[0], pair[1]]))
        );
        assert_eq!(root.peak(), limit);
        assert_eq!(root.used(), sensor_bytes);
        drop(sensor);
        assert_eq!(root.used(), 0);
        let tight = rrrah_core::MemoryBudget::new(limit - 1);
        assert!(matches!(
            read_optio_s4_sensor_with_budget(&request, &tight),
            Err(OptioS4SensorError::Memory(_))
        ));
        assert_eq!(tight.used(), 0);
        assert_eq!(tight.peak(), FILE_BYTES as u64);
    }
}
