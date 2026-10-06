//! Attached scalar NRRD arrays, retaining header fields and raw sample units.
use crate::{
    DecodeRequest,
    bounded_io::read_bounded,
    raster::{MAX_RASTER_BYTES, RasterDecodeError},
};
use rrrah_core::{DecodedRaster, RasterColorSpace, RasterPixels};
use std::{collections::BTreeMap, io::Read, sync::Arc};
fn bad(s: &'static str) -> RasterDecodeError {
    RasterDecodeError::InvalidNrrd(s)
}
pub(crate) fn has_magic(b: &[u8]) -> bool {
    b.starts_with(b"NRRD")
}
#[derive(Debug, Clone)]
pub struct NrrdImage {
    pub raster: DecodedRaster,
    /// Original field descriptors; identifiers are case-insensitive canonical keys.
    pub fields: BTreeMap<String, String>,
    pub key_values: BTreeMap<String, String>,
}
impl NrrdImage {
    /// Explicit linear grayscale window, without changing stored scientific data.
    pub fn windowed(&self, minimum: f32, maximum: f32) -> Result<DecodedRaster, RasterDecodeError> {
        crate::scientific::windowed(&self.raster, minimum, maximum, bad)
    }
}
pub fn decode_nrrd(request: &DecodeRequest) -> Result<NrrdImage, RasterDecodeError> {
    decode(&read_bounded(request)?, request)
}
fn unescape(value: &str) -> Result<String, RasterDecodeError> {
    let mut out = String::new();
    let mut chars = value.chars();
    while let Some(c) = chars.next() {
        if c != '\\' {
            out.push(c);
        } else {
            out.push(match chars.next() {
                Some('n') => '\n',
                Some('\\') => '\\',
                _ => return Err(bad("invalid key-value escape")),
            });
        }
    }
    Ok(out)
}
pub(crate) fn decode(b: &[u8], request: &DecodeRequest) -> Result<NrrdImage, RasterDecodeError> {
    request.check_cancelled()?;
    let mut at = 0;
    let mut fields = BTreeMap::new();
    let mut key_values = BTreeMap::new();
    let mut version = 0;
    loop {
        let limit = b.len().min(1024 * 1024);
        if at >= limit {
            return Err(bad("missing or oversized header"));
        }
        let n = b[at..limit]
            .iter()
            .position(|v| *v == b'\n')
            .ok_or_else(|| bad("missing header line terminator"))?;
        let raw = &b[at..at + n];
        at += n + 1;
        if !raw.is_ascii() {
            return Err(bad("header must be ASCII"));
        }
        let line = std::str::from_utf8(raw)
            .unwrap()
            .strip_suffix('\r')
            .unwrap_or(std::str::from_utf8(raw).unwrap());
        if version == 0 {
            version = match line {
                "NRRD0001" => 1,
                "NRRD0002" => 2,
                "NRRD0003" => 3,
                "NRRD0004" => 4,
                "NRRD0005" => 5,
                _ => return Err(bad("unsupported magic/version")),
            };
            continue;
        }
        if line.is_empty() {
            break;
        }
        if line.starts_with('#') {
            continue;
        }
        if let Some((key, value)) = line.split_once(":=") {
            if version < 2 || key.is_empty() {
                return Err(bad("invalid key-value pair"));
            }
            key_values.insert(key.to_owned(), unescape(value)?);
            continue;
        }
        let (key, value) = line.split_once(": ").ok_or_else(|| bad("invalid field syntax"))?;
        if key.is_empty() || key.starts_with(char::is_whitespace) {
            return Err(bad("invalid field identifier"));
        }
        let key = key.to_ascii_lowercase();
        if matches!(key.as_str(), "sizes" | "kinds") && !fields.contains_key("dimension") {
            return Err(bad("per-axis fields precede dimension"));
        }
        if fields.insert(key, value.trim_end().to_owned()).is_some() {
            return Err(bad("duplicate field"));
        }
    }
    let get = |key: &str| {
        fields
            .get(key)
            .map(String::as_str)
            .ok_or_else(|| bad("missing required field"))
    };
    if fields.contains_key("data file") || fields.contains_key("datafile") {
        return Err(bad("detached data is not yet supported"));
    }
    for key in ["byte skip", "byteskip", "line skip", "lineskip"] {
        if fields.get(key).is_some_and(|v| v != "0") {
            return Err(bad("nonzero data skipping is unsupported"));
        }
    }
    let dimension = get("dimension")?
        .parse::<usize>()
        .map_err(|_| bad("invalid dimension"))?;
    if !matches!(dimension, 2 | 3) {
        return Err(bad("only scalar planes/volumes are supported"));
    }
    let sizes: Vec<usize> = get("sizes")?
        .split_whitespace()
        .map(|v| v.parse().map_err(|_| bad("invalid size")))
        .collect::<Result<_, _>>()?;
    if sizes.len() != dimension || sizes.iter().any(|v| *v == 0 || *v > 65536) {
        return Err(bad("invalid axis sizes"));
    }
    if let Some(kinds) = fields.get("kinds") {
        let kinds: Vec<_> = kinds.split_whitespace().collect();
        if kinds.len() != dimension
            || kinds
                .iter()
                .any(|v| !matches!(v.to_ascii_lowercase().as_str(), "domain" | "space"))
        {
            return Err(bad("non-scalar or unsupported axis kinds"));
        }
    } else if dimension == 3 {
        return Err(bad("volume requires explicit scalar axis kinds"));
    }
    let typ = get("type")?.to_ascii_lowercase();
    let (kind, bytes) = match typ.as_str() {
        "uchar" | "unsigned char" | "uint8" | "uint8_t" => (0, 1),
        "ushort" | "unsigned short" | "unsigned short int" | "uint16" | "uint16_t" => (1, 2),
        "short" | "short int" | "signed short" | "signed short int" | "int16" | "int16_t" => (2, 2),
        "float" => (3, 4),
        _ => return Err(bad("unsupported scalar type")),
    };
    let encoding = get("encoding")?.to_ascii_lowercase();
    let be = if bytes > 1 && encoding != "ascii" && encoding != "text" && encoding != "txt" {
        match get("endian")?.to_ascii_lowercase().as_str() {
            "big" => true,
            "little" => false,
            _ => return Err(bad("invalid endian")),
        }
    } else {
        false
    };
    let count = sizes
        .iter()
        .try_fold(1usize, |a, v| a.checked_mul(*v))
        .ok_or(RasterDecodeError::OutputTooLarge)?;
    let expected = count
        .checked_mul(bytes)
        .ok_or(RasterDecodeError::OutputTooLarge)?;
    if expected as u64 > MAX_RASTER_BYTES || sizes[0] as u64 * sizes[1] as u64 * 16 > MAX_RASTER_BYTES {
        return Err(RasterDecodeError::OutputTooLarge);
    }
    let slices = if dimension == 3 { sizes[2] } else { 1 };
    if request.image_index >= slices {
        return Err(crate::DecodeError::UnsupportedImageIndex {
            index: request.image_index,
        }
        .into());
    }
    let mut inflated = Vec::new();
    let payload = match encoding.as_str() {
        "raw" => &b[at..],
        "gzip" | "gz" => {
            let mut decoder = flate2::bufread::GzDecoder::new(&b[at..]);
            let mut chunk = [0u8; 65536];
            loop {
                request.check_cancelled()?;
                let n = decoder.read(&mut chunk).map_err(|_| bad("invalid gzip stream"))?;
                if n == 0 {
                    break;
                }
                if inflated.len() + n > expected {
                    return Err(bad("gzip exceeds declared sample count"));
                }
                inflated.extend_from_slice(&chunk[..n]);
            }
            if !decoder.get_ref().is_empty() {
                return Err(bad("trailing gzip members/data"));
            }
            &inflated
        }
        "ascii" | "text" | "txt" => &b[at..],
        _ => return Err(bad("unsupported encoding")),
    };
    let plane = sizes[0] * sizes[1];
    let first = request.image_index * plane;
    let mut values = Vec::with_capacity(plane * 4);
    if matches!(encoding.as_str(), "ascii" | "text" | "txt") {
        let text = std::str::from_utf8(payload).map_err(|_| bad("invalid ASCII samples"))?;
        let mut n = 0;
        for token in text.split_whitespace() {
            if n % 4096 == 0 {
                request.check_cancelled()?;
            }
            let value = match kind {
                0 => token
                    .parse::<u8>()
                    .map(f32::from)
                    .map_err(|_| bad("invalid uint8 sample"))?,
                1 => token
                    .parse::<u16>()
                    .map(f32::from)
                    .map_err(|_| bad("invalid uint16 sample"))?,
                2 => token
                    .parse::<i16>()
                    .map(f32::from)
                    .map_err(|_| bad("invalid int16 sample"))?,
                _ => token.parse::<f32>().map_err(|_| bad("invalid float sample"))?,
            };
            if n >= count {
                return Err(bad("excess ASCII samples"));
            }
            if n >= first && n < first + plane {
                values.extend([value, value, value, 1.0]);
            }
            n += 1;
        }
        if n != count {
            return Err(bad("truncated ASCII samples"));
        }
    } else {
        if payload.len() != expected {
            return Err(bad("payload length differs from sample count"));
        }
        for (i, v) in payload[first * bytes..(first + plane) * bytes]
            .chunks_exact(bytes)
            .enumerate()
        {
            if i % 4096 == 0 {
                request.check_cancelled()?;
            }
            let value = match kind {
                0 => f32::from(v[0]),
                1 => {
                    let a = v.try_into().unwrap();
                    f32::from(if be {
                        u16::from_be_bytes(a)
                    } else {
                        u16::from_le_bytes(a)
                    })
                }
                2 => {
                    let a = v.try_into().unwrap();
                    f32::from(if be {
                        i16::from_be_bytes(a)
                    } else {
                        i16::from_le_bytes(a)
                    })
                }
                _ => {
                    let a = v.try_into().unwrap();
                    if be {
                        f32::from_be_bytes(a)
                    } else {
                        f32::from_le_bytes(a)
                    }
                }
            };
            values.extend([value, value, value, 1.0]);
        }
    }
    request.check_cancelled()?;
    let raster = DecodedRaster::new(
        sizes[0] as u32,
        sizes[1] as u32,
        RasterPixels::Rgba32Float(Arc::new(values).into()),
        RasterColorSpace::Unspecified,
    )?
    .with_image_selection(request.image_index, slices)?;
    Ok(NrrdImage {
        raster,
        fields,
        key_values,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn independent_scalar_volume_oracles() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/raster");
        for line in include_str!("../../../tests/fixtures/raster/nrrd-manifest.tsv")
            .lines()
            .filter(|s| !s.starts_with('#') && !s.is_empty())
        {
            let columns: Vec<_> = line.split('\t').collect();
            let mut request = DecodeRequest::new(root.join(columns[0]));
            request.image_index = columns[1].parse().unwrap();
            let image = decode_nrrd(&request).unwrap();
            assert_eq!((image.raster.width(), image.raster.height()), (5, 3));
            assert_eq!(image.raster.image_index(), request.image_index);
            assert_eq!(image.raster.image_count(), 2);
            assert_eq!(image.raster.color_space(), &RasterColorSpace::Unspecified);
            assert_eq!(image.fields["content"], "CC0 scalar volume");
            assert_eq!(image.key_values["measurement"], "raw units");
            let RasterPixels::Rgba32Float(values) = image.raster.pixels() else {
                panic!()
            };
            let oracle = std::fs::read(root.join(columns[4])).unwrap();
            for (value, expected) in values.iter().zip(oracle.chunks_exact(4)) {
                assert_eq!(
                    value.to_bits(),
                    u32::from_le_bytes(expected.try_into().unwrap()),
                    "{} slice {}",
                    columns[0],
                    columns[1]
                );
            }
        }
    }
    #[test]
    fn public_router_selects_volume_under_raw_suffix() {
        let path = std::env::temp_dir().join(format!("rrrah-nrrd-router-{}.cr3", std::process::id()));
        std::fs::write(
            &path,
            include_bytes!("../../../tests/fixtures/raster/nrrd-i16-raw-big.nrrd"),
        )
        .unwrap();
        let mut request = DecodeRequest::new(&path);
        request.image_index = 1;
        let result = crate::decode_image(&request);
        std::fs::remove_file(&path).unwrap();
        let crate::DecodedImage::Raster(raster) = result.unwrap() else {
            panic!()
        };
        assert_eq!(raster.image_index(), 1);
        assert_eq!(raster.image_count(), 2);
        assert!(crate::prepare_raster_for_display(&raster).is_err());
    }
    fn raw(header: &str, payload: &[u8]) -> Vec<u8> {
        let mut bytes = header.as_bytes().to_vec();
        bytes.extend_from_slice(payload);
        bytes
    }
    #[test]
    fn explicit_window_and_nonfinite_policy() {
        let request = DecodeRequest::new("synthetic.nrrd");
        let header = "NRRD0005\ntype: float\ndimension: 2\nsizes: 3 1\nencoding: raw\nendian: little\nnote:=a\\nb\\\\c\n\n";
        let payload: Vec<_> = [-1f32, 0., 2.].into_iter().flat_map(f32::to_le_bytes).collect();
        let image = decode(&raw(header, &payload), &request).unwrap();
        assert_eq!(image.key_values["note"], "a\nb\\c");
        let window = image.windowed(-1., 1.).unwrap();
        let RasterPixels::Rgba32Float(values) = window.pixels() else {
            panic!()
        };
        assert_eq!(&values[..], &[0., 0., 0., 1., 0.5, 0.5, 0.5, 1., 1., 1., 1., 1.]);
        assert!(image.windowed(1., 1.).is_err());
        let payload: Vec<_> = [f32::NAN, f32::INFINITY, f32::NEG_INFINITY]
            .into_iter()
            .flat_map(f32::to_le_bytes)
            .collect();
        let image = decode(&raw(header, &payload), &request).unwrap();
        assert!(image.windowed(0., 1.).is_err());
        let RasterPixels::Rgba32Float(values) = image.raster.pixels() else {
            panic!()
        };
        assert!(values[0].is_nan());
        assert_eq!(values[4], f32::INFINITY);
    }
    #[test]
    fn malformed_and_unsupported_contracts_are_rejected() {
        let header = "NRRD0005\ntype: uint8\ndimension: 2\nsizes: 2 1\nencoding: raw\n\n";
        let request = DecodeRequest::new("synthetic.nrrd");
        assert!(decode(&raw(header, &[0, 255]), &request).is_ok());
        assert!(decode(&raw(&header.replace('\n', "\r\n"), &[0, 255]), &request).is_ok());
        for changed in [
            header.replace("NRRD0005", "NRRD0000"),
            header.replace("sizes: 2 1", "sizes: 0 1"),
            header.replace("sizes: 2 1", "sizes: 2"),
            header.replace("type: uint8", "type: double"),
            header.replace("encoding: raw", "encoding: bzip2"),
            header.replace("encoding: raw", "encoding: raw\ntype: uint8"),
            header.replace("encoding: raw", "encoding: raw\ndata file: external.raw"),
            header.replace("encoding: raw", "encoding: raw\nbyte skip: -1"),
            header.replace("encoding: raw", "encoding: raw\nkinds: RGB-color domain"),
            header.replace("dimension: 2\nsizes: 2 1", "sizes: 2 1\ndimension: 2"),
        ] {
            assert!(decode(&raw(&changed, &[0, 255]), &request).is_err(), "{changed}");
        }
        for payload in [&[0][..], &[0, 255, 1][..]] {
            assert!(decode(&raw(header, payload), &request).is_err());
        }
        let ascii = header.replace("encoding: raw", "encoding: ascii");
        for payload in ["0", "0 255 1", "0 256", "0 -1"] {
            assert!(decode(&raw(&ascii, payload.as_bytes()), &request).is_err());
        }
        let gzip = include_bytes!("../../../tests/fixtures/raster/nrrd-u8-gzip-little.nrrd");
        let mut corrupt = gzip.to_vec();
        let last = corrupt.len() - 5;
        corrupt[last] ^= 1;
        assert!(decode(&corrupt, &request).is_err());
        let mut trailing = gzip.to_vec();
        trailing.push(0);
        assert!(decode(&trailing, &request).is_err());
        assert!(decode(&gzip[..gzip.len() - 1], &request).is_err());
        let mut selected = request;
        selected.image_index = 1;
        assert!(decode(&raw(header, &[0, 255]), &selected).is_err());
    }
}
