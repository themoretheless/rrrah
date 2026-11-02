//! Resolved borrowed material streams; no per-triangle JSON lookup or mesh copy.
use crate::{
    DecodeRequest,
    glb::{GlbAccessor, GlbError, GlbMaterial, GlbPrimitive, GlbShape},
};

/// Local-space corner. Missing normals/tangents remain explicit so a renderer
/// can select flat normals/derivative tangent frames instead of invented data.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GlbVertex {
    pub position: [f64; 3],
    pub normal: Option<[f64; 3]>,
    pub tangent: Option<[f64; 4]>,
    pub color: [f64; 4],
    /// Base color, metallic/roughness, normal, occlusion, emissive UVs.
    /// None means the material has no binding for this role.
    pub texcoords: [Option<[f64; 2]>; 5],
}

impl GlbVertex {
    pub fn texcoord(self, role: crate::glb::GlbTextureRole) -> Option<[f64; 2]> {
        use crate::glb::GlbTextureRole::*;
        self.texcoords[match role {
            BaseColor => 0,
            MetallicRoughness => 1,
            Normal => 2,
            Occlusion => 3,
            Emissive => 4,
        }]
    }
}

/// Accessors and material are resolved once; indices, sparse samples, normalized
/// components and winding continue to use the original validated primitive.
/// Keep the source document alive. Instance transforms belong to GlbScenePrimitive.
#[derive(Debug, Clone, Copy)]
pub struct GlbVertexStreams<'a> {
    primitive: GlbPrimitive<'a>,
    material: GlbMaterial,
    normal: Option<GlbAccessor<'a>>,
    tangent: Option<GlbAccessor<'a>>,
    color: Option<GlbAccessor<'a>>,
    texcoords: [Option<GlbAccessor<'a>>; 5],
    texcoord_alias: [usize; 5],
}
impl<'a> GlbVertexStreams<'a> {
    pub(crate) fn new(
        primitive: GlbPrimitive<'a>,
        material: GlbMaterial,
        request: &DecodeRequest,
    ) -> Result<Self, GlbError> {
        request.check_cancelled()?;
        let bindings = [
            material.base_color_texture,
            material.metallic_roughness_texture,
            material.normal_texture.map(|v| v.binding),
            material.occlusion_texture.map(|v| v.binding),
            material.emissive_texture,
        ];
        let mut texcoords = [None; 5];
        let mut texcoord_alias = std::array::from_fn(|i| i);
        for (role, binding) in bindings.iter().enumerate() {
            let Some(binding) = binding else { continue };
            request.check_cancelled()?;
            if binding.tex_coord >= 64 {
                return Err(GlbError::Limit("texture coordinate set"));
            }
            if let Some(previous) = bindings[..role]
                .iter()
                .position(|v| v.is_some_and(|b| b.tex_coord == binding.tex_coord))
            {
                texcoords[role] = texcoords[previous];
                texcoord_alias[role] = previous;
                continue;
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
            texcoords[role] = Some(
                primitive
                    .attribute(std::str::from_utf8(&name[..length]).unwrap(), request)?
                    .ok_or(GlbError::Invalid("missing material texture coordinate set"))?,
            );
        }
        Ok(Self {
            primitive,
            material,
            normal: primitive.attribute("NORMAL", request)?,
            tangent: primitive.attribute("TANGENT", request)?,
            color: primitive.attribute("COLOR_0", request)?,
            texcoords,
            texcoord_alias,
        })
    }
    pub fn material(self) -> GlbMaterial {
        self.material
    }
    pub fn vertex_count(self) -> usize {
        self.primitive.positions().len()
    }
    pub fn triangle_count(self) -> usize {
        self.primitive.triangle_count()
    }
    /// Source vertex index, not an expanded triangle-corner index.
    pub fn vertex(self, index: usize, request: &DecodeRequest) -> Result<GlbVertex, GlbError> {
        request.check_cancelled()?;
        fn values<const N: usize>(a: GlbAccessor<'_>, index: usize) -> Result<[f64; N], GlbError> {
            let mut out = [0.; N];
            for (c, value) in out.iter_mut().enumerate() {
                *value = a.component(index, c)?;
            }
            Ok(out)
        }
        let mut color = [1.; 4];
        if let Some(a) = self.color {
            let count = if a.shape() == GlbShape::Vec3 { 3 } else { 4 };
            for (c, value) in color[..count].iter_mut().enumerate() {
                *value = a.component(index, c)?.clamp(0., 1.);
            }
        }
        let mut texcoords = [None; 5];
        for role in 0..5 {
            texcoords[role] = if self.texcoord_alias[role] != role {
                texcoords[self.texcoord_alias[role]]
            } else {
                self.texcoords[role].map(|a| values(a, index)).transpose()?
            };
        }
        let result = GlbVertex {
            position: values(self.primitive.positions(), index)?,
            normal: self.normal.map(|a| values(a, index)).transpose()?,
            tangent: self.tangent.map(|a| values(a, index)).transpose()?,
            color,
            texcoords,
        };
        request.check_cancelled()?;
        Ok(result)
    }
    pub fn triangle(self, index: usize, request: &DecodeRequest) -> Result<Option<[GlbVertex; 3]>, GlbError> {
        request.check_cancelled()?;
        let Some([a, b, c]) = self.primitive.triangle_indices(index) else {
            return Ok(None);
        };
        Ok(Some([
            self.vertex(a as usize, request)?,
            self.vertex(b as usize, request)?,
            self.vertex(c as usize, request)?,
        ]))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::glb::GlbDocument;
    use rrrah_core::MemoryBudget;
    fn request(name: &str, budget: &MemoryBudget) -> DecodeRequest {
        let mut r = DecodeRequest::new(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../../tests/fixtures/glb")
                .join(name),
        );
        r.memory_budget = Some(budget.clone());
        r
    }
    #[test]
    fn producer_indexed_positions_normals_uvs_agree_for_every_corner_without_expansion() {
        let budget = MemoryBudget::new(65536);
        let r = request("BoxTextured.glb", &budget);
        let document = GlbDocument::read(&r).unwrap();
        let primitive = document.primitive(0, 0, &r).unwrap();
        let used = budget.used();
        let peak = budget.peak();
        let streams = primitive.vertex_streams(&r).unwrap();
        assert_eq!(streams.vertex_count(), 24);
        assert_eq!(streams.triangle_count(), 12);
        let mut corners = 0;
        for row in include_str!("../../../tests/fixtures/glb/BoxTextured-vertices.tsv").lines() {
            let f: Vec<f64> = row.split('\t').map(|v| v.parse().unwrap()).collect();
            let v = streams.triangle(f[0] as usize, &r).unwrap().unwrap()[f[1] as usize];
            assert_eq!(
                primitive.triangle_indices(f[0] as usize).unwrap()[f[1] as usize],
                f[2] as u32
            );
            assert_eq!(v.position, f[3..6]);
            assert_eq!(v.normal.unwrap(), f[6..9]);
            assert_eq!(v.texcoords[0].unwrap(), f[9..11]);
            assert_eq!(v.texcoords[1..], [None; 4]);
            assert_eq!(v.tangent, None);
            assert_eq!(v.color, [1.; 4]);
            corners += 1;
        }
        assert_eq!(corners, 36);
        assert!(streams.triangle(12, &r).unwrap().is_none());
        assert!(streams.vertex(24, &r).is_err());
        assert_eq!(budget.used(), used);
        assert_eq!(budget.peak(), peak);
        drop(document);
        assert_eq!(budget.used(), 0);
    }
    #[test]
    fn authored_role_uvs_tangent_color_defaults_and_missing_sets_are_explicit() {
        let budget = MemoryBudget::new(65536);
        let mut r = request("material-controls.glb", &budget);
        let document = GlbDocument::read(&r).unwrap();
        let primitive = document.primitive(0, 0, &r).unwrap();
        let streams = primitive.vertex_streams(&r).unwrap();
        let used = budget.used();
        let peak = budget.peak();
        for i in 0..2 {
            for (vertex, index) in streams
                .triangle(i, &r)
                .unwrap()
                .unwrap()
                .into_iter()
                .zip(primitive.triangle_indices(i).unwrap())
            {
                let positions = [[0., 0., 0.], [1., 0., 0.], [1., 1., 0.], [0., 1., 0.]];
                assert_eq!(vertex.position, positions[index as usize]);
                assert_eq!(vertex.normal, Some([0., 0., 1.]));
                assert_eq!(vertex.tangent, Some([1., 0., 0., -1.]));
                assert_eq!(vertex.color, [1., 0.5, 0., 0.75]);
                assert_eq!(
                    vertex.texcoords,
                    [
                        Some([0., 1.]),
                        Some([0., 1.]),
                        Some([0.25, 0.5]),
                        Some([0., 1.]),
                        Some([0., 1.])
                    ]
                );
            }
        }
        let untextured = document.primitive(1, 0, &r).unwrap().vertex_streams(&r).unwrap();
        let v = untextured.vertex(0, &r).unwrap();
        assert_eq!(v.normal, None);
        assert_eq!(v.tangent, None);
        assert_eq!(v.color, [1.; 4]);
        assert_eq!(v.texcoords, [None; 5]);
        let mut material = streams.material();
        material.normal_texture.as_mut().unwrap().binding.tex_coord = 2;
        assert!(matches!(
            GlbVertexStreams::new(primitive, material, &r),
            Err(GlbError::Invalid("missing material texture coordinate set"))
        ));
        material.normal_texture.as_mut().unwrap().binding.tex_coord = 64;
        assert!(matches!(
            GlbVertexStreams::new(primitive, material, &r),
            Err(GlbError::Limit(_))
        ));
        let token = std::sync::Arc::new(std::sync::atomic::AtomicU64::new(2));
        r.cancellation = Some(crate::GenerationToken::new(token.clone(), 1));
        assert!(matches!(
            streams.vertex(0, &r),
            Err(GlbError::Source(crate::DecodeError::Cancelled))
        ));
        assert!(matches!(
            streams.triangle(usize::MAX, &r),
            Err(GlbError::Source(crate::DecodeError::Cancelled))
        ));
        assert!(primitive.vertex_streams(&r).is_err());
        r.cancellation = Some(crate::GenerationToken::new(token, 2));
        assert_eq!(
            streams
                .vertex(0, &r)
                .unwrap()
                .texcoord(crate::glb::GlbTextureRole::Normal),
            Some([0.25, 0.5])
        );
        assert_eq!(budget.used(), used);
        assert_eq!(budget.peak(), peak);
        drop(document);
        assert_eq!(budget.used(), 0);
    }
}
