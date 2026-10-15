//! Allocation-free outer-document DSC bounding box inspection.
use crate::{EpsNumber, EpsSource, parse_eps_number};

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct EpsBounds {
    pub minimum: [f64; 2],
    pub maximum: [f64; 2],
}
#[derive(Debug, thiserror::Error)]
pub enum EpsBoundsError {
    #[error("missing EPS outer bounding box")]
    Missing,
    #[error("invalid or conflicting EPS DSC bounding boxes")]
    Syntax,
    #[error("EPS DSC line/nesting limit exceeded")]
    Limit,
    #[error("binary/data DSC sections require a stream-aware reader")]
    Data,
    #[error("EPS bounding box inspection cancelled")]
    Cancelled,
}
fn trim(mut bytes: &[u8]) -> &[u8] {
    while bytes.first().is_some_and(|b| matches!(b, 0 | 9 | 12 | 32)) {
        bytes = &bytes[1..];
    }
    while bytes.last().is_some_and(|b| matches!(b, 0 | 9 | 12 | 32)) {
        bytes = &bytes[..bytes.len() - 1];
    }
    bytes
}
fn parse<F: FnMut() -> bool>(
    bytes: &[u8],
    hires: bool,
    cancelled: &mut F,
) -> Result<EpsBounds, EpsBoundsError> {
    let mut words = bytes
        .split(|b| matches!(b, 0 | 9 | 12 | 32))
        .filter(|b| !b.is_empty());
    let mut values = [0.; 4];
    for value in &mut values {
        let word = words.next().ok_or(EpsBoundsError::Syntax)?;
        let number = parse_eps_number(word, 64, &mut *cancelled)
            .map_err(|error| {
                if matches!(error, crate::EpsNumberError::Cancelled) {
                    EpsBoundsError::Cancelled
                } else {
                    EpsBoundsError::Syntax
                }
            })?
            .ok_or(EpsBoundsError::Syntax)?;
        // DSC fields are decimal, not PostScript radix notation.
        if word.contains(&b'#') {
            return Err(EpsBoundsError::Syntax);
        }
        *value = match number {
            EpsNumber::Integer(n) => f64::from(n),
            // Metadata is not a VM real object: preserve decimal precision
            // rather than widening the VM's rounded f32 value.
            EpsNumber::Real(_) if hires => std::str::from_utf8(word)
                .map_err(|_| EpsBoundsError::Syntax)?
                .parse::<f64>()
                .map_err(|_| EpsBoundsError::Syntax)?,
            _ => return Err(EpsBoundsError::Syntax),
        };
    }
    if words.next().is_some()
        || !values.iter().all(|v| v.is_finite())
        || values[0] >= values[2]
        || values[1] >= values[3]
    {
        return Err(EpsBoundsError::Syntax);
    }
    Ok(EpsBounds {
        minimum: [values[0], values[1]],
        maximum: [values[2], values[3]],
    })
}
/// Inspects only outer header/trailer comments. Embedded documents cannot
/// replace the page bounds. HiResBoundingBox takes precedence when present.
/// Data blocks are explicitly refused until the source reader can skip them.
pub fn inspect_eps_bounds<F: FnMut() -> bool>(
    source: &EpsSource<'_>,
    mut cancelled: F,
) -> Result<EpsBounds, EpsBoundsError> {
    let mut bytes = source.postscript;
    let mut depth = 0usize;
    let mut header = true;
    let mut trailer = false;
    let mut bbox = None;
    let mut hires = None;
    let mut deferred = [false; 2];
    while !bytes.is_empty() {
        if cancelled() {
            return Err(EpsBoundsError::Cancelled);
        }
        let mut end = 0;
        while end < bytes.len() && !matches!(bytes[end], 10 | 13) {
            if end >= 65536 {
                return Err(EpsBoundsError::Limit);
            }
            if end % 4096 == 0 && cancelled() {
                return Err(EpsBoundsError::Cancelled);
            }
            end += 1;
        }
        let line = trim(&bytes[..end]);
        bytes = &bytes[end..];
        if bytes.first() == Some(&13) {
            bytes = &bytes[1..];
        }
        if bytes.first() == Some(&10) {
            bytes = &bytes[1..];
        }
        if line.starts_with(b"%%BeginBinary:") || line.starts_with(b"%%BeginData:") {
            return Err(EpsBoundsError::Data);
        }
        if line.starts_with(b"%%BeginDocument:") {
            depth += 1;
            if depth > 128 {
                return Err(EpsBoundsError::Limit);
            }
            continue;
        }
        if line == b"%%EndDocument" {
            depth = depth.checked_sub(1).ok_or(EpsBoundsError::Syntax)?;
            continue;
        }
        if depth != 0 {
            continue;
        }
        if line == b"%%Trailer" {
            trailer = true;
            header = false;
            continue;
        }
        if line == b"%%EndComments" {
            header = false;
            continue;
        }
        if !line.is_empty() && !line.starts_with(b"%") {
            header = false;
        }
        if !(header || trailer) {
            continue;
        }
        let (index, payload) = if let Some(p) = line.strip_prefix(b"%%BoundingBox:") {
            (0, trim(p))
        } else if let Some(p) = line.strip_prefix(b"%%HiResBoundingBox:") {
            (1, trim(p))
        } else {
            continue;
        };
        let slot = if index == 0 { &mut bbox } else { &mut hires };
        if payload == b"(atend)" {
            if !header || slot.is_some() {
                return Err(EpsBoundsError::Syntax);
            }
            deferred[index] = true;
            continue;
        }
        if deferred[index] && !trailer {
            return Err(EpsBoundsError::Syntax);
        }
        let bounds = parse(payload, index == 1, &mut cancelled)?;
        if slot.is_some_and(|existing| existing != bounds) {
            return Err(EpsBoundsError::Syntax);
        }
        *slot = Some(bounds);
    }
    if cancelled() {
        return Err(EpsBoundsError::Cancelled);
    }
    if depth != 0 || (deferred[0] && bbox.is_none()) || (deferred[1] && hires.is_none()) {
        return Err(EpsBoundsError::Syntax);
    }
    hires.or(bbox).ok_or(EpsBoundsError::Missing)
}
#[cfg(test)]
mod tests {
    use super::*;
    fn source(bytes: &[u8]) -> EpsSource<'_> {
        EpsSource {
            postscript: bytes,
            wmf_preview: None,
            tiff_preview: None,
        }
    }
    #[test]
    fn outer_header_trailer_and_hires_precedence() {
        let decimal =
            inspect_eps_bounds(&source(b"%%HiResBoundingBox: 0.1 0.2 10.3 20.4\n"), || false).unwrap();
        assert_eq!(decimal.minimum, [0.1, 0.2]);
        assert_eq!(decimal.maximum, [10.3, 20.4]);
        let code=b"%!PS-Adobe-3.0 EPSF-3.0\r\n%%BoundingBox: (atend)\n%%HiResBoundingBox: -1.5 2.25 10.5 20.75\n%%EndComments\n%%BeginDocument: nested.eps\n%%BoundingBox: 0 0 999 999\n%%EndDocument\n0 0 moveto\n%%BoundingBox: 0 0 500 500\n%%Trailer\n%%BoundingBox: -2 2 11 21\n";
        assert_eq!(
            inspect_eps_bounds(&source(code), || false).unwrap(),
            EpsBounds {
                minimum: [-1.5, 2.25],
                maximum: [10.5, 20.75]
            }
        );
    }
    #[test]
    fn refuses_ambiguous_unsupported_missing_and_cancelled_sources() {
        for code in [
            b"%%BoundingBox: 0 0 0 1\n".as_slice(),
            b"%%BoundingBox: 0 0 1 1\n%%BoundingBox: 0 0 2 2\n",
            b"%%BoundingBox: (atend)\n",
            b"%%BoundingBox: 0.5 0 1 1\n",
            b"%%EndDocument\n",
        ] {
            assert!(matches!(
                inspect_eps_bounds(&source(code), || false),
                Err(EpsBoundsError::Syntax)
            ));
        }
        assert!(matches!(
            inspect_eps_bounds(&source(b"%%BeginData: 20 Binary Bytes\n"), || false),
            Err(EpsBoundsError::Data)
        ));
        assert!(matches!(
            inspect_eps_bounds(&source(b"0 0 moveto\n%%BoundingBox: 0 0 1 1\n"), || false),
            Err(EpsBoundsError::Missing)
        ));
        assert!(matches!(
            inspect_eps_bounds(&source(b"%%BoundingBox: 0 0 1 1\n"), || true),
            Err(EpsBoundsError::Cancelled)
        ));
    }
}
