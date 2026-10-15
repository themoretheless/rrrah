//! Native Kodak DCS760C DCR compression-65000 subset.
use super::{CameraDirectory, CameraFile, CameraMetadata, CameraQuirks, camera_error};
use crate::DecodeError;
pub(crate) struct DcrQuirks;
fn storage<'a>(file: &CameraFile<'a>, raw: &CameraDirectory<'_>) -> Result<&'a [u8], DecodeError> {
    if file.data().get(..2) != Some(&b"MM"[..]) {
        return Err(camera_error("DCR", "unqualified sensor byte order"));
    }
    let scalar = |tag| super::required_scalar("DCR", raw, tag);
    if scalar(256)? != 3040
        || scalar(257)? != 2016
        || scalar(258)? != 12
        || scalar(259)? != 65000
        || scalar(277)? != 1
        || scalar(278)? != 2016
        || scalar(279)? != 5703212
    {
        return Err(camera_error("DCR", "unqualified DCS760C storage layout"));
    }
    let start = usize::try_from(scalar(273)?).map_err(|_| DecodeError::DimensionOverflow)?;
    let end = start.checked_add(5703212).ok_or(DecodeError::DimensionOverflow)?;
    let directory_offset = usize::try_from(raw.offset()).map_err(|_| DecodeError::DimensionOverflow)?;
    if start < directory_offset {
        return Err(camera_error("DCR", "sensor overlaps TIFF metadata"));
    }
    file.data()
        .get(start..end)
        .ok_or_else(|| camera_error("DCR", "truncated sensor words"))
}
impl CameraQuirks for DcrQuirks {
    fn format_name(&self) -> &'static str {
        "DCR"
    }
    fn read_metadata(
        &self,
        file: &CameraFile<'_>,
        raw: &CameraDirectory<'_>,
    ) -> Result<CameraMetadata, DecodeError> {
        let mut make = None;
        let mut model = None;
        let mut orientation = rrrah_core::Orientation::Normal;
        for d in file.directories() {
            if make.is_none() {
                make = super::optional_ascii("DCR", d, 271)?;
            }
            if model.is_none() {
                model = super::optional_ascii("DCR", d, 272)?;
            }
            if let Some(v) = super::optional_scalar("DCR", d, 274)? {
                orientation = if v == 9 {
                    rrrah_core::Orientation::Normal
                } else {
                    super::orientation_from_tag("DCR", v)?
                };
            }
        }
        let make = make.ok_or_else(|| camera_error("DCR", "missing make"))?;
        let model = model
            .ok_or_else(|| camera_error("DCR", "missing model"))?
            .trim()
            .to_owned();
        if make != "Kodak" || model != "DCS760C" {
            return Err(camera_error("DCR", "unqualified camera profile"));
        }
        storage(file, raw)?;
        super::kodak_dcr_color::response_curve(file)?;
        let white_balance = super::kodak_dcr_color::white_balance(file)?;
        let matrix = super::color::profile(&make, &model)
            .ok_or_else(|| camera_error("DCR", "missing calibrated profile"))?;
        if raw
            .entry("DCR", 33422)?
            .ok_or_else(|| camera_error("DCR", "missing CFA"))?
            .raw_bytes()
            != [1, 0, 2, 1]
        {
            return Err(camera_error("DCR", "unqualified CFA"));
        }
        use rrrah_core::{CfaColor, CfaPattern, LevelGrid, Rect, WhiteLevel};
        Ok(CameraMetadata {
            make,
            model,
            width: 3040,
            height: 2016,
            bits_per_sample: 12,
            cfa: CfaPattern {
                width: 2,
                height: 2,
                cells: vec![CfaColor::Green, CfaColor::Red, CfaColor::Blue, CfaColor::Green],
            },
            black_level: LevelGrid {
                width: 1,
                height: 1,
                components: 1,
                values: vec![0.],
            },
            white_level: WhiteLevel(vec![4095.]),
            white_balance,
            xyz_to_camera: matrix,
            active_area: None,
            crop_area: Some(Rect::new(0, 0, 3040, 2016)),
            orientation,
        })
    }
    fn decode_pixels(
        &self,
        file: &CameraFile<'_>,
        raw: &CameraDirectory<'_>,
        cancelled: &(dyn Fn() -> bool + Sync),
    ) -> Result<Vec<u16>, DecodeError> {
        let bytes = storage(file, raw)?;
        let curve = super::kodak_dcr_color::response_curve(file)?;
        let mut pixels = super::kodak_65000::decode(bytes, 3040, 2016, cancelled)?;
        for row in pixels.chunks_mut(3040) {
            if cancelled() {
                return Err(DecodeError::Cancelled);
            }
            for sample in row {
                *sample = curve[usize::from(*sample)];
            }
        }
        Ok(pixels)
    }
}
