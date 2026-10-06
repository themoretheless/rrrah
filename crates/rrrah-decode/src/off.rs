//! Indexed OFF meshes; source attributes are retained independently of display shading.
use crate::{DecodeError, DecodeRequest, bounded_io::read_bounded};
use std::ops::Range;
use thiserror::Error;
const BUDGET: usize = 512 * 1024 * 1024;
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OffEncoding {
    Ascii,
    Binary,
}
#[derive(Debug, Clone, PartialEq)]
pub enum OffColor {
    Default,
    Map(i32),
    Bytes(Vec<u8>),
    Floats(Vec<f64>),
}
#[derive(Debug, Clone)]
pub struct OffFace {
    pub indices: Range<usize>,
    pub color: OffColor,
}
#[derive(Debug, Clone)]
pub struct OffMesh {
    pub encoding: OffEncoding,
    pub positions: Vec<[f64; 3]>,
    pub normals: Vec<[f64; 3]>,
    pub colors: Vec<[f64; 4]>,
    pub texcoords: Vec<[f64; 2]>,
    pub indices: Vec<u32>,
    pub faces: Vec<OffFace>,
    pub triangles: Vec<[u32; 3]>,
    pub bounds: Option<[[f64; 3]; 2]>,
    pub declared_edges: u32,
}
#[derive(Debug, Error)]
pub enum OffDecodeError {
    #[error(transparent)]
    Source(#[from] DecodeError),
    #[error("invalid OFF: {0}")]
    Invalid(&'static str),
    #[error("unsupported OFF: {0}")]
    Unsupported(&'static str),
}
type Result<T> = std::result::Result<T, OffDecodeError>;
struct Input<'a> {
    bytes: &'a [u8],
    at: usize,
    binary: bool,
}
impl<'a> Input<'a> {
    fn skip(&mut self) {
        loop {
            while self.bytes.get(self.at).is_some_and(u8::is_ascii_whitespace) {
                self.at += 1;
            }
            if self.bytes.get(self.at) == Some(&b'#') {
                while self
                    .bytes
                    .get(self.at)
                    .is_some_and(|v| !matches!(v, b'\n' | b'\r'))
                {
                    self.at += 1;
                }
            } else {
                break;
            }
        }
    }
    fn token(&mut self) -> Result<&'a str> {
        self.skip();
        let start = self.at;
        while self
            .bytes
            .get(self.at)
            .is_some_and(|v| !v.is_ascii_whitespace() && *v != b'#')
        {
            self.at += 1;
            if self.at - start > 128 {
                return Err(OffDecodeError::Invalid("token limit exceeded"));
            }
        }
        if start == self.at {
            return Err(OffDecodeError::Invalid("missing token"));
        }
        std::str::from_utf8(&self.bytes[start..self.at])
            .map_err(|_| OffDecodeError::Invalid("non-ASCII token"))
    }
    fn word(&mut self) -> Result<[u8; 4]> {
        let end = self
            .at
            .checked_add(4)
            .ok_or(OffDecodeError::Invalid("offset overflow"))?;
        let word = self
            .bytes
            .get(self.at..end)
            .ok_or(OffDecodeError::Invalid("truncated binary payload"))?;
        self.at = end;
        word.try_into()
            .map_err(|_| OffDecodeError::Invalid("binary word size"))
    }
    fn uint(&mut self) -> Result<u32> {
        if self.binary {
            let value = i32::from_be_bytes(self.word()?);
            u32::try_from(value).map_err(|_| OffDecodeError::Invalid("negative count or index"))
        } else {
            self.token()?
                .parse()
                .map_err(|_| OffDecodeError::Invalid("invalid count or index"))
        }
    }
    fn size(&mut self) -> Result<usize> {
        usize::try_from(self.uint()?)
            .map_err(|_| OffDecodeError::Invalid("count or index exceeds address space"))
    }
    fn real(&mut self) -> Result<f64> {
        let value = if self.binary {
            f64::from(f32::from_be_bytes(self.word()?))
        } else {
            self.token()?
                .parse()
                .map_err(|_| OffDecodeError::Invalid("invalid scalar"))?
        };
        if !value.is_finite() {
            return Err(OffDecodeError::Invalid("non-finite scalar"));
        }
        Ok(value)
    }
    fn array<const N: usize>(&mut self) -> Result<[f64; N]> {
        let mut out = [0.; N];
        for item in &mut out {
            *item = self.real()?;
        }
        Ok(out)
    }
    fn face_color(&mut self, budget: &mut GeometryBudget) -> Result<OffColor> {
        if self.binary {
            let n = self.uint()?;
            if n == 0 {
                return Ok(OffColor::Default);
            }
            if !matches!(n, 3 | 4) {
                return Err(OffDecodeError::Invalid("invalid binary face color count"));
            }
            let mut values = Vec::new();
            reserve(
                &mut values,
                usize::try_from(n).map_err(|_| OffDecodeError::Invalid("color count overflow"))?,
                budget,
            )?;
            for _ in 0..n {
                values.push(self.real()?);
            }
            if values.iter().any(|v| !(0.0..=1.0).contains(v)) {
                return Err(OffDecodeError::Invalid("color range"));
            }
            return Ok(OffColor::Floats(values));
        }
        let start = self.at;
        while self
            .bytes
            .get(self.at)
            .is_some_and(|v| !matches!(v, b'\n' | b'\r' | b'#'))
        {
            self.at += 1;
            if self.at - start > 1024 {
                return Err(OffDecodeError::Invalid("face color line limit"));
            }
        }
        let line = std::str::from_utf8(&self.bytes[start..self.at])
            .map_err(|_| OffDecodeError::Invalid("face color text"))?;
        let mut tokens = [""; 4];
        let mut count = 0;
        for token in line.split_ascii_whitespace() {
            if count == tokens.len() {
                return Err(OffDecodeError::Invalid("face color component count"));
            }
            tokens[count] = token;
            count += 1;
        }
        let tokens = &tokens[..count];
        match count {
            0 => Ok(OffColor::Default),
            1 => Ok(OffColor::Map(
                tokens[0]
                    .parse()
                    .map_err(|_| OffDecodeError::Invalid("colormap index"))?,
            )),
            3 | 4 => {
                if tokens.iter().all(|v| v.parse::<i32>().is_ok()) {
                    let mut values = Vec::new();
                    reserve(&mut values, count, budget)?;
                    for token in tokens {
                        values.push(
                            token
                                .parse::<u8>()
                                .map_err(|_| OffDecodeError::Invalid("byte color range"))?,
                        );
                    }
                    Ok(OffColor::Bytes(values))
                } else {
                    let mut values = Vec::new();
                    reserve(&mut values, count, budget)?;
                    for token in tokens {
                        let value = token
                            .parse::<f64>()
                            .map_err(|_| OffDecodeError::Invalid("float color"))?;
                        if !value.is_finite() || !(0.0..=1.0).contains(&value) {
                            return Err(OffDecodeError::Invalid("color range"));
                        }
                        values.push(value);
                    }
                    Ok(OffColor::Floats(values))
                }
            }
            _ => Err(OffDecodeError::Invalid("face color component count")),
        }
    }
}
struct GeometryBudget {
    remaining: usize,
    reservation: Option<rrrah_core::Reservation>,
}
impl GeometryBudget {
    fn new(budget: Option<&rrrah_core::MemoryBudget>) -> Result<Self> {
        let reservation = budget
            .map(|budget| budget.try_reserve(std::mem::size_of::<OffMesh>() as u64))
            .transpose()
            .map_err(DecodeError::from)?;
        Ok(Self {
            remaining: BUDGET,
            reservation,
        })
    }
    fn charge(&mut self, bytes: usize) -> Result<()> {
        let remaining = self
            .remaining
            .checked_sub(bytes)
            .ok_or(OffDecodeError::Invalid("storage budget exceeded"))?;
        if let Some(reservation) = &mut self.reservation {
            let total = reservation
                .bytes()
                .checked_add(bytes as u64)
                .ok_or(OffDecodeError::Invalid("storage overflow"))?;
            reservation.ensure_bytes(total).map_err(DecodeError::from)?;
        }
        self.remaining = remaining;
        Ok(())
    }
}
pub(crate) fn decode_off_for_model(request: &DecodeRequest) -> Result<crate::model::ModelBuffer<OffMesh>> {
    request.check_cancelled()?;
    if request.image_index != 0 {
        return Err(OffDecodeError::Invalid("OFF has no selectable image"));
    }
    let bytes = read_bounded(request)?;
    let (mesh, reservation) = decode_bytes_budgeted(&bytes, request.memory_budget.as_ref(), || {
        request.check_cancelled().map_err(Into::into)
    })?;
    Ok(match reservation {
        Some(reservation) => crate::model::ModelBuffer::from_reserved(mesh, reservation),
        None => std::sync::Arc::new(mesh).into(),
    })
}
fn reserve<T>(values: &mut Vec<T>, extra: usize, budget: &mut GeometryBudget) -> Result<()> {
    let size = extra
        .checked_mul(std::mem::size_of::<T>())
        .ok_or(OffDecodeError::Invalid("storage overflow"))?;
    if size > budget.remaining {
        return Err(OffDecodeError::Invalid("storage budget exceeded"));
    }
    budget.charge(size)?;
    let old_capacity = values.capacity();
    values
        .try_reserve_exact(extra)
        .map_err(|_| OffDecodeError::Invalid("allocation failed"))?;
    let actual = (values.capacity() - old_capacity) * std::mem::size_of::<T>();
    if actual > size {
        budget.charge(actual - size)?;
    }
    Ok(())
}
fn push<T>(values: &mut Vec<T>, value: T, budget: &mut GeometryBudget) -> Result<()> {
    if values.len() == values.capacity() {
        let extra = values
            .len()
            .max(32)
            .min(budget.remaining / std::mem::size_of::<T>());
        if extra == 0 {
            return Err(OffDecodeError::Invalid("storage budget exceeded"));
        }
        reserve(values, extra, budget)?;
    }
    values.push(value);
    Ok(())
}
pub fn decode_off(request: &DecodeRequest) -> Result<OffMesh> {
    request.check_cancelled()?;
    if request.image_index != 0 {
        return Err(OffDecodeError::Invalid("OFF has no selectable image"));
    }
    let bytes = read_bounded(request)?;
    decode_bytes_budgeted(&bytes, request.memory_budget.as_ref(), || {
        request.check_cancelled().map_err(Into::into)
    })
    .map(|(mesh, _)| mesh)
}
#[cfg(test)]
fn decode_bytes(bytes: &[u8], cancel: impl FnMut() -> Result<()>) -> Result<OffMesh> {
    decode_bytes_budgeted(bytes, None, cancel).map(|(mesh, _)| mesh)
}
fn decode_bytes_budgeted(
    bytes: &[u8],
    managed: Option<&rrrah_core::MemoryBudget>,
    mut cancel: impl FnMut() -> Result<()>,
) -> Result<(OffMesh, Option<rrrah_core::Reservation>)> {
    let mut input = Input {
        bytes,
        at: 0,
        binary: false,
    };
    let first = input.token()?;
    let (mut st, mut c, mut n, mut nd) = (false, false, false, false);
    if let Some(mut prefix) = first.strip_suffix("OFF") {
        if let Some(rest) = prefix.strip_prefix("ST") {
            st = true;
            prefix = rest;
        }
        if let Some(rest) = prefix.strip_prefix('C') {
            c = true;
            prefix = rest;
        }
        if let Some(rest) = prefix.strip_prefix('N') {
            n = true;
            prefix = rest;
        }
        if prefix.starts_with('4') {
            return Err(OffDecodeError::Unsupported("homogeneous coordinates"));
        }
        if let Some(rest) = prefix.strip_prefix('n') {
            nd = true;
            prefix = rest;
        }
        if !prefix.is_empty() {
            return Err(OffDecodeError::Unsupported("header variant"));
        }
        let at = input.at;
        if input.token()? == "BINARY" {
            let tail_start = input.at;
            while input.bytes.get(input.at).is_some_and(|v| *v != b'\n') {
                input.at += 1;
                if input.at - tail_start > 65536 {
                    return Err(OffDecodeError::Invalid("binary header limit"));
                }
            }
            let tail = std::str::from_utf8(&bytes[tail_start..input.at])
                .map_err(|_| OffDecodeError::Invalid("binary header"))?;
            if !tail
                .split_once('#')
                .map_or(tail, |(prefix, _)| prefix)
                .trim()
                .is_empty()
                || input.at == bytes.len()
            {
                return Err(OffDecodeError::Invalid("binary header terminator"));
            }
            input.at += 1;
            input.binary = true;
        } else {
            input.at = at;
        }
    } else {
        input.at = 0;
    }
    if nd && input.uint()? != 3 {
        return Err(OffDecodeError::Unsupported("non-3D coordinates"));
    }
    let vertices = input.size()?;
    let faces = input.size()?;
    let declared_edges = input.uint()?;
    // Every vertex and face consumes source bytes. Reject implausible counts before allocation.
    if vertices > bytes.len() / 3 || faces > bytes.len() / 4 {
        return Err(OffDecodeError::Invalid("counts exceed input"));
    }
    let mut mesh = OffMesh {
        encoding: if input.binary {
            OffEncoding::Binary
        } else {
            OffEncoding::Ascii
        },
        positions: Vec::new(),
        normals: Vec::new(),
        colors: Vec::new(),
        texcoords: Vec::new(),
        indices: Vec::new(),
        faces: Vec::new(),
        triangles: Vec::new(),
        bounds: None,
        declared_edges,
    };
    let mut budget = GeometryBudget::new(managed)?;
    reserve(&mut mesh.positions, vertices, &mut budget)?;
    reserve(&mut mesh.faces, faces, &mut budget)?;
    if n {
        reserve(&mut mesh.normals, vertices, &mut budget)?;
    }
    if c {
        reserve(&mut mesh.colors, vertices, &mut budget)?;
    }
    if st {
        reserve(&mut mesh.texcoords, vertices, &mut budget)?;
    }
    for _ in 0..vertices {
        cancel()?;
        let position = input.array()?;
        let bounds = mesh.bounds.get_or_insert([position, position]);
        for axis in 0..3 {
            bounds[0][axis] = bounds[0][axis].min(position[axis]);
            bounds[1][axis] = bounds[1][axis].max(position[axis]);
        }
        mesh.positions.push(position);
        if n {
            mesh.normals.push(input.array()?);
        }
        if c {
            let color = input.array()?;
            if color.iter().any(|v| !(0.0..=1.0).contains(v)) {
                return Err(OffDecodeError::Invalid("vertex color range"));
            }
            mesh.colors.push(color);
        }
        if st {
            mesh.texcoords.push(input.array()?);
        }
    }
    let mut work = 64_000_000;

    for _ in 0..faces {
        cancel()?;
        let count = input.size()?;
        if count < 3 {
            return Err(OffDecodeError::Unsupported("point or line faces"));
        }
        if count > 1024 {
            return Err(OffDecodeError::Invalid("polygon corner limit"));
        }
        let start = mesh.indices.len();
        for _ in 0..count {
            let index = input.uint()?;
            if usize::try_from(index).map_err(|_| OffDecodeError::Invalid("vertex index overflow"))?
                >= vertices
            {
                return Err(OffDecodeError::Invalid("vertex index out of range"));
            }
            push(&mut mesh.indices, index, &mut budget)?;
        }
        let color = input.face_color(&mut budget)?;
        let indices = &mesh.indices[start..];
        if count == 3 {
            push(
                &mut mesh.triangles,
                [indices[0], indices[1], indices[2]],
                &mut budget,
            )?;
        } else {
            let mut point_credit = managed
                .map(|budget| budget.try_reserve(0))
                .transpose()
                .map_err(DecodeError::from)?;
            let mut points = crate::obj::triangulation_buffer(count, [0.0; 3], &mut point_credit).map_err(
                |e| match e {
                    crate::ObjDecodeError::Source(e) => OffDecodeError::Source(e),
                    _ => OffDecodeError::Invalid("polygon scratch allocation"),
                },
            )?;
            for (point, index) in points.iter_mut().zip(indices) {
                let index =
                    usize::try_from(*index).map_err(|_| OffDecodeError::Invalid("vertex index overflow"))?;
                *point = *mesh
                    .positions
                    .get(index)
                    .ok_or(OffDecodeError::Invalid("vertex index out of range"))?;
            }
            let (triangles, _scratch_credit) = crate::obj::triangulate_budgeted(
                &points,
                &mut work,
                &mut || {
                    cancel().map_err(|e| match e {
                        OffDecodeError::Source(e) => crate::ObjDecodeError::Source(e),
                        _ => crate::ObjDecodeError::Invalid("cancel callback error"),
                    })
                },
                managed,
            )
            .map_err(|e| match e {
                crate::ObjDecodeError::Source(e) => OffDecodeError::Source(e),
                crate::ObjDecodeError::Unsupported(e) => OffDecodeError::Unsupported(e),
                crate::ObjDecodeError::Invalid(e) => OffDecodeError::Invalid(e),
            })?;
            for triangle in triangles {
                push(&mut mesh.triangles, triangle.map(|i| indices[i]), &mut budget)?;
            }
        }
        mesh.faces.push(OffFace {
            indices: start..mesh.indices.len(),
            color,
        });
    }
    if !input.binary {
        input.skip();
    }
    if input.at != bytes.len() {
        return Err(OffDecodeError::Invalid("trailing data"));
    }
    if vertices > 0 && faces == 0 {
        return Err(OffDecodeError::Unsupported("point cloud rendering"));
    }
    Ok((mesh, budget.reservation))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn integer_storage_boundaries_are_explicit() {
        for token in ["0", "2147483647", "4294967295"] {
            let mut input = Input {
                bytes: token.as_bytes(),
                at: 0,
                binary: false,
            };
            let expected = usize::try_from(token.parse::<u32>().unwrap());
            match expected {
                Ok(expected) => assert_eq!(input.size().unwrap(), expected),
                Err(_) => assert!(input.size().is_err()),
            }
        }
        for token in ["-1", "4294967296", "18446744073709551615"] {
            let mut input = Input {
                bytes: token.as_bytes(),
                at: 0,
                binary: false,
            };
            assert!(input.size().is_err());
        }
        for value in [i32::MIN, -1, 0, i32::MAX] {
            let bytes = value.to_be_bytes();
            let mut input = Input {
                bytes: &bytes,
                at: 0,
                binary: true,
            };
            if value < 0 {
                assert!(input.size().is_err());
            } else {
                assert_eq!(input.size().unwrap(), usize::try_from(value).unwrap());
            }
        }
    }
    #[test]
    fn face_color_storage_is_admitted_before_allocation() {
        let mut binary = 3_u32.to_be_bytes().to_vec();
        for value in [0.25_f32, 0.5, 1.0] {
            binary.extend_from_slice(&value.to_be_bytes());
        }
        for (bytes, binary, needed) in [
            (&b"1 2 3\n"[..], false, 3_u64),
            (&b"0.25 0.5 1.0\n"[..], false, 24),
            (binary.as_slice(), true, 24),
        ] {
            let base = std::mem::size_of::<OffMesh>() as u64;
            let root = rrrah_core::MemoryBudget::new(base + needed - 1);
            let mut budget = GeometryBudget::new(Some(&root)).unwrap();
            let mut input = Input { bytes, at: 0, binary };
            assert!(matches!(
                input.face_color(&mut budget),
                Err(OffDecodeError::Source(DecodeError::Memory(_)))
            ));
            assert_eq!(root.used(), base);
            if binary {
                assert_eq!(input.at, 4);
            }
            drop(budget);
            assert_eq!(root.used(), 0);
            let root = rrrah_core::MemoryBudget::new(base + needed);
            let mut budget = GeometryBudget::new(Some(&root)).unwrap();
            let mut input = Input { bytes, at: 0, binary };
            let color = input.face_color(&mut budget).unwrap();
            assert_eq!(root.used(), base + needed);
            assert!(matches!(color, OffColor::Bytes(_) | OffColor::Floats(_)));
            drop(color);
            drop(budget);
            assert_eq!(root.used(), 0);
        }
    }
    #[test]
    fn managed_geometry_reserves_before_growth_and_transfers_credit_to_model() {
        for name in [
            "off-attributes.off",
            "off-attributes-binary.off",
            "off-concave-u.off",
        ] {
            let mut request = DecodeRequest::new(root().join(name));
            let expected = crate::decode_model(&request).unwrap();
            let output = expected.capacity_bytes();
            let source = std::fs::metadata(&request.path).unwrap().len();
            let budget = rrrah_core::MemoryBudget::new(
                source + output + if name.contains("concave") { 128 * 1024 } else { 0 },
            );
            request.memory_budget = Some(budget.clone());
            let actual = crate::decode_model(&request).unwrap();
            assert_eq!(actual.capacity_bytes(), output);
            assert_eq!(
                actual.triangles().collect::<Vec<_>>(),
                expected.triangles().collect::<Vec<_>>()
            );
            assert_eq!(budget.used(), output);
            if name.contains("concave") {
                assert!(budget.peak() > source + output);
            } else {
                assert_eq!(budget.peak(), source + output);
            }
            let held = actual.clone();
            drop(actual);
            assert_eq!(budget.used(), output);
            drop(held);
            assert_eq!(budget.used(), 0);
            let tight = rrrah_core::MemoryBudget::new(source + output - 1);
            request.memory_budget = Some(tight.clone());
            assert!(matches!(
                crate::decode_model(&request),
                Err(crate::ModelDecodeError::Off(OffDecodeError::Source(
                    DecodeError::Memory(_)
                )))
            ));
            assert_eq!(tight.used(), 0);
        }
    }
    fn decode(bytes: &[u8]) -> Result<OffMesh> {
        decode_bytes(bytes, || Ok(()))
    }
    fn root() -> std::path::PathBuf {
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/models")
    }
    fn cross(a: [f64; 3], b: [f64; 3], c: [f64; 3]) -> [f64; 3] {
        let u = std::array::from_fn::<_, 3, _>(|i| b[i] - a[i]);
        let v = std::array::from_fn::<_, 3, _>(|i| c[i] - a[i]);
        [
            u[1] * v[2] - u[2] * v[1],
            u[2] * v[0] - u[0] * v[2],
            u[0] * v[1] - u[1] * v[0],
        ]
    }
    fn contains(triangles: &[[[f64; 3]; 3]], point: [f64; 3], axes: [usize; 2]) -> bool {
        triangles.iter().any(|[a, b, c]| {
            let edge = |a: [f64; 3], b: [f64; 3]| {
                (b[axes[0]] - a[axes[0]]) * (point[axes[1]] - a[axes[1]])
                    - (b[axes[1]] - a[axes[1]]) * (point[axes[0]] - a[axes[0]])
            };
            let d = [edge(*a, *b), edge(*b, *c), edge(*c, *a)];
            d.iter().all(|v| *v >= -1e-10) || d.iter().all(|v| *v <= 1e-10)
        })
    }
    #[test]
    fn independent_geometry_oracles_preserve_surface_winding_and_float64() {
        for line in std::fs::read_to_string(root().join("off-manifest.tsv"))
            .unwrap()
            .lines()
            .filter(|v| !v.starts_with('#'))
        {
            let fields: Vec<_> = line.split('\t').collect();
            let path = root().join(fields[0]);
            let model = crate::decode_model(&DecodeRequest::new(&path)).unwrap();
            assert_eq!(model.format_name(), "OFF");
            let actual: Vec<_> = model.triangles().collect();
            let bytes = std::fs::read(root().join(fields[1])).unwrap();
            let expected: Vec<[[f64; 3]; 3]> = bytes
                .chunks_exact(72)
                .map(|v| {
                    std::array::from_fn(|i| {
                        std::array::from_fn(|j| {
                            f64::from_le_bytes(v[(i * 3 + j) * 8..(i * 3 + j + 1) * 8].try_into().unwrap())
                        })
                    })
                })
                .collect();
            assert_eq!(bytes.len() % 72, 0);
            assert_eq!(actual.len(), expected.len(), "{}", fields[0]);
            let summed = |triangles: &[[[f64; 3]; 3]]| {
                triangles.iter().fold([0.; 3], |sum, [a, b, c]| {
                    let n = cross(*a, *b, *c);
                    std::array::from_fn(|i| sum[i] + n[i])
                })
            };
            assert_eq!(summed(&actual), summed(&expected), "{}", fields[0]);
            let bounds = model.bounds().unwrap();
            let axes = if fields[0].contains("yz") { [1, 2] } else { [0, 1] };
            for y in 0..41 {
                for x in 0..37 {
                    let mut p = bounds[0];
                    p[axes[0]] += (bounds[1][axes[0]] - bounds[0][axes[0]]) * (x as f64 + 0.371) / 37.;
                    p[axes[1]] += (bounds[1][axes[1]] - bounds[0][axes[1]]) * (y as f64 + 0.619) / 41.;
                    assert_eq!(
                        contains(&actual, p, axes),
                        contains(&expected, p, axes),
                        "{} at {p:?}",
                        fields[0]
                    );
                }
            }
            if fields[0].contains("offset64") {
                assert_eq!(actual[0][1][0] - actual[0][0][0], 2.);
            }
        }
    }
    #[test]
    fn attributes_binary_and_ascii_match_authored_data() {
        for suffix in ["", "-binary"] {
            let mesh = decode_off(&DecodeRequest::new(
                root().join(format!("off-attributes{suffix}.off")),
            ))
            .unwrap();
            assert_eq!(
                mesh.encoding,
                if suffix.is_empty() {
                    OffEncoding::Ascii
                } else {
                    OffEncoding::Binary
                }
            );
            assert_eq!(mesh.positions, [[-1., -1., 0.], [1., -1., 0.], [0., 1., 0.]]);
            assert_eq!(mesh.normals, [[0., 0., 2.]; 3]);
            assert_eq!(mesh.colors, [[0.25, 0.5, 0.75, 1.]; 3]);
            assert_eq!(mesh.texcoords, [[0., 0.], [0.25, -0.5], [0.5, -1.]]);
            assert_eq!(mesh.declared_edges, 123);
            assert_eq!(mesh.indices, [0, 1, 2]);
            assert_eq!(mesh.faces[0].indices, 0..3);
            assert_eq!(mesh.faces[0].color, OffColor::Floats(vec![0.25, 0.5, 0.75, 1.]));
        }
        let base = "OFF 3 3 0\n0 0 0 1 0 0 0 1 0\n3 0 1 2 12\n3 0 1 2 255 128 0 255 # bytes\n3 0 1 2\n";
        let mesh = decode(base.as_bytes()).unwrap();
        assert_eq!(mesh.faces[0].color, OffColor::Map(12));
        assert_eq!(mesh.faces[1].color, OffColor::Bytes(vec![255, 128, 0, 255]));
        assert_eq!(mesh.faces[2].color, OffColor::Default);
    }
    #[test]
    fn free_format_comments_headerless_dimension_and_malformed_inputs() {
        let text = "OFF 3 1 0 # counts\n0\n0 0 # vertex\n1 0 0\n0 1 0\n3\n0 1 2\n";
        let mesh = decode(text.as_bytes()).unwrap();
        assert_eq!(mesh.triangles, [[0, 1, 2]]);
        assert_eq!(
            decode(text.replace("OFF ", "").as_bytes()).unwrap().positions,
            mesh.positions
        );
        assert_eq!(
            decode(text.replace("OFF ", "nOFF 3 ").as_bytes())
                .unwrap()
                .positions,
            mesh.positions
        );
        assert_eq!(
            decode(text.replace('\n', "\r\n").as_bytes()).unwrap().positions,
            mesh.positions
        );
        for text in [
            "OFF 999999999 0 0",
            "OFF -1 0 0",
            "OFF 3 1 0\nNaN 0 0\n1 0 0\n0 1 0\n3 0 1 2\n",
            "OFF 3 1 0\n0 0 0\n1 0 0\n0 1 0\n3 0 1 3\n",
            "OFF 3 1 0\n0 0 0\n1 0 0\n0 1 0\n3 0 1 2 1 2\n",
            "OFF 0 0 0 junk",
            "OFF BINARY junk\n",
            "NCOFF 0 0 0",
            "4OFF 0 0 0",
            "nOFF 4 0 0 0",
            "OFF 1 0 0\n0 0 0\n",
        ] {
            assert!(decode(text.as_bytes()).is_err(), "accepted {text}");
        }
        let bytes = std::fs::read(root().join("off-attributes-binary.off")).unwrap();
        for end in 0..bytes.len() {
            assert!(decode(&bytes[..end]).is_err(), "accepted prefix {end}");
        }
        let mut trailing = bytes.clone();
        trailing.push(0);
        assert!(decode(&trailing).is_err());
        let header = bytes.iter().position(|v| *v == b'\n').unwrap() + 1;
        let mut negative = bytes.clone();
        negative[header..header + 4].copy_from_slice(&(-1_i32).to_be_bytes());
        assert!(decode(&negative).is_err());
        let mut nan = bytes;
        nan[header + 12..header + 16].copy_from_slice(&f32::NAN.to_be_bytes());
        assert!(decode(&nan).is_err());
    }
    #[test]
    fn cancellation_budget_and_polygon_failures_are_explicit() {
        let bytes = std::fs::read(root().join("off-concave-u.off")).unwrap();
        let mut calls = 0;
        assert!(matches!(
            decode_bytes(&bytes, || {
                calls += 1;
                if calls > 9 {
                    Err(DecodeError::Cancelled.into())
                } else {
                    Ok(())
                }
            }),
            Err(OffDecodeError::Source(DecodeError::Cancelled))
        ));
        let mut request = DecodeRequest::new(root().join("missing.off"));
        request.cancellation = Some(crate::GenerationToken::new(
            std::sync::Arc::new(std::sync::atomic::AtomicU64::new(2)),
            1,
        ));
        assert!(matches!(
            decode_off(&request),
            Err(OffDecodeError::Source(DecodeError::Cancelled))
        ));
        let mut values = Vec::<[f64; 3]>::new();
        let mut budget = GeometryBudget {
            remaining: 23,
            reservation: None,
        };
        assert!(reserve(&mut values, 1, &mut budget).is_err());
        for text in [
            "OFF 4 1 0\n0 0 0\n1 1 0\n1 0 0\n0 1 0\n4 0 1 2 3\n",
            "OFF 4 1 0\n0 0 0\n1 0 0\n1 1 1\n0 1 0\n4 0 1 2 3\n",
        ] {
            assert!(decode(text.as_bytes()).is_err());
        }
    }
}
