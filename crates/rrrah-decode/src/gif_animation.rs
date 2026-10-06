//! Bounded selected GIF presentation frames, without decoding later pixels.
use crate::{
    DecodeRequest,
    bounded_io::read_bounded,
    raster::{MAX_RASTER_BYTES, RasterDecodeError},
};
use image::{AnimationDecoder, ImageDecoder, Limits, codecs::gif::GifDecoder};
use rrrah_core::{DecodedRaster, RasterColorSpace, RasterPixels};
use std::{io::Cursor, sync::Arc};
#[derive(Debug, Clone)]
pub struct GifImage {
    pub raster: DecodedRaster,
    pub delay_ms: u32,
    /// None: play once; Some(0): infinite; Some(n): n repeats after first play.
    pub repeats: Option<u16>,
}
fn bad(s: &'static str) -> RasterDecodeError {
    RasterDecodeError::InvalidGif(s)
}
pub(crate) fn has_magic(b: &[u8]) -> bool {
    b.starts_with(b"GIF87a") || b.starts_with(b"GIF89a")
}
struct Scan<'a> {
    b: &'a [u8],
    at: usize,
}
impl<'a> Scan<'a> {
    fn take(&mut self, n: usize) -> Result<&'a [u8], RasterDecodeError> {
        let end = self
            .at
            .checked_add(n)
            .filter(|&e| e <= self.b.len())
            .ok_or_else(|| bad("truncated block"))?;
        let result = &self.b[self.at..end];
        self.at = end;
        Ok(result)
    }
    fn byte(&mut self) -> Result<u8, RasterDecodeError> {
        Ok(self.take(1)?[0])
    }
    fn blocks(&mut self, request: &DecodeRequest) -> Result<(), RasterDecodeError> {
        loop {
            request.check_cancelled()?;
            let n = self.byte()? as usize;
            if n == 0 {
                return Ok(());
            }
            self.take(n)?;
        }
    }
}
fn word(b: &[u8]) -> u16 {
    u16::from_le_bytes(b.try_into().unwrap())
}
fn inventory(b: &[u8], request: &DecodeRequest) -> Result<(usize, Vec<u32>, Option<u16>), RasterDecodeError> {
    if !has_magic(b) {
        return Err(bad("signature"));
    }
    let mut s = Scan { b, at: 6 };
    let h = s.take(7)?;
    let (w, hgt) = (word(&h[..2]) as u32, word(&h[2..4]) as u32);
    if w == 0 || hgt == 0 || u64::from(w) * u64::from(hgt) * 16 > MAX_RASTER_BYTES {
        return Err(RasterDecodeError::OutputTooLarge);
    }
    if h[4] & 128 != 0 {
        s.take(3usize << ((h[4] & 7) + 1))?;
    }
    let mut delays = Vec::new();
    let mut delay = 0;
    let mut control = false;
    let mut repeats = None;
    loop {
        request.check_cancelled()?;
        match s.byte()? {
            0x3b => {
                if s.at != b.len() || delays.is_empty() || control {
                    return Err(bad("trailing data or missing image"));
                }
                return Ok((delays.len(), delays, repeats));
            }
            0x2c => {
                let d = s.take(9)?;
                let (x, y, fw, fh) = (
                    word(&d[..2]) as u32,
                    word(&d[2..4]) as u32,
                    word(&d[4..6]) as u32,
                    word(&d[6..8]) as u32,
                );
                if fw == 0 || fh == 0 || x + fw > w || y + fh > hgt || d[8] & 0x18 != 0 {
                    return Err(bad("frame geometry/reserved flags"));
                }
                if d[8] & 128 != 0 {
                    s.take(3usize << ((d[8] & 7) + 1))?;
                }
                if !(2..=8).contains(&s.byte()?) {
                    return Err(bad("LZW code size"));
                }
                s.blocks(request)?;
                delays.push(delay);
                if delays.len() > 4096 {
                    return Err(bad("too many frames"));
                }
                delay = 0;
                control = false;
            }
            0x21 => match s.byte()? {
                0xf9 => {
                    if control || s.byte()? != 4 {
                        return Err(bad("duplicate/invalid graphic control"));
                    }
                    let g = s.take(4)?;
                    if g[0] & 0xe2 != 0 || (g[0] >> 2) & 7 > 3 {
                        return Err(bad("unsupported graphic control"));
                    }
                    delay = u32::from(word(&g[1..3])) * 10;
                    if s.byte()? != 0 {
                        return Err(bad("graphic control terminator"));
                    }
                    control = true;
                }
                0xff => {
                    let n = s.byte()? as usize;
                    let app = s.take(n)?;
                    if app == b"NETSCAPE2.0" || app == b"ANIMEXTS1.0" {
                        if repeats.is_some() || s.byte()? != 3 {
                            return Err(bad("invalid loop extension"));
                        }
                        let v = s.take(3)?;
                        if v[0] != 1 || s.byte()? != 0 {
                            return Err(bad("invalid loop control"));
                        }
                        repeats = Some(word(&v[1..]));
                    } else {
                        s.blocks(request)?;
                    }
                }
                0xfe => s.blocks(request)?,
                _ => return Err(bad("unsupported extension (including plain text)")),
            },
            _ => return Err(bad("unknown block")),
        }
    }
}
pub fn decode_gif(request: &DecodeRequest) -> Result<GifImage, RasterDecodeError> {
    request.check_cancelled()?;
    decode(&read_bounded(request)?, request)
}
pub(crate) fn decode(b: &[u8], request: &DecodeRequest) -> Result<GifImage, RasterDecodeError> {
    let (count, delays, repeats) = inventory(b, request)?;
    if request.image_index >= count {
        return Err(crate::DecodeError::UnsupportedImageIndex {
            index: request.image_index,
        }
        .into());
    }
    let mut decoder = GifDecoder::new(Cursor::new(b))?;
    let mut limits = Limits::default();
    limits.max_alloc = Some(MAX_RASTER_BYTES);
    decoder.set_limits(limits)?;
    for (index, frame) in decoder.into_frames().enumerate() {
        request.check_cancelled()?;
        let frame = frame?;
        if index == request.image_index {
            let pixels = frame.into_buffer();
            let raster = DecodedRaster::new(
                pixels.width(),
                pixels.height(),
                RasterPixels::Rgba8(Arc::new(pixels.into_raw()).into()),
                RasterColorSpace::Srgb,
            )?
            .with_image_selection(index, count)?;
            return Ok(GifImage {
                raster,
                delay_ms: delays[index],
                repeats,
            });
        }
    }
    Err(bad("missing selected frame"))
}

#[cfg(test)]
mod tests {
    use super::*;
    fn root() -> std::path::PathBuf {
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/raster")
    }
    #[test]
    fn independent_selected_presentations() {
        let root = root();
        for line in std::fs::read_to_string(root.join("gif-animation-manifest.tsv"))
            .unwrap()
            .lines()
            .filter(|s| !s.starts_with('#'))
        {
            let c: Vec<_> = line.split('\t').collect();
            let mut request = DecodeRequest::new(root.join(c[0]));
            request.image_index = c[1].parse().unwrap();
            let image = decode_gif(&request).unwrap();
            assert_eq!((image.raster.width(), image.raster.height()), (5, 3));
            assert_eq!(image.raster.image_count(), 4);
            assert_eq!(image.raster.image_index(), request.image_index);
            assert_eq!(image.delay_ms, c[4].parse::<u32>().unwrap());
            assert_eq!(image.repeats, Some(2));
            let RasterPixels::Rgba8(pixels) = image.raster.pixels() else {
                panic!()
            };
            assert_eq!(
                pixels.as_slice(),
                std::fs::read(root.join(c[6])).unwrap(),
                "{} frame {}",
                c[0],
                c[1]
            );
            let common = crate::decode_raster(&request).unwrap();
            let RasterPixels::Rgba8(common_pixels) = common.pixels() else {
                panic!()
            };
            assert_eq!(common_pixels.as_slice(), pixels.as_slice());
        }
    }
    #[test]
    fn unselected_lzw_is_not_decoded_and_magic_routes_selection() {
        let path = root().join("gif-animation-disposal-3.gif");
        let mut b = std::fs::read(&path).unwrap();
        let mut request = DecodeRequest::new(&path);
        let mut scan = Scan { b: &b, at: 13 };
        if b[10] & 128 != 0 {
            scan.take(3usize << ((b[10] & 7) + 1)).unwrap();
        }
        let mut last = Vec::new();
        loop {
            match scan.byte().unwrap() {
                0x3b => break,
                0x21 => {
                    scan.byte().unwrap();
                    scan.blocks(&request).unwrap();
                }
                0x2c => {
                    let d = scan.take(9).unwrap();
                    if d[8] & 128 != 0 {
                        scan.take(3usize << ((d[8] & 7) + 1)).unwrap();
                    }
                    scan.byte().unwrap();
                    last.clear();
                    loop {
                        let n = scan.byte().unwrap() as usize;
                        if n == 0 {
                            break;
                        }
                        let start = scan.at;
                        scan.take(n).unwrap();
                        last.push(start..start + n);
                    }
                }
                _ => panic!(),
            }
        }
        for range in last {
            b[range].fill(255);
        }
        assert!(decode(&b, &request).is_ok());
        request.image_index = 3;
        assert!(decode(&b, &request).is_err());
        let other = std::env::temp_dir().join(format!(
            "rrrah-gif-{}-{}.raw",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::copy(path, &other).unwrap();
        request.path = other;
        assert_eq!(crate::decode_raster(&request).unwrap().image_index(), 3);
        std::fs::remove_file(&request.path).unwrap();
    }
    #[test]
    fn bounds_truncation_and_index() {
        let path = root().join("gif-animation-disposal-3.gif");
        let b = std::fs::read(&path).unwrap();
        let mut request = DecodeRequest::new(path);
        for n in 0..b.len() {
            assert!(decode(&b[..n], &request).is_err(), "prefix {n}");
        }
        request.image_index = 4;
        assert!(decode(&b, &request).is_err());
        request.image_index = 0;
        let mut trailing = b.clone();
        trailing.push(0);
        assert!(decode(&trailing, &request).is_err());
        let mut dimensions = b;
        dimensions[6..8].fill(0);
        assert!(decode(&dimensions, &request).is_err());
    }
}
