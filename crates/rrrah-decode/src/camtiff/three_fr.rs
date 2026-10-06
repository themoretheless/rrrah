//! Native uncompressed Hasselblad X1D 3FR subset.
use super::{CameraDirectory, CameraFile, CameraMetadata, CameraQuirks, camera_error};
use crate::DecodeError;
pub(crate) struct ThreeFrQuirks;
fn storage<'a>(file: &CameraFile<'a>, raw: &CameraDirectory<'_>) -> Result<&'a [u8], DecodeError> {
    if file.data().get(..2) != Some(&b"II"[..]) {
        return Err(camera_error("3FR", "unqualified sensor byte order"));
    }
    let scalar = |tag| super::required_scalar("3FR", raw, tag);
    if scalar(256)? != 8384
        || scalar(257)? != 6304
        || scalar(258)? != 16
        || scalar(259)? != 1
        || scalar(277)? != 1
        || scalar(278)? != 6304
        || scalar(279)? != 8384 * 6304 * 2
    {
        return Err(camera_error("3FR", "unqualified X1D storage layout"));
    }
    let start = usize::try_from(scalar(273)?).map_err(|_| DecodeError::DimensionOverflow)?;
    let end = start
        .checked_add(8384 * 6304 * 2)
        .ok_or(DecodeError::DimensionOverflow)?;
    let directory_offset = usize::try_from(raw.offset()).map_err(|_| DecodeError::DimensionOverflow)?;
    if start < directory_offset {
        return Err(camera_error("3FR", "sensor overlaps TIFF metadata"));
    }
    file.data()
        .get(start..end)
        .ok_or_else(|| camera_error("3FR", "truncated sensor words"))
}
impl CameraQuirks for ThreeFrQuirks {
    fn format_name(&self) -> &'static str {
        "3FR"
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
                make = super::optional_ascii("3FR", d, 271)?;
            }
            if model.is_none() {
                model = super::optional_ascii("3FR", d, 272)?;
            }
            if let Some(v) = super::optional_scalar("3FR", d, 274)? {
                orientation = super::orientation_from_tag("3FR", v)?;
            }
        }
        let make = make.ok_or_else(|| camera_error("3FR", "missing make"))?;
        let model = model
            .ok_or_else(|| camera_error("3FR", "missing model"))?
            .trim()
            .to_owned();
        if make != "Hasselblad" || model != "Hasselblad X1D" {
            return Err(camera_error("3FR", "unqualified camera profile"));
        }
        storage(file, raw)?;
        let root = &file.directories()[0];
        let color = super::hasselblad::read_file_color(root)?;
        let numeric = |directory: &CameraDirectory<'_>, tag| {
            directory
                .entry("3FR", tag)?
                .ok_or_else(|| camera_error("3FR", "missing sensor metadata"))?
                .numeric_values()
                .map_err(|e| camera_error("3FR", e.to_string()))
        };
        let black = numeric(raw, 50714)?;
        let white = numeric(raw, 50717)?;
        let origin = numeric(raw, 50719)?;
        let size = numeric(raw, 50720)?;
        if black != [256.0] || white != [65535.0] || origin != [50.0, 100.0] || size != [8272.0, 6200.0] {
            return Err(camera_error("3FR", "unqualified X1D levels or crop"));
        }
        use rrrah_core::{CfaColor, CfaPattern, LevelGrid, Rect, WhiteLevel};
        Ok(CameraMetadata {
            make,
            model,
            width: 8384,
            height: 6304,
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
            white_balance: color.white_balance,
            xyz_to_camera: color.xyz_to_camera,
            active_area: None,
            crop_area: Some(Rect::new(50, 100, 8272, 6200)),
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
        if cancelled() {
            return Err(DecodeError::Cancelled);
        }
        let mut pixels = Vec::new();
        pixels
            .try_reserve_exact(8384 * 6304)
            .map_err(|_| camera_error("3FR", "sensor allocation failed"))?;
        for (i, pair) in bytes.chunks_exact(2).enumerate() {
            if i % 8384 == 0 && cancelled() {
                return Err(DecodeError::Cancelled);
            }
            pixels.push(u16::from_le_bytes([pair[0], pair[1]]));
        }
        Ok(pixels)
    }
}
