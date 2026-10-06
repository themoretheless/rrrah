//! Bounded EIP package inventory. This does not render Capture One adjustments.
//! Source storage is borrowed; RAW inflation uses a managed output buffer.
//! Nothing is written to the filesystem.
use std::{collections::HashSet, io::Cursor};

const MAX_ENTRIES: usize = 64;
const MAX_DIRECTORY_BYTES: usize = 64 * 1024;
const MAX_EXPANDED_BYTES: u64 = 1024 * 1024 * 1024;

#[derive(Debug, thiserror::Error)]
pub enum EipError {
    #[error("invalid EIP package: {0}")]
    Invalid(&'static str),
    #[error("invalid EIP ZIP: {0}")]
    Zip(#[from] zip::result::ZipError),
    #[error(transparent)]
    Memory(#[from] rrrah_core::BufferError),
    #[error("EIP payload I/O or CRC failure: {0}")]
    Io(#[from] std::io::Error),
    #[error("EIP operation cancelled")]
    Cancelled,
    #[error(transparent)]
    Decode(#[from] crate::DecodeError),
    #[error("EIP RAW member has no in-memory native decoder: {0}")]
    UnsupportedRaw(String),
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EipEntry {
    pub name: String,
    pub bytes: u64,
    pub compressed_bytes: u64,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EipManifest {
    pub entries: Vec<EipEntry>,
    pub raw_index: usize,
}

#[derive(Debug)]
pub struct EipRaw {
    pub manifest: EipManifest,
    pub bytes: rrrah_core::SharedBuffer<u8>,
}

/// Inflates only the unique RAW candidate into budgeted immutable storage.
/// Caller retains ownership/accounting of the borrowed package. ZIP metadata
/// and codec scratch are bounded separately; these are not total RSS accounting.
/// Assets/settings are inventoried but never silently applied to the RAW.
pub fn read_eip_raw(
    bytes: &[u8],
    budget: &rrrah_core::MemoryBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<EipRaw, EipError> {
    use std::io::Read;
    if cancelled() {
        return Err(EipError::Cancelled);
    }
    let manifest = inspect_eip(bytes)?;
    if cancelled() {
        return Err(EipError::Cancelled);
    }
    let length = usize::try_from(manifest.entries[manifest.raw_index].bytes)
        .map_err(|_| EipError::Invalid("RAW length overflow"))?;
    let mut output = budget.try_buffer(length, 0_u8)?;
    let mut archive = zip::ZipArchive::new(Cursor::new(bytes))?;
    let mut raw = archive.by_index(manifest.raw_index)?;
    for chunk in output.chunks_mut(64 * 1024) {
        if cancelled() {
            return Err(EipError::Cancelled);
        }
        raw.read_exact(chunk)?;
    }
    // Drive the reader through EOF so CRC verification cannot be skipped after
    // exactly filling the admitted buffer. Unexpected expansion is refused.
    if cancelled() {
        return Err(EipError::Cancelled);
    }
    let mut extra = [0; 1];
    if raw.read(&mut extra)? != 0 {
        return Err(EipError::Invalid("RAW payload length mismatch"));
    }
    if cancelled() {
        return Err(EipError::Cancelled);
    }
    Ok(EipRaw {
        manifest,
        bytes: output.freeze(),
    })
}

#[derive(Debug)]
pub struct EipSensor {
    pub manifest: EipManifest,
    pub decoded: crate::DecodeOutput,
    /// Inner RAW recipe; a package cache key must also fingerprint the complete EIP.
    pub raw_recipe: rrrah_core::MosaicRecipeManifest,
}

/// Explicit sensor-only import. Capture One settings, masks and ICC/LCC assets
/// are inventoried but not interpreted; this is not authored appearance reproduction.
/// Source package bytes are borrowed. RAW inflation and sensor output share the
/// request's required memory budget, and the inflated source is released before return.
pub fn decode_eip_sensor(bytes: &[u8], request: &crate::DecodeRequest) -> Result<EipSensor, EipError> {
    use crate::camtiff::{CameraFormat, NativeCameraDecoder};
    let total_started = std::time::Instant::now();
    request.check_cancelled()?;
    if request.image_index != 0 {
        return Err(crate::DecodeError::UnsupportedImageIndex {
            index: request.image_index,
        }
        .into());
    }
    let budget = request
        .memory_budget
        .as_ref()
        .ok_or(EipError::Invalid("sensor import requires a memory budget"))?;
    let cancelled = || {
        request
            .cancellation
            .as_ref()
            .is_some_and(crate::GenerationToken::is_cancelled)
    };
    let inventory = inspect_eip(bytes)?;
    let candidate = &inventory.entries[inventory.raw_index].name;
    let extension = candidate
        .rsplit_once('.')
        .map(|(_, ext)| ext.to_ascii_lowercase())
        .unwrap_or_default();
    if !matches!(
        extension.as_str(),
        "mrw"
            | "cr3"
            | "dng"
            | "iiq"
            | "mos"
            | "cr2"
            | "nef"
            | "nrw"
            | "arw"
            | "sr2"
            | "srf"
            | "orf"
            | "pef"
            | "ptx"
            | "rw2"
            | "rwl"
            | "raf"
            | "erf"
            | "kdc"
            | "srw"
            | "3fr"
            | "fff"
            | "dcr"
            | "dcs"
            | "raw"
    ) {
        return Err(EipError::UnsupportedRaw(candidate.clone()));
    }
    let raw = read_eip_raw(bytes, budget, &cancelled)?;
    let name = &raw.manifest.entries[raw.manifest.raw_index].name;
    let extension = name
        .rsplit_once('.')
        .map(|(_, e)| e.to_ascii_lowercase())
        .unwrap_or_default();
    if extension == "mrw" {
        let source_open = total_started.elapsed();
        let raw_recipe = crate::RawDecoder::mosaic_recipe(&crate::NativeMrwDecoder, request)?;
        let decoded =
            crate::NativeMrwDecoder.decode_source(request, raw.bytes, source_open, total_started)?;
        return Ok(EipSensor {
            manifest: raw.manifest,
            decoded,
            raw_recipe,
        });
    }
    if extension == "cr3" {
        let source_open = total_started.elapsed();
        let decoded =
            crate::NativeCr3Decoder.decode_source(request, raw.bytes, source_open, total_started)?;
        return Ok(EipSensor {
            manifest: raw.manifest,
            decoded,
            raw_recipe: crate::NATIVE_EOS_R8_MOSAIC_CONTRACT_1,
        });
    }
    if extension == "dng" {
        let source_open = total_started.elapsed();
        let decoded =
            crate::NativeDngDecoder.decode_source(request, raw.bytes, source_open, total_started)?;
        return Ok(EipSensor {
            manifest: raw.manifest,
            decoded,
            raw_recipe: crate::NATIVE_DNG_MOSAIC_CONTRACT_1,
        });
    }
    let format = match extension.as_str() {
        "iiq" => CameraFormat::Iiq,
        "mos" => CameraFormat::Mos,
        "cr2" => CameraFormat::Cr2,
        "nef" => CameraFormat::Nef,
        "nrw" => CameraFormat::Nrw,
        "arw" | "sr2" => CameraFormat::Arw,
        "srf" => CameraFormat::Srf,
        "orf" => CameraFormat::Orf,
        "pef" | "ptx" => CameraFormat::Pef,
        "rw2" | "rwl" => CameraFormat::Rw2,
        "raf" => CameraFormat::Raf,
        "erf" => CameraFormat::Erf,
        "kdc" => CameraFormat::Kdc,
        "srw" => CameraFormat::Srw,
        "3fr" => CameraFormat::ThreeFr,
        "fff" => CameraFormat::Fff,
        "dcr" => CameraFormat::Dcr,
        "dcs" => CameraFormat::Dcs,
        "raw" if crate::sniff::sniff(&raw.bytes) == crate::sniff::SniffedFormat::Rw2 => CameraFormat::Rw2,
        _ => return Err(EipError::UnsupportedRaw(name.clone())),
    };
    let source_open = total_started.elapsed();
    let decoded =
        NativeCameraDecoder::new(format).decode_source(request, raw.bytes, source_open, total_started)?;
    Ok(EipSensor {
        manifest: raw.manifest,
        decoded,
        raw_recipe: format.recipe(),
    })
}

/// Lists a classic single-disk ZIP package with exactly one RAW candidate.
/// ZIP64 directories, encryption, links, unsafe names and unsupported codecs are refused.
/// Entry metadata is bounded; payload CRCs are not verified until payload read.
pub fn inspect_eip(bytes: &[u8]) -> Result<EipManifest, EipError> {
    let invalid = EipError::Invalid;
    // Admission before ZipArchive allocates from attacker-controlled directory counts.
    let start = bytes.len().saturating_sub(22 + 65535);
    let end = bytes
        .len()
        .checked_sub(22)
        .ok_or(invalid("missing ZIP directory"))?;
    let footer = (start..=end)
        .rev()
        .find(|&p| {
            bytes[p..p + 4] == *b"PK\x05\x06"
                && p + 22 + usize::from(u16::from_le_bytes([bytes[p + 20], bytes[p + 21]])) == bytes.len()
        })
        .ok_or(invalid("missing ZIP directory"))?;
    let u16_at = |p| usize::from(u16::from_le_bytes([bytes[p], bytes[p + 1]]));
    let u32_at = |p| u32::from_le_bytes(bytes[p..p + 4].try_into().unwrap());
    let count = u16_at(footer + 10);
    if u16_at(footer + 4) != 0
        || u16_at(footer + 6) != 0
        || u16_at(footer + 8) != count
        || count == 0
        || count > MAX_ENTRIES
    {
        return Err(invalid("unsupported disk or entry count"));
    }
    let directory_bytes = usize::try_from(u32_at(footer + 12)).map_err(|_| invalid("directory overflow"))?;
    let directory_start = usize::try_from(u32_at(footer + 16)).map_err(|_| invalid("directory overflow"))?;
    if directory_bytes > MAX_DIRECTORY_BYTES || directory_start.checked_add(directory_bytes) != Some(footer) {
        return Err(invalid("unsupported directory extent or ZIP64"));
    }
    let mut archive = zip::ZipArchive::new(Cursor::new(bytes))?;
    if archive.len() != count {
        return Err(invalid("directory count mismatch"));
    }
    let mut names = HashSet::new();
    let mut entries = Vec::with_capacity(count);
    let mut raw_index = None;
    let mut total = 0_u64;
    for i in 0..count {
        let entry = archive.by_index_raw(i)?;
        let name = std::str::from_utf8(entry.name_raw()).map_err(|_| invalid("non-UTF8 entry name"))?;
        if name.is_empty()
            || name.len() > 256
            || name.contains(['\\', ':', '\0'])
            || name.starts_with('/')
            || name.split('/').any(|part| part == ".." || part == ".")
        {
            return Err(invalid("unsafe entry name"));
        }
        if !names.insert(name.to_owned()) {
            return Err(invalid("duplicate entry name"));
        }
        if entry.encrypted()
            || entry.is_symlink()
            || !matches!(
                entry.compression(),
                zip::CompressionMethod::Stored | zip::CompressionMethod::Deflated
            )
        {
            return Err(invalid("unsupported entry storage"));
        }
        total = total
            .checked_add(entry.size())
            .filter(|n| *n <= MAX_EXPANDED_BYTES)
            .ok_or(invalid("expanded package limit"))?;
        let extension = name.rsplit_once('.').map(|(_, ext)| ext.to_ascii_lowercase());
        if !entry.is_dir()
            && extension.as_deref().is_some_and(|ext| {
                matches!(
                    ext,
                    "iiq"
                        | "raw"
                        | "dng"
                        | "cr2"
                        | "cr3"
                        | "nef"
                        | "nrw"
                        | "arw"
                        | "sr2"
                        | "srf"
                        | "orf"
                        | "raf"
                        | "pef"
                        | "rw2"
                        | "rwl"
                        | "mrw"
                        | "erf"
                        | "kdc"
                        | "srw"
                        | "3fr"
                        | "fff"
                        | "dcr"
                        | "dcs"
                        | "mos"
                        | "mef"
                        | "crw"
                        | "x3f"
                        | "gpr"
                        | "bay"
                        | "cap"
                        | "rwz"
                )
            })
        {
            if entry.size() == 0 || raw_index.replace(entries.len()).is_some() {
                return Err(invalid("empty or ambiguous RAW source"));
            }
        }
        entries.push(EipEntry {
            name: name.to_owned(),
            bytes: entry.size(),
            compressed_bytes: entry.compressed_size(),
        });
    }
    Ok(EipManifest {
        entries,
        raw_index: raw_index.ok_or(invalid("missing RAW source"))?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    fn package(names: &[&str], compression: zip::CompressionMethod) -> Vec<u8> {
        let mut writer = zip::ZipWriter::new(Cursor::new(Vec::new()));
        for name in names {
            writer
                .start_file(
                    *name,
                    zip::write::SimpleFileOptions::default().compression_method(compression),
                )
                .unwrap();
            writer.write_all(b"payload").unwrap();
        }
        writer.finish().unwrap().into_inner()
    }
    #[test]
    fn inventories_raw_and_assets_without_inflating() {
        for compression in [zip::CompressionMethod::Stored, zip::CompressionMethod::Deflated] {
            let b = package(
                &[
                    "0.IIQ",
                    "CaptureOne/Settings/settings.cos",
                    "Profiles/camera.icc",
                    "Profiles/lens.lcc",
                ],
                compression,
            );
            let m = inspect_eip(&b).unwrap();
            assert_eq!(m.raw_index, 0);
            assert_eq!(m.entries.len(), 4);
            assert!(m.entries.iter().all(|e| e.bytes == 7));
            // CRC damage does not change the structural inventory: payload verification is separate.
            let mut corrupted = b.clone();
            corrupted[35] ^= 1;
            assert!(inspect_eip(&corrupted).is_ok());
        }
    }
    #[test]
    fn rejects_unsafe_ambiguous_missing_and_duplicate_sources() {
        for names in [
            &["../0.iiq"][..],
            &["/0.iiq"],
            &["C:0.iiq"],
            &["0.iiq", "1.dng"],
            &["settings.cos"],
        ] {
            assert!(inspect_eip(&package(names, zip::CompressionMethod::Stored)).is_err());
        }
        let mut duplicate = package(&["0.iiq", "a.icc", "b.icc"], zip::CompressionMethod::Stored);
        for p in 0..duplicate.len() - 5 {
            if &duplicate[p..p + 5] == b"b.icc" {
                duplicate[p] = b'a';
            }
        }
        assert!(inspect_eip(&duplicate).is_err());
    }
    #[test]
    fn rejects_encryption_unknown_codec_and_expansion_claims() {
        let valid = package(&["0.iiq"], zip::CompressionMethod::Stored);
        let central = valid.windows(4).position(|b| b == b"PK\x01\x02").unwrap();
        for (offset, value) in [(8, 1_u16), (10, 99_u16)] {
            let mut bad = valid.clone();
            bad[central + offset..central + offset + 2].copy_from_slice(&value.to_le_bytes());
            assert!(inspect_eip(&bad).is_err());
        }
        for size in [0_u32, 1024 * 1024 * 1024 + 1] {
            let mut bad = valid.clone();
            bad[central + 24..central + 28].copy_from_slice(&size.to_le_bytes());
            assert!(inspect_eip(&bad).is_err());
        }
        let huge_name = format!("{}.iiq", "a".repeat(256));
        assert!(inspect_eip(&package(&[&huge_name], zip::CompressionMethod::Stored)).is_err());
    }

    #[test]
    fn managed_raw_exact_bytes_crc_and_last_owner_accounting() {
        for compression in [zip::CompressionMethod::Stored, zip::CompressionMethod::Deflated] {
            let package = package(&["0.iiq", "settings.cos"], compression);
            let budget = rrrah_core::MemoryBudget::new(7);
            let raw = read_eip_raw(&package, &budget, &|| false).unwrap();
            assert_eq!(&*raw.bytes, b"payload");
            assert_eq!(raw.manifest.entries[raw.manifest.raw_index].name, "0.iiq");
            let held = raw.bytes.clone();
            drop(raw);
            assert_eq!(budget.used(), 7);
            drop(held);
            assert_eq!(budget.used(), 0);
            let refused = rrrah_core::MemoryBudget::new(6);
            assert!(matches!(
                read_eip_raw(&package, &refused, &|| false),
                Err(EipError::Memory(_))
            ));
            assert_eq!(refused.peak(), 0);
            let mut corrupted = package.clone();
            let central = corrupted.windows(4).position(|b| b == b"PK\x01\x02").unwrap();
            // Corrupt CRC in both headers without corrupting DEFLATE data.
            corrupted[14] ^= 1;
            corrupted[central + 16] ^= 1;
            assert!(matches!(
                read_eip_raw(&corrupted, &budget, &|| false),
                Err(EipError::Io(_))
            ));
            assert_eq!(budget.used(), 0);
        }
    }
    #[test]
    fn cancellation_before_and_during_inflation_releases_admission() {
        use std::cell::Cell;
        let package = package(&["0.iiq"], zip::CompressionMethod::Deflated);
        let empty = rrrah_core::MemoryBudget::new(0);
        assert!(matches!(
            read_eip_raw(&[], &empty, &|| true),
            Err(EipError::Cancelled)
        ));
        assert_eq!(empty.peak(), 0);
        for stop in [2, 3, 4] {
            let calls = Cell::new(0);
            let budget = rrrah_core::MemoryBudget::new(7);
            let cancelled = || {
                calls.set(calls.get() + 1);
                calls.get() >= stop
            };
            assert!(matches!(
                read_eip_raw(&package, &budget, &cancelled),
                Err(EipError::Cancelled)
            ));
            assert_eq!(budget.used(), 0);
            assert_eq!(budget.peak(), if stop == 2 { 0 } else { 7 });
        }
    }
    #[test]
    #[ignore = "requires independently authored ZIP containing pinned IIQ source"]
    fn independent_zip_raw_matches_original_source() {
        let package = std::fs::read(std::env::var("RRRAH_EIP_PACKAGE").unwrap()).unwrap();
        let source = std::fs::read(std::env::var("RRRAH_EIP_RAW_SOURCE").unwrap()).unwrap();
        let budget = rrrah_core::MemoryBudget::new(source.len() as u64);
        let output = read_eip_raw(&package, &budget, &|| false).unwrap();
        assert_eq!(&*output.bytes, source.as_slice());
        assert_eq!(budget.used(), source.len() as u64);
        drop(output);
        assert_eq!(budget.used(), 0);
        // Cancel between chunks after some of the real large RAW has inflated.
        let calls = std::cell::Cell::new(0);
        let cancelled = || {
            calls.set(calls.get() + 1);
            calls.get() >= 10
        };
        assert!(matches!(
            read_eip_raw(&package, &budget, &cancelled),
            Err(EipError::Cancelled)
        ));
        assert_eq!(budget.used(), 0);
    }

    #[test]
    #[ignore = "requires authored IIQ ZIP and independent corrected LibRaw sensor oracle"]
    fn memory_sensor_import_matches_full_independent_sensor() {
        let package = std::fs::read(std::env::var("RRRAH_EIP_PACKAGE").unwrap()).unwrap();
        let oracle = std::fs::read(std::env::var("RRRAH_EIP_SENSOR_ORACLE").unwrap()).unwrap();
        let budget = rrrah_core::MemoryBudget::new(96 * 1024 * 1024);
        // Deliberately nonexistent path proves import uses only supplied package bytes.
        let mut request = crate::DecodeRequest::new("/nonexistent/eip-sensor-import/source.eip");
        request.memory_budget = Some(budget.clone());
        let sensor = decode_eip_sensor(&package, &request).unwrap();
        assert_eq!(sensor.raw_recipe.decoder_backend_id(), 20);
        assert_eq!(sensor.manifest.entries.len(), 2);
        let mosaic = &sensor.decoded.mosaic;
        assert_eq!((mosaic.metadata.width, mosaic.metadata.height), (4134, 4128));
        assert!(mosaic.pixels.is_managed());
        assert_eq!(mosaic.pixels.len() * 2, oracle.len());
        assert!(
            mosaic
                .pixels
                .iter()
                .zip(oracle.chunks_exact(2))
                .all(|(a, b)| *a == u16::from_le_bytes([b[0], b[1]]))
        );
        assert_eq!(budget.used(), oracle.len() as u64);
        let held = mosaic.pixels.clone();
        drop(sensor);
        assert_eq!(budget.used(), oracle.len() as u64);
        drop(held);
        assert_eq!(budget.used(), 0);
        let small = rrrah_core::MemoryBudget::new(21495886);
        request.memory_budget = Some(small.clone());
        assert!(decode_eip_sensor(&package, &request).is_err());
        assert_eq!(small.used(), 0);
    }

    #[test]
    fn sensor_import_refuses_unsupported_raw_before_inflation_and_releases_bad_camera() {
        let zero = rrrah_core::MemoryBudget::new(0);
        let mut request = crate::DecodeRequest::new("not-read.eip");
        request.memory_budget = Some(zero.clone());
        let unsupported = package(&["0.mef"], zip::CompressionMethod::Deflated);
        assert!(matches!(
            decode_eip_sensor(&unsupported, &request),
            Err(EipError::UnsupportedRaw(_))
        ));
        assert_eq!(zero.peak(), 0);
        let budget = rrrah_core::MemoryBudget::new(7);
        request.memory_budget = Some(budget.clone());
        let malformed = package(&["0.iiq"], zip::CompressionMethod::Deflated);
        assert!(matches!(
            decode_eip_sensor(&malformed, &request),
            Err(EipError::Decode(_))
        ));
        assert_eq!(budget.peak(), 7);
        assert_eq!(budget.used(), 0);
        request.image_index = 1;
        assert!(matches!(
            decode_eip_sensor(&[], &request),
            Err(EipError::Decode(crate::DecodeError::UnsupportedImageIndex {
                index: 1
            }))
        ));
    }

    #[test]
    #[ignore = "requires authored DNG ZIP and independent LibRaw full sensor oracle"]
    fn memory_dng_import_matches_full_independent_sensor() {
        let package = std::fs::read(std::env::var("RRRAH_EIP_DNG_PACKAGE").unwrap()).unwrap();
        let oracle = std::fs::read(std::env::var("RRRAH_EIP_DNG_ORACLE").unwrap()).unwrap();
        let budget = rrrah_core::MemoryBudget::new(128 * 1024 * 1024);
        let mut request = crate::DecodeRequest::new("/nonexistent/eip-dng-import/source.eip");
        request.memory_budget = Some(budget.clone());
        let sensor = decode_eip_sensor(&package, &request).unwrap();
        assert_eq!(sensor.raw_recipe, crate::NATIVE_DNG_MOSAIC_CONTRACT_1);
        let mosaic = &sensor.decoded.mosaic;
        assert_eq!((mosaic.metadata.width, mosaic.metadata.height), (4352, 2868));
        assert_eq!(
            mosaic.metadata.cfa.as_ref().unwrap().cells,
            [
                rrrah_core::CfaColor::Blue,
                rrrah_core::CfaColor::Green,
                rrrah_core::CfaColor::Green,
                rrrah_core::CfaColor::Red
            ]
        );
        assert_eq!(
            mosaic.metadata.effective_crop(),
            rrrah_core::Rect::new(10, 10, 4288, 2848)
        );
        assert_eq!(mosaic.metadata.black_level.values, [1., 9., 9., 0.]);
        assert_eq!(mosaic.metadata.white_level.0, [4094.]);
        assert_eq!(mosaic.metadata.white_balance, [1.30078125 / 1.03125, 1., 3., 1.]);
        assert!(mosaic.pixels.is_managed());
        assert_eq!(mosaic.pixels.len() * 2, oracle.len());
        assert!(
            mosaic
                .pixels
                .iter()
                .zip(oracle.chunks_exact(2))
                .all(|(a, b)| *a == u16::from_le_bytes([b[0], b[1]]))
        );
        assert_eq!(budget.used(), oracle.len() as u64);
        let held = mosaic.pixels.clone();
        drop(sensor);
        assert_eq!(budget.used(), oracle.len() as u64);
        drop(held);
        assert_eq!(budget.used(), 0);
        let inventory = inspect_eip(&package).unwrap();
        let source_bytes = inventory.entries[inventory.raw_index].bytes;
        let small = rrrah_core::MemoryBudget::new(source_bytes);
        request.memory_budget = Some(small.clone());
        assert!(decode_eip_sensor(&package, &request).is_err());
        assert_eq!(small.used(), 0);
    }

    #[test]
    #[ignore = "requires authored local EOS R8 packages and independent sensor oracles"]
    fn memory_cr3_import_matches_both_independent_local_sensor_oracles() {
        let corpus = std::path::PathBuf::from(std::env::var("RRRAH_EIP_CR3_CORPUS").unwrap());
        let oracles = std::path::PathBuf::from(std::env::var("RRRAH_EIP_CR3_ORACLES").unwrap());
        for (id, wb) in [
            ("IMG_9043", [1678., 1024., 1659., 1024.]),
            ("IMG_9074", [1691., 1024., 1641., 1024.]),
        ] {
            let oracle = std::fs::read(oracles.join(format!("{id}.qualified-oracle.u16le"))).unwrap();
            for encoding in ["stored", "deflated"] {
                let package = std::fs::read(corpus.join(id).join(format!("cr3-{encoding}.eip"))).unwrap();
                let budget = rrrah_core::MemoryBudget::new(128 * 1024 * 1024);
                let mut request = crate::DecodeRequest::new("/nonexistent/eip-cr3-import/source.eip");
                request.memory_budget = Some(budget.clone());
                let sensor = decode_eip_sensor(&package, &request).unwrap();
                assert_eq!(sensor.raw_recipe, crate::NATIVE_EOS_R8_MOSAIC_CONTRACT_1);
                let mosaic = &sensor.decoded.mosaic;
                assert_eq!((mosaic.metadata.width, mosaic.metadata.height), (6188, 4120));
                assert_eq!(
                    mosaic.metadata.effective_crop(),
                    rrrah_core::Rect::new(168, 108, 6000, 4000)
                );
                assert_eq!(mosaic.metadata.black_level.values, [512.; 4]);
                assert_eq!(mosaic.metadata.white_level.0, [16383.]);
                assert_eq!(mosaic.metadata.white_balance, wb.map(|value| value / 1024.));
                assert!(mosaic.pixels.is_managed());
                assert_eq!(mosaic.pixels.len() * 2, oracle.len());
                assert!(
                    mosaic
                        .pixels
                        .iter()
                        .zip(oracle.chunks_exact(2))
                        .all(|(a, b)| *a == u16::from_le_bytes([b[0], b[1]])),
                    "{id}/{encoding}"
                );
                let held = mosaic.pixels.clone();
                drop(sensor);
                assert_eq!(budget.used(), oracle.len() as u64);
                drop(held);
                assert_eq!(budget.used(), 0);
                let inventory = inspect_eip(&package).unwrap();
                let source_bytes = inventory.entries[inventory.raw_index].bytes;
                let small = rrrah_core::MemoryBudget::new(source_bytes);
                request.memory_budget = Some(small.clone());
                assert!(decode_eip_sensor(&package, &request).is_err());
                assert_eq!(small.used(), 0);
            }
        }
    }

    #[test]
    #[ignore = "requires authored MRW packages and independent LibRaw full sensor oracles"]
    fn memory_mrw_import_matches_both_independent_sensor_oracles() {
        let corpus = std::path::PathBuf::from(std::env::var("RRRAH_EIP_MRW_CORPUS").unwrap());
        let oracles = std::path::PathBuf::from(std::env::var("RRRAH_EIP_MRW_ORACLES").unwrap());
        for (id, width, height, white, wb) in [
            (1826, 3016, 2008, 4091., [435., 256., 435., 256.]),
            (4419, 3272, 2456, 3983., [513., 256., 368., 256.]),
        ] {
            let oracle = std::fs::read(oracles.join(format!("rrrah-eip-mrw-{id}-reference.u16le"))).unwrap();
            for encoding in ["stored", "deflated"] {
                let package =
                    std::fs::read(corpus.join(id.to_string()).join(format!("mrw-{encoding}.eip"))).unwrap();
                let budget = rrrah_core::MemoryBudget::new(64 * 1024 * 1024);
                let mut request = crate::DecodeRequest::new("/nonexistent/eip-mrw-import/source.eip");
                request.memory_budget = Some(budget.clone());
                let sensor = decode_eip_sensor(&package, &request).unwrap();
                assert_eq!(sensor.raw_recipe.decoder_backend_id(), 12);
                let mosaic = &sensor.decoded.mosaic;
                assert_eq!((mosaic.metadata.width, mosaic.metadata.height), (width, height));
                assert_eq!(
                    mosaic.metadata.effective_crop(),
                    rrrah_core::Rect::new(0, 0, width, height)
                );
                assert_eq!(mosaic.metadata.black_level.values, [0.]);
                assert_eq!(mosaic.metadata.white_level.0, [white]);
                assert_eq!(mosaic.metadata.white_balance, wb.map(|v| v / 256.));
                assert!(mosaic.pixels.is_managed());
                assert_eq!(mosaic.pixels.len() * 2, oracle.len());
                assert!(
                    mosaic
                        .pixels
                        .iter()
                        .zip(oracle.chunks_exact(2))
                        .all(|(a, b)| *a == u16::from_le_bytes([b[0], b[1]])),
                    "{id}/{encoding}"
                );
                let held = mosaic.pixels.clone();
                drop(sensor);
                assert_eq!(budget.used(), oracle.len() as u64);
                drop(held);
                assert_eq!(budget.used(), 0);
                let inventory = inspect_eip(&package).unwrap();
                let small = rrrah_core::MemoryBudget::new(inventory.entries[inventory.raw_index].bytes);
                request.memory_budget = Some(small.clone());
                assert!(decode_eip_sensor(&package, &request).is_err());
                assert_eq!(small.used(), 0);
            }
        }
    }

    #[test]
    fn directory_admission_precedes_zip_metadata_allocation() {
        let valid = package(&["0.iiq"], zip::CompressionMethod::Stored);
        for n in 0..valid.len() {
            assert!(inspect_eip(&valid[..n]).is_err());
        }
        for field in [4, 6, 8, 10, 12, 16] {
            let mut bad = valid.clone();
            let p = bad.len() - 22 + field;
            bad[p] = 0xff;
            bad[p + 1] = 0xff;
            assert!(inspect_eip(&bad).is_err());
        }
    }
}
