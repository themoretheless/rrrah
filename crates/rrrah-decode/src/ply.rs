//! Typed PLY 1.0 storage and bounded polygon-mesh extraction.
use crate::{DecodeError, DecodeRequest, bounded_io::read_bounded};
use thiserror::Error;
const BUDGET: usize = 512 * 1024 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlyType {
    I8,
    U8,
    I16,
    U16,
    I32,
    U32,
    F32,
    F64,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlyEncoding {
    Ascii,
    LittleEndian,
    BigEndian,
}
#[derive(Debug, Clone)]
pub enum PlyScalars {
    I8(Vec<i8>),
    U8(Vec<u8>),
    I16(Vec<i16>),
    U16(Vec<u16>),
    I32(Vec<i32>),
    U32(Vec<u32>),
    F32(Vec<f32>),
    F64(Vec<f64>),
}
#[derive(Debug, Clone)]
pub enum PlyValues {
    Scalar(PlyScalars),
    List {
        count_type: PlyType,
        offsets: Vec<u32>,
        values: PlyScalars,
    },
}
#[derive(Debug, Clone)]
pub struct PlyProperty {
    pub name: String,
    pub values: PlyValues,
}
#[derive(Debug, Clone)]
pub struct PlyElement {
    pub name: String,
    pub count: usize,
    pub properties: Vec<PlyProperty>,
}
#[derive(Debug, Clone)]
pub struct PlyMesh {
    pub encoding: PlyEncoding,
    pub comments: Vec<String>,
    pub object_info: Vec<String>,
    pub elements: Vec<PlyElement>,
    pub triangles: Vec<[u32; 3]>,
    pub bounds: Option<[[f64; 3]; 2]>,
    pub(crate) vertex_element: usize,
    pub(crate) position_properties: [usize; 3],
}
impl PlyMesh {
    pub(crate) fn vertex(&self, index: u32) -> [f64; 3] {
        self.position_properties.map(
            |p| match &self.elements[self.vertex_element].properties[p].values {
                PlyValues::Scalar(values) => values.get(index as usize).unwrap(),
                PlyValues::List { .. } => unreachable!("validated scalar coordinates"),
            },
        )
    }
}
#[derive(Debug, Error)]
pub enum PlyDecodeError {
    #[error(transparent)]
    Source(#[from] DecodeError),
    #[error("invalid PLY: {0}")]
    Invalid(&'static str),
    #[error("unsupported PLY: {0}")]
    Unsupported(&'static str),
}
type Result<T> = std::result::Result<T, PlyDecodeError>;
#[derive(Clone, Copy)]
enum Number {
    I8(i8),
    U8(u8),
    I16(i16),
    U16(u16),
    I32(i32),
    U32(u32),
    F32(f32),
    F64(f64),
}

macro_rules! scalar_methods {
    ($($kind:ident : $ty:ty),*) => {
        impl PlyScalars {
            fn new(kind: PlyType) -> Self { match kind { $(PlyType::$kind => Self::$kind(Vec::new()),)* } }
            pub fn kind(&self) -> PlyType { match self { $(Self::$kind(_) => PlyType::$kind,)* } }
            pub fn len(&self) -> usize { match self { $(Self::$kind(v) => v.len(),)* } }
            pub fn is_empty(&self) -> bool { self.len()==0 }
            pub(crate) fn integer_index(&self,index:usize) -> Result<usize> { match self { $(Self::$kind(v) => v.get(index).copied().map(Number::$kind).ok_or(PlyDecodeError::Invalid("list index outside storage"))?.index(),)* } }
            pub fn get(&self,index:usize) -> Option<f64> { match self { $(Self::$kind(v) => v.get(index).map(|v| f64::from(*v)),)* } }
            fn push(&mut self,value:Number,budget:&mut GeometryBudget) -> Result<()> { match (self,value) { $((Self::$kind(v),Number::$kind(n)) => push(v,n,budget),)* _ => unreachable!("matching source type"), } }
        }
    }
}
scalar_methods!(I8:i8,U8:u8,I16:i16,U16:u16,I32:i32,U32:u32,F32:f32,F64:f64);
struct GeometryBudget {
    remaining: usize,
    reservation: Option<rrrah_core::Reservation>,
}
impl GeometryBudget {
    fn new(budget: Option<&rrrah_core::MemoryBudget>) -> Result<Self> {
        let reservation = budget
            .map(|budget| budget.try_reserve(std::mem::size_of::<PlyMesh>() as u64))
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
            .ok_or(PlyDecodeError::Invalid("storage budget exceeded"))?;
        if let Some(reservation) = &mut self.reservation {
            let total = reservation
                .bytes()
                .checked_add(bytes as u64)
                .ok_or(PlyDecodeError::Invalid("storage overflow"))?;
            reservation.ensure_bytes(total).map_err(DecodeError::from)?;
        }
        self.remaining = remaining;
        Ok(())
    }
}
fn map_polygon_error(error: crate::ObjDecodeError) -> PlyDecodeError {
    match error {
        crate::ObjDecodeError::Source(source) => PlyDecodeError::Source(source),
        crate::ObjDecodeError::Invalid(reason) => PlyDecodeError::Invalid(reason),
        crate::ObjDecodeError::Unsupported(reason) => PlyDecodeError::Unsupported(reason),
    }
}
pub(crate) fn decode_ply_for_model(request: &DecodeRequest) -> Result<crate::model::ModelBuffer<PlyMesh>> {
    request.check_cancelled()?;
    if request.image_index != 0 {
        return Err(PlyDecodeError::Invalid("PLY has no selectable image"));
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
fn reserve_storage<T>(values: &mut Vec<T>, extra: usize, budget: &mut GeometryBudget) -> Result<()> {
    let size = std::mem::size_of::<T>();
    let bytes = extra
        .checked_mul(size)
        .ok_or(PlyDecodeError::Invalid("storage overflow"))?;
    budget.charge(bytes)?;
    let old_capacity = values.capacity();
    values
        .try_reserve_exact(extra)
        .map_err(|_| PlyDecodeError::Invalid("allocation failed"))?;
    let actual = (values.capacity() - old_capacity) * size;
    if actual > bytes {
        budget.charge(actual - bytes)?;
    }
    Ok(())
}
fn header_push<T>(values: &mut Vec<T>, value: T, budget: &mut GeometryBudget) -> Result<()> {
    if values.len() == values.capacity() {
        reserve_storage(values, values.capacity().max(4), budget)?;
    }
    values.push(value);
    Ok(())
}
fn owned_header_string(value: &str, budget: &mut GeometryBudget) -> Result<String> {
    budget.charge(value.len())?;
    let mut text = String::new();
    text.try_reserve_exact(value.len())
        .map_err(|_| PlyDecodeError::Invalid("header string allocation failed"))?;
    if text.capacity() > value.len() {
        budget.charge(text.capacity() - value.len())?;
    }
    text.push_str(value);
    Ok(text)
}
fn push<T>(values: &mut Vec<T>, value: T, budget: &mut GeometryBudget) -> Result<()> {
    if values.len() == values.capacity() {
        let size = std::mem::size_of::<T>();
        let extra = values
            .len()
            .max(32)
            .min((budget.remaining / size).min(u32::MAX as usize - values.len()));
        if extra == 0 {
            return Err(PlyDecodeError::Invalid("storage budget exceeded"));
        }
        reserve_storage(values, extra, budget)?;
    }
    values.push(value);
    Ok(())
}
impl PlyType {
    fn parse(value: &str) -> Result<Self> {
        Ok(match value {
            "char" | "int8" => Self::I8,
            "uchar" | "uint8" => Self::U8,
            "short" | "int16" => Self::I16,
            "ushort" | "uint16" => Self::U16,
            "int" | "int32" => Self::I32,
            "uint" | "uint32" => Self::U32,
            "float" | "float32" => Self::F32,
            "double" | "float64" => Self::F64,
            _ => return Err(PlyDecodeError::Unsupported("scalar type")),
        })
    }
    fn is_integer(self) -> bool {
        !matches!(self, Self::F32 | Self::F64)
    }
    fn size(self) -> usize {
        match self {
            Self::I8 | Self::U8 => 1,
            Self::I16 | Self::U16 => 2,
            Self::I32 | Self::U32 | Self::F32 => 4,
            Self::F64 => 8,
        }
    }
}
impl Number {
    fn index(self) -> Result<usize> {
        let v = match self {
            Self::I8(v) => i64::from(v),
            Self::U8(v) => i64::from(v),
            Self::I16(v) => i64::from(v),
            Self::U16(v) => i64::from(v),
            Self::I32(v) => i64::from(v),
            Self::U32(v) => i64::from(v),
            _ => return Err(PlyDecodeError::Invalid("floating list count")),
        };
        usize::try_from(v).map_err(|_| PlyDecodeError::Invalid("negative count or index"))
    }
}
enum Reader<'a> {
    Ascii(std::str::SplitAsciiWhitespace<'a>),
    Binary {
        bytes: &'a [u8],
        position: usize,
        big: bool,
    },
}
impl Reader<'_> {
    fn number(&mut self, kind: PlyType) -> Result<Number> {
        match self {
            Self::Ascii(tokens) => {
                let token = tokens
                    .next()
                    .filter(|v| v.len() <= 128)
                    .ok_or(PlyDecodeError::Invalid("missing or oversized scalar"))?;
                macro_rules! parse {
                    ($ty:ty,$variant:ident) => {
                        Number::$variant(
                            token
                                .parse::<$ty>()
                                .map_err(|_| PlyDecodeError::Invalid("scalar outside declared type"))?,
                        )
                    };
                }
                Ok(match kind {
                    PlyType::I8 => parse!(i8, I8),
                    PlyType::U8 => parse!(u8, U8),
                    PlyType::I16 => parse!(i16, I16),
                    PlyType::U16 => parse!(u16, U16),
                    PlyType::I32 => parse!(i32, I32),
                    PlyType::U32 => parse!(u32, U32),
                    PlyType::F32 => parse!(f32, F32),
                    PlyType::F64 => parse!(f64, F64),
                })
            }
            Self::Binary { bytes, position, big } => {
                let end = position
                    .checked_add(kind.size())
                    .ok_or(PlyDecodeError::Invalid("range overflow"))?;
                let raw = bytes
                    .get(*position..end)
                    .ok_or(PlyDecodeError::Invalid("truncated scalar"))?;
                *position = end;
                macro_rules! value {
                    ($ty:ty,$variant:ident) => {
                        Number::$variant(if *big {
                            <$ty>::from_be_bytes(raw.try_into().unwrap())
                        } else {
                            <$ty>::from_le_bytes(raw.try_into().unwrap())
                        })
                    };
                }
                Ok(match kind {
                    PlyType::I8 => Number::I8(i8::from_ne_bytes([raw[0]])),
                    PlyType::U8 => Number::U8(raw[0]),
                    PlyType::I16 => value!(i16, I16),
                    PlyType::U16 => value!(u16, U16),
                    PlyType::I32 => value!(i32, I32),
                    PlyType::U32 => value!(u32, U32),
                    PlyType::F32 => value!(f32, F32),
                    PlyType::F64 => value!(f64, F64),
                })
            }
        }
    }
    fn finished(&mut self) -> bool {
        match self {
            Self::Ascii(v) => v.next().is_none(),
            Self::Binary { bytes, position, .. } => *position == bytes.len(),
        }
    }
}
fn line<'a>(bytes: &'a [u8], position: &mut usize, ending: &[u8]) -> Result<&'a str> {
    let length = bytes
        .get(*position..)
        .and_then(|v| {
            v[..v.len().min((256 * 1024_usize).saturating_sub(*position))]
                .windows(ending.len())
                .position(|v| v == ending)
        })
        .ok_or(PlyDecodeError::Invalid("unterminated header"))?;
    if *position + length + ending.len() > 256 * 1024 {
        return Err(PlyDecodeError::Invalid("header limit exceeded"));
    }
    let raw = &bytes[*position..*position + length];
    *position += length + ending.len();
    std::str::from_utf8(raw.strip_suffix(b"\r").unwrap_or(raw))
        .map_err(|_| PlyDecodeError::Invalid("invalid header text"))
}
pub fn decode_ply(request: &DecodeRequest) -> Result<PlyMesh> {
    request.check_cancelled()?;
    if request.image_index != 0 {
        return Err(PlyDecodeError::Invalid("PLY has no selectable image"));
    }
    let bytes = read_bounded(request)?;
    decode_bytes_budgeted(&bytes, request.memory_budget.as_ref(), || {
        request.check_cancelled().map_err(Into::into)
    })
    .map(|(mesh, _)| mesh)
}
#[cfg(test)]
fn decode_bytes(bytes: &[u8], cancel: impl FnMut() -> Result<()>) -> Result<PlyMesh> {
    decode_bytes_budgeted(bytes, None, cancel).map(|(mesh, _)| mesh)
}
fn decode_bytes_budgeted(
    bytes: &[u8],
    managed: Option<&rrrah_core::MemoryBudget>,
    mut cancel: impl FnMut() -> Result<()>,
) -> Result<(PlyMesh, Option<rrrah_core::Reservation>)> {
    let ending: &[u8] = if bytes.starts_with(b"ply\r\n") {
        b"\r\n"
    } else if bytes.starts_with(b"ply\n") {
        b"\n"
    } else if bytes.starts_with(b"ply\r") {
        b"\r"
    } else {
        return Err(PlyDecodeError::Invalid("missing magic"));
    };
    let mut position = 0;
    if line(bytes, &mut position, ending)? != "ply" {
        return Err(PlyDecodeError::Invalid("missing magic"));
    }
    let mut budget = GeometryBudget::new(managed)?;
    let mut mesh = PlyMesh {
        encoding: PlyEncoding::Ascii,
        comments: Vec::new(),
        object_info: Vec::new(),
        elements: Vec::new(),
        triangles: Vec::new(),
        bounds: None,
        vertex_element: 0,
        position_properties: [0; 3],
    };
    let mut has_format = false;
    let mut property_count = 0;
    loop {
        cancel()?;
        let line = line(bytes, &mut position, ending)?.trim();
        let mut tokens = line.split_ascii_whitespace();
        match tokens.next() {
            Some("format") => {
                if has_format || !mesh.elements.is_empty() {
                    return Err(PlyDecodeError::Invalid("duplicate or misplaced format"));
                }
                mesh.encoding = match tokens.next() {
                    Some("ascii") => PlyEncoding::Ascii,
                    Some("binary_little_endian") => PlyEncoding::LittleEndian,
                    Some("binary_big_endian") => PlyEncoding::BigEndian,
                    _ => return Err(PlyDecodeError::Unsupported("encoding")),
                };
                if tokens.next() != Some("1.0") || tokens.next().is_some() {
                    return Err(PlyDecodeError::Unsupported("format version"));
                }
                has_format = true;
            }
            Some("comment") => {
                let text = owned_header_string(line.strip_prefix("comment").unwrap().trim(), &mut budget)?;
                header_push(&mut mesh.comments, text, &mut budget)?;
            }
            Some("obj_info") => {
                let text = owned_header_string(line.strip_prefix("obj_info").unwrap().trim(), &mut budget)?;
                header_push(&mut mesh.object_info, text, &mut budget)?;
            }
            Some("element") => {
                if !has_format || mesh.elements.len() == 128 {
                    return Err(PlyDecodeError::Invalid("element limit or missing format"));
                }
                let name = tokens
                    .next()
                    .ok_or(PlyDecodeError::Invalid("missing element name"))?;
                let count = tokens
                    .next()
                    .and_then(|v| v.parse::<u32>().ok())
                    .and_then(|v| usize::try_from(v).ok())
                    .ok_or(PlyDecodeError::Invalid("element count"))?;
                if tokens.next().is_some() || mesh.elements.iter().any(|v| v.name == name) {
                    return Err(PlyDecodeError::Invalid("duplicate or malformed element"));
                }
                let name = owned_header_string(name, &mut budget)?;
                header_push(
                    &mut mesh.elements,
                    PlyElement {
                        name,
                        count,
                        properties: Vec::new(),
                    },
                    &mut budget,
                )?;
            }
            Some("property") => {
                property_count += 1;
                if property_count > 4096 {
                    return Err(PlyDecodeError::Invalid("property limit"));
                }
                let element = mesh
                    .elements
                    .last_mut()
                    .ok_or(PlyDecodeError::Invalid("property before element"))?;
                let kind = tokens
                    .next()
                    .ok_or(PlyDecodeError::Invalid("missing property type"))?;
                let values = if kind == "list" {
                    let count = PlyType::parse(
                        tokens
                            .next()
                            .ok_or(PlyDecodeError::Invalid("missing count type"))?,
                    )?;
                    if !count.is_integer() {
                        return Err(PlyDecodeError::Invalid("non-integer count type"));
                    }
                    let value = PlyType::parse(
                        tokens
                            .next()
                            .ok_or(PlyDecodeError::Invalid("missing list type"))?,
                    )?;
                    let mut offsets = Vec::new();
                    reserve_storage(&mut offsets, 1, &mut budget)?;
                    offsets.push(0);
                    PlyValues::List {
                        count_type: count,
                        offsets,
                        values: PlyScalars::new(value),
                    }
                } else {
                    PlyValues::Scalar(PlyScalars::new(PlyType::parse(kind)?))
                };
                let name = tokens
                    .next()
                    .ok_or(PlyDecodeError::Invalid("missing property name"))?;
                if tokens.next().is_some() || element.properties.iter().any(|v| v.name == name) {
                    return Err(PlyDecodeError::Invalid("duplicate or malformed property"));
                }
                let name = owned_header_string(name, &mut budget)?;
                header_push(&mut element.properties, PlyProperty { name, values }, &mut budget)?;
            }
            Some("end_header") => {
                if tokens.next().is_some() || !has_format {
                    return Err(PlyDecodeError::Invalid("invalid end_header"));
                }
                break;
            }
            _ => return Err(PlyDecodeError::Unsupported("header statement")),
        }
    }
    mesh.vertex_element = mesh
        .elements
        .iter()
        .position(|v| v.name == "vertex")
        .ok_or(PlyDecodeError::Unsupported("missing vertex element"))?;
    let vertices = &mesh.elements[mesh.vertex_element];
    for (axis, name) in ["x", "y", "z"].iter().enumerate() {
        mesh.position_properties[axis] = vertices
            .properties
            .iter()
            .position(|v| v.name == *name)
            .filter(|p| matches!(vertices.properties[*p].values, PlyValues::Scalar(_)))
            .ok_or(PlyDecodeError::Unsupported("missing scalar coordinates"))?;
    }
    let vertex_count = vertices.count;
    let faces = mesh.elements.iter().position(|v| v.name == "face");
    if vertex_count > 0 && faces.is_none_or(|i| mesh.elements[i].count == 0) {
        return Err(PlyDecodeError::Unsupported("point cloud rendering"));
    }
    let mut reader = match mesh.encoding {
        PlyEncoding::Ascii => Reader::Ascii(
            std::str::from_utf8(&bytes[position..])
                .map_err(|_| PlyDecodeError::Invalid("invalid ASCII body"))?
                .split_ascii_whitespace(),
        ),
        encoding => Reader::Binary {
            bytes: &bytes[position..],
            position: 0,
            big: encoding == PlyEncoding::BigEndian,
        },
    };
    for element in &mut mesh.elements {
        if element.count > 0 && element.properties.is_empty() {
            return Err(PlyDecodeError::Invalid("nonempty element without properties"));
        }
        for row in 0..element.count {
            if row % 1024 == 0 {
                cancel()?;
            }
            for property in &mut element.properties {
                match &mut property.values {
                    PlyValues::Scalar(values) => {
                        let number = reader.number(values.kind())?;
                        values.push(number, &mut budget)?;
                    }
                    PlyValues::List {
                        count_type,
                        offsets,
                        values,
                    } => {
                        let count = reader.number(*count_type)?.index()?;
                        if count > 1024 * 1024 {
                            return Err(PlyDecodeError::Invalid("list length limit"));
                        }
                        for i in 0..count {
                            if i % 4096 == 0 {
                                cancel()?;
                            }
                            let number = reader.number(values.kind())?;
                            values.push(number, &mut budget)?;
                        }
                        push(
                            offsets,
                            u32::try_from(values.len())
                                .map_err(|_| PlyDecodeError::Invalid("list offset overflow"))?,
                            &mut budget,
                        )?;
                    }
                }
            }
        }
    }
    if !reader.finished() {
        return Err(PlyDecodeError::Invalid("trailing body data"));
    }
    for row in 0..vertex_count {
        if row % 1024 == 0 {
            cancel()?;
        }
        let row = u32::try_from(row).map_err(|_| PlyDecodeError::Invalid("vertex index overflow"))?;
        if !mesh.vertex(row).iter().all(|v| v.is_finite()) {
            return Err(PlyDecodeError::Invalid("non-finite position"));
        }
    }
    let faces = mesh.elements.iter().position(|v| v.name == "face");
    if vertex_count > 0 && faces.is_none_or(|i| mesh.elements[i].count == 0) {
        return Err(PlyDecodeError::Unsupported("point cloud rendering"));
    }
    if let Some(faces) = faces {
        let element = &mesh.elements[faces];
        let mut candidates = element
            .properties
            .iter()
            .filter(|v| v.name == "vertex_indices" || v.name == "vertex_index");
        let candidate = candidates.next();
        if candidate.is_none() || candidates.next().is_some() {
            return Err(PlyDecodeError::Invalid("missing or ambiguous face indices"));
        }
        let PlyValues::List { offsets, values, .. } = &candidate.unwrap().values else {
            return Err(PlyDecodeError::Invalid("face indices must be a list"));
        };
        if !values.kind().is_integer() {
            return Err(PlyDecodeError::Invalid("non-integer face indices"));
        }
        let mut work = 64 * 1024 * 1024;
        let mut index_storage = [0_u32; 1024];
        for row in 0..element.count {
            if row % 1024 == 0 {
                cancel()?;
            }
            let range = offsets[row] as usize..offsets[row + 1] as usize;
            if !(3..=1024).contains(&range.len()) {
                return Err(PlyDecodeError::Unsupported("face size"));
            }
            let indices = &mut index_storage[..range.len()];
            for (out, index) in indices.iter_mut().zip(range) {
                let value = values.integer_index(index)?;
                if value >= vertex_count {
                    return Err(PlyDecodeError::Invalid("face index outside vertex array"));
                }
                *out = u32::try_from(value).map_err(|_| PlyDecodeError::Invalid("face index overflow"))?;
            }
            if indices.len() == 3 {
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
                let mut points = crate::obj::triangulation_buffer(indices.len(), [0.0; 3], &mut point_credit)
                    .map_err(map_polygon_error)?;
                for (point, index) in points.iter_mut().zip(indices.iter()) {
                    *point = mesh.vertex(*index);
                }
                let (triangles, _scratch_credit) = crate::obj::triangulate_budgeted(
                    &points,
                    &mut work,
                    &mut || {
                        cancel().map_err(|error| match error {
                            PlyDecodeError::Source(source) => crate::ObjDecodeError::Source(source),
                            _ => crate::ObjDecodeError::Invalid("cancellation callback"),
                        })
                    },
                    managed,
                )
                .map_err(map_polygon_error)?;
                for triangle in triangles {
                    push(&mut mesh.triangles, triangle.map(|i| indices[i]), &mut budget)?;
                }
            }
            for index in indices.iter() {
                let vertex = mesh.vertex(*index);
                let bounds = mesh.bounds.get_or_insert([vertex, vertex]);
                for axis in 0..3 {
                    bounds[0][axis] = bounds[0][axis].min(vertex[axis]);
                    bounds[1][axis] = bounds[1][axis].max(vertex[axis]);
                }
            }
        }
    }
    cancel()?;
    Ok((mesh, budget.reservation))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn native_integer_indices_preserve_signed_and_unsigned_boundaries() {
        assert_eq!(
            PlyScalars::U32(vec![u32::MAX]).integer_index(0).unwrap(),
            usize::try_from(u32::MAX).unwrap()
        );
        assert_eq!(
            PlyScalars::I32(vec![i32::MAX]).integer_index(0).unwrap(),
            usize::try_from(i32::MAX).unwrap()
        );
        assert!(PlyScalars::I32(vec![i32::MIN]).integer_index(0).is_err());
        assert!(PlyScalars::I8(vec![i8::MIN]).integer_index(0).is_err());
        assert_eq!(PlyScalars::U8(vec![u8::MAX]).integer_index(0).unwrap(), 255);
        assert!(PlyScalars::F64(vec![1.0]).integer_index(0).is_err());
        assert!(PlyScalars::U32(vec![]).integer_index(0).is_err());
    }
    #[test]
    fn header_memory_rejection_precedes_body_validation_and_cleans_up() {
        let base = std::mem::size_of::<PlyMesh>() as u64;
        let text = format!(
            "ply\nformat ascii 1.0\ncomment {}\nend_header\ninvalid body",
            "x".repeat(4096)
        );
        let root = rrrah_core::MemoryBudget::new(base + 16);
        assert!(matches!(
            decode_bytes_budgeted(text.as_bytes(), Some(&root), || Ok(())),
            Err(PlyDecodeError::Source(DecodeError::Memory(_)))
        ));
        assert_eq!(root.used(), 0);
        // The old post-header admission instead reached missing-vertex validation.
        assert!(matches!(
            decode_bytes(text.as_bytes(), || Ok(())),
            Err(PlyDecodeError::Unsupported("missing vertex element"))
        ));
        let text = b"ply\nformat ascii 1.0\nelement vertex 0\nend_header\n";
        let root = rrrah_core::MemoryBudget::new(base + "vertex".len() as u64);
        assert!(matches!(
            decode_bytes_budgeted(text, Some(&root), || Ok(())),
            Err(PlyDecodeError::Source(DecodeError::Memory(_)))
        ));
        assert_eq!(root.used(), 0);
        let root = rrrah_core::MemoryBudget::new(base + 4096);
        let text = b"ply\nformat ascii 1.0\ncomment retained\nelement vertex 0\nproperty float x\nproperty float y\nproperty float z\nend_header\n";
        let result = decode_bytes_budgeted(text, Some(&root), || {
            if root.used() > base {
                Err(PlyDecodeError::Source(DecodeError::Cancelled))
            } else {
                Ok(())
            }
        });
        assert!(matches!(
            result,
            Err(PlyDecodeError::Source(DecodeError::Cancelled))
        ));
        assert_eq!(root.used(), 0);
    }
    #[test]
    fn managed_typed_columns_lists_and_polygons_retain_exact_credit() {
        let paths = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/models");
        for name in [
            "ply-32-ascii.ply",
            "ply-64-le.ply",
            "ply-offset64-be.ply",
            "ply-u64-ascii.ply",
        ] {
            let mut request = DecodeRequest::new(paths.join(name));
            let expected = crate::decode_model(&request).unwrap();
            let output = expected.capacity_bytes();
            let source = std::fs::metadata(&request.path).unwrap().len();
            let budget = rrrah_core::MemoryBudget::new(source + output + 128 * 1024);
            request.memory_budget = Some(budget.clone());
            let actual = crate::decode_model(&request).unwrap();
            assert_eq!(actual.capacity_bytes(), output);
            assert_eq!(actual.bounds(), expected.bounds());
            assert_eq!(
                actual.triangles().collect::<Vec<_>>(),
                expected.triangles().collect::<Vec<_>>()
            );
            assert_eq!(budget.used(), output);
            let held = actual.clone();
            let (crate::DecodedModel::Ply(a), crate::DecodedModel::Ply(b)) = (&actual, &held) else {
                panic!()
            };
            assert!(a.is_managed() && a.ptr_eq(b));
            let crate::DecodedModel::Ply(reference) = &expected else {
                panic!()
            };
            assert_eq!(a.comments, reference.comments);
            assert_eq!(a.object_info, reference.object_info);
            for (element, reference) in a.elements.iter().zip(&reference.elements) {
                assert_eq!(element.name, reference.name);
                assert_eq!(element.count, reference.count);
                for (property, reference) in element.properties.iter().zip(&reference.properties) {
                    assert_eq!(property.name, reference.name);
                    match (&property.values, &reference.values) {
                        (PlyValues::Scalar(a), PlyValues::Scalar(b)) => assert_eq!(raw(a), raw(b)),
                        (
                            PlyValues::List {
                                offsets: a,
                                values: av,
                                ..
                            },
                            PlyValues::List {
                                offsets: b,
                                values: bv,
                                ..
                            },
                        ) => {
                            assert_eq!(a, b);
                            assert_eq!(raw(av), raw(bv));
                        }
                        _ => panic!("property kind changed"),
                    }
                }
            }
            drop(actual);
            assert_eq!(budget.used(), output);
            drop(held);
            assert_eq!(budget.used(), 0);
            let tight = rrrah_core::MemoryBudget::new(source + std::mem::size_of::<PlyMesh>() as u64);
            request.memory_budget = Some(tight.clone());
            assert!(matches!(
                crate::decode_model(&request),
                Err(crate::ModelDecodeError::Ply(PlyDecodeError::Source(
                    DecodeError::Memory(_)
                )))
            ));
            assert_eq!(tight.used(), 0);
        }
    }
    fn raw(values: &PlyScalars) -> Vec<u8> {
        macro_rules! bytes {
            ($v:expr) => {
                $v.iter().flat_map(|v| v.to_le_bytes()).collect()
            };
        }
        match values {
            PlyScalars::I8(v) => v.iter().map(|v| *v as u8).collect(),
            PlyScalars::U8(v) => v.clone(),
            PlyScalars::I16(v) => bytes!(v),
            PlyScalars::U16(v) => bytes!(v),
            PlyScalars::I32(v) => bytes!(v),
            PlyScalars::U32(v) => bytes!(v),
            PlyScalars::F32(v) => bytes!(v),
            PlyScalars::F64(v) => bytes!(v),
        }
    }
    fn root() -> std::path::PathBuf {
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/models")
    }
    #[test]
    fn all_native_columns_and_encodings_match_independent_oracles() {
        let root = root();
        let manifest = std::fs::read_to_string(root.join("ply-manifest.tsv")).unwrap();
        let mut checked = 0;
        for line in manifest.lines().filter(|v| !v.starts_with('#')) {
            let fields: Vec<_> = line.split('\t').collect();
            let mesh = decode_ply(&DecodeRequest::new(root.join(fields[0]))).unwrap();
            let mut native = Vec::new();
            let mut schema = Vec::new();
            for element in &mesh.elements {
                for property in &element.properties {
                    let (values, count, offsets) = match &property.values {
                        PlyValues::Scalar(v) => (v, "-".to_owned(), 0),
                        PlyValues::List {
                            count_type,
                            offsets,
                            values,
                        } => {
                            native.extend(offsets.iter().flat_map(|v| v.to_le_bytes()));
                            (values, format!("{count_type:?}"), offsets.len())
                        }
                    };
                    let bytes = raw(values);
                    schema.push(format!(
                        "{}\t{}\t{:?}\t{}\t{}\t{}",
                        element.name,
                        property.name,
                        values.kind(),
                        count,
                        offsets,
                        bytes.len()
                    ));
                    native.extend(bytes);
                }
            }
            assert_eq!(
                native,
                std::fs::read(root.join(format!("{}.native.bin", fields[0]))).unwrap(),
                "{}",
                fields[0]
            );
            assert_eq!(
                schema.join("\n") + "\n",
                std::fs::read_to_string(root.join(format!("{}.schema.tsv", fields[0]))).unwrap(),
                "{}",
                fields[0]
            );
            assert_eq!(mesh.comments, ["CC0 authored geometry"]);
            assert_eq!(mesh.object_info, ["native type oracle"]);
            assert_eq!(mesh.triangles.len(), fields[1].parse::<usize>().unwrap());
            let source = crate::decode_model(&DecodeRequest::new(root.join(fields[0]))).unwrap();
            assert_eq!(source.format_name(), "PLY");
            if !fields[0].contains("u64") {
                let actual: Vec<u8> = source
                    .triangles()
                    .flatten()
                    .flatten()
                    .flat_map(|v| v.to_le_bytes())
                    .collect();
                assert_eq!(
                    actual,
                    std::fs::read(root.join(fields[2])).unwrap(),
                    "{}",
                    fields[0]
                );
            }
            if fields[0].contains("offset64") {
                let triangles: Vec<_> = source.triangles().collect();
                assert_eq!(triangles[0][1][0] - triangles[0][0][0], 2.);
                assert_eq!(source.bounds().unwrap()[0][0], 1e12 - 1.);
            }
            checked += 1;
        }
        assert_eq!(checked, 12);
    }
    #[test]
    fn truncated_binary_headers_payload_and_invalid_schema_reject() {
        for name in ["ply-32-le.ply", "ply-64-be.ply"] {
            let bytes = std::fs::read(root().join(name)).unwrap();
            for end in 0..bytes.len() {
                assert!(
                    decode_bytes(&bytes[..end], || Ok(())).is_err(),
                    "{name}: accepted prefix {end}"
                );
            }
            let mut trailing = bytes;
            trailing.push(0);
            assert!(decode_bytes(&trailing, || Ok(())).is_err());
        }
        let mut bytes = std::fs::read(root().join("ply-32-le.ply")).unwrap();
        let header_end = bytes.windows(11).position(|v| v == b"end_header\n").unwrap() + 11;
        // A first payload byte equal to LF must survive CR-only headers.
        bytes[header_end] = 10;
        let expected = decode_bytes(&bytes, || Ok(())).unwrap();
        for ending in [b"\r".as_slice(), b"\r\n".as_slice()] {
            let mut converted = Vec::new();
            for line in bytes[..header_end].split_inclusive(|v| *v == b'\n') {
                converted.extend_from_slice(&line[..line.len() - 1]);
                converted.extend_from_slice(ending);
            }
            converted.extend_from_slice(&bytes[header_end..]);
            let actual = decode_bytes(&converted, || Ok(())).unwrap();
            assert_eq!(actual.triangles, expected.triangles);
            assert_eq!(actual.bounds, expected.bounds);
            let PlyValues::Scalar(red) = &actual.elements[0].properties[0].values else {
                panic!("scalar red");
            };
            assert_eq!(red.get(0), Some(10.));
        }
        let source = std::fs::read_to_string(root().join("ply-32-ascii.ply")).unwrap();
        for invalid in [
            source.replace("format ascii 1.0", "format ascii 2.0"),
            source.replace("property float x", "property float z"),
            source.replace(
                "property list uchar int vertex_indices",
                "property list float int vertex_indices",
            ),
            source.replace("property float x", "property list uchar float x"),
            source.replace("element vertex 3", "element vertex 4294967296"),
        ] {
            assert!(decode_bytes(invalid.as_bytes(), || Ok(())).is_err());
        }
        assert!(
            decode_bytes(
                b"ply\nformat ascii 1.0\nproperty float x\nend_header\n",
                || Ok(())
            )
            .is_err()
        );
    }
    #[test]
    fn face_before_vertex_negative_indices_nonfinite_positions_and_point_cloud_contract() {
        let header = "ply\nformat ascii 1.0\nelement face 1\nproperty list uchar int vertex_index\nelement vertex 3\nproperty double x\nproperty double y\nproperty double z\nend_header\n";
        let body = "3 0 1 2\n0 0 0\n1 0 0\n0 1 0\n";
        let mesh = decode_bytes((header.to_owned() + body).as_bytes(), || Ok(())).unwrap();
        assert_eq!(mesh.triangles, [[0, 1, 2]]);
        for invalid in [
            body.replace("3 0 1 2", "3 -1 1 2"),
            body.replace("3 0 1 2", "3 0 1 3"),
            body.replace("0 0 0", "NaN 0 0"),
            body.replace("3 0 1 2", "-1"),
        ] {
            assert!(decode_bytes((header.to_owned() + &invalid).as_bytes(), || Ok(())).is_err());
        }
        let cloud = "ply\nformat ascii 1.0\nelement vertex 1\nproperty float x\nproperty float y\nproperty float z\nend_header\n0 0 0\n";
        assert!(matches!(
            decode_bytes(cloud.as_bytes(), || Ok(())),
            Err(PlyDecodeError::Unsupported("point cloud rendering"))
        ));
    }
    #[test]
    fn cancellation_before_io_and_during_polygon_tessellation() {
        use std::sync::{Arc, atomic::AtomicU64};
        let mut request = DecodeRequest::new("/nonexistent/cancelled.PLY");
        request.cancellation = Some(crate::GenerationToken::new(Arc::new(AtomicU64::new(2)), 1));
        assert!(matches!(
            decode_ply(&request),
            Err(PlyDecodeError::Source(DecodeError::Cancelled))
        ));
        let bytes = std::fs::read(root().join("ply-u64-be.ply")).unwrap();
        let header_end = bytes.windows(11).position(|v| v == b"end_header\n").unwrap() + 11;
        let header_lines = bytes[..header_end].iter().filter(|v| **v == b'\n').count();
        let mut calls = 0;
        assert!(matches!(
            decode_bytes(&bytes, || {
                calls += 1;
                if calls > header_lines + 11 {
                    Err(DecodeError::Cancelled.into())
                } else {
                    Ok(())
                }
            }),
            Err(PlyDecodeError::Source(DecodeError::Cancelled))
        ));
        assert!(crate::is_supported_model_path(&request.path));
    }
}
