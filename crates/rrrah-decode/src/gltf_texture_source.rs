//! Zero-copy virtual codec input excluding non-pixel image metadata.
use super::GltfTextureError;
use crate::{DecodeRequest, GenerationToken};
use rrrah_core::{MemoryBudget, PixelBuffer, Reservation};
use rrrah_mesh::glb::GlbImageMime;
use std::io::{self, BufRead, Read, Seek, SeekFrom};
use std::ops::Range;
const MAX_SEGMENTS: usize = 65_536;
#[derive(Debug, Clone, Copy)]
pub(super) struct Segment {
    start: usize,
    length: usize,
    virtual_start: usize,
}
#[derive(Debug, Clone, Copy)]
pub(super) struct Header {
    pub width: u32,
    pub height: u32,
    pub sixteen: bool,
}
fn scan(
    bytes: &[u8],
    mime: GlbImageMime,
    request: &DecodeRequest,
    validate_crc: bool,
    mut emit: impl FnMut(Range<usize>) -> Result<(), GltfTextureError>,
) -> Result<Header, GltfTextureError> {
    request.check_cancelled()?;
    if bytes.len() as u64 > crate::bounded_io::MAX_INPUT_BYTES {
        return Err(GltfTextureError::Limit("encoded image bytes"));
    }
    let bad = GltfTextureError::Invalid;
    match mime {
        GlbImageMime::Png => {
            if bytes.get(..8) != Some(b"\x89PNG\r\n\x1a\n") {
                return Err(bad("PNG MIME/signature"));
            }
            emit(0..8)?;
            let mut at = 8;
            let mut header = None;
            let mut chunks = 0;
            let mut seen_idat = false;
            let mut ended_idat = false;
            while at < bytes.len() {
                chunks += 1;
                if chunks > MAX_SEGMENTS {
                    return Err(GltfTextureError::Limit("PNG chunk count"));
                }
                request.check_cancelled()?;
                let head = bytes.get(at..at + 8).ok_or(bad("PNG chunk header"))?;
                let length = u32::from_be_bytes(head[..4].try_into().unwrap()) as usize;
                let end = at
                    .checked_add(12)
                    .and_then(|v| v.checked_add(length))
                    .filter(|&v| v <= bytes.len())
                    .ok_or(bad("PNG chunk extent"))?;
                let kind = &head[4..8];
                if !kind.iter().all(u8::is_ascii_alphabetic) || kind[2].is_ascii_lowercase() {
                    return Err(bad("PNG chunk type"));
                }
                if kind[0].is_ascii_uppercase() && !matches!(kind, b"IHDR" | b"PLTE" | b"IDAT" | b"IEND") {
                    return Err(GltfTextureError::Unsupported("PNG critical chunk"));
                }
                if kind == b"IHDR" && at != 8 {
                    return Err(bad("duplicate PNG IHDR"));
                }
                if kind == b"IDAT" {
                    if ended_idat {
                        return Err(bad("nonconsecutive PNG IDAT"));
                    }
                    seen_idat = true;
                } else if seen_idat {
                    ended_idat = true;
                }
                if seen_idat && matches!(kind, b"PLTE" | b"tRNS") {
                    return Err(bad("late PNG palette/alpha"));
                }
                if at == 8 {
                    if kind != b"IHDR" || length != 13 {
                        return Err(bad("PNG first IHDR"));
                    }
                    let d = &bytes[at + 8..at + 21];
                    let depth = d[8];
                    let color = d[9];
                    let valid = match color {
                        0 => matches!(depth, 1 | 2 | 4 | 8 | 16),
                        2 | 4 | 6 => matches!(depth, 8 | 16),
                        3 => matches!(depth, 1 | 2 | 4 | 8),
                        _ => false,
                    };
                    if !valid || d[10] != 0 || d[11] != 0 || d[12] > 1 {
                        return Err(bad("PNG IHDR layout"));
                    }
                    header = Some(Header {
                        width: u32::from_be_bytes(d[..4].try_into().unwrap()),
                        height: u32::from_be_bytes(d[4..8].try_into().unwrap()),
                        sixteen: depth == 16,
                    });
                }
                if kind[0].is_ascii_uppercase() || kind == b"tRNS" {
                    emit(at..end)?;
                }
                if validate_crc {
                    let mut crc = flate2::Crc::new();
                    for data in bytes[at + 4..end - 4].chunks(65536) {
                        request.check_cancelled()?;
                        crc.update(data);
                    }
                    if crc.sum() != u32::from_be_bytes(bytes[end - 4..end].try_into().unwrap()) {
                        return Err(bad("PNG chunk CRC"));
                    }
                }
                at = end;
                if kind == b"IEND" {
                    if length != 0 || end != bytes.len() || !seen_idat {
                        return Err(bad("PNG IEND/trailing bytes"));
                    }
                    return header.ok_or(bad("PNG header"));
                }
            }
            Err(bad("PNG missing IEND"))
        }
        GlbImageMime::Jpeg => {
            if bytes.get(..2) != Some(b"\xff\xd8") {
                return Err(bad("JPEG MIME/signature"));
            }
            emit(0..2)?;
            let mut at = 2;
            let mut header = None;
            let mut scan_seen = false;
            while at < bytes.len() {
                request.check_cancelled()?;
                let start = at;
                if bytes[at] != 255 {
                    return Err(bad("JPEG marker prefix"));
                }
                while bytes.get(at) == Some(&255) {
                    if at % 65536 == 0 {
                        request.check_cancelled()?;
                    }
                    at += 1;
                }
                let marker = *bytes.get(at).ok_or(bad("JPEG marker"))?;
                at += 1;
                if marker == 0xd9 {
                    emit(start..at)?;
                    if at != bytes.len() || !scan_seen {
                        return Err(bad("JPEG EOI/trailing bytes"));
                    }
                    return header.ok_or(bad("JPEG missing SOF"));
                }
                if matches!(marker, 0 | 0xd8 | 0xd0..=0xd7) {
                    return Err(bad("JPEG standalone marker"));
                }
                if marker == 1 {
                    emit(start..at)?;
                    continue;
                }
                let len = bytes.get(at..at + 2).ok_or(bad("JPEG segment length"))?;
                let len = u16::from_be_bytes(len.try_into().unwrap()) as usize;
                if len < 2 {
                    return Err(bad("JPEG short segment"));
                }
                let end = at
                    .checked_add(len)
                    .filter(|&v| v <= bytes.len())
                    .ok_or(bad("JPEG segment extent"))?;
                if matches!(marker, 0xc0..=0xcf) && !matches!(marker, 0xc4 | 0xc8 | 0xcc) {
                    if !matches!(marker, 0xc0 | 0xc1 | 0xc2) {
                        return Err(GltfTextureError::Unsupported("JPEG coding process"));
                    }
                    let data = &bytes[at + 2..end];
                    if data.len() < 6 {
                        return Err(bad("JPEG SOF"));
                    }
                    let components = data[5] as usize;
                    if data[0] != 8 || !matches!(components, 1 | 3) {
                        return Err(GltfTextureError::Unsupported("JPEG depth/component count"));
                    }
                    if data.len() != 6 + 3 * components || header.is_some() {
                        return Err(bad("JPEG SOF extent/duplicate"));
                    }
                    header = Some(Header {
                        width: u16::from_be_bytes([data[3], data[4]]) as u32,
                        height: u16::from_be_bytes([data[1], data[2]]) as u32,
                        sixteen: false,
                    });
                }
                if !matches!(marker, 0xe1..=0xed | 0xef | 0xfe) {
                    emit(start..end)?;
                }
                at = end;
                if marker == 0xda {
                    if header.is_none() {
                        return Err(bad("JPEG scan without SOF"));
                    }
                    scan_seen = true;
                    let start = at;
                    let mut next_check = at;
                    while at < bytes.len() {
                        if at >= next_check {
                            next_check = at.saturating_add(65536);
                            request.check_cancelled()?;
                        }
                        if bytes[at] != 255 {
                            at += 1;
                            continue;
                        }
                        let mark = at;
                        let mut next = at + 1;
                        while bytes.get(next) == Some(&255) {
                            if next % 65536 == 0 {
                                request.check_cancelled()?;
                            }
                            next += 1;
                        }
                        let value = *bytes.get(next).ok_or(bad("JPEG entropy marker"))?;
                        if value == 0 || matches!(value, 0xd0..=0xd7) {
                            at = next + 1;
                        } else {
                            at = mark;
                            break;
                        }
                    }
                    if at > start {
                        emit(start..at)?;
                    }
                }
            }
            Err(bad("JPEG missing EOI"))
        }
    }
}
pub(super) fn prepare(
    bytes: &[u8],
    mime: GlbImageMime,
    request: &DecodeRequest,
    budget: &MemoryBudget,
) -> Result<(PixelBuffer<Segment>, Header, usize), GltfTextureError> {
    let mut count = 0;
    let header = scan(bytes, mime, request, true, |_| {
        if count == MAX_SEGMENTS {
            return Err(GltfTextureError::Limit("image segment count"));
        }
        count += 1;
        Ok(())
    })?;
    let mut segments = budget.try_buffer(
        count,
        Segment {
            start: 0,
            length: 0,
            virtual_start: 0,
        },
    )?;
    let mut index = 0;
    let mut length = 0;
    scan(bytes, mime, request, false, |range| {
        segments[index] = Segment {
            start: range.start,
            length: range.len(),
            virtual_start: length,
        };
        length += range.len();
        index += 1;
        Ok(())
    })?;
    request.check_cancelled()?;
    Ok((segments.freeze().into(), header, length))
}
#[derive(Debug)]
pub(super) struct FilteredSource<'a, 'b> {
    bytes: &'a [u8],
    segments: PixelBuffer<Segment>,
    length: usize,
    position: u64,
    cancel: Option<GenerationToken>,
    input_credit: Option<&'b mut Reservation>,
}
impl<'a, 'b> FilteredSource<'a, 'b> {
    pub fn new(
        bytes: &'a [u8],
        segments: PixelBuffer<Segment>,
        length: usize,
        cancel: Option<GenerationToken>,
        input_credit: Option<&'b mut Reservation>,
    ) -> Self {
        Self {
            bytes,
            segments,
            length,
            position: 0,
            cancel,
            input_credit,
        }
    }
}
impl BufRead for FilteredSource<'_, '_> {
    fn fill_buf(&mut self) -> io::Result<&[u8]> {
        if self.cancel.as_ref().is_some_and(GenerationToken::is_cancelled) {
            return Err(io::Error::other(crate::DecodeError::Cancelled));
        }
        let Some(at) = usize::try_from(self.position).ok().filter(|&v| v < self.length) else {
            return Ok(&[]);
        };
        let index = self.segments.partition_point(|s| s.virtual_start <= at) - 1;
        let segment = self.segments[index];
        let delta = at - segment.virtual_start;
        let count = (segment.length - delta).min(65536);
        Ok(&self.bytes[segment.start + delta..segment.start + delta + count])
    }
    fn consume(&mut self, count: usize) {
        self.position = self.position.saturating_add(count as u64);
    }
}
impl Read for FilteredSource<'_, '_> {
    fn read(&mut self, out: &mut [u8]) -> io::Result<usize> {
        let data = self.fill_buf()?;
        let count = data.len().min(out.len());
        out[..count].copy_from_slice(&data[..count]);
        self.consume(count);
        Ok(count)
    }
    fn read_to_end(&mut self, out: &mut Vec<u8>) -> io::Result<usize> {
        let start = out.len();
        let remaining = (self.length as u64).saturating_sub(self.position) as usize;
        out.try_reserve_exact(remaining).map_err(io::Error::other)?;
        if let Some(credit) = &mut self.input_credit {
            credit
                .ensure_bytes(out.capacity() as u64)
                .map_err(io::Error::other)?;
        }
        loop {
            let data = self.fill_buf()?;
            if data.is_empty() {
                break;
            }
            let count = data.len();
            out.extend_from_slice(data);
            self.consume(count);
        }
        Ok(out.len() - start)
    }
}
impl Seek for FilteredSource<'_, '_> {
    fn seek(&mut self, from: SeekFrom) -> io::Result<u64> {
        self.position = match from {
            SeekFrom::Start(at) => at,
            SeekFrom::Current(delta) => self
                .position
                .checked_add_signed(delta)
                .ok_or_else(|| io::Error::other("image seek overflow"))?,
            SeekFrom::End(delta) => (self.length as u64)
                .checked_add_signed(delta)
                .ok_or_else(|| io::Error::other("image seek overflow"))?,
        };
        Ok(self.position)
    }
}
