//! Owned PNG ICC inflation. Call after native declaration/CRC validation.
use crate::{
    DecodeError, DecodeRequest,
    raster::{MAX_RASTER_BYTES, RasterDecodeError},
};
use flate2::{Decompress, FlushDecompress, Status};
use rrrah_core::{MemoryBudget, Reservation};
type OwnedProfile = (Vec<u8>, Option<Reservation>);
fn bad(reason: &'static str) -> RasterDecodeError {
    RasterDecodeError::InvalidPngColor(reason)
}

pub(super) fn profile(
    bytes: &[u8],
    request: &DecodeRequest,
) -> Result<Option<OwnedProfile>, RasterDecodeError> {
    let mut offset = 8usize;
    for _ in 0..65536 {
        request.check_cancelled()?;
        let header = bytes
            .get(offset..offset.checked_add(8).ok_or_else(|| bad("ICC chunk overflow"))?)
            .ok_or_else(|| bad("truncated ICC chunk header"))?;
        let length = u32::from_be_bytes(header[..4].try_into().unwrap()) as usize;
        let end = offset
            .checked_add(12)
            .and_then(|n| n.checked_add(length))
            .ok_or_else(|| bad("ICC chunk overflow"))?;
        let chunk = bytes.get(offset..end).ok_or_else(|| bad("truncated ICC chunk"))?;
        match &header[4..8] {
            b"IDAT" | b"IEND" => return Ok(None),
            b"iCCP" => {
                let data = &chunk[8..8 + length];
                let name = data
                    .iter()
                    .position(|&v| v == 0)
                    .filter(|&n| n > 0 && n <= 79)
                    .ok_or_else(|| bad("invalid ICC profile name"))?;
                if data.get(name + 1) != Some(&0) {
                    return Err(bad("unsupported ICC compression method"));
                }
                let output = inflate(&data[name + 2..], request.memory_budget.as_ref(), &|| {
                    request.check_cancelled().map_err(Into::into)
                })?;
                return Ok(Some(output));
            }
            _ => {}
        }
        offset = end;
    }
    Err(bad("excessive ICC chunk scan"))
}
fn inflate(
    input: &[u8],
    budget: Option<&MemoryBudget>,
    check: &dyn Fn() -> Result<(), RasterDecodeError>,
) -> Result<OwnedProfile, RasterDecodeError> {
    check()?;
    let state = budget
        .map(|b| b.try_reserve(rrrah_scientific::DEFLATE_DECODER_CREDIT_BYTES))
        .transpose()
        .map_err(DecodeError::Memory)?;
    let mut inflater = Decompress::new(true);
    let mut credit = budget
        .map(|b| b.try_reserve(0))
        .transpose()
        .map_err(DecodeError::Memory)?;
    let mut output = Vec::new();
    let mut at = 0usize;
    let mut block = [0u8; 4096]; // Bounded stack scratch, not an untracked heap output.
    loop {
        check()?;
        let end = at.saturating_add(65536).min(input.len());
        let before_in = inflater.total_in();
        let before_out = inflater.total_out();
        let status = inflater
            .decompress(&input[at..end], &mut block, FlushDecompress::None)
            .map_err(|_| bad("invalid ICC zlib stream"))?;
        let used = usize::try_from(inflater.total_in() - before_in).map_err(|_| bad("ICC input count"))?;
        let produced =
            usize::try_from(inflater.total_out() - before_out).map_err(|_| bad("ICC output count"))?;
        at += used;
        check()?;
        let needed = output
            .len()
            .checked_add(produced)
            .filter(|&n| n as u64 <= MAX_RASTER_BYTES)
            .ok_or(RasterDecodeError::OutputTooLarge)?;
        if needed > output.capacity() {
            let mut capacity = output.capacity().checked_mul(2).unwrap_or(needed).max(needed);
            capacity = capacity.min(MAX_RASTER_BYTES as usize);
            if let (Some(budget), Some(credit)) = (budget, credit.as_ref()) {
                if (capacity as u64).saturating_sub(credit.bytes()) > budget.available_bytes() {
                    capacity = needed;
                }
            }
            if let Some(credit) = credit.as_mut() {
                credit
                    .ensure_bytes(capacity as u64)
                    .map_err(DecodeError::Memory)?;
            }
            output
                .try_reserve_exact(capacity - output.len())
                .map_err(|e| DecodeError::from(rrrah_core::BufferError::Allocate(e)))?;
            if let Some(credit) = credit.as_mut() {
                credit
                    .ensure_bytes(output.capacity() as u64)
                    .map_err(DecodeError::Memory)?;
            }
        }
        output.extend_from_slice(&block[..produced]);
        if status == Status::StreamEnd {
            if at != input.len() {
                return Err(bad("trailing ICC compressed data"));
            }
            if output.is_empty() {
                return Err(bad("empty ICC profile"));
            }
            drop(inflater);
            drop(state);
            return Ok((output, credit));
        }
        if used == 0 && produced == 0 {
            return Err(bad("truncated or stalled ICC zlib stream"));
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    #[test]
    fn exact_state_output_credit_short_checksum_and_last_owner() {
        let data = vec![17; 588];
        let mut encoder = flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::default());
        encoder.write_all(&data).unwrap();
        let encoded = encoder.finish().unwrap();
        let peak = rrrah_scientific::DEFLATE_DECODER_CREDIT_BYTES + data.len() as u64;
        let root = MemoryBudget::new(peak);
        let (output, credit) = inflate(&encoded, Some(&root), &|| Ok(())).unwrap();
        assert_eq!(output, data);
        assert_eq!((root.used(), root.peak()), (588, peak));
        drop(output);
        drop(credit);
        assert_eq!(root.used(), 0);
        let short = MemoryBudget::new(peak - 1);
        assert!(matches!(
            inflate(&encoded, Some(&short), &|| Ok(())),
            Err(RasterDecodeError::Source(DecodeError::Memory(_)))
        ));
        assert_eq!(short.used(), 0);
        let mut invalid = encoded.clone();
        *invalid.last_mut().unwrap() ^= 1;
        assert!(inflate(&invalid, Some(&root), &|| Ok(())).is_err());
        assert_eq!(root.used(), 0);
        let mut trailing = encoded;
        trailing.push(0);
        assert!(inflate(&trailing, Some(&root), &|| Ok(())).is_err());
        assert_eq!(root.used(), 0);
    }
    #[test]
    fn empty_block_stream_cancels_after_admission_and_retries() {
        let mut encoded = vec![0x78, 0x01];
        for _ in 0..100000 {
            encoded.extend_from_slice(&[0, 0, 0, 255, 255]);
        }
        encoded.extend_from_slice(&[1, 4, 0, 251, 255]);
        encoded.extend_from_slice(b"ICCP");
        let (mut first, mut second) = (1u32, 0u32);
        for &b in b"ICCP" {
            first += u32::from(b);
            second += first;
        }
        encoded.extend_from_slice(&((second << 16) | first).to_be_bytes());
        let root = MemoryBudget::new(rrrah_scientific::DEFLATE_DECODER_CREDIT_BYTES + 4);
        let checks = std::cell::Cell::new(0);
        let result = inflate(&encoded, Some(&root), &|| {
            checks.set(checks.get() + 1);
            if checks.get() == 3 {
                Err(DecodeError::Cancelled.into())
            } else {
                Ok(())
            }
        });
        assert!(matches!(
            result,
            Err(RasterDecodeError::Source(DecodeError::Cancelled))
        ));
        assert_eq!(
            (root.used(), root.peak()),
            (0, rrrah_scientific::DEFLATE_DECODER_CREDIT_BYTES)
        );
        let (output, credit) = inflate(&encoded, Some(&root), &|| Ok(())).unwrap();
        assert_eq!(output, b"ICCP");
        drop(output);
        drop(credit);
        assert_eq!(root.used(), 0);
    }
    #[test]
    fn cancellation_after_partial_profile_growth_releases_state_and_output() {
        let data = vec![29; 32769];
        let mut encoder = flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::default());
        encoder.write_all(&data).unwrap();
        let encoded = encoder.finish().unwrap();
        let root = MemoryBudget::new(rrrah_scientific::DEFLATE_DECODER_CREDIT_BYTES + data.len() as u64);
        let checks = std::cell::Cell::new(0);
        let result = inflate(&encoded, Some(&root), &|| {
            checks.set(checks.get() + 1);
            if checks.get() == 4 {
                assert_eq!(root.used(), rrrah_scientific::DEFLATE_DECODER_CREDIT_BYTES + 4096);
                Err(DecodeError::Cancelled.into())
            } else {
                Ok(())
            }
        });
        assert!(matches!(
            result,
            Err(RasterDecodeError::Source(DecodeError::Cancelled))
        ));
        assert_eq!(root.used(), 0);
        let (output, credit) = inflate(&encoded, Some(&root), &|| Ok(())).unwrap();
        assert_eq!(output, data);
        drop(output);
        drop(credit);
        assert_eq!(root.used(), 0);
    }
}
