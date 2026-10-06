//! Camera calibration and as-shot WB. Missing calibration is an error.
use super::{CameraDirectory, CameraFile, CameraMetadata, camera_error};
use crate::DecodeError;
use rrrah_core::{DngColorMatrix, camera_to_linear_srgb, select_dng_xyz_to_camera};

const PROFILES: &str = include_str!("../../data/camera-matrices.tsv");

pub(crate) fn profile(make: &str, model: &str) -> Option<[[f32; 3]; 4]> {
    for line in PROFILES.lines().filter(|line| !line.starts_with('#')) {
        let mut fields = line.split('\t');
        if fields.next()? != make.trim() || fields.next()? != model.trim() {
            continue;
        }
        let mut matrix = [[0.0; 3]; 4];
        for value in matrix[..3].iter_mut().flatten() {
            *value = fields.next()?.parse::<f32>().ok()? / 10_000.0;
        }
        return Some(matrix);
    }
    None
}

pub(super) fn gains(format: &'static str, values: &[f64], reciprocal: bool) -> Result<[f32; 4], DecodeError> {
    if values.len() != 3 && values.len() != 4 {
        return Err(camera_error(format, "white balance must contain 3 or 4 channels"));
    }
    if values.iter().any(|x| !x.is_finite() || *x <= 0.0) {
        return Err(camera_error(
            format,
            "white balance contains a non-positive or non-finite channel",
        ));
    }
    let mut result = [1.0; 4];
    for (i, value) in values.iter().enumerate() {
        let gain = if reciprocal {
            values[1] / value
        } else {
            value / values[1]
        };
        #[allow(clippy::cast_possible_truncation)]
        {
            result[i] = gain as f32;
        }
    }
    if result.iter().any(|x| !x.is_finite() || *x <= 0.0) {
        return Err(camera_error(format, "white balance exceeds representable range"));
    }
    Ok(result)
}

pub(super) fn resolve(
    format: &'static str,
    file: &CameraFile<'_>,
    raw: &CameraDirectory<'_>,
    metadata: &mut CameraMetadata,
) -> Result<(), DecodeError> {
    if format == "SRF" {
        if metadata.white_balance.iter().any(|v| !v.is_finite() || *v <= 0.) {
            return Err(camera_error(format, "invalid four-plane WB"));
        }
        rrrah_core::camera4_to_linear_srgb_precise(metadata.xyz_to_camera.map(|r| r.map(f64::from)))
            .map_err(|e| camera_error(format, e.to_string()))?;
        return Ok(());
    }
    if matches!(format, "3FR" | "FFF" | "DCR" | "DCS" | "MOS" | "IIQ") {
        gains(format, &metadata.white_balance.map(f64::from), false)?;
        if camera_to_linear_srgb(metadata.xyz_to_camera).is_none() {
            return Err(camera_error(format, "invalid embedded matrix"));
        }
        return Ok(());
    }
    let candidate = |matrix_tag, illuminant_tag| -> Result<Option<DngColorMatrix>, DecodeError> {
        let Some(entry) = raw.entry(format, matrix_tag)? else {
            return Ok(None);
        };
        let values = entry
            .numeric_values()
            .map_err(|e| camera_error(format, e.to_string()))?;
        if values.len() != 9 || values.iter().any(|v| !v.is_finite()) {
            return Err(camera_error(
                format,
                "camera color matrix must contain nine finite coefficients",
            ));
        }
        let illuminant = super::optional_scalar(format, raw, illuminant_tag)?
            .map(u16::try_from)
            .transpose()
            .map_err(|_| camera_error(format, "invalid calibration illuminant"))?;
        Ok(Some(DngColorMatrix {
            xyz_to_camera: [
                [values[0], values[1], values[2]],
                [values[3], values[4], values[5]],
                [values[6], values[7], values[8]],
            ],
            illuminant,
        }))
    };
    if let Some(matrix) = select_dng_xyz_to_camera(candidate(50721, 50778)?, candidate(50722, 50779)?) {
        for (dst, src) in metadata.xyz_to_camera[..3].iter_mut().zip(matrix) {
            #[allow(clippy::cast_possible_truncation)]
            {
                *dst = src.map(|v| v as f32);
            }
        }
        metadata.xyz_to_camera[3] = [0.0; 3];
    } else {
        metadata.xyz_to_camera = profile(&metadata.make, &metadata.model).ok_or_else(|| {
            camera_error(
                format,
                format!(
                    "no calibrated color profile for {} {}",
                    metadata.make, metadata.model
                ),
            )
        })?;
    }
    if camera_to_linear_srgb(metadata.xyz_to_camera).is_none() {
        return Err(camera_error(format, "invalid camera color matrix"));
    }
    if let Some(entry) = raw.entry(format, 50728)? {
        let values = entry
            .numeric_values()
            .map_err(|e| camera_error(format, e.to_string()))?;
        metadata.white_balance = gains(format, &values, true)?;
    } else {
        match format {
            "ARW" => metadata.white_balance = sony_wb(file)?,
            "NEF" => metadata.white_balance = nikon_wb(file)?,
            "CR2" | "ORF" | "PEF" => {
                metadata.white_balance = super::makernote_color::white_balance(file, format)?;
            }
            "ERF" | "KDC" | "SRW" => {} // Quirks resolve strict native vendor WB before this step.
            "RW2" => metadata.white_balance = panasonic_wb(file)?,
            "RAF" if super::raf::has_white_balance(file, raw)? => {}
            _ => {
                return Err(camera_error(
                    format,
                    "as-shot white balance metadata is unavailable; refusing neutral placeholder",
                ));
            }
        }
    }
    if format == "NEF" {
        apply_nikon_levels(file, metadata)?;
    }
    if format == "ORF" {
        super::makernote_color::apply_olympus_levels(file, metadata)?;
    }
    if format == "PEF" {
        super::makernote_color::apply_pentax_levels(file, metadata)?;
    }
    if format == "CR2" {
        super::makernote_color::apply_canon_levels(file, metadata)?;
    }
    gains(format, &metadata.white_balance.map(f64::from), false)?;
    Ok(())
}

fn apply_nikon_levels(file: &CameraFile<'_>, metadata: &mut CameraMetadata) -> Result<(), DecodeError> {
    let format = "NEF";
    if let Some(entry) = nikon_ifd(file)?
        .entry(0x003d)
        .map_err(|e| camera_error(format, e.to_string()))?
    {
        let values = entry
            .unsigned_values()
            .map_err(|e| camera_error(format, e.to_string()))?;
        if values.len() != 4 {
            return Err(camera_error(format, "Nikon black level must have four channels"));
        }
        let mut green = 0;
        let cells = metadata
            .cfa
            .cells
            .iter()
            .map(|color| {
                let index = match color {
                    rrrah_core::CfaColor::Red => 0,
                    rrrah_core::CfaColor::Blue => 2,
                    rrrah_core::CfaColor::Green => {
                        green += 1;
                        if green == 1 { 1 } else { 3 }
                    }
                    _ => unreachable!("validated RGB CFA"),
                };
                let value = u16::try_from(values[index])
                    .map_err(|_| camera_error(format, "Nikon black level exceeds u16"))?;
                Ok(f32::from(value))
            })
            .collect::<Result<Vec<_>, DecodeError>>()?;
        metadata.black_level = rrrah_core::LevelGrid {
            width: 2,
            height: 2,
            components: 1,
            values: cells,
        };
    }
    Ok(())
}

fn panasonic_wb(file: &CameraFile<'_>) -> Result<[f32; 4], DecodeError> {
    for directory in file.directories() {
        let red = super::optional_scalar("RW2", directory, 0x24)?;
        let green = super::optional_scalar("RW2", directory, 0x25)?;
        let blue = super::optional_scalar("RW2", directory, 0x26)?;
        if red.is_none() && green.is_none() && blue.is_none() {
            continue;
        }
        let (Some(r), Some(g), Some(b)) = (red, green, blue) else {
            return Err(camera_error("RW2", "partial Panasonic white balance"));
        };
        let channel = |value| {
            u16::try_from(value)
                .map(f64::from)
                .map_err(|_| camera_error("RW2", "Panasonic WB channel exceeds u16"))
        };
        return gains("RW2", &[channel(r)?, channel(g)?, channel(b)?], false);
    }
    Err(camera_error("RW2", "missing Panasonic white balance"))
}

fn nikon_ifd<'a>(file: &CameraFile<'a>) -> Result<crate::dng::tiff::Ifd<'a>, DecodeError> {
    let format = "NEF";
    let exif_offset = file
        .directories()
        .iter()
        .find_map(|d| super::optional_scalar(format, d, 0x8769).transpose())
        .transpose()?
        .ok_or_else(|| camera_error(format, "missing Nikon ExifIFD"))?;
    let exif = file.parse_ifd_at(format, exif_offset)?;
    let note = exif
        .entry(0x927c)
        .map_err(|e| camera_error(format, e.to_string()))?
        .ok_or_else(|| camera_error(format, "missing Nikon MakerNote"))?
        .raw_bytes();
    if !note.starts_with(b"Nikon\0") {
        return Err(camera_error(format, "unsupported Nikon WB makernote layout"));
    }
    let data = note
        .get(10..)
        .ok_or_else(|| camera_error(format, "truncated Nikon MakerNote"))?;
    let tiff = crate::dng::tiff::Tiff::parse(data, crate::dng::tiff::Limits::default())
        .map_err(|e| camera_error(format, e.to_string()))?;
    tiff.parse_ifd(tiff.first_ifd_offset())
        .map_err(|e| camera_error(format, e.to_string()))
}

fn nikon_wb(file: &CameraFile<'_>) -> Result<[f32; 4], DecodeError> {
    let format = "NEF";
    let ifd = nikon_ifd(file)?;
    let entry = ifd
        .entry(0x000c)
        .map_err(|e| camera_error(format, e.to_string()))?
        .ok_or_else(|| camera_error(format, "Nikon encrypted WB is not supported; no WB_RBLevels tag"))?;
    let values = entry
        .numeric_values()
        .map_err(|e| camera_error(format, e.to_string()))?;
    if values.len() != 4 {
        return Err(camera_error(
            format,
            "Nikon WB_RBLevels requires four rational values",
        ));
    }
    gains(format, &[values[0], 1.0, values[1], 1.0], false)
}

fn sony_private(file: &CameraFile<'_>) -> Result<(Vec<u8>, usize), DecodeError> {
    fn err(message: impl Into<String>) -> DecodeError {
        camera_error("ARW", message)
    }
    let private = file
        .directories()
        .iter()
        .find_map(|d| d.entry("ARW", 0xc634).ok().flatten())
        .ok_or_else(|| err("missing Sony DNGPrivateData"))?;
    let bytes = private.raw_bytes();
    let offset = bytes
        .get(..4)
        .ok_or_else(|| err("truncated Sony private pointer"))?;
    let ifd = file.parse_ifd_at("ARW", u64::from(file.byte_order().u32(offset)))?;
    let scalar = |tag| -> Result<usize, DecodeError> {
        let value = ifd
            .entry(tag)
            .map_err(|e| err(e.to_string()))?
            .ok_or_else(|| err(format!("missing Sony tag {tag:#x}")))?
            .unsigned_scalar()
            .map_err(|e| err(e.to_string()))?;
        usize::try_from(value).map_err(|_| err("Sony offset overflow".to_owned()))
    };
    let start = scalar(0x7200)?;
    let declared_length = scalar(0x7201)?;
    let length = declared_length & !3;
    // Bound the metadata buffer independently of the RAW image allocation.
    if length == 0 || declared_length > 1024 * 1024 || start > 4 * 1024 * 1024 {
        return Err(err("invalid Sony metadata length".to_owned()));
    }
    let key_bytes = ifd
        .entry(0x7221)
        .map_err(|e| err(e.to_string()))?
        .ok_or_else(|| err("missing Sony key".to_owned()))?
        .raw_bytes();
    let key_bytes: [u8; 4] = key_bytes
        .try_into()
        .map_err(|_| err("invalid Sony key length".to_owned()))?;
    let end = start
        .checked_add(length)
        .ok_or_else(|| err("Sony metadata range overflow".to_owned()))?;
    let encrypted = file
        .data()
        .get(start..end)
        .ok_or_else(|| err("Sony metadata outside file".to_owned()))?;
    // TIFF offsets are absolute; cap both the prefix and decrypted payload.
    let mut decrypted = vec![0; end];
    let header = file
        .data()
        .get(..8)
        .ok_or_else(|| err("truncated Sony TIFF header"))?;
    decrypted[..8].copy_from_slice(header);
    decrypt(
        encrypted,
        &mut decrypted[start..end],
        u32::from_le_bytes(key_bytes),
    );
    Ok((decrypted, start))
}

fn sony_wb(file: &CameraFile<'_>) -> Result<[f32; 4], DecodeError> {
    fn err(message: impl Into<String>) -> DecodeError {
        camera_error("ARW", message)
    }
    let (decrypted, start) = sony_private(file)?;
    let parsed = crate::dng::tiff::Tiff::parse(&decrypted, crate::dng::tiff::Limits::default())
        .map_err(|e| err(e.to_string()))?;
    let wb_ifd = parsed.parse_ifd(start as u64).map_err(|e| err(e.to_string()))?;
    for (tag, order) in [(0x7303, [1, 0, 2, 3]), (0x7313, [0, 1, 3, 2])] {
        if let Some(entry) = wb_ifd.entry(tag).map_err(|e| err(e.to_string()))? {
            let region_start = decrypted.as_ptr() as usize + start;
            let value_start = entry.raw_bytes().as_ptr() as usize;
            if value_start < region_start {
                return Err(err("Sony WB points outside decrypted metadata"));
            }
            let values = entry.numeric_values().map_err(|e| err(e.to_string()))?;
            if values.len() != 4 {
                return Err(err("Sony WB must have four channels"));
            }
            return gains("ARW", &order.map(|i| values[i]), false);
        }
    }
    Err(err("missing Sony white balance levels"))
}

fn decrypt(input: &[u8], output: &mut [u8], mut key: u32) {
    let mut pad = [0_u32; 128];
    for item in &mut pad[..4] {
        key = key.wrapping_mul(48_828_125).wrapping_add(1);
        *item = key;
    }
    pad[3] = (pad[3] << 1) | ((pad[0] ^ pad[2]) >> 31);
    for p in 4..127 {
        pad[p] = ((pad[p - 4] ^ pad[p - 2]) << 1) | ((pad[p - 3] ^ pad[p - 1]) >> 31);
    }
    for item in &mut pad[..127] {
        *item = item.swap_bytes();
    }
    for (i, (src, dst)) in input
        .as_chunks::<4>()
        .0
        .iter()
        .zip(output.as_chunks_mut::<4>().0.iter_mut())
        .enumerate()
    {
        let p = i + 127;
        pad[p & 127] = pad[(p + 1) & 127] ^ pad[(p + 65) & 127];
        let word = u32::from_le_bytes(*src) ^ pad[p & 127];
        dst.copy_from_slice(&word.to_le_bytes());
    }
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
    fn sony_fixture() -> Vec<u8> {
        let mut data = vec![0; 160];
        data[..8].copy_from_slice(b"II*\0\x08\0\0\0");
        data[8..10].copy_from_slice(&1_u16.to_le_bytes());
        entry(&mut data, 10, 0xc634, 1, 4, 32);
        data[32..34].copy_from_slice(&3_u16.to_le_bytes());
        entry(&mut data, 34, 0x7200, 4, 1, 128);
        entry(&mut data, 46, 0x7201, 4, 1, 32);
        entry(&mut data, 58, 0x7221, 1, 4, 0x1234_5678);
        // Golden XOR stream for key 0x12345678, checked against the documented Sony recurrence.
        let stream = [
            20, 197, 233, 187, 162, 50, 220, 125, 145, 57, 137, 78, 160, 242, 200, 175, 11, 248, 193, 234, 5,
            128, 41, 165, 53, 130, 145, 73, 74, 229, 194, 20,
        ];
        data[128..130].copy_from_slice(&1_u16.to_le_bytes());
        entry(&mut data, 130, 0x7313, 3, 4, 148);
        for (i, value) in [2000_u16, 1000, 1000, 1500].iter().enumerate() {
            data[148 + i * 2..150 + i * 2].copy_from_slice(&value.to_le_bytes());
        }
        for (value, mask) in data[128..160].iter_mut().zip(stream) {
            *value ^= mask;
        }
        data
    }
    #[test]
    fn panasonic_white_balance_reads_actual_directory_values() {
        let mut data = vec![0; 50];
        data[..8].copy_from_slice(b"IIU\0\x08\0\0\0");
        data[8..10].copy_from_slice(&3_u16.to_le_bytes());
        entry(&mut data, 10, 0x24, 3, 1, 512);
        entry(&mut data, 22, 0x25, 3, 1, 256);
        entry(&mut data, 34, 0x26, 3, 1, 384);
        let wb = panasonic_wb(&CameraFile::parse_tiff("RW2", &data).unwrap()).unwrap();
        assert_eq!(wb.map(f32::to_bits), [2.0_f32, 1.0, 1.5, 1.0].map(f32::to_bits));
        entry(&mut data, 22, 0x25, 3, 1, 0);
        assert!(panasonic_wb(&CameraFile::parse_tiff("RW2", &data).unwrap()).is_err());
    }
    #[test]
    fn sony_encrypted_metadata_has_correct_channel_order() {
        let data = sony_fixture();
        let file = CameraFile::parse_tiff("ARW", &data).unwrap();
        let wb = sony_wb(&file).unwrap();
        assert_eq!(wb.map(f32::to_bits), [2.0_f32, 1.0, 1.5, 1.0].map(f32::to_bits));
    }
    #[test]
    fn sony_rejects_invalid_metadata_ranges() {
        for length in [0, 1, 2, 3, 1024 * 1024 + 4, u32::MAX] {
            let mut data = sony_fixture();
            entry(&mut data, 46, 0x7201, 4, 1, length);
            assert!(sony_wb(&CameraFile::parse_tiff("ARW", &data).unwrap()).is_err());
        }
    }
    #[test]
    fn nikon_rational_white_balance_uses_green_relative_rb() {
        let mut data = vec![0; 112];
        data[..8].copy_from_slice(b"II*\0\x08\0\0\0");
        data[8..10].copy_from_slice(&1_u16.to_le_bytes());
        entry(&mut data, 10, 0x8769, 4, 1, 26);
        data[26..28].copy_from_slice(&1_u16.to_le_bytes());
        entry(&mut data, 28, 0x927c, 7, 68, 44);
        data[44..54].copy_from_slice(b"Nikon\0\x02\x10\0\0");
        data[54..62].copy_from_slice(b"II*\0\x08\0\0\0");
        data[62..64].copy_from_slice(&1_u16.to_le_bytes());
        entry(&mut data, 64, 0x000c, 5, 4, 26);
        for (i, value) in [2_u32, 3, 99, 77].iter().enumerate() {
            data[80 + i * 8..84 + i * 8].copy_from_slice(&value.to_le_bytes());
            data[84 + i * 8..88 + i * 8].copy_from_slice(&1_u32.to_le_bytes());
        }
        let wb = nikon_wb(&CameraFile::parse_tiff("NEF", &data).unwrap()).unwrap();
        assert_eq!(wb.map(f32::to_bits), [2.0_f32, 1.0, 3.0, 1.0].map(f32::to_bits));
    }
    #[test]
    fn profiles_are_exact_and_invertible() {
        assert!(profile("SONY", "ILCE-7M3").is_some());
        assert!(profile("SONY", "ILCE-7M3 unknown").is_none());
        assert!(profile("UNKNOWN", "ILCE-7M3").is_none());
        for line in PROFILES.lines().filter(|l| !l.starts_with('#')) {
            let f: Vec<_> = line.split('\t').collect();
            assert!(
                camera_to_linear_srgb(profile(f[0], f[1]).unwrap()).is_some(),
                "{} {}",
                f[0],
                f[1]
            );
        }
    }
    #[test]
    #[allow(clippy::float_cmp)] // Exact binary-representable test ratios.
    fn wb_uses_measured_channels() {
        assert_eq!(
            gains("ARW", &[2000., 1000., 1500., 1000.], false).unwrap(),
            [2., 1., 1.5, 1.]
        );
        assert_eq!(gains("DNG", &[0.5, 1., 0.25], true).unwrap(), [2., 1., 4., 1.]);
        for values in [[0., 1., 2.], [f64::NAN, 1., 2.], [1., -1., 2.]] {
            assert!(gains("ARW", &values, false).is_err());
        }
    }
}

pub(crate) fn sony_r1_black(file: &CameraFile<'_>) -> Result<rrrah_core::LevelGrid, DecodeError> {
    let (decrypted, start) = sony_private(file)?;
    let tiff = crate::dng::tiff::Tiff::parse(&decrypted, crate::dng::tiff::Limits::default())
        .map_err(|e| camera_error("ARW", e.to_string()))?;
    let ifd = tiff
        .parse_ifd(start as u64)
        .map_err(|e| camera_error("ARW", e.to_string()))?;
    let entry = ifd
        .entry(0x7300)
        .map_err(|e| camera_error("ARW", e.to_string()))?
        .ok_or_else(|| camera_error("ARW", "DSC-R1 requires black levels tag 0x7300"))?;
    let values = entry
        .numeric_values()
        .map_err(|e| camera_error("ARW", e.to_string()))?;
    if values.len() != 4 || values.iter().any(|v| !v.is_finite() || *v < 0.0 || *v >= 16383.0) {
        return Err(camera_error("ARW", "invalid DSC-R1 black levels"));
    }
    // Tag channels are RGBG; the full-sensor CFA is GRBG.
    Ok(rrrah_core::LevelGrid {
        width: 2,
        height: 2,
        components: 1,
        values: [1, 0, 2, 3].map(|i| values[i] as f32).to_vec(),
    })
}
