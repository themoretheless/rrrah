//! Bounded Minolta MRW block parsing and managed packed-sensor decoding.
//! Color adaptation and the production RAW router are integrated separately.
use crate::DecodeError;
use rrrah_core::{MemoryBudget, PixelBuffer};

fn error(message: &str) -> DecodeError {
    DecodeError::NativeCamera {
        format: "MRW",
        message: message.into(),
    }
}

#[derive(Debug)]
pub struct MrwLayout<'a> {
    pub width: u32,
    pub height: u32,
    /// Four native RGGB channel coefficients; not normalized display gains.
    pub white_balance: [u16; 4],
    pub tiff_metadata: &'a [u8],
    packed: &'a [u8],
}

impl<'a> MrwLayout<'a> {
    pub fn parse(data: &'a [u8]) -> Result<Self, DecodeError> {
        if data.get(..4) != Some(b"\0MRM") {
            return Err(error("missing big-endian MRW signature"));
        }
        let bytes: [u8; 4] = data
            .get(4..8)
            .ok_or_else(|| error("truncated header"))?
            .try_into()
            .unwrap();
        let end = usize::try_from(u32::from_be_bytes(bytes))
            .map_err(|_| error("metadata size overflow"))?
            .checked_add(8)
            .ok_or_else(|| error("metadata end overflow"))?;
        if end > data.len() {
            return Err(error("metadata outside source"));
        }
        let mut position = 8_usize;
        let (mut prd, mut wbg, mut ttw) = (None, None, None);
        let mut blocks = 0;
        while position < end {
            blocks += 1;
            if blocks > 4096 {
                return Err(error("too many metadata blocks"));
            }
            let header_end = position
                .checked_add(8)
                .ok_or_else(|| error("block header overflow"))?;
            let header = data
                .get(position..header_end)
                .filter(|_| header_end <= end)
                .ok_or_else(|| error("truncated block header"))?;
            let length = u32::from_be_bytes(header[4..8].try_into().unwrap()) as usize;
            let block_end = header_end
                .checked_add(length)
                .filter(|v| *v <= end)
                .ok_or_else(|| error("block exceeds metadata boundary"))?;
            let payload = &data[header_end..block_end];
            let slot = match &header[..4] {
                b"\0PRD" => Some(&mut prd),
                b"\0WBG" => Some(&mut wbg),
                b"\0TTW" => Some(&mut ttw),
                _ => None,
            };
            if let Some(slot) = slot {
                if slot.replace(payload).is_some() {
                    return Err(error("duplicate required block"));
                }
            }
            position = block_end;
        }
        let prd = prd
            .filter(|v| v.len() == 24)
            .ok_or_else(|| error("PRD must contain 24 bytes"))?;
        let height = u32::from(u16::from_be_bytes(prd[8..10].try_into().unwrap()));
        let width = u32::from(u16::from_be_bytes(prd[10..12].try_into().unwrap()));
        // Qualified storage is continuous pairs of MSB-first 12-bit samples.
        if prd[16..19] != [12, 12, 0x59] || prd[23] != 1 {
            return Err(error("unsupported sensor storage or CFA mode"));
        }
        let samples = u64::from(width) * u64::from(height);
        if samples == 0 || samples > 100_000_000 || samples % 2 != 0 {
            return Err(error("invalid sensor dimensions"));
        }
        let packed = &data[end..];
        if packed.len() as u64 != samples / 2 * 3 {
            return Err(error("sensor payload length mismatch"));
        }
        let wbg = wbg
            .filter(|v| v.len() == 12)
            .ok_or_else(|| error("WBG must contain 12 bytes"))?;
        if wbg[..4] != [2; 4] {
            return Err(error("unqualified WBG channel scaling"));
        }
        let white_balance =
            std::array::from_fn(|i| u16::from_be_bytes(wbg[4 + i * 2..6 + i * 2].try_into().unwrap()));
        if white_balance.contains(&0) {
            return Err(error("zero white balance coefficient"));
        }
        let tiff_metadata = ttw.ok_or_else(|| error("missing TTW metadata"))?;
        Ok(Self {
            width,
            height,
            white_balance,
            tiff_metadata,
            packed,
        })
    }

    /// Reserves output before allocation and checks cancellation every 4096 pairs.
    pub fn decode_sensor(
        &self,
        budget: &MemoryBudget,
        mut cancelled: impl FnMut() -> bool,
    ) -> Result<PixelBuffer<u16>, DecodeError> {
        if cancelled() {
            return Err(DecodeError::Cancelled);
        }
        let count = usize::try_from(u64::from(self.width) * u64::from(self.height))
            .map_err(|_| DecodeError::DimensionOverflow)?;
        let mut output = budget.try_buffer(count, 0_u16)?;
        for (i, bytes) in self.packed.chunks_exact(3).enumerate() {
            if i % 4096 == 0 && cancelled() {
                return Err(DecodeError::Cancelled);
            }
            output[2 * i] = (u16::from(bytes[0]) << 4) | u16::from(bytes[1] >> 4);
            output[2 * i + 1] = (u16::from(bytes[1] & 15) << 8) | u16::from(bytes[2]);
        }
        Ok(output.freeze().into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn fixture() -> Vec<u8> {
        let mut prd = [0; 24];
        prd[8..10].copy_from_slice(&1_u16.to_be_bytes());
        prd[10..12].copy_from_slice(&4_u16.to_be_bytes());
        prd[16..19].copy_from_slice(&[12, 12, 0x59]);
        prd[23] = 1;
        let mut bytes = b"\0MRM\0\0\0\0".to_vec();
        for (tag, payload) in [
            (b"\0PRD", &prd[..]),
            (b"\0WBG", &[2, 2, 2, 2, 1, 179, 1, 0, 1, 0, 1, 179][..]),
            (b"\0TTW", &[][..]),
        ] {
            bytes.extend_from_slice(tag);
            bytes.extend_from_slice(&(payload.len() as u32).to_be_bytes());
            bytes.extend_from_slice(payload);
        }
        let end = bytes.len();
        bytes[4..8].copy_from_slice(&((end - 8) as u32).to_be_bytes());
        bytes.extend_from_slice(&[0, 0x1f, 0xff, 0x12, 0x34, 0x56]);
        bytes
    }
    #[test]
    fn managed_sensor_admission_cancellation_and_shared_ownership() {
        let bytes = fixture();
        let layout = MrwLayout::parse(&bytes).unwrap();
        let budget = MemoryBudget::new(8);
        assert!(layout.decode_sensor(&MemoryBudget::new(7), || false).is_err());
        assert!(matches!(
            layout.decode_sensor(&budget, || true),
            Err(DecodeError::Cancelled)
        ));
        assert_eq!(budget.peak(), 0);
        let mut calls = 0;
        assert!(matches!(
            layout.decode_sensor(&budget, || {
                calls += 1;
                calls > 1
            }),
            Err(DecodeError::Cancelled)
        ));
        assert_eq!(budget.used(), 0);
        let pixels = layout.decode_sensor(&budget, || false).unwrap();
        assert_eq!(&*pixels, &[1, 4095, 0x123, 0x456]);
        let held = pixels.clone();
        drop(pixels);
        assert_eq!(budget.used(), 8);
        drop(held);
        assert_eq!(budget.used(), 0);
    }
    #[test]
    fn every_truncation_and_unqualified_storage_fail_before_allocation() {
        let bytes = fixture();
        for end in 0..bytes.len() {
            assert!(MrwLayout::parse(&bytes[..end]).is_err(), "truncation {end}");
        }
        for offset in [32, 33, 34, 39] {
            let mut bad = bytes.clone();
            bad[offset] = 0;
            assert!(MrwLayout::parse(&bad).is_err());
        }
        let mut bad = bytes.clone();
        bad[12..16].copy_from_slice(&u32::MAX.to_be_bytes());
        assert!(MrwLayout::parse(&bad).is_err());
    }
    #[test]
    #[ignore = "requires CC0 MRW object 1826 and independent LibRaw sensor dump"]
    fn dynax_7d_matches_every_independent_sensor_sample() {
        let root = std::path::PathBuf::from(std::env::var("RRRAH_MRW_CORPUS").unwrap());
        let bytes = std::fs::read(root.join("1826.mrw")).unwrap();
        let reference = std::fs::read(root.join("reference.u16le")).unwrap();
        let layout = MrwLayout::parse(&bytes).unwrap();
        assert_eq!((layout.width, layout.height), (3016, 2008));
        assert_eq!(layout.white_balance, [435, 256, 256, 435]);
        let budget = MemoryBudget::new(16 * 1024 * 1024);
        let pixels = layout.decode_sensor(&budget, || false).unwrap();
        assert_eq!(pixels.len() * 2, reference.len());
        assert!(
            pixels
                .iter()
                .zip(reference.chunks_exact(2))
                .all(|(a, b)| *a == u16::from_le_bytes([b[0], b[1]]))
        );
        drop(pixels);
        assert_eq!(budget.used(), 0);
    }
}

/// Native packed MRW backend. Qualified for Dynax 7D and DiMAGE A2 packed sensor profiles.
#[derive(Debug, Clone, Copy, Default)]
pub struct NativeMrwDecoder;
impl NativeMrwDecoder {
    /// Same validated MRW pipeline for filesystem and in-memory source owners.
    pub(crate) fn decode_source<S: std::ops::Deref<Target = [u8]>>(
        &self,
        request: &crate::DecodeRequest,
        data: S,
        source_open: std::time::Duration,
        total: std::time::Instant,
    ) -> Result<crate::DecodeOutput, DecodeError> {
        use rrrah_core::{
            CfaColor, CfaPattern, DecodedMosaic, LevelGrid, Orientation, Photometric, RawMetadata, WhiteLevel,
        };
        use std::time::Instant;
        request.check_cancelled()?;
        if request.image_index != 0 {
            return Err(DecodeError::UnsupportedImageIndex {
                index: request.image_index,
            });
        }
        let selected = Instant::now();
        let layout = MrwLayout::parse(&data)?;
        let tiff = crate::camtiff::CameraFile::parse_tiff("MRW", layout.tiff_metadata)?;
        let ascii = |tag| -> Result<String, DecodeError> {
            for directory in tiff.directories() {
                if let Some(value) = crate::camtiff::optional_ascii("MRW", directory, tag)? {
                    return Ok(value);
                }
            }
            Err(error("missing camera identity"))
        };
        let make = ascii(271)?;
        let model = ascii(272)?;
        let white = match (make.as_str(), model.as_str(), layout.width, layout.height) {
            ("KONICA MINOLTA", "DYNAX 7D", 3016, 2008) => 4091.0,
            ("Konica Minolta Camera, Inc.", "DiMAGE A2", 3272, 2456) => 3983.0,
            _ => return Err(error("MRW camera color/level profile not yet qualified")),
        };
        let xyz_to_camera = crate::camtiff::color::profile(&make, &model)
            .ok_or_else(|| error("missing calibrated camera matrix"))?;
        let mut orientation = Orientation::Normal;
        for directory in tiff.directories() {
            if let Some(value) = crate::camtiff::optional_scalar("MRW", directory, 274)? {
                orientation = crate::camtiff::orientation_from_tag("MRW", value)?;
                break;
            }
        }
        let wb = layout.white_balance;
        let white_balance = [
            wb[0] as f32 / wb[1] as f32,
            1.0,
            wb[3] as f32 / wb[1] as f32,
            wb[2] as f32 / wb[1] as f32,
        ];
        let metadata = RawMetadata {
            make,
            model,
            width: layout.width,
            height: layout.height,
            components_per_pixel: 1,
            bits_per_sample: 12,
            photometric: Photometric::Cfa,
            cfa: Some(CfaPattern {
                width: 2,
                height: 2,
                cells: vec![CfaColor::Red, CfaColor::Green, CfaColor::Green, CfaColor::Blue],
            }),
            black_level: LevelGrid {
                width: 1,
                height: 1,
                components: 1,
                values: vec![0.],
            },
            white_level: WhiteLevel(vec![white]),
            white_balance,
            xyz_to_camera,
            active_area: None,
            crop_area: None,
            orientation,
        };
        let decoder_select = selected.elapsed();
        request.check_cancelled()?;
        let raw_started = Instant::now();
        let budget = request
            .memory_budget
            .clone()
            .unwrap_or_else(|| MemoryBudget::new(u64::MAX));
        let pixels = layout.decode_sensor(&budget, || request.check_cancelled().is_err())?;
        let raw_image = raw_started.elapsed();
        drop(data);
        request.check_cancelled()?;
        let adapt_started = Instant::now();
        let mosaic = DecodedMosaic::new(metadata, pixels)?;
        let adapt_metadata = adapt_started.elapsed();
        Ok(crate::DecodeOutput {
            mosaic,
            timings: crate::DecodeTimings {
                source_open,
                decoder_select,
                raw_image,
                raw_decode: decoder_select + raw_image,
                native: None,
                dng: None,
                adapt: crate::AdaptTimings {
                    total: adapt_metadata,
                    ..Default::default()
                },
                adapt_metadata,
                total: total.elapsed(),
            },
        })
    }
}
impl crate::RawDecoder for NativeMrwDecoder {
    fn mosaic_recipe(
        &self,
        _: &crate::DecodeRequest,
    ) -> Result<rrrah_core::MosaicRecipeManifest, DecodeError> {
        Ok(rrrah_core::MosaicRecipeManifest::new(
            12,
            1,
            1,
            1,
            rrrah_core::DECODE_FULL_SENSOR_RAW
                | rrrah_core::DECODE_INTEGER_U16
                | rrrah_core::DECODE_SENSOR_COORDINATES
                | rrrah_core::DECODE_CROP_AS_METADATA
                | rrrah_core::DECODE_IMAGE_INDEX_IN_KEY,
            crate::WORKSPACE_LOCK_DIGEST,
        ))
    }
    fn decode(&self, request: &crate::DecodeRequest) -> Result<crate::DecodeOutput, DecodeError> {
        use std::time::Instant;
        let total = Instant::now();
        request.check_cancelled()?;
        if request.image_index != 0 {
            return Err(DecodeError::UnsupportedImageIndex {
                index: request.image_index,
            });
        }
        let source_started = Instant::now();
        let data = crate::bounded_io::read_managed(request)?;
        let source_open = source_started.elapsed();
        self.decode_source(request, data, source_open, total)
    }
}
