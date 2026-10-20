//! Exact, bounded dyadic control-net identities for coherent patch parameters.
//! Failure to prove an identity never classifies a nearly affine net as affine.
use super::{PixelPartitionError, PixelSourceOwner};
use crate::shading::TensorProductPatch;
use kurbo::{Affine, Point};
/// Source parameter identities proved over all sixteen Bernstein controls.
/// Coefficients are rounded once into device space and shared by every triangle.
/// This is not a certificate for a general inverse or nonlinear branch ordering.
#[derive(Debug, Clone, Copy)]
pub struct PatchParameterIdentity {
    patch_index: usize,
    u: Option<[f64; 3]>,
    v: Option<[f64; 3]>,
}
fn dyadic(value: f64) -> Option<(i128, i32)> {
    if !value.is_finite() {
        return None;
    }
    if value == 0. {
        return Some((0, 0));
    }
    let bits = value.to_bits();
    let exp = ((bits >> 52) & 2047) as i32;
    let mut significand = (bits & ((1u64 << 52) - 1)) | if exp == 0 { 0 } else { 1u64 << 52 };
    let trailing = significand.trailing_zeros();
    significand >>= trailing;
    let power = if exp == 0 { -1074 } else { exp - 1023 - 52 } + trailing as i32;
    let signed = if bits >> 63 != 0 {
        -(significand as i128)
    } else {
        significand as i128
    };
    Some((signed, power))
}
fn coordinates(patch: &TensorProductPatch) -> Option<([[i128; 2]; 16], i32)> {
    let mut binary = [[(0i128, 0i32); 2]; 16];
    let mut exponent = i32::MAX;
    for (i, p) in patch.control_points.iter().enumerate() {
        for (k, value) in [p.x, p.y].into_iter().enumerate() {
            binary[i][k] = dyadic(value)?;
            if binary[i][k].0 != 0 {
                exponent = exponent.min(binary[i][k].1);
            }
        }
    }
    if exponent == i32::MAX {
        exponent = 0;
    }
    let mut points = [[0; 2]; 16];
    for i in 0..16 {
        for k in 0..2 {
            let (number, power) = binary[i][k];
            if number == 0 {
                continue;
            }
            let shift = u32::try_from(power - exponent).ok()?;
            if shift > 126 {
                return None;
            }
            points[i][k] = number.checked_mul(1i128.checked_shl(shift)?)?;
        }
    }
    Some((points, exponent))
}
fn cross(a: [i128; 2], b: [i128; 2]) -> Option<i128> {
    a[0].checked_mul(b[1])?.checked_sub(a[1].checked_mul(b[0])?)
}
fn identity(points: &[[i128; 2]; 16], exponent: i32, indices: [i128; 16]) -> Option<[f64; 3]> {
    let origin = points[0];
    let mut basis = None;
    for i in 1..16 {
        for j in i + 1..16 {
            let a = [
                points[i][0].checked_sub(origin[0])?,
                points[i][1].checked_sub(origin[1])?,
            ];
            let b = [
                points[j][0].checked_sub(origin[0])?,
                points[j][1].checked_sub(origin[1])?,
            ];
            let Some(det) = cross(a, b) else { continue };
            if det != 0 {
                basis = Some((i, j, a, b, det));
                break;
            }
        }
        if basis.is_some() {
            break;
        }
    }
    let (i, j, a, b, det) = basis?;
    let q = indices[i].checked_sub(indices[0])?;
    let r = indices[j].checked_sub(indices[0])?;
    let nx = q.checked_mul(b[1])?.checked_sub(r.checked_mul(a[1])?)?;
    let ny = a[0].checked_mul(r)?.checked_sub(b[0].checked_mul(q)?)?;
    for k in 0..16 {
        let x = points[k][0].checked_sub(origin[0])?;
        let y = points[k][1].checked_sub(origin[1])?;
        let actual = nx.checked_mul(x)?.checked_add(ny.checked_mul(y)?)?;
        let expected = indices[k].checked_sub(indices[0])?.checked_mul(det)?;
        if actual != expected {
            return None;
        }
    }
    let constant = indices[0]
        .checked_mul(det)?
        .checked_sub(nx.checked_mul(origin[0])?)?
        .checked_sub(ny.checked_mul(origin[1])?)?;
    let factor = 2f64.powi(-exponent);
    let result = [
        constant as f64 / det as f64 / 3.,
        nx as f64 / det as f64 * factor / 3.,
        ny as f64 / det as f64 * factor / 3.,
    ];
    if result.iter().any(|v| !v.is_finite())
        || (constant != 0 && result[0] == 0.)
        || (nx != 0 && result[1] == 0.)
        || (ny != 0 && result[2] == 0.)
    {
        return None;
    }
    Some(result)
}
impl PatchParameterIdentity {
    /// Prove affine u/v identities in the original Tensor net, then compose
    /// them with one checked device inverse. Integer overflow or unrepresentable
    /// scale returns no certificate; it never falls back to a tolerance test.
    /// All arithmetic storage is fixed on the stack; no allocation occurs.
    pub fn tensor(patch: &TensorProductPatch, patch_index: usize, transform: Affine) -> Option<Self> {
        let order = [
            [0, 0],
            [0, 1],
            [0, 2],
            [0, 3],
            [1, 3],
            [2, 3],
            [3, 3],
            [3, 2],
            [3, 1],
            [3, 0],
            [2, 0],
            [1, 0],
            [1, 1],
            [1, 2],
            [2, 2],
            [2, 1],
        ];
        Self::from_control_net(patch, patch_index, transform, order)
    }
    /// An affine field of a Coons surface is determined by its four boundary
    /// cubics: their transfinite blend reproduces the affine source parameter.
    /// Check the original twelve controls, without rounded Tensor conversion.
    pub fn coons(patch: &crate::shading::CoonsPatch, patch_index: usize, transform: Affine) -> Option<Self> {
        let net = TensorProductPatch {
            control_points: std::array::from_fn(|i| patch.control_points[if i < 12 { i } else { 0 }]),
            colors: Default::default(),
        };
        let order = [
            [0, 0],
            [0, 1],
            [0, 2],
            [0, 3],
            [1, 3],
            [2, 3],
            [3, 3],
            [3, 2],
            [3, 1],
            [3, 0],
            [2, 0],
            [1, 0],
            [0, 0],
            [0, 0],
            [0, 0],
            [0, 0],
        ];
        Self::from_control_net(&net, patch_index, transform, order)
    }
    fn from_control_net(
        patch: &TensorProductPatch,
        patch_index: usize,
        transform: Affine,
        order: [[i128; 2]; 16],
    ) -> Option<Self> {
        let (points, exponent) = coordinates(patch)?;
        let inverse = transform.inverse().as_coeffs();
        if transform
            .as_coeffs()
            .iter()
            .chain(inverse.iter())
            .any(|v| !v.is_finite())
            || !transform.determinant().is_finite()
            || transform.determinant() == 0.
            || patch.control_points.iter().any(|p| {
                let q = transform * *p;
                !q.x.is_finite() || !q.y.is_finite()
            })
        {
            return None;
        }
        let compose = |field: [f64; 3]| {
            let f = [
                field[1].mul_add(inverse[4], field[2].mul_add(inverse[5], field[0])),
                field[1].mul_add(inverse[0], field[2] * inverse[1]),
                field[1].mul_add(inverse[2], field[2] * inverse[3]),
            ];
            (f.iter().all(|v| v.is_finite()) && (f[1] != 0. || f[2] != 0.)).then_some(f)
        };
        Some(Self {
            patch_index,
            u: identity(&points, exponent, order.map(|p| p[0])).and_then(compose),
            v: identity(&points, exponent, order.map(|p| p[1])).and_then(compose),
        })
    }
    /// Whether global source u was proved affine in source position.
    pub fn has_u(&self) -> bool {
        self.u.is_some()
    }
    /// Whether global source v was proved affine in source position.
    pub fn has_v(&self) -> bool {
        self.v.is_some()
    }
    /// Replace only proved fields with the shared expression in this pixel.
    /// The original field remains when no global identity was established.
    pub fn apply(&self, owner: &mut PixelSourceOwner, origin: Point) -> Result<(), PixelPartitionError> {
        if owner.patch_index != self.patch_index || !origin.x.is_finite() || !origin.y.is_finite() {
            return Err(PixelPartitionError::Invalid);
        }
        let local = |field: [f64; 3]| {
            let result = [
                field[1].mul_add(origin.x, field[2].mul_add(origin.y, field[0])),
                field[1],
                field[2],
            ];
            if result.iter().all(|v| v.is_finite()) {
                Ok(result)
            } else {
                Err(PixelPartitionError::Conditioning)
            }
        };
        let u = self.u.map(local).transpose()?;
        let v = self.v.map(local).transpose()?;
        if let Some(u) = u {
            owner.u = u;
        }
        if let Some(v) = v {
            owner.v = v;
        }
        Ok(())
    }
}
