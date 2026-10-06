//! Bounded vendor WB layouts. Unknown versions fail instead of guessing.
use super::{CameraFile, camera_error, optional_scalar};
use crate::{
    DecodeError,
    dng::tiff::{Ifd, Limits, Tiff},
};

fn values(format: &'static str, ifd: &Ifd<'_>, tag: u16) -> Result<Option<Vec<f64>>, DecodeError> {
    ifd.entry(tag)
        .map_err(|e| camera_error(format, e.to_string()))?
        .map(|entry| {
            entry
                .numeric_values()
                .map_err(|e| camera_error(format, e.to_string()))
        })
        .transpose()
}

fn wb_from_ifd<'a>(
    format: &'static str,
    ifd: &Ifd<'a>,
    child: impl Fn(u64) -> Result<Ifd<'a>, DecodeError>,
) -> Result<[f32; 4], DecodeError> {
    match format {
        "CR2" => {
            let data = values(format, ifd, 0x4001)?
                .ok_or_else(|| camera_error(format, "missing Canon ColorData"))?;
            let offset = match data.len() {
                582 => 25,
                653 => 34,
                _ => match data.first().copied() {
                    Some(
                        1.0 | 2.0 | 3.0 | 4.0 | 5.0 | 6.0 | 7.0 | 9.0 | 10.0 | 11.0 | 12.0 | 13.0 | 14.0
                        | 15.0,
                    ) => 63,
                    Some(65532.0 | 65533.0 | -4.0 | -3.0) => 71,
                    _ => return Err(camera_error(format, "unsupported Canon ColorData version")),
                },
            };
            let v = data
                .get(offset..offset + 4)
                .ok_or_else(|| camera_error(format, "truncated Canon ColorData WB"))?;
            super::color::gains(format, &[v[0], v[1], v[3], v[2]], false)
        }
        "PEF" => {
            let v = values(format, ifd, 0x0201)?
                .ok_or_else(|| camera_error(format, "missing Pentax WB_RGGBLevels"))?;
            if v.len() != 4 {
                return Err(camera_error(format, "Pentax WB requires four channels"));
            }
            super::color::gains(format, &[v[0], v[1], v[3], v[2]], false)
        }
        "ORF" => {
            if let (Some(r), Some(b)) = (values(format, ifd, 0x1017)?, values(format, ifd, 0x1018)?) {
                if r.len() != 1 || b.len() != 1 {
                    return Err(camera_error(format, "invalid Olympus WB multiplier count"));
                }
                return super::color::gains(format, &[r[0], 256.0, b[0]], false);
            }
            let pointer = ifd
                .entry(0x2040)
                .map_err(|e| camera_error(format, e.to_string()))?
                .ok_or_else(|| camera_error(format, "missing Olympus ImageProcessing"))?
                .unsigned_scalar()
                .map_err(|e| camera_error(format, e.to_string()))?;
            let processing = child(pointer)?;
            let v = values(format, &processing, 0x0100)?
                .ok_or_else(|| camera_error(format, "missing Olympus WB_RBLevels"))?;
            if v.len() != 2 && v.len() != 4 {
                return Err(camera_error(format, "invalid Olympus WB_RBLevels count"));
            }
            super::color::gains(format, &[v[0], 256.0, v[1]], false)
        }
        _ => Err(camera_error(format, "unsupported WB makernote")),
    }
}

pub(super) fn maker_note<'a>(file: &CameraFile<'a>, format: &'static str) -> Result<&'a [u8], DecodeError> {
    for directory in file.directories() {
        if let Some(entry) = directory.entry(format, 0x927c)? {
            return Ok(entry.raw_bytes());
        }
        if let Some(offset) = optional_scalar(format, directory, 0x8769)? {
            let exif = file.parse_ifd_at(format, offset)?;
            if let Some(entry) = exif
                .entry(0x927c)
                .map_err(|e| camera_error(format, e.to_string()))?
            {
                return Ok(entry.raw_bytes());
            }
        }
    }
    Err(camera_error(format, "missing camera MakerNote"))
}

pub(super) fn white_balance(file: &CameraFile<'_>, format: &'static str) -> Result<[f32; 4], DecodeError> {
    from_note(file, format, maker_note(file, format)?)
}

pub(super) fn apply_canon_levels(
    file: &CameraFile<'_>,
    metadata: &mut super::CameraMetadata,
) -> Result<(), DecodeError> {
    let format = "CR2";
    let note = maker_note(file, format)?;
    let offset = (note.as_ptr() as usize)
        .checked_sub(file.data().as_ptr() as usize)
        .ok_or_else(|| camera_error(format, "Canon MakerNote offset overflow"))?;
    let ifd = file.parse_ifd_at(format, offset as u64)?;
    if let Some(data) = values(format, &ifd, 0x4001)? {
        let offset = match data.first().copied() {
            Some(4.0 | 5.0) => Some(692),
            Some(6.0 | 7.0) => Some(715),
            Some(9.0) => Some(719),
            Some(65532.0 | -4.0) => Some(333),
            Some(65533.0 | -3.0) => Some(264),
            Some(10.0) => Some(if matches!(data.len(), 1273 | 1275) {
                479
            } else {
                504
            }),
            Some(11.0) => Some(728),
            Some(12.0 | 13.0 | 15.0) => Some(778),
            Some(14.0) => Some(556),
            _ => None,
        };
        if let Some(offset) = offset {
            let black = data
                .get(offset..offset + 4)
                .ok_or_else(|| camera_error(format, "truncated Canon black levels"))?;
            let shift = 14_u8.saturating_sub(metadata.bits_per_sample);
            let values = black
                .iter()
                .map(|value| {
                    if !value.is_finite() || *value < 0.0 || *value > 65535.0 || value.fract() != 0.0 {
                        return Err(camera_error(format, "invalid Canon black level"));
                    }
                    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
                    let integer = *value as u16;
                    let scaled = if matches!(data.first().copied(), Some(65533.0 | -3.0)) {
                        integer
                    } else {
                        integer >> shift
                    };
                    Ok(f32::from(scaled))
                })
                .collect::<Result<Vec<_>, DecodeError>>()?;
            metadata.black_level = rrrah_core::LevelGrid {
                width: 2,
                height: 2,
                components: 1,
                values,
            };
        }
    }
    if let Some(sensor) = values(format, &ifd, 0xe0)? {
        if sensor.len() < 9 {
            return Err(camera_error(format, "truncated Canon SensorInfo"));
        }
        let coord = |index: usize| -> Result<u32, DecodeError> {
            let v = sensor[index];
            if !v.is_finite() || v < 0.0 || v > f64::from(u32::MAX) || v.fract() != 0.0 {
                return Err(camera_error(format, "invalid Canon sensor border"));
            }
            #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
            Ok(v as u32)
        };
        let (left, top, right, bottom) = (coord(5)?, coord(6)?, coord(7)?, coord(8)?);
        if right < left || bottom < top || right >= metadata.width || bottom >= metadata.height {
            return Err(camera_error(format, "Canon sensor borders outside mosaic"));
        }
        metadata.crop_area = Some(rrrah_core::Rect::new(
            left,
            top,
            right - left + 1,
            bottom - top + 1,
        ));
    }
    Ok(())
}

pub(super) fn apply_pentax_levels(
    file: &CameraFile<'_>,
    metadata: &mut super::CameraMetadata,
) -> Result<(), DecodeError> {
    let format = "PEF";
    let note = maker_note(file, format)?;
    if !note.starts_with(b"AOC\0") {
        return Err(camera_error(
            format,
            "unsupported Pentax level metadata signature",
        ));
    }
    let offset = (note.as_ptr() as usize)
        .checked_sub(file.data().as_ptr() as usize)
        .and_then(|v| v.checked_add(6))
        .ok_or_else(|| camera_error(format, "Pentax MakerNote offset overflow"))?;
    let ifd = file.parse_ifd_at(format, offset as u64)?;
    if let Some(black) = values(format, &ifd, 0x0200)? {
        if black.len() != 4 {
            return Err(camera_error(
                format,
                "Pentax black level requires four CFA values",
            ));
        }
        let values = black
            .iter()
            .map(|v| {
                if !v.is_finite() || *v < 0.0 || *v > 65535.0 || v.fract() != 0.0 {
                    return Err(camera_error(format, "invalid Pentax black level"));
                }
                #[allow(clippy::cast_possible_truncation)]
                Ok(*v as f32)
            })
            .collect::<Result<Vec<_>, DecodeError>>()?;
        metadata.black_level = rrrah_core::LevelGrid {
            width: 2,
            height: 2,
            components: 1,
            values,
        };
    }
    if let (Some(origin), Some(size)) = (values(format, &ifd, 0x0038)?, values(format, &ifd, 0x0039)?) {
        if origin.len() != 2 || size.len() != 2 {
            return Err(camera_error(format, "invalid Pentax raw image crop count"));
        }
        let coord = |v: f64| -> Result<u32, DecodeError> {
            if !v.is_finite() || v < 0.0 || v > f64::from(u32::MAX) || v.fract() != 0.0 {
                return Err(camera_error(format, "invalid Pentax crop coordinate"));
            }
            #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
            Ok(v as u32)
        };
        metadata.crop_area = Some(rrrah_core::Rect::new(
            coord(origin[0])?,
            coord(origin[1])?,
            coord(size[0])?,
            coord(size[1])?,
        ));
    }
    Ok(())
}

pub(super) fn apply_olympus_levels(
    file: &CameraFile<'_>,
    metadata: &mut super::CameraMetadata,
) -> Result<(), DecodeError> {
    let format = "ORF";
    let note = maker_note(file, format)?;
    if !note.starts_with(b"OLYMPUS\0") {
        return Ok(());
    }
    if note.len() > 16 * 1024 * 1024 {
        return Err(camera_error(format, "Olympus MakerNote exceeds limit"));
    }
    let order = note
        .get(8..10)
        .ok_or_else(|| camera_error(format, "truncated Olympus header"))?;
    let header = match order {
        b"II" => b"II*\0\x0c\0\0\0",
        b"MM" => b"MM\0*\0\0\0\x0c",
        _ => return Err(camera_error(format, "invalid Olympus byte order")),
    };
    let mut bytes = note.to_vec();
    bytes[..8].copy_from_slice(header);
    let tiff = Tiff::parse(&bytes, Limits::default()).map_err(|e| camera_error(format, e.to_string()))?;
    let root = tiff
        .parse_ifd(12)
        .map_err(|e| camera_error(format, e.to_string()))?;
    let pointer = root
        .entry(0x2040)
        .map_err(|e| camera_error(format, e.to_string()))?
        .ok_or_else(|| camera_error(format, "missing Olympus ImageProcessing"))?
        .unsigned_scalar()
        .map_err(|e| camera_error(format, e.to_string()))?;
    let processing = tiff
        .parse_ifd(pointer)
        .map_err(|e| camera_error(format, e.to_string()))?;
    if let Some(valid_bits) = values(format, &processing, 0x0611)?
        && !matches!(valid_bits.first().copied(), Some(12.0))
    {
        return Err(camera_error(
            format,
            "Olympus entropy supports only 12 valid sensor bits",
        ));
    }
    if let Some(black) = values(format, &processing, 0x0600)? {
        if black.len() != 4 {
            return Err(camera_error(format, "Olympus black grid requires four cells"));
        }
        let values = black
            .into_iter()
            .map(|v| {
                if !v.is_finite() || !(0.0..=65535.0).contains(&v) || v.fract() != 0.0 {
                    return Err(camera_error(format, "invalid Olympus black level"));
                }
                #[allow(clippy::cast_possible_truncation)]
                Ok(v as f32)
            })
            .collect::<Result<Vec<_>, DecodeError>>()?;
        metadata.black_level = rrrah_core::LevelGrid {
            width: 2,
            height: 2,
            components: 1,
            values,
        };
    }
    let coordinates = [0x0612, 0x0613, 0x0614, 0x0615].map(|tag| values(format, &processing, tag));
    let mut crop = Vec::new();
    for coordinate in coordinates {
        if let Some(v) = coordinate? {
            let first = *v
                .first()
                .ok_or_else(|| camera_error(format, "empty Olympus crop coordinate"))?;
            if !first.is_finite() || !(0.0..=f64::from(u32::MAX)).contains(&first) || first.fract() != 0.0 {
                return Err(camera_error(format, "invalid Olympus crop coordinate"));
            }
            #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
            crop.push(first as u32);
        }
    }
    if !crop.is_empty() {
        if crop.len() != 4
            || crop[2] == 0
            || crop[3] == 0
            || crop[0].checked_add(crop[2]).is_none_or(|v| v > metadata.width)
            || crop[1].checked_add(crop[3]).is_none_or(|v| v > metadata.height)
        {
            return Err(camera_error(format, "invalid Olympus crop rectangle"));
        }
        metadata.crop_area = Some(rrrah_core::Rect::new(crop[0], crop[1], crop[2], crop[3]));
    }
    Ok(())
}

fn from_note(file: &CameraFile<'_>, format: &'static str, note: &[u8]) -> Result<[f32; 4], DecodeError> {
    // Olympus notes include an embedded preview as well as color metadata.
    if note.len() > 16 * 1024 * 1024 {
        return Err(camera_error(format, "MakerNote exceeds color metadata limit"));
    }
    if format == "ORF" && note.starts_with(b"OLYMPUS\0") {
        let order = note
            .get(8..10)
            .ok_or_else(|| camera_error(format, "truncated Olympus header"))?;
        if order != b"II" && order != b"MM" {
            return Err(camera_error(format, "invalid Olympus byte order"));
        }
        let mut data = note.to_vec();
        data[..8].copy_from_slice(if order == b"II" {
            b"II*\0\x0c\0\0\0"
        } else {
            b"MM\0*\0\0\0\x0c"
        });
        let tiff = Tiff::parse(&data, Limits::default()).map_err(|e| camera_error(format, e.to_string()))?;
        let ifd = tiff
            .parse_ifd(12)
            .map_err(|e| camera_error(format, e.to_string()))?;
        return wb_from_ifd(format, &ifd, |offset| {
            tiff.parse_ifd(offset)
                .map_err(|e| camera_error(format, e.to_string()))
        });
    }
    let prefix = match format {
        "CR2" => 0,
        "PEF" if note.starts_with(b"AOC\0") => 6,
        "ORF" if note.starts_with(b"OLYMP\0") => 8,
        _ => return Err(camera_error(format, "unsupported WB MakerNote signature")),
    };
    let offset = (note.as_ptr() as usize)
        .checked_sub(file.data().as_ptr() as usize)
        .and_then(|v| v.checked_add(prefix))
        .ok_or_else(|| camera_error(format, "MakerNote offset overflow"))?;
    let ifd = file.parse_ifd_at(format, offset as u64)?;
    wb_from_ifd(format, &ifd, |offset| file.parse_ifd_at(format, offset))
}

#[cfg(test)]
mod tests {
    use super::*;
    fn entry(data: &mut [u8], at: usize, tag: u16, kind: u16, count: u32, value: u32) {
        data[at..at + 2].copy_from_slice(&tag.to_le_bytes());
        data[at + 2..at + 4].copy_from_slice(&kind.to_le_bytes());
        data[at + 4..at + 8].copy_from_slice(&count.to_le_bytes());
        data[at + 8..at + 12].copy_from_slice(&value.to_le_bytes());
    }
    fn fixture(format: &str) -> Vec<u8> {
        let mut data = vec![0; 260];
        data[..8].copy_from_slice(b"II*\0\x08\0\0\0");
        data[8..10].copy_from_slice(&1_u16.to_le_bytes());
        entry(&mut data, 10, 0x8769, 4, 1, 26);
        data[26..28].copy_from_slice(&1_u16.to_le_bytes());
        let length = match format {
            "CR2" => 152,
            "PEF" => 32,
            "ORF" => 48,
            _ => unreachable!(),
        };
        entry(&mut data, 28, 0x927c, 7, length, 44);
        match format {
            "CR2" => {
                data[44..46].copy_from_slice(&1_u16.to_le_bytes());
                entry(&mut data, 46, 0x4001, 3, 67, 62);
                data[62..64].copy_from_slice(&2_u16.to_le_bytes());
                for (i, value) in [2000_u16, 1000, 1000, 1500].iter().enumerate() {
                    data[188 + i * 2..190 + i * 2].copy_from_slice(&value.to_le_bytes());
                }
            }
            "PEF" => {
                data[44..50].copy_from_slice(b"AOC\0II");
                data[50..52].copy_from_slice(&1_u16.to_le_bytes());
                entry(&mut data, 52, 0x0201, 3, 4, 68);
                for (i, value) in [2000_u16, 1000, 1000, 1500].iter().enumerate() {
                    data[68 + i * 2..70 + i * 2].copy_from_slice(&value.to_le_bytes());
                }
            }
            "ORF" => {
                data[44..56].copy_from_slice(b"OLYMPUS\0II\x03\0");
                data[56..58].copy_from_slice(&1_u16.to_le_bytes());
                entry(&mut data, 58, 0x2040, 4, 1, 30);
                data[74..76].copy_from_slice(&1_u16.to_le_bytes());
                entry(&mut data, 76, 0x0100, 3, 2, 512 | (384 << 16));
            }
            _ => unreachable!(),
        }
        data
    }
    #[test]
    fn vendor_makernotes_read_as_shot_channels() {
        for format in ["CR2", "PEF", "ORF"] {
            let data = fixture(format);
            let file = CameraFile::parse_tiff(format, &data).unwrap();
            let wb = white_balance(&file, format).unwrap();
            assert_eq!(
                wb.map(f32::to_bits),
                [2.0_f32, 1.0, 1.5, 1.0].map(f32::to_bits),
                "{format}"
            );
        }
    }
    #[test]
    fn canon_unknown_colordata_version_is_rejected() {
        let mut data = fixture("CR2");
        data[62..64].copy_from_slice(&99_u16.to_le_bytes());
        assert!(white_balance(&CameraFile::parse_tiff("CR2", &data).unwrap(), "CR2").is_err());
    }
}
