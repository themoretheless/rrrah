//! Native GLB 2 container and bounded JSON document. Scene semantics are a separate layer.
//! BIN and JSON remain slices of one immutable, optionally admitted allocation.
use crate::{DecodeError, DecodeRequest};
use std::ops::Range;
use thiserror::Error;

const JSON: u32 = 0x4e4f534a;
const BIN: u32 = 0x004e4942;
pub const MAX_GLB_JSON_BYTES: usize = 16 * 1024 * 1024;
pub const MAX_GLB_CHUNKS: usize = 65_536;

#[derive(Debug, Error)]
pub enum GlbError {
    #[error(transparent)]
    Source(#[from] DecodeError),
    #[error("invalid GLB container: {0}")]
    Invalid(&'static str),
    #[error("unsupported GLB container version: {0}")]
    Version(u32),
    #[error("unsupported glTF: {0}")]
    Unsupported(&'static str),
    #[error("GLB resource limit: {0}")]
    Limit(&'static str),
}

/// Byte ranges validated against the complete container, not JSON semantics.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GlbLayout {
    pub json: Range<usize>,
    pub bin: Option<Range<usize>>,
    pub ignored_chunks: usize,
}
impl GlbLayout {
    /// No heap allocation or BIN traversal. Unknown chunks are skipped only
    /// after validating their framing, and may not displace JSON/BIN ordering.
    pub fn parse(bytes: &[u8], request: &DecodeRequest) -> Result<Self, GlbError> {
        request.check_cancelled()?;
        if bytes.len() < 12 || &bytes[..4] != b"glTF" {
            return Err(GlbError::Invalid("missing header or magic"));
        }
        let word = |at: usize| u32::from_le_bytes(bytes[at..at + 4].try_into().unwrap());
        let version = word(4);
        if version != 2 {
            return Err(GlbError::Version(version));
        }
        if u64::from(word(8)) != bytes.len() as u64 {
            return Err(GlbError::Invalid("declared length differs from source"));
        }
        let mut at = 12;
        let mut chunks = 0;
        let mut json = None;
        let mut bin = None;
        let mut ignored_chunks = 0;
        while at < bytes.len() {
            request.check_cancelled()?;
            if chunks == MAX_GLB_CHUNKS {
                return Err(GlbError::Limit("chunk count"));
            }
            let header_end = at
                .checked_add(8)
                .filter(|&v| v <= bytes.len())
                .ok_or(GlbError::Invalid("truncated chunk header"))?;
            let length = word(at) as usize;
            let kind = word(at + 4);
            if length % 4 != 0 {
                return Err(GlbError::Invalid("unaligned chunk"));
            }
            let end = header_end
                .checked_add(length)
                .filter(|&v| v <= bytes.len())
                .ok_or(GlbError::Invalid("truncated chunk payload"))?;
            if chunks == 0 && kind != JSON {
                return Err(GlbError::Invalid("JSON must be first"));
            }
            match kind {
                JSON => {
                    if chunks != 0 {
                        return Err(GlbError::Invalid("duplicate or misplaced JSON"));
                    }
                    if length == 0 {
                        return Err(GlbError::Invalid("empty JSON chunk"));
                    }
                    if length > MAX_GLB_JSON_BYTES {
                        return Err(GlbError::Limit("JSON bytes"));
                    }
                    json = Some(header_end..end);
                }
                BIN => {
                    if chunks != 1 {
                        return Err(GlbError::Invalid("duplicate or misplaced BIN"));
                    }
                    bin = Some(header_end..end);
                }
                _ => ignored_chunks += 1,
            }
            chunks += 1;
            at = end;
        }
        request.check_cancelled()?;
        Ok(Self {
            json: json.ok_or(GlbError::Invalid("missing JSON chunk"))?,
            bin,
            ignored_chunks,
        })
    }
}

/// Foundation for the forthcoming scene decoder. Not registered as a model
/// format: validating this container does not validate or render a glTF asset.
#[derive(Debug, Clone)]
pub struct GlbSource {
    bytes: rrrah_core::PixelBuffer<u8>,
    layout: GlbLayout,
}
impl GlbSource {
    pub fn read(request: &DecodeRequest) -> Result<Self, GlbError> {
        request.check_cancelled()?;
        if request.image_index != 0 {
            return Err(DecodeError::UnsupportedImageIndex {
                index: request.image_index,
            }
            .into());
        }
        let bytes = crate::bounded_io::read_managed(request)?;
        let layout = GlbLayout::parse(&bytes, request)?;
        Ok(Self { bytes, layout })
    }
    /// Original JSON bytes including padding. This container-only accessor
    /// does not validate JSON; `GlbDocument` validates syntax and asset version.
    /// Scene/accessor schema validation remains a subsequent layer.
    pub fn json_bytes(&self) -> &[u8] {
        &self.bytes[self.layout.json.clone()]
    }
    /// Includes up to three padding bytes. The scene layer must check declared
    /// buffer.byteLength and zero padding before exposing accessor storage.
    pub fn bin_bytes(&self) -> Option<&[u8]> {
        self.layout.bin.clone().map(|range| &self.bytes[range])
    }
    pub fn layout(&self) -> &GlbLayout {
        &self.layout
    }
}

pub use crate::glb_accessor::{GlbAccessor, GlbComponent, GlbShape, MAX_ACCESSOR_ELEMENTS};
pub use crate::glb_json::{JsonArray, JsonKind, JsonObject, JsonValue};
pub use crate::glb_material::{
    GlbAlphaMode, GlbImageMime, GlbImageSource, GlbMaterial, GlbMaterialSamples, GlbNormalTexture,
    GlbOcclusionTexture, GlbSampler, GlbSurface, GlbTexture, GlbTextureBinding, GlbTextureRole,
};
pub use crate::glb_primitive::{GlbPrimitive, GlbTopology};
pub use crate::glb_scene::{GlbInstance, GlbScene, GlbScenePrimitive};
pub use crate::glb_transform::GlbTransform;
pub use crate::glb_vertex::{GlbVertex, GlbVertexStreams};

/// Syntactically validated, admitted JSON document over an immutable GLB source.
/// Scene/accessor semantics are not yet a decoded model.
#[derive(Debug, Clone)]
pub struct GlbDocument {
    source: GlbSource,
    json: crate::glb_json::JsonTape,
}
impl GlbDocument {
    pub fn read(request: &DecodeRequest) -> Result<Self, GlbError> {
        let source = GlbSource::read(request)?;
        let json = crate::glb_json::JsonTape::parse(source.json_bytes(), request)?;
        let document = Self { source, json };
        let root = document.root();
        let asset = root
            .field("asset", request)?
            .ok_or(GlbError::Invalid("missing asset"))?;
        let version = asset
            .field("version", request)?
            .ok_or(GlbError::Invalid("missing asset.version"))?;
        if !version.string_eq("2.0") {
            return Err(GlbError::Unsupported("asset.version"));
        }
        if let Some(minimum) = asset.field("minVersion", request)? {
            if !minimum.string_eq("2.0") {
                return Err(GlbError::Unsupported("asset.minVersion"));
            }
        }
        if let Some(required) = root.field("extensionsRequired", request)? {
            let mut required = required
                .array()
                .ok_or(GlbError::Invalid("extensionsRequired array"))?;
            if required.next().is_some() {
                return Err(GlbError::Unsupported("required extensions"));
            }
        }
        request.check_cancelled()?;
        Ok(document)
    }
    pub(crate) fn json_at(&self, index: u32) -> JsonValue<'_> {
        self.json
            .at(self.source.json_bytes(), index)
            .expect("validated source token handle")
    }
    pub fn root(&self) -> JsonValue<'_> {
        self.json.root(self.source.json_bytes())
    }
    pub fn source(&self) -> &GlbSource {
        &self.source
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn container(chunks: &[(u32, &[u8])]) -> Vec<u8> {
        let mut data = b"glTF\x02\0\0\0\0\0\0\0".to_vec();
        for (kind, payload) in chunks {
            data.extend_from_slice(&(payload.len() as u32).to_le_bytes());
            data.extend_from_slice(&kind.to_le_bytes());
            data.extend_from_slice(payload);
        }
        let length = data.len() as u32;
        data[8..12].copy_from_slice(&length.to_le_bytes());
        data
    }
    fn parse(bytes: &[u8]) -> Result<GlbLayout, GlbError> {
        GlbLayout::parse(bytes, &DecodeRequest::new("unused.glb"))
    }
    #[test]
    fn original_box_document_admits_exact_source_and_native_token_tape() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/glb/Box.glb");
        // Independent Python census: 165 tokens; native fixed token is 16 bytes.
        let budget = rrrah_core::MemoryBudget::new(4304);
        let mut request = DecodeRequest::new(path);
        request.memory_budget = Some(budget.clone());
        let document = GlbDocument::read(&request).unwrap();
        assert_eq!(budget.used(), 4304);
        let accessors = document.root().field("accessors", &request).unwrap().unwrap();
        let mut accessors = accessors.array().unwrap();
        assert_eq!(
            accessors
                .next()
                .unwrap()
                .field("count", &request)
                .unwrap()
                .unwrap()
                .as_u64(),
            Some(36)
        );
        assert_eq!(
            accessors
                .next()
                .unwrap()
                .field("count", &request)
                .unwrap()
                .unwrap()
                .as_u64(),
            Some(24)
        );
        assert_eq!(
            accessors
                .next()
                .unwrap()
                .field("count", &request)
                .unwrap()
                .unwrap()
                .as_u64(),
            Some(24)
        );
        assert!(accessors.next().is_none());
        let clone = document.clone();
        drop(document);
        assert_eq!(budget.used(), 4304);
        drop(clone);
        assert_eq!(budget.used(), 0);
        request.memory_budget = Some(rrrah_core::MemoryBudget::new(4303));
        assert!(matches!(
            GlbDocument::read(&request),
            Err(GlbError::Source(DecodeError::Memory(_)))
        ));
        let budget = request.memory_budget.as_ref().unwrap();
        assert_eq!(budget.used(), 0);
        assert_eq!(budget.peak(), 1664);
    }
    #[test]
    fn document_version_schema_failures_release_source_and_json_credit() {
        let path = std::env::temp_dir().join(format!("rrrah-glb-document-{}.glb", std::process::id()));
        let budget = rrrah_core::MemoryBudget::new(4096);
        let mut request = DecodeRequest::new(&path);
        request.memory_budget = Some(budget.clone());
        for (text, valid) in [
            (r#"{"asset":{"version":"2.0"}}"#, true),
            (
                r#"{"asset":{"version":"1.0","\u0076ersion":"2.0"},"extensionsRequired":[]}"#,
                true,
            ),
            (r#"{"asset":{"version":"2.0","minVersion":"2.0"}}"#, true),
            (r#"{"asset":{"version":"2.1"}}"#, false),
            (r#"{"asset":{"version":"2.0","minVersion":"2.1"}}"#, false),
            (
                r#"{"asset":{"version":"2.0"},"extensionsRequired":["KHR_unknown"]}"#,
                false,
            ),
            (r#"{"asset":{"version":"2.0"},"extensionsRequired":{}}"#, false),
            (r#"{"asset":1}"#, false),
            (r#"{"asset":{}}"#, false),
            ("[]", false),
            (r#"{"asset":{"version":"2.0"},}"#, false),
        ] {
            let mut bytes = text.as_bytes().to_vec();
            while bytes.len() % 4 != 0 {
                bytes.push(b' ');
            }
            std::fs::write(&path, container(&[(JSON, &bytes)])).unwrap();
            let result = GlbDocument::read(&request);
            assert_eq!(result.is_ok(), valid, "{text}: {result:?}");
            drop(result);
            assert_eq!(budget.used(), 0);
        }
        std::fs::remove_file(path).unwrap();
    }

    #[test]
    fn original_khronos_box_matches_independent_container_ranges() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/glb/Box.glb");
        let budget = rrrah_core::MemoryBudget::new(1664);
        let mut request = DecodeRequest::new(path);
        request.memory_budget = Some(budget.clone());
        let source = GlbSource::read(&request).unwrap();
        assert_eq!(
            source.layout(),
            &GlbLayout {
                json: 20..1008,
                bin: Some(1016..1664),
                ignored_chunks: 0
            }
        );
        assert_eq!(source.json_bytes().len(), 988);
        assert_eq!(source.bin_bytes().unwrap().len(), 648);
        assert_eq!(budget.used(), 1664);
        let clone = source.clone();
        drop(source);
        assert_eq!(budget.used(), 1664);
        drop(clone);
        assert_eq!(budget.used(), 0);
        request.memory_budget = Some(rrrah_core::MemoryBudget::new(1663));
        assert!(matches!(
            GlbSource::read(&request),
            Err(GlbError::Source(DecodeError::Memory(_)))
        ));
        assert_eq!(request.memory_budget.as_ref().unwrap().used(), 0);
    }

    #[test]
    fn json_bin_and_unknown_chunks_keep_exact_borrowed_ranges() {
        let json = b"{\"asset\":{\"version\":\"2.0\"}} ";
        assert_eq!(json.len() % 4, 0);
        let data = container(&[
            (JSON, json),
            (BIN, &[1, 2, 3, 0]),
            (123, &[5, 6, 7, 8]),
            (456, &[]),
        ]);
        let layout = parse(&data).unwrap();
        assert_eq!(&data[layout.json], json);
        assert_eq!(&data[layout.bin.unwrap()], &[1, 2, 3, 0]);
        assert_eq!(layout.ignored_chunks, 2);
        let layout = parse(&container(&[(JSON, b"{}  "), (123, &[])])).unwrap();
        assert_eq!(layout.bin, None);
        assert_eq!(layout.ignored_chunks, 1);
    }
    #[test]
    fn malformed_order_alignment_versions_and_all_truncated_prefixes_reject() {
        let valid = container(&[(JSON, b"{}  "), (BIN, &[0; 4])]);
        for end in 0..valid.len() {
            assert!(parse(&valid[..end]).is_err(), "prefix {end}");
        }
        for chunks in [
            vec![],
            vec![(BIN, &b"    "[..])],
            vec![(JSON, &b"{}  "[..]), (JSON, &b"{}  "[..])],
            vec![(JSON, &b"{}  "[..]), (123, &b""[..]), (BIN, &b""[..])],
            vec![(JSON, &b"{}  "[..]), (BIN, &b""[..]), (BIN, &b""[..])],
            vec![(JSON, &b"{}"[..])],
            vec![(JSON, &b""[..])],
        ] {
            assert!(parse(&container(&chunks)).is_err());
        }
        let mut bad = valid.clone();
        bad[4..8].copy_from_slice(&1u32.to_le_bytes());
        assert!(matches!(parse(&bad), Err(GlbError::Version(1))));
        let mut bad = valid.clone();
        bad[12..16].copy_from_slice(&u32::MAX.to_le_bytes());
        assert!(parse(&bad).is_err());
        let mut bad = valid;
        bad.push(0);
        assert!(parse(&bad).is_err());
    }
    #[test]
    fn limits_and_cancellation_precede_publication() {
        let mut chunks = vec![(123, &b""[..]); MAX_GLB_CHUNKS];
        chunks[0] = (JSON, b"{}  ");
        assert!(parse(&container(&chunks)).is_ok());
        chunks.push((123, b""));
        assert!(matches!(
            parse(&container(&chunks)),
            Err(GlbError::Limit("chunk count"))
        ));
        assert!(matches!(
            parse(&container(&[(JSON, &vec![b' '; MAX_GLB_JSON_BYTES + 4])])),
            Err(GlbError::Limit("JSON bytes"))
        ));
        let mut request = DecodeRequest::new("unused.glb");
        request.cancellation = Some(crate::GenerationToken::new(
            std::sync::Arc::new(std::sync::atomic::AtomicU64::new(2)),
            1,
        ));
        assert!(matches!(
            GlbLayout::parse(&[], &request),
            Err(GlbError::Source(DecodeError::Cancelled))
        ));
        assert!(matches!(
            GlbSource::read(&request),
            Err(GlbError::Source(DecodeError::Cancelled))
        ));
    }
    #[test]
    fn admitted_source_clones_hold_one_credit_and_failures_release_it() {
        let path = std::env::temp_dir().join(format!("rrrah-glb-source-{}.glb", std::process::id()));
        let data = container(&[(JSON, b"{}  "), (BIN, &[3, 2, 1, 0])]);
        std::fs::write(&path, &data).unwrap();
        let budget = rrrah_core::MemoryBudget::new(data.len() as u64);
        let mut request = DecodeRequest::new(&path);
        request.memory_budget = Some(budget.clone());
        let source = GlbSource::read(&request).unwrap();
        assert_eq!(source.json_bytes(), b"{}  ");
        assert_eq!(source.bin_bytes(), Some(&[3, 2, 1, 0][..]));
        assert_eq!(budget.used(), data.len() as u64);
        let clone = source.clone();
        assert_eq!(
            source.bin_bytes().unwrap().as_ptr(),
            clone.bin_bytes().unwrap().as_ptr()
        );
        drop(source);
        assert_eq!(budget.used(), data.len() as u64);
        drop(clone);
        assert_eq!(budget.used(), 0);
        request.memory_budget = Some(rrrah_core::MemoryBudget::new(data.len() as u64 - 1));
        assert!(GlbSource::read(&request).is_err());
        assert_eq!(request.memory_budget.as_ref().unwrap().used(), 0);
        request.memory_budget = Some(budget.clone());
        let mut bad = data;
        bad[0] = 0;
        std::fs::write(&path, bad).unwrap();
        assert!(GlbSource::read(&request).is_err());
        assert_eq!(budget.used(), 0);
        std::fs::remove_file(path).unwrap();
    }
}
