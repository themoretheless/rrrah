//! Content-based RAW format detection from the first bytes of a file.
//!
//! The sniffer reads at most [`SNIFF_BYTES`] bytes and never panics on short
//! or truncated input. It distinguishes the strong container magics (CR3 ISO
//! BMFF, CR2, ORF, RW2, RAF) from the generic TIFF family; TIFF-family camera
//! formats (NEF, ARW, PEF) share the classic TIFF magic and are refined later
//! by their backend through the Make/Model tags.

use std::{fs::File, io::Read, path::Path};

/// Maximum number of header bytes examined by the sniffer.
// PICT file-data-fork framing ends at byte 552 (512-byte application header,
// 10-byte picture frame and 30-byte version/header prefix).
pub(crate) const SNIFF_BYTES: usize = 552;

/// Coarse container classification from content alone.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SniffedFormat {
    /// ISO BMFF with an `ftyp` major brand of `crx ` (Canon CR3).
    Cr3,
    /// Canon CIFF container; camera subset is validated by the decoder.
    Crw,
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
    /// Minolta big-endian MRM block container.
    Mrw,
    /// No recognized signature.
    Unknown,
}

/// Classifies a header slice. Pure and total: any input, including an empty
/// one, maps to exactly one [`SniffedFormat`].
pub(crate) fn sniff(header: &[u8]) -> SniffedFormat {
    if (header.starts_with(b"II") || header.starts_with(b"MM"))
        && header.get(6..14) == Some(b"HEAPCCDR") {
        return SniffedFormat::Crw;
    }
    if header.starts_with(b"\0MRM") {
        return SniffedFormat::Mrw;
    }
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
    let (header, length) = read_header(File::open(path).ok()?).ok()?;
    Some(sniff(&header[..length]))
}

/// Bounded stack storage, with no heap allocation for format detection.
pub(crate) fn read_header(mut reader: impl Read) -> std::io::Result<([u8; SNIFF_BYTES], usize)> {
    let mut bytes = [0_u8; SNIFF_BYTES];
    let mut length = 0;
    while length < bytes.len() {
        match reader.read(&mut bytes[length..]) {
            Ok(0) => break,
            Ok(n) => length += n,
            Err(error) if error.kind() == std::io::ErrorKind::Interrupted => continue,
            Err(error) => return Err(error),
        }
    }
    Ok((bytes, length))
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

/// Bounded classic-TIFF identity probe for Kodak's legacy DCS520C TIFF.
/// Reads at most 128 root entries and two 64-byte ASCII strings; no sensor read.
/// DNGVersion prevents routing a converted DNG through the native Kodak path.
pub(crate) fn is_kodak_dcs520(path: &Path) -> bool {
    use std::io::{Seek, SeekFrom};
    fn probe(path: &Path) -> Option<bool> {
        let mut file = File::open(path).ok()?;
        let mut header = [0_u8; 8];
        file.read_exact(&mut header).ok()?;
        let little = match &header[..4] {
            b"II\x2a\0" => true,
            b"MM\0\x2a" => false,
            _ => return None,
        };
        let u16_at = |b: &[u8]| {
            if little {
                u16::from_le_bytes(b.try_into().unwrap())
            } else {
                u16::from_be_bytes(b.try_into().unwrap())
            }
        };
        let u32_at = |b: &[u8]| {
            if little {
                u32::from_le_bytes(b.try_into().unwrap())
            } else {
                u32::from_be_bytes(b.try_into().unwrap())
            }
        };
        let root = u64::from(u32_at(&header[4..8]));
        if root < 8 {
            return None;
        }
        file.seek(SeekFrom::Start(root)).ok()?;
        let mut count = [0; 2];
        file.read_exact(&mut count).ok()?;
        let count = u16_at(&count);
        if count > 128 {
            return None;
        }
        let mut identities = [None, None];
        for i in 0..count {
            file.seek(SeekFrom::Start(root + 2 + u64::from(i) * 12)).ok()?;
            let mut entry = [0; 12];
            file.read_exact(&mut entry).ok()?;
            let tag = u16_at(&entry[..2]);
            if tag == 50706 {
                return Some(false);
            }
            let slot = match tag {
                271 => 0,
                272 => 1,
                _ => continue,
            };
            if identities[slot].is_some() || u16_at(&entry[2..4]) != 2 {
                return None;
            }
            let n = usize::try_from(u32_at(&entry[4..8])).ok()?;
            if !(1..=64).contains(&n) {
                return None;
            }
            let mut text = [0; 64];
            if n <= 4 {
                text[..n].copy_from_slice(&entry[8..8 + n]);
            } else {
                file.seek(SeekFrom::Start(u64::from(u32_at(&entry[8..12]))))
                    .ok()?;
                file.read_exact(&mut text[..n]).ok()?;
            }
            let expected: &[u8] = if slot == 0 { b"Kodak\0" } else { b"DCS520C\0" };
            identities[slot] = Some(&text[..n] == expected);
        }
        Some(identities == [Some(true), Some(true)])
    }
    probe(path).unwrap_or(false)
}

#[cfg(test)]
mod kodak_identity_tests {
    #[test]
    fn bounded_probe_requires_unique_native_identity_and_excludes_dng() {
        let path = std::env::temp_dir().join(format!("rrrah-kodak-probe-{}.tif", std::process::id()));
        for little in [true, false] {
            let word = |v: u16| if little { v.to_le_bytes() } else { v.to_be_bytes() };
            let long = |v: u32| if little { v.to_le_bytes() } else { v.to_be_bytes() };
            let mut bytes = vec![0u8; 80];
            bytes[..2].copy_from_slice(if little { b"II" } else { b"MM" });
            bytes[2..4].copy_from_slice(&word(42));
            bytes[4..8].copy_from_slice(&long(8));
            bytes[8..10].copy_from_slice(&word(2));
            for (i, tag, count, offset) in [(0, 271, 6, 60), (1, 272, 8, 66)] {
                let p = 10 + i * 12;
                bytes[p..p + 2].copy_from_slice(&word(tag));
                bytes[p + 2..p + 4].copy_from_slice(&word(2));
                bytes[p + 4..p + 8].copy_from_slice(&long(count));
                bytes[p + 8..p + 12].copy_from_slice(&long(offset));
            }
            bytes[60..66].copy_from_slice(b"Kodak\0");
            bytes[66..74].copy_from_slice(b"DCS520C\0");
            std::fs::write(&path, &bytes).unwrap();
            assert!(super::is_kodak_dcs520(&path));
            let mut converted = bytes.clone();
            converted[8..10].copy_from_slice(&word(3));
            converted[34..36].copy_from_slice(&word(50706));
            std::fs::write(&path, converted).unwrap();
            assert!(!super::is_kodak_dcs520(&path));
            let mut duplicate = bytes.clone();
            duplicate[22..24].copy_from_slice(&word(271));
            std::fs::write(&path, duplicate).unwrap();
            assert!(!super::is_kodak_dcs520(&path));
            let mut oversized = bytes.clone();
            oversized[14..18].copy_from_slice(&long(65));
            std::fs::write(&path, oversized).unwrap();
            assert!(!super::is_kodak_dcs520(&path));
            bytes[73] = b'X';
            std::fs::write(&path, bytes).unwrap();
            assert!(!super::is_kodak_dcs520(&path));
        }
        std::fs::remove_file(path).unwrap();
    }
}
