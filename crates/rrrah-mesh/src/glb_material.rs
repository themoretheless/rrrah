//! Core metallic-roughness material data and a point-texel reference evaluator.
//! No image codec, filter, BRDF, GPU shading or URI fetching is performed here.
use crate::{
    DecodeRequest,
    glb::{GlbDocument, GlbError, JsonKind, JsonValue},
    glb_accessor::{array_item, integer, required},
};
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GlbAlphaMode {
    Opaque,
    Mask,
    Blend,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GlbTextureRole {
    BaseColor,
    MetallicRoughness,
    Normal,
    Occlusion,
    Emissive,
}
impl GlbTextureRole {
    pub fn is_srgb(self) -> bool {
        matches!(self, Self::BaseColor | Self::Emissive)
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GlbTextureBinding {
    pub texture: usize,
    pub tex_coord: usize,
}
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GlbNormalTexture {
    pub binding: GlbTextureBinding,
    pub scale: f64,
}
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GlbOcclusionTexture {
    pub binding: GlbTextureBinding,
    pub strength: f64,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GlbSampler {
    pub mag_filter: Option<u32>,
    pub min_filter: Option<u32>,
    pub wrap_s: u32,
    pub wrap_t: u32,
}
impl Default for GlbSampler {
    fn default() -> Self {
        Self {
            mag_filter: None,
            min_filter: None,
            wrap_s: 10497,
            wrap_t: 10497,
        }
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GlbTexture {
    pub image: usize,
    pub sampler: GlbSampler,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GlbImageMime {
    Png,
    Jpeg,
}
#[derive(Debug, Clone, Copy)]
pub enum GlbImageSource<'a> {
    Embedded {
        mime: GlbImageMime,
        bytes: &'a [u8],
    },
    Uri {
        mime: Option<GlbImageMime>,
        uri: JsonValue<'a>,
    },
}
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GlbMaterial {
    pub base_color: [f64; 4],
    pub metallic: f64,
    pub roughness: f64,
    pub emissive: [f64; 3],
    pub alpha_mode: GlbAlphaMode,
    pub alpha_cutoff: f64,
    pub double_sided: bool,
    pub base_color_texture: Option<GlbTextureBinding>,
    pub metallic_roughness_texture: Option<GlbTextureBinding>,
    pub normal_texture: Option<GlbNormalTexture>,
    pub occlusion_texture: Option<GlbOcclusionTexture>,
    pub emissive_texture: Option<GlbTextureBinding>,
}
impl Default for GlbMaterial {
    fn default() -> Self {
        Self {
            base_color: [1.; 4],
            metallic: 1.,
            roughness: 1.,
            emissive: [0.; 3],
            alpha_mode: GlbAlphaMode::Opaque,
            alpha_cutoff: 0.5,
            double_sided: false,
            base_color_texture: None,
            metallic_roughness_texture: None,
            normal_texture: None,
            occlusion_texture: None,
            emissive_texture: None,
        }
    }
}
/// Unfiltered raw normalized texture samples. Color RGB is sRGB encoded, alpha
/// is linear; all data textures are linear. Missing bound samples are errors.
#[derive(Debug, Clone, Copy, Default)]
pub struct GlbMaterialSamples {
    pub base_color: Option<[f64; 4]>,
    pub metallic_roughness: Option<[f64; 4]>,
    pub normal: Option<[f64; 4]>,
    pub occlusion: Option<[f64; 4]>,
    pub emissive: Option<[f64; 4]>,
    pub vertex_color: Option<[f64; 4]>,
}
/// Straight linear color/coverage parameters, not lit or composited output.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GlbSurface {
    pub base_color: [f64; 3],
    pub alpha: f64,
    pub discard: bool,
    pub metallic: f64,
    pub roughness: f64,
    pub emissive: [f64; 3],
    pub tangent_normal: Option<[f64; 3]>,
    pub occlusion: f64,
}
fn scalar(value: JsonValue<'_>) -> Result<f64, GlbError> {
    value.as_f64().ok_or(GlbError::Invalid("material finite number"))
}
fn range(value: f64, low: f64, high: f64) -> Result<f64, GlbError> {
    if value.is_finite() && (low..=high).contains(&value) {
        Ok(value)
    } else {
        Err(GlbError::Invalid("material value range"))
    }
}
fn factor(object: JsonValue<'_>, name: &str, default: f64, request: &DecodeRequest) -> Result<f64, GlbError> {
    object
        .field(name, request)?
        .map(scalar)
        .transpose()?
        .map_or(Ok(default), |v| range(v, 0., 1.))
}
fn color<const N: usize>(
    object: JsonValue<'_>,
    name: &str,
    default: [f64; N],
    request: &DecodeRequest,
) -> Result<[f64; N], GlbError> {
    let Some(value) = object.field(name, request)? else {
        return Ok(default);
    };
    let mut array = value.array().ok_or(GlbError::Invalid("material color array"))?;
    let mut out = [0.; N];
    for channel in &mut out {
        *channel = range(
            scalar(array.next().ok_or(GlbError::Invalid("material color length"))?)?,
            0.,
            1.,
        )?;
    }
    if array.next().is_some() {
        return Err(GlbError::Invalid("material color length"));
    }
    Ok(out)
}
fn mime(value: JsonValue<'_>) -> Result<GlbImageMime, GlbError> {
    if value.kind() != JsonKind::String {
        return Err(GlbError::Invalid("image MIME string"));
    }
    if value
        .string_chars()
        .unwrap()
        .map(|c| c.to_ascii_lowercase())
        .eq("image/png".chars())
    {
        Ok(GlbImageMime::Png)
    } else if value
        .string_chars()
        .unwrap()
        .map(|c| c.to_ascii_lowercase())
        .eq("image/jpeg".chars())
    {
        Ok(GlbImageMime::Jpeg)
    } else {
        Err(GlbError::Unsupported("non-core image MIME"))
    }
}
fn option_u32(object: JsonValue<'_>, name: &str, request: &DecodeRequest) -> Result<Option<u32>, GlbError> {
    object
        .field(name, request)?
        .map(|v| {
            integer(v).and_then(|v| u32::try_from(v).map_err(|_| GlbError::Invalid("sampler integer range")))
        })
        .transpose()
}
impl GlbDocument {
    pub fn sampler(&self, index: Option<usize>, request: &DecodeRequest) -> Result<GlbSampler, GlbError> {
        request.check_cancelled()?;
        let Some(index) = index else {
            return Ok(GlbSampler::default());
        };
        let value = array_item(self.root(), "samplers", index, request)?;
        let mag_filter = option_u32(value, "magFilter", request)?;
        let min_filter = option_u32(value, "minFilter", request)?;
        let wrap_s = option_u32(value, "wrapS", request)?.unwrap_or(10497);
        let wrap_t = option_u32(value, "wrapT", request)?.unwrap_or(10497);
        if mag_filter.is_some_and(|v| !matches!(v, 9728 | 9729))
            || min_filter.is_some_and(|v| !matches!(v, 9728 | 9729 | 9984 | 9985 | 9986 | 9987))
            || !matches!(wrap_s, 33071 | 33648 | 10497)
            || !matches!(wrap_t, 33071 | 33648 | 10497)
        {
            return Err(GlbError::Unsupported("sampler filter/wrap mode"));
        }
        request.check_cancelled()?;
        Ok(GlbSampler {
            mag_filter,
            min_filter,
            wrap_s,
            wrap_t,
        })
    }
    pub fn texture(&self, index: usize, request: &DecodeRequest) -> Result<GlbTexture, GlbError> {
        request.check_cancelled()?;
        let value = array_item(self.root(), "textures", index, request)?;
        let image = integer(
            value
                .field("source", request)?
                .ok_or(GlbError::Unsupported("texture without core image source"))?,
        )?;
        let image_object = array_item(self.root(), "images", image, request)?;
        if image_object.object().is_none() {
            return Err(GlbError::Invalid("image object"));
        }
        let sampler = self.sampler(
            value.field("sampler", request)?.map(integer).transpose()?,
            request,
        )?;
        self.image_source(image, request)?;
        request.check_cancelled()?;
        Ok(GlbTexture { image, sampler })
    }
    pub fn image_source(
        &self,
        index: usize,
        request: &DecodeRequest,
    ) -> Result<GlbImageSource<'_>, GlbError> {
        request.check_cancelled()?;
        let value = array_item(self.root(), "images", index, request)?;
        let uri = value.field("uri", request)?;
        let view = value.field("bufferView", request)?;
        let mime = value.field("mimeType", request)?.map(mime).transpose()?;
        let result = match (uri, view) {
            (Some(uri), None) => {
                if uri.kind() != JsonKind::String {
                    return Err(GlbError::Invalid("image URI string"));
                }
                Ok(GlbImageSource::Uri { mime, uri })
            }
            (None, Some(view)) => {
                let mime = mime.ok_or(GlbError::Invalid("embedded image missing MIME"))?;
                let view = self.view(integer(view)?, request)?;
                if view.stride.is_some() || view.target.is_some() {
                    return Err(GlbError::Invalid("image bufferView stride/target"));
                }
                Ok(GlbImageSource::Embedded {
                    mime,
                    bytes: view.bytes,
                })
            }
            _ => Err(GlbError::Invalid("image requires exactly URI or bufferView")),
        }?;
        request.check_cancelled()?;
        Ok(result)
    }
    fn texture_binding(
        &self,
        object: JsonValue<'_>,
        name: &str,
        request: &DecodeRequest,
    ) -> Result<Option<GlbTextureBinding>, GlbError> {
        let Some(value) = object.field(name, request)? else {
            return Ok(None);
        };
        let texture = integer(required(value, "index", request)?)?;
        let tex_coord = value
            .field("texCoord", request)?
            .map(integer)
            .transpose()?
            .unwrap_or(0);
        if tex_coord >= 64 {
            return Err(GlbError::Limit("texture coordinate set"));
        }
        self.texture(texture, request)?;
        Ok(Some(GlbTextureBinding { texture, tex_coord }))
    }
    /// Core material semantics only. Images are not decoded/fetched and sampler
    /// filtering is not executed. Optional extension data remains in source JSON.
    pub fn material(&self, index: Option<usize>, request: &DecodeRequest) -> Result<GlbMaterial, GlbError> {
        request.check_cancelled()?;
        let Some(index) = index else {
            return Ok(GlbMaterial::default());
        };
        let value = array_item(self.root(), "materials", index, request)?;
        if value.object().is_none() {
            return Err(GlbError::Invalid("material object"));
        }
        let mut out = GlbMaterial::default();
        if let Some(pbr) = value.field("pbrMetallicRoughness", request)? {
            out.base_color = color(pbr, "baseColorFactor", out.base_color, request)?;
            out.metallic = factor(pbr, "metallicFactor", 1., request)?;
            out.roughness = factor(pbr, "roughnessFactor", 1., request)?;
            out.base_color_texture = self.texture_binding(pbr, "baseColorTexture", request)?;
            out.metallic_roughness_texture =
                self.texture_binding(pbr, "metallicRoughnessTexture", request)?;
        }
        out.emissive = color(value, "emissiveFactor", out.emissive, request)?;
        out.emissive_texture = self.texture_binding(value, "emissiveTexture", request)?;
        if let Some(mode) = value.field("alphaMode", request)? {
            out.alpha_mode = if mode.string_eq("OPAQUE") {
                GlbAlphaMode::Opaque
            } else if mode.string_eq("MASK") {
                GlbAlphaMode::Mask
            } else if mode.string_eq("BLEND") {
                GlbAlphaMode::Blend
            } else if mode.kind() == JsonKind::String {
                return Err(GlbError::Unsupported("alpha mode"));
            } else {
                return Err(GlbError::Invalid("alpha mode string"));
            };
        }
        if let Some(cutoff) = value.field("alphaCutoff", request)? {
            let v = scalar(cutoff)?;
            if v < 0. {
                return Err(GlbError::Invalid("alpha cutoff"));
            }
            out.alpha_cutoff = v;
        }
        if let Some(sided) = value.field("doubleSided", request)? {
            out.double_sided = sided.as_bool().ok_or(GlbError::Invalid("doubleSided boolean"))?;
        }
        if let Some(binding) = self.texture_binding(value, "normalTexture", request)? {
            let slot = value.field("normalTexture", request)?.unwrap();
            out.normal_texture = Some(GlbNormalTexture {
                binding,
                scale: slot
                    .field("scale", request)?
                    .map(scalar)
                    .transpose()?
                    .unwrap_or(1.),
            });
        }
        if let Some(binding) = self.texture_binding(value, "occlusionTexture", request)? {
            let slot = value.field("occlusionTexture", request)?.unwrap();
            out.occlusion_texture = Some(GlbOcclusionTexture {
                binding,
                strength: factor(slot, "strength", 1., request)?,
            });
        }
        request.check_cancelled()?;
        Ok(out)
    }
}
fn sample(bound: bool, value: Option<[f64; 4]>) -> Result<[f64; 4], GlbError> {
    if bound {
        value.ok_or(GlbError::Invalid("missing bound texture sample"))
    } else {
        Ok([1.; 4])
    }
}
fn channel(value: f64) -> Result<f64, GlbError> {
    range(value, 0., 1.)
}
fn srgb(value: f64) -> Result<f64, GlbError> {
    let v = channel(value)?;
    Ok(if v <= 0.04045 {
        v / 12.92
    } else {
        ((v + 0.055) / 1.055).powf(2.4)
    })
}
impl GlbMaterial {
    pub fn validate_primitive_texcoords(
        self,
        primitive: crate::glb::GlbPrimitive<'_>,
        request: &DecodeRequest,
    ) -> Result<(), GlbError> {
        for binding in [
            self.base_color_texture,
            self.metallic_roughness_texture,
            self.normal_texture.map(|v| v.binding),
            self.occlusion_texture.map(|v| v.binding),
            self.emissive_texture,
        ]
        .into_iter()
        .flatten()
        {
            request.check_cancelled()?;
            if binding.tex_coord >= 64 {
                return Err(GlbError::Limit("texture coordinate set"));
            }
            let mut name = *b"TEXCOORD_00";
            let length = if binding.tex_coord < 10 {
                name[9] = b'0' + binding.tex_coord as u8;
                10
            } else {
                name[9] = b'0' + (binding.tex_coord / 10) as u8;
                name[10] = b'0' + (binding.tex_coord % 10) as u8;
                11
            };
            let name = std::str::from_utf8(&name[..length]).unwrap();
            if primitive.attribute(name, request)?.is_none() {
                return Err(GlbError::Invalid("missing material texture coordinate set"));
            }
        }
        request.check_cancelled()?;
        Ok(())
    }
    /// Point-texel reference, not an image filtering/shading fallback. sRGB
    /// decoding must precede filtering in the eventual renderer. RGB stays
    /// straight; alpha masking/blending does not premultiply material color.
    pub fn evaluate_texel(self, samples: GlbMaterialSamples) -> Result<GlbSurface, GlbError> {
        // Public fields may be edited by callers; maintain the same invariants.
        for v in self
            .base_color
            .into_iter()
            .chain(self.emissive)
            .chain([self.metallic, self.roughness])
        {
            channel(v)?;
        }
        if !self.alpha_cutoff.is_finite() || self.alpha_cutoff < 0. {
            return Err(GlbError::Invalid("alpha cutoff"));
        }
        let base = sample(self.base_color_texture.is_some(), samples.base_color)?;
        let mr = sample(
            self.metallic_roughness_texture.is_some(),
            samples.metallic_roughness,
        )?;
        let emissive = sample(self.emissive_texture.is_some(), samples.emissive)?;
        let vertex = samples.vertex_color.unwrap_or([1.; 4]);
        if vertex.into_iter().any(|v| !v.is_finite()) {
            return Err(GlbError::Invalid("non-finite vertex color"));
        }
        let vertex = vertex.map(|v| v.clamp(0., 1.));
        let mut color = [0.; 3];
        let mut emission = [0.; 3];
        for c in 0..3 {
            color[c] = self.base_color[c] * srgb(base[c])? * vertex[c];
            emission[c] = self.emissive[c] * srgb(emissive[c])?;
        }
        let raw_alpha = if self.alpha_mode == GlbAlphaMode::Opaque {
            1.
        } else {
            self.base_color[3] * channel(base[3])? * vertex[3]
        };
        let discard = self.alpha_mode == GlbAlphaMode::Mask && raw_alpha < self.alpha_cutoff;
        let alpha = match self.alpha_mode {
            GlbAlphaMode::Opaque => 1.,
            GlbAlphaMode::Mask => {
                if discard {
                    0.
                } else {
                    1.
                }
            }
            GlbAlphaMode::Blend => raw_alpha,
        };
        let metallic = self.metallic * channel(mr[2])?;
        let roughness = self.roughness * channel(mr[1])?;
        let occlusion = if let Some(slot) = self.occlusion_texture {
            channel(slot.strength)?;
            let value = sample(true, samples.occlusion)?;
            1. + slot.strength * (channel(value[0])? - 1.)
        } else {
            1.
        };
        let tangent_normal = if let Some(slot) = self.normal_texture {
            if !slot.scale.is_finite() {
                return Err(GlbError::Invalid("normal scale"));
            }
            let value = sample(true, samples.normal)?;
            let normal = [
                (2. * channel(value[0])? - 1.) * slot.scale,
                (2. * channel(value[1])? - 1.) * slot.scale,
                2. * channel(value[2])? - 1.,
            ];
            let largest = normal.into_iter().map(f64::abs).fold(0., f64::max);
            if largest == 0. {
                return Err(GlbError::Invalid("zero tangent normal sample"));
            }
            let scaled = normal.map(|v| v / largest);
            let length = scaled.into_iter().map(|v| v * v).sum::<f64>().sqrt();
            Some(scaled.map(|v| v / length))
        } else {
            None
        };
        Ok(GlbSurface {
            base_color: color,
            alpha,
            discard,
            metallic,
            roughness,
            emissive: emission,
            tangent_normal,
            occlusion,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rrrah_core::MemoryBudget;
    use std::sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    };
    fn fixture(name: &str) -> std::path::PathBuf {
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../tests/fixtures/glb")
            .join(name)
    }
    fn req(name: &str, budget: &MemoryBudget) -> DecodeRequest {
        let mut r = DecodeRequest::new(fixture(name));
        r.memory_budget = Some(budget.clone());
        r
    }
    fn close(a: f64, b: f64) {
        assert!((a - b).abs() <= 2e-13 * b.abs().max(1.), "{a} != {b}");
    }
    fn samples() -> Vec<GlbMaterialSamples> {
        std::fs::read_to_string(fixture("material-samples.tsv"))
            .unwrap()
            .lines()
            .map(|line| {
                let v: Vec<f64> = line.split('\t').map(|v| v.parse().unwrap()).collect();
                let slot = |i: usize| Some([v[1 + i * 4], v[2 + i * 4], v[3 + i * 4], v[4 + i * 4]]);
                GlbMaterialSamples {
                    base_color: slot(0),
                    metallic_roughness: slot(1),
                    normal: slot(2),
                    occlusion: slot(3),
                    emissive: slot(4),
                    vertex_color: slot(5),
                }
            })
            .collect()
    }
    #[test]
    fn parsed_core_parameters_and_decimal_surface_controls_preserve_linear_semantics() {
        let budget = MemoryBudget::new(1024 * 1024);
        let request = req("material-controls.glb", &budget);
        let document = GlbDocument::read(&request).unwrap();
        let used = budget.used();
        let peak = budget.peak();
        let mut count = 0;
        for line in std::fs::read_to_string(fixture("material-parameters.tsv"))
            .unwrap()
            .lines()
        {
            let v: Vec<f64> = line.split('\t').map(|v| v.parse().unwrap()).collect();
            let m = document.material(Some(v[0] as usize), &request).unwrap();
            assert_eq!(m.base_color, [v[1], v[2], v[3], v[4]]);
            assert_eq!(m.metallic, v[5]);
            assert_eq!(m.roughness, v[6]);
            assert_eq!(m.emissive, [v[7], v[8], v[9]]);
            assert_eq!(
                m.alpha_mode,
                match v[10] as usize {
                    0 => GlbAlphaMode::Opaque,
                    1 => GlbAlphaMode::Mask,
                    _ => GlbAlphaMode::Blend,
                }
            );
            assert_eq!(m.alpha_cutoff, v[11]);
            assert_eq!(m.double_sided, v[12] != 0.);
            assert_eq!(m.normal_texture.map_or(0., |v| v.scale), v[13]);
            assert_eq!(m.occlusion_texture.map_or(1., |v| v.strength), v[14]);
            count += 1;
        }
        assert_eq!(count, 8);
        let samples = samples();
        let mut count = 0;
        for line in std::fs::read_to_string(fixture("material-surfaces.tsv"))
            .unwrap()
            .lines()
        {
            let v: Vec<f64> = line.split('\t').map(|v| v.parse().unwrap()).collect();
            let m = document.material(Some(v[0] as usize), &request).unwrap();
            let s = m.evaluate_texel(samples[v[1] as usize]).unwrap();
            for c in 0..3 {
                close(s.base_color[c], v[2 + c]);
                close(s.emissive[c], v[9 + c]);
            }
            close(s.alpha, v[5]);
            assert_eq!(s.discard, v[6] != 0.);
            close(s.metallic, v[7]);
            close(s.roughness, v[8]);
            close(s.occlusion, v[12]);
            assert_eq!(s.tangent_normal.is_some(), v[13] != 0.);
            if let Some(n) = s.tangent_normal {
                for c in 0..3 {
                    close(n[c], v[14 + c]);
                }
            }
            count += 1;
            assert_eq!(budget.used(), used);
            assert_eq!(budget.peak(), peak);
        }
        assert_eq!(count, 32);
        let default = document.material(None, &request).unwrap();
        assert_eq!(default, GlbMaterial::default());
        let s = default.evaluate_texel(GlbMaterialSamples::default()).unwrap();
        assert_eq!(s.base_color, [1.; 3]);
        assert_eq!(s.alpha, 1.);
        assert_eq!(s.metallic, 1.);
        assert_eq!(s.roughness, 1.);
        let textured = document.material(Some(1), &request).unwrap();
        textured
            .validate_primitive_texcoords(document.primitive(0, 0, &request).unwrap(), &request)
            .unwrap();
        assert!(
            textured
                .validate_primitive_texcoords(document.primitive(1, 0, &request).unwrap(), &request)
                .is_err()
        );
        assert_eq!(textured.normal_texture.unwrap().binding.tex_coord, 1);
        let image = document.image_source(0, &request).unwrap();
        let GlbImageSource::Embedded { mime, bytes } = image else {
            panic!()
        };
        assert_eq!(mime, GlbImageMime::Png);
        assert_eq!(&bytes[..8], b"\x89PNG\r\n\x1a\n");
        let GlbImageSource::Uri { mime, uri } = document.image_source(1, &request).unwrap() else {
            panic!()
        };
        assert_eq!(mime, Some(GlbImageMime::Png));
        assert!(uri.string_eq("textures/unfetched.png"));
        assert_eq!(
            document.texture(0, &request).unwrap().sampler,
            GlbSampler {
                mag_filter: Some(9729),
                min_filter: Some(9987),
                wrap_s: 33071,
                wrap_t: 33648
            }
        );
        assert_eq!(
            document.texture(1, &request).unwrap().sampler,
            GlbSampler::default()
        );
        assert_eq!(
            document.sampler(Some(1), &request).unwrap(),
            GlbSampler::default()
        );
        drop(document);
        assert_eq!(budget.used(), 0);
    }
    #[test]
    fn original_textured_box_keeps_factors_sampler_and_embedded_png_bytes_borrowed() {
        let budget = MemoryBudget::new(32768);
        let request = req("BoxTextured.glb", &budget);
        let document = GlbDocument::read(&request).unwrap();
        let used = budget.used();
        let peak = budget.peak();
        let m = document.material(Some(0), &request).unwrap();
        assert_eq!(m.base_color, [1.; 4]);
        assert_eq!(m.metallic, 0.);
        assert_eq!(m.roughness, 1.);
        assert_eq!(m.emissive, [0.; 3]);
        assert_eq!(m.alpha_mode, GlbAlphaMode::Opaque);
        assert_eq!(
            m.base_color_texture,
            Some(GlbTextureBinding {
                texture: 0,
                tex_coord: 0
            })
        );
        m.validate_primitive_texcoords(document.primitive(0, 0, &request).unwrap(), &request)
            .unwrap();
        let t = document.texture(0, &request).unwrap();
        assert_eq!(t.image, 0);
        assert_eq!(
            t.sampler,
            GlbSampler {
                mag_filter: Some(9729),
                min_filter: Some(9986),
                wrap_s: 10497,
                wrap_t: 10497
            }
        );
        let GlbImageSource::Embedded { mime, bytes } = document.image_source(0, &request).unwrap() else {
            panic!()
        };
        assert_eq!(mime, GlbImageMime::Png);
        assert_eq!(
            bytes,
            &include_bytes!("../../../tests/fixtures/glb/BoxTextured.glb")[2204..5954]
        );
        assert_eq!(bytes.len(), 3750);
        assert_eq!(&bytes[..8], b"\x89PNG\r\n\x1a\n");
        let clone = document.clone();
        let GlbImageSource::Embedded { bytes: other, .. } = clone.image_source(0, &request).unwrap() else {
            panic!()
        };
        assert_eq!(bytes.as_ptr(), other.as_ptr());
        assert_eq!(budget.used(), used);
        assert_eq!(budget.peak(), peak);
        drop(document);
        assert_eq!(budget.used(), used);
        drop(clone);
        assert_eq!(budget.used(), 0);
    }
    #[test]
    fn absent_bound_samples_unused_channels_alpha_boundaries_and_bad_values_are_explicit() {
        let budget = MemoryBudget::new(1024 * 1024);
        let request = req("material-controls.glb", &budget);
        let document = GlbDocument::read(&request).unwrap();
        let m = document.material(Some(1), &request).unwrap();
        let mut input = samples()[0];
        let expected = m.evaluate_texel(input).unwrap();
        input.metallic_roughness.as_mut().unwrap()[0] = f64::NAN;
        input.metallic_roughness.as_mut().unwrap()[3] = f64::NAN;
        input.normal.as_mut().unwrap()[3] = f64::NAN;
        input.occlusion.as_mut().unwrap()[1..].fill(f64::NAN);
        input.emissive.as_mut().unwrap()[3] = f64::NAN;
        assert_eq!(m.evaluate_texel(input).unwrap(), expected);
        for slot in 0..5 {
            let mut input = samples()[0];
            match slot {
                0 => input.base_color = None,
                1 => input.metallic_roughness = None,
                2 => input.normal = None,
                3 => input.occlusion = None,
                _ => input.emissive = None,
            };
            assert!(m.evaluate_texel(input).is_err());
        }
        let mut bad = samples()[0];
        bad.normal = Some([0.5, 0.5, 0.5, 1.]);
        assert!(m.evaluate_texel(bad).is_err());
        let mut bad = samples()[0];
        bad.metallic_roughness.as_mut().unwrap()[1] = f64::NAN;
        assert!(m.evaluate_texel(bad).is_err());
        let mut opaque = samples()[0];
        opaque.base_color.as_mut().unwrap()[3] = f64::NAN;
        assert_eq!(
            document
                .material(Some(5), &request)
                .unwrap()
                .evaluate_texel(opaque)
                .unwrap()
                .alpha,
            1.
        );
        let mask = document
            .material(Some(3), &request)
            .unwrap()
            .evaluate_texel(samples()[3])
            .unwrap();
        assert_eq!(mask.alpha, 1.);
        assert!(!mask.discard);
        let masked = document
            .material(Some(2), &request)
            .unwrap()
            .evaluate_texel(samples()[3])
            .unwrap();
        assert_eq!(masked.alpha, 0.);
        assert!(masked.discard);
        assert_eq!(mask.base_color, masked.base_color);
        assert!(GlbTextureRole::BaseColor.is_srgb());
        assert!(GlbTextureRole::Emissive.is_srgb());
        for role in [
            GlbTextureRole::Normal,
            GlbTextureRole::Occlusion,
            GlbTextureRole::MetallicRoughness,
        ] {
            assert!(!role.is_srgb());
        }
        drop(document);
        assert_eq!(budget.used(), 0);
    }
    #[test]
    fn malformed_material_and_reference_contracts_refuse_without_extra_credit() {
        let budget = MemoryBudget::new(1024 * 1024);
        let controls = include_bytes!("../../../tests/fixtures/glb/material-controls.glb");
        let request = DecodeRequest::new("unused.glb");
        let layout = crate::glb::GlbLayout::parse(controls, &request).unwrap();
        let binary = &controls[layout.bin.unwrap()];
        let mut cases = 0;
        for line in std::fs::read_to_string(fixture("material-invalid.tsv"))
            .unwrap()
            .lines()
        {
            let (name, hex) = line.split_once('\t').unwrap();
            let mut json: Vec<u8> = hex
                .as_bytes()
                .chunks_exact(2)
                .map(|v| u8::from_str_radix(std::str::from_utf8(v).unwrap(), 16).unwrap())
                .collect();
            while json.len() % 4 != 0 {
                json.push(b' ');
            }
            let mut bytes = b"glTF\x02\0\0\0".to_vec();
            bytes.extend_from_slice(&((28 + json.len() + binary.len()) as u32).to_le_bytes());
            for (kind, chunk) in [(0x4e4f534au32, json.as_slice()), (0x004e4942u32, binary)] {
                bytes.extend_from_slice(&(chunk.len() as u32).to_le_bytes());
                bytes.extend_from_slice(&kind.to_le_bytes());
                bytes.extend_from_slice(chunk);
            }
            let path =
                std::env::temp_dir().join(format!("rrrah-material-bad-{}-{cases}.glb", std::process::id()));
            std::fs::write(&path, bytes).unwrap();
            let mut request = DecodeRequest::new(&path);
            request.memory_budget = Some(budget.clone());
            let document = GlbDocument::read(&request).unwrap();
            std::fs::remove_file(path).unwrap();
            let used = budget.used();
            let result = document.material(Some(1), &request);
            if matches!(name, "mag-enum" | "min-enum" | "wrap-enum" | "mode-extension") {
                assert!(
                    matches!(&result, Err(GlbError::Unsupported(_))),
                    "{name}: {result:?}"
                );
            }
            assert!(result.is_err(), "{name}");
            assert_eq!(budget.used(), used);
            drop(document);
            assert_eq!(budget.used(), 0);
            cases += 1;
        }
        assert_eq!(cases, 28);
    }
    #[test]
    fn material_cancellation_new_generation_and_owner_retention() {
        let budget = MemoryBudget::new(32768);
        let mut request = req("BoxTextured.glb", &budget);
        let generation = Arc::new(AtomicU64::new(1));
        request.cancellation = Some(crate::GenerationToken::new(generation.clone(), 1));
        let document = GlbDocument::read(&request).unwrap();
        let used = budget.used();
        generation.store(2, Ordering::Release);
        assert!(matches!(
            document.material(Some(0), &request),
            Err(GlbError::Source(crate::DecodeError::Cancelled))
        ));
        assert!(document.texture(0, &request).is_err());
        assert!(document.image_source(0, &request).is_err());
        request.cancellation = Some(crate::GenerationToken::new(generation, 2));
        assert!(document.material(Some(0), &request).is_ok());
        assert_eq!(budget.used(), used);
        drop(document);
        assert_eq!(budget.used(), 0);
    }
}
