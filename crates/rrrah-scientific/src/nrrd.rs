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
// flate2 1.1.10 bounds each zero-terminated gzip field at 65535 bytes;
// incremental u8 Vec growth fits a conservative 65536-byte capacity.
fn gzip_header_credit(source: &[u8]) -> Result<u64, RasterDecodeError> {
    let header = source.get(..10).ok_or_else(|| bad("truncated gzip header"))?;
    if header[..3] != [0x1f, 0x8b, 8] || header[3] & 0xe0 != 0 {
        return Err(bad("invalid gzip header"));
    }
    let flags = header[3];
    let mut bytes = 65536 * (u64::from(flags & 8 != 0) + u64::from(flags & 16 != 0));
    if flags & 4 != 0 {
        let size = source
            .get(10..12)
            .ok_or_else(|| bad("truncated gzip extra length"))?;
        let len = usize::from(u16::from_le_bytes(size.try_into().unwrap()));
        source
            .get(12..12 + len)
            .ok_or_else(|| bad("truncated gzip extra data"))?;
        bytes += len as u64;
    }
    if flags & 2 != 0 {
        bytes += std::mem::size_of::<flate2::Crc>() as u64;
    }
    Ok(bytes)
}
fn buffer<T: Copy>(
    len: usize,
    value: T,
    request: &DecodeRequest,
) -> Result<rrrah_core::PixelBuffer<T>, RasterDecodeError> {
    Ok(match &request.memory_budget {
        Some(budget) => budget
            .try_buffer(len, value)
            .map_err(crate::DecodeError::Memory)?
            .freeze()
            .into(),
        None => Arc::new(vec![value; len]).into(),
    })
}
fn ascii_sample(token: &str, kind: u8) -> Result<f32, RasterDecodeError> {
    match kind {
        0 => token
            .parse::<u8>()
            .map(f32::from)
            .map_err(|_| bad("invalid uint8 sample")),
        1 => token
            .parse::<u16>()
            .map(f32::from)
            .map_err(|_| bad("invalid uint16 sample")),
        2 => token
            .parse::<i16>()
            .map(f32::from)
            .map_err(|_| bad("invalid int16 sample")),
        _ => token.parse::<f32>().map_err(|_| bad("invalid float sample")),
    }
}
fn spacing_aspect(
    fields: &BTreeMap<String, String>,
    dimension: usize,
) -> Result<Option<f32>, RasterDecodeError> {
    let Some(text) = fields.get("spacings") else {
        return Ok(None);
    };
    let mut values = [f64::NAN; 3];
    let mut count = 0;
    for token in text.split_whitespace() {
        if count >= dimension {
            return Err(bad("spacing axis count"));
        }
        let value = token.parse::<f64>().map_err(|_| bad("invalid spacing"))?;
        if value.is_infinite() || value == 0. {
            return Err(bad("invalid spacing"));
        }
        values[count] = value;
        count += 1;
    }
    if count != dimension {
        return Err(bad("spacing axis count"));
    }
    if fields.contains_key("space directions") || values[0].is_nan() || values[1].is_nan() {
        return Ok(None);
    }
    if let Some(units) = fields.get("units") {
        let units: Vec<_> = units.split_whitespace().collect();
        // Multiword/escaped units need a full units parser. Do not infer a
        // physical ratio across different or ambiguous coordinate units.
        if units.len() != dimension
            || units
                .iter()
                .any(|unit| unit.len() < 2 || !unit.starts_with('"') || !unit.ends_with('"'))
            || units[0] != units[1]
        {
            return Ok(None);
        }
    }
    let ratio = (values[0] / values[1]).abs() as f32;
    if !ratio.is_finite() || ratio <= 0. {
        return Err(bad("unrepresentable spacing ratio"));
    }
    Ok(Some(ratio))
}
pub fn has_magic(b: &[u8]) -> bool {
    b.starts_with(b"NRRD")
}
/// Shared original NRRD descriptors with retained metadata admission credit.
#[derive(Debug)]
pub struct NrrdFields {
    values: BTreeMap<String, String>,
    _credit: Option<Arc<rrrah_core::Reservation>>,
}
impl std::ops::Deref for NrrdFields {
    type Target = BTreeMap<String, String>;
    fn deref(&self) -> &Self::Target {
        &self.values
    }
}
pub(crate) fn header_credit(b: &[u8], request: &DecodeRequest) -> Result<u64, RasterDecodeError> {
    let limit = b.len().min(1024 * 1024);
    let mut at = 0;
    let mut first = true;
    let mut credit = 0u64;
    loop {
        request.check_cancelled()?;
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
        let raw = raw.strip_suffix(b"\r").unwrap_or(raw);
        if first {
            if !matches!(
                raw,
                b"NRRD0001" | b"NRRD0002" | b"NRRD0003" | b"NRRD0004" | b"NRRD0005"
            ) {
                return Err(bad("unsupported magic/version"));
            }
            first = false;
            continue;
        }
        if raw.is_empty() {
            return Ok(credit);
        }
        if raw.starts_with(b"#") {
            continue;
        }
        // Covers two String capacities plus conservative per-record BTreeMap
        // node capacity (including sparsely populated root/split nodes).
        credit += 2 * raw.len() as u64 + 1024;
    }
}
#[derive(Debug, Clone)]
pub struct NrrdImage {
    request: DecodeRequest,
    pub raster: DecodedRaster,
    /// Original field descriptors; identifiers are case-insensitive canonical keys.
    pub fields: Arc<NrrdFields>,
    pub key_values: Arc<NrrdFields>,
}
impl NrrdImage {
    /// Reuse decoded samples with an explicit display budget and cancellation.
    /// This overrides the original decode request for the new output only.
    pub fn windowed_with_request(
        &self,
        minimum: f32,
        maximum: f32,
        request: &DecodeRequest,
    ) -> Result<DecodedRaster, RasterDecodeError> {
        crate::scientific::windowed_with_request(&self.raster, minimum, maximum, bad, Some(request))
    }

    /// Explicit linear grayscale window, without changing stored scientific data.
    pub fn windowed(&self, minimum: f32, maximum: f32) -> Result<DecodedRaster, RasterDecodeError> {
        crate::scientific::windowed_with_request(&self.raster, minimum, maximum, bad, Some(&self.request))
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
pub fn decode(b: &[u8], request: &DecodeRequest) -> Result<NrrdImage, RasterDecodeError> {
    request.check_cancelled()?;
    let credit = header_credit(b, request)?;
    let metadata_credit = request
        .memory_budget
        .as_ref()
        .map(|budget| budget.try_reserve(credit).map(Arc::new))
        .transpose()
        .map_err(crate::DecodeError::Memory)?;
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
        if matches!(key.as_str(), "sizes" | "kinds" | "spacings") && !fields.contains_key("dimension") {
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
    let pixel_aspect = spacing_aspect(&fields, dimension)?;
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
    let plane = sizes[0] * sizes[1];
    let first = request.image_index * plane;
    let mut inflated = None;
    let payload = match encoding.as_str() {
        "raw" => &b[at..],
        "gzip" | "gz" => {
            let decoder_bytes = crate::DEFLATE_DECODER_CREDIT_BYTES + gzip_header_credit(&b[at..])?;
            let _decoder_credit = request
                .memory_budget
                .as_ref()
                .map(|budget| budget.try_reserve(decoder_bytes))
                .transpose()
                .map_err(crate::DecodeError::Memory)?;
            let mut selected = buffer(plane * bytes, 0u8, request)?;
            let output = selected.get_mut().unwrap();
            let begin = first * bytes;
            let end = begin + output.len();
            let mut consumed = 0usize;
            let mut decoder = flate2::bufread::GzDecoder::new(&b[at..]);
            let mut chunk = [0u8; 65536];
            loop {
                request.check_cancelled()?;
                let n = decoder.read(&mut chunk).map_err(|_| bad("invalid gzip stream"))?;
                if n == 0 {
                    break;
                }
                if consumed + n > expected {
                    return Err(bad("gzip exceeds declared sample count"));
                }
                let left = consumed.max(begin);
                let right = (consumed + n).min(end);
                if left < right {
                    output[left - begin..right - begin]
                        .copy_from_slice(&chunk[left - consumed..right - consumed]);
                }
                consumed += n;
            }
            if consumed != expected {
                return Err(bad("payload length differs from sample count"));
            }
            if !decoder.get_ref().is_empty() {
                return Err(bad("trailing gzip members/data"));
            }
            inflated = Some(selected);
            inflated.as_ref().unwrap().as_slice()
        }
        "ascii" | "text" | "txt" => &b[at..],
        _ => return Err(bad("unsupported encoding")),
    };
    let ascii = matches!(encoding.as_str(), "ascii" | "text" | "txt");
    if ascii {
        let text = std::str::from_utf8(payload).map_err(|_| bad("invalid ASCII samples"))?;
        let mut n = 0;
        for token in text.split_whitespace() {
            if n % 4096 == 0 {
                request.check_cancelled()?;
            }
            if n >= count {
                return Err(bad("excess ASCII samples"));
            }
            ascii_sample(token, kind)?;
            n += 1;
        }
        if n != count {
            return Err(bad("truncated ASCII samples"));
        }
    } else if payload.len()
        != if inflated.is_some() {
            plane * bytes
        } else {
            expected
        }
    {
        return Err(bad("payload length differs from sample count"));
    }
    let mut values = buffer(plane * 4, 0f32, request)?;
    let output = values.get_mut().unwrap();
    if ascii {
        let text = std::str::from_utf8(payload).map_err(|_| bad("invalid ASCII samples"))?;
        for (n, token) in text.split_whitespace().enumerate() {
            if n % 4096 == 0 {
                request.check_cancelled()?;
            }
            if n >= first + plane {
                break;
            }
            if n >= first {
                let value = ascii_sample(token, kind)?;
                let offset = (n - first) * 4;
                output[offset..offset + 4].copy_from_slice(&[value, value, value, 1.0]);
            }
        }
    } else {
        let begin = if inflated.is_some() { 0 } else { first * bytes };
        for (i, v) in payload[begin..begin + plane * bytes]
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
            output[i * 4..i * 4 + 4].copy_from_slice(&[value, value, value, 1.0]);
        }
    }
    request.check_cancelled()?;
    let raster = DecodedRaster::new(
        sizes[0] as u32,
        sizes[1] as u32,
        RasterPixels::Rgba32Float(values),
        RasterColorSpace::Unspecified,
    )?
    .with_image_selection(request.image_index, slices)?
    .with_pixel_aspect(pixel_aspect)?;
    Ok(NrrdImage {
        request: request.clone(),
        raster,
        fields: Arc::new(NrrdFields {
            values: fields,
            _credit: metadata_credit.clone(),
        }),
        key_values: Arc::new(NrrdFields {
            values: key_values,
            _credit: metadata_credit,
        }),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn cloned_images_share_retained_header_maps_without_copying_strings() {
        let mut source =
            b"NRRD0005\ntype: uchar\ndimension: 2\nsizes: 1 1\nencoding: raw\nlarge-key:=".to_vec();
        source.extend(std::iter::repeat_n(b'x', 65536));
        source.extend(b"\n\n\x7f");
        let metadata = header_credit(&source, &DecodeRequest::new("header")).unwrap();
        let budget = rrrah_core::MemoryBudget::new(metadata + 16);
        let mut request = DecodeRequest::new("metadata.nrrd");
        request.memory_budget = Some(budget.clone());
        let image = decode(&source, &request).unwrap();
        let shared = image.clone();
        assert!(Arc::ptr_eq(&image.fields, &shared.fields));
        assert!(Arc::ptr_eq(&image.key_values, &shared.key_values));
        let fields = image.fields.clone();
        let key_values = image.key_values.clone();
        let pointer = key_values["large-key"].as_ptr();
        drop(image);
        assert_eq!(budget.used(), metadata + 16);
        assert_eq!(shared.key_values["large-key"].as_ptr(), pointer);
        drop(shared);
        assert_eq!(budget.used(), metadata);
        assert_eq!(fields["encoding"], "raw");
        assert_eq!(key_values["large-key"].len(), 65536);
        assert_eq!(Arc::strong_count(&fields), 1);
        assert_eq!(Arc::strong_count(&key_values), 1);
        drop(fields);
        assert_eq!(budget.used(), metadata);
        drop(key_values);
        assert_eq!(budget.used(), 0);
    }
    #[test]
    fn gzip_optional_fields_crc_and_maximum_strings_are_admitted() {
        let original = include_bytes!("../../../tests/fixtures/raster/nrrd-u8-gzip-little.nrrd");
        let at = original.windows(2).position(|v| v == b"\n\n").unwrap() + 2;
        assert_eq!(original[at + 3], 0);
        let reference = decode(original, &DecodeRequest::new("reference.nrrd")).unwrap();
        let mut cases = 0;
        for fields in 0u8..8 {
            for crc in [false, true] {
                for length in [1usize, 65535] {
                    let mut source = original[..at + 10].to_vec();
                    source[at + 3] = (fields << 2) | (u8::from(crc) << 1);
                    if fields & 1 != 0 {
                        source.extend((length as u16).to_le_bytes());
                        source.resize(source.len() + length, 42);
                    }
                    for flag in [2, 4] {
                        if fields & flag != 0 {
                            source.resize(source.len() + length, b'x');
                            source.push(0);
                        }
                    }
                    if crc {
                        let mut checksum = flate2::Crc::new();
                        checksum.update(&source[at..]);
                        source.extend((checksum.sum() as u16).to_le_bytes());
                    }
                    source.extend(&original[at + 10..]);
                    let header_bytes = gzip_header_credit(&source[at..]).unwrap();
                    let metadata = header_credit(&source, &DecodeRequest::new("header")).unwrap();
                    let peak = metadata + crate::DEFLATE_DECODER_CREDIT_BYTES + header_bytes + 15;
                    let budget = rrrah_core::MemoryBudget::new(peak);
                    let mut request = DecodeRequest::new("optional.nrrd");
                    request.memory_budget = Some(budget.clone());
                    let image = decode(&source, &request).unwrap();
                    let (RasterPixels::Rgba32Float(actual), RasterPixels::Rgba32Float(expected)) =
                        (image.raster.pixels(), reference.raster.pixels())
                    else {
                        panic!()
                    };
                    assert!(
                        actual
                            .iter()
                            .map(|v| v.to_bits())
                            .eq(expected.iter().map(|v| v.to_bits()))
                    );
                    assert_eq!((budget.used(), budget.peak()), (240 + metadata, peak));
                    drop(image);
                    assert_eq!(budget.used(), 0);
                    let short = rrrah_core::MemoryBudget::new(peak - 1);
                    request.memory_budget = Some(short.clone());
                    assert!(matches!(
                        decode(&source, &request),
                        Err(RasterDecodeError::Source(crate::DecodeError::Memory(_)))
                    ));
                    assert_eq!(short.used(), 0);
                    if crc {
                        let header_end = source.len() - (original.len() - at - 10);
                        source[header_end - 1] ^= 1;
                        request.memory_budget = Some(budget.clone());
                        assert!(decode(&source, &request).is_err());
                        assert_eq!(budget.used(), 0);
                    }
                    cases += 1;
                }
            }
        }
        assert_eq!(cases, 32);
    }
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
            let source_bytes = std::fs::metadata(&request.path).unwrap().len();
            let scratch = if image.fields["encoding"] == "gzip" {
                15 * match image.fields["type"].as_str() {
                    "uint8" => 1,
                    "float" => 4,
                    _ => 2,
                }
            } else {
                0
            };
            let metadata = header_credit(&std::fs::read(&request.path).unwrap(), &request).unwrap();
            let exact = metadata
                + source_bytes
                + scratch
                + if scratch == 0 {
                    240
                } else {
                    crate::DEFLATE_DECODER_CREDIT_BYTES.max(240)
                };
            let budget = rrrah_core::MemoryBudget::new(exact);
            request.memory_budget = Some(budget.clone());
            let admitted = decode_nrrd(&request).unwrap();
            assert_eq!(budget.peak(), exact);
            assert_eq!(budget.used(), metadata + 240);
            drop(admitted);
            assert_eq!(budget.used(), 0);
            let short = rrrah_core::MemoryBudget::new(exact - 1);
            request.memory_budget = Some(short.clone());
            assert!(matches!(
                decode_nrrd(&request),
                Err(RasterDecodeError::Source(crate::DecodeError::Memory(_)))
            ));
            assert_eq!(short.used(), 0);
            let window_peak = exact.max(480);
            let window_budget = rrrah_core::MemoryBudget::new(window_peak);
            request.memory_budget = Some(window_budget.clone());
            let window = crate::ScalarWindow::new(-100., 100.).unwrap();
            let windowed = crate::decode_raster_with_window(&request, window).unwrap();
            assert_eq!(window_budget.peak(), window_peak);
            assert_eq!(window_budget.used(), 240);
            assert_eq!(windowed.color_space(), &RasterColorSpace::LinearSrgb);
            drop(windowed);
            assert_eq!(window_budget.used(), 0);
            let short_window = rrrah_core::MemoryBudget::new(window_peak - 1);
            request.memory_budget = Some(short_window.clone());
            assert!(matches!(
                crate::decode_raster_with_window(&request, window),
                Err(RasterDecodeError::Source(crate::DecodeError::Memory(_)))
            ));
            assert_eq!(short_window.used(), 0);
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
    fn spacing_ratio_survives_window_without_resampling() {
        let header = "NRRD0005\ntype: uint8\ndimension: 3\nsizes: 2 1 2\nkinds: domain domain domain\nencoding: raw\nspacings: 2 -1 nan\nunits: \"mm\" \"mm\" \"mm\"\n\n";
        let request = DecodeRequest::new("spacing.nrrd");
        let image = decode(&raw(header, &[0, 1, 2, 3]), &request).unwrap();
        assert_eq!(image.raster.pixel_aspect(), Some(2.));
        let window = image.windowed(0., 3.).unwrap();
        assert_eq!(window.pixel_aspect(), Some(2.));
        assert_eq!((window.width(), window.height()), (2, 1));
        for changed in [
            "0 1 1",
            "inf 1 1",
            "1 1 -inf",
            "1 2",
            "1 2 3 4",
            "broken 1 1",
            "1e300 1e-300 1",
        ] {
            let changed = header.replace("2 -1 nan", changed);
            assert!(matches!(
                decode(&raw(&changed, &[0, 1, 2, 3]), &request),
                Err(RasterDecodeError::InvalidNrrd(_))
            ));
        }
        for changed in [
            header.replace("2 -1 nan", "nan 1 1"),
            header.replace("\"mm\" \"mm\"", "\"mm\" \"s\""),
            header.replace(
                "encoding: raw",
                "encoding: raw\nspace directions: (2,0,0) (0,1,0) (0,0,1)",
            ),
        ] {
            assert_eq!(
                decode(&raw(&changed, &[0, 1, 2, 3]), &request)
                    .unwrap()
                    .raster
                    .pixel_aspect(),
                None
            );
        }
    }
    #[test]
    fn raw_and_ascii_corruption_precedes_output_admission() {
        let header = "NRRD0005\ntype: uint8\ndimension: 3\nsizes: 2 1 2\nkinds: domain domain domain\nencoding: raw\n\n";
        let metadata = header_credit(
            &raw(&header.replace("encoding: raw", "encoding: ascii"), &[]),
            &DecodeRequest::new("header"),
        )
        .unwrap();
        let budget = rrrah_core::MemoryBudget::new(128 + metadata);
        let pressure = budget.try_reserve(128).unwrap();
        let mut request = DecodeRequest::new("pressure.nrrd");
        request.memory_budget = Some(budget.clone());
        for index in 0..2 {
            request.image_index = index;
            for payload in [&[0, 1, 2][..], &[0, 1, 2, 3, 4][..]] {
                assert!(matches!(
                    decode(&raw(header, payload), &request),
                    Err(RasterDecodeError::InvalidNrrd(_))
                ));
            }
            let ascii = header.replace("encoding: raw", "encoding: ascii");
            for payload in ["0 1", "0 1 2 3 4", "0 1 broken 3", "0 1 256 3", "0 1 -1 3"] {
                assert!(
                    matches!(
                        decode(&raw(&ascii, payload.as_bytes()), &request),
                        Err(RasterDecodeError::InvalidNrrd(_))
                    ),
                    "index={index}, payload={payload}"
                );
            }
            assert!(matches!(
                decode(&raw(&ascii, b"0 1 2 3"), &request),
                Err(RasterDecodeError::Source(crate::DecodeError::Memory(_)))
            ));
            assert_eq!(budget.used(), 128);
        }
        drop(pressure);
        assert_eq!(budget.used(), 0);
    }
    #[test]
    fn gzip_volume_retains_only_selected_slice_and_checks_late_crc() {
        use std::io::Write;
        let header = "NRRD0005\ntype: ushort\ndimension: 3\nsizes: 3 2 128\nkinds: domain domain domain\nencoding: gzip\nendian: little\n\n";
        let samples: Vec<_> = (0..768u16).flat_map(u16::to_le_bytes).collect();
        let mut encoder = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
        encoder.write_all(&samples).unwrap();
        let compressed = encoder.finish().unwrap();
        let bytes = raw(header, &compressed);
        for index in [0, 63, 127] {
            // 12-byte selected integer slice plus 96-byte float RGBA output,
            // plus pinned backend credit, independently of full-volume size.
            let metadata = header_credit(&bytes, &DecodeRequest::new("header")).unwrap();
            let budget = rrrah_core::MemoryBudget::new(metadata + 12 + crate::DEFLATE_DECODER_CREDIT_BYTES);
            let mut request = DecodeRequest::new("volume.nrrd");
            request.image_index = index;
            request.memory_budget = Some(budget.clone());
            let image = decode(&bytes, &request).unwrap();
            let RasterPixels::Rgba32Float(values) = image.raster.pixels() else {
                panic!()
            };
            for i in 0..6 {
                assert_eq!(values[i * 4], (index * 6 + i) as f32);
            }
            assert_eq!(image.raster.image_count(), 128);
            assert_eq!(
                (budget.used(), budget.peak()),
                (96 + metadata, metadata + 12 + crate::DEFLATE_DECODER_CREDIT_BYTES)
            );
            drop(image);
            assert_eq!(budget.used(), 0);
            let mut corrupt = bytes.clone();
            let crc = corrupt.len() - 8;
            corrupt[crc] ^= 1;
            assert!(matches!(
                decode(&corrupt, &request),
                Err(RasterDecodeError::InvalidNrrd("invalid gzip stream"))
            ));
            assert_eq!(budget.used(), 0);
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
        let result = decode_nrrd(&request);
        std::fs::remove_file(&path).unwrap();
        let raster = result.unwrap().raster;
        assert_eq!(raster.image_index(), 1);
        assert_eq!(raster.image_count(), 2);
        assert_eq!(raster.color_space(), &RasterColorSpace::Unspecified);
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
