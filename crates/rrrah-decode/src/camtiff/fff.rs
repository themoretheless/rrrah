//! Native predictor-8 Hasselblad CFV-50 3FR subset.
use super::{CameraDirectory, CameraFile, CameraMetadata, CameraQuirks, camera_error};
use crate::DecodeError;
pub(crate) struct FffQuirks;
fn storage<'a>(file: &CameraFile<'a>, raw: &CameraDirectory<'_>) -> Result<&'a [u8], DecodeError> {
    if file.data().get(..2) != Some(&b"MM"[..]) {
        return Err(camera_error("FFF", "unqualified sensor byte order"));
    }
    let scalar = |tag| super::required_scalar("FFF", raw, tag);
    if scalar(256)? != 8282
        || scalar(257)? != 6240
        || scalar(258)? != 16
        || scalar(259)? != 7
        || scalar(277)? != 1
        || scalar(278)? != 6240
        || scalar(279)? != 69614078
    {
        return Err(camera_error("FFF", "unqualified CFV-50 storage layout"));
    }
    let start = usize::try_from(scalar(273)?).map_err(|_| DecodeError::DimensionOverflow)?;
    let end = start
        .checked_add(69614078)
        .ok_or(DecodeError::DimensionOverflow)?;
    let directory_offset = usize::try_from(raw.offset()).map_err(|_| DecodeError::DimensionOverflow)?;
    if start < directory_offset {
        return Err(camera_error("FFF", "sensor overlaps TIFF metadata"));
    }
    file.data()
        .get(start..end)
        .ok_or_else(|| camera_error("FFF", "truncated sensor words"))
}
impl CameraQuirks for FffQuirks {
    fn format_name(&self) -> &'static str {
        "FFF"
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
                make = super::optional_ascii("FFF", d, 271)?;
            }
            if model.is_none() {
                model = super::optional_ascii("FFF", d, 272)?;
            }
            if let Some(v) = super::optional_scalar("FFF", d, 274)? {
                orientation = super::orientation_from_tag("FFF", v)?;
            }
        }
        let make = make.ok_or_else(|| camera_error("FFF", "missing make"))?;
        let model = model
            .ok_or_else(|| camera_error("FFF", "missing model"))?
            .trim()
            .to_owned();
        if make != "Hasselblad" || model != "Hasselblad CFV-50" {
            return Err(camera_error("FFF", "unqualified camera profile"));
        }
        storage(file, raw)?;
        let root = &file.directories()[0];
        let neutral = root
            .entry("FFF", 50728)?
            .ok_or_else(|| camera_error("FFF", "missing AsShotNeutral"))?
            .numeric_values()
            .map_err(|e| camera_error("FFF", e.to_string()))?;
        let white_balance = super::color::gains("FFF", &neutral, true)?;
        let matrix = super::color::profile("Hasselblad", "50-Coated-FFF")
            .ok_or_else(|| camera_error("FFF", "missing calibrated profile"))?;
        let numeric = |directory: &CameraDirectory<'_>, tag| {
            directory
                .entry("FFF", tag)?
                .ok_or_else(|| camera_error("FFF", "missing sensor metadata"))?
                .numeric_values()
                .map_err(|e| camera_error("FFF", e.to_string()))
        };
        let black = numeric(raw, 50714)?;
        let white = numeric(raw, 50717)?;
        let origin = numeric(raw, 50719)?;
        let size = numeric(raw, 50720)?;
        if black != [256.0] || white != [65535.0] || origin != [54.0, 16.0] || size != [8176.0, 6132.0] {
            return Err(camera_error("FFF", "unqualified CFV-50 levels or crop"));
        }
        use rrrah_core::{CfaColor, CfaPattern, LevelGrid, Rect, WhiteLevel};
        Ok(CameraMetadata {
            make,
            model,
            width: 8282,
            height: 6240,
            bits_per_sample: 16,
            cfa: CfaPattern {
                width: 2,
                height: 2,
                cells: vec![CfaColor::Red, CfaColor::Green, CfaColor::Green, CfaColor::Blue],
            },
            black_level: LevelGrid {
                width: 1,
                height: 1,
                components: 1,
                values: vec![256.],
            },
            white_level: WhiteLevel(vec![65535.]),
            white_balance,
            xyz_to_camera: matrix,
            active_area: None,
            crop_area: Some(Rect::new(54, 16, 8176, 6132)),
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
        super::hasselblad_entropy::decode(bytes, 8282, 6240, cancelled)
    }
}
