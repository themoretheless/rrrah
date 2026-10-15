//! Bounded DSC-F828 encrypted sensor extraction. RGBE color integration is separate.
use super::camera_error;
use crate::DecodeError;
fn error(message: &str) -> DecodeError {
    camera_error("SRF", message)
}
struct Cipher {
    state: [u32; 128],
    position: usize,
}
impl Cipher {
    fn new(mut key: u32) -> Self {
        let mut state = [0u32; 128];
        for value in &mut state[..4] {
            key = key.wrapping_mul(48_828_125).wrapping_add(1);
            *value = key;
        }
        state[3] = (state[3] << 1) | ((state[0] ^ state[2]) >> 31);
        for i in 4..127 {
            state[i] = ((state[i - 4] ^ state[i - 2]) << 1) | ((state[i - 3] ^ state[i - 1]) >> 31);
        }
        for value in &mut state[..127] {
            *value = value.swap_bytes();
        }
        Self { state, position: 127 }
    }
    fn word(&mut self, input: [u8; 4]) -> [u8; 4] {
        let p = self.position;
        let value = self.state[(p + 1) & 127] ^ self.state[(p + 65) & 127];
        self.state[p & 127] = value;
        self.position = (p + 1) & 127;
        (u32::from_le_bytes(input) ^ value).to_le_bytes()
    }
}
pub(super) fn decode_f828(bytes: &[u8], cancelled: &dyn Fn() -> bool) -> Result<Vec<u16>, DecodeError> {
    if cancelled() {
        return Err(DecodeError::Cancelled);
    }
    let marker = *bytes
        .get(200896)
        .ok_or_else(|| error("truncated master key selector"))? as usize;
    let key_at = 200896 + marker * 4;
    let master = u32::from_be_bytes(
        bytes
            .get(key_at..key_at + 4)
            .ok_or_else(|| error("truncated master key"))?
            .try_into()
            .unwrap(),
    );
    let encrypted_header = bytes
        .get(164600..164640)
        .ok_or_else(|| error("truncated encrypted key header"))?;
    let mut header = [0u8; 40];
    let mut cipher = Cipher::new(master);
    for (src, dst) in encrypted_header.chunks_exact(4).zip(header.chunks_exact_mut(4)) {
        dst.copy_from_slice(&cipher.word(src.try_into().unwrap()));
    }
    let mut key = 0u32;
    for &value in header[22..26].iter().rev() {
        key = (key << 8) | value as u32;
    }
    const WIDTH: usize = 3360;
    const HEIGHT: usize = 2460;
    let sensor = bytes
        .get(862144..862144 + WIDTH * HEIGHT * 2)
        .ok_or_else(|| error("truncated encrypted sensor"))?;
    let mut output = Vec::new();
    output
        .try_reserve_exact(WIDTH * HEIGHT)
        .map_err(|_| error("sensor allocation failed"))?;
    let mut cipher = Cipher::new(key);
    for row in sensor.chunks_exact(WIDTH * 2) {
        if cancelled() {
            return Err(DecodeError::Cancelled);
        }
        for word in row.chunks_exact(4) {
            let decrypted = cipher.word(word.try_into().unwrap());
            for pair in decrypted.chunks_exact(2) {
                let value = u16::from_be_bytes(pair.try_into().unwrap());
                if value > 16383 {
                    return Err(error("decrypted sample exceeds 14 bits"));
                }
                output.push(value);
            }
        }
    }
    Ok(output)
}
/// Extract the three declared as-shot coefficients. A fourth RGBE coefficient
/// must be resolved separately; this API does not silently duplicate green.
pub(super) fn as_shot_rgb_f828(bytes: &[u8]) -> Result<[u16; 3], DecodeError> {
    let selector = *bytes
        .get(200896)
        .ok_or_else(|| error("truncated WB master selector"))? as usize;
    let at = 200896 + selector * 4;
    let key = u32::from_be_bytes(
        bytes
            .get(at..at + 4)
            .ok_or_else(|| error("truncated WB master key"))?
            .try_into()
            .unwrap(),
    );
    let encrypted = bytes
        .get(164600..164640 + 2048)
        .ok_or_else(|| error("truncated encrypted WB tables"))?;
    let mut first = Cipher::new(key);
    let mut header = [0u8; 40];
    for (src, dst) in encrypted[..40].chunks_exact(4).zip(header.chunks_exact_mut(4)) {
        dst.copy_from_slice(&first.word(src.try_into().unwrap()));
    }
    let u16at = |b: &[u8], at| u16::from_le_bytes([b[at], b[at + 1]]);
    let u32at = |b: &[u8], at| u32::from_le_bytes(b[at..at + 4].try_into().unwrap());
    if u16at(&header, 0) != 2
        || u16at(&header, 2) != 0
        || u16at(&header, 4) != 4
        || u32at(&header, 6) != 1
        || u16at(&header, 14) != 1
        || u16at(&header, 16) != 4
        || u32at(&header, 18) != 1
        || u32at(&header, 26) != 164640
    {
        return Err(error("unqualified SRF1 key directory"));
    }
    let mut second = Cipher::new(u32at(&header, 10));
    let mut table = [0u8; 2048];
    for (src, dst) in encrypted[40..].chunks_exact(4).zip(table.chunks_exact_mut(4)) {
        dst.copy_from_slice(&second.word(first.word(src.try_into().unwrap())));
    }
    let count = u16at(&table, 0) as usize;
    if count == 0 || 2 + count * 12 + 4 > table.len() {
        return Err(error("WB directory exceeds bounded window"));
    }
    let mut values = [None; 3];
    for record in table[2..2 + count * 12].chunks_exact(12) {
        let tag = u16at(record, 0);
        if (0xd0..=0xd2).contains(&tag) {
            let index = (tag - 0xd0) as usize;
            if values[index].is_some()
                || u16at(record, 2) != 3
                || u32at(record, 4) != 1
                || u16at(record, 10) != 0
            {
                return Err(error("invalid as-shot WB entry"));
            }
            let value = u16at(record, 8);
            if value == 0 {
                return Err(error("zero as-shot WB coefficient"));
            }
            values[index] = Some(value);
        }
    }
    Ok([
        values[0].ok_or_else(|| error("missing red WB"))?,
        values[1].ok_or_else(|| error("missing green WB"))?,
        values[2].ok_or_else(|| error("missing blue WB"))?,
    ])
}
/// DSC-F828 SRF WB interpretation: native SRF2 stores R/G/B coefficients,
/// and the SRF format policy assigns the green coefficient to E. This matches
/// LibRaw 0.22.0 parseSonySRF, tags 0xd0..0xd2 (cam_mul[3] = cam_mul[1]).
/// It shares a gain, not a sensor plane or matrix row. Missing/zero values fail.
/// This is source-parser policy evidence, not photographic color certification.
pub(super) fn white_balance_f828(bytes: &[u8]) -> Result<[f32; 4], DecodeError> {
    let [red, green, blue] = as_shot_rgb_f828(bytes)?;
    let denominator = f32::from(green);
    Ok([
        f32::from(red) / denominator,
        1.,
        f32::from(blue) / denominator,
        1.,
    ])
}
/// Qualified DSC-F828 metadata; kept private until the complete viewer route
/// supports RGBE. Sensor-phase black levels retain the Emerald plane.
pub(super) fn metadata_f828(bytes: &[u8]) -> Result<rrrah_core::RawMetadata, DecodeError> {
    use rrrah_core::{
        CfaColor::*, CfaPattern, LevelGrid, Orientation, Photometric, RawMetadata, Rect, WhiteLevel,
    };
    let file = super::CameraFile::parse_tiff("SRF", bytes)?;
    let root = file
        .directories()
        .first()
        .ok_or_else(|| error("missing root IFD"))?;
    if file.data().get(..4) != Some(&b"II\x2a\0"[..])
        || root.offset() != 8
        || super::optional_ascii("SRF", root, 271)?.as_deref() != Some("SONY")
        || super::optional_ascii("SRF", root, 272)?.as_deref() != Some("DSC-F828")
        || super::required_scalar("SRF", root, 256)? != 3360
        || super::required_scalar("SRF", root, 257)? != 2460
        || super::required_scalar("SRF", root, 279)? != 16531200
        || super::required_scalar("SRF", root, 274)? != 1
        || bytes.len() != 17393344
    {
        return Err(error("unqualified DSC-F828 container or geometry"));
    }
    let metadata = RawMetadata {
        make: "Sony".into(),
        model: "DSC-F828".into(),
        width: 3360,
        height: 2460,
        components_per_pixel: 1,
        bits_per_sample: 14,
        photometric: Photometric::Cfa,
        cfa: Some(CfaPattern {
            width: 2,
            height: 2,
            cells: vec![Emerald, Red, Blue, Green],
        }),
        black_level: LevelGrid {
            width: 2,
            height: 2,
            components: 1,
            values: vec![492., 558., 491., 621.],
        },
        white_level: WhiteLevel(vec![16368.]),
        white_balance: white_balance_f828(bytes)?,
        xyz_to_camera: [
            [0.7924, -0.1910, -0.0777],
            [-0.8226, 1.5459, 0.2998],
            [-0.1517, 0.2199, 0.6818],
            [-0.7242, 1.1401, 0.3481],
        ],
        active_area: None,
        crop_area: Some(Rect::new(6, 0, 3287, 2460)),
        orientation: Orientation::Normal,
    };
    metadata.validate()?;
    rrrah_core::rgbe::Calibration::new(&metadata).map_err(|e| error(&e.to_string()))?;
    Ok(metadata)
}
pub(crate) struct SrfQuirks;
impl super::CameraQuirks for SrfQuirks {
    fn format_name(&self) -> &'static str {
        "SRF"
    }
    fn select_raw_ifd<'a>(
        &self,
        file: &'a super::CameraFile<'a>,
    ) -> Result<&'a super::CameraDirectory<'a>, DecodeError> {
        file.directories()
            .first()
            .ok_or_else(|| error("missing root IFD"))
    }
    fn read_metadata(
        &self,
        file: &super::CameraFile<'_>,
        _: &super::CameraDirectory<'_>,
    ) -> Result<super::CameraMetadata, DecodeError> {
        let m = metadata_f828(file.data())?;
        Ok(super::CameraMetadata {
            make: m.make,
            model: m.model,
            width: m.width,
            height: m.height,
            bits_per_sample: m.bits_per_sample,
            cfa: m.cfa.unwrap(),
            black_level: m.black_level,
            white_level: m.white_level,
            white_balance: m.white_balance,
            xyz_to_camera: m.xyz_to_camera,
            active_area: m.active_area,
            crop_area: m.crop_area,
            orientation: m.orientation,
        })
    }
    fn decode_pixels(
        &self,
        file: &super::CameraFile<'_>,
        _: &super::CameraDirectory<'_>,
        cancelled: &(dyn Fn() -> bool + Sync),
    ) -> Result<Vec<u16>, DecodeError> {
        decode_f828(file.data(), cancelled)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn cipher_continues_across_chunks_and_sensor_rejects_truncation() {
        let mut a = Cipher::new(0x12345678);
        let mut b = Cipher::new(0x12345678);
        for i in 0..300u32 {
            let encrypted = a.word(i.to_le_bytes());
            assert_eq!(b.word(encrypted), i.to_le_bytes());
        }
        assert!(decode_f828(&[], &|| false).is_err());
        assert!(as_shot_rgb_f828(&[]).is_err());
        assert!(matches!(decode_f828(&[], &|| true), Err(DecodeError::Cancelled)));
    }
    #[test]
    #[ignore = "requires pinned DSC-F828 SRF and independent LibRaw unpack oracle"]
    fn real_full_encrypted_sensor_matches_independent_oracle() {
        let source = std::fs::read(std::env::var("RRRAH_SRF_SOURCE").unwrap()).unwrap();
        let reference = std::fs::read(std::env::var("RRRAH_SRF_ORACLE").unwrap()).unwrap();
        let metadata = metadata_f828(&source).unwrap();
        assert_eq!(metadata.cfa.as_ref().unwrap().rgbe_quad().unwrap(), [3, 0, 2, 1]);
        assert_eq!(metadata.black_level.values, [492., 558., 491., 621.]);
        let calibration = rrrah_core::rgbe::Calibration::new(&metadata).unwrap();
        for (phase, black) in [492u16, 558, 491, 621].into_iter().enumerate() {
            assert_eq!(
                calibration.normalize_site((phase % 2) as u32, (phase / 2) as u32, black),
                Some(0.)
            );
        }
        // Independent LibRaw rgb_cam[3][4] oracle includes the Emerald column.
        let oracle_matrix = [
            [1.63739419, -0.252761811, -0.003541585058, -0.38109079],
            [0.06718456, 0.8223665357, -0.5305757523, 0.6410246491],
            [-0.0008971288335, -0.3551472425, 1.415327907, -0.05928355828],
        ];
        for (actual, expected) in calibration.camera_to_rgb().into_iter().zip(oracle_matrix) {
            for (a, b) in actual.into_iter().zip(expected) {
                assert!((a - b).abs() < 1e-6);
            }
        }
        for planes in [[1.; 4], [2.5, 0.1, 1.7, 0.9], [0., 0., 0., 1.]] {
            let actual = calibration.linear_rgb(planes).unwrap();
            for (a, row) in actual.into_iter().zip(oracle_matrix) {
                let expected: f64 = row.into_iter().zip(planes).map(|(m, v)| m * v).sum();
                assert!((a - expected).abs() < 4e-6);
            }
        }
        let mut bad_model = source.clone();
        bad_model[274] = b'X';
        assert!(metadata_f828(&bad_model).is_err());
        let mut bad_size = source.clone();
        bad_size.pop();
        assert!(metadata_f828(&bad_size).is_err());
        assert_eq!(as_shot_rgb_f828(&source).unwrap(), [397, 256, 728]);
        assert_eq!(
            white_balance_f828(&source).unwrap(),
            [397. / 256., 1., 728. / 256., 1.]
        );
        // Both metadata cipher layers are XOR streams. Flipping the known
        // plaintext WB bits changes only that field in the encrypted source.
        for (at, value) in [(0x28726, 397u16), (0x28732, 256u16), (0x2873e, 728u16)] {
            let mut malformed = source.clone();
            for (offset, byte) in value.to_le_bytes().into_iter().enumerate() {
                malformed[at + offset] ^= byte;
            }
            assert!(white_balance_f828(&malformed).is_err(), "zero WB at {at}");
        }
        let mut duplicate = source.clone();
        duplicate[0x28732 - 8] ^= 1; // Replace tag 0xd1 with duplicate 0xd0.
        assert!(white_balance_f828(&duplicate).is_err());
        let mut wrong_type = source.clone();
        wrong_type[0x28726 - 6] ^= 3 ^ 4; // SHORT becomes LONG.
        assert!(white_balance_f828(&wrong_type).is_err());
        let actual = decode_f828(&source, &|| false).unwrap();
        assert_eq!(actual.len() * 2, reference.len());
        for (i, (a, b)) in actual.iter().zip(reference.chunks_exact(2)).enumerate() {
            assert_eq!(*a, u16::from_le_bytes(b.try_into().unwrap()), "sample {i}");
        }
        assert!(decode_f828(&source[..862144 + 3360 * 2460 * 2 - 1], &|| false).is_err());
    }
}
