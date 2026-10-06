//! Native Kodak DCS520C lossless-JPEG sensor and validated TIFF adapter.
use crate::DecodeError;
#[cfg(test)]
use rrrah_core::{MemoryBudget, PixelBuffer};
fn error(message: &str) -> DecodeError {
    super::camera_error("DCS", message)
}
#[cfg(test)]
fn decode_520(
    bytes: &[u8],
    budget: &MemoryBudget,
    cancelled: &(dyn Fn() -> bool + Sync),
) -> Result<PixelBuffer<u16>, DecodeError> {
    use super::CameraQuirks;
    if cancelled() {
        return Err(DecodeError::Cancelled);
    }
    let file = super::CameraFile::parse_tiff("DCS", bytes)?;
    let raw = DcsQuirks.select_raw_ifd(&file)?;
    DcsQuirks.read_metadata(&file, raw)?;
    let reservation = budget.try_reserve(1736 * 1160 * 2)?;
    let samples = DcsQuirks.decode_pixels(&file, raw, cancelled)?;
    Ok(reservation.try_adopt(samples)?.into())
}

// Shared native camera backend adapter. TIFF dispatch probes Kodak identity;
// metadata extraction independently validates the exact camera/storage subset.
pub(crate) struct DcsQuirks;
impl super::CameraQuirks for DcsQuirks {
    fn format_name(&self) -> &'static str {
        "DCS"
    }
    fn read_metadata(
        &self,
        file: &super::CameraFile<'_>,
        raw: &super::CameraDirectory<'_>,
    ) -> Result<super::CameraMetadata, DecodeError> {
        let root = file
            .directories()
            .first()
            .ok_or_else(|| error("missing root IFD"))?;
        let make = super::optional_ascii("DCS", root, 271)?.ok_or_else(|| error("missing make"))?;
        let model = super::optional_ascii("DCS", root, 272)?.ok_or_else(|| error("missing model"))?;
        if make != "Kodak" || model != "DCS520C" {
            return Err(error("unqualified camera"));
        }
        for (tag, expected) in [
            (256, 1736),
            (257, 1160),
            (258, 12),
            (259, 7),
            (273, 273056),
            (277, 1),
            (278, 1160),
            (279, 1697917),
        ] {
            if super::required_scalar("DCS", raw, tag)? != expected {
                return Err(error("unqualified sensor storage"));
            }
        }
        if file.data().get(273056..1970973).is_none() {
            return Err(error("truncated sensor"));
        }
        let cfa = raw.entry("DCS", 33422)?.ok_or_else(|| error("missing CFA"))?;
        if cfa.raw_bytes() != [1, 0, 2, 1] {
            return Err(error("unqualified CFA"));
        }
        let orientation = match super::optional_scalar("DCS", root, 274)? {
            Some(9) => rrrah_core::Orientation::Normal,
            Some(value) => super::orientation_from_tag("DCS", value)?,
            None => return Err(error("missing orientation")),
        };
        super::kodak_dcr_color::response_curve(file)?;
        let white_balance = super::kodak_dcr_color::white_balance(file)?;
        let xyz_to_camera =
            super::color::profile(&make, &model).ok_or_else(|| error("missing calibrated camera profile"))?;
        use rrrah_core::{CfaColor, CfaPattern, LevelGrid, Rect, WhiteLevel};
        Ok(super::CameraMetadata {
            make,
            model,
            width: 1736,
            height: 1160,
            bits_per_sample: 12,
            cfa: CfaPattern {
                width: 2,
                height: 2,
                cells: vec![CfaColor::Green, CfaColor::Red, CfaColor::Blue, CfaColor::Green],
            },
            // Exact model calibration: LibRaw colordata DCS520C black = 178.
            black_level: LevelGrid {
                width: 1,
                height: 1,
                components: 1,
                values: vec![178.],
            },
            white_level: WhiteLevel(vec![4095.]),
            white_balance,
            xyz_to_camera,
            active_area: None,
            crop_area: Some(Rect::new(0, 0, 1736, 1160)),
            orientation,
        })
    }
    fn decode_pixels(
        &self,
        file: &super::CameraFile<'_>,
        _: &super::CameraDirectory<'_>,
        cancelled: &(dyn Fn() -> bool + Sync),
    ) -> Result<Vec<u16>, DecodeError> {
        if cancelled() {
            return Err(DecodeError::Cancelled);
        }
        let curve = super::kodak_dcr_color::response_curve(file)?;
        let bytes = file
            .data()
            .get(273056..1970973)
            .ok_or_else(|| error("truncated sensor"))?;
        let mut image = crate::dng::lossless_jpeg::decode_kodak_dcs520(bytes, cancelled).map_err(|e| {
            if matches!(e, crate::dng::lossless_jpeg::LosslessJpegError::Cancelled { .. }) {
                DecodeError::Cancelled
            } else {
                error(&e.to_string())
            }
        })?;
        for row in image.samples.chunks_mut(1736) {
            if cancelled() {
                return Err(DecodeError::Cancelled);
            }
            for value in row {
                *value = *curve
                    .get(usize::from(*value))
                    .ok_or_else(|| error("sample outside response curve"))?;
            }
        }
        Ok(image.samples)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn cancelled_empty_source_has_no_allocation() {
        let budget = MemoryBudget::new(0);
        assert!(matches!(
            decode_520(&[], &budget, &|| true),
            Err(DecodeError::Cancelled)
        ));
        assert_eq!(budget.peak(), 0);
    }
    #[test]
    #[ignore = "requires original CC0 DCS520C TIFF and independent sensor oracle"]
    fn original_tiff_public_route_is_managed_and_matches_oracle() {
        use crate::RawDecoder;
        let path = std::path::PathBuf::from(std::env::var("RRRAH_DCS_SOURCE").unwrap());
        assert!(crate::sniff::is_kodak_dcs520(&path));
        let reference = std::fs::read(std::env::var("RRRAH_DCS_ORACLE").unwrap()).unwrap();
        let budget = MemoryBudget::new(16 * 1024 * 1024);
        let mut request = crate::DecodeRequest::new(&path);
        request.memory_budget = Some(budget.clone());
        assert_eq!(
            crate::NativeRawDecoder
                .mosaic_recipe(&request)
                .unwrap()
                .decoder_backend_id(),
            22
        );
        let crate::DecodedImage::Sensor(output) = crate::decode_image(&request).unwrap() else {
            panic!("Kodak TIFF selected a raster preview");
        };
        assert!(output.mosaic.pixels.is_managed());
        assert_eq!(output.mosaic.pixels.len() * 2, reference.len());
        assert!(
            output
                .mosaic
                .pixels
                .iter()
                .zip(reference.chunks_exact(2))
                .all(|(a, b)| *a == u16::from_le_bytes([b[0], b[1]]))
        );
        assert_eq!(output.mosaic.metadata.black_level.values, [178.]);
        assert_eq!(
            output.mosaic.metadata.white_balance,
            [1758. / 1834., 1., 3652. / 1834., 1.]
        );
        let preview = output.mosaic.thumbnail_rgba8(128);
        assert!(!preview.is_empty());
        assert!(preview.chunks_exact(4).all(|p| p[3] == 255));
        let held = output.mosaic.pixels.clone();
        drop(output);
        assert_eq!(budget.used(), 4027520);
        drop(held);
        assert_eq!(budget.used(), 0);
        let small = MemoryBudget::new(4027519);
        request.memory_budget = Some(small.clone());
        assert!(crate::decode_image(&request).is_err());
        assert_eq!(small.used(), 0);
    }

    #[test]
    #[ignore = "requires CC0 DCS520C source and independent sensor oracle"]
    fn native_response_curve_matches_first_oracle_sample() {
        let source = std::fs::read(std::env::var("RRRAH_DCS_SOURCE").unwrap()).unwrap();
        let reference = std::fs::read(std::env::var("RRRAH_DCS_ORACLE").unwrap()).unwrap();
        let file = super::super::CameraFile::parse_tiff("DCS", &source).unwrap();
        let curve = super::super::kodak_dcr_color::response_curve(&file).unwrap();
        assert_eq!(curve[482], u16::from_le_bytes(reference[..2].try_into().unwrap()));
        assert_eq!(curve[482], 1346);
    }

    #[test]
    #[ignore = "requires CC0 DCS520C source and independent sensor oracle"]
    fn real_sensor_and_wb_match_independent_oracle() {
        let source = std::fs::read(std::env::var("RRRAH_DCS_SOURCE").unwrap()).unwrap();
        let reference = std::fs::read(std::env::var("RRRAH_DCS_ORACLE").unwrap()).unwrap();
        use super::super::CameraQuirks;
        let file = super::super::CameraFile::parse_tiff("DCS", &source).unwrap();
        let raw = DcsQuirks.select_raw_ifd(&file).unwrap();
        let metadata = DcsQuirks.read_metadata(&file, raw).unwrap();
        assert_eq!((metadata.width, metadata.height), (1736, 1160));
        assert_eq!(metadata.black_level.values, [178.]);
        assert_eq!(metadata.white_level.0, [4095.]);
        assert_eq!(metadata.white_balance, [1758. / 1834., 1., 3652. / 1834., 1.]);
        assert_eq!(metadata.orientation, rrrah_core::Orientation::Normal);
        let adapted = DcsQuirks.decode_pixels(&file, raw, &|| false).unwrap();
        assert!(
            adapted
                .iter()
                .zip(reference.chunks_exact(2))
                .all(|(a, b)| *a == u16::from_le_bytes([b[0], b[1]]))
        );
        let budget = MemoryBudget::new(8 * 1024 * 1024);
        let output = decode_520(&source, &budget, &|| false).unwrap();
        assert_eq!(output.len() * 2, reference.len());
        for (i, (a, b)) in output.iter().zip(reference.chunks_exact(2)).enumerate() {
            assert_eq!(*a, u16::from_le_bytes([b[0], b[1]]), "sample {i}");
        }
        // Generic JPEG remains strict; only the qualified camera path admits this tail.
        assert!(crate::dng::lossless_jpeg::decode(&source[273056..], &|| false).is_err());
        for delta in [1_u8, 0x10, 0x80] {
            let mut bad_tail = source.clone();
            let last_entropy = bad_tail.len() - 3;
            bad_tail[last_entropy] ^= delta;
            assert!(decode_520(&bad_tail, &budget, &|| false).is_err());
            assert_eq!(budget.used(), 1736 * 1160 * 2);
        }
        let held = output.clone();
        drop(output);
        assert_eq!(budget.used(), 1736 * 1160 * 2);
        drop(held);
        assert_eq!(budget.used(), 0);
        let file = super::super::CameraFile::parse_tiff("DCS", &source).unwrap();
        assert_eq!(
            super::super::kodak_dcr_color::white_balance(&file).unwrap(),
            [1758. / 1834., 1., 3652. / 1834., 1.]
        );
        assert!(decode_520(&source, &MemoryBudget::new(1736 * 1160 * 2 - 1), &|| false).is_err());
        let mut malformed = source.clone();
        malformed[273056 + 36 + 5] = 0xff;
        assert!(decode_520(&malformed, &budget, &|| false).is_err());
        assert_eq!(budget.used(), 0);
        assert!(decode_520(&source[..source.len() - 1], &budget, &|| false).is_err());
    }
}
