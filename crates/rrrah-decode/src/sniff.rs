//! Content-based RAW format detection from the first bytes of a file.
//!
//! The sniffer reads at most [`SNIFF_BYTES`] bytes and never panics on short
//! or truncated input. It distinguishes the strong container magics (CR3 ISO
//! BMFF, CR2, ORF, RW2, RAF) from the generic TIFF family; TIFF-family camera
//! formats (NEF, ARW, PEF) share the classic TIFF magic and are refined later
//! by their backend through the Make/Model tags.

use std::{fs::File, io::Read, path::Path};

/// Maximum number of header bytes examined by the sniffer.
pub(crate) const SNIFF_BYTES: usize = 256;

/// Coarse container classification from content alone.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SniffedFormat {
    /// ISO BMFF with an `ftyp` major brand of `crx ` (Canon CR3).
    Cr3,
    /// Little-endian TIFF with the `CR\x02\0` signature at offset 8.
    Cr2,
    /// Classic TIFF or `BigTIFF`, byte order either way. Camera formats that
    /// live in plain TIFF containers (DNG, NEF, ARW, PEF, generic TIFF) all
    /// land here and are refined by extension or backend metadata.
    TiffFamily,
    /// Olympus ORF: `IIRO`, `IIRS`, or the big-endian `MMOR` variant.
    Orf,
    /// Panasonic RW2: `IIU\0`.
    Rw2,
    /// Fujifilm RAF: `FUJIFILMCCD-RAW ` ASCII header.
    Raf,
    /// No recognized signature.
    Unknown,
}

/// Classifies a header slice. Pure and total: any input, including an empty
/// one, maps to exactly one [`SniffedFormat`].
pub(crate) fn sniff(header: &[u8]) -> SniffedFormat {
    // RAF: fixed ASCII signature at offset 0.
    if header.starts_with(b"FUJIFILMCCD-RAW") {
        return SniffedFormat::Raf;
    }
    // ORF: Olympus-specific magic words replace the usual TIFF magic.
    if header.starts_with(b"IIRO") || header.starts_with(b"IIRS") || header.starts_with(b"MMOR") {
        return SniffedFormat::Orf;
    }
    // RW2: little-endian byte order mark with Panasonic's 0x55 magic.
    if header.starts_with(b"IIU\0") {
        return SniffedFormat::Rw2;
    }
    // CR3: ISO BMFF box header — size (4 bytes), "ftyp", major brand "crx ".
    if header.get(4..8) == Some(b"ftyp") && header.get(8..12) == Some(b"crx ") {
        return SniffedFormat::Cr3;
    }
    // Classic little-endian TIFF; CR2 adds its own signature at offset 8.
    if header.starts_with(b"II*\0") {
        if header.get(8..12) == Some(b"CR\x02\0") {
            return SniffedFormat::Cr2;
        }
        return SniffedFormat::TiffFamily;
    }
    // Classic big-endian TIFF and both BigTIFF byte orders.
    if header.starts_with(b"MM\0*") || header.starts_with(b"II+\0") || header.starts_with(b"MM\0+") {
        return SniffedFormat::TiffFamily;
    }
    SniffedFormat::Unknown
}

/// Sniffs a file on disk, reading at most [`SNIFF_BYTES`] bytes. Returns
/// `None` when the file cannot be opened or read, so callers can fall back to
/// extension-based routing.
pub(crate) fn sniff_file(path: &Path) -> Option<SniffedFormat> {
    let file = File::open(path).ok()?;
    let mut header = Vec::with_capacity(SNIFF_BYTES);
    file.take(u64::try_from(SNIFF_BYTES).ok()?)
        .read_to_end(&mut header)
        .ok()?;
    Some(sniff(&header))
}

#[cfg(test)]
mod tests {
    use super::{SniffedFormat, sniff};

    #[test]
    fn detects_cr3_iso_bmff_brand() {
        let mut header = vec![0_u8; 32];
        header[0..4].copy_from_slice(&24_u32.to_be_bytes());
        header[4..8].copy_from_slice(b"ftyp");
        header[8..12].copy_from_slice(b"crx ");
        assert_eq!(sniff(&header), SniffedFormat::Cr3);
        // A different major brand must not match.
        header[8..12].copy_from_slice(b"qt  ");
        assert_eq!(sniff(&header), SniffedFormat::Unknown);
    }

    #[test]
    fn detects_cr2_signature_at_offset_8() {
        let mut header = b"II*\0".to_vec();
        header.extend_from_slice(&16_u32.to_le_bytes()); // first IFD offset
        header.extend_from_slice(b"CR\x02\0");
        header.extend_from_slice(&0_u32.to_le_bytes()); // raw IFD offset
        assert_eq!(sniff(&header), SniffedFormat::Cr2);
    }

    #[test]
    fn detects_tiff_family_variants() {
        assert_eq!(sniff(b"II*\0\x08\0\0\0"), SniffedFormat::TiffFamily);
        assert_eq!(sniff(b"MM\0*\0\0\0\x08"), SniffedFormat::TiffFamily);
        assert_eq!(sniff(b"II+\0\x08\0\0\0"), SniffedFormat::TiffFamily);
        assert_eq!(sniff(b"MM\0+\0\x08\0\0"), SniffedFormat::TiffFamily);
    }

    #[test]
    fn detects_orf_magics() {
        assert_eq!(sniff(b"IIRO\x08\0"), SniffedFormat::Orf);
        assert_eq!(sniff(b"IIRS\x08\0"), SniffedFormat::Orf);
        assert_eq!(sniff(b"MMOR\0\0"), SniffedFormat::Orf);
    }

    #[test]
    fn detects_rw2_magic() {
        assert_eq!(sniff(b"IIU\0\x08\0\0\0"), SniffedFormat::Rw2);
    }

    #[test]
    fn detects_raf_header() {
        let mut header = b"FUJIFILMCCD-RAW ".to_vec();
        header.extend_from_slice(&[0_u8; 16]);
        assert_eq!(sniff(&header), SniffedFormat::Raf);
    }

    #[test]
    fn short_and_empty_inputs_never_panic() {
        assert_eq!(sniff(&[]), SniffedFormat::Unknown);
        assert_eq!(sniff(b"I"), SniffedFormat::Unknown);
        assert_eq!(sniff(b"II"), SniffedFormat::Unknown);
        assert_eq!(sniff(b"II*"), SniffedFormat::Unknown);
        // Truncated RAF prefix must not match.
        assert_eq!(sniff(b"FUJIFILMCCD"), SniffedFormat::Unknown);
        // Truncated ftyp without the brand must not match.
        assert_eq!(sniff(b"\0\0\0\x18ftyp"), SniffedFormat::Unknown);
        // TIFF magic too short for the CR2 offset-8 check stays TIFF family.
        assert_eq!(sniff(b"II*\0"), SniffedFormat::TiffFamily);
    }

    #[test]
    fn unrelated_content_is_unknown() {
        assert_eq!(sniff(b"\xff\xd8\xff\xe1Exif\0\0"), SniffedFormat::Unknown);
        assert_eq!(sniff(b"\x89PNG\r\n\x1a\n"), SniffedFormat::Unknown);
        assert_eq!(sniff(&[0_u8; 256]), SniffedFormat::Unknown);
    }
}
