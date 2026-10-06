//! Epson ERF MakerNote color parameters, with bounded native TIFF access.
use crate::{
    DecodeError,
    camtiff::{CameraFile, camera_error, makernote_color},
};

#[derive(Debug, Clone, PartialEq)]
pub struct EpsonMakerMetadata {
    pub white_balance: [f32; 4],
    /// Full-sensor RGGB spatial black values, including both green channels.
    pub black_grid: [f32; 4],
}

pub fn read_erf_maker_metadata(data: &[u8]) -> Result<EpsonMakerMetadata, DecodeError> {
    let file = CameraFile::parse_tiff("ERF", data)?;
    let note = makernote_color::maker_note(&file, "ERF")?;
    if !note.starts_with(b"EPSON\0\x01\0") {
        return Err(camera_error("ERF", "unsupported Epson MakerNote header"));
    }
    let start = note.as_ptr() as usize - data.as_ptr() as usize;
    let end = start
        .checked_add(note.len())
        .ok_or(DecodeError::DimensionOverflow)?;
    let ifd = file.parse_ifd_at("ERF", (start + 8) as u64)?;
    let bounded = |tag| {
        let entry = ifd
            .entry(tag)
            .map_err(|e| camera_error("ERF", e.to_string()))?
            .ok_or_else(|| camera_error("ERF", format!("missing Epson tag {tag:#x}")))?;
        let bytes = entry.raw_bytes();
        let offset = bytes.as_ptr() as usize - data.as_ptr() as usize;
        if offset < start || offset.checked_add(bytes.len()).is_none_or(|v| v > end) {
            return Err(camera_error("ERF", "color parameters outside MakerNote"));
        }
        Ok(entry)
    };
    let black = bounded(0x0401)?
        .numeric_values()
        .map_err(|e| camera_error("ERF", e.to_string()))?;
    if black.len() != 4 || black.iter().any(|v| !v.is_finite() || *v < 0. || *v >= 4095.) {
        return Err(camera_error("ERF", "invalid Epson black levels"));
    }
    let wb = bounded(0x0e80)?.raw_bytes();
    if wb.len() != 256 {
        return Err(camera_error("ERF", "unsupported Epson WB payload length"));
    }
    let order = file.byte_order();
    let r = f32::from(order.u16(&wb[48..50])) * 567. / 65536.;
    let b = f32::from(order.u16(&wb[50..52])) * 431. / 65536.;
    if r <= 0. || b <= 0. {
        return Err(camera_error("ERF", "zero Epson white balance"));
    }
    Ok(EpsonMakerMetadata {
        white_balance: [r, 1., b, 1.],
        black_grid: std::array::from_fn(|i| black[i] as f32),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    #[ignore = "requires pinned CC0 Epson R-D1 ERF object 2680"]
    fn rd1_color_parameters_match_independent_oracle_and_reject_bad_wb() {
        let data = std::fs::read(std::env::var("RRRAH_ERF_SOURCE").unwrap()).unwrap();
        let actual = read_erf_maker_metadata(&data).unwrap();
        assert_eq!(actual.black_grid, [61., 64., 63., 60.]);
        assert_eq!(actual.white_balance, [1.7563019, 1., 2.1242218, 1.]);
        let mut bad = data.clone();
        bad[1038 + 48..1038 + 50].fill(0);
        assert!(read_erf_maker_metadata(&bad).is_err());
        for end in [0, 8, 708, 716, 1000, 1038, 1293] {
            assert!(read_erf_maker_metadata(&data[..end]).is_err());
        }
    }
}

pub(crate) struct ErfQuirks;
impl super::CameraQuirks for ErfQuirks {
    fn format_name(&self) -> &'static str {
        "ERF"
    }
    fn read_metadata(
        &self,
        file: &CameraFile<'_>,
        raw: &super::CameraDirectory<'_>,
    ) -> Result<super::CameraMetadata, DecodeError> {
        use rrrah_core::{CfaColor, CfaPattern, LevelGrid, Orientation, WhiteLevel};
        let identity = |tag| -> Result<String, DecodeError> {
            for directory in file.directories() {
                if let Some(value) = super::optional_ascii("ERF", directory, tag)? {
                    return Ok(value);
                }
            }
            Err(camera_error("ERF", "missing camera identity"))
        };
        let make = identity(271)?;
        let model = identity(272)?;
        let (width, height, _) = storage(file, raw)?;
        if make != "SEIKO EPSON CORP." || model != "R-D1" || (width, height) != (3040, 2024) {
            return Err(camera_error("ERF", "unqualified camera/sensor profile"));
        }
        let cfa = raw
            .entry("ERF", 33422)?
            .ok_or_else(|| camera_error("ERF", "missing CFA"))?;
        if cfa.raw_bytes() != [0, 1, 1, 2] {
            return Err(camera_error("ERF", "unsupported CFA"));
        }
        let color = read_erf_maker_metadata(file.data())?;
        let mut orientation = Orientation::Normal;
        for directory in file.directories() {
            if let Some(v) = super::optional_scalar("ERF", directory, 274)? {
                orientation = super::orientation_from_tag("ERF", v)?;
                break;
            }
        }
        Ok(super::CameraMetadata {
            make,
            model,
            width,
            height,
            bits_per_sample: 12,
            cfa: CfaPattern {
                width: 2,
                height: 2,
                cells: vec![CfaColor::Red, CfaColor::Green, CfaColor::Green, CfaColor::Blue],
            },
            black_level: LevelGrid {
                width: 2,
                height: 2,
                components: 1,
                values: color.black_grid.to_vec(),
            },
            white_level: WhiteLevel(vec![4095.]),
            white_balance: color.white_balance,
            xyz_to_camera: super::color::profile("SEIKO EPSON CORP.", "R-D1")
                .ok_or_else(|| camera_error("ERF", "missing calibrated matrix"))?,
            active_area: None,
            crop_area: None,
            orientation,
        })
    }
    fn decode_pixels(
        &self,
        file: &CameraFile<'_>,
        raw: &super::CameraDirectory<'_>,
        cancelled: &(dyn Fn() -> bool + Sync),
    ) -> Result<Vec<u16>, DecodeError> {
        let (width, height, bytes) = storage(file, raw)?;
        if cancelled() {
            return Err(DecodeError::Cancelled);
        }
        let count = usize::try_from(u64::from(width) * u64::from(height))
            .map_err(|_| DecodeError::DimensionOverflow)?;
        let mut pixels = Vec::new();
        pixels
            .try_reserve_exact(count)
            .map_err(|_| camera_error("ERF", "sensor allocation failed"))?;
        for (i, block) in bytes.chunks_exact(16).enumerate() {
            if i % (width as usize / 10) == 0 && cancelled() {
                return Err(DecodeError::Cancelled);
            }
            for pair in block[..15].chunks_exact(3) {
                pixels.push((u16::from(pair[0]) << 4) | u16::from(pair[1] >> 4));
                pixels.push((u16::from(pair[1] & 15) << 8) | u16::from(pair[2]));
            }
        }
        Ok(pixels)
    }
}
fn storage<'a>(
    file: &CameraFile<'a>,
    raw: &super::CameraDirectory<'_>,
) -> Result<(u32, u32, &'a [u8]), DecodeError> {
    let scalar = |tag| super::required_scalar("ERF", raw, tag);
    let width = u32::try_from(scalar(256)?).map_err(|_| DecodeError::DimensionOverflow)?;
    let height = u32::try_from(scalar(257)?).map_err(|_| DecodeError::DimensionOverflow)?;
    if width == 0
        || height == 0
        || width % 10 != 0
        || u64::from(width) * u64::from(height) > 100_000_000
        || scalar(258)? != 12
        || scalar(259)? != 32769
        || scalar(262)? != 32803
    {
        return Err(camera_error("ERF", "unsupported packed sensor layout"));
    }
    if super::optional_scalar("ERF", raw, 277)?.is_some_and(|v| v != 1)
        || super::optional_scalar("ERF", raw, 278)?.is_some_and(|v| v < u64::from(height))
    {
        return Err(camera_error("ERF", "unsupported strip geometry"));
    }
    let start = usize::try_from(scalar(273)?).map_err(|_| DecodeError::DimensionOverflow)?;
    let length = usize::try_from(scalar(279)?).map_err(|_| DecodeError::DimensionOverflow)?;
    if length as u64 != u64::from(width) * u64::from(height) / 10 * 16 {
        return Err(camera_error("ERF", "sensor length mismatch"));
    }
    let end = start.checked_add(length).ok_or(DecodeError::DimensionOverflow)?;
    let bytes = file
        .data()
        .get(start..end)
        .ok_or_else(|| camera_error("ERF", "sensor outside source"))?;
    Ok((width, height, bytes))
}
