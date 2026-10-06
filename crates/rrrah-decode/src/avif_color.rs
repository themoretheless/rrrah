//! Resolve color properties through primary-item ipma associations, never byte searching.
use crate::raster::RasterDecodeError;
use rrrah_core::RasterColorSpace;
fn invalid(s: &'static str) -> RasterDecodeError {
    RasterDecodeError::InvalidAvif(s)
}
fn boxes(mut b: &[u8]) -> Result<Vec<([u8; 4], &[u8])>, RasterDecodeError> {
    let mut out = Vec::new();
    while !b.is_empty() {
        if b.len() < 8 {
            return Err(invalid("truncated box header"));
        }
        let short = u32::from_be_bytes(b[..4].try_into().unwrap());
        let kind = b[4..8].try_into().unwrap();
        let (size, head) = match short {
            0 => (b.len(), 8),
            1 => {
                if b.len() < 16 {
                    return Err(invalid("truncated extended box"));
                }
                (
                    usize::try_from(u64::from_be_bytes(b[8..16].try_into().unwrap()))
                        .map_err(|_| invalid("box size overflow"))?,
                    16,
                )
            }
            _ => (short as usize, 8),
        };
        if size < head || size > b.len() {
            return Err(invalid("box outside parent"));
        }
        if out.len() >= 65536 {
            return Err(invalid("too many boxes"));
        }
        out.push((kind, &b[head..size]));
        b = &b[size..];
    }
    Ok(out)
}
fn take<'a>(b: &mut &'a [u8], n: usize) -> Result<&'a [u8], RasterDecodeError> {
    if b.len() < n {
        return Err(invalid("truncated item association"));
    }
    let (v, rest) = b.split_at(n);
    *b = rest;
    Ok(v)
}
fn number(b: &mut &[u8], n: usize) -> Result<u32, RasterDecodeError> {
    Ok(take(b, n)?.iter().fold(0, |a, &v| a * 256 + u32::from(v)))
}
#[derive(Default)]
pub(crate) struct Properties {
    pub bit_depth: Option<u8>,
    pub color: Option<RasterColorSpace>,
    pub rotation: u8,
    pub mirror: Option<u8>,
    pub clean_aperture: Option<[u8; 32]>,
    pub handled_aperture_associations: Vec<usize>,
}

#[cfg(test)]
fn declaration(bytes: &[u8]) -> Result<Option<RasterColorSpace>, RasterDecodeError> {
    Ok(properties(bytes)?.color)
}

pub(crate) fn properties(bytes: &[u8]) -> Result<Properties, RasterDecodeError> {
    if image::guess_format(bytes).ok() != Some(image::ImageFormat::Avif) {
        return Ok(Properties::default());
    }
    let top = boxes(bytes)?;
    let metas: Vec<_> = top.iter().filter(|(k, _)| k == b"meta").collect();
    if metas.len() != 1 {
        return Err(invalid("missing or duplicate meta box"));
    }
    let mut meta = metas[0].1;
    let full = take(&mut meta, 4)?;
    if full != [0; 4] {
        return Err(invalid("unsupported meta version"));
    }
    let children = boxes(meta)?;
    let mut primary = None;
    let mut iprp = None;
    for (kind, data) in children {
        match &kind {
            b"pitm" => {
                if primary.is_some() {
                    return Err(invalid("duplicate primary item"));
                }
                let mut b = data;
                let full = take(&mut b, 4)?;
                primary = Some(number(
                    &mut b,
                    match full[0] {
                        0 => 2,
                        1 => 4,
                        _ => return Err(invalid("unsupported primary-item version")),
                    },
                )?);
            }
            b"iprp" => {
                if iprp.replace(data).is_some() {
                    return Err(invalid("duplicate item properties"));
                }
            }
            _ => {}
        }
    }
    let primary = primary.ok_or_else(|| invalid("missing primary item"))?;
    let iprp = iprp.ok_or_else(|| invalid("missing item properties"))?;
    let children = boxes(iprp)?;
    let mut properties = None;
    let mut associations = Vec::new();
    for (kind, data) in children {
        match &kind {
            b"ipco" => {
                if properties.replace(boxes(data)?).is_some() {
                    return Err(invalid("duplicate property container"));
                }
            }
            b"ipma" => associations.push(data),
            _ => {}
        }
    }
    let properties = properties.ok_or_else(|| invalid("missing property container"))?;
    let mut selected = Vec::new();
    for mut b in associations {
        let full = take(&mut b, 4)?;
        if full[0] > 1 || full[1] != 0 || full[2] != 0 || full[3] > 1 {
            return Err(invalid("unsupported association version or flags"));
        }
        let entries = number(&mut b, 4)?;
        if entries > 65536 {
            return Err(invalid("too many item associations"));
        }
        for _ in 0..entries {
            let id = number(&mut b, if full[0] == 0 { 2 } else { 4 })?;
            let count = number(&mut b, 1)?;
            for _ in 0..count {
                let association_offset = b.as_ptr() as usize - bytes.as_ptr() as usize;
                let index = number(&mut b, if full[3] & 1 == 0 { 1 } else { 2 })?
                    & if full[3] & 1 == 0 { 127 } else { 32767 };
                if index > properties.len() as u32 {
                    return Err(invalid("property index outside container"));
                }
                if id == primary && index != 0 {
                    selected.push((index as usize - 1, association_offset));
                }
            }
        }
        if !b.is_empty() {
            return Err(invalid("trailing association bytes"));
        }
    }
    let mut handled_aperture_associations = Vec::new();
    let mut clean_aperture = None;
    let mut rotation = None;
    let mut mirror = None;
    let mut color = None;
    let mut bit_depth = None;
    let mut icc = None;
    for (index, association_offset) in selected {
        let (kind, data) = properties[index];
        match &kind {
            b"av1C" => {
                if data.len() < 4 || data[0] != 0x81 || data[2] & 0x60 == 0x20 {
                    return Err(invalid("invalid AV1 configuration depth"));
                }
                let depth = if data[2] & 0x40 == 0 {
                    8
                } else if data[2] & 0x20 == 0 {
                    10
                } else {
                    12
                };
                if bit_depth.replace(depth).is_some() {
                    return Err(invalid("duplicate AV1 configuration"));
                }
            }
            b"irot" => {
                if mirror.is_some() {
                    return Err(invalid("rotation after mirroring"));
                }
                if data.len() != 1 || data[0] > 3 || rotation.replace(data[0]).is_some() {
                    return Err(invalid("invalid or duplicate rotation"));
                }
            }
            b"imir" => {
                if data.len() != 1 || data[0] > 1 || mirror.replace(data[0]).is_some() {
                    return Err(invalid("invalid or duplicate mirror"));
                }
            }
            b"clap" => {
                if rotation.is_some() || mirror.is_some() {
                    return Err(invalid("clean aperture after rotation or mirroring"));
                }
                let aperture: [u8; 32] = data
                    .try_into()
                    .map_err(|_| invalid("invalid clean-aperture length"))?;
                handled_aperture_associations.push(association_offset);
                if clean_aperture.replace(aperture).is_some() {
                    return Err(invalid("duplicate clean aperture"));
                }
            }
            _ => {}
        }
        if &kind != b"colr" {
            continue;
        }
        if data.starts_with(b"prof") || data.starts_with(b"rICC") {
            if icc.replace(RasterColorSpace::Icc(data[4..].to_vec())).is_some() {
                return Err(invalid("multiple primary ICC properties"));
            }
            continue;
        }
        if color.is_some() {
            return Err(invalid("multiple primary NCLX properties"));
        }
        color = Some(if data.starts_with(b"nclx") {
            if data.len() != 11 || data[10] & 127 != 0 {
                return Err(invalid("invalid NCLX property"));
            }
            let primaries = u16::from_be_bytes([data[4], data[5]]);
            let transfer = u16::from_be_bytes([data[6], data[7]]);
            match (primaries, transfer) {
                (1, 13) => RasterColorSpace::Srgb,
                (1, 8) => RasterColorSpace::LinearSrgb,
                _ => RasterColorSpace::Cicp {
                    primaries,
                    transfer,
                    matrix: u16::from_be_bytes([data[8], data[9]]),
                    full_range: data[10] & 128 != 0,
                },
            }
        } else if data.starts_with(b"prof") || data.starts_with(b"rICC") {
            RasterColorSpace::Icc(data[4..].to_vec())
        } else {
            RasterColorSpace::Unspecified
        });
    }
    Ok(Properties {
        bit_depth,
        color: icc.or(color),
        rotation: rotation.unwrap_or(0),
        mirror,
        clean_aperture,
        handled_aperture_associations,
    })
}

/// Exact pixel-aligned clean aperture; rational metadata is never rounded.
pub(crate) fn crop_rect(data: &[u8; 32], width: u32, height: u32) -> Result<[u32; 4], RasterDecodeError> {
    let unsigned = |at| u32::from_be_bytes(data[at..at + 4].try_into().unwrap());
    let axis = |full: u32, at: usize, offset: usize| -> Result<(u32, u32), RasterDecodeError> {
        let (n, d) = (unsigned(at), unsigned(at + 4));
        let od = unsigned(offset + 4);
        if d == 0 || od == 0 || n == 0 || n % d != 0 {
            return Err(invalid("nonintegral or invalid clean-aperture extent"));
        }
        let extent = n / d;
        if extent > full {
            return Err(invalid("clean aperture exceeds image"));
        }
        let on = i32::from_be_bytes(data[offset..offset + 4].try_into().unwrap());
        let numerator = i128::from(full - extent) * i128::from(od) + 2 * i128::from(on);
        let denominator = 2 * i128::from(od);
        if numerator < 0 || numerator % denominator != 0 {
            return Err(invalid("nonintegral or negative clean-aperture origin"));
        }
        let origin = numerator / denominator;
        if origin + i128::from(extent) > i128::from(full) {
            return Err(invalid("clean aperture outside image"));
        }
        Ok((origin as u32, extent))
    };
    let (x, w) = axis(width, 0, 16)?;
    let (y, h) = axis(height, 8, 24)?;
    Ok([x, y, w, h])
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn clean_aperture_and_orientation_match_independent_libavif_metadata() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/raster");
        let oracle = include_str!("../../../tests/fixtures/raster/avif-crop-libavif-1.4.2.tsv");
        let mut count = 0;
        for line in oracle.lines() {
            let fields: Vec<_> = line.split('\t').collect();
            assert_eq!(fields.len(), 10);
            let values: Vec<u32> = fields[1..].iter().map(|v| v.parse().unwrap()).collect();
            let bytes = std::fs::read(root.join(fields[0])).unwrap();
            let parsed = properties(&bytes).unwrap();
            let rect = crop_rect(&parsed.clean_aperture.unwrap(), values[0], values[1]).unwrap();
            assert_eq!(rect.as_slice(), &values[2..6], "{}", fields[0]);
            assert_eq!(u32::from(parsed.rotation), values[7], "{}", fields[0]);
            let mirror = (values[6] & 8 != 0).then_some(values[8] as u8);
            assert_eq!(parsed.mirror, mirror, "{}", fields[0]);
            if fields[0].contains("top-left") {
                assert_eq!(&rect[..2], &[0, 0]);
            }
            count += 1;
        }
        assert_eq!(count, 24);
    }
    #[test]
    fn primary_transform_order_is_validated_before_codec_decode() {
        let original =
            include_bytes!("../../../tests/fixtures/raster/avif-clean-aperture-orientation-5.avif");
        let parsed = properties(original).unwrap();
        let crop = parsed.handled_aperture_associations[0];
        // This fixture associates clap, irot, imir consecutively. Reorder only
        // those associations, preserving property payloads and coded extents.
        let transforms = [original[crop], original[crop + 1], original[crop + 2]];
        assert_eq!(transforms, [0x87, 0x85, 0x86]);
        for order in [[0, 2, 1], [1, 0, 2], [1, 2, 0], [2, 0, 1], [2, 1, 0]] {
            let mut malformed = original.to_vec();
            for (slot, index) in order.into_iter().enumerate() {
                malformed[crop + slot] = transforms[index];
            }
            assert!(matches!(
                properties(&malformed),
                Err(RasterDecodeError::InvalidAvif(_))
            ));
        }
        assert!(parsed.clean_aperture.is_some());
        assert!(parsed.mirror.is_some());
    }
    #[test]
    fn clean_aperture_follows_primary_association_and_rejects_duplicates() {
        fn bx(tag: &[u8; 4], payload: &[u8]) -> Vec<u8> {
            let mut out = ((payload.len() + 8) as u32).to_be_bytes().to_vec();
            out.extend(tag);
            out.extend(payload);
            out
        }
        let aperture: Vec<u8> = [4u32, 1, 2, 1, 0, 1, 0, 1]
            .into_iter()
            .flat_map(u32::to_be_bytes)
            .collect();
        let build = |item: u16, count: u8| {
            let mut association = vec![0, 0, 0, 0, 0, 0, 0, 1];
            association.extend(item.to_be_bytes());
            association.push(count);
            association.extend(std::iter::repeat_n(1u8, count as usize));
            let mut iprp = bx(b"ipco", &bx(b"clap", &aperture));
            iprp.extend(bx(b"ipma", &association));
            let mut meta = vec![0; 4];
            meta.extend(bx(b"pitm", &[0, 0, 0, 0, 0, 1]));
            meta.extend(bx(b"iprp", &iprp));
            let mut image = bx(b"ftyp", b"avif\0\0\0\0avif");
            image.extend(bx(b"meta", &meta));
            image
        };
        assert_eq!(
            properties(&build(1, 1))
                .unwrap()
                .clean_aperture
                .unwrap()
                .as_slice(),
            aperture
        );
        assert!(properties(&build(2, 1)).unwrap().clean_aperture.is_none());
        assert!(properties(&build(1, 2)).is_err());
    }
    #[test]
    fn exact_clean_aperture_signed_offsets_and_bounds() {
        let header = |values: [i64; 8]| {
            let mut bytes = [0u8; 32];
            for (slot, value) in bytes.chunks_exact_mut(4).zip(values) {
                slot.copy_from_slice(&(value as u32).to_be_bytes());
            }
            bytes
        };
        assert_eq!(
            crop_rect(&header([4, 1, 2, 1, -1, 1, 0, 1]), 8, 6).unwrap(),
            [1, 2, 4, 2]
        );
        assert_eq!(
            crop_rect(&header([4, 1, 2, 1, -1, 2, 1, 2]), 7, 5).unwrap(),
            [1, 2, 4, 2]
        );
        assert_eq!(
            crop_rect(&header([8, 2, 4, 2, 0, 1, 0, 1]), 8, 6).unwrap(),
            [2, 2, 4, 2]
        );
        for values in [
            [4, 0, 2, 1, 0, 1, 0, 1],
            [3, 2, 2, 1, 0, 1, 0, 1],
            [4, 1, 2, 1, 0, 0, 0, 1],
            [4, 1, 2, 1, -3, 1, 0, 1],
            [9, 1, 2, 1, 0, 1, 0, 1],
            [4, 1, 2, 1, 3, 1, 0, 1],
        ] {
            assert!(crop_rect(&header(values), 8, 6).is_err());
        }
        assert!(crop_rect(&header([4, 1, 2, 1, 0, 1, 0, 1]), 7, 5).is_err());
    }
    #[test]
    fn primary_color_association_and_icc_precedence() {
        assert_eq!(
            declaration(include_bytes!("../../../tests/fixtures/raster/avif-rgb.avif")).unwrap(),
            Some(RasterColorSpace::Srgb)
        );
        assert!(matches!(
            declaration(include_bytes!(
                "../../../tests/fixtures/raster/avif-alpha-icc.avif"
            ))
            .unwrap(),
            Some(RasterColorSpace::Icc(_))
        ));
    }
    #[test]
    fn invalid_nclx_and_unknown_transfer() {
        let original = include_bytes!("../../../tests/fixtures/raster/avif-rgb.avif");
        let at = original.windows(4).position(|v| v == b"nclx").unwrap();
        for (transfer, full_range) in [(16u16, true), (18u16, false)] {
            let mut hdr = original.to_vec();
            hdr[at + 4..at + 6].copy_from_slice(&9u16.to_be_bytes());
            hdr[at + 6..at + 8].copy_from_slice(&transfer.to_be_bytes());
            hdr[at + 8..at + 10].copy_from_slice(&9u16.to_be_bytes());
            hdr[at + 10] = if full_range { 128 } else { 0 };
            assert_eq!(
                declaration(&hdr).unwrap(),
                Some(RasterColorSpace::Cicp {
                    primaries: 9,
                    transfer,
                    matrix: 9,
                    full_range,
                })
            );
        }
        let mut b = original.to_vec();
        b[at + 10] |= 1;
        assert!(declaration(&b).is_err());
        let mut b = original.to_vec();
        b[at + 6..at + 8].copy_from_slice(&16u16.to_be_bytes());
        assert!(matches!(
            declaration(&b).unwrap(),
            Some(RasterColorSpace::Cicp {
                primaries: 1,
                transfer: 16,
                ..
            })
        ));
        let mut b = original.to_vec();
        b[at + 6..at + 8].copy_from_slice(&8u16.to_be_bytes());
        assert_eq!(declaration(&b).unwrap(), Some(RasterColorSpace::LinearSrgb));
    }
    #[test]
    fn truncated_and_overflowing_boxes_are_errors() {
        assert!(boxes(&[0; 7]).is_err());
        assert!(boxes(&[0, 0, 0, 1, 0, 0, 0, 0]).is_err());
        assert!(boxes(&[255; 8]).is_err());
    }
    #[test]
    fn invalid_transform_reserved_bits_are_rejected() {
        let original = include_bytes!("../../../tests/fixtures/raster/avif-orientation-5.avif");
        for tag in [b"irot", b"imir"] {
            let offset = original.windows(4).position(|v| v == tag).unwrap() + 4;
            let mut b = original.to_vec();
            b[offset] |= 128;
            assert!(properties(&b).is_err());
        }
    }
}
