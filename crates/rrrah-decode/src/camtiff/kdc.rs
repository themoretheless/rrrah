//! Strict Kodak P880 packed sensor subset. Private Kodak directories are bounded
//! by the shared TIFF parser; preview dimensions never select the sensor geometry.
use super::{CameraDirectory, CameraFile, CameraMetadata, CameraQuirks, camera_error};
use crate::DecodeError;

pub(crate) struct KdcQuirks;
fn root<'a, 'b>(file: &'b CameraFile<'a>) -> Result<&'b CameraDirectory<'a>, DecodeError> {
    file.directories()
        .iter()
        .find(|d| d.entry("KDC", 0xfe00).ok().flatten().is_some())
        .ok_or_else(|| camera_error("KDC", "missing Kodak private directory pointer"))
}
fn private<'a>(file: &CameraFile<'a>) -> Result<crate::dng::tiff::Ifd<'a>, DecodeError> {
    let offset = super::required_scalar("KDC", root(file)?, 0xfe00)?;
    file.parse_ifd_at("KDC", offset)
}
fn scalar(ifd: &crate::dng::tiff::Ifd<'_>, tag: u16) -> Result<u64, DecodeError> {
    ifd.entry(tag)
        .map_err(|e| camera_error("KDC", e.to_string()))?
        .ok_or_else(|| camera_error("KDC", format!("missing Kodak tag {tag:#x}")))?
        .unsigned_scalar()
        .map_err(|e| camera_error("KDC", e.to_string()))
}
fn storage<'a>(file: &CameraFile<'a>, raw: &CameraDirectory<'_>) -> Result<&'a [u8], DecodeError> {
    let private = private(file)?;
    if scalar(&private, 0xfa13)? != 3280
        || scalar(&private, 0xfa14)? != 2454
        || super::required_scalar("KDC", raw, 259)? != 1
    {
        return Err(camera_error(
            "KDC",
            "unqualified P880 sensor geometry or compression",
        ));
    }
    let entry = raw
        .entry("KDC", 0xfd04)?
        .ok_or_else(|| camera_error("KDC", "missing packed storage descriptor"))?;
    let bytes = entry.raw_bytes();
    if entry.field_type != crate::dng::tiff::FieldType::Long || bytes.len() < 52 || bytes.len() % 4 != 0 {
        return Err(camera_error("KDC", "invalid packed storage descriptor"));
    }
    let order = file.byte_order();
    let start = u64::from(order.u32(&bytes[16..20]))
        .checked_add(u64::from(order.u32(&bytes[48..52])))
        .ok_or(DecodeError::DimensionOverflow)?;
    let end = start
        .checked_add(3280 * 2454 * 3 / 2)
        .ok_or(DecodeError::DimensionOverflow)?;
    if start < raw.offset() || end != super::required_scalar("KDC", root(file)?, 513)? {
        return Err(camera_error(
            "KDC",
            "sensor range conflicts with preview boundary",
        ));
    }
    file.data()
        .get(
            usize::try_from(start).map_err(|_| DecodeError::DimensionOverflow)?
                ..usize::try_from(end).map_err(|_| DecodeError::DimensionOverflow)?,
        )
        .ok_or_else(|| camera_error("KDC", "truncated packed sensor"))
}
impl CameraQuirks for KdcQuirks {
    fn format_name(&self) -> &'static str {
        "KDC"
    }
    fn select_raw_ifd<'a>(&self, file: &'a CameraFile<'a>) -> Result<&'a CameraDirectory<'a>, DecodeError> {
        let mut candidates = file
            .directories()
            .iter()
            .filter(|d| d.entry("KDC", 0xfd04).ok().flatten().is_some());
        let raw = candidates
            .next()
            .ok_or_else(|| camera_error("KDC", "missing Kodak sensor directory"))?;
        if candidates.next().is_some() {
            return Err(camera_error("KDC", "ambiguous Kodak sensor directory"));
        }
        Ok(raw)
    }
    fn read_metadata(
        &self,
        file: &CameraFile<'_>,
        raw: &CameraDirectory<'_>,
    ) -> Result<CameraMetadata, DecodeError> {
        let root = root(file)?;
        let make =
            super::optional_ascii("KDC", root, 271)?.ok_or_else(|| camera_error("KDC", "missing make"))?;
        let model =
            super::optional_ascii("KDC", root, 272)?.ok_or_else(|| camera_error("KDC", "missing model"))?;
        if make != "EASTMAN KODAK COMPANY" || model != "KODAK P880 ZOOM DIGITAL CAMERA" {
            return Err(camera_error("KDC", "unqualified camera profile"));
        }
        storage(file, raw)?;
        if raw
            .entry("KDC", 0xfd09)?
            .ok_or_else(|| camera_error("KDC", "missing native CFA"))?
            .raw_bytes()
            != [2, 1, 1, 0]
        {
            return Err(camera_error("KDC", "unqualified CFA"));
        }
        let private = private(file)?;
        let wb_tag = match scalar(&private, 0xfa0d)? {
            0 => 0xfa25,
            1 => 0xfa28,
            2 => 0xfa27,
            3 => 0xfa29,
            6 => 0xfa2a,
            _ => return Err(camera_error("KDC", "unsupported white balance selection")),
        };
        let wb_entry = private
            .entry(wb_tag)
            .map_err(|e| camera_error("KDC", e.to_string()))?
            .ok_or_else(|| camera_error("KDC", "missing selected white balance"))?;
        if wb_entry.field_type != crate::dng::tiff::FieldType::Long {
            return Err(camera_error("KDC", "invalid white balance coefficient type"));
        }
        let wb = wb_entry
            .unsigned_values()
            .map_err(|e| camera_error("KDC", e.to_string()))?;
        if wb.len() != 3 || wb.iter().any(|v| *v == 0 || *v > u64::from(u32::MAX)) {
            return Err(camera_error("KDC", "invalid selected white balance"));
        }
        use rrrah_core::{CfaColor, CfaPattern, LevelGrid, WhiteLevel};
        Ok(CameraMetadata {
            make,
            model,
            width: 3280,
            height: 2454,
            bits_per_sample: 12,
            cfa: CfaPattern {
                width: 2,
                height: 2,
                cells: vec![CfaColor::Blue, CfaColor::Green, CfaColor::Green, CfaColor::Red],
            },
            black_level: LevelGrid {
                width: 1,
                height: 1,
                components: 1,
                values: vec![0.],
            },
            white_level: WhiteLevel(vec![3963.]),
            white_balance: [wb[0] as f32 / wb[1] as f32, 1., wb[2] as f32 / wb[1] as f32, 1.],
            xyz_to_camera: super::color::profile("EASTMAN KODAK COMPANY", "KODAK P880 ZOOM DIGITAL CAMERA")
                .ok_or_else(|| camera_error("KDC", "missing calibrated matrix"))?,
            active_area: None,
            crop_area: None,
            orientation: super::orientation_from_tag("KDC", super::required_scalar("KDC", root, 274)?)?,
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
            .try_reserve_exact(3280 * 2454)
            .map_err(|_| camera_error("KDC", "sensor allocation failed"))?;
        for (i, pair) in bytes.chunks_exact(3).enumerate() {
            if i % (3280 / 2) == 0 && cancelled() {
                return Err(DecodeError::Cancelled);
            }
            pixels.push((u16::from(pair[0]) << 4) | u16::from(pair[1] >> 4));
            pixels.push((u16::from(pair[1] & 15) << 8) | u16::from(pair[2]));
        }
        Ok(pixels)
    }
}
