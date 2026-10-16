//! Owned DICOM RLE frame codec (PS3.5 Annex G), independent of encapsulation.
//! Returns selected-frame integer bytes in declared planar layout, little endian.
use crate::{DecodeRequest, RasterDecodeError};
use rrrah_core::PixelBuffer;
use std::sync::Arc;
fn bad(message: &'static str) -> RasterDecodeError {
    RasterDecodeError::InvalidDicom(message)
}
#[derive(Debug, Clone, Copy)]
pub struct RleFrameLayout {
    pub width: u32,
    pub height: u32,
    pub samples: u16,
    pub bits: u16,
    pub planar: bool,
}
fn plane<const EMIT: bool>(
    source: &[u8],
    width: usize,
    pixels: usize,
    request: &DecodeRequest,
    mut emit: impl FnMut(usize, u8),
) -> Result<(), RasterDecodeError> {
    let (mut at, mut produced, mut commands) = (0usize, 0usize, 0usize);
    while produced < pixels {
        if commands % 4096 == 0 {
            request.check_cancelled()?;
        }
        commands += 1;
        let code = *source.get(at).ok_or_else(|| bad("truncated RLE plane"))?;
        at += 1;
        if code == 128 {
            continue;
        }
        let count = if code <= 127 {
            usize::from(code) + 1
        } else {
            257 - usize::from(code)
        };
        if count > width - produced % width {
            return Err(bad("RLE run crosses row boundary"));
        }
        if code <= 127 {
            let end = at.checked_add(count).ok_or_else(|| bad("RLE literal overflow"))?;
            let values = source.get(at..end).ok_or_else(|| bad("truncated RLE literal"))?;
            if EMIT {
                for (i, &value) in values.iter().enumerate() {
                    emit(produced + i, value);
                }
            }
            at = end;
        } else {
            let value = *source.get(at).ok_or_else(|| bad("truncated RLE replicate"))?;
            at += 1;
            if EMIT {
                for i in 0..count {
                    emit(produced + i, value);
                }
            }
        }
        produced += count;
    }
    let tail = &source[at..];
    let tail = if tail.last() == Some(&0) {
        &tail[..tail.len() - 1]
    } else {
        tail
    };
    for chunk in tail.chunks(4096) {
        request.check_cancelled()?;
        if !chunk.iter().all(|&v| v == 128) {
            return Err(bad("excess RLE plane data"));
        }
    }
    Ok(())
}
/// Decode a single RLE frame. Encapsulated-frame selection and DICOM metadata
/// interpretation are separate. All output allocation is admitted before use;
/// malformed packet/header validation precedes that allocation.
pub fn decode_frame(
    frame: &[u8],
    layout: RleFrameLayout,
    request: &DecodeRequest,
) -> Result<PixelBuffer<u8>, RasterDecodeError> {
    decode_frame_impl(frame, layout, request, false)
}
pub(crate) fn decode_frame_padded(
    frame: &[u8],
    layout: RleFrameLayout,
    request: &DecodeRequest,
) -> Result<PixelBuffer<u8>, RasterDecodeError> {
    decode_frame_impl(frame, layout, request, true)
}
fn decode_frame_impl(
    frame: &[u8],
    layout: RleFrameLayout,
    request: &DecodeRequest,
    pad: bool,
) -> Result<PixelBuffer<u8>, RasterDecodeError> {
    request.check_cancelled()?;
    if layout.width == 0
        || layout.height == 0
        || !matches!(layout.samples, 1 | 3)
        || !matches!(layout.bits, 8 | 16)
    {
        return Err(bad("unsupported RLE frame layout"));
    }
    let pixels = usize::try_from(u64::from(layout.width) * u64::from(layout.height))
        .map_err(|_| RasterDecodeError::OutputTooLarge)?;
    let unit = usize::from(layout.bits / 8);
    let samples = usize::from(layout.samples);
    let output = pixels
        .checked_mul(samples)
        .and_then(|n| n.checked_mul(unit))
        .ok_or(RasterDecodeError::OutputTooLarge)?;
    if output as u64 > crate::raster::MAX_RASTER_BYTES || frame.len() as u64 > crate::raster::MAX_RASTER_BYTES
    {
        return Err(RasterDecodeError::OutputTooLarge);
    }
    if frame.len() < 64 || frame.len() % 2 != 0 {
        return Err(bad("RLE header/frame length"));
    }
    let word = |at: usize| u32::from_le_bytes(frame[at..at + 4].try_into().unwrap());
    let count = usize::try_from(word(0)).map_err(|_| bad("RLE segment count"))?;
    if count != samples * unit || count > 15 {
        return Err(bad("RLE segment count mismatch"));
    }
    let mut starts = [0usize; 16];
    for (i, start) in starts[..15].iter_mut().enumerate() {
        let value = word(4 + i * 4) as usize;
        if i >= count {
            if value != 0 {
                return Err(bad("unused RLE segment offset"));
            }
        } else {
            if value < 64 || value >= frame.len() || value % 2 != 0 {
                return Err(bad("RLE segment offset"));
            }
            *start = value;
        }
    }
    if starts[0] != 64 {
        return Err(bad("RLE first segment must follow header"));
    }
    starts[count] = frame.len();
    for range in starts[..=count].windows(2) {
        if range[0] >= range[1] {
            return Err(bad("unordered RLE segments"));
        }
        plane::<false>(
            &frame[range[0]..range[1]],
            layout.width as usize,
            pixels,
            request,
            |_, _| {},
        )?;
    }
    request.check_cancelled()?;
    let allocation = output + if pad { output % 2 } else { 0 };
    let mut result: PixelBuffer<u8> = match &request.memory_budget {
        Some(budget) => budget
            .try_buffer(allocation, 0u8)
            .map_err(crate::DecodeError::Memory)?
            .freeze()
            .into(),
        None => Arc::new(vec![0u8; allocation]).into(),
    };
    let out = result.get_mut().unwrap();
    for segment in 0..count {
        let channel = segment / unit;
        let byte = unit - 1 - segment % unit;
        plane::<true>(
            &frame[starts[segment]..starts[segment + 1]],
            layout.width as usize,
            pixels,
            request,
            |pixel, value| {
                let sample = if layout.planar {
                    channel * pixels + pixel
                } else {
                    pixel * samples + channel
                };
                out[sample * unit + byte] = value;
            },
        )?;
    }
    request.check_cancelled()?;
    Ok(result)
}
#[cfg(test)]
mod tests {
    use super::*;
    fn frame(segments: &[&[u8]]) -> Vec<u8> {
        let mut data = vec![0u8; 64];
        data[..4].copy_from_slice(&(segments.len() as u32).to_le_bytes());
        for (i, bytes) in segments.iter().enumerate() {
            let offset = data.len() as u32;
            data[4 + i * 4..8 + i * 4].copy_from_slice(&offset.to_le_bytes());
            data.extend_from_slice(bytes);
        }
        data
    }
    #[test]
    fn rows_runs_nop_padding_precision_layout_and_managed_admission() {
        let data = frame(&[&[2, 1, 2, 3, 254, 9]]);
        let layout = RleFrameLayout {
            width: 3,
            height: 2,
            samples: 1,
            bits: 8,
            planar: false,
        };
        let budget = rrrah_core::MemoryBudget::new(6);
        let mut request = DecodeRequest::new("frame.rle");
        request.memory_budget = Some(budget.clone());
        let output = decode_frame(&data, layout, &request).unwrap();
        assert_eq!(&*output, [1, 2, 3, 9, 9, 9]);
        assert_eq!(budget.peak(), 6);
        drop(output);
        assert_eq!(budget.used(), 0);
        let data = frame(&[&[128, 0, 7, 0]]);
        let one = RleFrameLayout {
            width: 1,
            height: 1,
            ..layout
        };
        assert_eq!(&*decode_frame(&data, one, &request).unwrap(), [7]);
        let data = frame(&[
            &[1, 0x12, 0x34, 0],
            &[1, 0xab, 0xcd, 0],
            &[255, 0x56],
            &[255, 0xef],
            &[255, 0x78],
            &[255, 0x01],
        ]);
        let rgb = RleFrameLayout {
            width: 2,
            height: 1,
            samples: 3,
            bits: 16,
            planar: false,
        };
        let request = DecodeRequest::new("rgb.rle");
        assert_eq!(
            &*decode_frame(&data, rgb, &request).unwrap(),
            [0xab, 0x12, 0xef, 0x56, 1, 0x78, 0xcd, 0x34, 0xef, 0x56, 1, 0x78]
        );
        assert_eq!(
            &*decode_frame(&data, RleFrameLayout { planar: true, ..rgb }, &request).unwrap(),
            [0xab, 0x12, 0xcd, 0x34, 0xef, 0x56, 0xef, 0x56, 1, 0x78, 1, 0x78]
        );
    }
    #[test]
    fn malformed_packets_and_headers_fail_before_output_admission() {
        let layout = RleFrameLayout {
            width: 3,
            height: 1,
            samples: 1,
            bits: 8,
            planar: false,
        };
        let budget = rrrah_core::MemoryBudget::new(0);
        let mut request = DecodeRequest::new("bad.rle");
        request.memory_budget = Some(budget.clone());
        for packet in [&[253, 9][..], &[2, 1], &[2, 1, 2, 3, 0, 1], &[128, 128], &[0, 1]] {
            assert!(matches!(
                decode_frame(&frame(&[packet]), layout, &request),
                Err(RasterDecodeError::InvalidDicom(_))
            ));
            assert_eq!(budget.peak(), 0);
        }
        let valid = frame(&[&[254, 9]]);
        for end in 0..valid.len() {
            assert!(matches!(
                decode_frame(&valid[..end], layout, &request),
                Err(RasterDecodeError::InvalidDicom(_))
            ));
        }
        for (at, value) in [(0, 2u32), (4, 66), (8, 64)] {
            let mut bad = valid.clone();
            bad[at..at + 4].copy_from_slice(&value.to_le_bytes());
            assert!(matches!(
                decode_frame(&bad, layout, &request),
                Err(RasterDecodeError::InvalidDicom(_))
            ));
        }
        request.memory_budget = Some(rrrah_core::MemoryBudget::new(2));
        assert!(decode_frame(&valid, layout, &request).is_err());
        assert_eq!(request.memory_budget.as_ref().unwrap().used(), 0);
        request.cancellation = Some(crate::GenerationToken::new(
            Arc::new(std::sync::atomic::AtomicU64::new(2)),
            1,
        ));
        assert!(matches!(
            decode_frame(&valid, layout, &request),
            Err(RasterDecodeError::Source(crate::DecodeError::Cancelled))
        ));
    }
    #[test]
    fn cancellation_during_tail_validation_is_observed() {
        let generation = Arc::new(std::sync::atomic::AtomicU64::new(1));
        let mut request = DecodeRequest::new("tail.rle");
        request.cancellation = Some(crate::GenerationToken::new(generation.clone(), 1));
        assert!(matches!(
            plane::<true>(&[0, 7, 128, 128], 1, 1, &request, |_, _| generation
                .store(2, std::sync::atomic::Ordering::Release)),
            Err(RasterDecodeError::Source(crate::DecodeError::Cancelled))
        ));
    }
    #[test]
    fn independent_pydicom_frame_oracles_all_planes_and_bounded_buffers() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/raster");
        for line in include_str!("../../../tests/fixtures/raster/dicom-rle-frame-manifest.tsv")
            .lines()
            .skip(1)
        {
            let f: Vec<_> = line.split('\t').collect();
            let width = f[1].parse().unwrap();
            let height = f[2].parse().unwrap();
            let samples = f[3].parse().unwrap();
            let bits = f[4].parse().unwrap();
            let source = std::fs::read(root.join(f[0])).unwrap();
            let bytes = u64::from(width) * u64::from(height) * u64::from(samples) * u64::from(bits / 8);
            for planar in [false, true] {
                let budget = rrrah_core::MemoryBudget::new(bytes);
                let mut request = DecodeRequest::new(f[0]);
                request.memory_budget = Some(budget.clone());
                let layout = RleFrameLayout {
                    width,
                    height,
                    samples,
                    bits,
                    planar,
                };
                let frame = decode_frame(&source, layout, &request).unwrap();
                assert_eq!(
                    frame.as_slice(),
                    std::fs::read(root.join(format!(
                        "{}.{}",
                        f[0],
                        if planar { "planar" } else { "interleaved" }
                    )))
                    .unwrap()
                );
                assert_eq!(budget.used(), bytes);
                drop(frame);
                assert_eq!(budget.used(), 0);
                assert_eq!(budget.peak(), bytes);
                request.memory_budget = Some(rrrah_core::MemoryBudget::new(bytes - 1));
                assert!(decode_frame(&source, layout, &request).is_err());
                assert_eq!(request.memory_budget.unwrap().used(), 0);
            }
        }
    }
    #[test]
    #[ignore = "external packaged RLE frames and independent pydicom samples"]
    fn packaged_rle_frames_match_all_native_component_bytes() {
        let root = std::path::PathBuf::from(std::env::var_os("RRRAH_RLE_ORACLES").expect("oracles"));
        for line in std::fs::read_to_string(root.join("manifest.tsv"))
            .unwrap()
            .lines()
        {
            let f: Vec<_> = line.split('\t').collect();
            let source = std::fs::read(root.join(format!("{}.rle", f[0]))).unwrap();
            let layout = RleFrameLayout {
                width: f[1].parse().unwrap(),
                height: f[2].parse().unwrap(),
                samples: f[3].parse().unwrap(),
                bits: f[4].parse().unwrap(),
                planar: false,
            };
            let expected = std::fs::read(root.join(format!("{}.pixels", f[0]))).unwrap();
            let budget = rrrah_core::MemoryBudget::new(expected.len() as u64);
            let mut request = DecodeRequest::new(f[0]);
            request.memory_budget = Some(budget.clone());
            let frame = decode_frame(&source, layout, &request).unwrap();
            assert_eq!(frame.as_slice(), expected);
            drop(frame);
            assert_eq!(budget.used(), 0);
            assert_eq!(budget.peak(), expected.len() as u64);
        }
    }
}
