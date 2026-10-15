//! Qualified Samsung EX1 SRW subset: little-endian sensor words in big-endian TIFF.
use super::{CameraDirectory, CameraFile, CameraMetadata, CameraQuirks, camera_error};
use crate::DecodeError;
pub(crate) struct SrwQuirks;
fn white_balance(file: &CameraFile<'_>) -> Result<[f32; 4], DecodeError> {
    let note = super::makernote_color::maker_note(file, "SRW")?;
    let order = file.byte_order();
    let count = note
        .get(..2)
        .map(|b| usize::from(order.u16(b)))
        .ok_or_else(|| camera_error("SRW", "short MakerNote"))?;
    if count == 0 || count > 1024 || 2 + count * 12 > note.len() {
        return Err(camera_error("SRW", "invalid MakerNote table"));
    }
    let values = |wanted: u16, expected: usize| -> Result<&[u8], DecodeError> {
        let mut found = None;
        for record in note[2..2 + count * 12].chunks_exact(12) {
            if order.u16(&record[..2]) != wanted {
                continue;
            }
            if found.is_some()
                || order.u16(&record[2..4]) != 4
                || order.u32(&record[4..8]) as usize != expected
            {
                return Err(camera_error(
                    "SRW",
                    "invalid Samsung WB field type, count or duplicate",
                ));
            }
            let start =
                usize::try_from(order.u32(&record[8..12])).map_err(|_| DecodeError::DimensionOverflow)?;
            let end = start
                .checked_add(expected * 4)
                .ok_or(DecodeError::DimensionOverflow)?;
            if start < 2 + count * 12 {
                return Err(camera_error("SRW", "WB overlaps MakerNote table"));
            }
            found = Some(
                note.get(start..end)
                    .ok_or_else(|| camera_error("SRW", "WB outside MakerNote"))?,
            );
        }
        found.ok_or_else(|| camera_error("SRW", "missing Samsung WB field"))
    };
    let keys = values(0xa020, 11)?;
    let encoded = values(0xa021, 4)?;
    let mut gains = [0u32; 4];
    for i in 0..4 {
        gains[i] = order
            .u32(&encoded[i * 4..i * 4 + 4])
            .checked_sub(order.u32(&keys[i * 4..i * 4 + 4]))
            .filter(|v| *v != 0)
            .ok_or_else(|| camera_error("SRW", "invalid keyed WB coefficient"))?;
    }
    Ok([
        gains[0] as f32 / gains[1] as f32,
        1.,
        gains[3] as f32 / gains[1] as f32,
        gains[2] as f32 / gains[1] as f32,
    ])
}
fn storage<'a>(file: &CameraFile<'a>, raw: &CameraDirectory<'_>) -> Result<&'a [u8], DecodeError> {
    let scalar = |tag| super::required_scalar("SRW", raw, tag);
    if scalar(256)? != 3688
        || scalar(257)? != 2780
        || scalar(258)? != 14
        || scalar(259)? != 32769
        || scalar(277)? != 3
        || scalar(278)? != 2780
        || scalar(279)? != 3688 * 2780 * 2
    {
        return Err(camera_error("SRW", "unqualified EX1 storage layout"));
    }
    let start = usize::try_from(scalar(273)?).map_err(|_| DecodeError::DimensionOverflow)?;
    let end = start
        .checked_add(3688 * 2780 * 2)
        .ok_or(DecodeError::DimensionOverflow)?;
    let directory_offset = usize::try_from(raw.offset()).map_err(|_| DecodeError::DimensionOverflow)?;
    if start < directory_offset {
        return Err(camera_error("SRW", "sensor overlaps TIFF metadata"));
    }
    file.data()
        .get(start..end)
        .ok_or_else(|| camera_error("SRW", "truncated sensor words"))
}
impl CameraQuirks for SrwQuirks {
    fn format_name(&self) -> &'static str {
        "SRW"
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
                make = super::optional_ascii("SRW", d, 271)?;
            }
            if model.is_none() {
                model = super::optional_ascii("SRW", d, 272)?;
            }
            if let Some(v) = super::optional_scalar("SRW", d, 274)? {
                orientation = super::orientation_from_tag("SRW", v)?;
            }
        }
        let make = make.ok_or_else(|| camera_error("SRW", "missing make"))?;
        let model = model
            .ok_or_else(|| camera_error("SRW", "missing model"))?
            .trim()
            .to_owned();
        if make != "SAMSUNG" || model != "EX1" {
            return Err(camera_error("SRW", "unqualified camera profile"));
        }
        storage(file, raw)?;
        if raw
            .entry("SRW", 33422)?
            .ok_or_else(|| camera_error("SRW", "missing CFA"))?
            .raw_bytes()
            != [0, 1, 1, 2]
        {
            return Err(camera_error("SRW", "unqualified CFA"));
        }
        use rrrah_core::{CfaColor, CfaPattern, LevelGrid, Rect, WhiteLevel};
        Ok(CameraMetadata {
            make,
            model,
            width: 3688,
            height: 2780,
            bits_per_sample: 14,
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
            white_level: WhiteLevel(vec![15872.]),
            white_balance: white_balance(file)?,
            xyz_to_camera: super::color::profile("SAMSUNG", "EX1")
                .ok_or_else(|| camera_error("SRW", "missing calibrated matrix"))?,
            active_area: None,
            crop_area: Some(Rect::new(0, 2, 3682, 2760)),
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
            .try_reserve_exact(3688 * 2780)
            .map_err(|_| camera_error("SRW", "sensor allocation failed"))?;
        for (i, pair) in bytes.chunks_exact(2).enumerate() {
            if i % 3688 == 0 && cancelled() {
                return Err(DecodeError::Cancelled);
            }
            pixels.push(u16::from_le_bytes([pair[0], pair[1]]));
        }
        Ok(pixels)
    }
}
