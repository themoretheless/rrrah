//! Selected 2D DDS mip, BC1/BC2/BC3 and ordinary RGB, retaining straight alpha.
use crate::{
    DecodeRequest,
    raster::{MAX_RASTER_BYTES, RasterDecodeError},
};
use rrrah_core::{DecodedRaster, RasterColorSpace, RasterPixels};
use std::sync::Arc;
fn invalid(s: &'static str) -> RasterDecodeError {
    RasterDecodeError::InvalidDds(s)
}
fn u32le(b: &[u8], i: usize) -> u32 {
    u32::from_le_bytes(b[i..i + 4].try_into().unwrap())
}
fn rgb565(v: u16) -> [u8; 4] {
    let r = (v >> 11) & 31;
    let g = (v >> 5) & 63;
    let b = v & 31;
    [
        (r << 3 | r >> 2) as u8,
        (g << 2 | g >> 4) as u8,
        (b << 3 | b >> 2) as u8,
        255,
    ]
}
fn block(b: &[u8], variant: u8) -> [[u8; 4]; 16] {
    let c = if variant == 1 { b } else { &b[8..] };
    let a = u16::from_le_bytes([c[0], c[1]]);
    let z = u16::from_le_bytes([c[2], c[3]]);
    let mut colors = [rgb565(a), rgb565(z), [0; 4], [0; 4]];
    if a > z || variant != 1 {
        for k in 0..3 {
            colors[2][k] = ((2 * u16::from(colors[0][k]) + u16::from(colors[1][k])) / 3) as u8;
            colors[3][k] = ((u16::from(colors[0][k]) + 2 * u16::from(colors[1][k])) / 3) as u8;
        }
        colors[2][3] = 255;
        colors[3][3] = 255;
    } else {
        for k in 0..3 {
            colors[2][k] = ((u16::from(colors[0][k]) + u16::from(colors[1][k])) / 2) as u8;
        }
        colors[2][3] = 255;
    }
    let indices = u32le(c, 4);
    let mut out = [[0; 4]; 16];
    let mut alpha = [0u8; 8];
    let mut alpha_indices = 0u64;
    if variant == 3 {
        alpha[0] = b[0];
        alpha[1] = b[1];
        if b[0] > b[1] {
            for i in 2..8 {
                alpha[i] = (((8 - i) * usize::from(b[0]) + (i - 1) * usize::from(b[1])) / 7) as u8;
            }
        } else {
            for i in 2..6 {
                alpha[i] = (((6 - i) * usize::from(b[0]) + (i - 1) * usize::from(b[1])) / 5) as u8;
            }
            alpha[7] = 255;
        }
        for i in 0..6 {
            alpha_indices |= u64::from(b[2 + i]) << (i * 8);
        }
    }
    for i in 0..16 {
        out[i] = colors[((indices >> (i * 2)) & 3) as usize];
        if variant == 2 {
            out[i][3] = ((b[i / 2] >> (4 * (i % 2))) & 15) * 17;
        }
        if variant == 3 {
            out[i][3] = alpha[((alpha_indices >> (i * 3)) & 7) as usize];
        }
    }
    out
}
struct SelectedMip {
    start: usize,
    width: u32,
    height: u32,
    count: usize,
}
fn select_mip(
    bytes: &[u8],
    request: &DecodeRequest,
    start: usize,
    width: u32,
    height: u32,
    length: impl Fn(u32, u32, usize) -> Result<usize, RasterDecodeError>,
) -> Result<SelectedMip, RasterDecodeError> {
    let declared = u32le(bytes, 28) as usize;
    let count = if declared == 0 && u32le(bytes, 8) & 0x20000 == 0 {
        1
    } else {
        declared
    };
    if count == 0 || count > (u32::BITS - width.max(height).leading_zeros()) as usize {
        return Err(invalid("invalid mip count"));
    }
    let mut offset = start;
    let mut selected = None;
    for level in 0..count {
        request.check_cancelled()?;
        let w = (width >> level).max(1);
        let h = (height >> level).max(1);
        let end = offset
            .checked_add(length(w, h, level)?)
            .filter(|v| *v <= bytes.len())
            .ok_or_else(|| invalid("truncated mip storage"))?;
        if level == request.image_index {
            selected = Some(SelectedMip {
                start: offset,
                width: w,
                height: h,
                count,
            });
        }
        offset = end;
    }
    if offset != bytes.len() {
        return Err(invalid("trailing texture storage"));
    }
    selected.ok_or_else(|| {
        crate::DecodeError::UnsupportedImageIndex {
            index: request.image_index,
        }
        .into()
    })
}
struct PixelLayout {
    start: usize,
    bits: u32,
    masks: [u32; 4],
    color: RasterColorSpace,
}

fn uncompressed(
    bytes: &[u8],
    request: &DecodeRequest,
    width: u32,
    height: u32,
    layout: Option<PixelLayout>,
) -> Result<DecodedRaster, RasterDecodeError> {
    let layout = if let Some(layout) = layout {
        layout
    } else {
        let flags = u32le(bytes, 80);
        let bits = u32le(bytes, 88);
        if flags & 0x40 == 0 || flags & !0x41 != 0 || !matches!(bits, 16 | 24 | 32) {
            return Err(invalid("unsupported uncompressed pixel layout"));
        }
        if flags & 1 != 0 && u32le(bytes, 104) == 0 {
            return Err(invalid("missing alpha mask"));
        }
        PixelLayout {
            start: 128,
            bits,
            masks: [
                u32le(bytes, 92),
                u32le(bytes, 96),
                u32le(bytes, 100),
                if flags & 1 != 0 { u32le(bytes, 104) } else { 0 },
            ],
            color: RasterColorSpace::AssumedSrgb,
        }
    };
    let bits = layout.bits;
    let masks = layout.masks;
    let mut used = 0u32;
    for (i, &mask) in masks.iter().enumerate() {
        if mask == 0 {
            if i < 3 {
                return Err(invalid("missing channel mask"));
            }
            continue;
        }
        let value = mask >> mask.trailing_zeros();
        if value & value.wrapping_add(1) != 0 || mask & used != 0 || bits < 32 && mask >> bits != 0 {
            return Err(invalid("invalid or overlapping channel mask"));
        }
        used |= mask;
    }
    let size = (bits / 8) as usize;
    let row_size = width as usize * size;
    let pitch = if u32le(bytes, 8) & 8 != 0 {
        u32le(bytes, 20) as usize
    } else {
        row_size
    };
    if pitch < row_size {
        return Err(invalid("pitch smaller than row"));
    }
    let mip = select_mip(bytes, request, layout.start, width, height, |w, h, level| {
        if u32le(bytes, 28) > 1 && pitch != row_size {
            return Err(invalid(
                "padded uncompressed mip chains require pitch interpretation",
            ));
        }
        let row = if level == 0 { pitch } else { w as usize * size };
        let length = row as u64 * u64::from(h);
        if length > MAX_RASTER_BYTES {
            return Err(RasterDecodeError::OutputTooLarge);
        }
        Ok(length as usize)
    })?;
    let width = mip.width;
    let height = mip.height;
    let row_size = width as usize * size;
    let pitch = if request.image_index == 0 { pitch } else { row_size };
    let total = (pitch as u64) * u64::from(height);
    if total > MAX_RASTER_BYTES {
        return Err(RasterDecodeError::OutputTooLarge);
    }
    let raw = bytes
        .get(mip.start..mip.start + total as usize)
        .ok_or_else(|| invalid("truncated uncompressed image"))?;
    let mut pixels = Vec::new();
    pixels
        .try_reserve_exact(width as usize * height as usize * 4)
        .map_err(|_| RasterDecodeError::OutputTooLarge)?;
    for row in raw.chunks_exact(pitch) {
        request.check_cancelled()?;
        for pixel in row[..row_size].chunks_exact(size) {
            let mut packed = [0; 4];
            packed[..size].copy_from_slice(pixel);
            let value = u32::from_le_bytes(packed);
            for &mask in &masks {
                if mask == 0 {
                    pixels.push(255);
                } else {
                    let shift = mask.trailing_zeros();
                    let max = u64::from(mask >> shift);
                    let sample = u64::from((value & mask) >> shift);
                    pixels.push(((sample * 255 + max / 2) / max) as u8);
                }
            }
        }
    }
    Ok(DecodedRaster::new(
        width,
        height,
        RasterPixels::Rgba8(Arc::new(pixels).into()),
        layout.color,
    )?
    .with_image_selection(request.image_index, mip.count)?)
}

pub(crate) fn decode(bytes: &[u8], request: &DecodeRequest) -> Result<DecodedRaster, RasterDecodeError> {
    request.check_cancelled()?;
    if bytes.len() < 128 || !bytes.starts_with(b"DDS ") || u32le(bytes, 4) != 124 || u32le(bytes, 76) != 32 {
        return Err(invalid("invalid header"));
    }
    if u32le(bytes, 8) & 0x1007 != 0x1007 || u32le(bytes, 108) & 0x1000 == 0 {
        return Err(invalid("missing required DDS header flags"));
    }
    let width = u32le(bytes, 16);
    let height = u32le(bytes, 12);
    if width == 0 || height == 0 || u64::from(width) * u64::from(height) > MAX_RASTER_BYTES / 4 {
        return Err(RasterDecodeError::OutputTooLarge);
    }
    if u32le(bytes, 112) != 0 || u32le(bytes, 24) > 1 {
        return Err(invalid("cube and volume textures require selection"));
    }
    if u32le(bytes, 80) & 4 == 0 {
        return uncompressed(bytes, request, width, height, None);
    }
    let mut start = 128;
    let mut opaque = false;
    let mut color = RasterColorSpace::AssumedSrgb;
    let variant = match &bytes[84..88] {
        b"DXT1" => 1,
        b"DXT3" => 2,
        b"DXT5" => 3,
        b"DX10" => {
            if bytes.len() < 148 {
                return Err(invalid("truncated DX10 header"));
            }
            start = 148;
            if u32le(bytes, 132) != 3 || u32le(bytes, 136) != 0 || u32le(bytes, 140) != 1 {
                return Err(invalid("DDS array, cube or non-2D texture requires selection"));
            }
            if !matches!(u32le(bytes, 144), 0 | 1 | 3) {
                return Err(invalid("premultiplied or custom alpha unsupported"));
            }
            opaque = u32le(bytes, 144) == 3;
            let format = u32le(bytes, 128);
            if matches!(format, 28 | 29 | 87 | 88 | 91 | 93) {
                let color = if matches!(format, 29 | 91 | 93) {
                    RasterColorSpace::Srgb
                } else {
                    RasterColorSpace::LinearSrgb
                };
                let mut masks = if matches!(format, 28 | 29) {
                    [0xff, 0xff00, 0xff0000, 0xff000000]
                } else {
                    [0xff0000, 0xff00, 0xff, 0xff000000]
                };
                if opaque || matches!(format, 88 | 93) {
                    masks[3] = 0;
                }
                return uncompressed(
                    bytes,
                    request,
                    width,
                    height,
                    Some(PixelLayout {
                        start: 148,
                        bits: 32,
                        masks,
                        color,
                    }),
                );
            }
            color = match format {
                72 | 75 | 78 => RasterColorSpace::Srgb,
                71 | 74 | 77 => RasterColorSpace::LinearSrgb,
                _ => return Err(invalid("unsupported or typeless DXGI format")),
            };
            match format {
                71 | 72 => 1,
                74 | 75 => 2,
                77 | 78 => 3,
                _ => unreachable!(),
            }
        }
        _ => return Err(invalid("unsupported FourCC")),
    };
    let size = if variant == 1 { 8 } else { 16 };
    let mip = select_mip(bytes, request, start, width, height, |w, h, _| {
        Ok(w.div_ceil(4) as usize * h.div_ceil(4) as usize * size)
    })?;
    let width = mip.width;
    let height = mip.height;
    let start = mip.start;
    let columns = width.div_ceil(4) as usize;
    let rows = height.div_ceil(4) as usize;
    let size = if variant == 1 { 8 } else { 16 };
    let encoded = bytes
        .get(start..start + columns * rows * size)
        .ok_or_else(|| invalid("truncated block storage"))?;
    let mut pixels = Vec::new();
    pixels
        .try_reserve_exact(width as usize * height as usize * 4)
        .map_err(|_| RasterDecodeError::OutputTooLarge)?;
    pixels.resize(width as usize * height as usize * 4, 0);
    for y in 0..rows {
        request.check_cancelled()?;
        for x in 0..columns {
            let offset = (y * columns + x) * size;
            let decoded = block(&encoded[offset..offset + size], variant);
            for dy in 0..4 {
                for dx in 0..4 {
                    let px = x * 4 + dx;
                    let py = y * 4 + dy;
                    if px < width as usize && py < height as usize {
                        let dst = (py * width as usize + px) * 4;
                        pixels[dst..dst + 4].copy_from_slice(&decoded[dy * 4 + dx]);
                    }
                }
            }
        }
    }
    if opaque {
        for pixel in pixels.chunks_exact_mut(4) {
            pixel[3] = 255;
        }
    }
    Ok(
        DecodedRaster::new(width, height, RasterPixels::Rgba8(Arc::new(pixels).into()), color)?
            .with_image_selection(request.image_index, mip.count)?,
    )
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn selected_mips_match_standalone_pillow_oracles() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/raster");
        let manifest = std::fs::read_to_string(root.join("dds-mip-manifest.tsv")).unwrap();
        for line in manifest.lines().filter(|s| !s.starts_with('#') && !s.is_empty()) {
            let c: Vec<_> = line.split('\t').collect();
            let mut request = DecodeRequest::new(root.join(c[0]));
            request.image_index = c[1].parse().unwrap();
            let raster = crate::decode_raster(&request).unwrap();
            assert_eq!(
                (raster.width(), raster.height()),
                (c[2].parse().unwrap(), c[3].parse().unwrap())
            );
            assert_eq!(
                (raster.image_index(), raster.image_count()),
                (request.image_index, 4)
            );
            let color = match c[4] {
                "linear" => RasterColorSpace::LinearSrgb,
                "srgb" => RasterColorSpace::Srgb,
                _ => RasterColorSpace::AssumedSrgb,
            };
            assert_eq!(raster.color_space(), &color);
            let RasterPixels::Rgba8(values) = raster.pixels() else {
                panic!()
            };
            assert_eq!(
                &values[..],
                std::fs::read(root.join(c[5])).unwrap(),
                "{} mip {}",
                c[0],
                c[1]
            );
            let prepared = crate::prepare_raster_for_display(&raster).unwrap();
            assert_eq!(
                (prepared.image_index(), prepared.image_count()),
                (request.image_index, 4)
            );
            let bytes = std::fs::read(&request.path).unwrap();
            assert!(decode(&bytes[..bytes.len() - 1], &request).is_err());
            request.image_index = 4;
            assert!(matches!(
                decode(&bytes, &request),
                Err(RasterDecodeError::Source(
                    crate::DecodeError::UnsupportedImageIndex { index: 4 }
                ))
            ));
        }
    }
    #[test]
    fn invalid_mip_count_trailing_storage_and_ambiguous_pitch_are_rejected() {
        let original = include_bytes!("../../../tests/fixtures/raster/bgra32.dds");
        let request = DecodeRequest::new("synthetic.dds");
        for count in [3u32, 32] {
            let mut bytes = original.to_vec();
            bytes[28..32].copy_from_slice(&count.to_le_bytes());
            assert!(decode(&bytes, &request).is_err());
        }
        let mut bytes = original.to_vec();
        bytes.push(0);
        assert!(decode(&bytes, &request).is_err());
        let mut bytes = original.to_vec();
        bytes[28..32].copy_from_slice(&2u32.to_le_bytes());
        bytes[20..24].copy_from_slice(&16u32.to_le_bytes());
        bytes.resize(128 + 48 + 4, 0);
        assert!(matches!(
            decode(&bytes, &request),
            Err(RasterDecodeError::InvalidDds(
                "padded uncompressed mip chains require pitch interpretation"
            ))
        ));
        let mut bytes = original.to_vec();
        bytes[28..32].fill(0);
        assert!(decode(&bytes, &request).is_ok());
        let flags = u32le(&bytes, 8) | 0x20000;
        bytes[8..12].copy_from_slice(&flags.to_le_bytes());
        assert!(decode(&bytes, &request).is_err());
    }
    #[test]
    fn bc1_transparency() {
        let p = block(&[0, 0, 255, 255, 255, 255, 255, 255], 1);
        assert_eq!(p, [[0; 4]; 16]);
    }
    #[test]
    fn bc2_explicit_alpha() {
        let mut b = [255; 16];
        b[0] = 0x80;
        b[8..12].copy_from_slice(&[0, 248, 0, 0]);
        b[12..].fill(0);
        let p = block(&b, 2);
        assert_eq!(p[0], [255, 0, 0, 0]);
        assert_eq!(p[1], [255, 0, 0, 136]);
    }
    #[test]
    fn bc3_alpha_endpoints() {
        let mut b = [0; 16];
        b[1] = 255;
        b[2] = 63;
        let p = block(&b, 3);
        assert_eq!(p[0][3], 255);
        assert_eq!(p[1][3], 255);
    }
    fn dx10(format: u32, alpha: u32) -> Vec<u8> {
        let mut b = include_bytes!("../../../tests/fixtures/raster/bc1-alpha.dds")[..128].to_vec();
        b[84..88].copy_from_slice(b"DX10");
        for v in [format, 3, 0, 1, alpha] {
            b.extend(v.to_le_bytes());
        }
        b.extend([0, 0, 255, 255, 255, 255, 255, 255]);
        b
    }
    #[test]
    fn dx10_color_and_opaque_alpha() {
        for (format, color) in [(71, RasterColorSpace::LinearSrgb), (72, RasterColorSpace::Srgb)] {
            let f = decode(&dx10(format, 3), &DecodeRequest::new("unused")).unwrap();
            assert_eq!(f.color_space(), &color);
            let RasterPixels::Rgba8(p) = f.pixels() else {
                panic!()
            };
            assert!(p.chunks_exact(4).all(|p| p[3] == 255));
        }
    }
    #[test]
    fn malformed_and_unsupported_dx10_are_typed_errors() {
        for b in [dx10(70, 0), dx10(72, 2), dx10(72, 4)] {
            assert!(matches!(
                decode(&b, &DecodeRequest::new("unused")),
                Err(RasterDecodeError::InvalidDds(_))
            ));
        }
        let b = dx10(72, 1);
        for length in [4, 127, 140, 150] {
            assert!(decode(&b[..length], &DecodeRequest::new("unused")).is_err());
        }
        let mut b = b;
        b[140..144].copy_from_slice(&2u32.to_le_bytes());
        assert!(decode(&b, &DecodeRequest::new("unused")).is_err());
    }
    #[test]
    fn uncompressed_masks_pitch_and_bounds() {
        let base = include_bytes!("../../../tests/fixtures/raster/bgra32.dds");
        for (offset, value) in [(20, 8u32), (96, 0xff0000), (92, 0x00f000f0), (104, 0)] {
            let mut b = base.to_vec();
            b[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
            assert!(decode(&b, &DecodeRequest::new("unused")).is_err());
        }
        assert!(decode(&base[..base.len() - 1], &DecodeRequest::new("unused")).is_err());
    }
    #[test]
    fn uncompressed_dx10_transfer_and_bgrx_alpha() {
        let b = include_bytes!("../../../tests/fixtures/raster/dx10-28.dds");
        for format in [28u32, 29, 87, 88, 91, 93] {
            let mut data = b.to_vec();
            data[128..132].copy_from_slice(&format.to_le_bytes());
            let f = decode(&data, &DecodeRequest::new("unused")).unwrap();
            assert_eq!(
                f.color_space(),
                &if matches!(format, 29 | 91 | 93) {
                    RasterColorSpace::Srgb
                } else {
                    RasterColorSpace::LinearSrgb
                }
            );
            let RasterPixels::Rgba8(p) = f.pixels() else {
                panic!()
            };
            if matches!(format, 88 | 93) {
                assert!(p.chunks_exact(4).all(|p| p[3] == 255));
            } else {
                assert_eq!(p[3], 0);
                assert_eq!(p[7], 128);
            }
        }
    }
}
