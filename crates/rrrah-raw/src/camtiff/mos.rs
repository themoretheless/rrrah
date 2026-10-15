//! Native Leaf Aptus 22 MOS, single compression-99 tile.
use super::{CameraDirectory, CameraFile, CameraMetadata, CameraQuirks, camera_error};
use crate::DecodeError;
pub(crate) struct MosQuirks;
fn storage<'a>(file: &CameraFile<'a>, raw: &CameraDirectory<'_>) -> Result<&'a [u8], DecodeError> {
    if file.data().get(..2) != Some(&b"MM"[..]) {
        return Err(camera_error("MOS", "unqualified byte order"));
    }
    for (tag, expected) in [
        (256, 4008),
        (257, 5344),
        (258, 16),
        (259, 99),
        (262, 1),
        (277, 1),
        (284, 2),
        (322, 4008),
        (323, 5344),
    ] {
        if super::required_scalar("MOS", raw, tag)? != expected {
            return Err(camera_error("MOS", "unqualified storage layout"));
        }
    }
    let start = usize::try_from(super::required_scalar("MOS", raw, 324)?)
        .map_err(|_| DecodeError::DimensionOverflow)?;
    let bytes = usize::try_from(super::required_scalar("MOS", raw, 325)?)
        .map_err(|_| DecodeError::DimensionOverflow)?;
    if start < 8 || bytes == 0 {
        return Err(camera_error("MOS", "invalid tile extent"));
    }
    let end = start.checked_add(bytes).ok_or(DecodeError::DimensionOverflow)?;
    file.data()
        .get(start..end)
        .ok_or_else(|| camera_error("MOS", "truncated sensor tile"))
}
impl CameraQuirks for MosQuirks {
    fn format_name(&self) -> &'static str {
        "MOS"
    }
    fn read_metadata(
        &self,
        file: &CameraFile<'_>,
        raw: &CameraDirectory<'_>,
    ) -> Result<CameraMetadata, DecodeError> {
        storage(file, raw)?;
        let entry = raw
            .entry("MOS", 34310)?
            .ok_or_else(|| camera_error("MOS", "missing native packets"))?;
        let leaf = super::leaf_packets::parse(entry.raw_bytes())?;
        let matrix = super::color::profile("Leaf", "Aptus 22")
            .ok_or_else(|| camera_error("MOS", "missing calibrated profile"))?;
        use rrrah_core::{CfaColor, CfaPattern, LevelGrid, Orientation, Rect, WhiteLevel};
        Ok(CameraMetadata {
            make: "Leaf".into(),
            model: "Aptus 22".into(),
            width: 4008,
            height: 5344,
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
                values: vec![0.],
            },
            white_level: WhiteLevel(vec![16383.]),
            white_balance: leaf.white_balance,
            xyz_to_camera: matrix,
            active_area: None,
            crop_area: Some(Rect::new(0, 0, 4008, 5344)),
            orientation: if leaf.rotation_degrees == 90 {
                Orientation::Rotate90
            } else {
                return Err(camera_error("MOS", "unqualified orientation"));
            },
        })
    }
    fn decode_pixels(
        &self,
        file: &CameraFile<'_>,
        raw: &CameraDirectory<'_>,
        cancelled: &(dyn Fn() -> bool + Sync),
    ) -> Result<Vec<u16>, DecodeError> {
        let image = crate::dng::lossless_jpeg::decode_leaf_mos(storage(file, raw)?, cancelled).map_err(
            |e| match e {
                crate::dng::lossless_jpeg::LosslessJpegError::Cancelled { .. } => DecodeError::Cancelled,
                other => camera_error("MOS", other.to_string()),
            },
        )?;
        if (image.width, image.height, image.precision) != (2004, 5344, 16)
            || image.component_ids != [1, 2]
            || image.samples.len() != 4008 * 5344
        {
            return Err(camera_error("MOS", "unqualified JPEG geometry"));
        }
        Ok(image.samples)
    }
}
