//! Strict Kodak DCR native WB; no neutral substitute for missing coefficients.
use super::{CameraFile, camera_error};
use crate::DecodeError;
pub(crate) fn white_balance(file: &CameraFile<'_>) -> Result<[f32; 4], DecodeError> {
    let offset = file
        .directories()
        .iter()
        .find_map(|d| super::optional_scalar("DCR", d, 33424).transpose())
        .transpose()?
        .ok_or_else(|| camera_error("DCR", "missing Kodak private IFD"))?;
    let private = file.parse_ifd_at("DCR", offset)?;
    let select = private
        .entry(1020)
        .map_err(|e| camera_error("DCR", e.to_string()))?
        .ok_or_else(|| camera_error("DCR", "missing WB selector"))?
        .unsigned_scalar()
        .map_err(|e| camera_error("DCR", e.to_string()))?;
    let tag = match select {
        0 => 2120,
        1 => 2121,
        2 => 2122,
        3 => 2123,
        4 => 2124,
        5 => 2125,
        _ => return Err(camera_error("DCR", "unqualified WB selector")),
    };
    // Software overrides need their own qualified interpretation.
    if private
        .entry(1021)
        .map_err(|e| camera_error("DCR", e.to_string()))?
        .is_some_and(|e| e.count == 72)
    {
        return Err(camera_error("DCR", "unqualified software WB override"));
    }
    let values = private
        .entry(tag)
        .map_err(|e| camera_error("DCR", e.to_string()))?
        .ok_or_else(|| camera_error("DCR", "missing selected WB"))?
        .numeric_values()
        .map_err(|e| camera_error("DCR", e.to_string()))?;
    if values.len() != 3 || values.iter().any(|v| !v.is_finite() || *v <= 0.0 || *v > 65535.0) {
        return Err(camera_error("DCR", "invalid WB coefficients"));
    }
    let v = values.into_iter().map(|x| x as f32).collect::<Vec<_>>();
    let square = v[1] * v[1];
    let gains = [
        (square / v[0]).trunc(),
        v[1].trunc(),
        (square / v[2]).trunc(),
        v[1].trunc(),
    ];
    super::color::gains("DCR", &gains.map(f64::from), false)
}
#[cfg(test)]
mod tests {
    #[test]
    #[ignore = "requires CC0 Kodak DCS760C source; six independently pinned WB modes"]
    fn all_native_wb_selectors_match_independent_oracle() {
        let source = std::fs::read(std::env::var("RRRAH_KODAK_DCR_SOURCE").unwrap()).unwrap();
        let count = u16::from_be_bytes(source[12..14].try_into().unwrap()) as usize;
        let offset = (0..count)
            .map(|i| 14 + i * 12)
            .find(|p| u16::from_be_bytes(source[*p..*p + 2].try_into().unwrap()) == 1020)
            .unwrap()
            + 8;
        let expected = [
            [1.134984, 1.0, 1.8642173, 1.0],
            [0.8613914, 1.0, 2.392459, 1.0],
            [1.4614949, 1.0, 2.1098528, 1.0],
            [1.1355386, 1.0, 1.8624445, 1.0],
            [1.1257632, 1.0, 2.0122101, 1.0],
            [1.0829688, 1.0, 1.8743849, 1.0],
        ];
        for (mode, expected) in expected.into_iter().enumerate() {
            let mut bytes = source.clone();
            bytes[offset..offset + 2].copy_from_slice(&(mode as u16).to_be_bytes());
            let file = super::super::CameraFile::parse_tiff("DCR", &bytes).unwrap();
            let actual = super::white_balance(&file).unwrap();
            for (a, b) in actual.into_iter().zip(expected) {
                assert!((a - b).abs() < 1e-6, "mode {mode}");
            }
        }
        let mut invalid = source;
        invalid[offset..offset + 2].copy_from_slice(&255u16.to_be_bytes());
        let file = super::super::CameraFile::parse_tiff("DCR", &invalid).unwrap();
        assert!(super::white_balance(&file).is_err());
    }
    #[test]
    #[ignore = "requires CC0 Kodak DCS760C source"]
    fn native_daylight_wb_matches_libraw() {
        let source = std::fs::read(std::env::var("RRRAH_KODAK_DCR_SOURCE").unwrap()).unwrap();
        let file = super::super::CameraFile::parse_tiff("DCR", &source).unwrap();
        assert_eq!(
            super::white_balance(&file).unwrap(),
            [2842.0 / 2504.0, 1.0, 4668.0 / 2504.0, 1.0]
        );
    }
}

pub(crate) fn response_curve(file: &CameraFile<'_>) -> Result<[u16; 4096], DecodeError> {
    let offset = file
        .directories()
        .iter()
        .find_map(|d| super::optional_scalar("DCR", d, 33424).transpose())
        .transpose()?
        .ok_or_else(|| camera_error("DCR", "missing Kodak private IFD"))?;
    let private = file.parse_ifd_at("DCR", offset)?;
    let entry = private
        .entry(2317)
        .map_err(|e| camera_error("DCR", e.to_string()))?
        .ok_or_else(|| camera_error("DCR", "missing response curve"))?;
    if entry.field_type != crate::dng::tiff::FieldType::Short
        || entry.count != 4096
        || entry.raw_bytes().len() != 8192
    {
        return Err(camera_error("DCR", "invalid response curve layout"));
    }
    let bytes = entry.raw_bytes();
    let curve = std::array::from_fn(|i| file.byte_order().u16(&bytes[i * 2..i * 2 + 2]));
    if curve.iter().any(|v| *v > 4095) {
        return Err(camera_error(
            "DCR",
            "response curve outside qualified 12-bit range",
        ));
    }
    Ok(curve)
}
