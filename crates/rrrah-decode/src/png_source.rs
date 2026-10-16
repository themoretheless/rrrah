//! Zero-copy PNG stream excluding the natively handled EXIF chunk.
use crate::{DecodeRequest, raster::RasterDecodeError};
use image::metadata::Orientation;
use std::io::{self, BufRead, Read, Seek, SeekFrom};
#[derive(Debug)]
pub(super) struct PngSource<'a> {
    bytes: &'a [u8],
    excluded: Option<(usize, usize)>,
    position: u64,
    cancellation: Option<crate::GenerationToken>,
}
impl PngSource<'_> {
    fn len(&self) -> usize {
        self.bytes.len() - self.excluded.map_or(0, |(start, end)| end - start)
    }
}
impl BufRead for PngSource<'_> {
    fn fill_buf(&mut self) -> io::Result<&[u8]> {
        if self
            .cancellation
            .as_ref()
            .is_some_and(|token| token.is_cancelled())
        {
            return Err(io::Error::other(crate::DecodeError::Cancelled));
        }
        let Some(at) = usize::try_from(self.position).ok().filter(|&at| at < self.len()) else {
            return Ok(&[]);
        };
        let data = match self.excluded {
            Some((start, _)) if at < start => &self.bytes[at..start],
            Some((start, end)) => &self.bytes[at + (end - start)..],
            None => &self.bytes[at..],
        };
        Ok(&data[..data.len().min(65536)])
    }
    fn consume(&mut self, count: usize) {
        self.position = self.position.saturating_add(count as u64);
    }
}
impl Read for PngSource<'_> {
    fn read(&mut self, output: &mut [u8]) -> io::Result<usize> {
        let data = self.fill_buf()?;
        let count = data.len().min(output.len());
        output[..count].copy_from_slice(&data[..count]);
        self.consume(count);
        Ok(count)
    }
}
impl Seek for PngSource<'_> {
    fn seek(&mut self, from: SeekFrom) -> io::Result<u64> {
        self.position = match from {
            SeekFrom::Start(at) => at,
            SeekFrom::Current(delta) => self
                .position
                .checked_add_signed(delta)
                .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "PNG seek overflow"))?,
            SeekFrom::End(delta) => (self.len() as u64)
                .checked_add_signed(delta)
                .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "PNG seek overflow"))?,
        };
        Ok(self.position)
    }
}
/// Non-EXIF malformed chunk framing is deferred to the pixel decoder to retain
/// its existing admission/error order. EXIF is validated before it is excluded.
pub(super) fn prepare<'a>(
    bytes: &'a [u8],
    request: &DecodeRequest,
) -> Result<(PngSource<'a>, Orientation), RasterDecodeError> {
    let (mut source, orientation) =
        prepare_checked(bytes, &|| request.check_cancelled().map_err(Into::into))?;
    source.cancellation = request.cancellation.clone();
    Ok((source, orientation))
}
fn prepare_checked<'a>(
    bytes: &'a [u8],
    check: &dyn Fn() -> Result<(), RasterDecodeError>,
) -> Result<(PngSource<'a>, Orientation), RasterDecodeError> {
    let bad = |reason| RasterDecodeError::InvalidPngColor(reason);
    let mut at = 8usize;
    let mut excluded = None;
    let mut orientation = Orientation::NoTransforms;
    let mut seen_idat = false;
    while let Some(header) = bytes.get(at..at.saturating_add(8)) {
        check()?;
        let length = u32::from_be_bytes(header[..4].try_into().unwrap()) as usize;
        let kind = &header[4..8];
        let end = at.checked_add(12).and_then(|n| n.checked_add(length));
        let Some((end, chunk)) = end.and_then(|end| bytes.get(at..end).map(|chunk| (end, chunk))) else {
            if kind == b"eXIf" {
                return Err(bad("truncated PNG EXIF chunk"));
            }
            break;
        };
        if kind == b"eXIf" {
            if seen_idat {
                return Err(bad("PNG EXIF must precede IDAT"));
            }
            if excluded.is_some() {
                return Err(bad("duplicate PNG EXIF chunk"));
            }
            let mut crc = flate2::Crc::new();
            for part in chunk[4..8 + length].chunks(65536) {
                check()?;
                crc.update(part);
            }
            check()?;
            if crc.sum() != u32::from_be_bytes(chunk[8 + length..12 + length].try_into().unwrap()) {
                return Err(bad("PNG EXIF CRC mismatch"));
            }
            orientation =
                Orientation::from_exif_chunk(&chunk[8..8 + length]).unwrap_or(Orientation::NoTransforms);
            excluded = Some((at, end));
        }
        if kind == b"IDAT" {
            seen_idat = true;
        }
        if kind == b"IEND" {
            break;
        }
        at = end;
    }
    check()?;
    Ok((
        PngSource {
            bytes,
            excluded,
            position: 0,
            cancellation: None,
        },
        orientation,
    ))
}
#[cfg(test)]
mod tests {
    use super::*;
    fn source(before: bool) -> (Vec<u8>, Vec<u8>) {
        let base = include_bytes!("../../../tests/fixtures/raster/pattern.png");
        let exif = b"II\x2a\0\x08\0\0\0\x01\0\x12\x01\x03\0\x01\0\0\0\x06\0\0\0\0\0\0\0";
        let mut chunk = (exif.len() as u32).to_be_bytes().to_vec();
        chunk.extend_from_slice(b"eXIf");
        chunk.extend_from_slice(exif);
        let mut crc = flate2::Crc::new();
        crc.update(&chunk[4..]);
        chunk.extend_from_slice(&crc.sum().to_be_bytes());
        let split = if before { 33 } else { base.len() - 12 };
        let mut bytes = base[..split].to_vec();
        bytes.extend_from_slice(&chunk);
        bytes.extend_from_slice(&base[split..]);
        (bytes, chunk)
    }
    #[test]
    fn excludes_early_exif_rejects_late_and_preserves_read_seek_source() {
        for before in [false, true] {
            let (bytes, _) = source(before);
            let original = bytes.clone();
            let request = DecodeRequest::new("exif.png");
            if !before {
                assert!(matches!(
                    prepare(&bytes, &request),
                    Err(RasterDecodeError::InvalidPngColor("PNG EXIF must precede IDAT"))
                ));
                assert!(super::super::decode_raster_bytes(bytes.clone(), &request).is_err());
                assert_eq!(bytes, original);
                continue;
            }
            let (mut reader, orientation) = prepare(&bytes, &request).unwrap();
            assert_eq!(orientation, Orientation::Rotate90);
            let expected = include_bytes!("../../../tests/fixtures/raster/pattern.png");
            let mut output = Vec::new();
            reader.read_to_end(&mut output).unwrap();
            assert_eq!(&output[..], expected);
            for at in 0..expected.len() {
                reader.seek(SeekFrom::Start(at as u64)).unwrap();
                let mut byte = [0];
                reader.read_exact(&mut byte).unwrap();
                assert_eq!(byte[0], expected[at]);
            }
            assert_eq!(
                reader.seek(SeekFrom::End(-12)).unwrap(),
                (expected.len() - 12) as u64
            );
            let mut tail = Vec::new();
            reader.read_to_end(&mut tail).unwrap();
            assert_eq!(&tail[..], &expected[expected.len() - 12..]);
            assert!(reader.seek(SeekFrom::Start(u64::MAX)).is_ok());
            assert_eq!(reader.read(&mut [0]).unwrap(), 0);
            reader.seek(SeekFrom::Start(0)).unwrap();
            assert!(reader.seek(SeekFrom::Current(-1)).is_err());
            assert_eq!(bytes, original);
            let root = rrrah_core::MemoryBudget::new(2048);
            let mut managed = request.clone();
            managed.memory_budget = Some(root.clone());
            let frame = super::super::decode_raster_bytes(bytes.clone(), &managed).unwrap();
            let rrrah_core::RasterPixels::Rgba8(actual) = frame.pixels() else {
                panic!()
            };
            let reference = include_bytes!("../../../tests/fixtures/raster/pattern.png.rgba");
            for y in 0..16 {
                for x in 0..16 {
                    let target = (y * 16 + x) * 4;
                    let origin = ((15 - x) * 16 + y) * 4;
                    assert_eq!(&actual[target..target + 4], &reference[origin..origin + 4]);
                }
            }
            assert_eq!((root.used(), root.peak()), (1024, 2048));
            drop(frame);
            assert_eq!(root.used(), 0);
        }
    }
    #[test]
    fn duplicate_bad_crc_and_truncated_exif_are_refused() {
        let request = DecodeRequest::new("bad-exif.png");
        let (bytes, chunk) = source(true);
        let mut duplicate = bytes[..33].to_vec();
        duplicate.extend_from_slice(&chunk);
        duplicate.extend_from_slice(&bytes[33..]);
        assert!(matches!(
            prepare(&duplicate, &request),
            Err(RasterDecodeError::InvalidPngColor("duplicate PNG EXIF chunk"))
        ));
        let mut corrupt = bytes.clone();
        corrupt[33 + chunk.len() - 1] ^= 1;
        assert!(matches!(
            prepare(&corrupt, &request),
            Err(RasterDecodeError::InvalidPngColor("PNG EXIF CRC mismatch"))
        ));
        assert!(prepare(&bytes[..33 + chunk.len() - 1], &request).is_err());
    }
    #[test]
    fn cancellation_during_large_exif_crc_is_bounded_and_source_stays_unchanged() {
        let base = include_bytes!("../../../tests/fixtures/raster/pattern.png");
        let mut exif = b"II\x2a\0\x08\0\0\0\x01\0\x12\x01\x03\0\x01\0\0\0\x06\0\0\0\0\0\0\0".to_vec();
        exif.resize(4 * 1024 * 1024, 0);
        let mut chunk = (exif.len() as u32).to_be_bytes().to_vec();
        chunk.extend_from_slice(b"eXIf");
        chunk.extend_from_slice(&exif);
        let mut crc = flate2::Crc::new();
        crc.update(&chunk[4..]);
        chunk.extend_from_slice(&crc.sum().to_be_bytes());
        let mut bytes = base[..33].to_vec();
        bytes.extend_from_slice(&chunk);
        bytes.extend_from_slice(&base[33..]);
        let original = bytes.clone();
        let checks = std::cell::Cell::new(0);
        let result = prepare_checked(&bytes, &|| {
            checks.set(checks.get() + 1);
            if checks.get() == 4 {
                Err(crate::DecodeError::Cancelled.into())
            } else {
                Ok(())
            }
        });
        assert!(matches!(
            result,
            Err(RasterDecodeError::Source(crate::DecodeError::Cancelled))
        ));
        assert_eq!(checks.get(), 4);
        assert_eq!(bytes, original);
        let (_, orientation) = prepare_checked(&bytes, &|| Ok(())).unwrap();
        assert_eq!(orientation, Orientation::Rotate90);
    }
    #[test]
    fn input_chunks_are_bounded_and_cancelled_reads_do_not_retry_as_interrupted() {
        use std::sync::{
            Arc,
            atomic::{AtomicU64, Ordering},
        };
        let bytes = vec![17; 3 * 65536 + 19];
        let generation = Arc::new(AtomicU64::new(1));
        let mut source = PngSource {
            bytes: &bytes,
            excluded: None,
            position: 0,
            cancellation: Some(crate::GenerationToken::new(generation.clone(), 1)),
        };
        assert_eq!(source.fill_buf().unwrap().len(), 65536);
        source.consume(65536);
        assert_eq!(source.fill_buf().unwrap().len(), 65536);
        generation.store(2, Ordering::Release);
        assert_eq!(
            source.read(&mut [0; 16]).unwrap_err().kind(),
            io::ErrorKind::Other
        );
        assert_eq!(source.position, 65536);
    }
}
