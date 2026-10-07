//! EPS source framing. Embedded TIFF/WMF previews are never the artwork.
use std::ops::Range;

#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum EpsInspectError {
    #[error("invalid EPS signature or header")]
    Header,
    #[error("invalid EPS binary section ranges")]
    Range,
    #[error("EPS source exceeds inspection limit")]
    Limit,
}

#[derive(Debug, Clone)]
pub struct EpsSource<'a> {
    pub postscript: &'a [u8],
    pub wmf_preview: Option<&'a [u8]>,
    pub tiff_preview: Option<&'a [u8]>,
}

/// Candidate framing only; full syntax and DOS section ranges are validated
/// by `inspect_eps_source` before execution.
pub(crate) fn has_magic(bytes: &[u8]) -> bool {
    bytes.starts_with(&[0xc5, 0xd0, 0xd3, 0xc6])
        || (bytes.starts_with(b"%!PS-Adobe-")
            && bytes
                .split(|b| matches!(b, b'\r' | b'\n'))
                .next()
                .is_some_and(|line| {
                    line.split(|b| b.is_ascii_whitespace())
                        .any(|word| word.starts_with(b"EPSF-"))
                }))
}

/// Locate the original PostScript without interpreting it or substituting a
/// preview. DOS EPS section offsets are unsigned little-endian absolute offsets.
/// Source memory belongs to the caller. No allocations or renderer are involved.
pub fn inspect_eps_source(bytes: &[u8], max_source_bytes: usize) -> Result<EpsSource<'_>, EpsInspectError> {
    use EpsInspectError as E;
    if bytes.len() > max_source_bytes {
        return Err(E::Limit);
    }
    let (postscript, wmf_preview, tiff_preview) = if bytes.starts_with(&[0xc5, 0xd0, 0xd3, 0xc6]) {
        if bytes.len() < 30 {
            return Err(E::Header);
        }
        let section = |at: usize| -> Result<Option<Range<usize>>, E> {
            let offset = u32::from_le_bytes(bytes[at..at + 4].try_into().unwrap()) as usize;
            let length = u32::from_le_bytes(bytes[at + 4..at + 8].try_into().unwrap()) as usize;
            if offset == 0 && length == 0 {
                return Ok(None);
            }
            let end = offset.checked_add(length).ok_or(E::Range)?;
            if offset < 30 || length == 0 || end > bytes.len() {
                return Err(E::Range);
            }
            Ok(Some(offset..end))
        };
        let ps = section(4)?.ok_or(E::Range)?;
        let wmf = section(12)?;
        let tiff = section(20)?;
        if wmf.is_some() && tiff.is_some() {
            return Err(E::Range);
        }
        for (a, b) in [
            (&Some(ps.clone()), &wmf),
            (&Some(ps.clone()), &tiff),
            (&wmf, &tiff),
        ] {
            if let (Some(a), Some(b)) = (a, b) {
                if a.start < b.end && b.start < a.end {
                    return Err(E::Range);
                }
            }
        }
        let checksum = u16::from_le_bytes(bytes[28..30].try_into().unwrap());
        if checksum != u16::MAX {
            let actual = bytes[..28].iter().fold(0u16, |v, b| v ^ u16::from(*b));
            if actual != checksum {
                return Err(E::Header);
            }
        }
        (&bytes[ps], wmf.map(|r| &bytes[r]), tiff.map(|r| &bytes[r]))
    } else {
        (bytes, None, None)
    };
    let end = postscript
        .iter()
        .take(256)
        .position(|b| matches!(b, b'\r' | b'\n'))
        .unwrap_or(postscript.len().min(256));
    if end > 255 {
        return Err(E::Header);
    }
    let line = std::str::from_utf8(&postscript[..end]).map_err(|_| E::Header)?;
    let mut words = line.split_ascii_whitespace();
    let ps = words.next().ok_or(E::Header)?;
    let eps = words.next().ok_or(E::Header)?;
    if !ps.starts_with("%!PS-Adobe-")
        || !eps.starts_with("EPSF-")
        || !matches!(&ps[11..], "1.0" | "2.0" | "3.0")
        || !matches!(&eps[5..], "1.0" | "1.2" | "2.0" | "3.0")
        || words.next().is_some()
    {
        return Err(E::Header);
    }
    Ok(EpsSource {
        postscript,
        wmf_preview,
        tiff_preview,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn ascii_and_dos_sources_borrow_original_artwork_and_isolate_previews() {
        let ps = b"%!PS-Adobe-3.0 EPSF-3.0\n%%BoundingBox: -2 -3 10 20\n0 0 moveto\n";
        let view = inspect_eps_source(ps, ps.len()).unwrap();
        assert_eq!(view.postscript.as_ptr(), ps.as_ptr());
        assert!(view.tiff_preview.is_none());
        let mut binary = vec![0; 30];
        binary[..4].copy_from_slice(&[0xc5, 0xd0, 0xd3, 0xc6]);
        for (at, value) in [(4, 30), (8, ps.len() as u32), (20, 30 + ps.len() as u32), (24, 4)] {
            binary[at..at + 4].copy_from_slice(&value.to_le_bytes());
        }
        binary[28..30].copy_from_slice(&u16::MAX.to_le_bytes());
        binary.extend_from_slice(ps);
        binary.extend_from_slice(b"TIFF");
        let view = inspect_eps_source(&binary, binary.len()).unwrap();
        assert_eq!(view.postscript, ps);
        assert_eq!(view.tiff_preview, Some(&b"TIFF"[..]));
        assert_eq!(view.postscript.as_ptr(), binary[30..].as_ptr());
        let checksum = binary[..28].iter().fold(0u16, |v, b| v ^ u16::from(*b));
        binary[28..30].copy_from_slice(&checksum.to_le_bytes());
        assert!(inspect_eps_source(&binary, binary.len()).is_ok());
        binary[28] ^= 1;
        assert!(matches!(
            inspect_eps_source(&binary, binary.len()),
            Err(EpsInspectError::Header)
        ));
        binary[28..30].copy_from_slice(&u16::MAX.to_le_bytes());
        assert!(matches!(
            inspect_eps_source(&binary, binary.len() - 1),
            Err(EpsInspectError::Limit)
        ));
        binary[20..24].copy_from_slice(&30u32.to_le_bytes());
        assert!(matches!(
            inspect_eps_source(&binary, binary.len()),
            Err(EpsInspectError::Range)
        ));
        for input in [
            b"%!PS-Adobe-3.0\n".as_slice(),
            b"%!PS-Adobe-3.0 EPSF-9.0\n",
            b"II*\0preview",
        ] {
            assert!(inspect_eps_source(input, 1024).is_err());
        }
    }
}

impl<'a> EpsSource<'a> {
    /// Tokens borrow original PostScript; string bytes are not yet decoded.
    /// Count/length/string-nesting caps and cancellation bound lexical processing.
    pub fn tokens<F: FnMut() -> bool>(
        &self,
        max_tokens: usize,
        max_token_bytes: usize,
        cancelled: F,
    ) -> crate::EpsTokens<'a, F> {
        crate::EpsTokens::new(self.postscript, max_tokens, max_token_bytes, cancelled)
    }
}
