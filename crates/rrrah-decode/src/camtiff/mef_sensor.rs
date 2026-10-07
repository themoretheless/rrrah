//! Explicit Mamiya ZD sensor import. No color route until native WB resolves.
use crate::DecodeError;
use rrrah_core::{MemoryBudget, PixelBuffer};
fn error(message: &str) -> DecodeError {
    super::camera_error("MEF", message)
}
/// Source bytes are borrowed; their allocation accounting belongs to the caller.
/// Output admission precedes allocation and is retained through the last owner.
fn packed_12(
    bytes: &[u8],
    width: u32,
    height: u32,
    budget: &MemoryBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<PixelBuffer<u16>, DecodeError> {
    if cancelled() {
        return Err(DecodeError::Cancelled);
    }
    if width == 0 || height == 0 || width % 2 != 0 {
        return Err(error("invalid packed sensor dimensions"));
    }
    let count = (width as usize)
        .checked_mul(height as usize)
        .ok_or(DecodeError::DimensionOverflow)?;
    let length = count
        .checked_div(2)
        .and_then(|n| n.checked_mul(3))
        .ok_or(DecodeError::DimensionOverflow)?;
    if bytes.len() != length {
        return Err(error("packed sensor length differs from geometry"));
    }
    let mut output = budget.try_buffer(count, 0u16)?;
    let row_bytes = width as usize / 2 * 3;
    for (source, destination) in bytes
        .chunks_exact(row_bytes)
        .zip(output.chunks_exact_mut(width as usize))
    {
        if cancelled() {
            return Err(DecodeError::Cancelled);
        }
        for (encoded, decoded) in source.chunks_exact(3).zip(destination.chunks_exact_mut(2)) {
            decoded[0] = (u16::from(encoded[0]) << 4) | (u16::from(encoded[1]) >> 4);
            decoded[1] = ((u16::from(encoded[1]) & 15) << 8) | u16::from(encoded[2]);
        }
    }
    if cancelled() {
        return Err(DecodeError::Cancelled);
    }
    Ok(output.freeze().into())
}
/// Import the qualified Mamiya ZD packed sensor layout (4016 x 5344).
/// Source memory is caller-owned; output is managed and cancellable. Returns
/// sensor codes only: no white balance, color development or preview fallback.
/// Other models/storage layouts are rejected. This does not enable MEF viewing.
pub fn decode_mef_zd_sensor(
    bytes: &[u8],
    budget: &MemoryBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<PixelBuffer<u16>, DecodeError> {
    if cancelled() {
        return Err(DecodeError::Cancelled);
    }
    let file = super::CameraFile::parse_tiff("MEF", bytes)?;
    let root = file
        .directories()
        .first()
        .ok_or_else(|| error("missing root IFD"))?;
    // This camera pads fixed-width ASCII fields with multiple trailing NULs.
    // Admit its exact declared representation rather than weakening TIFF ASCII.
    let make = root.entry("MEF", 271)?.ok_or_else(|| error("missing make"))?;
    let model = root.entry("MEF", 272)?.ok_or_else(|| error("missing model"))?;
    if bytes.get(..4) != Some(&b"MM\0\x2a"[..])
        || make.field_type != crate::dng::tiff::FieldType::Ascii
        || model.field_type != crate::dng::tiff::FieldType::Ascii
        || make.raw_bytes() != b"Mamiya-OP Co.,Ltd.\0\0"
        || model.raw_bytes() != b"MAMIYA ZD\0\0\0\0\0\0\0"
    {
        return Err(error("unqualified Mamiya camera"));
    }
    let mut sensor = None;
    for directory in file.directories() {
        if super::optional_scalar("MEF", directory, 256)? == Some(4016)
            && super::optional_scalar("MEF", directory, 257)? == Some(5344)
        {
            if sensor.replace(directory).is_some() {
                return Err(error("ambiguous sensor directory"));
            }
        }
    }
    let raw = sensor.ok_or_else(|| error("missing qualified sensor directory"))?;
    for (tag, value) in [(259, 1), (273, 4383052), (278, 5344), (279, 32192256)] {
        if super::required_scalar("MEF", raw, tag)? != value {
            return Err(error("unqualified packed storage"));
        }
    }
    let bits = raw
        .entry("MEF", 258)?
        .ok_or_else(|| error("missing bit depth"))?
        .numeric_values()
        .map_err(|e| error(&e.to_string()))?;
    if bits != [12., 12., 12.] {
        return Err(error("unqualified declared bit depths"));
    }
    let sensor = bytes
        .get(4383052..4383052 + 32192256)
        .ok_or_else(|| error("truncated packed sensor"))?;
    packed_12(sensor, 4016, 5344, budget, cancelled)
}
/// Read a qualified ZD sensor file with source and output under one root budget.
/// Only image zero is supported. Returns raw codes, not developed camera color.
pub fn decode_mef_zd_sensor_file(
    request: &crate::DecodeRequest,
    budget: &MemoryBudget,
) -> Result<PixelBuffer<u16>, DecodeError> {
    request.check_cancelled()?;
    if request.image_index != 0 {
        return Err(DecodeError::UnsupportedImageIndex {
            index: request.image_index,
        });
    }
    let mut bounded = request.clone();
    bounded.memory_budget = Some(budget.clone());
    let source = crate::bounded_io::read_bounded(&bounded)?;
    decode_mef_zd_sensor(&source, budget, &|| {
        bounded
            .cancellation
            .as_ref()
            .is_some_and(crate::GenerationToken::is_cancelled)
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn file_import_rejects_selection_and_stale_generation_before_reading() {
        let budget = MemoryBudget::new(1);
        let mut request = crate::DecodeRequest::new("nonexistent.mef");
        request.image_index = 1;
        assert!(matches!(
            decode_mef_zd_sensor_file(&request, &budget),
            Err(DecodeError::UnsupportedImageIndex { index: 1 })
        ));
        request.image_index = 0;
        request.cancellation = Some(crate::GenerationToken::new(
            std::sync::Arc::new(std::sync::atomic::AtomicU64::new(2)),
            1,
        ));
        assert!(matches!(
            decode_mef_zd_sensor_file(&request, &budget),
            Err(DecodeError::Cancelled)
        ));
        assert_eq!(budget.peak(), 0);
    }

    #[test]
    #[ignore = "requires pinned Mamiya ZD source; qualifies file admission, not camera color"]
    fn real_zd_file_import_shares_source_and_output_budget() {
        let path = std::env::var("RRRAH_MEF_SOURCE").unwrap();
        let source = std::fs::read(&path).unwrap();
        let weight = 4016 * 5344 * 2_u64;
        let oracle_budget = MemoryBudget::new(weight);
        let expected = decode_mef_zd_sensor(&source, &oracle_budget, &|| false).unwrap();
        let budget = MemoryBudget::new(source.len() as u64 + weight);
        let request = crate::DecodeRequest::new(&path);
        let output = decode_mef_zd_sensor_file(&request, &budget).unwrap();
        assert_eq!(&output[..], &expected[..]);
        assert_eq!(budget.used(), weight);
        assert_eq!(budget.peak(), source.len() as u64 + weight);
        let held = output.clone();
        drop(output);
        assert_eq!(budget.used(), weight);
        drop(held);
        assert_eq!(budget.used(), 0);
        let short = MemoryBudget::new(source.len() as u64 + weight - 1);
        assert!(matches!(
            decode_mef_zd_sensor_file(&request, &short),
            Err(DecodeError::Memory(_))
        ));
        assert_eq!(short.used(), 0);
        assert_eq!(short.peak(), source.len() as u64);
        let too_small = MemoryBudget::new(source.len() as u64 - 1);
        assert!(decode_mef_zd_sensor_file(&request, &too_small).is_err());
        assert_eq!(too_small.peak(), 0);
    }

    #[test]
    fn packed_boundaries_admission_cancel_and_ownership() {
        let bytes = [0x00, 0x0f, 0xff, 0x12, 0x3a, 0xbc];
        let budget = MemoryBudget::new(8);
        let output = packed_12(&bytes, 2, 2, &budget, &|| false).unwrap();
        assert_eq!(&output[..], &[0, 4095, 0x123, 0xabc]);
        let held = output.clone();
        drop(output);
        assert_eq!(budget.used(), 8);
        drop(held);
        assert_eq!(budget.used(), 0);
        let calls = std::cell::Cell::new(0);
        let cancel_final = || {
            let n = calls.get();
            calls.set(n + 1);
            n == 3
        };
        assert!(matches!(
            packed_12(&bytes, 2, 2, &budget, &cancel_final),
            Err(DecodeError::Cancelled)
        ));
        assert_eq!(budget.used(), 0);
        assert!(packed_12(&bytes, 2, 2, &MemoryBudget::new(7), &|| false).is_err());
        for (w, h, length) in [(3, 2, 6), (0, 2, 6), (2, 0, 6), (2, 2, 5)] {
            assert!(packed_12(&bytes[..length], w, h, &budget, &|| false).is_err());
        }
        assert!(matches!(
            packed_12(&bytes, 2, 2, &budget, &|| true),
            Err(DecodeError::Cancelled)
        ));
        let calls = std::cell::Cell::new(0);
        let cancelled = || {
            let n = calls.get();
            calls.set(n + 1);
            n == 2
        };
        assert!(matches!(
            packed_12(&bytes, 2, 2, &budget, &cancelled),
            Err(DecodeError::Cancelled)
        ));
        assert_eq!(budget.used(), 0);
    }
    #[test]
    #[ignore = "requires pinned Mamiya ZD source; validates lifecycle, not photographic color"]
    fn real_zd_sensor_admission_cancellation_and_last_owner() {
        let source = std::fs::read(std::env::var("RRRAH_MEF_SOURCE").unwrap()).unwrap();
        let weight = 4016 * 5344 * 2_u64;
        let budget = MemoryBudget::new(weight);
        let output = decode_mef_zd_sensor(&source, &budget, &|| false).unwrap();
        assert_eq!(output.len() as u64 * 2, weight);
        assert!(output.iter().all(|v| *v <= 4095));
        let owner = output.clone();
        drop(output);
        assert_eq!(budget.used(), weight);
        drop(owner);
        assert_eq!(budget.used(), 0);
        let short = MemoryBudget::new(weight - 1);
        assert!(matches!(
            decode_mef_zd_sensor(&source, &short, &|| false),
            Err(DecodeError::Memory(_))
        ));
        assert_eq!(short.peak(), 0);
        let calls = std::cell::Cell::new(0);
        let cancelled = || {
            calls.set(calls.get() + 1);
            calls.get() >= 128
        };
        let cancel_budget = MemoryBudget::new(weight);
        assert!(matches!(
            decode_mef_zd_sensor(&source, &cancel_budget, &cancelled),
            Err(DecodeError::Cancelled)
        ));
        assert_eq!(cancel_budget.peak(), weight);
        assert_eq!(cancel_budget.used(), 0);
        assert!(decode_mef_zd_sensor(&source, &budget, &|| false).is_ok());
        assert_eq!(budget.used(), 0);
        // Corrupt the actual sensor IFD rather than the rendered preview.
        let offset = 116920;
        let count = usize::from(u16::from_be_bytes([source[offset], source[offset + 1]]));
        for tag in [256u16, 257, 273, 278, 279] {
            let entry = (0..count)
                .map(|i| offset + 2 + i * 12)
                .find(|&p| u16::from_be_bytes([source[p], source[p + 1]]) == tag)
                .unwrap();
            let mut damaged = source.clone();
            damaged[entry + 8..entry + 12].fill(0);
            let refusal_budget = MemoryBudget::new(weight);
            assert!(
                decode_mef_zd_sensor(&damaged, &refusal_budget, &|| false).is_err(),
                "tag {tag}"
            );
            assert_eq!(
                refusal_budget.peak(),
                0,
                "tag {tag} allocated output before refusing"
            );
        }
    }
    #[test]
    #[ignore = "requires external Mamiya ZD source and independent sensor oracle"]
    fn real_zd_full_sensor_matches_oracle() {
        let source = std::fs::read(std::env::var("RRRAH_MEF_SOURCE").unwrap()).unwrap();
        let reference = std::fs::read(std::env::var("RRRAH_MEF_ORACLE").unwrap()).unwrap();
        let budget = MemoryBudget::new(64 * 1024 * 1024);
        let output = decode_mef_zd_sensor(&source, &budget, &|| false).unwrap();
        assert_eq!(output.len() * 2, reference.len());
        for (i, (a, b)) in output.iter().zip(reference.chunks_exact(2)).enumerate() {
            assert_eq!(*a, u16::from_le_bytes([b[0], b[1]]), "sample {i}");
        }
        let weight = output.len() as u64 * 2;
        assert_eq!(budget.used(), weight);
        let alias = output.clone();
        drop(output);
        assert_eq!(budget.used(), weight);
        drop(alias);
        assert_eq!(budget.used(), 0);
        let short = MemoryBudget::new(weight - 1);
        assert!(matches!(
            decode_mef_zd_sensor(&source, &short, &|| false),
            Err(DecodeError::Memory(_))
        ));
        assert_eq!(short.used(), 0);
        assert!(decode_mef_zd_sensor(&source[..source.len() - 1], &budget, &|| false).is_err());
    }
}
