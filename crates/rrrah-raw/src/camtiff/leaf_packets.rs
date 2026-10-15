//! Bounded native Leaf PKTS metadata for the qualified Aptus 22 MOS subset.
use super::camera_error;
use crate::DecodeError;

#[derive(Debug, PartialEq)]
pub(super) struct LeafMetadata {
    pub white_balance: [f32; 4],
    pub rotation_degrees: u16,
}
fn error(message: &str) -> DecodeError {
    camera_error("MOS", message)
}
fn numbers<const N: usize>(bytes: &[u8]) -> Result<[u32; N], DecodeError> {
    if bytes.len() > 256 {
        return Err(error("numeric packet size limit"));
    }
    let bytes = bytes
        .strip_suffix(&[0])
        .ok_or_else(|| error("unterminated packet value"))?;
    let text = std::str::from_utf8(bytes).map_err(|_| error("non-ASCII packet value"))?;
    let mut values = text.split_ascii_whitespace();
    let mut result = [0; N];
    for value in &mut result {
        *value = values
            .next()
            .ok_or_else(|| error("missing packet number"))?
            .parse()
            .map_err(|_| error("invalid packet number"))?;
    }
    if values.next().is_some() {
        return Err(error("extra packet number"));
    }
    Ok(result)
}
fn walk<'a>(
    bytes: &'a [u8],
    depth: usize,
    count: &mut usize,
    fields: &mut [Option<&'a [u8]>; 6],
) -> Result<(), DecodeError> {
    if depth > 16 {
        return Err(error("packet nesting limit"));
    }
    let mut position = 0usize;
    while position < bytes.len() {
        // Native nested packet sequences have one terminal NUL inside their length.
        if depth > 0 && bytes.get(position..) == Some(&[0][..]) {
            return Ok(());
        }
        *count += 1;
        if *count > 4096 {
            return Err(error("packet count limit"));
        }
        let header = bytes
            .get(position..position.checked_add(52).ok_or(DecodeError::DimensionOverflow)?)
            .ok_or_else(|| error("truncated packet header"))?;
        if &header[..8] != b"PKTS\0\0\0\x01" {
            return Err(error("invalid packet marker/version"));
        }
        let name_end = header[8..48]
            .iter()
            .position(|b| *b == 0)
            .ok_or_else(|| error("unterminated packet name"))?;
        if header[8 + name_end..48].iter().any(|b| *b != 0) {
            return Err(error("invalid packet name padding"));
        }
        let name = &header[8..8 + name_end];
        let length = u32::from_be_bytes(header[48..52].try_into().unwrap()) as usize;
        let start = position + 52;
        let end = start.checked_add(length).ok_or(DecodeError::DimensionOverflow)?;
        let payload = bytes
            .get(start..end)
            .ok_or_else(|| error("packet escapes parent"))?;
        let names: [&[u8]; 6] = [
            b"ShootObj_back_type",
            b"CaptProf_number_of_planes",
            b"CaptProf_raw_data_rotation",
            b"ImgProf_rotation_angle",
            b"CaptProf_mosaic_pattern",
            b"NeutObj_neutrals",
        ];
        if let Some(index) = names.iter().position(|candidate| *candidate == name) {
            if fields[index].replace(payload).is_some() {
                return Err(error("duplicate calibration packet"));
            }
        }
        if payload.starts_with(b"PKTS") {
            walk(payload, depth + 1, count, fields)?;
        }
        position = end;
    }
    Ok(())
}
pub(super) fn parse(bytes: &[u8]) -> Result<LeafMetadata, DecodeError> {
    if bytes.len() > 8 * 1024 * 1024 {
        return Err(error("metadata size limit"));
    }
    let mut fields = [None; 6];
    walk(bytes, 0, &mut 0, &mut fields)?;
    let field = |index: usize| fields[index].ok_or_else(|| error("missing calibration packet"));
    if numbers::<1>(field(0)?)? != [12]
        || numbers::<1>(field(1)?)? != [1]
        || numbers::<1>(field(2)?)? != [0]
        || numbers::<1>(field(3)?)? != [90]
        || numbers::<4>(field(4)?)? != [1, 3, 2, 0]
    {
        return Err(error("unqualified Leaf profile or CFA/rotation"));
    }
    let neutral = numbers::<4>(field(5)?)?;
    if neutral.iter().any(|n| *n == 0 || *n > 65535) {
        return Err(error("invalid native white balance"));
    }
    let coefficients = [
        neutral[0] as f32 / neutral[1] as f32,
        neutral[0] as f32 / neutral[2] as f32,
        neutral[0] as f32 / neutral[3] as f32,
    ];
    Ok(LeafMetadata {
        white_balance: [
            coefficients[0] / coefficients[1],
            1.0,
            coefficients[2] / coefficients[1],
            1.0,
        ],
        rotation_degrees: 90,
    })
}
#[cfg(test)]
mod tests {
    use super::*;
    fn packet(name: &str, payload: &[u8]) -> Vec<u8> {
        let mut out = b"PKTS\0\0\0\x01".to_vec();
        let mut field = [0; 40];
        field[..name.len()].copy_from_slice(name.as_bytes());
        out.extend(field);
        out.extend((payload.len() as u32).to_be_bytes());
        out.extend(payload);
        out
    }
    fn fixture_with_neutral(neutral: &str) -> Vec<u8> {
        let fields = [
            ("ShootObj_back_type", "12\0"),
            ("CaptProf_number_of_planes", "1\0"),
            ("CaptProf_raw_data_rotation", "0\0"),
            ("ImgProf_rotation_angle", "90\0"),
            ("CaptProf_mosaic_pattern", "1\n3\n2\n0\0"),
            ("NeutObj_neutrals", neutral),
        ];
        let bytes: Vec<u8> = fields
            .into_iter()
            .flat_map(|(k, v)| packet(k, v.as_bytes()))
            .collect();
        packet("camera_profile", &bytes)
    }
    #[test]
    fn native_wb_and_packet_boundaries() {
        let bytes = fixture_with_neutral("3270\n2703\n3270\n2232\0");
        let metadata = parse(&bytes).unwrap();
        assert_eq!(
            metadata.white_balance,
            [3270.0 / 2703.0, 1.0, 3270.0 / 2232.0, 1.0]
        );
        for length in 0..bytes.len() {
            assert!(parse(&bytes[..length]).is_err(), "prefix {length}");
        }
        let mut duplicate = bytes.clone();
        duplicate.extend(packet("NeutObj_neutrals", b"1 1 1 1\0"));
        assert!(parse(&duplicate).is_err());
        let mut nested = bytes;
        for _ in 0..18 {
            nested = packet("nested", &nested);
        }
        assert!(parse(&nested).is_err());
        for invalid in ["0 1 1 1\0", "1 1 1 65536\0", "1 1 1\0", "1 1 1 1 1\0", "1 1 1 1"] {
            assert!(parse(&fixture_with_neutral(invalid)).is_err());
        }
    }
    #[test]
    #[ignore = "requires pinned Leaf Aptus 22 MOS"]
    fn real_native_packets_match_independent_wb() {
        let bytes = std::fs::read(std::env::var("RRRAH_MOS_SOURCE").unwrap()).unwrap();
        let metadata = parse(&bytes[4968..531582]).unwrap();
        assert!((metadata.white_balance[0] - 1.209766984).abs() < 1e-6);
        assert!((metadata.white_balance[2] - 1.465053797).abs() < 1e-6);
        for (offset, replacement) in [
            (8035, b"0000".as_slice()),
            (7418, b"99".as_slice()),
            (4968 + 48, &[0xff, 0xff, 0xff, 0xff]),
        ] {
            let mut invalid = bytes[4968..531582].to_vec();
            let start = offset - 4968;
            invalid[start..start + replacement.len()].copy_from_slice(replacement);
            assert!(parse(&invalid).is_err(), "source mutation {offset}");
        }
    }
}
