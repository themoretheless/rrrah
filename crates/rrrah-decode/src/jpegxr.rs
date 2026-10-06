//! Bounded JPEG-XR Annex-A container and native sample import.
use crate::{
    DecodeRequest,
    raster::{MAX_RASTER_BYTES, RasterDecodeError},
};
use jpegxr::PixelFormat;
use rrrah_core::{DecodedRaster, RasterColorSpace, RasterPixels};
use std::{
    collections::BTreeMap,
    io::{Cursor, Read, Seek, SeekFrom},
    sync::Arc,
};
fn bad(s: &'static str) -> RasterDecodeError {
    RasterDecodeError::InvalidJpegXr(s)
}
pub(crate) fn has_magic(b: &[u8]) -> bool {
    b.starts_with(b"II\xbc\0") || b.starts_with(b"II\xbc\x01")
}
fn u16le(b: &[u8]) -> u16 {
    u16::from_le_bytes(b.try_into().unwrap())
}
fn u32le(b: &[u8]) -> u32 {
    u32::from_le_bytes(b.try_into().unwrap())
}
#[derive(Clone, Copy)]
enum Sample {
    U8,
    U16,
    F16,
    F32,
}
#[derive(Clone, Copy)]
struct Layout {
    format: PixelFormat,
    sample: Sample,
    channels: usize,
    bytes: usize,
    bgr: bool,
    premul: bool,
}
impl Layout {
    fn float(self) -> bool {
        matches!(self.sample, Sample::F16 | Sample::F32)
    }
}
// GUID whitelist is checked before entering native PixelFormatLookup.
fn layout(guid: &[u8]) -> Result<Layout, RasterDecodeError> {
    if guid
        == [
            0x24, 0xc3, 0xdd, 0x6f, 0x03, 0x4e, 0xfe, 0x4b, 0xb1, 0x85, 0x3d, 0x77, 0x76, 0x8d, 0xc9, 0x08,
        ]
    {
        return Ok(Layout {
            format: PixelFormat::PixelFormat8bppGray,
            sample: Sample::U8,
            channels: 1,
            bytes: 1,
            bgr: false,
            premul: false,
        });
    }
    if guid
        == [
            0x24, 0xc3, 0xdd, 0x6f, 0x03, 0x4e, 0xfe, 0x4b, 0xb1, 0x85, 0x3d, 0x77, 0x76, 0x8d, 0xc9, 0x0b,
        ]
    {
        return Ok(Layout {
            format: PixelFormat::PixelFormat16bppGray,
            sample: Sample::U16,
            channels: 1,
            bytes: 2,
            bgr: false,
            premul: false,
        });
    }
    if guid
        == [
            0x24, 0xc3, 0xdd, 0x6f, 0x03, 0x4e, 0xfe, 0x4b, 0xb1, 0x85, 0x3d, 0x77, 0x76, 0x8d, 0xc9, 0x11,
        ]
    {
        return Ok(Layout {
            format: PixelFormat::PixelFormat32bppGrayFloat,
            sample: Sample::F32,
            channels: 1,
            bytes: 4,
            bgr: false,
            premul: false,
        });
    }
    if guid
        == [
            0x24, 0xc3, 0xdd, 0x6f, 0x03, 0x4e, 0xfe, 0x4b, 0xb1, 0x85, 0x3d, 0x77, 0x76, 0x8d, 0xc9, 0x3e,
        ]
    {
        return Ok(Layout {
            format: PixelFormat::PixelFormat16bppGrayHalf,
            sample: Sample::F16,
            channels: 1,
            bytes: 2,
            bgr: false,
            premul: false,
        });
    }
    if guid
        == [
            0x24, 0xc3, 0xdd, 0x6f, 0x03, 0x4e, 0xfe, 0x4b, 0xb1, 0x85, 0x3d, 0x77, 0x76, 0x8d, 0xc9, 0x0d,
        ]
    {
        return Ok(Layout {
            format: PixelFormat::PixelFormat24bppRGB,
            sample: Sample::U8,
            channels: 3,
            bytes: 3,
            bgr: false,
            premul: false,
        });
    }
    if guid
        == [
            0x24, 0xc3, 0xdd, 0x6f, 0x03, 0x4e, 0xfe, 0x4b, 0xb1, 0x85, 0x3d, 0x77, 0x76, 0x8d, 0xc9, 0x0c,
        ]
    {
        return Ok(Layout {
            format: PixelFormat::PixelFormat24bppBGR,
            sample: Sample::U8,
            channels: 3,
            bytes: 3,
            bgr: true,
            premul: false,
        });
    }
    if guid
        == [
            0x95, 0x6b, 0x8c, 0xd9, 0xfe, 0x3e, 0xd6, 0x47, 0xbb, 0x25, 0xeb, 0x17, 0x48, 0xab, 0x0c, 0xf1,
        ]
    {
        return Ok(Layout {
            format: PixelFormat::PixelFormat32bppRGB,
            sample: Sample::U8,
            channels: 3,
            bytes: 4,
            bgr: false,
            premul: false,
        });
    }
    if guid
        == [
            0x24, 0xc3, 0xdd, 0x6f, 0x03, 0x4e, 0xfe, 0x4b, 0xb1, 0x85, 0x3d, 0x77, 0x76, 0x8d, 0xc9, 0x0e,
        ]
    {
        return Ok(Layout {
            format: PixelFormat::PixelFormat32bppBGR,
            sample: Sample::U8,
            channels: 3,
            bytes: 4,
            bgr: true,
            premul: false,
        });
    }
    if guid
        == [
            0x2d, 0xad, 0xc7, 0xf5, 0x8d, 0x6a, 0xdd, 0x43, 0xa7, 0xa8, 0xa2, 0x99, 0x35, 0x26, 0x1a, 0xe9,
        ]
    {
        return Ok(Layout {
            format: PixelFormat::PixelFormat32bppRGBA,
            sample: Sample::U8,
            channels: 4,
            bytes: 4,
            bgr: false,
            premul: false,
        });
    }
    if guid
        == [
            0x24, 0xc3, 0xdd, 0x6f, 0x03, 0x4e, 0xfe, 0x4b, 0xb1, 0x85, 0x3d, 0x77, 0x76, 0x8d, 0xc9, 0x0f,
        ]
    {
        return Ok(Layout {
            format: PixelFormat::PixelFormat32bppBGRA,
            sample: Sample::U8,
            channels: 4,
            bytes: 4,
            bgr: true,
            premul: false,
        });
    }
    if guid
        == [
            0x50, 0xa6, 0xc4, 0x3c, 0x27, 0xa5, 0x37, 0x4d, 0xa9, 0x16, 0x31, 0x42, 0xc7, 0xeb, 0xed, 0xba,
        ]
    {
        return Ok(Layout {
            format: PixelFormat::PixelFormat32bppPRGBA,
            sample: Sample::U8,
            channels: 4,
            bytes: 4,
            bgr: false,
            premul: true,
        });
    }
    if guid
        == [
            0x24, 0xc3, 0xdd, 0x6f, 0x03, 0x4e, 0xfe, 0x4b, 0xb1, 0x85, 0x3d, 0x77, 0x76, 0x8d, 0xc9, 0x10,
        ]
    {
        return Ok(Layout {
            format: PixelFormat::PixelFormat32bppPBGRA,
            sample: Sample::U8,
            channels: 4,
            bytes: 4,
            bgr: true,
            premul: true,
        });
    }
    if guid
        == [
            0x24, 0xc3, 0xdd, 0x6f, 0x03, 0x4e, 0xfe, 0x4b, 0xb1, 0x85, 0x3d, 0x77, 0x76, 0x8d, 0xc9, 0x15,
        ]
    {
        return Ok(Layout {
            format: PixelFormat::PixelFormat48bppRGB,
            sample: Sample::U16,
            channels: 3,
            bytes: 6,
            bgr: false,
            premul: false,
        });
    }
    if guid
        == [
            0x24, 0xc3, 0xdd, 0x6f, 0x03, 0x4e, 0xfe, 0x4b, 0xb1, 0x85, 0x3d, 0x77, 0x76, 0x8d, 0xc9, 0x16,
        ]
    {
        return Ok(Layout {
            format: PixelFormat::PixelFormat64bppRGBA,
            sample: Sample::U16,
            channels: 4,
            bytes: 8,
            bgr: false,
            premul: false,
        });
    }
    if guid
        == [
            0x24, 0xc3, 0xdd, 0x6f, 0x03, 0x4e, 0xfe, 0x4b, 0xb1, 0x85, 0x3d, 0x77, 0x76, 0x8d, 0xc9, 0x17,
        ]
    {
        return Ok(Layout {
            format: PixelFormat::PixelFormat64bppPRGBA,
            sample: Sample::U16,
            channels: 4,
            bytes: 8,
            bgr: false,
            premul: true,
        });
    }
    if guid
        == [
            0x24, 0xc3, 0xdd, 0x6f, 0x03, 0x4e, 0xfe, 0x4b, 0xb1, 0x85, 0x3d, 0x77, 0x76, 0x8d, 0xc9, 0x3b,
        ]
    {
        return Ok(Layout {
            format: PixelFormat::PixelFormat48bppRGBHalf,
            sample: Sample::F16,
            channels: 3,
            bytes: 6,
            bgr: false,
            premul: false,
        });
    }
    if guid
        == [
            0x24, 0xc3, 0xdd, 0x6f, 0x03, 0x4e, 0xfe, 0x4b, 0xb1, 0x85, 0x3d, 0x77, 0x76, 0x8d, 0xc9, 0x42,
        ]
    {
        return Ok(Layout {
            format: PixelFormat::PixelFormat64bppRGBHalf,
            sample: Sample::F16,
            channels: 3,
            bytes: 8,
            bgr: false,
            premul: false,
        });
    }
    if guid
        == [
            0x24, 0xc3, 0xdd, 0x6f, 0x03, 0x4e, 0xfe, 0x4b, 0xb1, 0x85, 0x3d, 0x77, 0x76, 0x8d, 0xc9, 0x3a,
        ]
    {
        return Ok(Layout {
            format: PixelFormat::PixelFormat64bppRGBAHalf,
            sample: Sample::F16,
            channels: 4,
            bytes: 8,
            bgr: false,
            premul: false,
        });
    }
    if guid
        == [
            0x8f, 0xd7, 0xfe, 0xe3, 0xdb, 0xe8, 0xcf, 0x4a, 0x84, 0xc1, 0xe9, 0x7f, 0x61, 0x36, 0xb3, 0x27,
        ]
    {
        return Ok(Layout {
            format: PixelFormat::PixelFormat96bppRGBFloat,
            sample: Sample::F32,
            channels: 3,
            bytes: 12,
            bgr: false,
            premul: false,
        });
    }
    if guid
        == [
            0x24, 0xc3, 0xdd, 0x6f, 0x03, 0x4e, 0xfe, 0x4b, 0xb1, 0x85, 0x3d, 0x77, 0x76, 0x8d, 0xc9, 0x1b,
        ]
    {
        return Ok(Layout {
            format: PixelFormat::PixelFormat128bppRGBFloat,
            sample: Sample::F32,
            channels: 3,
            bytes: 16,
            bgr: false,
            premul: false,
        });
    }
    if guid
        == [
            0x24, 0xc3, 0xdd, 0x6f, 0x03, 0x4e, 0xfe, 0x4b, 0xb1, 0x85, 0x3d, 0x77, 0x76, 0x8d, 0xc9, 0x19,
        ]
    {
        return Ok(Layout {
            format: PixelFormat::PixelFormat128bppRGBAFloat,
            sample: Sample::F32,
            channels: 4,
            bytes: 16,
            bgr: false,
            premul: false,
        });
    }
    if guid
        == [
            0x24, 0xc3, 0xdd, 0x6f, 0x03, 0x4e, 0xfe, 0x4b, 0xb1, 0x85, 0x3d, 0x77, 0x76, 0x8d, 0xc9, 0x1a,
        ]
    {
        return Ok(Layout {
            format: PixelFormat::PixelFormat128bppPRGBAFloat,
            sample: Sample::F32,
            channels: 4,
            bytes: 16,
            bgr: false,
            premul: true,
        });
    }
    Err(bad("unsupported pixel-format GUID"))
}
// Check both primary and separate-alpha headers before native allocation.
fn validate_stream(
    data: &[u8],
    w: u32,
    h: u32,
    layout: Layout,
    alpha: bool,
) -> Result<(), RasterDecodeError> {
    if data.len() < 16
        || &data[..8] != b"WMPHOTO\0"
        || data[8] >> 4 != 1
        || !matches!(data[8] & 15, 0 | 1 | 9)
        || data[9] & 3 == 3
    {
        return Err(bad("codestream header"));
    }
    let depth = match layout.sample {
        Sample::U8 => 1,
        Sample::U16 => 2,
        Sample::F16 => 4,
        Sample::F32 => 7,
    };
    if data[11] & 15 != depth || data[11] >> 4 != if alpha || layout.channels == 1 { 0 } else { 7 } {
        return Err(bad("codestream sample format"));
    }
    let mut bit = 96usize;
    let mut read = |n: usize| -> Result<u32, RasterDecodeError> {
        let mut v = 0u32;
        for _ in 0..n {
            let b = *data
                .get(bit / 8)
                .ok_or_else(|| bad("truncated codestream header"))?;
            v = (v << 1) | u32::from((b >> (7 - bit % 8)) & 1);
            bit += 1;
        }
        Ok(v)
    };
    let short = data[10] & 128 != 0;
    let bits = if short { 16 } else { 32 };
    let rw = u64::from(read(bits)?) + 1;
    let rh = u64::from(read(bits)?) + 1;
    if rw > u64::from(w) + 126 || rh > u64::from(h) + 126 || rw == 0 || rh == 0 {
        return Err(bad("oversized codestream dimensions"));
    }
    if data[9] & 128 != 0 {
        let nv = u64::from(read(12)?) + 1;
        let nh = u64::from(read(12)?) + 1;
        if nv > rw.div_ceil(16) || nh > rh.div_ceil(16) || nv * nh > 1_048_576 {
            return Err(bad("tile dimensions/count"));
        }
    }
    Ok(())
}
struct Metadata {
    w: u32,
    h: u32,
    orientation: u32,
    orientation_at: Option<usize>,
    layout: Layout,
    profile: Option<Vec<u8>>,
    alpha: Option<(usize, usize)>,
}
fn metadata(b: &[u8], request: &DecodeRequest) -> Result<Metadata, RasterDecodeError> {
    if !has_magic(b) || b.len() < 8 {
        return Err(bad("Annex-A signature"));
    }
    let at = u32le(&b[4..8]) as usize;
    let count = b
        .get(at..at.saturating_add(2))
        .map(u16le)
        .ok_or_else(|| bad("directory offset"))? as usize;
    if count == 0 || count > 4096 {
        return Err(bad("directory count"));
    }
    let end = at
        .checked_add(2 + count * 12 + 4)
        .filter(|&v| v <= b.len())
        .ok_or_else(|| bad("directory bounds"))?;
    if u32le(&b[end - 4..end]) != 0 {
        return Err(bad("multiple image directories unsupported"));
    }
    let mut entries = BTreeMap::new();
    let mut orientation_at = None;
    for (index, e) in b[at + 2..end - 4].chunks_exact(12).enumerate() {
        request.check_cancelled()?;
        let tag = u16le(&e[..2]);
        if tag == 0xbc02 {
            orientation_at = Some(at + 2 + index * 12 + 8);
        }
        let typ = u16le(&e[2..4]);
        let n = u32le(&e[4..8]) as usize;
        let unit = match typ {
            1 | 2 | 6 | 7 => 1,
            3 | 8 => 2,
            4 | 9 | 11 => 4,
            5 | 10 | 12 => 8,
            _ => return Err(bad("directory field type")),
        };
        let size = n.checked_mul(unit).ok_or_else(|| bad("field size"))?;
        let data = if size <= 4 {
            &e[8..8 + size]
        } else {
            let offset = u32le(&e[8..12]) as usize;
            let end = offset
                .checked_add(size)
                .filter(|&v| v <= b.len())
                .ok_or_else(|| bad("field bounds"))?;
            &b[offset..end]
        };
        if entries.insert(tag, (typ, n, data, u32le(&e[8..12]))).is_some() {
            return Err(bad("duplicate directory tag"));
        }
    }
    let scalar = |tag: u16| -> Result<u32, RasterDecodeError> {
        let &(typ, n, data, _) = entries.get(&tag).ok_or_else(|| bad("missing required tag"))?;
        if n != 1 {
            return Err(bad("scalar tag count"));
        }
        match typ {
            1 => Ok(u32::from(data[0])),
            3 => Ok(u32::from(u16le(data))),
            4 => Ok(u32le(data)),
            _ => Err(bad("scalar tag type")),
        }
    };
    let (w, h) = (scalar(0xbc80)?, scalar(0xbc81)?);
    if w == 0 || h == 0 || w > 65536 || h > 65536 || u64::from(w) * u64::from(h) * 64 > MAX_RASTER_BYTES {
        return Err(RasterDecodeError::OutputTooLarge);
    }
    let &(typ, n, guid, _) = entries.get(&0xbc01).ok_or_else(|| bad("missing pixel format"))?;
    if !matches!(typ, 1 | 7) || n != 16 {
        return Err(bad("pixel format type/count"));
    }
    let layout = layout(guid)?;
    let orientation = if entries.contains_key(&0xbc02) {
        scalar(0xbc02)?
    } else {
        0
    };
    if orientation > 7 {
        return Err(bad("orientation range"));
    }
    let range = |offset: u16, count: u16| -> Result<(usize, usize), RasterDecodeError> {
        let start = scalar(offset)? as usize;
        let n = if entries.contains_key(&count) {
            scalar(count)? as usize
        } else {
            0
        };
        let end = if count == 0xbcc1 && n == 0 {
            b.len()
        } else if count == 0xbcc3 && n == b.len() {
            n
        } else {
            start.checked_add(n).ok_or_else(|| bad("codestream range"))?
        };
        if start < 8 || start >= end || end > b.len() || (start < at + 2 + entries.len() * 12 + 4 && end > at)
        {
            return Err(bad("codestream range/directory overlap"));
        }
        Ok((start, end))
    };
    let image = range(0xbcc0, 0xbcc1)?;
    validate_stream(&b[image.0..image.1], w, h, layout, false)?;
    let alpha = if entries.contains_key(&0xbcc2) || entries.contains_key(&0xbcc3) {
        if !entries.contains_key(&0xbcc2) || !entries.contains_key(&0xbcc3) || layout.channels != 4 {
            return Err(bad("alpha storage tags"));
        }
        let alpha = range(0xbcc2, 0xbcc3)?;
        if alpha.0 < image.1 && image.0 < alpha.1 {
            return Err(bad("overlapping image/alpha storage"));
        }
        validate_stream(&b[alpha.0..alpha.1], w, h, layout, true)?;
        Some(alpha)
    } else {
        if layout.channels == 4 {
            return Err(bad("interleaved alpha unsupported"));
        }
        None
    };
    let profile = entries
        .get(&0x8773)
        .map(|&(typ, n, data, _)| {
            if !matches!(typ, 1 | 7) || n == 0 || n > 16 * 1024 * 1024 {
                return Err(bad("ICC type/size"));
            }
            Ok(data.to_vec())
        })
        .transpose()?;
    Ok(Metadata {
        w,
        h,
        orientation,
        orientation_at,
        layout,
        profile,
        alpha,
    })
}
// Patch the inline orientation in the borrowed stream: no compressed-file copy.
struct UnorientedReader<'a> {
    inner: Cursor<&'a [u8]>,
    at: Option<usize>,
}
impl Read for UnorientedReader<'_> {
    fn read(&mut self, out: &mut [u8]) -> std::io::Result<usize> {
        let start = self.inner.position() as usize;
        let n = self.inner.read(out)?;
        if let Some(at) = self.at {
            let first = at.max(start);
            let last = (at + 4).min(start + n);
            if first < last {
                out[first - start..last - start].fill(0);
            }
        }
        Ok(n)
    }
}
impl Seek for UnorientedReader<'_> {
    fn seek(&mut self, pos: SeekFrom) -> std::io::Result<u64> {
        self.inner.seek(pos)
    }
}
// A small synthetic gray container views the separate alpha stream without
// copying compressed bytes. The native wrapper does not expose its alpha mode.
struct PlaneReader<'a> {
    header: Vec<u8>,
    body: &'a [u8],
    position: u64,
}
impl Read for PlaneReader<'_> {
    fn read(&mut self, out: &mut [u8]) -> std::io::Result<usize> {
        let total = self.header.len() + self.body.len();
        let mut at = usize::try_from(self.position).unwrap_or(usize::MAX);
        if at >= total {
            return Ok(0);
        }
        let n = out.len().min(total - at);
        let mut done = 0;
        if at < self.header.len() {
            let k = n.min(self.header.len() - at);
            out[..k].copy_from_slice(&self.header[at..at + k]);
            done = k;
            at += k;
        }
        if done < n {
            let at = at - self.header.len();
            out[done..n].copy_from_slice(&self.body[at..at + n - done]);
        }
        self.position += n as u64;
        Ok(n)
    }
}
impl Seek for PlaneReader<'_> {
    fn seek(&mut self, pos: SeekFrom) -> std::io::Result<u64> {
        let at = match pos {
            SeekFrom::Start(n) => i128::from(n),
            SeekFrom::Current(n) => i128::from(self.position) + i128::from(n),
            SeekFrom::End(n) => (self.header.len() + self.body.len()) as i128 + i128::from(n),
        };
        self.position = u64::try_from(at)
            .map_err(|_| std::io::Error::new(std::io::ErrorKind::InvalidInput, "invalid seek"))?;
        Ok(self.position)
    }
}
fn decode_alpha(
    b: &[u8],
    m: &Metadata,
    request: &DecodeRequest,
) -> Result<Option<Vec<u8>>, RasterDecodeError> {
    let Some((start, end)) = m.alpha else {
        return Ok(None);
    };
    let (guid_end, bytes) = match m.layout.sample {
        Sample::U8 => (0x08, 1),
        Sample::U16 => (0x0b, 2),
        Sample::F16 => (0x3e, 2),
        Sample::F32 => (0x11, 4),
    };
    let guid = [
        0x24, 0xc3, 0xdd, 0x6f, 0x03, 0x4e, 0xfe, 0x4b, 0xb1, 0x85, 0x3d, 0x77, 0x76, 0x8d, 0xc9, guid_end,
    ];
    let mut header = b"II\xbc\0".to_vec();
    header.extend_from_slice(&24u32.to_le_bytes());
    header.extend_from_slice(&guid);
    header.extend_from_slice(&5u16.to_le_bytes());
    for (tag, typ, n, value) in [
        (0xbc01u16, 1u16, 16u32, 8u32),
        (0xbc80, 4, 1, m.w),
        (0xbc81, 4, 1, m.h),
        (0xbcc0, 4, 1, 90),
        (0xbcc1, 4, 1, (end - start) as u32),
    ] {
        header.extend_from_slice(&tag.to_le_bytes());
        header.extend_from_slice(&typ.to_le_bytes());
        header.extend_from_slice(&n.to_le_bytes());
        header.extend_from_slice(&value.to_le_bytes());
    }
    header.extend_from_slice(&0u32.to_le_bytes());
    debug_assert_eq!(header.len(), 90);
    let mut decoder = jpegxr::ImageDecode::with_reader(PlaneReader {
        header,
        body: &b[start..end],
        position: 0,
    })?;
    if decoder.get_size()? != (m.w as i32, m.h as i32) || decoder.get_pixel_format()? != layout(&guid)?.format
    {
        return Err(bad("alpha dimensions/pixel format disagree"));
    }
    let mut pixels = vec![0; m.w as usize * m.h as usize * bytes];
    request.check_cancelled()?;
    decoder.copy_all(&mut pixels, m.w as usize * bytes)?;
    request.check_cancelled()?;
    Ok(Some(pixels))
}
fn orient<T: Copy>(source: Vec<T>, w: usize, h: usize, o: u32) -> Vec<T> {
    if o == 0 {
        return source;
    }
    let mut out = source.clone();
    let width = if o >= 4 { h } else { w };
    for y in 0..h {
        for x in 0..w {
            let (dx, dy) = if o >= 4 {
                (
                    if matches!(o, 4 | 5) { h - 1 - y } else { y },
                    if matches!(o, 5 | 7) { w - 1 - x } else { x },
                )
            } else {
                (
                    if o & 2 != 0 { w - 1 - x } else { x },
                    if o & 1 != 0 { h - 1 - y } else { y },
                )
            };
            out[(dy * width + dx) * 4..(dy * width + dx + 1) * 4]
                .copy_from_slice(&source[(y * w + x) * 4..(y * w + x + 1) * 4]);
        }
    }
    out
}
pub(crate) fn decode(b: &[u8], request: &DecodeRequest) -> Result<DecodedRaster, RasterDecodeError> {
    request.check_cancelled()?;
    let metadata = metadata(b, request)?;
    let layout = metadata.layout;
    let mut decoder = jpegxr::ImageDecode::with_reader(UnorientedReader {
        inner: Cursor::new(b),
        at: metadata.orientation_at,
    })?;
    match decoder.get_pixel_format() {
        Ok(format) if format == layout.format => {}
        // jpegxr 0.3.1's Rust reverse lookup accidentally registers the UINT48
        // GUID for RGBHalf48. The complete container GUID was already checked;
        // native JXRLib uses the correct GUID and six-byte HALF layout.
        Err(jpegxr::JXRError::UnrecognizedPixelFormat)
            if layout.format == PixelFormat::PixelFormat48bppRGBHalf => {}
        Err(e) => return Err(e.into()),
        _ => return Err(bad("native pixel format disagrees")),
    }
    let (w, h) = decoder.get_size()?;
    let expected = if metadata.orientation >= 4 {
        (metadata.h, metadata.w)
    } else {
        (metadata.w, metadata.h)
    };
    if (w, h) != (metadata.w as i32, metadata.h as i32) {
        return Err(bad("codestream/container dimensions disagree"));
    }
    let pixels = w as usize * h as usize;
    let stride = w as usize * layout.bytes;
    let mut native = vec![0; pixels * layout.bytes];
    request.check_cancelled()?;
    decoder.copy_all(&mut native, stride)?;
    request.check_cancelled()?;
    let alpha = decode_alpha(b, &metadata, request)?;
    if let Some(alpha) = alpha {
        let bytes = match layout.sample {
            Sample::U8 => 1,
            Sample::U16 | Sample::F16 => 2,
            Sample::F32 => 4,
        };
        for (raw, a) in native
            .chunks_exact_mut(layout.bytes)
            .zip(alpha.chunks_exact(bytes))
        {
            raw[3 * bytes..4 * bytes].copy_from_slice(a);
        }
    }
    let color = metadata.profile.map_or(
        if layout.float() {
            RasterColorSpace::LinearSrgb
        } else {
            RasterColorSpace::Srgb
        },
        RasterColorSpace::Icc,
    );
    let scalar = |raw: &[u8], c: usize| -> f32 {
        match layout.sample {
            Sample::U8 => f32::from(raw[c]) / 255.,
            Sample::U16 => f32::from(u16le(&raw[c * 2..c * 2 + 2])) / 65535.,
            Sample::F16 => crate::sample::half_to_f32(u16le(&raw[c * 2..c * 2 + 2])),
            Sample::F32 => f32::from_le_bytes(raw[c * 4..c * 4 + 4].try_into().unwrap()),
        }
    };
    let mut rgba8 = Vec::new();
    let mut rgba16 = Vec::new();
    let mut rgba_float = Vec::new();
    if layout.float() || layout.premul {
        rgba_float.reserve(pixels * 4);
    } else if matches!(layout.sample, Sample::U8) {
        rgba8.reserve(pixels * 4);
    } else {
        rgba16.reserve(pixels * 4);
    }
    for (index, raw) in native.chunks_exact(layout.bytes).enumerate() {
        if index % 65536 == 0 {
            request.check_cancelled()?;
        }
        let indices = if layout.channels == 1 {
            [0, 0, 0]
        } else if layout.bgr {
            [2, 1, 0]
        } else {
            [0, 1, 2]
        };
        if layout.float() || layout.premul {
            let mut p = [
                scalar(raw, indices[0]),
                scalar(raw, indices[1]),
                scalar(raw, indices[2]),
                if layout.channels == 4 { scalar(raw, 3) } else { 1. },
            ];
            if p.iter().any(|v| !v.is_finite()) || !(0.0..=1.0).contains(&p[3]) {
                return Err(bad("nonfinite samples or invalid alpha"));
            }
            if layout.premul {
                if p[3] == 0. {
                    if p[..3].iter().any(|&v| v != 0.) {
                        return Err(bad("zero-alpha associated emission"));
                    }
                } else {
                    for c in 0..3 {
                        p[c] /= p[3];
                        if !p[c].is_finite() {
                            return Err(bad("associated RGB overflow"));
                        }
                    }
                }
            }
            rgba_float.extend_from_slice(&p);
        } else if matches!(layout.sample, Sample::U8) {
            rgba8.extend_from_slice(&[
                raw[indices[0]],
                raw[indices[1]],
                raw[indices[2]],
                if layout.channels == 4 { raw[3] } else { 255 },
            ]);
        } else {
            for c in indices {
                rgba16.push(u16le(&raw[c * 2..c * 2 + 2]));
            }
            rgba16.push(if layout.channels == 4 {
                u16le(&raw[6..8])
            } else {
                65535
            });
        }
    }
    let pixels = if layout.float() || layout.premul {
        RasterPixels::Rgba32Float(
            Arc::new(orient(rgba_float, w as usize, h as usize, metadata.orientation)).into(),
        )
    } else if matches!(layout.sample, Sample::U8) {
        RasterPixels::Rgba8(Arc::new(orient(rgba8, w as usize, h as usize, metadata.orientation)).into())
    } else {
        RasterPixels::Rgba16(Arc::new(orient(rgba16, w as usize, h as usize, metadata.orientation)).into())
    };
    Ok(DecodedRaster::new(expected.0, expected.1, pixels, color)?)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn root() -> std::path::PathBuf {
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/raster")
    }
    #[test]
    fn typed_oracles_and_orientation() {
        let root = root();
        for row in std::fs::read_to_string(root.join("jpegxr-manifest.tsv"))
            .unwrap()
            .lines()
            .filter(|s| !s.starts_with('#'))
        {
            let c: Vec<_> = row.split('\t').collect();
            let request = DecodeRequest::new(root.join(c[0]));
            let raster = crate::decode_raster(&request).unwrap_or_else(|e| panic!("{}: {e}", c[0]));
            assert_eq!(
                (raster.width(), raster.height()),
                (c[1].parse().unwrap(), c[2].parse().unwrap()),
                "{}",
                c[0]
            );
            let oracle = std::fs::read(root.join(c[5])).unwrap();
            match raster.pixels() {
                RasterPixels::Rgba8(v) => assert_eq!(v.as_slice(), oracle.as_slice(), "{}", c[0]),
                RasterPixels::Rgba16(v) => assert_eq!(
                    v.as_slice(),
                    oracle.chunks_exact(2).map(u16le).collect::<Vec<_>>(),
                    "{}",
                    c[0]
                ),
                RasterPixels::Rgba32Float(v) => assert_eq!(
                    v.iter().map(|x| x.to_bits()).collect::<Vec<_>>(),
                    oracle.chunks_exact(4).map(u32le).collect::<Vec<_>>(),
                    "{}",
                    c[0]
                ),
            }
            if c[0].contains("icc") {
                assert!(matches!(raster.color_space(), RasterColorSpace::Icc(_)));
            } else if c[3] == "f32" {
                assert_eq!(raster.color_space(), &RasterColorSpace::LinearSrgb);
            } else {
                assert_eq!(raster.color_space(), &RasterColorSpace::Srgb);
            }
            assert!(crate::is_supported_image_path(&request.path));
        }
    }
    #[test]
    fn malformed_container_fields_reject() {
        let path = root().join("jxr-uint8-3.jxr");
        let request = DecodeRequest::new(&path);
        let source = std::fs::read(path).unwrap();
        let at = u32le(&source[4..8]) as usize;
        let count = u16le(&source[at..at + 2]) as usize;
        let entry = |tag| {
            (0..count)
                .map(|i| at + 2 + i * 12)
                .find(|&i| u16le(&source[i..i + 2]) == tag)
                .unwrap()
        };
        for (tag, value) in [
            (0xbc02, 8u32),
            (0xbc80, 0),
            (0xbc81, 65537),
            (0xbcc0, 0),
            (0xbcc1, u32::MAX),
        ] {
            let mut b = source.clone();
            let i = entry(tag);
            b[i + 8..i + 12].copy_from_slice(&value.to_le_bytes());
            assert!(metadata(&b, &request).is_err(), "tag {tag:x}");
        }
        let mut b = source.clone();
        let i = entry(0xbc01);
        let guid = u32le(&b[i + 8..i + 12]) as usize;
        b[guid] ^= 1;
        assert!(metadata(&b, &request).is_err());
        let mut b = source;
        b[at + 4..at + 6].copy_from_slice(&13u16.to_le_bytes());
        assert!(metadata(&b, &request).is_err());
    }
    #[test]
    fn truncated_sources_reject_before_native_allocation() {
        for name in [
            "jxr-uint8-3.jxr",
            "jxr-uint16-4.jxr",
            "jxr-uint16-4-legacy-alpha-count.jxr",
            "jxr-uint8-3-icc.jxr",
        ] {
            let path = root().join(name);
            let request = DecodeRequest::new(&path);
            let bytes = std::fs::read(path).unwrap();
            for n in 0..bytes.len() {
                assert!(metadata(&bytes[..n], &request).is_err(), "{name}: prefix {n}");
            }
        }
    }
}
