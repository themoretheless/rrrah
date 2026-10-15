//! File-origin Hasselblad calibration, kept separate from camera-table profiles.
use super::{CameraDirectory, camera_error};
use crate::DecodeError;

#[derive(Debug)]
pub(crate) struct FileColor {
    pub white_balance: [f32; 4],
    pub xyz_to_camera: [[f32; 3]; 4],
}

pub(crate) fn read_file_color(directory: &CameraDirectory<'_>) -> Result<FileColor, DecodeError> {
    let values = |tag| {
        directory
            .entry("3FR", tag)?
            .ok_or_else(|| camera_error("3FR", format!("missing color tag {tag}")))?
            .numeric_values()
            .map_err(|e| camera_error("3FR", e.to_string()))
    };
    let neutral = values(50728)?;
    let white_balance = super::color::gains("3FR", &neutral, true)?;
    let coefficients = values(50721)?;
    if coefficients.len() != 9 || coefficients.iter().any(|v| !v.is_finite()) {
        return Err(camera_error("3FR", "invalid embedded color matrix"));
    }
    let mut matrix = [[0.0; 3]; 4];
    for (dst, src) in matrix[..3].iter_mut().flatten().zip(coefficients) {
        *dst = src as f32;
    }
    if rrrah_core::camera_to_linear_srgb(matrix).is_none() {
        return Err(camera_error("3FR", "noninvertible embedded color matrix"));
    }
    Ok(FileColor {
        white_balance,
        xyz_to_camera: matrix,
    })
}

#[cfg(test)]
mod tests {
    #[test]
    #[ignore = "requires external pinned CC0 Hasselblad X1D object 2058"]
    fn real_3fr_retains_embedded_precision_and_rejects_zero_neutral() {
        let path = std::env::var("RRRAH_HASSELBLAD_3FR_SOURCE").unwrap();
        let mut bytes = std::fs::read(path).unwrap();
        assert_eq!(&bytes[355..370], b"Hasselblad X1D\0");
        {
            let file = super::super::CameraFile::parse_tiff("3FR", &bytes).unwrap();
            let color = super::read_file_color(&file.directories()[0]).unwrap();
            assert_eq!(
                color.white_balance,
                [103138.0 / 65536.0, 1.0, 169446.0 / 65536.0, 1.0]
            );
            assert_eq!(color.xyz_to_camera[0], [0.493206, -0.083518, 0.014067]);
            assert_ne!(color.xyz_to_camera[0][0], 0.4932);
        }
        // Zero numerator in the file's first AsShotNeutral rational.
        assert_eq!(&bytes[567..571], &65536u32.to_le_bytes());
        bytes[567..571].fill(0);
        let file = super::super::CameraFile::parse_tiff("3FR", &bytes).unwrap();
        assert!(super::read_file_color(&file.directories()[0]).is_err());
    }
}
