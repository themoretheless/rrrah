//! Streaming lossless GLB snapshots: original container, selected scene and raw pixels.
use crate::glb::{GlbDocument, GlbError, GlbSource, GlbTextureRole};
use crate::{DecodeRequest, GlbAsset, GlbAssetError, GltfTexturePixels};
use rrrah_core::MemoryBudget;
use std::io::{Read, Write};
const MAGIC: &[u8; 8] = b"RRGLB001";
const BLOCK: usize = 16 * 1024;
fn invalid(text: &'static str) -> GlbAssetError {
    GlbError::Invalid(text).into()
}
fn io_read(
    reader: &mut (impl Read + ?Sized),
    out: &mut [u8],
    request: &DecodeRequest,
    mut hash: Option<&mut blake3::Hasher>,
) -> Result<(), GlbAssetError> {
    let mut at = 0;
    while at < out.len() {
        request.check_cancelled()?;
        let end = (at + BLOCK).min(out.len());
        match reader.read(&mut out[at..end]) {
            Ok(0) => return Err(std::io::Error::from(std::io::ErrorKind::UnexpectedEof).into()),
            Ok(n) if n <= end - at => {
                if let Some(h) = &mut hash {
                    h.update(&out[at..at + n]);
                }
                at += n;
            }
            Ok(_) => return Err(invalid("swap reader count")),
            Err(e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
            Err(e) => return Err(e.into()),
        }
    }
    request.check_cancelled()?;
    Ok(())
}
fn io_write(
    writer: &mut (impl Write + ?Sized),
    data: &[u8],
    request: &DecodeRequest,
    mut hash: Option<&mut blake3::Hasher>,
) -> Result<(), GlbAssetError> {
    let mut at = 0;
    while at < data.len() {
        request.check_cancelled()?;
        let end = (at + BLOCK).min(data.len());
        match writer.write(&data[at..end]) {
            Ok(0) => return Err(std::io::Error::from(std::io::ErrorKind::WriteZero).into()),
            Ok(n) if n <= end - at => {
                if let Some(h) = &mut hash {
                    h.update(&data[at..at + n]);
                }
                at += n;
            }
            Ok(_) => return Err(invalid("swap writer count")),
            Err(e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
            Err(e) => return Err(e.into()),
        }
    }
    request.check_cancelled()?;
    Ok(())
}
pub fn glb_asset_swap_payload_len(asset: &GlbAsset) -> Result<u64, GlbAssetError> {
    let mut length = 64u64
        .checked_add(asset.scene().document().source().container_bytes().len() as u64)
        .ok_or_else(|| invalid("swap length"))?;
    for image in asset.images() {
        let image = image.for_role(GlbTextureRole::Normal);
        let bytes = match image.pixels() {
            GltfTexturePixels::Rgba8(p) => p.len() as u64,
            GltfTexturePixels::Rgba16(p) => p.len() as u64 * 2,
        };
        length = length
            .checked_add(32)
            .and_then(|n| n.checked_add(bytes))
            .ok_or_else(|| invalid("swap length"))?;
    }
    Ok(length)
}
/// No complete serialized buffer is created. Fixed 16 KiB stack scratch is used
/// for canonical little-endian u16 conversion. Caller owns disk/quota policy.
pub fn write_glb_asset_swap_payload(
    asset: &GlbAsset,
    writer: &mut (impl Write + ?Sized),
    request: &DecodeRequest,
) -> Result<(), GlbAssetError> {
    request.check_cancelled()?;
    glb_asset_swap_payload_len(asset)?;
    let mut hash = blake3::Hasher::new();
    let source = asset.scene().document().source().container_bytes();
    let mut header = [0u8; 32];
    header[..8].copy_from_slice(MAGIC);
    header[8..16].copy_from_slice(&(asset.scene().scene_index() as u64).to_le_bytes());
    header[16..24].copy_from_slice(&(source.len() as u64).to_le_bytes());
    header[24..32].copy_from_slice(&(asset.images().len() as u64).to_le_bytes());
    io_write(writer, &header, request, Some(&mut hash))?;
    io_write(writer, source, request, Some(&mut hash))?;
    for image in asset.images() {
        request.check_cancelled()?;
        let pixels = image.for_role(GlbTextureRole::Normal);
        let (bits, bytes) = match pixels.pixels() {
            GltfTexturePixels::Rgba8(p) => (8u32, p.len() as u64),
            GltfTexturePixels::Rgba16(p) => (16u32, p.len() as u64 * 2),
        };
        let mut record = [0u8; 32];
        record[..8].copy_from_slice(&(image.source_index as u64).to_le_bytes());
        record[8..12].copy_from_slice(&pixels.width().to_le_bytes());
        record[12..16].copy_from_slice(&pixels.height().to_le_bytes());
        record[16..20].copy_from_slice(&bits.to_le_bytes());
        record[24..32].copy_from_slice(&bytes.to_le_bytes());
        io_write(writer, &record, request, Some(&mut hash))?;
        match pixels.pixels() {
            GltfTexturePixels::Rgba8(p) => io_write(writer, p, request, Some(&mut hash))?,
            GltfTexturePixels::Rgba16(p) => {
                let mut block = [0u8; BLOCK];
                for values in p.chunks(BLOCK / 2) {
                    request.check_cancelled()?;
                    for (value, out) in values.iter().zip(block.chunks_exact_mut(2)) {
                        out.copy_from_slice(&value.to_le_bytes());
                    }
                    io_write(writer, &block[..values.len() * 2], request, Some(&mut hash))?;
                }
            }
        }
    }
    io_write(writer, hash.finalize().as_bytes(), request, None)
}
/// Restore without opening the original path or rerunning PNG/JPEG entropy
/// decoding. All metadata/source framing and snapshot integrity are revalidated.
/// Read exactly `bytes` bytes, leaving following envelope records untouched.
pub fn read_glb_asset_swap_payload(
    reader: &mut (impl Read + ?Sized),
    bytes: u64,
    request: &DecodeRequest,
) -> Result<GlbAsset, GlbAssetError> {
    request.check_cancelled()?;
    if request.image_index != 0 {
        return Err(crate::DecodeError::UnsupportedImageIndex {
            index: request.image_index,
        }
        .into());
    }
    if bytes < 64 {
        return Err(invalid("short GLB swap payload"));
    }
    let mut request = request.clone();
    let budget = request
        .memory_budget
        .clone()
        .unwrap_or_else(|| MemoryBudget::new(1024 * 1024 * 1024));
    request.memory_budget = Some(budget.clone());
    let request = &request;
    let mut reader = reader.take(bytes);
    let mut hash = blake3::Hasher::new();
    let mut header = [0u8; 32];
    io_read(&mut reader, &mut header, request, Some(&mut hash))?;
    if &header[..8] != MAGIC {
        return Err(invalid("GLB swap magic/version"));
    }
    let scene = u64::from_le_bytes(header[8..16].try_into().unwrap());
    let source_len = u64::from_le_bytes(header[16..24].try_into().unwrap());
    let image_count = u64::from_le_bytes(header[24..32].try_into().unwrap());
    if source_len < 12
        || source_len > crate::bounded_io::MAX_INPUT_BYTES
        || image_count > 65536
        || source_len + image_count * 32 + 32 > reader.limit()
    {
        return Err(invalid("GLB swap counts/length"));
    }
    let source_len = usize::try_from(source_len).map_err(|_| invalid("GLB swap source range"))?;
    let mut source = budget.try_buffer(source_len, 0u8)?;
    io_read(&mut reader, &mut source, request, Some(&mut hash))?;
    let document = GlbDocument::from_source(GlbSource::from_managed(source.freeze(), request)?, request)?;
    let scene = usize::try_from(scene).map_err(|_| invalid("GLB swap scene range"))?;
    let result = crate::glb_asset::prepare_glb_asset(
        document,
        Some(scene),
        request,
        &budget,
        Some(image_count as usize),
        |index, mime, encoded, request| {
            let mut record = [0u8; 32];
            io_read(&mut reader, &mut record, request, Some(&mut hash))?;
            let source_index = u64::from_le_bytes(record[..8].try_into().unwrap());
            let width = u32::from_le_bytes(record[8..12].try_into().unwrap());
            let height = u32::from_le_bytes(record[12..16].try_into().unwrap());
            let bits = u32::from_le_bytes(record[16..20].try_into().unwrap());
            let length = u64::from_le_bytes(record[24..32].try_into().unwrap());
            if source_index != index as u64
                || record[20..24] != [0; 4]
                || ![8, 16].contains(&bits)
                || width == 0
                || height == 0
                || width > 65536
                || height > 65536
            {
                return Err(invalid("GLB swap image layout"));
            }
            let samples = u64::from(width) * u64::from(height) * 4;
            if length != samples * u64::from(bits / 8)
                || length > 512 * 1024 * 1024
                || length + 32 > reader.limit()
            {
                return Err(invalid("GLB swap pixel length"));
            }
            let samples = usize::try_from(samples).map_err(|_| invalid("GLB swap pixel range"))?;
            crate::gltf_texture::restored_texture_layout(
                encoded,
                mime,
                width,
                height,
                bits == 16,
                request,
                &budget,
            )?;
            let pixels = if bits == 8 {
                let mut pixels = budget.try_buffer(samples, 0u8)?;
                io_read(&mut reader, &mut pixels, request, Some(&mut hash))?;
                GltfTexturePixels::Rgba8(pixels.freeze().into())
            } else {
                let mut pixels = budget.try_buffer(samples, 0u16)?;
                let mut block = [0u8; BLOCK];
                for values in pixels.chunks_mut(BLOCK / 2) {
                    io_read(
                        &mut reader,
                        &mut block[..values.len() * 2],
                        request,
                        Some(&mut hash),
                    )?;
                    for (value, raw) in values.iter_mut().zip(block.chunks_exact(2)) {
                        *value = u16::from_le_bytes(raw.try_into().unwrap());
                    }
                }
                GltfTexturePixels::Rgba16(pixels.freeze().into())
            };
            Ok(crate::gltf_texture::restored_texture(
                width, height, pixels, request,
            )?)
        },
    )?;
    if reader.limit() != 32 {
        return Err(invalid("GLB swap trailing/missing bytes"));
    }
    let mut digest = [0u8; 32];
    io_read(&mut reader, &mut digest, request, None)?;
    if &digest != hash.finalize().as_bytes() {
        return Err(invalid("GLB swap checksum"));
    }
    request.check_cancelled()?;
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::decode_glb_asset;
    use std::sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    };
    fn root() -> std::path::PathBuf {
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/glb")
    }
    fn request(path: impl Into<std::path::PathBuf>, budget: &MemoryBudget) -> DecodeRequest {
        let mut r = DecodeRequest::new(path.into());
        r.memory_budget = Some(budget.clone());
        r
    }
    fn changed_image(name: &str, mime: &str) -> std::path::PathBuf {
        static SERIAL: AtomicU64 = AtomicU64::new(0);
        let source = std::fs::read(root().join("BoxTextured.glb")).unwrap();
        let n = u32::from_le_bytes(source[12..16].try_into().unwrap()) as usize;
        let mut doc: serde_json::Value = serde_json::from_slice(&source[20..20 + n]).unwrap();
        let mut bin = source[28 + n..].to_vec();
        while bin.len() % 4 != 0 {
            bin.push(0);
        }
        let bytes = std::fs::read(root().join("texture-codec").join(name)).unwrap();
        let view = doc["bufferViews"].as_array().unwrap().len();
        doc["bufferViews"]
            .as_array_mut()
            .unwrap()
            .push(serde_json::json!({"buffer":0,"byteOffset":bin.len(),"byteLength":bytes.len()}));
        bin.extend(bytes);
        doc["buffers"][0]["byteLength"] = bin.len().into();
        while bin.len() % 4 != 0 {
            bin.push(0);
        }
        doc["images"][0]["bufferView"] = view.into();
        doc["images"][0]["mimeType"] = mime.into();
        if name == "pattern.progressive.jpg" {
            doc["images"]
                .as_array_mut()
                .unwrap()
                .insert(0, serde_json::json!({"uri":"unused-source-index-zero.png"}));
            for texture in doc["textures"].as_array_mut().unwrap() {
                let index = texture["source"].as_u64().unwrap();
                texture["source"] = (index + 1).into();
            }
        }
        let mut json = serde_json::to_vec(&doc).unwrap();
        while json.len() % 4 != 0 {
            json.push(b' ');
        }
        let mut out = b"glTF\x02\0\0\0".to_vec();
        out.extend_from_slice(&((28 + json.len() + bin.len()) as u32).to_le_bytes());
        out.extend_from_slice(&(json.len() as u32).to_le_bytes());
        out.extend_from_slice(&0x4e4f534au32.to_le_bytes());
        out.extend(json);
        out.extend_from_slice(&(bin.len() as u32).to_le_bytes());
        out.extend_from_slice(&0x004e4942u32.to_le_bytes());
        out.extend(bin);
        out.extend_from_slice(&4u32.to_le_bytes());
        out.extend_from_slice(b"XTRA");
        out.extend_from_slice(&[0xde, 0xad, 0xbe, 0xef]);
        let total = out.len() as u32;
        out[8..12].copy_from_slice(&total.to_le_bytes());
        let path = std::env::temp_dir().join(format!(
            "rrrah-glb-swap-{}-{}.glb",
            std::process::id(),
            SERIAL.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::write(&path, out).unwrap();
        path
    }
    fn snapshot(asset: &GlbAsset) -> Vec<u8> {
        let mut out = Vec::new();
        write_glb_asset_swap_payload(asset, &mut out, &DecodeRequest::new("snapshot")).unwrap();
        assert_eq!(out.len() as u64, glb_asset_swap_payload_len(asset).unwrap());
        out
    }
    #[test]
    fn raw_8_16_baseline_progressive_and_absent_source_restore_without_codec_calls() {
        let paths = [
            root().join("BoxTextured.glb"),
            changed_image("color6-16.png", "image/png"),
            changed_image("pattern.jpg", "image/jpeg"),
            changed_image("pattern.progressive.jpg", "image/jpeg"),
        ];
        for (i, path) in paths.into_iter().enumerate() {
            let budget = MemoryBudget::new(4 * 1024 * 1024);
            let r = request(&path, &budget);
            let asset = decode_glb_asset(&r, None).unwrap();
            let weight = asset.capacity_bytes();
            let source = asset.scene().document().source().container_bytes().to_vec();
            let bytes = snapshot(&asset);
            if i == 1 {
                let encoded_len = u64::from_le_bytes(bytes[16..24].try_into().unwrap()) as usize;
                assert_eq!(
                    &bytes[32 + encoded_len + 32..bytes.len() - 32],
                    include_bytes!("../../../tests/fixtures/glb/texture-codec/color6-16.png.rgba16le")
                );
            }
            let calls = crate::gltf_texture::codec_calls();
            drop(asset);
            assert_eq!(budget.used(), 0);
            if i != 0 {
                std::fs::remove_file(&path).unwrap();
            }
            let r = request("source-does-not-exist-for-swap-restore.glb", &budget);
            let mut stream = std::io::Cursor::new([bytes.clone(), b"next-record".to_vec()].concat());
            let restored = read_glb_asset_swap_payload(&mut stream, bytes.len() as u64, &r).unwrap();
            assert_eq!(stream.position(), bytes.len() as u64);
            assert_eq!(restored.scene().document().source().container_bytes(), source);
            assert_eq!(
                restored.scene().document().source().layout().ignored_chunks,
                usize::from(i != 0)
            );
            assert_eq!(restored.images()[0].source_index, usize::from(i == 3));
            assert_eq!(restored.capacity_bytes(), weight);
            assert_eq!(budget.used(), weight);
            assert_eq!(snapshot(&restored), bytes);
            assert_eq!(crate::gltf_texture::codec_calls(), calls);
            eprintln!(
                "GLB snapshot case{i}: {} bytes, retained {weight}, zero additional codec calls",
                bytes.len()
            );
            drop(restored);
            assert_eq!(budget.used(), 0);
        }
    }
    #[test]
    fn explicit_nondefault_and_empty_scene_selection_survives_the_snapshot() {
        let budget = MemoryBudget::new(4 * 1024 * 1024);
        let r = request(root().join("scene-controls.glb"), &budget);
        for selection in [0, 3] {
            let asset = decode_glb_asset(&r, Some(selection)).unwrap();
            assert_eq!(asset.scene().scene_index(), selection);
            let bounds = asset.bounds();
            let instances = asset.scene().instances().count();
            let payload = snapshot(&asset);
            drop(asset);
            assert_eq!(budget.used(), 0);
            let restored = read_glb_asset_swap_payload(&mut &payload[..], payload.len() as u64, &r).unwrap();
            assert_eq!(restored.scene().scene_index(), selection);
            assert_eq!(restored.bounds(), bounds);
            assert_eq!(restored.scene().instances().count(), instances);
            assert_eq!(snapshot(&restored), payload);
            drop(restored);
            assert_eq!(budget.used(), 0);
        }
    }
    #[test]
    fn actual_swap_store_streaming_roundtrip_disk_quota_and_last_owners() {
        let budget = MemoryBudget::new(4 * 1024 * 1024);
        let r = request(root().join("BoxTextured.glb"), &budget);
        let asset = decode_glb_asset(&r, None).unwrap();
        let bytes = glb_asset_swap_payload_len(&asset).unwrap();
        let weight = asset.capacity_bytes();
        let store = rrrah_swap::SwapStore::new(
            &std::env::temp_dir(),
            rrrah_swap::SwapLimits {
                max_bytes: bytes,
                max_objects: Some(1),
            },
        )
        .unwrap();
        let handle = store
            .write_stream(
                bytes,
                || false,
                |writer| write_glb_asset_swap_payload(&asset, writer, &r).map_err(std::io::Error::other),
            )
            .unwrap();
        drop(asset);
        assert_eq!(budget.used(), 0);
        assert_eq!(store.usage().unwrap().bytes, bytes);
        assert!(store.write_stream(bytes, || false, |_| Ok(())).is_err());
        let calls = crate::gltf_texture::codec_calls();
        let restored = store
            .read_stream(
                &handle,
                || false,
                |reader| read_glb_asset_swap_payload(reader, bytes, &r).map_err(std::io::Error::other),
            )
            .unwrap();
        assert_eq!(crate::gltf_texture::codec_calls(), calls);
        assert_eq!(restored.capacity_bytes(), weight);
        assert_eq!(budget.used(), weight);
        let held = handle.clone();
        drop(handle);
        assert_eq!(store.usage().unwrap().bytes, bytes);
        drop(held);
        assert_eq!(store.usage().unwrap().bytes, 0);
        assert_eq!(budget.used(), weight);
        drop(restored);
        assert_eq!(budget.used(), 0);
        // A serializer cancellation after writing pixels must roll back disk admission.
        let asset = decode_glb_asset(&r, None).unwrap();
        let token = Arc::new(AtomicU64::new(1));
        let mut cancelled = r.clone();
        cancelled.cancellation = Some(crate::GenerationToken::new(token.clone(), 1));
        struct CancelWrite<'a> {
            sink: &'a mut dyn Write,
            token: Arc<AtomicU64>,
            written: usize,
        }
        impl Write for CancelWrite<'_> {
            fn write(&mut self, b: &[u8]) -> std::io::Result<usize> {
                let n = self.sink.write(b)?;
                self.written += n;
                if self.written >= 32768 {
                    self.token.store(2, Ordering::Release);
                }
                Ok(n)
            }
            fn flush(&mut self) -> std::io::Result<()> {
                self.sink.flush()
            }
        }
        assert!(
            store
                .write_stream(
                    bytes,
                    || false,
                    |sink| write_glb_asset_swap_payload(
                        &asset,
                        &mut CancelWrite {
                            sink,
                            token,
                            written: 0
                        },
                        &cancelled
                    )
                    .map_err(std::io::Error::other)
                )
                .is_err()
        );
        assert_eq!(store.usage().unwrap().bytes, 0);
        assert_eq!(store.usage().unwrap().objects, 0);
        drop(asset);
        assert_eq!(budget.used(), 0);
        eprintln!(
            "GLB snapshot SwapStore disk exact {bytes}; retained RAM {weight}; both roots release to zero"
        );
    }
    #[test]
    fn truncation_checksum_shape_quota_and_interrupted_cancel_never_publish_partial_asset() {
        let probe = MemoryBudget::new(4 * 1024 * 1024);
        let r = request(root().join("BoxTextured.glb"), &probe);
        let asset = decode_glb_asset(&r, None).unwrap();
        let bytes = snapshot(&asset);
        drop(asset);
        assert_eq!(probe.used(), 0);
        // All header prefixes plus cuts at source, pixel and footer boundaries.
        let source = u64::from_le_bytes(bytes[16..24].try_into().unwrap()) as usize;
        let mut cuts: Vec<usize> = (0..64).collect();
        cuts.extend([
            32 + source - 1,
            32 + source,
            32 + source + 31,
            32 + source + 32,
            bytes.len() - 33,
            bytes.len() - 32,
            bytes.len() - 1,
        ]);
        for cut in cuts {
            assert!(
                read_glb_asset_swap_payload(&mut &bytes[..cut], cut as u64, &r).is_err(),
                "cut={cut}"
            );
            assert_eq!(probe.used(), 0);
        }
        for offset in [
            0,
            8,
            16,
            24,
            32 + source,
            32 + source + 8,
            32 + source + 16,
            32 + source + 20,
            32 + source + 24,
            32 + source + 32,
            bytes.len() - 1,
        ] {
            let mut bad = bytes.clone();
            bad[offset] ^= 0x80;
            assert!(
                read_glb_asset_swap_payload(&mut &bad[..], bad.len() as u64, &r).is_err(),
                "offset={offset}"
            );
            assert_eq!(probe.used(), 0);
        }
        let mut mismatch = bytes.clone();
        mismatch[32 + source + 8..32 + source + 12].copy_from_slice(&128u32.to_le_bytes());
        mismatch[32 + source + 12..32 + source + 16].copy_from_slice(&512u32.to_le_bytes());
        let end = mismatch.len() - 32;
        let digest = blake3::hash(&mismatch[..end]);
        mismatch[end..].copy_from_slice(digest.as_bytes());
        assert!(read_glb_asset_swap_payload(&mut &mismatch[..], mismatch.len() as u64, &r).is_err());
        assert_eq!(probe.used(), 0);
        let measure = MemoryBudget::new(4 * 1024 * 1024);
        let rq = request("absent.glb", &measure);
        drop(read_glb_asset_swap_payload(&mut &bytes[..], bytes.len() as u64, &rq).unwrap());
        let peak = measure.peak();
        assert_eq!(measure.used(), 0);
        for (limit, success) in [(peak - 1, false), (peak, true)] {
            let budget = MemoryBudget::new(limit);
            let rq = request("absent.glb", &budget);
            let result = read_glb_asset_swap_payload(&mut &bytes[..], bytes.len() as u64, &rq);
            assert_eq!(result.is_ok(), success);
            drop(result);
            assert_eq!(budget.used(), 0);
        }
        struct Choppy<'a> {
            bytes: &'a [u8],
            interrupt: bool,
        }
        impl Read for Choppy<'_> {
            fn read(&mut self, b: &mut [u8]) -> std::io::Result<usize> {
                self.interrupt = !self.interrupt;
                if self.interrupt {
                    return Err(std::io::ErrorKind::Interrupted.into());
                }
                let n = b.len().min(37);
                self.bytes.read(&mut b[..n])
            }
        }
        let restored = read_glb_asset_swap_payload(
            &mut Choppy {
                bytes: &bytes,
                interrupt: false,
            },
            bytes.len() as u64,
            &r,
        )
        .unwrap();
        assert_eq!(snapshot(&restored), bytes);
        drop(restored);
        assert_eq!(probe.used(), 0);
        struct ChoppyWrite {
            out: Vec<u8>,
            interrupt: bool,
        }
        impl Write for ChoppyWrite {
            fn write(&mut self, b: &[u8]) -> std::io::Result<usize> {
                self.interrupt = !self.interrupt;
                if self.interrupt {
                    return Err(std::io::ErrorKind::Interrupted.into());
                }
                let n = b.len().min(53);
                self.out.extend_from_slice(&b[..n]);
                Ok(n)
            }
            fn flush(&mut self) -> std::io::Result<()> {
                Ok(())
            }
        }
        let asset = decode_glb_asset(&r, None).unwrap();
        let mut short = ChoppyWrite {
            out: Vec::new(),
            interrupt: false,
        };
        write_glb_asset_swap_payload(&asset, &mut short, &r).unwrap();
        assert_eq!(short.out, bytes);
        drop(asset);
        assert_eq!(probe.used(), 0);
        let token = Arc::new(AtomicU64::new(1));
        let mut cancelled = r.clone();
        cancelled.cancellation = Some(crate::GenerationToken::new(token.clone(), 1));
        struct CancelRead<'a> {
            bytes: &'a [u8],
            token: Arc<AtomicU64>,
            consumed: usize,
        }
        impl Read for CancelRead<'_> {
            fn read(&mut self, b: &mut [u8]) -> std::io::Result<usize> {
                let n = b.len().min(37);
                let read = self.bytes.read(&mut b[..n])?;
                self.consumed += read;
                if self.consumed >= 32768 {
                    self.token.store(2, Ordering::Release);
                }
                Ok(read)
            }
        }
        assert!(
            read_glb_asset_swap_payload(
                &mut CancelRead {
                    bytes: &bytes,
                    token,
                    consumed: 0
                },
                bytes.len() as u64,
                &cancelled
            )
            .is_err()
        );
        assert_eq!(probe.used(), 0);
        let mut output = Vec::new();
        assert!(
            write_glb_asset_swap_payload(&decode_glb_asset(&r, None).unwrap(), &mut output, &cancelled)
                .is_err()
        );
        assert!(output.is_empty());
        assert_eq!(probe.used(), 0);
        eprintln!(
            "GLB snapshot restore managed peak {peak}; one-byte refusal, truncation/corruption/interrupted reads/cancel cleanup passed"
        );
    }
}
