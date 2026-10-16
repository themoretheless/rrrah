//! PSD/PSB stored RGB/grayscale composition, raw and PackBits.
use crate::{
    DecodeRequest,
    raster::{MAX_RASTER_BYTES, RasterDecodeError},
};
use rrrah_core::{DecodedRaster, RasterColorSpace, RasterPixels};
use std::sync::Arc;
pub fn has_magic(b: &[u8]) -> bool {
    b.starts_with(b"8BPS")
}
fn bad(s: &'static str) -> RasterDecodeError {
    RasterDecodeError::InvalidPsd(s)
}
struct Reader<'a> {
    b: &'a [u8],
    at: usize,
}
impl<'a> Reader<'a> {
    fn take(&mut self, n: usize) -> Result<&'a [u8], RasterDecodeError> {
        let end = self.at.checked_add(n).ok_or_else(|| bad("length overflow"))?;
        let b = self.b.get(self.at..end).ok_or_else(|| bad("truncated section"))?;
        self.at = end;
        Ok(b)
    }
    fn word(&mut self) -> Result<u16, RasterDecodeError> {
        Ok(u16::from_be_bytes(self.take(2)?.try_into().unwrap()))
    }
    fn long(&mut self) -> Result<u32, RasterDecodeError> {
        Ok(u32::from_be_bytes(self.take(4)?.try_into().unwrap()))
    }
    fn size(&mut self, large: bool) -> Result<usize, RasterDecodeError> {
        if large {
            usize::try_from(u64::from_be_bytes(self.take(8)?.try_into().unwrap()))
                .map_err(|_| bad("section overflow"))
        } else {
            Ok(self.long()? as usize)
        }
    }
}
#[cfg(test)]
fn unpack(bytes: &[u8], out: &mut Vec<u8>, expected: usize) -> Result<(), RasterDecodeError> {
    unpack_checked(bytes, out, expected, |_| Ok(()))
}
fn unpack_checked(
    bytes: &[u8],
    out: &mut Vec<u8>,
    expected: usize,
    mut check: impl FnMut(usize) -> Result<(), RasterDecodeError>,
) -> Result<(), RasterDecodeError> {
    let start = out.len();
    let mut r = Reader { b: bytes, at: 0 };
    let mut next_check = 0;
    while r.at < bytes.len() {
        // One PackBits command consumes at most129 bytes. The next check is
        // never more than64KiB plus one command beyond the previous check.
        if r.at >= next_check {
            check(r.at)?;
            next_check = r.at.saturating_add(65536);
        }
        let count = r.take(1)?[0] as i8;
        let n = if count >= 0 {
            count as usize + 1
        } else if count == -128 {
            0
        } else {
            (1 - i16::from(count)) as usize
        };
        if out.len() - start + n > expected {
            return Err(bad("RLE row overflow"));
        }
        if count >= 0 {
            out.extend_from_slice(r.take(n)?);
        } else if n > 0 {
            let v = r.take(1)?[0];
            out.resize(out.len() + n, v);
        }
    }
    check(r.at)?;
    if out.len() - start != expected {
        return Err(bad("RLE row underflow"));
    }
    Ok(())
}
fn inflate_zip(
    compressed: &[u8],
    data: &mut Vec<u8>,
    total: usize,
    request: &DecodeRequest,
) -> Result<(), RasterDecodeError> {
    inflate_zip_with_progress(compressed, data, total, request, |_, _| {})
}
fn inflate_zip_with_progress(
    compressed: &[u8],
    data: &mut Vec<u8>,
    total: usize,
    request: &DecodeRequest,
    mut progress: impl FnMut(u64, u64),
) -> Result<(), RasterDecodeError> {
    let _decoder_credit = reserve(request, rrrah_scientific::DEFLATE_DECODER_CREDIT_BYTES as usize)?;
    let mut decoder = flate2::Decompress::new(true);
    let mut chunk = [0u8; 65536];
    loop {
        request.check_cancelled()?;
        let before = (decoder.total_in(), decoder.total_out());
        let input = before.0 as usize;
        let output = (total - data.len()).clamp(1, chunk.len());
        let status = decoder
            .decompress(
                &compressed[input..compressed.len().min(input + 65536)],
                &mut chunk[..output],
                flate2::FlushDecompress::None,
            )
            .map_err(|_| bad("invalid ZIP stream"))?;
        let produced = (decoder.total_out() - before.1) as usize;
        if data.len() + produced > total {
            return Err(bad("ZIP output exceeds canvas"));
        }
        data.extend_from_slice(&chunk[..produced]);
        progress(decoder.total_in(), decoder.total_out());
        if status == flate2::Status::StreamEnd {
            if data.len() != total || decoder.total_in() != compressed.len() as u64 {
                return Err(bad("ZIP size or trailing bytes"));
            }
            return Ok(());
        }
        if before == (decoder.total_in(), decoder.total_out()) {
            return Err(bad("truncated/stalled ZIP stream"));
        }
    }
}
fn reserve(
    request: &DecodeRequest,
    bytes: usize,
) -> Result<Option<rrrah_core::Reservation>, RasterDecodeError> {
    request.check_cancelled()?;
    request
        .memory_budget
        .as_ref()
        .map(|budget| budget.try_reserve(bytes as u64))
        .transpose()
        .map_err(crate::DecodeError::from)
        .map_err(Into::into)
}
fn unmatte_pixels(
    pixels: &mut RasterPixels,
    mut check: impl FnMut() -> Result<(), crate::DecodeError>,
) -> Result<(), RasterDecodeError> {
    match pixels {
        RasterPixels::Rgba8(p) => {
            for (index, rgba) in p
                .get_mut()
                .ok_or_else(|| bad("merged pixels unexpectedly shared"))?
                .chunks_exact_mut(4)
                .enumerate()
            {
                if index.is_multiple_of(4096) {
                    check()?;
                }
                let a = u32::from(rgba[3]);
                if a > 0 {
                    for c in &mut rgba[..3] {
                        *c = ((u32::from(*c) + a).saturating_sub(255) * 255 / a).min(255) as u8;
                    }
                }
            }
        }
        RasterPixels::Rgba16(p) => {
            for (index, rgba) in p
                .get_mut()
                .ok_or_else(|| bad("merged pixels unexpectedly shared"))?
                .chunks_exact_mut(4)
                .enumerate()
            {
                if index.is_multiple_of(4096) {
                    check()?;
                }
                let a = u64::from(rgba[3]);
                if a > 0 {
                    for c in &mut rgba[..3] {
                        *c = ((u64::from(*c) + a).saturating_sub(65535) * 65535 / a).min(65535)
                            as u16;
                    }
                }
            }
        }
        RasterPixels::Rgba32Float(p) => {
            for (index, rgba) in p
                .get_mut()
                .ok_or_else(|| bad("merged pixels unexpectedly shared"))?
                .chunks_exact_mut(4)
                .enumerate()
            {
                if index.is_multiple_of(4096) {
                    check()?;
                }
                let a = f64::from(rgba[3]);
                if a > 0. && a < 1. {
                    for c in &mut rgba[..3] {
                        let result = ((f64::from(*c) + a - 1.) / a) as f32;
                        if c.is_finite() && !result.is_finite() {
                            return Err(bad("merged transparency unmatting exceeds float32 range"));
                        }
                        *c = result;
                    }
                }
            }
        }
    }
    check()?;
    Ok(())
}

pub fn decode(bytes: &[u8], request: &DecodeRequest) -> Result<DecodedRaster, RasterDecodeError> {
    request.check_cancelled()?;
    let mut r = Reader { b: bytes, at: 0 };
    if r.take(4)? != b"8BPS" {
        return Err(bad("signature"));
    }
    let version = r.word()?;
    if !matches!(version, 1 | 2) || r.take(6)? != [0; 6] {
        return Err(bad("version or reserved bytes"));
    }
    let large = version == 2;
    let channels = r.word()? as usize;
    let height = r.long()?;
    let width = r.long()?;
    let depth = r.word()?;
    let mode = r.word()?;
    let colors = match mode {
        1 => 1,
        3 => 3,
        _ => return Err(bad("color mode unsupported")),
    };
    if channels < colors
        || channels > 56
        || !matches!(depth, 8 | 16 | 32)
        || width == 0
        || height == 0
        || width > 65536
        || height > 65536
    {
        return Err(bad("dimensions, channels or depth"));
    }
    let sample = usize::from(depth / 8);
    let pixels = width as usize * height as usize;
    let plane_bytes = pixels.checked_mul(sample).ok_or_else(|| bad("size overflow"))?;
    let total = plane_bytes
        .checked_mul(channels)
        .ok_or_else(|| bad("size overflow"))?;
    if (total as u64) + (pixels as u64) * 4 * (sample as u64) > MAX_RASTER_BYTES {
        return Err(RasterDecodeError::OutputTooLarge);
    }
    let color_len = r.long()? as usize;
    if color_len != 0 {
        let data = r.take(color_len)?;
        // Observed float RGB/gray composites carry 74- or 112-byte
        // HDR toning-options block. It is not a palette or a sensor transfer
        // function: preserve stored float samples without applying toning.
        if depth != 32 || !matches!(data.len(), 74 | 112) || data[..8] != *b"hdrt\0\0\0\x03" {
            return Err(bad("unexpected color mode data"));
        }
    }
    let resource_len = r.long()? as usize;
    let mut resources = Reader {
        b: r.take(resource_len)?,
        at: 0,
    };
    let mut profile = None;
    let mut named_alpha = false;
    while resources.at < resources.b.len() {
        request.check_cancelled()?;
        if resources.take(4)? != b"8BIM" {
            return Err(bad("resource signature"));
        }
        let id = resources.word()?;
        let name = resources.take(1)?[0] as usize;
        resources.take(name)?;
        if (name + 1) % 2 != 0 {
            resources.take(1)?;
        }
        let size = resources.long()? as usize;
        let data = resources.take(size)?;
        if size % 2 != 0 {
            resources.take(1)?;
        }
        if id == 1053 {
            named_alpha = true;
        }
        if id == 1039 {
            if profile.is_some() || size > 4 * 1024 * 1024 {
                return Err(bad("duplicate or excessive ICC"));
            }
            profile = Some(data);
        }
    }
    let layer_len = r.size(large)?;
    let layer = r.take(layer_len)?;
    let mut alpha = channels == colors + 1 && !named_alpha;
    if !layer.is_empty() {
        let mut lr = Reader { b: layer, at: 0 };
        let info_len = lr.size(large)?;
        let info = lr.take(info_len)?;
        if !info.is_empty() {
            let mut ir = Reader { b: info, at: 0 };
            let count = ir.word()? as i16;
            if count != 0 {
                alpha = count < 0;
            }
        }
    }
    if alpha && channels <= colors {
        return Err(bad("missing merged alpha"));
    }
    let compression = r.word()?;
    if !matches!(compression, 0..=3) {
        return Err(bad("compression unsupported"));
    }
    let profile_credit = match profile {
        Some(data) => reserve(request, data.len())?,
        None => None,
    };
    let profile = profile.map(<[u8]>::to_vec);
    let _data_credit = if compression == 0 {
        None
    } else {
        reserve(request, total)?
    };
    let mut data = if compression == 0 {
        Vec::new()
    } else {
        Vec::with_capacity(total)
    };
    let mut borrowed_planes = None;
    match compression {
        0 => borrowed_planes = Some(r.take(total)?),
        1 => {
            let rows = channels * height as usize;
            let length_bytes = if large { 4 } else { 2 };
            let lengths = r.take(rows * length_bytes)?;
            for encoded in lengths.chunks_exact(length_bytes) {
                let len = if large {
                    u32::from_be_bytes(encoded.try_into().unwrap()) as usize
                } else {
                    usize::from(u16::from_be_bytes(encoded.try_into().unwrap()))
                };
                request.check_cancelled()?;
                unpack_checked(r.take(len)?, &mut data, width as usize * sample, |_| {
                    request.check_cancelled().map_err(Into::into)
                })?;
            }
        }
        2 | 3 => {
            let compressed = r.take(bytes.len() - r.at)?;
            inflate_zip(compressed, &mut data, total, request)?;
            if compression == 3 {
                let scratch_bytes = if depth == 32 { width as usize * sample } else { 0 };
                let _prediction_credit = reserve(request, scratch_bytes)?;
                let mut planes = vec![0u8; scratch_bytes];
                for row in data.chunks_exact_mut(width as usize * sample) {
                    request.check_cancelled()?;
                    match depth {
                        8 => {
                            for i in 1..row.len() {
                                row[i] = row[i].wrapping_add(row[i - 1]);
                            }
                        }
                        16 => {
                            let mut previous = 0u16;
                            for bytes in row.chunks_exact_mut(2) {
                                previous =
                                    previous.wrapping_add(u16::from_be_bytes(bytes.try_into().unwrap()));
                                bytes.copy_from_slice(&previous.to_be_bytes());
                            }
                        }
                        32 => {
                            for i in 1..row.len() {
                                row[i] = row[i].wrapping_add(row[i - 1]);
                            }
                            planes.copy_from_slice(row);
                            for x in 0..width as usize {
                                for c in 0..4 {
                                    row[x * 4 + c] = planes[c * width as usize + x];
                                }
                            }
                        }
                        _ => unreachable!(),
                    }
                }
            }
        }
        _ => return Err(bad("compression unsupported")),
    }
    if r.at != bytes.len() {
        return Err(bad("trailing composition bytes"));
    }
    let data = borrowed_planes.unwrap_or(&data);
    let color = profile.map_or_else(
        || {
            if request.assume_untagged_linear_srgb {
                RasterColorSpace::LinearSrgb
            } else if request.assume_untagged_srgb {
                RasterColorSpace::AssumedSrgb
            } else {
                RasterColorSpace::Unspecified
            }
        },
        RasterColorSpace::Icc,
    );
    let offset = |c: usize, p: usize| c * plane_bytes + p * sample;
    let output_credit = reserve(request, pixels * 4 * sample)?;
    macro_rules! interleave {
        ($ty:ty,$read:expr,$opaque:expr,$variant:ident) => {{
            let mut out = Vec::<$ty>::with_capacity(pixels * 4);
            for p in 0..pixels {
                if p % 65536 == 0 {
                    request.check_cancelled()?;
                }
                for c in 0..3 {
                    out.push($read(offset(if colors == 1 { 0 } else { c }, p)));
                }
                out.push(if alpha { $read(offset(colors, p)) } else { $opaque });
            }
            let storage = match output_credit {
                Some(credit) => credit.try_adopt(out).map_err(crate::DecodeError::from)?.into(),
                None => Arc::new(out).into(),
            };
            RasterPixels::$variant(storage)
        }};
    }
    let mut pixels = match depth {
        8 => interleave!(u8, |i| data[i], 255, Rgba8),
        16 => interleave!(
            u16,
            |i| u16::from_be_bytes(data[i..i + 2].try_into().unwrap()),
            65535,
            Rgba16
        ),
        _ => interleave!(
            f32,
            |i| f32::from_be_bytes(data[i..i + 4].try_into().unwrap()),
            1.0,
            Rgba32Float
        ),
    };
    if alpha { unmatte_pixels(&mut pixels, || request.check_cancelled())?; }
    let raster = DecodedRaster::new(width, height, pixels, color)?;
    let raster = match profile_credit {
        Some(credit) => raster
            .with_color_profile_reservation(credit)
            .map_err(crate::DecodeError::from)?,
        None => raster,
    };
    request.check_cancelled()?;
    Ok(raster)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn opaque_float_alpha_preserves_tiny_samples_signed_zero_and_nan_bits() {
        for rgb in [[-0.0,f32::from_bits(1),1e-30],[f32::from_bits(0x7fc00031),f32::MIN_POSITIVE,-1e-30]] {
            let budget=rrrah_core::MemoryBudget::new(16);
            let expected=[rgb[0],rgb[1],rgb[2],1.];
            let mut values=budget.try_buffer(4,0f32).unwrap();values.copy_from_slice(&expected);
            let mut pixels=RasterPixels::Rgba32Float(values.freeze().into());
            unmatte_pixels(&mut pixels,||Ok(())).unwrap();
            let RasterPixels::Rgba32Float(values)=&pixels else {panic!()};
            assert_eq!(values.iter().map(|v|v.to_bits()).collect::<Vec<_>>(),expected.map(f32::to_bits));
            drop(pixels);assert_eq!(budget.used(),0);
        }
    }

    #[test]
    fn float_unmatting_refuses_new_overflow_and_preserves_representable_hdr() {
        for alpha in [f32::MIN_POSITIVE,f32::from_bits(1)] {
            let budget=rrrah_core::MemoryBudget::new(16);
            let mut values=budget.try_buffer(4,0f32).unwrap();values.copy_from_slice(&[f32::MAX,2.,4.,alpha]);
            let mut pixels=RasterPixels::Rgba32Float(values.freeze().into());
            assert!(matches!(unmatte_pixels(&mut pixels,||Ok(())),Err(RasterDecodeError::InvalidPsd("merged transparency unmatting exceeds float32 range"))));
            drop(pixels);assert_eq!(budget.used(),0);
        }
        for alpha in [0.,1.] {
            let budget=rrrah_core::MemoryBudget::new(16);
            let mut values=budget.try_buffer(4,0f32).unwrap();values.copy_from_slice(&[f32::MAX,-2.,4.,alpha]);
            let mut pixels=RasterPixels::Rgba32Float(values.freeze().into());
            unmatte_pixels(&mut pixels,||Ok(())).unwrap();
            let RasterPixels::Rgba32Float(values)=&pixels else {panic!()};
            assert_eq!(&values[..],&[f32::MAX,-2.,4.,alpha]);
            drop(pixels);assert_eq!(budget.used(),0);
        }
    }

    #[test]
    fn white_unmatting_cancels_before_next_4096_pixel_block_at_every_precision() {
        for depth in [8,16,32] {
            let budget=rrrah_core::MemoryBudget::new(8193*16);
            let mut pixels=match depth {
                8=>RasterPixels::Rgba8(budget.try_buffer(8193*4,0u8).unwrap().freeze().into()),
                16=>RasterPixels::Rgba16(budget.try_buffer(8193*4,0u16).unwrap().freeze().into()),
                _=>RasterPixels::Rgba32Float(budget.try_buffer(8193*4,0f32).unwrap().freeze().into()),
            };
            match &mut pixels {
                RasterPixels::Rgba8(v)=>for p in v.get_mut().unwrap().chunks_exact_mut(4) {p.copy_from_slice(&[190,190,190,128]);},
                RasterPixels::Rgba16(v)=>for p in v.get_mut().unwrap().chunks_exact_mut(4) {p.copy_from_slice(&[49151,49151,49151,32768]);},
                RasterPixels::Rgba32Float(v)=>for p in v.get_mut().unwrap().chunks_exact_mut(4) {p.copy_from_slice(&[0.75,0.75,0.75,0.5]);},
            }
            let mut polls=0;
            assert!(matches!(unmatte_pixels(&mut pixels,|| {polls+=1;if polls==2 {Err(crate::DecodeError::Cancelled)} else {Ok(())}}),Err(RasterDecodeError::Source(crate::DecodeError::Cancelled))));
            assert_eq!(polls,2);
            for pixel in 0..8193 {
                match &pixels {
                    RasterPixels::Rgba8(v)=>assert_eq!(&v[pixel*4..pixel*4+4],if pixel<4096 {&[125,125,125,128]} else {&[190,190,190,128]}),
                    RasterPixels::Rgba16(v)=>assert_eq!(&v[pixel*4..pixel*4+4],if pixel<4096 {&[32767,32767,32767,32768]} else {&[49151,49151,49151,32768]}),
                    RasterPixels::Rgba32Float(v)=>assert_eq!(&v[pixel*4..pixel*4+4],if pixel<4096 {&[0.5,0.5,0.5,0.5]} else {&[0.75,0.75,0.75,0.5]}),
                }
            }
            drop(pixels);assert_eq!(budget.used(),0);
        }
    }

    #[test]
    fn independent_high_precision_alpha_controls_preserve_precision_and_hdr() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/raster");
        for depth in [16,32] {
            for mode in ["rgba","la"] {
                let channels = if mode=="rgba" {4} else {2};
                for version in [1,2] {
                    let suffix=if version==1 {"psd"} else {"psb"};
                    let name=format!("psd-tools-alpha-{mode}-{depth}-v{version}.{suffix}");
                    let bytes=std::fs::read(root.join(&name)).unwrap();
                    let expected=std::fs::read(root.join(format!("{name}.f32le"))).unwrap();
                    let expected=expected.chunks_exact(4).map(|b| f32::from_le_bytes(b.try_into().unwrap())).collect::<Vec<_>>();
                    let output_bytes=4*4*(depth/8) as u64;
                    let peak=output_bytes+4*channels as u64*(depth/8) as u64;
                    let budget=rrrah_core::MemoryBudget::new(peak);
                    let short=rrrah_core::MemoryBudget::new(peak-1);
                    let mut request=DecodeRequest::new(root.join(&name));
                    request.memory_budget=Some(short.clone());assert!(decode(&bytes,&request).is_err());assert_eq!(short.used(),0);
                    request.memory_budget=Some(budget.clone());
                    let image=decode(&bytes,&request).unwrap();
                    assert_eq!((image.width(),image.height()),(4,1));
                    for pixel in 0..4 {
                        for channel in 0..4 {
                            let oracle_channel=if channel==3 {channels-1} else if channels==2 {0} else {channel};
                            let reference=f64::from(expected[pixel*channels+oracle_channel]);
                            let actual=match image.pixels() {
                                RasterPixels::Rgba16(values)=>f64::from(values[pixel*4+channel])/65535.,
                                RasterPixels::Rgba32Float(values)=>f64::from(values[pixel*4+channel]),
                                _=>panic!("precision changed"),
                            };
                            let bound=if depth==16 && channel!=3 {1./65535.} else {2e-6};
                            assert!((actual-reference).abs()<=bound,"{name} pixel={pixel} channel={channel} actual={actual} reference={reference}");
                            if depth==32 && channel<3 && pixel>=2 {assert!(actual>1.);}
                        }
                    }
                    assert_eq!((budget.used(),budget.peak()),(output_bytes,peak));
                    let clone=image.clone();drop(image);assert_eq!(budget.used(),output_bytes);
                    drop(clone);assert_eq!(budget.used(),0);
                }
            }
        }
    }

    #[test]
    fn multirow_alpha_all_compressions_match_independent_native_oracles_and_budgets() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/raster");
        let manifest = include_str!("../../../tests/fixtures/raster/psd-tools-alpha-multi-manifest.tsv");
        let mut cases = 0;
        for row in manifest.lines().filter(|line| !line.starts_with('#') && !line.is_empty()) {
            let f: Vec<_> = row.split('\t').collect();
            let width: u64 = f[2].parse().unwrap();
            let height: u64 = f[3].parse().unwrap();
            let depth: u64 = f[4].parse().unwrap();
            let channels: u64 = f[5].parse().unwrap();
            let compression: u8 = f[6].parse().unwrap();
            let bytes = std::fs::read(root.join(f[0])).unwrap();
            let expected = std::fs::read(root.join(f[1])).unwrap();
            let output = width * height * 4 * (depth / 8);
            let planes = width * height * channels * (depth / 8);
            let peak = match compression {
                0 => output,
                1 => planes + output,
                2 | 3 => planes + rrrah_scientific::DEFLATE_DECODER_CREDIT_BYTES.max(output)
                    .max(if compression == 3 && depth == 32 { width * 4 } else { 0 }),
                _ => panic!("fixture compression"),
            };
            let mut request = DecodeRequest::new(root.join(f[0]));
            request.assume_untagged_linear_srgb = true;
            let short = rrrah_core::MemoryBudget::new(peak - 1);
            request.memory_budget = Some(short.clone());
            assert!(matches!(decode(&bytes, &request),
                Err(RasterDecodeError::Source(crate::DecodeError::Memory(_)))), "{}", f[0]);
            assert_eq!(short.used(), 0);
            let budget = rrrah_core::MemoryBudget::new(peak);
            request.memory_budget = Some(budget.clone());
            let image = decode(&bytes, &request).unwrap();
            assert_eq!((u64::from(image.width()), u64::from(image.height())), (width, height));
            assert_eq!(image.color_space(), &RasterColorSpace::LinearSrgb);
            match image.pixels() {
                RasterPixels::Rgba16(values) if depth == 16 => {
                    let oracle: Vec<_> = expected.chunks_exact(2)
                        .map(|b| u16::from_le_bytes(b.try_into().unwrap())).collect();
                    assert_eq!(values.as_slice(), oracle, "{}", f[0]);
                    assert!(values.is_managed());
                }
                RasterPixels::Rgba32Float(values) if depth == 32 => {
                    let oracle: Vec<_> = expected.chunks_exact(4)
                        .map(|b| u32::from_le_bytes(b.try_into().unwrap())).collect();
                    assert_eq!(values.iter().map(|v| v.to_bits()).collect::<Vec<_>>(), oracle, "{}", f[0]);
                    assert!(values.is_managed());
                    assert!(values.iter().any(|v| *v > 1.));
                    assert!(values.iter().any(|v| *v < 0.));
                }
                _ => panic!("source precision changed"),
            }
            assert_eq!((budget.used(), budget.peak()), (output, peak));
            let clone = image.clone();
            drop(image);
            assert_eq!(budget.used(), output);
            drop(clone);
            assert_eq!(budget.used(), 0);
            cases += 1;
        }
        assert_eq!(cases, 32);
    }

    #[test]
    fn original_gray_merged_alpha_psd_psb_matches_independent_decoders() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/raster");
        let expected = include_bytes!("../../../tests/fixtures/raster/psd-tools-gray0.rgba");
        let profile = include_bytes!("../../../tests/fixtures/raster/psd-tools-gray0.icc");
        let retained = (expected.len()+profile.len()) as u64;
        let peak = retained+400*359*2;
        for ext in ["psd","psb"] {
            let path = root.join(format!("psd-tools-gray0.{ext}"));
            let bytes = std::fs::read(&path).unwrap();
            let budget = rrrah_core::MemoryBudget::new(peak);
            let mut request = DecodeRequest::new(path);
            let short = rrrah_core::MemoryBudget::new(peak-1);
            request.memory_budget=Some(short.clone());assert!(decode(&bytes,&request).is_err());assert_eq!(short.used(),0);
            request.memory_budget=Some(budget.clone());
            let image=decode(&bytes,&request).unwrap();
            assert_eq!((image.width(),image.height()),(400,359));
            let RasterPixels::Rgba8(values)=image.pixels() else {panic!()};
            assert!(values.is_managed());assert_eq!(&values[..],&expected[..]);
            assert_eq!(image.color_space(),&RasterColorSpace::Icc(profile.to_vec()));
            assert!(values.chunks_exact(4).any(|p| p[3]>0 && p[3]<255));
            assert_eq!((budget.used(),budget.peak()),(retained,peak));
            let clone=image.clone();drop(image);assert_eq!(budget.used(),retained);
            drop(clone);assert_eq!(budget.used(),0);
        }
    }

    #[test]
    fn malformed_hdr_toning_sections_refuse_before_managed_admission() {
        let original = include_bytes!("../../../tests/fixtures/raster/psd-tools-4x4_32bit_rgb.psd");
        assert_eq!(u32::from_be_bytes(original[26..30].try_into().unwrap()),112);
        let mut cases = Vec::new();
        let mut signature = original.to_vec();signature[30] ^= 1;cases.push(signature);
        let mut version = original.to_vec();version[37] = 4;cases.push(version);
        let mut short = original.to_vec();short.remove(141);short[26..30].copy_from_slice(&111u32.to_be_bytes());cases.push(short);
        let mut long = original.to_vec();long.insert(142,0);long[26..30].copy_from_slice(&113u32.to_be_bytes());cases.push(long);
        let mut integer = original.to_vec();integer[22..24].copy_from_slice(&16u16.to_be_bytes());cases.push(integer);
        cases.push(original[..80].to_vec());
        for bytes in cases {
            let budget = rrrah_core::MemoryBudget::new(0);
            let mut request = DecodeRequest::new("malformed-hdrt.psd");
            request.memory_budget = Some(budget.clone());
            assert!(matches!(decode(&bytes,&request),Err(RasterDecodeError::InvalidPsd(_))));
            assert_eq!((budget.used(),budget.peak()),(0,0));
        }
    }

    #[test]
    fn original_rgb_gray_16_32_composites_match_independent_planes() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/raster");
        for name in ["4x4_16bit_rgb.psd","4x4_16bit_grayscale.psd","4x4_32bit_rgb.psd","4x4_32bit_grayscale.psd","32bit5x5.psb"] {
            let path = root.join(format!("psd-tools-{name}"));
            let bytes = std::fs::read(&path).unwrap();
            let expected = std::fs::read(path.with_file_name(format!("psd-tools-{name}.rgba-le"))).unwrap();
            let profile = std::fs::read(path.with_file_name(format!("psd-tools-{name}.icc"))).unwrap();
            let retained_bytes = (expected.len()+profile.len()) as u64;
            let budget = rrrah_core::MemoryBudget::new(retained_bytes);
            let mut request = DecodeRequest::new(path);
            let short = rrrah_core::MemoryBudget::new(retained_bytes-1);
            request.memory_budget = Some(short.clone());
            let error = decode(&bytes,&request).unwrap_err();
            assert!(format!("{error:?}").contains("Memory"),"{error:?}");
            assert_eq!(short.used(),0);
            assert!(short.peak()<retained_bytes);
            request.memory_budget = Some(budget.clone());
            let raster = decode(&bytes,&request).unwrap();
            assert_eq!(raster.color_space(),&RasterColorSpace::Icc(profile));
            assert_eq!((budget.used(),budget.peak()),(retained_bytes,retained_bytes));
            assert_eq!((raster.width(),raster.height()),if name.ends_with(".psb") {(5,5)} else {(4,4)});
            match raster.pixels() {
                RasterPixels::Rgba16(values) => {
                    assert!(values.is_managed());
                    for (&value, encoded) in values.iter().zip(expected.chunks_exact(2)) { assert_eq!(value,u16::from_le_bytes(encoded.try_into().unwrap()),"{name}"); }
                    assert_eq!(values.len()*2,expected.len());
                }
                RasterPixels::Rgba32Float(values) => {
                    assert!(values.is_managed());
                    for (&value, encoded) in values.iter().zip(expected.chunks_exact(4)) { assert_eq!(value.to_bits(),u32::from_le_bytes(encoded.try_into().unwrap()),"{name}"); }
                    assert_eq!(values.len()*4,expected.len());
                }
                _ => panic!("precision changed"),
            }
            let clone = raster.clone();
            let retained = budget.used();
            drop(raster);assert_eq!(budget.used(),retained);
            drop(clone);assert_eq!(budget.used(),0);
        }
    }

    #[test]
    fn composition_buffers_respect_peak_credit_and_owner_lifetime() {
        fn samples(raster: &DecodedRaster) -> (Vec<u32>, bool) {
            match raster.pixels() {
                RasterPixels::Rgba8(v) => (v.iter().map(|&x| u32::from(x)).collect(), v.is_managed()),
                RasterPixels::Rgba16(v) => (v.iter().map(|&x| u32::from(x)).collect(), v.is_managed()),
                RasterPixels::Rgba32Float(v) => (v.iter().map(|x| x.to_bits()).collect(), v.is_managed()),
            }
        }
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/raster");
        let names = [
            "composite-1-0.psd",
            "composite-1-1.psd",
            "composite-1-2.psd",
            "composite-1-3.psd",
            "composite-2-0.psb",
            "composite-2-1.psb",
            "composite-2-2.psb",
            "composite-2-3.psb",
            "prediction-16.psd",
            "prediction-32.psd",
            "alpha-merged.psd",
        ];
        for name in names {
            let bytes = std::fs::read(root.join(name)).unwrap();
            let mut request = DecodeRequest::new(name);
            let baseline = decode(&bytes, &request).unwrap();
            let expected = samples(&baseline).0;
            let output = u64::from(baseline.width())
                * u64::from(baseline.height())
                * 4
                * u64::from(u16::from_be_bytes([bytes[22], bytes[23]]) / 8);
            let budget = rrrah_core::MemoryBudget::new(1024 * 1024);
            request.memory_budget = Some(budget.clone());
            let image = decode(&bytes, &request).unwrap();
            assert_eq!(samples(&image), (expected.clone(), true));
            let peak = budget.peak();
            assert!(peak >= output);
            if name.ends_with("-0.psd") || name.ends_with("-0.psb") {
                assert_eq!(peak, output, "raw channel planes must borrow source storage");
            }
            assert_eq!(budget.used(), output);
            let clone = image.clone();
            drop(image);
            assert_eq!(budget.used(), output);
            drop(clone);
            assert_eq!(budget.used(), 0);
            for limit in [0, output - 1, peak - 1] {
                let denied = rrrah_core::MemoryBudget::new(limit);
                request.memory_budget = Some(denied.clone());
                assert!(
                    matches!(
                        decode(&bytes, &request),
                        Err(RasterDecodeError::Source(crate::DecodeError::Memory(_)))
                    ),
                    "{name} limit={limit}"
                );
                assert_eq!(denied.used(), 0);
                assert!(denied.peak() <= limit);
            }
            let exact = rrrah_core::MemoryBudget::new(peak);
            request.memory_budget = Some(exact.clone());
            let image = decode(&bytes, &request).unwrap();
            assert_eq!(samples(&image), (expected, true));
            assert_eq!(exact.peak(), peak);
            drop(image);
            assert_eq!(exact.used(), 0);
        }
        eprintln!("PSD_BUFFER_ADMISSION cases=11 refusals=33 final=0");
    }

    #[test]
    fn zip_decoder_state_is_admitted_before_inflate_and_released_before_output() {
        let bytes = include_bytes!("../../../tests/fixtures/raster/composite-1-2.psd");
        let decoder_bytes = rrrah_scientific::DEFLATE_DECODER_CREDIT_BYTES;
        let baseline = decode(bytes, &DecodeRequest::new("zip.psd")).unwrap();
        let depth = u64::from(u16::from_be_bytes([bytes[22], bytes[23]]) / 8);
        let channels = u64::from(u16::from_be_bytes([bytes[12], bytes[13]]));
        let pixels = u64::from(baseline.width()) * u64::from(baseline.height());
        let planes = pixels * channels * depth;
        let output = pixels * 4 * depth;
        let peak = planes + decoder_bytes.max(output);
        let short = rrrah_core::MemoryBudget::new(peak - 1);
        let mut request = DecodeRequest::new("zip.psd");
        request.memory_budget = Some(short.clone());
        assert!(matches!(decode(bytes, &request),
            Err(RasterDecodeError::Source(crate::DecodeError::Memory(
                rrrah_core::BufferError::Capacity { requested, .. }
            ))) if requested == decoder_bytes));
        assert_eq!((short.used(), short.peak()), (0, planes));
        let exact = rrrah_core::MemoryBudget::new(peak);
        request.memory_budget = Some(exact.clone());
        let image = decode(bytes, &request).unwrap();
        assert_eq!((exact.used(), exact.peak()), (output, peak));
        let clone = image.clone();
        drop(image);
        assert_eq!(exact.used(), output);
        drop(clone);
        assert_eq!(exact.used(), 0);
    }

    #[test]
    fn zip_empty_blocks_are_input_bounded_cancel_and_release_credit() {
        let samples = [7u8, 11, 13, 17, 19, 23];
        let mut compressed = vec![0x78, 0x01];
        for _ in 0..100_000 {
            compressed.extend_from_slice(&[0, 0, 0, 255, 255]);
        }
        compressed.extend_from_slice(&[1, 6, 0, 249, 255]);
        compressed.extend_from_slice(&samples);
        let (mut a, mut b) = (1u32, 0u32);
        for value in samples {
            a = (a + u32::from(value)) % 65521;
            b = (b + a) % 65521;
        }
        compressed.extend_from_slice(&((b << 16) | a).to_be_bytes());
        let mut request = DecodeRequest::new("bounded.psd");
        let mut data = Vec::with_capacity(6);
        let (mut consumed, mut produced, mut calls) = (0, 0, 0);
        inflate_zip_with_progress(&compressed, &mut data, 6, &request, |input, output| {
            assert!(input - consumed <= 65536);
            assert!(output - produced <= 65536);
            consumed = input;
            produced = output;
            calls += 1;
        })
        .unwrap();
        assert_eq!(data, samples);
        assert_eq!(consumed, compressed.len() as u64);
        assert!(calls >= 8);
        let mut psd = fixture(8, &[]);
        psd[38..40].copy_from_slice(&2u16.to_be_bytes());
        psd.extend_from_slice(&compressed);
        let decoded = decode(&psd, &request).unwrap();
        let RasterPixels::Rgba8(values) = decoded.pixels() else {
            panic!()
        };
        assert_eq!(values.as_slice(), [7, 13, 19, 255, 11, 17, 23, 255]);
        let decoder_bytes = rrrah_scientific::DEFLATE_DECODER_CREDIT_BYTES;
        let budget = rrrah_core::MemoryBudget::new(6 + decoder_bytes);
        let generation = Arc::new(std::sync::atomic::AtomicU64::new(0));
        request.memory_budget = Some(budget.clone());
        request.cancellation = Some(crate::GenerationToken::new(generation.clone(), 0));
        let mut cancelled_calls = 0;
        {
            let _credit = reserve(&request, 6).unwrap();
            let mut scratch = Vec::with_capacity(6);
            let result =
                inflate_zip_with_progress(&compressed, &mut scratch, 6, &request, |input, output| {
                    assert!(input > 0 && input <= 65536);
                    assert_eq!(output, 0);
                    assert_eq!(budget.used(), 6 + decoder_bytes);
                    cancelled_calls += 1;
                    generation.store(1, std::sync::atomic::Ordering::Release);
                });
            assert!(matches!(
                result,
                Err(RasterDecodeError::Source(crate::DecodeError::Cancelled))
            ));
            assert!(scratch.is_empty());
        }
        assert_eq!(cancelled_calls, 1);
        assert_eq!(budget.used(), 0);
        request.cancellation = None;
        for prefix in [0, 1, 65535, 65536, 65537, compressed.len() - 1] {
            assert!(inflate_zip(&compressed[..prefix], &mut Vec::with_capacity(6), 6, &request).is_err());
        }
        let mut trailing = compressed.clone();
        trailing.push(0);
        assert!(inflate_zip(&trailing, &mut Vec::with_capacity(6), 6, &request).is_err());
        assert!(inflate_zip(&compressed, &mut Vec::with_capacity(5), 5, &request).is_err());
        data.clear();
        inflate_zip(&compressed, &mut data, 6, &request).unwrap();
        assert_eq!(data, samples);
        eprintln!(
            "PSD_ZIP_BOUNDED source={} calls={calls} cancel_calls=1 refusals=8 final=0",
            compressed.len()
        );
    }

    #[test]
    fn raw_planes_need_only_output_credit_for_all_sample_precisions() {
        for version in [1u16, 2] {
            for depth in [8u16, 16, 32] {
                let values: [u32; 6] = match depth {
                    8 => [0, 255, 17, 19, 31, 37],
                    16 => [0, 65535, 32768, 32769, 1, 65534],
                    _ => [
                        (-0.0f32).to_bits(),
                        1.0f32.to_bits(),
                        (-3.0f32).to_bits(),
                        0x7fc01234,
                        4.0f32.to_bits(),
                        1.125f32.to_bits(),
                    ],
                };
                let encoded: Vec<_> = values
                    .iter()
                    .flat_map(|&value| match depth {
                        8 => vec![value as u8],
                        16 => (value as u16).to_be_bytes().to_vec(),
                        _ => value.to_be_bytes().to_vec(),
                    })
                    .collect();
                let mut bytes = fixture(depth, &encoded);
                bytes[4..6].copy_from_slice(&version.to_be_bytes());
                if version == 2 {
                    bytes.splice(34..34, [0u8; 4]);
                }
                let original = bytes.clone();
                let output = 8 * u64::from(depth / 8);
                let budget = rrrah_core::MemoryBudget::new(output);
                let mut request = DecodeRequest::new("raw.psd");
                request.memory_budget = Some(budget.clone());
                let image = decode(&bytes, &request).unwrap();
                let (actual, managed): (Vec<u32>, bool) = match image.pixels() {
                    RasterPixels::Rgba8(v) => (v.iter().map(|&x| u32::from(x)).collect(), v.is_managed()),
                    RasterPixels::Rgba16(v) => (v.iter().map(|&x| u32::from(x)).collect(), v.is_managed()),
                    RasterPixels::Rgba32Float(v) => (v.iter().map(|x| x.to_bits()).collect(), v.is_managed()),
                };
                let opaque = match depth {
                    8 => 255,
                    16 => 65535,
                    _ => 1.0f32.to_bits(),
                };
                assert_eq!(
                    actual,
                    [
                        values[0], values[2], values[4], opaque, values[1], values[3], values[5], opaque
                    ]
                );
                assert!(managed);
                assert_eq!(budget.peak(), output);
                assert_eq!(budget.used(), output);
                assert_eq!(bytes, original);
                drop(image);
                assert_eq!(budget.used(), 0);
                let denied = rrrah_core::MemoryBudget::new(output - 1);
                request.memory_budget = Some(denied.clone());
                assert!(matches!(
                    decode(&bytes, &request),
                    Err(RasterDecodeError::Source(crate::DecodeError::Memory(_)))
                ));
                assert_eq!(denied.peak(), 0);
                assert_eq!(denied.used(), 0);
            }
        }
        eprintln!("PSD_RAW_BORROWED cases=6 source_unchanged=true plane_allocation=0 final=0");
    }

    #[test]
    fn icc_resource_is_admitted_before_copy_and_shared_by_raster_clones() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/raster");
        let profile = std::fs::read(root.join("dicom-rgb-profile.icc")).unwrap();
        for version in [1u16, 2] {
            let mut bytes = fixture(8, &[7, 11, 13, 17, 19, 23]);
            bytes[4..6].copy_from_slice(&version.to_be_bytes());
            if version == 2 {
                bytes.splice(34..34, [0u8; 4]);
            }
            let mut resource = b"8BIM".to_vec();
            resource.extend_from_slice(&1039u16.to_be_bytes());
            resource.extend_from_slice(&[0, 0]);
            resource.extend_from_slice(&(profile.len() as u32).to_be_bytes());
            resource.extend_from_slice(&profile);
            if profile.len() % 2 != 0 {
                resource.push(0);
            }
            bytes[30..34].copy_from_slice(&(resource.len() as u32).to_be_bytes());
            bytes.splice(34..34, resource);
            for compression in [4u16, u16::MAX] {
                let mut invalid = bytes.clone();
                let at = invalid.len() - 8;
                invalid[at..at + 2].copy_from_slice(&compression.to_be_bytes());
                let zero = rrrah_core::MemoryBudget::new(0);
                let mut request = DecodeRequest::new("unsupported-compression.psd");
                request.memory_budget = Some(zero.clone());
                assert!(matches!(
                    decode(&invalid, &request),
                    Err(RasterDecodeError::InvalidPsd("compression unsupported"))
                ));
                assert_eq!((zero.used(), zero.peak()), (0, 0));
            }
            let required = profile.len() as u64 + 8;
            let budget = rrrah_core::MemoryBudget::new(required);
            let mut request = DecodeRequest::new("profile.psd");
            request.memory_budget = Some(budget.clone());
            let raster = decode(&bytes, &request).unwrap();
            assert_eq!(raster.color_space(), &RasterColorSpace::Icc(profile.clone()));
            assert_eq!(raster.color_profile_capacity_bytes(), profile.len() as u64);
            assert_eq!(budget.used(), required);
            assert_eq!(budget.peak(), required);
            let shared = raster.clone().try_manage_pixels(&budget).unwrap();
            assert_eq!(
                budget.used(),
                required,
                "generic management must not charge ICC twice"
            );
            assert!(std::ptr::eq(raster.color_space(), shared.color_space()));
            drop(raster);
            assert_eq!(budget.used(), required);
            drop(shared);
            assert_eq!(budget.used(), 0);
            for limit in [profile.len() as u64 - 1, required - 1] {
                let denied = rrrah_core::MemoryBudget::new(limit);
                request.memory_budget = Some(denied.clone());
                assert!(matches!(
                    decode(&bytes, &request),
                    Err(RasterDecodeError::Source(crate::DecodeError::Memory(_)))
                ));
                assert_eq!(denied.used(), 0);
                assert!(denied.peak() <= limit);
            }
        }
        eprintln!(
            "PSD_ICC_ADMISSION versions=2 profile_bytes={} exact_and_short=true shared_credit=true final=0",
            profile.len()
        );
    }

    #[test]
    fn psb_rle_noop_rows_check_cancellation_and_preserve_valid_output() {
        let mut first = vec![128u8; 1_000_000];
        first.extend_from_slice(&[1, 7, 11]);
        let mut request = DecodeRequest::new("noop.psb");
        let budget = rrrah_core::MemoryBudget::new(2);
        request.memory_budget = Some(budget.clone());
        let mut calls = 0;
        {
            let _credit = reserve(&request, 2).unwrap();
            let mut out = Vec::with_capacity(2);
            let result = unpack_checked(&first, &mut out, 2, |offset| {
                calls += 1;
                if calls == 2 {
                    assert_eq!(offset, 65536);
                    return Err(crate::DecodeError::Cancelled.into());
                }
                assert_eq!(offset, 0);
                Ok(())
            });
            assert!(matches!(
                result,
                Err(RasterDecodeError::Source(crate::DecodeError::Cancelled))
            ));
            assert!(out.is_empty());
        }
        assert_eq!(budget.used(), 0);
        let mut out = Vec::with_capacity(2);
        let mut previous = 0;
        unpack_checked(&first, &mut out, 2, |offset| {
            assert!(offset - previous <= 65536 + 128);
            previous = offset;
            Ok(())
        })
        .unwrap();
        assert_eq!(out, [7, 11]);
        assert_eq!(previous, first.len());
        let mut bytes = fixture(8, &[]);
        bytes[4..6].copy_from_slice(&2u16.to_be_bytes());
        bytes.splice(34..34, [0u8; 4]);
        bytes[42..44].copy_from_slice(&1u16.to_be_bytes());
        for length in [first.len() as u32, 3, 3] {
            bytes.extend_from_slice(&length.to_be_bytes());
        }
        bytes.extend_from_slice(&first);
        bytes.extend_from_slice(&[1, 13, 17, 1, 19, 23]);
        let exact = rrrah_core::MemoryBudget::new(14);
        request.memory_budget = Some(exact.clone());
        let image = decode(&bytes, &request).unwrap();
        let RasterPixels::Rgba8(pixels) = image.pixels() else {
            panic!()
        };
        assert_eq!(pixels.as_slice(), [7, 13, 19, 255, 11, 17, 23, 255]);
        assert_eq!((exact.used(), exact.peak()), (8, 14));
        drop(image);
        assert_eq!(exact.used(), 0);
        eprintln!("PSB_RLE_BOUNDED noop_commands=1000000 cancel_at_input=65536 exact_peak=14 final=0");
    }

    #[test]
    fn pinned_original_psd_psb_rgb16_composites_match_independent_planar_oracle() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/raster");
        let manifest = include_str!("../../../tests/fixtures/raster/psd-tools-16bit-manifest.tsv");
        let mut cases = 0;
        for row in manifest
            .lines()
            .filter(|line| !line.starts_with('#') && !line.is_empty())
        {
            let fields: Vec<_> = row.split('\t').collect();
            let bytes = std::fs::read(root.join(fields[0])).unwrap();
            let oracle = std::fs::read(root.join(fields[3])).unwrap();
            let expected: Vec<_> = oracle
                .chunks_exact(2)
                .map(|b| u16::from_le_bytes(b.try_into().unwrap()))
                .collect();
            let profile = std::fs::read(root.join(fields[5])).unwrap();
            let required = oracle.len() as u64 + profile.len() as u64;
            let budget = rrrah_core::MemoryBudget::new(required);
            let mut request = DecodeRequest::new(fields[0]);
            request.memory_budget = Some(budget.clone());
            let image = decode(&bytes, &request).unwrap();
            assert_eq!((image.width(), image.height()), (5, 5));
            assert_eq!(image.color_space(), &RasterColorSpace::Icc(profile));
            let RasterPixels::Rgba16(values) = image.pixels() else {
                panic!()
            };
            assert_eq!(values.as_slice(), expected);
            assert!(values.is_managed());
            assert!(
                values.iter().any(|value| value % 257 != 0),
                "source must retain native u16 precision"
            );
            assert_eq!((budget.used(), budget.peak()), (required, required));
            let shared = image.clone();
            drop(image);
            assert_eq!(budget.used(), required);
            drop(shared);
            assert_eq!(budget.used(), 0);
            let short = rrrah_core::MemoryBudget::new(required - 1);
            request.memory_budget = Some(short.clone());
            assert!(matches!(
                decode(&bytes, &request),
                Err(RasterDecodeError::Source(crate::DecodeError::Memory(_)))
            ));
            assert_eq!(short.used(), 0);
            cases += 1;
        }
        assert_eq!(cases, 2);
        eprintln!(
            "PSD16_ORIGINAL_PRODUCER cases=2 exact_u16_samples=200 icc_exact=true exact_and_short_credit=true final=0"
        );
    }

    fn fixture(depth: u16, samples: &[u8]) -> Vec<u8> {
        let mut b = b"8BPS".to_vec();
        b.extend(1u16.to_be_bytes());
        b.extend([0; 6]);
        b.extend(3u16.to_be_bytes());
        b.extend(1u32.to_be_bytes());
        b.extend(2u32.to_be_bytes());
        b.extend(depth.to_be_bytes());
        b.extend(3u16.to_be_bytes());
        b.extend([0; 12]);
        b.extend(0u16.to_be_bytes());
        b.extend(samples);
        b
    }
    #[test]
    fn retains_sixteen_bit_precision() {
        let samples = [32768u16, 32769, 1, 2, 65534, 65535];
        let bytes = fixture(
            16,
            &samples.into_iter().flat_map(u16::to_be_bytes).collect::<Vec<_>>(),
        );
        let f = decode(&bytes, &DecodeRequest::new("image.psd")).unwrap();
        let RasterPixels::Rgba16(p) = f.pixels() else {
            panic!()
        };
        assert_eq!(p.as_slice(), [32768, 1, 65534, 65535, 32769, 2, 65535, 65535]);
    }
    #[test]
    fn explicit_srgb_assumption_applies_to_untagged_psd_and_psb() {
        for version in [1u16, 2] {
            let mut bytes = fixture(8, &[0, 255, 128, 64, 255, 0]);
            bytes[4..6].copy_from_slice(&version.to_be_bytes());
            // PSB uses an eight-byte layer/mask section length.
            if version == 2 {
                bytes.splice(34..34, [0u8; 4]);
            }
            let mut request = DecodeRequest::new("image.psd");
            assert_eq!(
                decode(&bytes, &request).unwrap().color_space(),
                &RasterColorSpace::Unspecified
            );
            request.assume_untagged_srgb = true;
            let raster = decode(&bytes, &request).unwrap();
            assert_eq!(raster.color_space(), &RasterColorSpace::AssumedSrgb);
            request.assume_untagged_linear_srgb = true;
            assert_eq!(decode(&bytes, &request).unwrap().color_space(), &RasterColorSpace::LinearSrgb);
            // This test checks resource precedence/preservation, not ICC validity.
            let mut resource = b"8BIM".to_vec();
            resource.extend(1039u16.to_be_bytes());
            resource.extend([0u8; 2]);
            resource.extend(4u32.to_be_bytes());
            resource.extend(b"test");
            bytes[30..34].copy_from_slice(&(resource.len() as u32).to_be_bytes());
            bytes.splice(34..34, resource);
            let raster = decode(&bytes, &request).unwrap();
            let RasterColorSpace::Icc(profile) = raster.color_space() else {
                panic!("ICC must win")
            };
            assert_eq!(profile.as_slice(), b"test");
        }
    }
    #[test]
    fn retains_float_values() {
        let samples = [0.5f32, 2.0, 0.1, 0.2, 0.3, 0.4];
        let bytes = fixture(
            32,
            &samples.into_iter().flat_map(f32::to_be_bytes).collect::<Vec<_>>(),
        );
        let f = decode(&bytes, &DecodeRequest::new("image.psd")).unwrap();
        let RasterPixels::Rgba32Float(p) = f.pixels() else {
            panic!()
        };
        assert_eq!(p.as_slice(), [0.5, 0.1, 0.3, 1.0, 2.0, 0.2, 0.4, 1.0]);
    }
    #[test]
    fn packbits_repeats_noop_and_row_limits() {
        let mut out = vec![];
        unpack(&[128, 254, 17], &mut out, 3).unwrap();
        assert_eq!(out, [17; 3]);
        assert!(unpack(&[254, 17], &mut vec![], 2).is_err());
        assert!(unpack(&[1, 17], &mut vec![], 2).is_err());
    }
    #[test]
    fn independent_prediction_retains_high_precision() {
        let request = DecodeRequest::new("image.psd");
        let f = decode(
            include_bytes!("../../../tests/fixtures/raster/prediction-16.psd"),
            &request,
        )
        .unwrap();
        let RasterPixels::Rgba16(p) = f.pixels() else {
            panic!()
        };
        assert_eq!(p.as_slice(), [32768, 1, 65534, 65535, 32769, 2, 65535, 65535]);
        let f = decode(
            include_bytes!("../../../tests/fixtures/raster/prediction-32.psd"),
            &request,
        )
        .unwrap();
        let RasterPixels::Rgba32Float(p) = f.pixels() else {
            panic!()
        };
        assert_eq!(p.as_slice(), [0.5, 0.1, 0.3, 1.0, 2.0, 0.2, 0.4, 1.0]);
    }
    #[test]
    fn zip_rejects_truncation_and_trailing_stream_data() {
        let bytes = include_bytes!("../../../tests/fixtures/raster/composite-1-2.psd");
        let request = DecodeRequest::new("image.psd");
        assert!(decode(&bytes[..bytes.len() - 1], &request).is_err());
        let mut extra = bytes.to_vec();
        extra.push(0);
        assert!(decode(&extra, &request).is_err());
    }
}
