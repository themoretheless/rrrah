//! Validated borrowed triangle primitives in source-local coordinates.
use crate::{
    DecodeRequest,
    glb::{GlbAccessor, GlbComponent, GlbDocument, GlbError, GlbShape, JsonValue},
    glb_accessor::{array_item, integer, required},
};
const MAX_ATTRIBUTES: usize = 64;
const MAX_COMPONENT_WORK: usize = 128_000_000;
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GlbTopology {
    Triangles,
    TriangleStrip,
    TriangleFan,
}
#[derive(Debug, Clone, Copy)]
pub struct GlbPrimitive<'a> {
    document: &'a GlbDocument,
    source: JsonValue<'a>,
    positions: GlbAccessor<'a>,
    indices: Option<GlbAccessor<'a>>,
    topology: GlbTopology,
    triangles: usize,
    bounds: [[f64; 3]; 2],
    position_bounds: [[f64; 3]; 2],
    material: Option<usize>,
}
impl<'a> GlbPrimitive<'a> {
    pub fn vertex_streams(
        self,
        request: &DecodeRequest,
    ) -> Result<crate::glb::GlbVertexStreams<'a>, GlbError> {
        let material = self.document.material(self.material, request)?;
        crate::glb::GlbVertexStreams::new(self, material, request)
    }
    pub fn topology(self) -> GlbTopology {
        self.topology
    }
    pub fn triangle_count(self) -> usize {
        self.triangles
    }
    /// Bounds of referenced geometry; unreferenced positions do not affect fitting.
    pub fn bounds(self) -> [[f64; 3]; 2] {
        self.bounds
    }
    pub fn position_bounds(self) -> [[f64; 3]; 2] {
        self.position_bounds
    }
    pub fn positions(self) -> GlbAccessor<'a> {
        self.positions
    }
    pub fn indices(self) -> Option<GlbAccessor<'a>> {
        self.indices
    }
    pub fn material_index(self) -> Option<usize> {
        self.material
    }
    pub fn source(self) -> JsonValue<'a> {
        self.source
    }
    pub fn attribute(self, name: &str, request: &DecodeRequest) -> Result<Option<GlbAccessor<'a>>, GlbError> {
        required(self.source, "attributes", request)?
            .field(name, request)?
            .map(|index| self.document.accessor(integer(index)?, request))
            .transpose()
    }
    fn index(self, at: usize) -> u32 {
        self.indices
            .map_or(at as u32, |indices| indices.unsigned(at).unwrap())
    }
    pub fn triangle_indices(self, at: usize) -> Option<[u32; 3]> {
        if at >= self.triangles {
            return None;
        }
        Some(
            match self.topology {
                GlbTopology::Triangles => [at * 3, at * 3 + 1, at * 3 + 2],
                GlbTopology::TriangleStrip => [at, at + 1 + at % 2, at + 2 - at % 2],
                GlbTopology::TriangleFan => [at + 1, at + 2, 0],
            }
            .map(|i| self.index(i)),
        )
    }
    pub fn triangle(self, at: usize) -> Option<[[f64; 3]; 3]> {
        self.triangle_indices(at).map(|indices| {
            indices.map(|index| std::array::from_fn(|c| self.positions.component(index as usize, c).unwrap()))
        })
    }
    pub fn triangles(self) -> impl ExactSizeIterator<Item = [[f64; 3]; 3]> + Clone + 'a {
        (0..self.triangles).map(move |i| self.triangle(i).unwrap())
    }
}
#[derive(Clone, Copy)]
enum Semantic {
    Position,
    Normal,
    Tangent,
    Texcoord,
    Color,
    Joints,
    Weights,
    Custom,
}
fn semantic(name: JsonValue<'_>) -> Result<(Semantic, Option<(usize, usize)>), GlbError> {
    for (name_text, kind) in [
        ("POSITION", Semantic::Position),
        ("NORMAL", Semantic::Normal),
        ("TANGENT", Semantic::Tangent),
    ] {
        if name.string_eq(name_text) {
            return Ok((kind, None));
        }
    }
    if name.string_chars().is_some_and(|mut c| c.next() == Some('_')) {
        return Ok((Semantic::Custom, None));
    }
    for (family, (prefix, kind)) in [
        ("TEXCOORD_", Semantic::Texcoord),
        ("COLOR_", Semantic::Color),
        ("JOINTS_", Semantic::Joints),
        ("WEIGHTS_", Semantic::Weights),
    ]
    .into_iter()
    .enumerate()
    {
        let mut chars = name.string_chars().unwrap();
        if !prefix.chars().all(|c| chars.next() == Some(c)) {
            continue;
        }
        let digits = chars;
        let mut index = 0usize;
        let mut count = 0;
        let mut first = '0';
        for digit in digits {
            if !digit.is_ascii_digit() {
                return Err(GlbError::Invalid("attribute semantic index"));
            }
            if count == 0 {
                first = digit;
            }
            count += 1;
            index = index
                .checked_mul(10)
                .and_then(|v| v.checked_add(digit as usize - '0' as usize))
                .ok_or(GlbError::Limit("attribute set index"))?;
            if index >= 64 {
                return Err(GlbError::Limit("attribute set index"));
            }
        }
        if count == 0 || (count > 1 && first == '0') {
            return Err(GlbError::Invalid("attribute semantic index"));
        }
        return Ok((kind, Some((family, index))));
    }
    Err(GlbError::Invalid("unknown attribute semantic"))
}
fn validate_type(accessor: GlbAccessor<'_>, semantic: Semantic) -> Result<(), GlbError> {
    let float = accessor.component_type() == GlbComponent::F32 && !accessor.normalized();
    let unorm =
        matches!(accessor.component_type(), GlbComponent::U8 | GlbComponent::U16) && accessor.normalized();
    let shape = accessor.shape();
    let valid = match semantic {
        Semantic::Position | Semantic::Normal => float && shape == GlbShape::Vec3,
        Semantic::Tangent => float && shape == GlbShape::Vec4,
        Semantic::Texcoord => (float || unorm) && shape == GlbShape::Vec2,
        Semantic::Color => (float || unorm) && matches!(shape, GlbShape::Vec3 | GlbShape::Vec4),
        Semantic::Joints => {
            matches!(accessor.component_type(), GlbComponent::U8 | GlbComponent::U16)
                && !accessor.normalized()
                && shape == GlbShape::Vec4
        }
        Semantic::Weights => (float || unorm) && shape == GlbShape::Vec4,
        Semantic::Custom => true,
    };
    if valid {
        Ok(())
    } else {
        Err(GlbError::Invalid("attribute component/type"))
    }
}
fn update(bounds: &mut [[f64; 3]; 2], position: [f64; 3]) {
    for c in 0..3 {
        bounds[0][c] = bounds[0][c].min(position[c]);
        bounds[1][c] = bounds[1][c].max(position[c]);
    }
}
fn metadata_bounds(object: JsonValue<'_>, request: &DecodeRequest) -> Result<[[f64; 3]; 2], GlbError> {
    let mut result = [[0.; 3]; 2];
    for (index, name) in ["min", "max"].into_iter().enumerate() {
        let mut array = required(object, name, request)?
            .array()
            .ok_or(GlbError::Invalid("POSITION min/max array"))?;
        for c in 0..3 {
            let value = array
                .next()
                .and_then(|v| v.as_f64())
                .ok_or(GlbError::Invalid("POSITION min/max components"))? as f32;
            if !value.is_finite() {
                return Err(GlbError::Invalid("POSITION min/max float32 range"));
            }
            result[index][c] = f64::from(value);
        }
        if array.next().is_some() {
            return Err(GlbError::Invalid("POSITION min/max length"));
        }
    }
    if (0..3).any(|c| result[0][c] > result[1][c]) {
        return Err(GlbError::Invalid("POSITION min/max order"));
    }
    Ok(result)
}
impl GlbDocument {
    /// Validate a triangle primitive and retain only borrowed source views.
    /// Positions remain local: node transforms, morphing, skinning, materials
    /// and shader interpretation are separate scene/rendering responsibilities.
    pub fn primitive(
        &self,
        mesh: usize,
        primitive: usize,
        request: &DecodeRequest,
    ) -> Result<GlbPrimitive<'_>, GlbError> {
        request.check_cancelled()?;
        let mesh = array_item(self.root(), "meshes", mesh, request)?;
        let source = array_item(mesh, "primitives", primitive, request)?;
        if source.field("targets", request)?.is_some() {
            return Err(GlbError::Unsupported("morph targets"));
        }
        let topology = match source
            .field("mode", request)?
            .map(integer)
            .transpose()?
            .unwrap_or(4)
        {
            4 => GlbTopology::Triangles,
            5 => GlbTopology::TriangleStrip,
            6 => GlbTopology::TriangleFan,
            0..=3 => return Err(GlbError::Unsupported("point/line primitive")),
            _ => return Err(GlbError::Invalid("primitive mode")),
        };
        let attributes = required(source, "attributes", request)?;
        let position_index = integer(
            attributes
                .field("POSITION", request)?
                .ok_or(GlbError::Unsupported("primitive without POSITION"))?,
        )?;
        let positions = self.accessor(position_index, request)?;
        positions.validate_vertex()?;
        validate_type(positions, Semantic::Position)?;
        let position_object = array_item(self.root(), "accessors", position_index, request)?;
        let declared_bounds = metadata_bounds(position_object, request)?;
        let mut views = [None; MAX_ATTRIBUTES];
        let mut attribute_count = 0;
        let mut sets = [0u64; 4];
        let mut work = 0usize;
        let mut position_bounds = [[f64::INFINITY; 3], [f64::NEG_INFINITY; 3]];
        let mut entries = [None; MAX_ATTRIBUTES];
        let mut raw_count = 0;
        for entry in attributes
            .object()
            .ok_or(GlbError::Invalid("attributes object"))?
        {
            request.check_cancelled()?;
            if raw_count == MAX_ATTRIBUTES {
                return Err(GlbError::Limit("primitive attribute count"));
            }
            if entry.0.raw_json().len() > 4096 {
                return Err(GlbError::Limit("attribute name bytes"));
            }
            entries[raw_count] = Some(entry);
            raw_count += 1;
        }
        // Last decoded key wins, with bounded stack scratch and key work.
        for at in 0..raw_count {
            request.check_cancelled()?;
            let (name, value) = entries[at].unwrap();
            if entries[at + 1..raw_count]
                .iter()
                .flatten()
                .any(|(later, _)| later.string_chars().unwrap().eq(name.string_chars().unwrap()))
            {
                continue;
            }
            let (semantic, set) = semantic(name)?;
            let accessor = if matches!(semantic, Semantic::Position) {
                positions
            } else {
                self.accessor(integer(value)?, request)?
            };
            accessor.validate_vertex()?;
            if accessor.len() != positions.len() {
                return Err(GlbError::Invalid("attribute count mismatch"));
            }
            validate_type(accessor, semantic)?;
            if let Some((family, index)) = set {
                sets[family] |= 1u64 << index;
            }
            if let Some((view, stride)) = accessor.vertex_view() {
                if !stride
                    && views[..attribute_count]
                        .iter()
                        .flatten()
                        .any(|&(old, _)| old == view)
                {
                    return Err(GlbError::Invalid("shared vertex bufferView requires stride"));
                }
                views[attribute_count] = Some((view, stride));
            }
            attribute_count += 1;
            work = work
                .checked_add(
                    accessor
                        .len()
                        .checked_mul(accessor.shape().components())
                        .ok_or(GlbError::Limit("primitive component work"))?,
                )
                .ok_or(GlbError::Limit("primitive component work"))?;
            if work > MAX_COMPONENT_WORK {
                return Err(GlbError::Limit("primitive component work"));
            }
            for index in 0..accessor.len() {
                if index % 1024 == 0 {
                    request.check_cancelled()?;
                }
                let mut first = [0.; 3];
                for c in 0..accessor.shape().components() {
                    let value = accessor.component(index, c)?;
                    if matches!(semantic, Semantic::Weights) && value < 0. {
                        return Err(GlbError::Invalid("negative vertex weight"));
                    }
                    if matches!(semantic, Semantic::Tangent) && c == 3 && value != 1. && value != -1. {
                        return Err(GlbError::Invalid("tangent handedness"));
                    }
                    if c < 3 {
                        first[c] = value;
                    }
                }
                if matches!(semantic, Semantic::Position) {
                    update(&mut position_bounds, first);
                }
                if matches!(semantic, Semantic::Normal | Semantic::Tangent)
                    && (first.into_iter().map(|v| v * v).sum::<f64>() - 1.).abs() > 1e-4
                {
                    return Err(GlbError::Invalid("non-unit normal/tangent"));
                }
            }
        }
        if sets.into_iter().any(|mask| mask & (mask.wrapping_add(1)) != 0) {
            return Err(GlbError::Invalid("nonconsecutive attribute sets"));
        }
        if (position_object.field("bufferView", request)?.is_some()
            || position_object.field("sparse", request)?.is_some())
            && declared_bounds != position_bounds
        {
            return Err(GlbError::Invalid("POSITION min/max mismatch"));
        }
        let indices = source
            .field("indices", request)?
            .map(|index| self.accessor(integer(index)?, request))
            .transpose()?;
        let mut bounds = position_bounds;
        if let Some(indices) = indices {
            indices.validate_indices()?;
            bounds = [[f64::INFINITY; 3], [f64::NEG_INFINITY; 3]];
            let maximum = match indices.component_type() {
                GlbComponent::U8 => u8::MAX as u32,
                GlbComponent::U16 => u16::MAX as u32,
                _ => u32::MAX,
            };
            for at in 0..indices.len() {
                if at % 1024 == 0 {
                    request.check_cancelled()?;
                }
                let index = indices.unsigned(at)?;
                if index == maximum || index as usize >= positions.len() {
                    return Err(GlbError::Invalid("primitive index range/restart"));
                }
                update(
                    &mut bounds,
                    std::array::from_fn(|c| positions.component(index as usize, c).unwrap()),
                );
            }
        }
        let count = indices.map_or(positions.len(), |v| v.len());
        let triangles = match topology {
            GlbTopology::Triangles if count >= 3 && count % 3 == 0 => count / 3,
            GlbTopology::TriangleStrip | GlbTopology::TriangleFan if count >= 3 => count - 2,
            _ => return Err(GlbError::Invalid("primitive topology count")),
        };
        let material = source.field("material", request)?.map(integer).transpose()?;
        if let Some(index) = material {
            if array_item(self.root(), "materials", index, request)?
                .object()
                .is_none()
            {
                return Err(GlbError::Invalid("material object"));
            }
        }
        request.check_cancelled()?;
        Ok(GlbPrimitive {
            document: self,
            source,
            positions,
            indices,
            topology,
            triangles,
            bounds,
            position_bounds,
            material,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::DecodeError;
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
    fn request(name: &str, budget: &MemoryBudget) -> DecodeRequest {
        let mut r = DecodeRequest::new(fixture(name));
        r.memory_budget = Some(budget.clone());
        r
    }
    fn fields(line: &str) -> Vec<f64> {
        line.split('\t').map(|v| v.parse().unwrap()).collect()
    }
    #[test]
    fn original_and_authored_local_triangles_match_independent_oracles_and_fit_used_vertices() {
        for (model, oracle, count) in [
            ("Box.glb", "Box-triangles.tsv", 12),
            ("primitive-controls.glb", "primitive-triangles.tsv", 20),
        ] {
            let budget = MemoryBudget::new(1024 * 1024);
            let request = request(model, &budget);
            let document = GlbDocument::read(&request).unwrap();
            let used = budget.used();
            let peak = budget.peak();
            let mut total = 0;
            for line in std::fs::read_to_string(fixture(oracle)).unwrap().lines() {
                let values = fields(line);
                let mesh = values[0] as usize;
                let index = values[1] as usize;
                let primitive = document.primitive(mesh, 0, &request).unwrap();
                assert_eq!(
                    primitive.triangle_indices(index).unwrap(),
                    [values[2] as u32, values[3] as u32, values[4] as u32]
                );
                let triangle = primitive.triangle(index).unwrap();
                for corner in 0..3 {
                    for c in 0..3 {
                        assert_eq!(
                            triangle[corner][c].to_bits(),
                            values[5 + corner * 3 + c].to_bits(),
                            "{model} mesh{mesh} triangle{index}"
                        );
                    }
                }
                let mut iterator = primitive.triangles();
                assert_eq!(iterator.len(), primitive.triangle_count());
                assert_eq!(iterator.nth(index).unwrap(), triangle);
                assert_eq!(primitive.triangles().clone().count(), primitive.triangle_count());
                assert_eq!(budget.used(), used);
                assert_eq!(budget.peak(), peak);
                total += 1;
            }
            assert_eq!(total, count);
            drop(document);
            assert_eq!(budget.used(), 0);
        }
        let budget = MemoryBudget::new(1024 * 1024);
        let request = request("primitive-controls.glb", &budget);
        let document = GlbDocument::read(&request).unwrap();
        for line in std::fs::read_to_string(fixture("primitive-bounds.tsv"))
            .unwrap()
            .lines()
        {
            let v = fields(line);
            let p = document.primitive(v[0] as usize, 0, &request).unwrap();
            assert_eq!(p.bounds(), [[v[1], v[2], v[3]], [v[4], v[5], v[6]]]);
            assert_eq!(p.position_bounds(), [[v[7], v[8], v[9]], [v[10], v[11], v[12]]]);
            assert!(p.triangle(p.triangle_count()).is_none());
            assert!(p.triangle_indices(usize::MAX).is_none());
        }
        // Independent analytic footprints: the two-unit-wide strip is
        // clockwise; the unit-square fans/triangles are counterclockwise.
        for (mesh, expected_area) in [
            (0, 1.),
            (1, 1.),
            (2, -2.),
            (3, -2.),
            (4, 1.),
            (5, 1.),
            (6, 1.),
            (8, 0.),
        ] {
            let signed_area = document
                .primitive(mesh, 0, &request)
                .unwrap()
                .triangles()
                .map(|p| {
                    ((p[1][0] - p[0][0]) * (p[2][1] - p[0][1]) - (p[1][1] - p[0][1]) * (p[2][0] - p[0][0]))
                        / 2.
                })
                .sum::<f64>();
            assert_eq!(signed_area, expected_area, "analytic mesh {mesh}");
        }
        let p = document.primitive(0, 0, &request).unwrap();
        assert_eq!(p.material_index(), Some(0));
        assert!(p.attribute("NORMAL", &request).unwrap().is_some());
        assert!(p.attribute("TEXCOORD_1", &request).unwrap().is_some());
        assert_eq!(
            p.attribute("TEXCOORD_0", &request)
                .unwrap()
                .unwrap()
                .component(0, 1)
                .unwrap(),
            1.
        );
        assert!(p.attribute("_TEMPERATURE", &request).unwrap().is_some());
        assert!(p.attribute("_MISSING", &request).unwrap().is_none());
        assert_eq!(
            p.source().field("material", &request).unwrap().unwrap().as_u64(),
            Some(0)
        );
        drop(document);
        assert_eq!(budget.used(), 0);
    }
    #[test]
    fn malformed_topology_index_attribute_and_bounds_contracts_refuse_and_release_credit() {
        let budget = MemoryBudget::new(1024 * 1024);
        let mut total = 0;
        for line in std::fs::read_to_string(fixture("primitive-invalid.tsv"))
            .unwrap()
            .lines()
        {
            let (name, hex) = line.split_once('\t').unwrap();
            let data: Vec<u8> = hex
                .as_bytes()
                .chunks_exact(2)
                .map(|b| u8::from_str_radix(std::str::from_utf8(b).unwrap(), 16).unwrap())
                .collect();
            let path =
                std::env::temp_dir().join(format!("rrrah-primitive-bad-{}-{total}.glb", std::process::id()));
            std::fs::write(&path, data).unwrap();
            let mut request = DecodeRequest::new(&path);
            request.memory_budget = Some(budget.clone());
            let document = GlbDocument::read(&request).unwrap();
            std::fs::remove_file(path).unwrap();
            let used = budget.used();
            let peak = budget.peak();
            let result = document.primitive(0, 0, &request);
            assert!(result.is_err(), "{name}");
            assert_eq!(budget.used(), used);
            assert_eq!(budget.peak(), peak);
            drop(document);
            assert_eq!(budget.used(), 0);
            total += 1;
        }
        assert_eq!(total, 30);
    }
    #[test]
    fn cancelled_resolution_preserves_immutable_geometry_for_new_generation() {
        let budget = MemoryBudget::new(1024 * 1024);
        let mut request = request("Box.glb", &budget);
        let generation = Arc::new(AtomicU64::new(1));
        request.cancellation = Some(crate::GenerationToken::new(generation.clone(), 1));
        let document = GlbDocument::read(&request).unwrap();
        generation.store(2, Ordering::Release);
        assert!(matches!(
            document.primitive(0, 0, &request),
            Err(GlbError::Source(DecodeError::Cancelled))
        ));
        request.cancellation = Some(crate::GenerationToken::new(generation, 2));
        assert_eq!(document.primitive(0, 0, &request).unwrap().triangle_count(), 12);
        let used = budget.used();
        let clone = document.clone();
        drop(document);
        assert_eq!(budget.used(), used);
        drop(clone);
        assert_eq!(budget.used(), 0);
    }
}
