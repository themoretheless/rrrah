//! Bounded Canon EOS 10D CRW import with qualified format tables.
use crate::{DecodeError, DecodeRequest, crw_entropy::Huffman};

/// Decode the qualified EOS 10D subset using bundled table-zero data.
/// Without an explicit request budget, uses a bounded 256 MiB source/output root.
/// The native RAW router delegates the qualified CRW subset to this function.
pub fn decode_crw_10d(request: &DecodeRequest) -> Result<rrrah_core::DecodedMosaic, DecodeError> {
    let mut managed = request.clone();
    if managed.memory_budget.is_none() {
        managed.memory_budget = Some(rrrah_core::MemoryBudget::new(256 * 1024 * 1024));
    }
    decode_crw_10d_with_tables(&managed, include_bytes!("../data/crw/table-zero.bin"))
}

/// Decode the qualified Canon EOS 10D CIFF subset into a managed mosaic.
///
/// `table_zero` must be the qualified 209-byte Canon table-zero data (29-byte
/// first-symbol table followed by 180-byte remaining-symbol table). The caller
/// must provide those external bytes; pinned BLAKE3 identity and canonical shape
/// are validated before any source I/O.
/// The native router uses the bundled-table wrapper, not external table bytes.
/// An explicit request memory budget is required and charges source plus output.
/// Preset modes 4/7, unknown orientations/cameras/layouts refuse without fallback.
pub fn decode_crw_10d_with_tables(
    request: &DecodeRequest,
    table_zero: &[u8],
) -> Result<rrrah_core::DecodedMosaic, DecodeError> {
    decode_with_timings(request, table_zero).map(|output| output.mosaic)
}

fn decode_with_timings(request: &DecodeRequest, table_zero: &[u8]) -> Result<crate::DecodeOutput, DecodeError> {
    let total = std::time::Instant::now();
    request.check_cancelled()?;
    if request.image_index != 0 {
        return Err(DecodeError::NativeCamera { format: "CRW", message: "CRW image index must be zero".into() });
    }
    if request.memory_budget.is_none() {
        return Err(DecodeError::NativeCamera { format: "CRW", message: "explicit CRW memory budget required".into() });
    }
    validate_table_zero(table_zero)?;
    let source_started = std::time::Instant::now();
    let source = crate::bounded_io::read_managed(request)?;
    decode_source_with_tables(request, table_zero, source, source_started.elapsed(), total)
}

fn validate_table_zero(table_zero: &[u8]) -> Result<(), DecodeError> {
    if table_zero.len() != 209 || blake3::hash(table_zero).to_hex().as_str() != "47f131299f596ac3ba864797e19735428b8808a068f2b50edf250d6dec1b7916" {
        return Err(DecodeError::NativeCamera { format: "CRW", message: "unqualified CRW table-zero data".into() });
    }
    Ok(())
}

fn decode_source_with_tables<S: std::ops::Deref<Target = [u8]>>(
    request: &DecodeRequest, table_zero: &[u8], source: S,
    source_open: std::time::Duration, total: std::time::Instant,
) -> Result<crate::DecodeOutput, DecodeError> {
    let select_started = std::time::Instant::now();
    let error = |message: &str| DecodeError::NativeCamera {
        format: "CRW",
        message: message.into(),
    };
    request.check_cancelled()?;
    if request.image_index != 0 {
        return Err(error("CRW image index must be zero"));
    }
    let budget = request
        .memory_budget
        .as_ref()
        .ok_or_else(|| error("explicit CRW memory budget required"))?;
    validate_table_zero(table_zero)?;
    fn tree(bytes: &[u8]) -> Result<Huffman<'_>, &'static str> {
        let counts: [u8; 16] = bytes.get(..16).ok_or("short Huffman table")?.try_into().unwrap();
        let count: usize = counts.iter().map(|v| usize::from(*v)).sum();
        let symbols = bytes.get(16..16 + count).ok_or("short Huffman symbols")?;
        Huffman::new(counts, symbols).map_err(|_| "invalid Huffman table")
    }
    let first = tree(&table_zero[..29]).map_err(error)?;
    let rest = tree(&table_zero[29..]).map_err(error)?;
    let decoder_select = select_started.elapsed();
    let raw_started = std::time::Instant::now();
    let mosaic = crate::crw_entropy::decode_10d(&source, 0, (&first, &rest), budget, &|| {
        request.check_cancelled().is_err()
    })?;
    let raw_image = raw_started.elapsed();
    Ok(crate::DecodeOutput {
        mosaic,
        timings: crate::DecodeTimings {
            source_open,
            decoder_select,
            raw_image,
            raw_decode: decoder_select + raw_image,
            total: total.elapsed(),
            ..Default::default()
        },
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn admission_and_table_identity_refuse_before_source_io() {
        let mut request = DecodeRequest::new("/nonexistent/rrrah-crw-admission.crw");
        let budget = rrrah_core::MemoryBudget::new(1024);
        assert!(matches!(
            decode_crw_10d_with_tables(&request, &[]),
            Err(DecodeError::NativeCamera { .. })
        ));
        request.memory_budget = Some(budget.clone());
        for bytes in [&[][..], &[0; 208][..], &[0; 209][..], &[0; 210][..]] {
            assert!(matches!(
                decode_crw_10d_with_tables(&request, bytes),
                Err(DecodeError::NativeCamera { .. })
            ));
        }
        request.image_index = 1;
        assert!(matches!(
            decode_crw_10d_with_tables(&request, &[]),
            Err(DecodeError::NativeCamera { .. })
        ));
        request.cancellation = Some(crate::GenerationToken::new(
            std::sync::Arc::new(std::sync::atomic::AtomicU64::new(1)),
            0,
        ));
        assert!(matches!(
            decode_crw_10d_with_tables(&request, &[]),
            Err(DecodeError::Cancelled)
        ));
        assert_eq!(budget.used(), 0);
        assert_eq!(budget.peak(), 0);
    }
}

impl NativeCrwDecoder {
    pub(crate) fn decode_source<S: std::ops::Deref<Target = [u8]>>(
        &self, request: &DecodeRequest, source: S, source_open: std::time::Duration,
        total: std::time::Instant,
    ) -> Result<crate::DecodeOutput, DecodeError> {
        decode_source_with_tables(request, include_bytes!("../data/crw/table-zero.bin"), source, source_open, total)
    }
}

/// Native producer for the qualified EOS 10D subset; other CIFF cameras refuse.
#[derive(Debug, Clone, Copy, Default)]
pub struct NativeCrwDecoder;
impl crate::RawDecoder for NativeCrwDecoder {
    fn mosaic_recipe(
        &self,
        request: &DecodeRequest,
    ) -> Result<rrrah_core::MosaicRecipeManifest, DecodeError> {
        request.check_cancelled()?;
        if request.image_index != 0 {
            return Err(DecodeError::UnsupportedImageIndex {
                index: request.image_index,
            });
        }
        Ok(rrrah_core::MosaicRecipeManifest::new(
            23,
            1,
            1,
            1,
            rrrah_core::DECODE_FULL_SENSOR_RAW
                | rrrah_core::DECODE_INTEGER_U16
                | rrrah_core::DECODE_SENSOR_COORDINATES
                | rrrah_core::DECODE_CROP_AS_METADATA
                | rrrah_core::DECODE_IMAGE_INDEX_IN_KEY,
            crate::WORKSPACE_LOCK_DIGEST,
        ))
    }
    fn decode(&self, request: &DecodeRequest) -> Result<crate::DecodeOutput, DecodeError> {
        let mut managed = request.clone();
        if managed.memory_budget.is_none() {
            managed.memory_budget = Some(rrrah_core::MemoryBudget::new(256 * 1024 * 1024));
        }
        decode_with_timings(&managed, include_bytes!("../data/crw/table-zero.bin"))
    }
}

#[cfg(test)]
mod router_tests {
    use super::*;
    use crate::RawDecoder;
    #[test]
    fn ciff_signature_routes_even_with_unknown_extension() {
        let path = std::env::temp_dir().join(format!(
            "rrrah-ciff-route-{}-{}.bin",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::write(&path, b"II\x1a\0\0\0HEAPCCDR").unwrap();
        let request = DecodeRequest::new(&path);
        assert_eq!(
            crate::image_source_kind(&request).unwrap(),
            crate::ImageSourceKind::Sensor
        );
        assert_eq!(
            crate::NativeRawDecoder.mosaic_recipe(&request).unwrap(),
            NativeCrwDecoder.mosaic_recipe(&request).unwrap()
        );
        assert!(crate::is_supported_raw_path(std::path::Path::new("camera.CRW")));
        std::fs::remove_file(path).unwrap();
    }
    #[test]
    #[ignore = "requires pinned EOS 10D source via RRRAH_CRW_SOURCE"]
    fn real_crw_public_route_matches_explicit_import() {
        let path = std::env::var("RRRAH_CRW_SOURCE").unwrap();
        let request = DecodeRequest::new(&path);
        let direct = decode_crw_10d(&request).unwrap();
        let routed = crate::NativeRawDecoder.decode(&request).unwrap();
        assert_eq!(
            routed.timings.raw_decode,
            routed.timings.decoder_select + routed.timings.raw_image
        );
        assert!(routed.timings.total >= routed.timings.source_open + routed.timings.raw_decode);
        assert_eq!(direct.metadata, routed.mosaic.metadata);
        assert_eq!(&*direct.pixels, &*routed.mosaic.pixels);
    }
    #[test]
    #[ignore = "requires pinned EOS 10D source via RRRAH_CRW_SOURCE"]
    fn real_crw_routed_budget_pressure_is_retryable_and_last_owner_charged() {
        let path = std::env::var("RRRAH_CRW_SOURCE").unwrap();
        let source_bytes = std::fs::metadata(&path).unwrap().len();
        let output_bytes = 3152u64 * 2068 * 2;
        let budget = rrrah_core::MemoryBudget::new(source_bytes + output_bytes);
        let mut request = DecodeRequest::new(&path);
        request.memory_budget = Some(budget.clone());
        let competing = budget.try_buffer(1, 0u8).unwrap().freeze();
        assert!(matches!(
            crate::NativeRawDecoder.decode(&request),
            Err(DecodeError::Memory(_))
        ));
        assert_eq!(budget.used(), 1);
        drop(competing);
        let decoded = crate::NativeRawDecoder.decode(&request).unwrap();
        assert_eq!(budget.used(), output_bytes);
        assert_eq!(budget.peak(), source_bytes + output_bytes);
        let retained = decoded.mosaic.clone();
        drop(decoded);
        assert_eq!(budget.used(), output_bytes);
        drop(retained);
        assert_eq!(budget.used(), 0);
        request.memory_budget = Some(rrrah_core::MemoryBudget::new(source_bytes - 1));
        assert!(matches!(
            crate::NativeRawDecoder.decode(&request),
            Err(DecodeError::Memory(_))
        ));
        assert_eq!(request.memory_budget.as_ref().unwrap().peak(), 0);
    }
}
