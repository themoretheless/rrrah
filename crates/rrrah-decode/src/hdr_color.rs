//! Explicit Radiance RGBE primaries. Missing or unfamiliar primaries remain
//! unknown; encoded samples and exposure metadata are not rescaled here.
use crate::RasterDecodeError;
use rrrah_core::RasterColorSpace;

const REC709: [f64; 8] = [0.64, 0.33, 0.30, 0.60, 0.15, 0.06, 0.3127, 0.3290];

pub(crate) fn declaration(bytes: &[u8]) -> Result<Option<RasterColorSpace>, RasterDecodeError> {
    if !bytes.starts_with(b"#?RADIANCE\n") && !bytes.starts_with(b"#?RGBE\n") {
        return Ok(None);
    }
    let invalid = |reason| RasterDecodeError::InvalidHdrColor(reason);
    let end = bytes
        .iter()
        .take(1024 * 1024)
        .enumerate()
        .find_map(|(i, _)| (bytes.get(i..i + 2) == Some(b"\n\n")).then_some(i))
        .ok_or_else(|| invalid("missing or oversized header"))?;
    let mut primaries = None;
    let mut format = None;
    for line in bytes[..end].split(|b| *b == b'\n').skip(1) {
        // Comments and command history are opaque bytes. Only color fields
        // require text decoding; legacy producers may put tabs around keys.
        let Some(eq) = line.iter().position(|b| *b == b'=') else {
            continue;
        };
        let key = line[..eq].trim_ascii();
        if key != b"FORMAT" && key != b"PRIMARIES" {
            continue;
        }
        let value = std::str::from_utf8(&line[eq + 1..]).map_err(|_| invalid("non-UTF8 color field"))?;
        if key == b"FORMAT" {
            if format.replace(value.trim()).is_some() {
                return Err(invalid("duplicate format"));
            }
        }
        if key == b"PRIMARIES" {
            if primaries.is_some() {
                return Err(invalid("duplicate primaries"));
            }
            let mut values = [0.0f64; 8];
            let mut words = value.split_whitespace();
            for v in &mut values {
                *v = words
                    .next()
                    .ok_or_else(|| invalid("primaries length"))?
                    .parse()
                    .map_err(|_| invalid("primaries number"))?;
                if !v.is_finite() {
                    return Err(invalid("non-finite primaries"));
                }
            }
            if words.next().is_some() {
                return Err(invalid("primaries length"));
            }
            for xy in values.chunks_exact(2) {
                if xy[0] < 0.0 || xy[1] <= 0.0 || xy[0] + xy[1] > 1.0 {
                    return Err(invalid("invalid chromaticity"));
                }
            }
            primaries = Some(values);
        }
    }
    // XYZE carries XYZ, not RGB. Never label it sRGB from a PRIMARIES line.
    Ok(Some(
        if format == Some("32-bit_rle_rgbe")
            && primaries.is_some_and(|p| p.iter().zip(REC709).all(|(a, b)| (a - b).abs() <= 1e-7))
        {
            RasterColorSpace::LinearSrgb
        } else {
            RasterColorSpace::LinearRgbUnspecified
        },
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    fn header(extra: &str) -> Vec<u8> {
        format!("#?RADIANCE\nFORMAT=32-bit_rle_rgbe\n{extra}\n\n-Y 1 +X 1\n").into_bytes()
    }
    const TAG: &str = "PRIMARIES= .64 .33 .30 .60 .15 .06 .3127 .3290";
    #[test]
    fn legacy_keys_and_opaque_history_do_not_erase_color() {
        let mut bytes = header(&TAG.replace("PRIMARIES=", "\tPRIMARIES\t="));
        let at = bytes.iter().position(|b| *b == b'\n').unwrap() + 1;
        bytes.splice(at..at, b"# producer comment \xff\n".iter().copied());
        assert_eq!(declaration(&bytes).unwrap(), Some(RasterColorSpace::LinearSrgb));
        let bad = header(TAG)
            .into_iter()
            .map(|b| if b == b'.' { 0xff } else { b })
            .collect::<Vec<_>>();
        assert!(declaration(&bad).is_err());
        assert!(declaration(b"#?RADIANCE\nFORMAT=32-bit_rle_rgbe").is_err());
    }
    #[test]
    fn declared_hdr_preserves_samples_and_managed_ownership() {
        let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/raster");
        let original = crate::decode_raster(&crate::DecodeRequest::new(root.join("rle-hdr.hdr"))).unwrap();
        let budget = rrrah_core::MemoryBudget::new(1024 * 1024);
        let mut request = crate::DecodeRequest::new(root.join("rle-hdr-rec709-declared.hdr"));
        request.memory_budget = Some(budget.clone());
        let declared = crate::decode_raster(&request).unwrap();
        assert_eq!(declared.color_space(), &RasterColorSpace::LinearSrgb);
        let prepared = crate::prepare_raster_for_display_with_budget(&declared, Some(&budget)).unwrap();
        let rrrah_core::RasterPixels::Rgba32Float(expected) = original.pixels() else {
            panic!()
        };
        let rrrah_core::RasterPixels::Rgba32Float(actual) = prepared.pixels() else {
            panic!()
        };
        assert!(actual.iter().any(|v| *v > 1.0));
        assert_eq!(
            expected.iter().map(|v| v.to_bits()).collect::<Vec<_>>(),
            actual.iter().map(|v| v.to_bits()).collect::<Vec<_>>()
        );
        let rrrah_core::RasterPixels::Rgba32Float(native) = declared.pixels() else {
            panic!()
        };
        assert!(native.ptr_eq(actual));
        assert!(budget.used() > 0);
        drop(declared);
        drop(prepared);
        assert_eq!(budget.used(), 0);
    }
    #[test]
    fn declarations_refuse_unknown_and_malformed_color() {
        assert_eq!(
            declaration(&header(TAG)).unwrap(),
            Some(RasterColorSpace::LinearSrgb)
        );
        for h in [header(""), header(&TAG.replace(".30", ".29"))] {
            assert_eq!(
                declaration(&h).unwrap(),
                Some(RasterColorSpace::LinearRgbUnspecified)
            );
        }
        for tag in [
            "PRIMARIES= 1 2",
            "PRIMARIES= NaN .33 .30 .60 .15 .06 .3127 .3290",
            "PRIMARIES= .64 0 .30 .60 .15 .06 .3127 .3290",
            "PRIMARIES= .64 .33 .30 .60 .15 .06 .3127 .3290 1",
        ] {
            assert!(declaration(&header(tag)).is_err());
        }
        assert!(declaration(&header(&format!("{TAG}\n{TAG}"))).is_err());
        assert_eq!(
            declaration(
                &String::from_utf8(header(TAG))
                    .unwrap()
                    .replace("rgbe", "xyze")
                    .into_bytes()
            )
            .unwrap(),
            Some(RasterColorSpace::LinearRgbUnspecified)
        );
    }
}
