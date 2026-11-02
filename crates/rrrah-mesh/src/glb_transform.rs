//! Affine node math. Local matrices are checked for TRS decomposability;
//! composed global matrices may contain shear from valid parent/child TRS.
use crate::{
    DecodeRequest,
    glb::{GlbDocument, GlbError, JsonValue},
    glb_accessor::array_item,
};

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GlbTransform {
    columns: [f64; 16],
}
impl GlbTransform {
    pub const fn identity() -> Self {
        Self {
            columns: [1., 0., 0., 0., 0., 1., 0., 0., 0., 0., 1., 0., 0., 0., 0., 1.],
        }
    }
    pub fn columns(self) -> [f64; 16] {
        self.columns
    }
    /// Validate a local glTF matrix, retaining supplied column-major values.
    pub fn from_local_columns(columns: [f64; 16]) -> Result<Self, GlbError> {
        let value = Self { columns };
        value.finite()?;
        if columns[3] != 0. || columns[7] != 0. || columns[11] != 0. || columns[15] != 1. {
            return Err(GlbError::Invalid("node matrix is not affine"));
        }
        let mut axes = [[0.; 3]; 3];
        let mut present = [false; 3];
        for c in 0..3 {
            let v = [columns[4 * c], columns[4 * c + 1], columns[4 * c + 2]];
            if let Some(unit) = unit(v) {
                axes[c] = unit;
                present[c] = true;
            }
        }
        for a in 0..3 {
            for b in a + 1..3 {
                if present[a] && present[b] && dot(axes[a], axes[b]).abs() > 1e-6 {
                    return Err(GlbError::Invalid("node matrix contains shear"));
                }
            }
        }
        Ok(value)
    }
    pub fn from_trs(translation: [f64; 3], rotation: [f64; 4], scale: [f64; 3]) -> Result<Self, GlbError> {
        if translation
            .into_iter()
            .chain(rotation)
            .chain(scale)
            .any(|v| !v.is_finite())
        {
            return Err(GlbError::Invalid("non-finite node TRS"));
        }
        if rotation.into_iter().any(|v| v.abs() > 1.) {
            return Err(GlbError::Invalid("node quaternion component range"));
        }
        let largest = rotation.into_iter().map(f64::abs).fold(0., f64::max);
        if largest == 0. {
            return Err(GlbError::Invalid("zero node quaternion"));
        }
        let scaled = rotation.map(|v| v / largest);
        let length = scaled.into_iter().map(|v| v * v).sum::<f64>().sqrt();
        let norm = largest * length;
        if !norm.is_finite() || (norm - 1.).abs() > 1e-6 {
            return Err(GlbError::Invalid("non-unit node quaternion"));
        }
        let [x, y, z, w] = scaled.map(|v| v / length);
        let mut m = [
            1. - 2. * (y * y + z * z),
            2. * (x * y + z * w),
            2. * (x * z - y * w),
            0.,
            2. * (x * y - z * w),
            1. - 2. * (x * x + z * z),
            2. * (y * z + x * w),
            0.,
            2. * (x * z + y * w),
            2. * (y * z - x * w),
            1. - 2. * (x * x + y * y),
            0.,
            translation[0],
            translation[1],
            translation[2],
            1.,
        ];
        for c in 0..3 {
            for r in 0..3 {
                m[c * 4 + r] *= scale[c];
            }
        }
        let result = Self { columns: m };
        result
            .finite()
            .map_err(|_| GlbError::Limit("node transform arithmetic range"))?;
        Ok(result)
    }
    /// Parent * local; preserves valid world shear and reports numeric overflow.
    pub fn compose(self, local: Self) -> Result<Self, GlbError> {
        let a = self.columns;
        let b = local.columns;
        let mut m = Self::identity().columns;
        for c in 0..3 {
            for r in 0..3 {
                m[c * 4 + r] = (0..3).map(|k| a[k * 4 + r] * b[c * 4 + k]).sum();
            }
        }
        for r in 0..3 {
            m[12 + r] = (0..3).map(|k| a[k * 4 + r] * b[12 + k]).sum::<f64>() + a[12 + r];
        }
        let result = Self { columns: m };
        result
            .finite()
            .map_err(|_| GlbError::Limit("node transform arithmetic range"))?;
        Ok(result)
    }
    pub fn point(self, p: [f64; 3]) -> Result<[f64; 3], GlbError> {
        if p.into_iter().any(|v| !v.is_finite()) {
            return Err(GlbError::Invalid("non-finite transform point"));
        }
        let out = std::array::from_fn(|r| {
            (0..3).map(|c| self.columns[4 * c + r] * p[c]).sum::<f64>() + self.columns[12 + r]
        });
        if out.into_iter().any(|v| !v.is_finite()) {
            return Err(GlbError::Limit("transformed point arithmetic range"));
        }
        Ok(out)
    }
    /// -1 mirrored, +1 orientation preserving, 0 singular. A near-singular
    /// normalized determinant explicitly refuses rather than guessing a sign.
    pub fn orientation(self) -> Result<i8, GlbError> {
        let (axes, scales) = self.scaled_axes();
        if scales.contains(&0.) {
            return Ok(0);
        }
        let det = dot(axes[0], cross(axes[1], axes[2]));
        if det == 0. {
            return Ok(0);
        }
        if det.abs() < 1e-12 {
            return Err(GlbError::Unsupported("ill-conditioned transform orientation"));
        }
        Ok(if det < 0. { -1 } else { 1 })
    }
    /// Inverse-transpose direction, normalized without mutating source normals.
    /// Per-column scaling and binary exponents avoid forming huge inverse values.
    pub fn normal(self, normal: [f64; 3]) -> Result<[f64; 3], GlbError> {
        if normal.into_iter().any(|v| !v.is_finite()) {
            return Err(GlbError::Invalid("non-finite transform normal"));
        }
        if unit(normal).is_none() {
            return Err(GlbError::Invalid("zero transform normal"));
        }
        let (axes, scales) = self.scaled_axes();
        if scales.contains(&0.) {
            return Err(GlbError::Unsupported("singular normal transform"));
        }
        let cofactors = [
            cross(axes[1], axes[2]),
            cross(axes[2], axes[0]),
            cross(axes[0], axes[1]),
        ];
        let det = dot(axes[0], cofactors[0]);
        if det.abs() < 1e-12 {
            return Err(GlbError::Unsupported(
                "singular or ill-conditioned normal transform",
            ));
        }
        let mut ratios = [(0., i32::MIN); 3];
        let mut largest = i32::MIN;
        for i in 0..3 {
            if normal[i] != 0. {
                let (n, ne) = binary_parts(normal[i]);
                let (s, se) = binary_parts(scales[i]);
                ratios[i] = (n / s, ne - se);
                largest = largest.max(ne - se);
            }
        }
        let weights = ratios.map(|(mantissa, exponent)| {
            if mantissa == 0. {
                0.
            } else {
                mantissa * negative_power_of_two(exponent - largest)
            }
        });
        let sign = if det < 0. { -1. } else { 1. };
        let out = std::array::from_fn(|r| sign * (0..3).map(|c| cofactors[c][r] * weights[c]).sum::<f64>());
        unit(out).ok_or(GlbError::Limit("normal transform precision"))
    }
    fn scaled_axes(self) -> ([[f64; 3]; 3], [f64; 3]) {
        let mut axes = [[0.; 3]; 3];
        let mut scales = [0.; 3];
        for c in 0..3 {
            scales[c] = (0..3).map(|r| self.columns[c * 4 + r].abs()).fold(0., f64::max);
            if scales[c] != 0. {
                axes[c] = std::array::from_fn(|r| self.columns[c * 4 + r] / scales[c]);
            }
        }
        (axes, scales)
    }
    fn finite(self) -> Result<(), GlbError> {
        if self.columns.into_iter().any(|v| !v.is_finite()) {
            Err(GlbError::Invalid("node transform overflow/non-finite"))
        } else {
            Ok(())
        }
    }
}
fn negative_power_of_two(exponent: i32) -> f64 {
    if exponent < -1074 {
        0.
    } else if exponent < -1022 {
        f64::from_bits(1u64 << (exponent + 1074))
    } else {
        f64::from_bits(((exponent + 1023) as u64) << 52)
    }
}
fn binary_parts(value: f64) -> (f64, i32) {
    let bits = value.to_bits();
    let exponent = ((bits >> 52) & 0x7ff) as i32;
    if exponent == 0 {
        let (m, e) = binary_parts(value * 18_014_398_509_481_984.);
        return (m, e - 54);
    }
    let mantissa = f64::from_bits((bits & 0x800f_ffff_ffff_ffff) | (1023u64 << 52));
    (mantissa, exponent - 1023)
}
fn dot(a: [f64; 3], b: [f64; 3]) -> f64 {
    a.into_iter().zip(b).map(|(a, b)| a * b).sum()
}
fn cross(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}
fn unit(value: [f64; 3]) -> Option<[f64; 3]> {
    let scale = value.into_iter().map(f64::abs).fold(0., f64::max);
    if scale == 0. {
        return None;
    }
    let scaled = value.map(|v| v / scale);
    let length = dot(scaled, scaled).sqrt();
    Some(scaled.map(|v| v / length))
}
fn array<const N: usize>(value: JsonValue<'_>) -> Result<[f64; N], GlbError> {
    let mut values = value.array().ok_or(GlbError::Invalid("node transform array"))?;
    let mut result = [0.; N];
    for out in &mut result {
        *out = values
            .next()
            .and_then(|v| v.as_f64())
            .ok_or(GlbError::Invalid("node transform array value/length"))?;
    }
    if values.next().is_some() {
        return Err(GlbError::Invalid("node transform array length"));
    }
    Ok(result)
}
impl GlbDocument {
    /// Local transform only: this does not resolve parents, select a scene,
    /// apply animations/skins or register the asset as a viewable model.
    pub fn node_local_transform(
        &self,
        index: usize,
        request: &DecodeRequest,
    ) -> Result<GlbTransform, GlbError> {
        request.check_cancelled()?;
        let node = array_item(self.root(), "nodes", index, request)?;
        transform_from_json(node, request)
    }
}
pub(crate) fn transform_from_json(
    node: JsonValue<'_>,
    request: &DecodeRequest,
) -> Result<GlbTransform, GlbError> {
    let matrix = node.field("matrix", request)?;
    let translation = node.field("translation", request)?;
    let rotation = node.field("rotation", request)?;
    let scale = node.field("scale", request)?;
    let result = if let Some(matrix) = matrix {
        if translation.is_some() || rotation.is_some() || scale.is_some() {
            return Err(GlbError::Invalid("node matrix and TRS coexist"));
        }
        GlbTransform::from_local_columns(array(matrix)?)?
    } else {
        GlbTransform::from_trs(
            translation.map(array).transpose()?.unwrap_or([0.; 3]),
            rotation.map(array).transpose()?.unwrap_or([0., 0., 0., 1.]),
            scale.map(array).transpose()?.unwrap_or([1.; 3]),
        )?
    };
    request.check_cancelled()?;
    Ok(result)
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
    fn close(actual: [f64; 3], expected: [f64; 3]) {
        for c in 0..3 {
            assert!(
                (actual[c] - expected[c]).abs() <= 2e-14 * expected[c].abs().max(1.),
                "{actual:?} != {expected:?}"
            );
        }
    }
    #[test]
    fn decimal_local_points_and_gauss_jordan_normals_match_without_new_credit() {
        let budget = MemoryBudget::new(16384);
        let request = request("transform-controls.glb", &budget);
        let document = GlbDocument::read(&request).unwrap();
        let used = budget.used();
        let peak = budget.peak();
        for file in ["transform-points.tsv", "transform-normals.tsv"] {
            let mut cases = 0;
            for line in std::fs::read_to_string(fixture(file)).unwrap().lines() {
                let v: Vec<f64> = line.split('\t').map(|v| v.parse().unwrap()).collect();
                let index = v[0] as usize;
                let transform = if index == 100 {
                    GlbTransform::from_trs([0.; 3], [0., 0., 0., 1.], [2., 1., 3.])
                        .unwrap()
                        .compose(document.node_local_transform(1, &request).unwrap())
                        .unwrap()
                } else {
                    document.node_local_transform(index, &request).unwrap()
                };
                let input = [v[1], v[2], v[3]];
                let expected = [v[4], v[5], v[6]];
                close(
                    if file == "transform-points.tsv" {
                        transform.point(input).unwrap()
                    } else {
                        transform.normal(input).unwrap()
                    },
                    expected,
                );
                assert_eq!(budget.used(), used);
                assert_eq!(budget.peak(), peak);
                cases += 1;
            }
            assert_eq!(cases, if file == "transform-points.tsv" { 40 } else { 35 });
        }
        assert_eq!(
            document
                .node_local_transform(2, &request)
                .unwrap()
                .orientation()
                .unwrap(),
            -1
        );
        for index in [3, 4] {
            let t = document.node_local_transform(index, &request).unwrap();
            assert_eq!(t.orientation().unwrap(), 0);
            assert!(t.normal([0., 0., 1.]).is_err());
            assert!(t.point([1., 2., 3.]).is_ok());
        }
        drop(document);
        assert_eq!(budget.used(), 0);
    }
    #[test]
    fn original_box_node_matrix_transforms_all_local_triangle_corners() {
        let budget = MemoryBudget::new(4304);
        let request = request("Box.glb", &budget);
        let document = GlbDocument::read(&request).unwrap();
        let transform = document
            .node_local_transform(0, &request)
            .unwrap()
            .compose(document.node_local_transform(1, &request).unwrap())
            .unwrap();
        assert_eq!(transform.orientation().unwrap(), 1);
        close(transform.normal([0., 0., 1.]).unwrap(), [0., 1., 0.]);
        let primitive = document.primitive(0, 0, &request).unwrap();
        let mut count = 0;
        for line in std::fs::read_to_string(fixture("Box-node-triangles.tsv"))
            .unwrap()
            .lines()
        {
            let v: Vec<f64> = line.split('\t').map(|v| v.parse().unwrap()).collect();
            let triangle = primitive.triangle(v[0] as usize).unwrap();
            for c in 0..3 {
                close(
                    transform.point(triangle[c]).unwrap(),
                    [v[1 + c * 3], v[2 + c * 3], v[3 + c * 3]],
                );
            }
            count += 1;
        }
        assert_eq!(count, 12);
        assert_eq!(budget.peak(), 4304);
        drop(document);
        assert_eq!(budget.used(), 0);
    }
    #[test]
    fn malformed_nodes_cancel_and_last_owner_cleanup_keep_source_exact() {
        let budget = MemoryBudget::new(16384);
        let mut count = 0;
        for line in std::fs::read_to_string(fixture("transform-invalid.tsv"))
            .unwrap()
            .lines()
        {
            let (name, hex) = line.split_once('\t').unwrap();
            let bytes: Vec<u8> = hex
                .as_bytes()
                .chunks_exact(2)
                .map(|b| u8::from_str_radix(std::str::from_utf8(b).unwrap(), 16).unwrap())
                .collect();
            let path =
                std::env::temp_dir().join(format!("rrrah-node-bad-{}-{count}.glb", std::process::id()));
            std::fs::write(&path, bytes).unwrap();
            let mut request = DecodeRequest::new(&path);
            request.memory_budget = Some(budget.clone());
            let document = GlbDocument::read(&request).unwrap();
            std::fs::remove_file(path).unwrap();
            let used = budget.used();
            assert!(document.node_local_transform(0, &request).is_err(), "{name}");
            assert_eq!(budget.used(), used);
            drop(document);
            assert_eq!(budget.used(), 0);
            count += 1;
        }
        assert_eq!(count, 21);
        let mut request = request("Box.glb", &budget);
        let generation = Arc::new(AtomicU64::new(1));
        request.cancellation = Some(crate::GenerationToken::new(generation.clone(), 1));
        let document = GlbDocument::read(&request).unwrap();
        generation.store(2, Ordering::Release);
        assert!(matches!(
            document.node_local_transform(0, &request),
            Err(GlbError::Source(DecodeError::Cancelled))
        ));
        request.cancellation = Some(crate::GenerationToken::new(generation, 2));
        assert!(document.node_local_transform(0, &request).is_ok());
        let clone = document.clone();
        let used = budget.used();
        drop(document);
        assert_eq!(budget.used(), used);
        drop(clone);
        assert_eq!(budget.used(), 0);
    }
    #[test]
    fn global_shear_extreme_scales_and_numeric_failures_are_explicit() {
        let parent = GlbTransform::from_trs([1., 2., 3.], [0., 0., 0., 1.], [2., 1., 3.]).unwrap();
        let child = GlbTransform::from_trs([10., 20., 30.], [0., 0., 0.6, 0.8], [2., 3., 4.]).unwrap();
        let world = parent.compose(child).unwrap();
        assert!(GlbTransform::from_local_columns(world.columns()).is_err());
        close(
            world.point([1., 2., 3.]).unwrap(),
            parent.point(child.point([1., 2., 3.]).unwrap()).unwrap(),
        );
        for scales in [
            [1e300, 1e-300, 1.],
            [f64::from_bits(1), 1., 1.],
            [-1e300, 1e-300, 1.],
        ] {
            let transform = GlbTransform::from_trs([0.; 3], [0., 0., 0., 1.], scales).unwrap();
            assert_eq!(transform.point([1.; 3]).unwrap(), scales);
            let n = transform.normal([1.; 3]).unwrap();
            assert!(n.into_iter().all(f64::is_finite));
            assert!((dot(n, n) - 1.).abs() < 1e-14);
            if scales[0].abs() == f64::from_bits(1) {
                assert_eq!(n[0], 1.);
                assert_eq!(n[1].to_bits(), 1);
                assert_eq!(n[2].to_bits(), 1);
            } else {
                assert_eq!(n[1], 1.);
                assert!((n[2] / 1e-300 - 1.).abs() < 1e-13);
            }
            assert_eq!(
                transform.orientation().unwrap(),
                if scales[0] < 0. { -1 } else { 1 }
            );
        }
        let flat = GlbTransform::from_trs([0.; 3], [0., 0., 0., 1.], [1e-14, 1., 1.])
            .unwrap()
            .compose(GlbTransform::from_trs([0.; 3], [0., 0., 0.6, 0.8], [1.; 3]).unwrap())
            .unwrap();
        assert!(flat.point([1.; 3]).is_ok());
        assert!(matches!(flat.orientation(), Err(GlbError::Unsupported(_))));
        assert!(matches!(flat.normal([1.; 3]), Err(GlbError::Unsupported(_))));
        let huge = GlbTransform::from_trs([0.; 3], [0., 0., 0., 1.], [f64::MAX; 3]).unwrap();
        assert!(huge.point([2.; 3]).is_err());
        assert!(huge.compose(huge).is_err());
        assert!(GlbTransform::identity().normal([0.; 3]).is_err());
        assert!(GlbTransform::identity().point([f64::NAN, 0., 0.]).is_err());
        assert!(GlbTransform::from_trs([0.; 3], [0., 0., 0., 1.], [f64::INFINITY, 1., 1.]).is_err());
    }
}
