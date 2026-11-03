//! glTF-specific PNG/JPEG pixels: no display ICC/gamma/EXIF interpretation.
use crate::{DecodeError, DecodeRequest};
use image::{ColorType, ImageDecoder, Limits};
use rrrah_core::{MemoryBudget, PixelBuffer};
use rrrah_mesh::glb::{GlbDocument, GlbImageMime, GlbImageSource, GlbTextureRole};
use thiserror::Error;
#[path = "gltf_texture_source.rs"]
mod source;
#[cfg(test)]
std::thread_local! {static CODEC_CALLS:std::cell::Cell<u64>=const {std::cell::Cell::new(0)};}
#[cfg(test)]
pub(crate) fn codec_calls() -> u64 {
    CODEC_CALLS.with(std::cell::Cell::get)
}
const MAX_PIXELS_BYTES: u64 = 512 * 1024 * 1024;
#[derive(Debug, Error)]
pub enum GltfTextureError {
    #[error(transparent)]
    Source(#[from] DecodeError),
    #[error(transparent)]
    Container(#[from] rrrah_mesh::glb::GlbError),
    #[error(transparent)]
    Memory(#[from] rrrah_core::BufferError),
    #[error(transparent)]
    Codec(#[from] image::ImageError),
    #[error("invalid glTF texture: {0}")]
    Invalid(&'static str),
    #[error("glTF texture resource limit: {0}")]
    Limit(&'static str),
    #[error("unsupported glTF texture: {0}")]
    Unsupported(&'static str),
}
#[derive(Debug, Clone)]
pub enum GltfTexturePixels {
    Rgba8(PixelBuffer<u8>),
    Rgba16(PixelBuffer<u16>),
}
#[derive(Debug, Clone)]
pub struct GltfTextureImage {
    width: u32,
    height: u32,
    role: GlbTextureRole,
    pixels: GltfTexturePixels,
}
impl GltfTextureImage {
    pub fn width(&self) -> u32 {
        self.width
    }
    pub fn height(&self) -> u32 {
        self.height
    }
    pub fn role(&self) -> GlbTextureRole {
        self.role
    }
    pub fn pixels(&self) -> &GltfTexturePixels {
        &self.pixels
    }
    /// Retag shared raw pixels; color roles stay encoded until GPU interpretation.
    pub fn with_role(&self, role: GlbTextureRole) -> Self {
        Self { role, ..self.clone() }
    }
    pub fn texel(&self, x: u32, y: u32) -> Option<[f64; 4]> {
        if x >= self.width || y >= self.height {
            return None;
        }
        let at = (y as usize * self.width as usize + x as usize) * 4;
        Some(match &self.pixels {
            GltfTexturePixels::Rgba8(p) => std::array::from_fn(|c| f64::from(p[at + c]) / 255.),
            GltfTexturePixels::Rgba16(p) => std::array::from_fn(|c| f64::from(p[at + c]) / 65535.),
        })
    }
}
/// Admit source framing/metadata and validate a cached pixel layout without
/// constructing an entropy decoder. Snapshot integrity is checked by its owner.
pub(crate) fn restored_texture_layout(
    bytes: &[u8],
    mime: GlbImageMime,
    width: u32,
    height: u32,
    sixteen: bool,
    request: &DecodeRequest,
    budget: &MemoryBudget,
) -> Result<(), GltfTextureError> {
    let (_segments, header, _) = source::prepare(bytes, mime, request, budget)?;
    if width == 0
        || height == 0
        || width > 65536
        || height > 65536
        || width != header.width
        || height != header.height
        || sixteen != header.sixteen
    {
        return Err(GltfTextureError::Invalid("cached texture/source layout"));
    }
    request.check_cancelled()?;
    Ok(())
}
pub(crate) fn restored_texture(
    width: u32,
    height: u32,
    pixels: GltfTexturePixels,
    request: &DecodeRequest,
) -> Result<GltfTextureImage, GltfTextureError> {
    let (samples, sixteen) = match &pixels {
        GltfTexturePixels::Rgba8(p) => (p.len(), false),
        GltfTexturePixels::Rgba16(p) => (p.len(), true),
    };
    if width == 0
        || height == 0
        || width > 65536
        || height > 65536
        || u64::from(width) * u64::from(height) * 4 != samples as u64
        || samples as u64 * if sixteen { 2 } else { 1 } > MAX_PIXELS_BYTES
    {
        return Err(GltfTextureError::Invalid("cached texture pixel layout"));
    }
    request.check_cancelled()?;
    Ok(GltfTextureImage {
        width,
        height,
        role: GlbTextureRole::Normal,
        pixels,
    })
}
pub fn decode_glb_texture(
    document: &GlbDocument,
    texture: usize,
    role: GlbTextureRole,
    request: &DecodeRequest,
) -> Result<GltfTextureImage, GltfTextureError> {
    request.check_cancelled()?;
    let texture = document.texture(texture, request)?;
    match document.image_source(texture.image, request)? {
        GlbImageSource::Embedded { mime, bytes } => decode_gltf_texture_bytes(bytes, mime, role, request),
        GlbImageSource::Uri { .. } => Err(GltfTextureError::Unsupported(
            "external/data URI image resolution",
        )),
    }
}
/// Raw encoded/data samples are preserved. Backend codec scratch is separately
/// limited by codec/dimension caps; it is not claimed as complete root accounting.
pub fn decode_gltf_texture_bytes(
    bytes: &[u8],
    mime: GlbImageMime,
    role: GlbTextureRole,
    request: &DecodeRequest,
) -> Result<GltfTextureImage, GltfTextureError> {
    #[cfg(test)]
    CODEC_CALLS.with(|count| count.set(count.get() + 1));
    request.check_cancelled()?;
    if request.image_index != 0 {
        return Err(DecodeError::UnsupportedImageIndex {
            index: request.image_index,
        }
        .into());
    }
    let budget = request.memory_budget.clone().unwrap_or_else(|| {
        MemoryBudget::new(crate::bounded_io::MAX_INPUT_BYTES + 2 * MAX_PIXELS_BYTES + 2 * 1024 * 1024)
    });
    let (segments, header, filtered_length) = source::prepare(bytes, mime, request, &budget)?;
    if header.width == 0 {
        return Err(GltfTextureError::Invalid("zero image width"));
    }
    if header.height == 0 {
        return Err(if mime == GlbImageMime::Jpeg {
            GltfTextureError::Unsupported("JPEG deferred height/DNL")
        } else {
            GltfTextureError::Invalid("zero image height")
        });
    }
    if header.width > 65536 || header.height > 65536 {
        return Err(GltfTextureError::Limit("image dimensions"));
    }
    let samples = u64::from(header.width) * u64::from(header.height) * 4;
    let output_bytes = samples * if header.sixteen { 2 } else { 1 };
    if output_bytes > MAX_PIXELS_BYTES {
        return Err(GltfTextureError::Limit("decoded image bytes"));
    }
    let output_credit = budget.try_reserve(output_bytes)?;
    let mut input_credit = if mime == GlbImageMime::Jpeg {
        Some(budget.try_reserve(filtered_length as u64)?)
    } else {
        None
    };
    let reader = source::FilteredSource::new(
        bytes,
        segments,
        filtered_length,
        request.cancellation.clone(),
        input_credit.as_mut(),
    );
    let mut limits = Limits::default();
    limits.max_alloc = Some(MAX_PIXELS_BYTES);
    limits.max_image_width = Some(65536);
    limits.max_image_height = Some(65536);
    let decoder: Box<dyn ImageDecoder + '_> = match mime {
        GlbImageMime::Png => Box::new(
            image::codecs::png::PngDecoder::with_limits_without_text_or_icc(reader, limits.clone())
                .map_err(|e| codec_error(e, request))?,
        ),
        GlbImageMime::Jpeg => {
            let mut decoder =
                image::codecs::jpeg::JpegDecoder::new(reader).map_err(|e| codec_error(e, request))?;
            decoder.set_limits(limits)?;
            Box::new(decoder)
        }
    };
    request.check_cancelled()?;
    if decoder.dimensions() != (header.width, header.height) {
        return Err(GltfTextureError::Invalid("codec/header dimensions disagree"));
    }
    let color = decoder.color_type();
    let channels = match color {
        ColorType::L8 | ColorType::L16 => 1,
        ColorType::La8 | ColorType::La16 => 2,
        ColorType::Rgb8 | ColorType::Rgb16 => 3,
        ColorType::Rgba8 | ColorType::Rgba16 => 4,
        _ => return Err(GltfTextureError::Unsupported("codec sample layout")),
    };
    let sixteen = matches!(
        color,
        ColorType::L16 | ColorType::La16 | ColorType::Rgb16 | ColorType::Rgba16
    );
    if sixteen != header.sixteen {
        return Err(GltfTextureError::Invalid("codec/header depth disagree"));
    }
    let pixels = if color == ColorType::Rgba8 {
        let mut output = output_credit.try_buffer(samples as usize, 0u8)?;
        decoder
            .read_image(&mut output)
            .map_err(|e| codec_error(e, request))?;
        request.check_cancelled()?;
        GltfTexturePixels::Rgba8(output.freeze().into())
    } else {
        let mut native = budget.try_buffer(decoder.total_bytes() as usize, 0u8)?;
        decoder
            .read_image(&mut native)
            .map_err(|e| codec_error(e, request))?;
        request.check_cancelled()?;
        if sixteen {
            let mut out = output_credit.try_buffer(samples as usize, 0u16)?;
            for (i, pixel) in out.chunks_exact_mut(4).enumerate() {
                if i % 4096 == 0 {
                    request.check_cancelled()?;
                }
                let read = |c| {
                    let at = (i * channels + c) * 2;
                    u16::from_ne_bytes([native[at], native[at + 1]])
                };
                let rgb = if channels <= 2 {
                    [read(0); 3]
                } else {
                    [read(0), read(1), read(2)]
                };
                pixel[..3].copy_from_slice(&rgb);
                pixel[3] = if channels == 2 {
                    read(1)
                } else if channels == 4 {
                    read(3)
                } else {
                    u16::MAX
                };
            }
            GltfTexturePixels::Rgba16(out.freeze().into())
        } else {
            let mut out = output_credit.try_buffer(samples as usize, 0u8)?;
            for (i, pixel) in out.chunks_exact_mut(4).enumerate() {
                if i % 4096 == 0 {
                    request.check_cancelled()?;
                }
                let at = i * channels;
                let rgb = if channels <= 2 {
                    [native[at]; 3]
                } else {
                    [native[at], native[at + 1], native[at + 2]]
                };
                pixel[..3].copy_from_slice(&rgb);
                pixel[3] = if channels == 2 {
                    native[at + 1]
                } else if channels == 4 {
                    native[at + 3]
                } else {
                    255
                };
            }
            GltfTexturePixels::Rgba8(out.freeze().into())
        }
    };
    request.check_cancelled()?;
    Ok(GltfTextureImage {
        width: header.width,
        height: header.height,
        role,
        pixels,
    })
}
fn codec_error(error: image::ImageError, request: &DecodeRequest) -> GltfTextureError {
    match request.check_cancelled() {
        Err(error) => GltfTextureError::Source(error),
        Ok(()) => GltfTextureError::Codec(error),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn root() -> std::path::PathBuf {
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/glb")
    }
    fn request(budget: &MemoryBudget) -> DecodeRequest {
        let mut r = DecodeRequest::new("unopened-glb-texture");
        r.memory_budget = Some(budget.clone());
        r
    }
    #[test]
    fn png_jpeg_and_ignored_metadata_preserve_independent_raw_samples() {
        let folder = root().join("texture-codec");
        let budget = MemoryBudget::new(16 * 1024 * 1024);
        let request = request(&budget);
        let mut count = 0;
        for line in std::fs::read_to_string(folder.join("cases.tsv")).unwrap().lines() {
            let f: Vec<_> = line.split('\t').collect();
            let data = std::fs::read(folder.join(f[0])).unwrap();
            let expected = std::fs::read(folder.join(f[1])).unwrap();
            let mime = if f[0].ends_with(".png") {
                GlbImageMime::Png
            } else {
                GlbImageMime::Jpeg
            };
            let image = decode_gltf_texture_bytes(&data, mime, GlbTextureRole::BaseColor, &request).unwrap();
            assert_eq!(image.width(), f[2].parse::<u32>().unwrap());
            assert_eq!(image.height(), f[3].parse::<u32>().unwrap());
            let depth = f[4].parse::<u8>().unwrap();
            let tolerance = f[5].parse::<i32>().unwrap();
            match image.pixels() {
                GltfTexturePixels::Rgba8(p) => {
                    assert_eq!(depth, 8);
                    assert_eq!(p.len(), expected.len());
                    for (i, (actual, expected)) in p.iter().zip(&expected).enumerate() {
                        assert!(
                            (i32::from(*actual) - i32::from(*expected)).abs() <= tolerance,
                            "{} sample{i}: {actual}/{expected}",
                            f[0]
                        );
                    }
                    assert_eq!(budget.used(), p.len() as u64);
                }
                GltfTexturePixels::Rgba16(p) => {
                    assert_eq!(depth, 16);
                    assert_eq!(p.len() * 2, expected.len());
                    for (actual, bytes) in p.iter().zip(expected.chunks_exact(2)) {
                        assert_eq!(*actual, u16::from_le_bytes(bytes.try_into().unwrap()));
                    }
                    assert_eq!(budget.used(), (p.len() * 2) as u64);
                }
            }
            let other = image.with_role(GlbTextureRole::Normal);
            assert_eq!(other.role(), GlbTextureRole::Normal);
            assert_eq!(image.texel(0, 0), other.texel(0, 0));
            match (image.pixels(), other.pixels()) {
                (GltfTexturePixels::Rgba8(a), GltfTexturePixels::Rgba8(b)) => {
                    assert_eq!(a.as_ptr(), b.as_ptr())
                }
                (GltfTexturePixels::Rgba16(a), GltfTexturePixels::Rgba16(b)) => {
                    assert_eq!(a.as_ptr(), b.as_ptr())
                }
                _ => panic!(),
            };
            let used = budget.used();
            drop(image);
            assert_eq!(budget.used(), used);
            drop(other);
            assert_eq!(budget.used(), 0);
            count += 1;
        }
        assert_eq!(count, 15);
    }
    #[test]
    fn original_glb_texture_decodes_all_producer_pixels_and_retains_only_output() {
        let budget = MemoryBudget::new(1024 * 1024);
        let mut request = request(&budget);
        request.path = root().join("BoxTextured.glb");
        let document = GlbDocument::read(&request).unwrap();
        let source = budget.used();
        let image = decode_glb_texture(&document, 0, GlbTextureRole::BaseColor, &request).unwrap();
        assert_eq!((image.width(), image.height()), (256, 256));
        let GltfTexturePixels::Rgba8(p) = image.pixels() else {
            panic!()
        };
        let reference = std::fs::read(root().join("texture-codec/BoxTextured.rgba")).unwrap();
        assert_eq!(&p[..], reference);
        assert_eq!(budget.used(), source + 262144);
        drop(document);
        assert_eq!(budget.used(), 262144);
        drop(image);
        assert_eq!(budget.used(), 0);
    }
    #[test]
    fn exact_owned_buffer_budget_refusal_cancel_and_retry_release_all_transients() {
        let data = std::fs::read(root().join("texture-codec/rgba8.png")).unwrap();
        let budget = MemoryBudget::new(120);
        let mut request = request(&budget);
        let image =
            decode_gltf_texture_bytes(&data, GlbImageMime::Png, GlbTextureRole::Normal, &request).unwrap();
        assert_eq!(budget.peak(), 120);
        assert_eq!(budget.used(), 24);
        drop(image);
        assert_eq!(budget.used(), 0);
        let pressure = budget.try_reserve(1).unwrap();
        assert!(matches!(
            decode_gltf_texture_bytes(&data, GlbImageMime::Png, GlbTextureRole::Normal, &request),
            Err(GltfTextureError::Memory(_))
        ));
        assert_eq!(budget.used(), 1);
        drop(pressure);
        drop(decode_gltf_texture_bytes(&data, GlbImageMime::Png, GlbTextureRole::Normal, &request).unwrap());
        assert_eq!(budget.used(), 0);
        let token = std::sync::Arc::new(std::sync::atomic::AtomicU64::new(2));
        request.cancellation = Some(crate::GenerationToken::new(token.clone(), 1));
        assert!(matches!(
            decode_gltf_texture_bytes(&data, GlbImageMime::Png, GlbTextureRole::Normal, &request),
            Err(GltfTextureError::Source(DecodeError::Cancelled))
        ));
        assert_eq!(budget.used(), 0);
        request.cancellation = Some(crate::GenerationToken::new(token, 2));
        drop(decode_gltf_texture_bytes(&data, GlbImageMime::Png, GlbTextureRole::Normal, &request).unwrap());
        assert_eq!(budget.used(), 0);
    }
    #[test]
    fn all_truncated_prefixes_crc_mime_and_oversized_headers_refuse_without_panic() {
        let budget = MemoryBudget::new(1024 * 1024);
        let request = request(&budget);
        for name in ["rgba8.png", "pattern.jpg", "pattern.progressive.jpg"] {
            let data = std::fs::read(root().join("texture-codec").join(name)).unwrap();
            let mime = if name.ends_with("png") {
                GlbImageMime::Png
            } else {
                GlbImageMime::Jpeg
            };
            for end in 0..data.len() {
                assert!(
                    decode_gltf_texture_bytes(&data[..end], mime, GlbTextureRole::Normal, &request).is_err(),
                    "{name} prefix{end}"
                );
                assert_eq!(budget.used(), 0);
            }
            assert!(
                decode_gltf_texture_bytes(
                    &data,
                    if mime == GlbImageMime::Png {
                        GlbImageMime::Jpeg
                    } else {
                        GlbImageMime::Png
                    },
                    GlbTextureRole::Normal,
                    &request
                )
                .is_err()
            );
        }
        let mut deferred = std::fs::read(root().join("texture-codec/pattern.jpg")).unwrap();
        let sof = deferred.windows(2).position(|v| v == b"\xff\xc0").unwrap();
        deferred[sof + 5..sof + 7].fill(0);
        assert!(matches!(
            decode_gltf_texture_bytes(&deferred, GlbImageMime::Jpeg, GlbTextureRole::Normal, &request),
            Err(GltfTextureError::Unsupported("JPEG deferred height/DNL"))
        ));
        assert_eq!(budget.used(), 0);
        let data = std::fs::read(root().join("texture-codec/rgba8.png")).unwrap();
        for at in [29, data.len() - 1] {
            let mut bad = data.clone();
            bad[at] ^= 1;
            assert!(
                decode_gltf_texture_bytes(&bad, GlbImageMime::Png, GlbTextureRole::Normal, &request).is_err()
            );
            assert_eq!(budget.used(), 0);
        }
        let mut huge = data;
        huge[16..20].copy_from_slice(&u32::MAX.to_be_bytes());
        huge[20..24].copy_from_slice(&u32::MAX.to_be_bytes());
        let mut crc = flate2::Crc::new();
        crc.update(&huge[12..29]);
        huge[29..33].copy_from_slice(&crc.sum().to_be_bytes());
        assert!(matches!(
            decode_gltf_texture_bytes(&huge, GlbImageMime::Png, GlbTextureRole::Normal, &request),
            Err(GltfTextureError::Limit(_))
        ));
        assert_eq!(budget.used(), 0);
    }
}
