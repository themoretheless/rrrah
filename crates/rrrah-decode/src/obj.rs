//! Bounded OBJ polygon geometry. Material references are metadata, never executed.
use crate::{DecodeError, DecodeRequest, bounded_io::read_bounded};
use std::{ops::Range, sync::Arc};
use thiserror::Error;
const BUDGET: usize = 512 * 1024 * 1024;
const MAX_POLYGON: usize = 1024;
const MAX_LINE: usize = 65536;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ObjCorner {
    pub position: u32,
    pub texcoord: Option<u32>,
    pub normal: Option<u32>,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ObjTriangle {
    pub corners: [u32; 3],
    pub face: u32,
}
#[derive(Debug, Clone)]
pub struct ObjFace {
    pub corners: Range<usize>,
    pub object: Arc<str>,
    pub groups: Arc<[String]>,
    pub material: Option<Arc<str>>,
    pub smoothing_group: Option<u32>,
}
#[derive(Debug, Clone, Default)]
pub struct ObjMesh {
    /// Source xyz coordinates and rational-curve weight; omitted w is one.
    pub positions: Vec<[f32; 4]>,
    pub texcoords: Vec<[f32; 3]>,
    pub normals: Vec<[f32; 3]>,
    pub corners: Vec<ObjCorner>,
    pub faces: Vec<ObjFace>,
    pub triangles: Vec<ObjTriangle>,
    pub material_libraries: Vec<String>,
    pub bounds: Option<[[f32; 3]; 2]>,
}
impl ObjMesh {
    pub(crate) fn vertex(&self, index: u32) -> [f32; 3] {
        let v = self.positions[index as usize];
        [v[0], v[1], v[2]]
    }
}
#[derive(Debug, Error)]
pub enum ObjDecodeError {
    #[error(transparent)]
    Source(#[from] DecodeError),
    #[error("invalid OBJ: {0}")]
    Invalid(&'static str),
    #[error("unsupported OBJ: {0}")]
    Unsupported(&'static str),
}
type Result<T> = std::result::Result<T, ObjDecodeError>;

pub fn decode_obj(request: &DecodeRequest) -> Result<ObjMesh> {
    request.check_cancelled()?;
    if request.image_index != 0 {
        return Err(ObjDecodeError::Invalid("OBJ has no selectable image"));
    }
    let bytes = read_bounded(request)?;
    decode_bytes_budgeted(&bytes, request.memory_budget.as_ref(), || {
        request.check_cancelled().map_err(Into::into)
    })
    .map(|(mesh, _)| mesh)
}
struct GeometryBudget {
    remaining: usize,
    reservation: Option<rrrah_core::Reservation>,
}
impl GeometryBudget {
    fn new(budget: Option<&rrrah_core::MemoryBudget>) -> Result<Self> {
        let reservation = budget
            .map(|budget| budget.try_reserve(std::mem::size_of::<ObjMesh>() as u64))
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
            .ok_or(ObjDecodeError::Invalid("geometry budget exceeded"))?;
        if let Some(reservation) = &mut self.reservation {
            let total = reservation
                .bytes()
                .checked_add(bytes as u64)
                .ok_or(ObjDecodeError::Invalid("geometry storage overflow"))?;
            reservation.ensure_bytes(total).map_err(DecodeError::from)?;
        }
        self.remaining = remaining;
        Ok(())
    }
    fn charge_metadata(
        &mut self,
        bytes: usize,
        credits: &mut [&mut Option<rrrah_core::Reservation>],
    ) -> Result<()> {
        let transferred = credits
            .iter()
            .filter_map(|credit| credit.as_ref())
            .map(rrrah_core::Reservation::bytes)
            .sum::<u64>();
        let additional = (bytes as u64)
            .checked_sub(transferred)
            .ok_or(ObjDecodeError::Invalid(
                "metadata credit exceeds reference weight",
            ))?;
        let remaining = self
            .remaining
            .checked_sub(bytes)
            .ok_or(ObjDecodeError::Invalid("geometry budget exceeded"))?;
        if let Some(target) = &mut self.reservation {
            let total = target
                .bytes()
                .checked_add(additional)
                .ok_or(ObjDecodeError::Invalid("metadata credit overflow"))?;
            target.ensure_bytes(total).map_err(DecodeError::from)?;
            for credit in credits {
                if let Some(mut source) = credit.take() {
                    target.merge_from(&mut source).map_err(DecodeError::from)?;
                }
            }
        }
        self.remaining = remaining;
        Ok(())
    }
}
pub(crate) fn decode_obj_for_model(request: &DecodeRequest) -> Result<crate::model::ModelBuffer<ObjMesh>> {
    request.check_cancelled()?;
    if request.image_index != 0 {
        return Err(ObjDecodeError::Invalid("OBJ has no selectable image"));
    }
    let bytes = read_bounded(request)?;
    let (mesh, reservation) = decode_bytes_budgeted(&bytes, request.memory_budget.as_ref(), || {
        request.check_cancelled().map_err(Into::into)
    })?;
    Ok(match reservation {
        Some(reservation) => crate::model::ModelBuffer::from_reserved(mesh, reservation),
        None => Arc::new(mesh).into(),
    })
}
fn push<T>(values: &mut Vec<T>, value: T, budget: &mut GeometryBudget) -> Result<()> {
    let size = std::mem::size_of::<T>();
    if values.len() == values.capacity() {
        let extra = values
            .len()
            .max(32)
            .min((budget.remaining / size).min(u32::MAX as usize - values.len()));
        if extra == 0 {
            return Err(ObjDecodeError::Invalid("geometry budget exceeded"));
        }
        budget.charge(extra * size)?;
        let old_capacity = values.capacity();
        values
            .try_reserve_exact(extra)
            .map_err(|_| ObjDecodeError::Invalid("allocation failed"))?;
        let actual = (values.capacity() - old_capacity) * size;
        if actual > extra * size {
            budget.charge(actual - extra * size)?;
        }
    }
    values.push(value);
    Ok(())
}
fn append_scratch(
    text: &mut String,
    value: &str,
    credit: &mut Option<rrrah_core::Reservation>,
) -> Result<()> {
    let needed = text
        .len()
        .checked_add(value.len())
        .ok_or(ObjDecodeError::Invalid("text scratch overflow"))?;
    if needed > MAX_LINE {
        return Err(ObjDecodeError::Invalid("line limit exceeded"));
    }
    if needed > text.capacity() {
        if let Some(credit) = credit {
            credit.ensure_bytes(needed as u64).map_err(DecodeError::from)?;
        }
        text.try_reserve_exact(needed - text.len())
            .map_err(|_| ObjDecodeError::Invalid("text scratch allocation failed"))?;
        if let Some(credit) = credit {
            credit
                .ensure_bytes(text.capacity() as u64)
                .map_err(DecodeError::from)?;
        }
    }
    text.push_str(value);
    Ok(())
}
fn join_metadata_tokens<'a>(
    tokens: impl Iterator<Item = &'a str> + Clone,
    credit: &mut Option<rrrah_core::Reservation>,
    limit: usize,
) -> Result<String> {
    let mut length = 0_usize;
    for (count, token) in tokens.clone().enumerate() {
        length = length
            .checked_add(token.len())
            .and_then(|n| n.checked_add(usize::from(count > 0)))
            .ok_or(ObjDecodeError::Invalid("metadata string overflow"))?;
    }
    if length > limit {
        return Err(ObjDecodeError::Invalid("metadata string budget exceeded"));
    }
    if let Some(credit) = credit {
        credit.ensure_bytes(length as u64).map_err(DecodeError::from)?;
    }
    let mut name = String::new();
    name.try_reserve_exact(length)
        .map_err(|_| ObjDecodeError::Invalid("metadata scratch allocation failed"))?;
    if let Some(credit) = credit {
        credit
            .ensure_bytes(name.capacity() as u64)
            .map_err(DecodeError::from)?;
    }
    for (index, token) in tokens.enumerate() {
        if index > 0 {
            name.push(' ');
        }
        name.push_str(token);
    }
    Ok(name)
}
fn owned_groups(
    name: &str,
    budget: Option<&rrrah_core::MemoryBudget>,
) -> Result<(Arc<[String]>, Option<rrrah_core::Reservation>)> {
    let count = name.split_whitespace().count();
    let slots = count
        .checked_mul(std::mem::size_of::<String>())
        .ok_or(ObjDecodeError::Invalid("group storage overflow"))?;
    let chars = name
        .split_whitespace()
        .try_fold(0_usize, |sum, token| sum.checked_add(token.len()))
        .ok_or(ObjDecodeError::Invalid("group storage overflow"))?;
    let retained = slots
        .checked_add(chars)
        .ok_or(ObjDecodeError::Invalid("group storage overflow"))?;
    let mut credit = budget
        .map(|budget| budget.try_reserve(retained as u64))
        .transpose()
        .map_err(DecodeError::from)?;
    // Vec and Arc slot arrays may coexist during conversion; strings move without copying.
    let mut vector_credit = budget
        .map(|budget| budget.try_reserve(slots as u64))
        .transpose()
        .map_err(DecodeError::from)?;
    let mut groups = Vec::new();
    groups
        .try_reserve_exact(count)
        .map_err(|_| ObjDecodeError::Invalid("group array allocation failed"))?;
    if let Some(credit) = &mut vector_credit {
        credit
            .ensure_bytes((groups.capacity() * std::mem::size_of::<String>()) as u64)
            .map_err(DecodeError::from)?;
    }
    let mut actual = retained;
    for token in name.split_whitespace() {
        let mut group = String::new();
        group
            .try_reserve_exact(token.len())
            .map_err(|_| ObjDecodeError::Invalid("group string allocation failed"))?;
        if group.capacity() > token.len() {
            actual = actual
                .checked_add(group.capacity() - token.len())
                .ok_or(ObjDecodeError::Invalid("group storage overflow"))?;
            if let Some(credit) = &mut credit {
                credit.ensure_bytes(actual as u64).map_err(DecodeError::from)?;
            }
        }
        group.push_str(token);
        groups.push(group);
    }
    let groups: Arc<[String]> = groups.into();
    drop(vector_credit);
    Ok((groups, credit))
}
fn number(value: Option<&str>) -> Result<f32> {
    value
        .and_then(|v| v.parse::<f32>().ok())
        .filter(|v| v.is_finite())
        .ok_or(ObjDecodeError::Invalid("invalid or non-finite number"))
}
fn index(value: &str, count: usize) -> Result<u32> {
    let index = value
        .parse::<i64>()
        .map_err(|_| ObjDecodeError::Invalid("invalid index"))?;
    let resolved = match index.cmp(&0) {
        std::cmp::Ordering::Greater => index - 1,
        std::cmp::Ordering::Less => i64::try_from(count)
            .map_err(|_| ObjDecodeError::Invalid("declared array exceeds index range"))?
            .checked_add(index)
            .ok_or(ObjDecodeError::Invalid("relative index overflow"))?,
        std::cmp::Ordering::Equal => return Err(ObjDecodeError::Invalid("zero index")),
    };
    let resolved =
        usize::try_from(resolved).map_err(|_| ObjDecodeError::Invalid("index outside declared array"))?;
    if resolved >= count {
        return Err(ObjDecodeError::Invalid("index outside declared array"));
    }
    u32::try_from(resolved).map_err(|_| ObjDecodeError::Invalid("index exceeds storage limit"))
}
fn corner(value: &str, counts: [usize; 3]) -> Result<ObjCorner> {
    let mut parts = value.split('/');
    let position = index(parts.next().unwrap_or_default(), counts[0])?;
    let texture = parts.next();
    let normal = parts.next();
    if parts.next().is_some() || texture == Some("") && normal.is_none() || normal == Some("") {
        return Err(ObjDecodeError::Invalid("invalid face corner"));
    }
    Ok(ObjCorner {
        position,
        texcoord: texture
            .filter(|v| !v.is_empty())
            .map(|v| index(v, counts[1]))
            .transpose()?,
        normal: normal.map(|v| index(v, counts[2])).transpose()?,
    })
}
#[cfg(test)]
fn decode_bytes(bytes: &[u8], cancel: impl FnMut() -> Result<()>) -> Result<ObjMesh> {
    decode_bytes_budgeted(bytes, None, cancel).map(|(mesh, _)| mesh)
}
fn decode_bytes_budgeted(
    bytes: &[u8],
    managed: Option<&rrrah_core::MemoryBudget>,
    mut cancel: impl FnMut() -> Result<()>,
) -> Result<(ObjMesh, Option<rrrah_core::Reservation>)> {
    let text = std::str::from_utf8(bytes).map_err(|_| ObjDecodeError::Invalid("invalid text encoding"))?;
    let text = text.strip_prefix('\u{feff}').unwrap_or(text);
    let mut mesh = ObjMesh::default();
    let mut budget = GeometryBudget::new(managed)?;
    let mut string_budget = 16 * 1024 * 1024_usize;
    let mut work_budget = 64 * 1024 * 1024_usize;
    let mut object_credit = None;
    let mut material_credit = None;
    let mut groups_credit = None;
    let mut object: Arc<str> = Arc::from("");
    let mut groups: Arc<[String]> = Arc::from([]);
    let mut material = None;
    let mut smoothing = None;
    let mut continued_credit = managed
        .map(|budget| budget.try_reserve(0))
        .transpose()
        .map_err(DecodeError::from)?;
    let mut continued = String::new();
    for (line_number, physical) in text.lines().enumerate() {
        if line_number % 1024 == 0 {
            cancel()?;
        }
        if physical.len() + continued.len() > MAX_LINE {
            return Err(ObjDecodeError::Invalid("line limit exceeded"));
        }
        let physical = physical.split('#').next().unwrap_or_default().trim();
        if continued.len() + physical.len() > MAX_LINE {
            return Err(ObjDecodeError::Invalid("line limit exceeded"));
        }
        let continuation = physical.strip_suffix('\\');
        if let Some(line) = continuation {
            append_scratch(&mut continued, line, &mut continued_credit)?;
            append_scratch(&mut continued, " ", &mut continued_credit)?;
            continue;
        }
        let line = if continued.is_empty() {
            physical
        } else {
            append_scratch(&mut continued, physical, &mut continued_credit)?;
            &continued
        };
        let mut tokens = line.split_whitespace();
        match tokens.next().unwrap_or_default() {
            "" => {}
            "v" => {
                let xyz = [
                    number(tokens.next())?,
                    number(tokens.next())?,
                    number(tokens.next())?,
                ];
                let w = tokens.next().map(|v| number(Some(v))).transpose()?.unwrap_or(1.);
                if tokens.next().is_some() {
                    return Err(ObjDecodeError::Unsupported("vertex color or extra components"));
                }
                push(&mut mesh.positions, [xyz[0], xyz[1], xyz[2], w], &mut budget)?;
            }
            "vt" => {
                let u = number(tokens.next())?;
                let v = tokens.next().map(|v| number(Some(v))).transpose()?.unwrap_or(0.);
                let w = tokens.next().map(|v| number(Some(v))).transpose()?.unwrap_or(0.);
                if tokens.next().is_some() {
                    return Err(ObjDecodeError::Invalid("extra texture components"));
                }
                push(&mut mesh.texcoords, [u, v, w], &mut budget)?;
            }
            "vn" => {
                let normal = [
                    number(tokens.next())?,
                    number(tokens.next())?,
                    number(tokens.next())?,
                ];
                if tokens.next().is_some() {
                    return Err(ObjDecodeError::Invalid("extra normal components"));
                }
                push(&mut mesh.normals, normal, &mut budget)?;
            }
            "f" => {
                let start = mesh.corners.len();
                for value in tokens {
                    if mesh.corners.len() - start == MAX_POLYGON {
                        return Err(ObjDecodeError::Invalid("polygon corner limit"));
                    }
                    let corner = corner(
                        value,
                        [mesh.positions.len(), mesh.texcoords.len(), mesh.normals.len()],
                    )?;
                    push(&mut mesh.corners, corner, &mut budget)?;
                }
                let end = mesh.corners.len();
                if end - start < 3 {
                    return Err(ObjDecodeError::Invalid("face has fewer than three corners"));
                }
                let source_corners = &mesh.corners[start..end];
                let layout = (
                    source_corners[0].texcoord.is_some(),
                    source_corners[0].normal.is_some(),
                );
                if source_corners
                    .iter()
                    .any(|v| (v.texcoord.is_some(), v.normal.is_some()) != layout)
                {
                    return Err(ObjDecodeError::Invalid("inconsistent face corner fields"));
                }
                let face_index = mesh.faces.len() as u32;
                if end - start == 3 {
                    push(
                        &mut mesh.triangles,
                        ObjTriangle {
                            corners: [start as u32, (start + 1) as u32, (start + 2) as u32],
                            face: face_index,
                        },
                        &mut budget,
                    )?;
                } else {
                    let mut point_credit = managed
                        .map(|budget| budget.try_reserve(0))
                        .transpose()
                        .map_err(DecodeError::from)?;
                    let mut points = triangulation_buffer(end - start, [0.0; 3], &mut point_credit)?;
                    for (out, corner) in points.iter_mut().zip(source_corners) {
                        *out = mesh.vertex(corner.position).map(f64::from);
                    }
                    let (triangles, _scratch_credit) =
                        triangulate_budgeted(&points, &mut work_budget, &mut cancel, managed)?;
                    for indices in triangles {
                        push(
                            &mut mesh.triangles,
                            ObjTriangle {
                                corners: indices.map(|v| (start + v) as u32),
                                face: face_index,
                            },
                            &mut budget,
                        )?;
                    }
                }
                for corner in &mesh.corners[start..end] {
                    let vertex = mesh.vertex(corner.position);
                    let bounds = mesh.bounds.get_or_insert([vertex, vertex]);
                    for axis in 0..3 {
                        bounds[0][axis] = bounds[0][axis].min(vertex[axis]);
                        bounds[1][axis] = bounds[1][axis].max(vertex[axis]);
                    }
                }
                // Match the conservative cache weight for shared metadata references.
                let metadata = object.len()
                    + material.as_ref().map_or(0, |v: &Arc<str>| v.len())
                    + groups.len() * std::mem::size_of::<String>()
                    + groups.iter().map(String::capacity).sum::<usize>();
                budget.charge_metadata(
                    metadata,
                    &mut [&mut object_credit, &mut material_credit, &mut groups_credit],
                )?;
                push(
                    &mut mesh.faces,
                    ObjFace {
                        corners: start..end,
                        object: object.clone(),
                        groups: groups.clone(),
                        material: material.clone(),
                        smoothing_group: smoothing,
                    },
                    &mut budget,
                )?;
            }
            command @ ("o" | "g" | "usemtl" | "mtllib") => {
                let mut name_credit = managed
                    .map(|budget| budget.try_reserve(0))
                    .transpose()
                    .map_err(DecodeError::from)?;
                let name = join_metadata_tokens(tokens, &mut name_credit, string_budget)?;
                string_budget -= name.len();
                match command {
                    "o" => {
                        let credit = managed
                            .map(|budget| budget.try_reserve(name.len() as u64))
                            .transpose()
                            .map_err(DecodeError::from)?;
                        object = Arc::from(name);
                        object_credit = credit;
                    }
                    "g" => {
                        let (next, credit) = owned_groups(&name, managed)?;
                        groups = next;
                        groups_credit = credit;
                    }
                    "usemtl" => {
                        if name.is_empty() || name == "off" {
                            material = None;
                            material_credit = None;
                        } else {
                            let credit = managed
                                .map(|budget| budget.try_reserve(name.len() as u64))
                                .transpose()
                                .map_err(DecodeError::from)?;
                            material = Some(Arc::from(name));
                            material_credit = credit;
                        }
                    }
                    _ => {
                        for path in name.split_whitespace() {
                            budget.charge(path.len())?;
                            let mut owned = String::new();
                            owned
                                .try_reserve_exact(path.len())
                                .map_err(|_| ObjDecodeError::Invalid("library path allocation failed"))?;
                            if owned.capacity() > path.len() {
                                budget.charge(owned.capacity() - path.len())?;
                            }
                            owned.push_str(path);
                            push(&mut mesh.material_libraries, owned, &mut budget)?;
                        }
                    }
                }
            }
            "s" => {
                smoothing = match tokens.next() {
                    Some("off" | "0") => None,
                    Some("on") => Some(1),
                    Some(value) => Some(
                        value
                            .parse::<u32>()
                            .ok()
                            .filter(|v| *v > 0)
                            .ok_or(ObjDecodeError::Invalid("invalid smoothing group"))?,
                    ),
                    None => return Err(ObjDecodeError::Invalid("missing smoothing group")),
                };
                if tokens.next().is_some() {
                    return Err(ObjDecodeError::Invalid("extra smoothing argument"));
                }
            }
            _ => {
                return Err(ObjDecodeError::Unsupported(
                    "free-form, point, line, display or unknown statement",
                ));
            }
        }
        continued.clear();
    }
    if !continued.is_empty() {
        return Err(ObjDecodeError::Invalid("unfinished line continuation"));
    }
    cancel()?;
    Ok((mesh, budget.reservation))
}
fn cross(a: [f64; 2], b: [f64; 2], c: [f64; 2]) -> f64 {
    (b[0] - a[0]) * (c[1] - a[1]) - (b[1] - a[1]) * (c[0] - a[0])
}
fn on_segment(a: [f64; 2], b: [f64; 2], p: [f64; 2]) -> bool {
    cross(a, b, p).abs() <= 1e-12
        && (0..2).all(|i| p[i] >= a[i].min(b[i]) - 1e-12 && p[i] <= a[i].max(b[i]) + 1e-12)
}
fn intersects(a: [f64; 2], b: [f64; 2], c: [f64; 2], d: [f64; 2]) -> bool {
    let sides = [cross(a, b, c), cross(a, b, d), cross(c, d, a), cross(c, d, b)];
    (sides[0] * sides[1] < 0. && sides[2] * sides[3] < 0.)
        || on_segment(a, b, c)
        || on_segment(a, b, d)
        || on_segment(c, d, a)
        || on_segment(c, d, b)
}
pub(crate) fn triangulation_buffer<T: Copy>(
    length: usize,
    value: T,
    reservation: &mut Option<rrrah_core::Reservation>,
) -> Result<Vec<T>> {
    let bytes = length
        .checked_mul(std::mem::size_of::<T>())
        .ok_or(ObjDecodeError::Invalid("scratch overflow"))?;
    let charge = |reservation: &mut Option<rrrah_core::Reservation>, bytes: usize| -> Result<()> {
        if let Some(reservation) = reservation {
            let total = reservation
                .bytes()
                .checked_add(bytes as u64)
                .ok_or(ObjDecodeError::Invalid("scratch overflow"))?;
            reservation.ensure_bytes(total).map_err(DecodeError::from)?;
        }
        Ok(())
    };
    charge(reservation, bytes)?;
    let mut values = Vec::new();
    values
        .try_reserve_exact(length)
        .map_err(|_| ObjDecodeError::Invalid("scratch allocation failed"))?;
    let actual = values.capacity() * std::mem::size_of::<T>();
    if actual > bytes {
        charge(reservation, actual - bytes)?;
    }
    values.resize(length, value);
    Ok(values)
}
#[cfg(test)]
pub(crate) fn triangulate(
    points: &[[f64; 3]],
    work: &mut usize,
    cancel: &mut impl FnMut() -> Result<()>,
) -> Result<Vec<[usize; 3]>> {
    triangulate_budgeted(points, work, cancel, None).map(|(triangles, _)| triangles)
}
pub(crate) fn triangulate_budgeted(
    points: &[[f64; 3]],
    work: &mut usize,
    cancel: &mut impl FnMut() -> Result<()>,
    budget: Option<&rrrah_core::MemoryBudget>,
) -> Result<(Vec<[usize; 3]>, Option<rrrah_core::Reservation>)> {
    let mut reservation = budget
        .map(|budget| budget.try_reserve(0))
        .transpose()
        .map_err(DecodeError::from)?;

    let count = points.len();
    if count == 3 {
        return Ok((triangulation_buffer(1, [0, 1, 2], &mut reservation)?, reservation));
    }
    let cost = count
        .checked_mul(count)
        .and_then(|v| v.checked_mul(count))
        .ok_or(ObjDecodeError::Invalid("polygon work budget exceeded"))?;
    if cost > *work {
        return Err(ObjDecodeError::Invalid("polygon work budget exceeded"));
    }
    *work -= cost;
    let origin = points[0].map(f64::from);
    let extent = points
        .iter()
        .flat_map(|v| (0..3).map(move |i| (f64::from(v[i]) - origin[i]).abs()))
        .fold(0_f64, f64::max);
    if extent == 0. || !extent.is_finite() {
        return Err(ObjDecodeError::Invalid("collapsed polygon"));
    }
    let mut normalized = triangulation_buffer(count, [0.0; 3], &mut reservation)?;
    for (out, v) in normalized.iter_mut().zip(points) {
        *out = std::array::from_fn(|i| (v[i] - origin[i]) / extent);
    }
    let mut normal = [0.; 3];
    for i in 0..count {
        let a = normalized[i];
        let b = normalized[(i + 1) % count];
        normal[0] += (a[1] - b[1]) * (a[2] + b[2]);
        normal[1] += (a[2] - b[2]) * (a[0] + b[0]);
        normal[2] += (a[0] - b[0]) * (a[1] + b[1]);
    }
    let length = normal.iter().map(|v| v * v).sum::<f64>().sqrt();
    if length < 1e-12 || !length.is_finite() {
        return Err(ObjDecodeError::Invalid("zero-area or self-intersecting polygon"));
    }
    normal = normal.map(|v| v / length);
    if normalized
        .iter()
        .any(|v| v.iter().zip(normal).map(|(a, b)| a * b).sum::<f64>().abs() > 1e-6)
    {
        return Err(ObjDecodeError::Unsupported("non-planar polygon"));
    }
    let axis = (0..3)
        .max_by(|a, b| normal[*a].abs().total_cmp(&normal[*b].abs()))
        .unwrap();
    let projection_axes = match axis {
        0 => [1, 2],
        1 => [0, 2],
        _ => [0, 1],
    };
    let mut projected = triangulation_buffer(count, [0.0; 2], &mut reservation)?;
    for (out, v) in projected.iter_mut().zip(&normalized) {
        *out = [v[projection_axes[0]], v[projection_axes[1]]];
    }
    for i in 0..count {
        if i % 32 == 0 {
            cancel()?;
        }
        let next = (i + 1) % count;
        if projected[i] == projected[next] {
            return Err(ObjDecodeError::Invalid("duplicate adjacent vertex"));
        }
        for j in i + 1..count {
            let other = (j + 1) % count;
            if next == j || other == i {
                continue;
            }
            if intersects(projected[i], projected[next], projected[j], projected[other]) {
                return Err(ObjDecodeError::Invalid("self-intersecting polygon"));
            }
        }
    }
    let area = (0..count)
        .map(|i| {
            let a = projected[i];
            let b = projected[(i + 1) % count];
            a[0] * b[1] - a[1] * b[0]
        })
        .sum::<f64>();
    let sign = area.signum();
    let mut remaining = triangulation_buffer(count, 0_usize, &mut reservation)?;
    for (i, value) in remaining.iter_mut().enumerate() {
        *value = i;
    }
    let mut triangles = triangulation_buffer(count - 2, [0; 3], &mut reservation)?;
    triangles.clear();
    while remaining.len() > 3 {
        cancel()?;
        let n = remaining.len();
        let mut found = false;
        for i in 0..n {
            let ids = [remaining[(i + n - 1) % n], remaining[i], remaining[(i + 1) % n]];
            let [a, b, c] = ids.map(|j| projected[j]);
            let turn = cross(a, b, c) * sign;
            if turn.abs() <= 1e-12 {
                if turn == 0. && on_segment(a, c, b) {
                    remaining.remove(i);
                    found = true;
                    break;
                }
                return Err(ObjDecodeError::Unsupported(
                    "backtracking or numerically ambiguous polygon edge",
                ));
            }
            if turn < 0. {
                continue;
            }
            let occupied = remaining.iter().filter(|j| !ids.contains(j)).any(|j| {
                let p = projected[*j];
                cross(a, b, p) * sign >= -1e-12
                    && cross(b, c, p) * sign >= -1e-12
                    && cross(c, a, p) * sign >= -1e-12
            });
            if !occupied {
                triangles.push(ids);
                remaining.remove(i);
                found = true;
                break;
            }
        }
        if !found {
            return Err(ObjDecodeError::Invalid("polygon cannot be triangulated"));
        }
    }
    if remaining.len() == 3 {
        triangles.push([remaining[0], remaining[1], remaining[2]]);
    }
    Ok((triangles, reservation))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn relative_index_extremes_reject_without_wrapping() {
        assert_eq!(index("-1", 3).unwrap(), 2);
        assert_eq!(index("-3", 3).unwrap(), 0);
        for value in ["0", "-4", "9223372036854775807", "-9223372036854775808"] {
            assert!(index(value, 3).is_err());
        }
        #[cfg(target_pointer_width = "64")]
        assert!(index("-1", usize::MAX).is_err());
    }
    #[test]
    fn group_arrays_account_conversion_overlap_and_transfer_credit_to_faces() {
        let slots = 2 * std::mem::size_of::<String>() as u64;
        let retained = slots + 6;
        let root = rrrah_core::MemoryBudget::new(retained + slots);
        let (groups, credit) = owned_groups("one two", Some(&root)).unwrap();
        assert_eq!(&*groups, &["one".to_owned(), "two".to_owned()]);
        assert_eq!(root.used(), retained);
        assert_eq!(root.peak(), retained + slots);
        drop(groups);
        drop(credit);
        assert_eq!(root.used(), 0);
        let tight = rrrah_core::MemoryBudget::new(retained + slots - 1);
        assert!(matches!(
            owned_groups("one two", Some(&tight)),
            Err(ObjDecodeError::Source(DecodeError::Memory(_)))
        ));
        assert_eq!(tight.used(), 0);
        let bytes =
            b"mtllib one.mtl two.mtl\ng one two\nv 0 0 0\nv 1 0 0\nv 0 1 0\nf 1 2 3\nf 1 2 3\ng unused\n";
        let expected = crate::DecodedModel::Obj(Arc::new(decode(bytes).unwrap()).into()).capacity_bytes();
        let root = rrrah_core::MemoryBudget::new(expected + 1024);
        let (mesh, credit) = decode_bytes_budgeted(bytes, Some(&root), || Ok(())).unwrap();
        assert!(Arc::ptr_eq(&mesh.faces[0].groups, &mesh.faces[1].groups));
        assert_eq!(mesh.material_libraries, ["one.mtl", "two.mtl"]);
        assert_eq!(&*mesh.faces[0].groups, &["one".to_owned(), "two".to_owned()]);
        assert_eq!(root.used(), expected);
        drop(mesh);
        drop(credit);
        assert_eq!(root.used(), 0);
    }
    #[test]
    fn object_material_credit_transfers_once_and_cleans_up_unused_state() {
        let bytes =
            b"o object\nusemtl paint\nv 0 0 0\nv 1 0 0\nv 0 1 0\nf 1 2 3\nf 1 2 3\nusemtl off\no unused\n";
        let legacy = decode(bytes).unwrap();
        let expected = crate::DecodedModel::Obj(Arc::new(legacy).into()).capacity_bytes();
        let root = rrrah_core::MemoryBudget::new(expected + 64);
        let (mesh, credit) = decode_bytes_budgeted(bytes, Some(&root), || Ok(())).unwrap();
        assert_eq!(mesh.faces[0].object.as_ref(), "object");
        assert_eq!(mesh.faces[0].material.as_deref(), Some("paint"));
        assert!(Arc::ptr_eq(&mesh.faces[0].object, &mesh.faces[1].object));
        assert!(Arc::ptr_eq(
            mesh.faces[0].material.as_ref().unwrap(),
            mesh.faces[1].material.as_ref().unwrap()
        ));
        assert_eq!(root.used(), expected);
        assert_eq!(credit.as_ref().unwrap().bytes(), expected);
        drop(mesh);
        drop(credit);
        assert_eq!(root.used(), 0);
        let bytes = b"o abcdef\n";
        let base = std::mem::size_of::<ObjMesh>() as u64;
        let tight = rrrah_core::MemoryBudget::new(base + 6);
        assert!(matches!(
            decode_bytes_budgeted(bytes, Some(&tight), || Ok(())),
            Err(ObjDecodeError::Source(DecodeError::Memory(_)))
        ));
        assert_eq!(tight.used(), 0);
        let text = format!("o kept\nusemtl held\n{}", "\n".repeat(1024));
        let root = rrrah_core::MemoryBudget::new(base + 64);
        let mut calls = 0;
        let result = decode_bytes_budgeted(text.as_bytes(), Some(&root), || {
            calls += 1;
            if calls > 1 {
                assert_eq!(root.used(), base + 8);
                Err(ObjDecodeError::Source(DecodeError::Cancelled))
            } else {
                Ok(())
            }
        });
        assert!(matches!(
            result,
            Err(ObjDecodeError::Source(DecodeError::Cancelled))
        ));
        assert_eq!(root.used(), 0);
    }
    #[test]
    fn metadata_join_preserves_spacing_and_rejects_before_large_token_allocation() {
        let root = rrrah_core::MemoryBudget::new(7);
        let mut credit = Some(root.try_reserve(0).unwrap());
        let name = join_metadata_tokens("  one\t two  ".split_whitespace(), &mut credit, 7).unwrap();
        assert_eq!(name, "one two");
        assert_eq!(root.used(), name.capacity() as u64);
        drop(name);
        drop(credit);
        assert_eq!(root.used(), 0);
        let root = rrrah_core::MemoryBudget::new(6);
        let mut credit = Some(root.try_reserve(0).unwrap());
        assert!(matches!(
            join_metadata_tokens("one two".split_whitespace(), &mut credit, 7),
            Err(ObjDecodeError::Source(DecodeError::Memory(_)))
        ));
        assert_eq!(root.used(), 0);
        let input = format!("g {}\n", "x ".repeat(10000));
        let root = rrrah_core::MemoryBudget::new(std::mem::size_of::<ObjMesh>() as u64 + 1024);
        assert!(matches!(
            decode_bytes_budgeted(input.as_bytes(), Some(&root), || Ok(())),
            Err(ObjDecodeError::Source(DecodeError::Memory(_)))
        ));
        assert_eq!(root.used(), 0);
        assert!(decode(input.as_bytes()).is_ok());
    }
    #[test]
    fn continued_line_scratch_is_admitted_before_growth_and_released() {
        let root = rrrah_core::MemoryBudget::new(6);
        let mut credit = Some(root.try_reserve(0).unwrap());
        let mut text = String::new();
        append_scratch(&mut text, "v 0 ", &mut credit).unwrap();
        let capacity = text.capacity();
        assert_eq!(root.used(), capacity as u64);
        assert!(matches!(
            append_scratch(&mut text, "0 0", &mut credit),
            Err(ObjDecodeError::Source(DecodeError::Memory(_)))
        ));
        assert_eq!(text, "v 0 ");
        assert_eq!(text.capacity(), capacity);
        drop(text);
        drop(credit);
        assert_eq!(root.used(), 0);
        let bytes = b"v 0 \\\n0 0\nv 1 0 0\nv 0 1 0\nf 1 2 3\n";
        let root = rrrah_core::MemoryBudget::new(std::mem::size_of::<ObjMesh>() as u64 + 6);
        assert!(matches!(
            decode_bytes_budgeted(bytes, Some(&root), || Ok(())),
            Err(ObjDecodeError::Source(DecodeError::Memory(_)))
        ));
        assert_eq!(root.used(), 0);
        let root = rrrah_core::MemoryBudget::new(128 * 1024);
        let (mesh, credit) = decode_bytes_budgeted(bytes, Some(&root), || Ok(())).unwrap();
        assert_eq!(mesh.triangles.len(), 1);
        assert_eq!(root.used(), credit.as_ref().unwrap().bytes());
        drop(mesh);
        drop(credit);
        assert_eq!(root.used(), 0);
    }
    #[test]
    fn managed_obj_arrays_and_polygon_scratch_share_budget_and_owner_lifetime() {
        let root_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/models");
        for name in ["obj-triangle.obj", "obj-negative-quad.obj", "obj-concave-u.obj"] {
            let mut request = DecodeRequest::new(root_path.join(name));
            let expected = crate::decode_model(&request).unwrap();
            let output = expected.capacity_bytes();
            let source = std::fs::metadata(&request.path).unwrap().len();
            let budget = rrrah_core::MemoryBudget::new(source + output + 128 * 1024);
            request.memory_budget = Some(budget.clone());
            let actual = crate::decode_model(&request).unwrap();
            assert_eq!(actual.capacity_bytes(), output);
            assert_eq!(
                actual.triangles().collect::<Vec<_>>(),
                expected.triangles().collect::<Vec<_>>()
            );
            assert_eq!(actual.bounds(), expected.bounds());
            assert_eq!(budget.used(), output);
            let held = actual.clone();
            let (crate::DecodedModel::Obj(a), crate::DecodedModel::Obj(b)) = (&actual, &held) else {
                panic!()
            };
            assert!(a.is_managed() && a.ptr_eq(b));
            drop(actual);
            assert_eq!(budget.used(), output);
            drop(held);
            assert_eq!(budget.used(), 0);
            let tight = rrrah_core::MemoryBudget::new(source + std::mem::size_of::<ObjMesh>() as u64);
            request.memory_budget = Some(tight.clone());
            assert!(matches!(
                crate::decode_model(&request),
                Err(crate::ModelDecodeError::Obj(ObjDecodeError::Source(
                    DecodeError::Memory(_)
                )))
            ));
            assert_eq!(tight.used(), 0);
        }
    }
    #[test]
    fn managed_triangulation_bounds_scratch_and_cleans_up_failure_and_cancel() {
        let points = [[0., 0., 0.], [1., 0., 0.], [1., 1., 0.], [0., 1., 0.]];
        let bytes = 4
            * (std::mem::size_of::<[f64; 3]>()
                + std::mem::size_of::<[f64; 2]>()
                + std::mem::size_of::<usize>())
            + 2 * std::mem::size_of::<[usize; 3]>();
        let root = rrrah_core::MemoryBudget::new(bytes as u64);
        let (triangles, credit) =
            triangulate_budgeted(&points, &mut 1000, &mut || Ok(()), Some(&root)).unwrap();
        assert_eq!(triangles.len(), 2);
        assert_eq!(root.used(), bytes as u64);
        drop(triangles);
        drop(credit);
        assert_eq!(root.used(), 0);
        let tight = rrrah_core::MemoryBudget::new(bytes as u64 - 1);
        assert!(matches!(
            triangulate_budgeted(&points, &mut 1000, &mut || Ok(()), Some(&tight)),
            Err(ObjDecodeError::Source(DecodeError::Memory(_)))
        ));
        assert_eq!(tight.used(), 0);
        let cancelled = triangulate_budgeted(
            &points,
            &mut 1000,
            &mut || {
                assert!(root.used() > 0);
                Err(ObjDecodeError::Source(DecodeError::Cancelled))
            },
            Some(&root),
        );
        assert!(matches!(
            cancelled,
            Err(ObjDecodeError::Source(DecodeError::Cancelled))
        ));
        assert_eq!(root.used(), 0);
    }
    const CONCAVE: &[u8] =
        b"# CC0 L shape, area 3\nv 0 0 0\nv 2 0 0\nv 2 1 0\nv 1 1 0\nv 1 2 0\nv 0 2 0\nf 1 2 3 4 5 6\n";
    fn decode(bytes: &[u8]) -> Result<ObjMesh> {
        decode_bytes(bytes, || Ok(()))
    }
    fn area(mesh: &ObjMesh) -> f64 {
        mesh.triangles
            .iter()
            .map(|triangle| {
                let [a, b, c] = triangle
                    .corners
                    .map(|corner| mesh.vertex(mesh.corners[corner as usize].position));
                cross(
                    [a[0] as f64, a[1] as f64],
                    [b[0] as f64, b[1] as f64],
                    [c[0] as f64, c[1] as f64],
                ) * 0.5
            })
            .sum()
    }
    #[test]
    fn concave_polygon_retains_area_winding_and_cutout() {
        let mesh = decode(CONCAVE).unwrap();
        assert_eq!(mesh.triangles.len(), 4);
        assert_eq!(area(&mesh), 3.);
        assert_eq!(mesh.bounds, Some([[0., 0., 0.], [2., 2., 0.]]));
        for triangle in &mesh.triangles {
            let [a, b, c] = triangle
                .corners
                .map(|corner| mesh.vertex(mesh.corners[corner as usize].position));
            let centroid = [(a[0] + b[0] + c[0]) / 3., (a[1] + b[1] + c[1]) / 3.];
            assert!(centroid[0] <= 1. || centroid[1] <= 1.);
        }
        let reverse = std::str::from_utf8(CONCAVE)
            .unwrap()
            .replace("f 1 2 3 4 5 6", "f 6 5 4 3 2 1");
        assert_eq!(area(&decode(reverse.as_bytes()).unwrap()), -3.);
    }
    #[test]
    fn negative_indices_uv_normals_weights_and_state_are_retained() {
        let mesh=decode(b"mtllib authored.mtl\no thing\ng one two\ns 7\nusemtl blue\nv 4 0 0 2\nv 0 2 0\nv 0 0 0\nvt 0.25\nvt 0.5 0.75\nvt 1 1 1\nvn 0 0 2\nf -3/-3/-1 -2/-2/-1 -1/-1/-1\nv 99 99 99\ns off\nusemtl off\nf 1 2 3\n").unwrap();
        assert_eq!(mesh.vertex(0), [4., 0., 0.]);
        assert_eq!(mesh.positions[0], [4., 0., 0., 2.]);
        assert_eq!(mesh.texcoords[0], [0.25, 0., 0.]);
        assert_eq!(mesh.normals[0], [0., 0., 2.]);
        assert_eq!(
            mesh.corners[0],
            ObjCorner {
                position: 0,
                texcoord: Some(0),
                normal: Some(0)
            }
        );
        assert_eq!(&*mesh.faces[0].object, "thing");
        assert_eq!(&*mesh.faces[0].groups, ["one", "two"]);
        assert_eq!(mesh.faces[0].material.as_deref(), Some("blue"));
        assert_eq!(mesh.faces[0].smoothing_group, Some(7));
        assert!(mesh.faces[1].material.is_none());
        assert!(mesh.faces[1].smoothing_group.is_none());
        assert_eq!(mesh.bounds, Some([[0., 0., 0.], [4., 2., 0.]]));
        assert_eq!(mesh.material_libraries, ["authored.mtl"]);
    }
    #[test]
    fn malformed_geometry_and_unsupported_elements_fail() {
        for text in [
            "v NaN 0 0",
            "v 0 0",
            "v 0 0 0 1 1",
            "csh touch /tmp/should-never-run",
            "curv 0 1 1 2 3",
            "v 0 0 0\nf 0 1 1",
            "v 0 0 0\nf -2 1 1",
            "v 0 0 0\nf 2 1 1",
            "v 0 0 0\nf 1/ 1 1",
            "v 0 0 0\nvn 0 0 1\nf 1//1 1 1",
            "v 0 0 0\nf 1 1",
            "v 0 0 0\\",
        ] {
            assert!(decode(text.as_bytes()).is_err(), "accepted {text}");
        }
        for face in [
            "v 0 0 0\nv 1 1 0\nv 1 0 0\nv 0 1 0\nf 1 2 3 4",
            "v 0 0 0\nv 1 0 0\nv 1 1 0.5\nv 0 1 0\nf 1 2 3 4",
            "v 0 0 0\nv 1 0 0\nv 0 1 0\nf 1 2 2 3",
        ] {
            assert!(decode(face.as_bytes()).is_err());
        }
    }
    #[test]
    fn line_continuations_collinear_edges_rotated_planes_and_limits() {
        let mesh=decode(b"\xef\xbb\xbfv 0 0 0\r\nv 1 0 0\r\nv 2 0 0\r\nv 2 1 0\r\nv 0 1 0\r\nf 1 2 \\\r\n3 4 5 # comment\r\n").unwrap();
        assert_eq!(area(&mesh), 2.);
        let rotated = std::str::from_utf8(CONCAVE)
            .unwrap()
            .lines()
            .map(|line| {
                if line.starts_with("v ") {
                    let values: Vec<_> = line.split_whitespace().collect();
                    format!("v 0 {} {}", values[1], values[2])
                } else {
                    line.to_string()
                }
            })
            .collect::<Vec<_>>()
            .join("\n");
        assert_eq!(decode(rotated.as_bytes()).unwrap().triangles.len(), 4);
        assert!(decode(&vec![b' '; MAX_LINE + 1]).is_err());
        assert!(matches!(
            decode_bytes(CONCAVE, || Err(DecodeError::Cancelled.into())),
            Err(ObjDecodeError::Source(DecodeError::Cancelled))
        ));
        let mut work = 0;
        assert!(
            triangulate(
                &[[0., 0., 0.], [1., 0., 0.], [1., 1., 0.], [0., 1., 0.]],
                &mut work,
                &mut || Ok(())
            )
            .is_err()
        );
    }
}

#[cfg(test)]
mod fixture_tests {
    use super::*;
    fn signed_area(triangles: &[[[f32; 3]; 3]]) -> [f64; 3] {
        let mut area = [0.; 3];
        for [a, b, c] in triangles {
            let u = std::array::from_fn::<_, 3, _>(|i| f64::from(b[i]) - f64::from(a[i]));
            let v = std::array::from_fn::<_, 3, _>(|i| f64::from(c[i]) - f64::from(a[i]));
            area[0] += (u[1] * v[2] - u[2] * v[1]) * 0.5;
            area[1] += (u[2] * v[0] - u[0] * v[2]) * 0.5;
            area[2] += (u[0] * v[1] - u[1] * v[0]) * 0.5;
        }
        area
    }
    fn contains(triangles: &[[[f32; 3]; 3]], axes: [usize; 2], p: [f64; 2]) -> bool {
        triangles.iter().any(|triangle| {
            let [a, b, c] = triangle.map(|v| [f64::from(v[axes[0]]), f64::from(v[axes[1]])]);
            let sides = [cross(a, b, p), cross(b, c, p), cross(c, a, p)];
            sides.iter().all(|v| *v >= -1e-10) || sides.iter().all(|v| *v <= 1e-10)
        })
    }
    #[test]
    fn common_model_matches_independent_geometry_oracles() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/models");
        let manifest = std::fs::read_to_string(root.join("obj-manifest.tsv")).unwrap();
        let mut checked = 0;
        for line in manifest.lines().filter(|line| !line.starts_with('#')) {
            let fields: Vec<_> = line.split('\t').collect();
            let bytes = std::fs::read(root.join(fields[1])).unwrap();
            assert_eq!(bytes.len() % 36, 0);
            let oracle: Vec<[[f32; 3]; 3]> = bytes
                .chunks_exact(36)
                .map(|triangle| {
                    std::array::from_fn(|i| {
                        std::array::from_fn(|j| {
                            f32::from_le_bytes(
                                triangle[(i * 3 + j) * 4..(i * 3 + j + 1) * 4].try_into().unwrap(),
                            )
                        })
                    })
                })
                .collect();
            let request = DecodeRequest::new(root.join(fields[0]));
            let model = crate::decode_model(&request).unwrap();
            let decoded: Vec<_> = model
                .triangles()
                .map(|t| t.map(|p| p.map(|v| v as f32)))
                .collect();
            let a = signed_area(&decoded);
            let b = signed_area(&oracle);
            for axis in 0..3 {
                assert!(
                    (a[axis] - b[axis]).abs() < 1e-8,
                    "{}: area {a:?} vs {b:?}",
                    fields[0]
                );
            }
            let bounds = model.bounds().unwrap();
            let mut oracle_bounds = [[f32::INFINITY; 3], [f32::NEG_INFINITY; 3]];
            for vertex in oracle.iter().flatten() {
                for axis in 0..3 {
                    oracle_bounds[0][axis] = oracle_bounds[0][axis].min(vertex[axis]);
                    oracle_bounds[1][axis] = oracle_bounds[1][axis].max(vertex[axis]);
                }
            }
            assert_eq!(bounds, oracle_bounds.map(|b| b.map(f64::from)), "{}", fields[0]);
            let drop_axis = (0..3).max_by(|i, j| a[*i].abs().total_cmp(&a[*j].abs())).unwrap();
            let axes: Vec<_> = (0..3).filter(|i| *i != drop_axis).collect();
            let axes = [axes[0], axes[1]];
            for x in 0..37 {
                for y in 0..41 {
                    let p = [
                        bounds[0][axes[0]]
                            + (x as f64 + 0.314159) / 37.
                                * f64::from(bounds[1][axes[0]] - bounds[0][axes[0]]),
                        bounds[0][axes[1]]
                            + (y as f64 + 0.271828) / 41.
                                * f64::from(bounds[1][axes[1]] - bounds[0][axes[1]]),
                    ];
                    assert_eq!(
                        contains(&decoded, axes, p),
                        contains(&oracle, axes, p),
                        "{}: coverage {p:?}",
                        fields[0]
                    );
                }
            }
            checked += 1;
        }
        assert_eq!(checked, 7);
    }
    #[test]
    fn cancelled_obj_never_opens_source() {
        use std::sync::{Arc, atomic::AtomicU64};
        let mut request = DecodeRequest::new("/nonexistent/rrrah-cancelled-model.OBJ");
        request.cancellation = Some(crate::GenerationToken::new(Arc::new(AtomicU64::new(2)), 1));
        assert!(matches!(
            decode_obj(&request),
            Err(ObjDecodeError::Source(DecodeError::Cancelled))
        ));
        assert!(crate::is_supported_model_path(&request.path));
    }
}
