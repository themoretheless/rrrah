//! Native P20+ format-3 pipeline, with qualified cache and Metal integration.
use super::{CameraMetadata, camera_error, phaseone_container::Directory};
use crate::DecodeError;
use rrrah_core::{CfaColor, CfaPattern, LevelGrid, Orientation, Rect, WhiteLevel};
fn error(message: &str) -> DecodeError {
    camera_error("IIQ", message)
}
pub(super) struct Prepared<'a> {
    pub(super) metadata: CameraMetadata,
    sensor: &'a [u8],
    rows: &'a [u8],
    defects: &'a [u8],
    fields: [&'a [u8]; 3],
}
impl<'a> Prepared<'a> {
    pub(super) fn parse(bytes: &'a [u8]) -> Result<Self, DecodeError> {
        if bytes.get(..4) != Some(&b"II\x2a\0"[..]) {
            return Err(error("unqualified TIFF header"));
        }
        let root = Directory::open(bytes, 8, false)?;
        for (tag, value) in [
            (0x108, 4134),
            (0x109, 4128),
            (0x10e, 3),
            (0x100, 1),
            (0x21d, 1024),
            (0x10a, 26),
            (0x10b, 21),
            (0x10c, 4096),
            (0x10d, 4094),
        ] {
            if root.scalar(tag)? != value {
                return Err(error("unqualified P20+ geometry or storage"));
            }
        }
        let model = root.payload(0x301, 256)?;
        if !model.starts_with(b"P20+-H, Firmware:") {
            return Err(error("unqualified camera profile"));
        }
        let wb = root.payload(0x107, 12)?;
        if wb.len() != 12 {
            return Err(error("invalid native white balance length"));
        }
        let mut gains = [0f64; 3];
        for (value, word) in gains.iter_mut().zip(wb.chunks_exact(4)) {
            *value = f32::from_le_bytes(word.try_into().unwrap()) as f64;
        }
        let white_balance = super::color::gains("IIQ", &gains, false)?;
        let xyz_to_camera = super::color::profile("Phase One", "P20+")
            .ok_or_else(|| error("missing calibrated P20+ profile"))?;
        let sensor = root.payload(0x10f, 64 * 1024 * 1024)?;
        let rows = root.payload(0x21c, 4128 * 4)?;
        if rows.len() != 4128 * 4 {
            return Err(error("invalid row table length"));
        }
        let calibration = root.payload(0x110, 8 * 1024 * 1024)?;
        let directory = Directory::open(calibration, 0, true)?;
        for tag in directory.tags() {
            if !matches!(
                tag,
                0x404
                    | 0x405
                    | 0x406
                    | 0x407
                    | 0x402
                    | 0x408
                    | 0x413
                    | 0x40f
                    | 0x418
                    | 0x400
                    | 0x416
                    | 0x410
                    | 0x40b
            ) {
                return Err(error("unqualified calibration operation"));
            }
        }
        let defects = directory.payload(0x400, 65536 * 8)?;
        let fields = [
            directory.payload(0x416, 2 * 1024 * 1024)?,
            directory.payload(0x410, 2 * 1024 * 1024)?,
            directory.payload(0x40b, 2 * 1024 * 1024)?,
        ];
        Ok(Self {
            metadata: CameraMetadata {
                make: "Phase One".into(),
                model: "P20+".into(),
                width: 4134,
                height: 4128,
                bits_per_sample: 16,
                cfa: CfaPattern {
                    width: 2,
                    height: 2,
                    cells: vec![CfaColor::Green, CfaColor::Blue, CfaColor::Red, CfaColor::Green],
                },
                black_level: LevelGrid {
                    width: 1,
                    height: 1,
                    components: 1,
                    values: vec![0.],
                },
                white_level: WhiteLevel(vec![64508.]),
                white_balance,
                xyz_to_camera,
                active_area: None,
                // Align the stored odd top margin to the next Bayer row, preserving the bottom.
                crop_area: Some(Rect::new(26, 22, 4096, 4093)),
                orientation: Orientation::Rotate90,
            },
            sensor,
            rows,
            defects,
            fields,
        })
    }
    pub(super) fn decode(&self, cancelled: &dyn Fn() -> bool) -> Result<Vec<u16>, DecodeError> {
        if cancelled() {
            return Err(DecodeError::Cancelled);
        }
        let offsets: Vec<u32> = self
            .rows
            .chunks_exact(4)
            .map(|b| u32::from_le_bytes(b.try_into().unwrap()))
            .collect();
        let mut pixels = super::phaseone_entropy::decode(self.sensor, &offsets, 4134, 4128, cancelled)?;
        super::phaseone_calibration::subtract_black(&mut pixels, 4134, 1024, cancelled)?;
        super::phaseone_calibration::repair_pixels(&mut pixels, 4134, 4128, self.defects, cancelled)?;
        for (field, channels) in self.fields.iter().zip([2, 2, 4]) {
            super::phaseone_calibration::flat_field(&mut pixels, 4134, 4128, field, channels, cancelled)?;
        }
        super::phaseone_calibration::repair_columns(&mut pixels, 4134, 4128, self.defects, cancelled)?;
        Ok(pixels)
    }
}
pub(crate) struct IiqQuirks;
impl super::CameraQuirks for IiqQuirks {
    fn format_name(&self) -> &'static str {
        "IIQ"
    }
    fn select_raw_ifd<'a>(
        &self,
        file: &'a super::CameraFile<'a>,
    ) -> Result<&'a super::CameraDirectory<'a>, DecodeError> {
        file.directories()
            .first()
            .ok_or_else(|| error("missing TIFF directory"))
    }
    fn read_metadata(
        &self,
        file: &super::CameraFile<'_>,
        _: &super::CameraDirectory<'_>,
    ) -> Result<CameraMetadata, DecodeError> {
        Ok(Prepared::parse(file.data())?.metadata)
    }
    fn decode_pixels(
        &self,
        file: &super::CameraFile<'_>,
        _: &super::CameraDirectory<'_>,
        cancelled: &(dyn Fn() -> bool + Sync),
    ) -> Result<Vec<u16>, DecodeError> {
        Prepared::parse(file.data())?.decode(cancelled)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    #[ignore = "external P20+ source/oracle; informational per-stage timing, no wall-clock gate"]
    fn real_pipeline_stage_timing_preserves_oracle() {
        use std::time::Instant;
        let source = std::fs::read(std::env::var("RRRAH_IIQ_SOURCE").unwrap()).unwrap();
        let oracle = std::fs::read(std::env::var("RRRAH_IIQ_CORRECTED_ORACLE").unwrap()).unwrap();
        let expected: Vec<u16> = oracle
            .chunks_exact(2)
            .map(|b| u16::from_le_bytes(b.try_into().unwrap()))
            .collect();
        let mut timings: [Vec<f64>; 8] = std::array::from_fn(|_| Vec::new());
        for iteration in 0..15 {
            let mut elapsed = [0f64; 8];
            let started = Instant::now();
            let prepared = Prepared::parse(&source).unwrap();
            let offsets: Vec<u32> = prepared
                .rows
                .chunks_exact(4)
                .map(|b| u32::from_le_bytes(b.try_into().unwrap()))
                .collect();
            elapsed[0] = started.elapsed().as_secs_f64() * 1000.;
            let started = Instant::now();
            let mut pixels =
                super::super::phaseone_entropy::decode(prepared.sensor, &offsets, 4134, 4128, &|| false)
                    .unwrap();
            elapsed[1] = started.elapsed().as_secs_f64() * 1000.;
            let started = Instant::now();
            super::super::phaseone_calibration::subtract_black(&mut pixels, 4134, 1024, &|| false).unwrap();
            elapsed[2] = started.elapsed().as_secs_f64() * 1000.;
            let started = Instant::now();
            super::super::phaseone_calibration::repair_pixels(
                &mut pixels,
                4134,
                4128,
                prepared.defects,
                &|| false,
            )
            .unwrap();
            elapsed[3] = started.elapsed().as_secs_f64() * 1000.;
            for (index, (field, channels)) in prepared.fields.iter().zip([2, 2, 4]).enumerate() {
                let started = Instant::now();
                super::super::phaseone_calibration::flat_field(
                    &mut pixels,
                    4134,
                    4128,
                    field,
                    channels,
                    &|| false,
                )
                .unwrap();
                elapsed[4 + index] = started.elapsed().as_secs_f64() * 1000.;
            }
            let started = Instant::now();
            super::super::phaseone_calibration::repair_columns(
                &mut pixels,
                4134,
                4128,
                prepared.defects,
                &|| false,
            )
            .unwrap();
            elapsed[7] = started.elapsed().as_secs_f64() * 1000.;
            assert_eq!(pixels, expected, "iteration {iteration}");
            if iteration >= 3 {
                for (samples, value) in timings.iter_mut().zip(elapsed) {
                    samples.push(value);
                }
                eprintln!("iiq_stage_sample,{},{elapsed:?}", iteration - 2);
            }
        }
        for (name, mut samples) in [
            "parse",
            "entropy",
            "black",
            "pixels",
            "luma416",
            "luma410",
            "chroma40b",
            "columns",
        ]
        .into_iter()
        .zip(timings)
        {
            samples.sort_by(f64::total_cmp);
            eprintln!(
                "iiq_stage,{name},p50_ms={:.6},p95_ms={:.6}",
                samples[6], samples[10]
            );
        }
    }
    #[test]
    #[ignore = "requires controlled P20+ clustered-column variants and LibRaw corrected oracles"]
    fn real_clustered_columns_match_full_independent_sensors() {
        use crate::RawDecoder;
        let root = std::env::var("RRRAH_IIQ_CLUSTER_ROOT").unwrap();
        for variant in ["nearby", "adjacent", "edge"] {
            let budget = rrrah_core::MemoryBudget::new(64 * 1024 * 1024);
            let mut request = crate::DecodeRequest::new(format!("{root}/rrrah-iiq-{variant}.iiq"));
            request.memory_budget = Some(budget.clone());
            let decoded = crate::NativeRawDecoder.decode(&request).unwrap();
            let reference = std::fs::read(format!("{root}/rrrah-iiq-{variant}-reference.u16le")).unwrap();
            assert_eq!(decoded.mosaic.pixels.len() * 2, reference.len());
            for (i, (a, b)) in decoded
                .mosaic
                .pixels
                .iter()
                .zip(reference.chunks_exact(2))
                .enumerate()
            {
                assert_eq!(
                    *a,
                    u16::from_le_bytes(b.try_into().unwrap()),
                    "{variant} sample {i}"
                );
            }
            drop(decoded);
            assert_eq!(budget.used(), 0);
        }
    }
    #[test]
    #[ignore = "requires pinned P20+ IIQ and corrected oracle"]
    fn public_iiq_router_managed_memory_and_full_sensor() {
        use crate::RawDecoder;
        let source = std::env::var("RRRAH_IIQ_SOURCE").unwrap();
        let mut request = crate::DecodeRequest::new(source);
        let budget = rrrah_core::MemoryBudget::new(64 * 1024 * 1024);
        request.memory_budget = Some(budget.clone());
        let output = crate::NativeRawDecoder.decode(&request).unwrap();
        assert!(output.mosaic.pixels.is_managed());
        assert_eq!(budget.used(), 34130304);
        assert!(budget.peak() <= budget.limit());
        let oracle = std::fs::read(std::env::var("RRRAH_IIQ_CORRECTED_ORACLE").unwrap()).unwrap();
        assert_eq!(oracle.len(), output.mosaic.pixels.len() * 2);
        for (i, (a, b)) in output
            .mosaic
            .pixels
            .iter()
            .zip(oracle.chunks_exact(2))
            .enumerate()
        {
            assert_eq!(*a, u16::from_le_bytes(b.try_into().unwrap()), "sample {i}");
        }
        let owner = output.mosaic.clone();
        drop(output);
        assert_eq!(budget.used(), 34130304);
        drop(owner);
        assert_eq!(budget.used(), 0);
        let small = rrrah_core::MemoryBudget::new(32 * 1024 * 1024);
        request.memory_budget = Some(small.clone());
        assert!(matches!(
            crate::NativeRawDecoder.decode(&request),
            Err(DecodeError::Memory(_))
        ));
        assert_eq!(small.used(), 0);
    }
    #[test]
    #[ignore = "requires pinned P20+ source and original full corrected oracle"]
    fn real_native_pipeline_matches_independent_full_sensor_and_color() {
        let source = std::fs::read(std::env::var("RRRAH_IIQ_SOURCE").unwrap()).unwrap();
        let prepared = Prepared::parse(&source).unwrap();
        for (actual, expected) in
            prepared.metadata.white_balance[..3]
                .iter()
                .zip([2.314913273, 1., 1.031825066])
        {
            assert!((*actual as f64 - expected).abs() < 1e-6);
        }
        assert_eq!(prepared.metadata.crop_area, Some(Rect::new(26, 22, 4096, 4093)));
        let rgb = rrrah_core::camera_to_linear_srgb(prepared.metadata.xyz_to_camera).unwrap();
        let expected = [
            [2.449301481f32, -1.270811081, -0.1784904301],
            [-0.06848108023, 1.24422729, -0.1757462472],
            [0.1003994048, -0.5513836741, 1.45098424],
        ];
        for (actual, reference) in rgb.iter().flatten().zip(expected.iter().flatten()) {
            assert!((actual - reference).abs() < 1e-5);
        }
        let output = prepared.decode(&|| false).unwrap();
        let oracle = std::fs::read(std::env::var("RRRAH_IIQ_CORRECTED_ORACLE").unwrap()).unwrap();
        assert_eq!(output.len() * 2, oracle.len());
        for (i, (actual, reference)) in output.iter().zip(oracle.chunks_exact(2)).enumerate() {
            assert_eq!(
                *actual,
                u16::from_le_bytes(reference.try_into().unwrap()),
                "sample {i}"
            );
        }
        assert!(matches!(prepared.decode(&|| true), Err(DecodeError::Cancelled)));
        drop(prepared);
        let mut invalid = source.clone();
        invalid[20583392..20583396].copy_from_slice(&0f32.to_le_bytes());
        assert!(Prepared::parse(&invalid).is_err());
        invalid[20583392..20583396].copy_from_slice(&f32::NAN.to_le_bytes());
        assert!(Prepared::parse(&invalid).is_err());
        invalid = source.clone();
        invalid[20608244] = b'X';
        assert!(Prepared::parse(&invalid).is_err());
        assert!(Prepared::parse(&source[..21348900]).is_err());
    }
}
