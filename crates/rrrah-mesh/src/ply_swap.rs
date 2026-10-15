//! Typed PLY streaming swap with budgeted native restoration.
use crate::{PlyEncoding, PlyMesh, PlyScalars, PlyType, PlyValues};
use std::io::{self, Read, Write};
fn uint(w: &mut impl Write, n: usize) -> io::Result<()> {
    w.write_all(
        &u64::try_from(n)
            .map_err(|_| io::Error::other("PLY count overflow"))?
            .to_le_bytes(),
    )
}
fn text(w: &mut impl Write, s: &str) -> io::Result<()> {
    uint(w, s.len())?;
    w.write_all(s.as_bytes())
}
fn kind(kind: PlyType) -> u8 {
    match kind {
        PlyType::I8 => 0,
        PlyType::U8 => 1,
        PlyType::I16 => 2,
        PlyType::U16 => 3,
        PlyType::I32 => 4,
        PlyType::U32 => 5,
        PlyType::F32 => 6,
        PlyType::F64 => 7,
    }
}
fn width(kind: PlyType) -> u64 {
    match kind {
        PlyType::I8 | PlyType::U8 => 1,
        PlyType::I16 | PlyType::U16 => 2,
        PlyType::I32 | PlyType::U32 | PlyType::F32 => 4,
        PlyType::F64 => 8,
    }
}
fn scalars(w: &mut impl Write, values: &PlyScalars) -> io::Result<()> {
    w.write_all(&[kind(values.kind())])?;
    uint(w, values.len())?;
    macro_rules! write_values { ($($variant:ident),*) => { match values { $(PlyScalars::$variant(values)=>crate::off_swap::scalar_blocks(w,values.iter().map(|value|value.to_le_bytes())).map_err(io::Error::other),)* } } }
    write_values!(I8, U8, I16, U16, I32, U32, F32, F64)
}
/// Writes native typed columns and list offsets without numeric conversion or
/// a second geometry buffer. Source encoding is retained as metadata.
pub fn write_ply_swap_payload(mesh: &PlyMesh, w: &mut impl Write) -> io::Result<()> {
    w.write_all(b"RRPLY001")?;
    w.write_all(&[match mesh.encoding {
        PlyEncoding::Ascii => 0,
        PlyEncoding::LittleEndian => 1,
        PlyEncoding::BigEndian => 2,
    }])?;
    uint(w, mesh.vertex_element)?;
    for position in mesh.position_properties {
        uint(w, position)?;
    }
    for strings in [&mesh.comments, &mesh.object_info] {
        uint(w, strings.len())?;
        for string in strings {
            text(w, string)?;
        }
    }
    uint(w, mesh.elements.len())?;
    for element in &mesh.elements {
        text(w, &element.name)?;
        uint(w, element.count)?;
        uint(w, element.properties.len())?;
        for property in &element.properties {
            text(w, &property.name)?;
            match &property.values {
                PlyValues::Scalar(values) => {
                    w.write_all(&[0])?;
                    scalars(w, values)?;
                }
                PlyValues::List {
                    count_type,
                    offsets,
                    values,
                } => {
                    w.write_all(&[1, kind(*count_type)])?;
                    uint(w, offsets.len())?;
                    crate::off_swap::scalar_blocks(w, offsets.iter().map(|offset| offset.to_le_bytes()))
                        .map_err(io::Error::other)?;
                    scalars(w, values)?;
                }
            }
        }
    }
    uint(w, mesh.triangles.len())?;
    crate::off_swap::scalar_blocks(
        w,
        mesh.triangles.iter().flatten().map(|index| index.to_le_bytes()),
    )
    .map_err(io::Error::other)
}
/// Checked framing size; column values and offsets are not scanned.
pub fn ply_swap_payload_len(mesh: &PlyMesh) -> io::Result<u64> {
    fn add(total: &mut u64, count: usize, stride: u64) -> io::Result<()> {
        *total = u64::try_from(count)
            .ok()
            .and_then(|n| n.checked_mul(stride))
            .and_then(|n| total.checked_add(n))
            .ok_or_else(|| io::Error::other("PLY payload length overflow"))?;
        Ok(())
    }
    fn string(total: &mut u64, value: &str) -> io::Result<()> {
        add(total, 1, 8)?;
        add(total, value.len(), 1)
    }
    fn column(total: &mut u64, values: &PlyScalars) -> io::Result<()> {
        add(total, 1, 9)?;
        add(total, values.len(), width(values.kind()))
    }
    let mut bytes = 8 + 1 + 4 * 8 + 4 * 8;
    for strings in [&mesh.comments, &mesh.object_info] {
        for value in strings {
            string(&mut bytes, value)?;
        }
    }
    for element in &mesh.elements {
        string(&mut bytes, &element.name)?;
        add(&mut bytes, 1, 16)?;
        for property in &element.properties {
            string(&mut bytes, &property.name)?;
            add(&mut bytes, 1, 1)?;
            match &property.values {
                PlyValues::Scalar(values) => column(&mut bytes, values)?,
                PlyValues::List { offsets, values, .. } => {
                    add(&mut bytes, 1, 9)?;
                    add(&mut bytes, offsets.len(), 4)?;
                    column(&mut bytes, values)?;
                }
            }
        }
    }
    add(&mut bytes, mesh.triangles.len(), 12)?;
    Ok(bytes)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn all_scalar_types_preserve_native_bits_in_independent_layout() {
        for (tag, values, raw) in [
            (0, PlyScalars::I8(vec![i8::MIN, i8::MAX]), vec![128, 127]),
            (1, PlyScalars::U8(vec![0, u8::MAX]), vec![0, 255]),
            (
                2,
                PlyScalars::I16(vec![i16::MIN, i16::MAX]),
                vec![0, 128, 255, 127],
            ),
            (3, PlyScalars::U16(vec![0, u16::MAX]), vec![0, 0, 255, 255]),
            (
                4,
                PlyScalars::I32(vec![i32::MIN, i32::MAX]),
                vec![0, 0, 0, 128, 255, 255, 255, 127],
            ),
            (
                5,
                PlyScalars::U32(vec![0, u32::MAX]),
                vec![0, 0, 0, 0, 255, 255, 255, 255],
            ),
            (
                6,
                PlyScalars::F32(vec![-0.0, 1.0]),
                vec![0, 0, 0, 128, 0, 0, 128, 63],
            ),
            (
                7,
                PlyScalars::F64(vec![-0.0, 1.0]),
                vec![0, 0, 0, 0, 0, 0, 0, 128, 0, 0, 0, 0, 0, 0, 240, 63],
            ),
        ] {
            let mut actual = Vec::new();
            scalars(&mut actual, &values).unwrap();
            let mut expected = vec![tag];
            expected.extend_from_slice(&[2, 0, 0, 0, 0, 0, 0, 0]);
            expected.extend_from_slice(&raw);
            assert_eq!(actual, expected);
        }
    }
    #[test]
    fn blocked_scalar_restore_preserves_bits_and_bounds_reader_calls() {
        struct CountReader {
            data: std::io::Cursor<Vec<u8>>,
            calls: usize,
        }
        impl Read for CountReader {
            fn read(&mut self, bytes: &mut [u8]) -> io::Result<usize> {
                self.calls += 1;
                self.data.read(bytes)
            }
        }
        let values = PlyScalars::F64(
            (0..5003)
                .map(|index| match index % 4 {
                    0 => -0.0,
                    1 => f64::from_bits(0x7ff8_0000_0000_0042),
                    2 => f64::INFINITY,
                    _ => f64::MIN_POSITIVE,
                })
                .collect(),
        );
        let mut encoded = Vec::new();
        scalars(&mut encoded, &values).unwrap();
        let mut reader = CountReader {
            data: std::io::Cursor::new(encoded.clone()),
            calls: 0,
        };
        let mut input = (&mut reader).take(encoded.len() as u64);
        let root = rrrah_core::MemoryBudget::new(5003 * 8);
        let mut credit = root.try_reserve(0).unwrap();
        let restored = read_scalars(&mut input, &mut credit).unwrap();
        assert_eq!(input.limit(), 0);
        assert_eq!(reader.calls, 5); // Type, count, and three bounded payload blocks.
        let mut actual = Vec::new();
        scalars(&mut actual, &restored).unwrap();
        assert_eq!(actual, encoded);
        assert_eq!(root.used(), 5003 * 8);
        drop(restored);
        drop(credit);
        assert_eq!(root.used(), 0);
    }
    #[test]
    fn native_fixture_framing_length_matches_all_encodings_and_precision() {
        for name in [
            "ply-32-ascii.ply",
            "ply-32-le.ply",
            "ply-32-be.ply",
            "ply-64-ascii.ply",
            "ply-64-le.ply",
            "ply-64-be.ply",
            "ply-offset64-ascii.ply",
            "ply-offset64-le.ply",
            "ply-offset64-be.ply",
            "ply-u64-ascii.ply",
            "ply-u64-le.ply",
            "ply-u64-be.ply",
        ] {
            let path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("../../tests/fixtures/models")
                .join(name);
            let mesh = crate::decode_ply(&crate::DecodeRequest::new(path)).unwrap();
            let mut bytes = Vec::new();
            write_ply_swap_payload(&mesh, &mut bytes).unwrap();
            assert_eq!(bytes.len() as u64, ply_swap_payload_len(&mesh).unwrap(), "{name}");
            assert_eq!(&bytes[..8], b"RRPLY001");
            let root = rrrah_core::MemoryBudget::new(1024 * 1024);
            let restored = read_ply_swap_payload(&mut bytes.as_slice(), bytes.len() as u64, &root).unwrap();
            let mut encoded = Vec::new();
            write_ply_swap_payload(&restored, &mut encoded).unwrap();
            assert_eq!(encoded, bytes, "{name}");
            assert_eq!(restored.bounds, mesh.bounds);
            assert_eq!(restored.encoding, mesh.encoding);
            assert_eq!(restored.comments, mesh.comments);
            assert_eq!(restored.object_info, mesh.object_info);
            assert_eq!(restored.triangles, mesh.triangles);
            assert_eq!(restored.elements.len(), mesh.elements.len());
            for (actual, expected) in restored.elements.iter().zip(&mesh.elements) {
                assert_eq!(actual.name, expected.name);
                assert_eq!(actual.count, expected.count);
                assert_eq!(actual.properties.len(), expected.properties.len());
                for (a, e) in actual.properties.iter().zip(&expected.properties) {
                    assert_eq!(a.name, e.name);
                    let (a, e) = match (&a.values, &e.values) {
                        (PlyValues::Scalar(a), PlyValues::Scalar(e)) => (a, e),
                        (
                            PlyValues::List {
                                count_type: ac,
                                offsets: ao,
                                values: a,
                            },
                            PlyValues::List {
                                count_type: ec,
                                offsets: eo,
                                values: e,
                            },
                        ) => {
                            assert_eq!(ac, ec);
                            assert_eq!(ao, eo);
                            (a, e)
                        }
                        _ => panic!("{name}: property representation changed"),
                    };
                    assert_eq!(a.kind(), e.kind());
                    assert_eq!(a.len(), e.len());
                    match (a, e) {
                        (PlyScalars::F32(a), PlyScalars::F32(e)) => {
                            assert!(a.iter().zip(e).all(|(a, e)| a.to_bits() == e.to_bits()))
                        }
                        (PlyScalars::F64(a), PlyScalars::F64(e)) => {
                            assert!(a.iter().zip(e).all(|(a, e)| a.to_bits() == e.to_bits()))
                        }
                        _ => {
                            for index in 0..a.len() {
                                assert_eq!(a.get(index), e.get(index));
                            }
                        }
                    }
                }
            }
            let retained = crate::DecodedModel::Ply(restored.clone()).capacity_bytes();
            assert_eq!(root.used(), retained);
            let clone = restored.clone();
            drop(restored);
            assert_eq!(root.used(), retained);
            drop(clone);
            assert_eq!(root.used(), 0);
            let short = rrrah_core::MemoryBudget::new(retained - 1);
            assert!(matches!(
                read_ply_swap_payload(&mut bytes.as_slice(), bytes.len() as u64, &short),
                Err(PlySwapError::Memory(_))
            ));
            assert_eq!(short.used(), 0);
            for end in 0..bytes.len() {
                assert!(read_ply_swap_payload(&mut &bytes[..end], bytes.len() as u64, &root).is_err());
                assert_eq!(root.used(), 0);
            }
            let mut invalid_triangle = bytes.clone();
            let last = invalid_triangle.len() - 12;
            invalid_triangle[last..last + 4].copy_from_slice(&u32::MAX.to_le_bytes());
            assert!(
                read_ply_swap_payload(
                    &mut invalid_triangle.as_slice(),
                    invalid_triangle.len() as u64,
                    &root
                )
                .is_err()
            );
            assert_eq!(root.used(), 0);
            let name_end = bytes
                .windows(b"vertex_indices".len())
                .position(|value| value == b"vertex_indices")
                .unwrap()
                + b"vertex_indices".len();
            let mut invalid_offsets = bytes.clone();
            invalid_offsets[name_end + 10..name_end + 14].copy_from_slice(&1_u32.to_le_bytes());
            assert!(
                read_ply_swap_payload(
                    &mut invalid_offsets.as_slice(),
                    invalid_offsets.len() as u64,
                    &root
                )
                .is_err()
            );
            assert_eq!(root.used(), 0);
            let mut selector = bytes.clone();
            selector[9..17].copy_from_slice(&u64::MAX.to_le_bytes());
            assert!(read_ply_swap_payload(&mut selector.as_slice(), selector.len() as u64, &root).is_err());
            assert_eq!(root.used(), 0);
            let mut trailing = bytes.clone();
            trailing.push(0);
            assert!(read_ply_swap_payload(&mut trailing.as_slice(), trailing.len() as u64, &root).is_err());
            assert_eq!(root.used(), 0);
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum PlySwapError {
    #[error(transparent)]
    Io(#[from] io::Error),
    #[error(transparent)]
    Memory(#[from] rrrah_core::BufferError),
    #[error("invalid PLY swap payload: {0}")]
    Invalid(&'static str),
}
type RestoreResult<T> = std::result::Result<T, PlySwapError>;
fn bad() -> PlySwapError {
    PlySwapError::Invalid("layout or value")
}
fn reserve<T>(n: usize, credit: &mut rrrah_core::Reservation) -> RestoreResult<Vec<T>> {
    let bytes = n
        .checked_mul(size_of::<T>())
        .and_then(|n| u64::try_from(n).ok())
        .ok_or_else(bad)?;
    let next = credit
        .bytes()
        .checked_add(bytes)
        .filter(|n| *n <= 512 * 1024 * 1024)
        .ok_or_else(bad)?;
    credit.ensure_bytes(next)?;
    let mut out = Vec::new();
    out.try_reserve_exact(n).map_err(|_| bad())?;
    let actual = out
        .capacity()
        .checked_mul(size_of::<T>())
        .and_then(|n| u64::try_from(n).ok())
        .ok_or_else(bad)?;
    credit.ensure_bytes(
        next.checked_add(actual.checked_sub(bytes).ok_or_else(bad)?)
            .filter(|n| *n <= 512 * 1024 * 1024)
            .ok_or_else(bad)?,
    )?;
    Ok(out)
}
fn read_tag(r: &mut impl std::io::Read) -> RestoreResult<u8> {
    let mut b = [0];
    r.read_exact(&mut b)?;
    Ok(b[0])
}
fn read_u64(r: &mut impl std::io::Read) -> RestoreResult<u64> {
    let mut b = [0; 8];
    r.read_exact(&mut b)?;
    Ok(u64::from_le_bytes(b))
}
fn read_size(r: &mut impl std::io::Read) -> RestoreResult<usize> {
    usize::try_from(read_u64(r)?).map_err(|_| bad())
}
fn read_count<R: std::io::Read>(r: &mut std::io::Take<R>, stride: u64) -> RestoreResult<usize> {
    let n = read_u64(r)?;
    if n > r.limit() / stride {
        return Err(bad());
    }
    usize::try_from(n).map_err(|_| bad())
}
fn read_type(r: &mut impl std::io::Read) -> RestoreResult<PlyType> {
    match read_tag(r)? {
        0 => Ok(PlyType::I8),
        1 => Ok(PlyType::U8),
        2 => Ok(PlyType::I16),
        3 => Ok(PlyType::U16),
        4 => Ok(PlyType::I32),
        5 => Ok(PlyType::U32),
        6 => Ok(PlyType::F32),
        7 => Ok(PlyType::F64),
        _ => Err(bad()),
    }
}
fn integer(kind: PlyType) -> bool {
    !matches!(kind, PlyType::F32 | PlyType::F64)
}
fn read_text<R: std::io::Read>(
    r: &mut std::io::Take<R>,
    credit: &mut rrrah_core::Reservation,
) -> RestoreResult<String> {
    let n = read_count(r, 1)?;
    if n > 65536 {
        return Err(bad());
    }
    let mut bytes = reserve(n, credit)?;
    bytes.resize(n, 0);
    r.read_exact(&mut bytes)?;
    String::from_utf8(bytes).map_err(|_| bad())
}
fn read_scalars<R: std::io::Read>(
    r: &mut std::io::Take<R>,
    credit: &mut rrrah_core::Reservation,
) -> RestoreResult<PlyScalars> {
    let kind = read_type(r)?;
    let n = read_count(r, width(kind))?;
    macro_rules! read_values { ($($variant:ident:$ty:ty),*) => {match kind { $(PlyType::$variant=>{
        let mut values=reserve::<$ty>(n,credit)?;
        let mut block = [0_u8; 16 * 1024];
        let per_block = block.len() / size_of::<$ty>();
        while values.len() < n {
            let count = (n - values.len()).min(per_block);
            let bytes = count * size_of::<$ty>();
            r.read_exact(&mut block[..bytes])?;
            for raw in block[..bytes].chunks_exact(size_of::<$ty>()) {
                values.push(<$ty>::from_le_bytes(raw.try_into().map_err(|_| bad())?));
            }
        }
        PlyScalars::$variant(values)
    },)* }} }
    let values = read_values!(I8:i8,U8:u8,I16:i16,U16:u16,I32:i32,U32:u32,F32:f32,F64:f64);
    Ok(values)
}
/// Restores typed columns directly into their final budgeted arrays. The store
/// supplies checksum verification, cancellation and the exact payload length.
pub fn read_ply_swap_payload(
    reader: &mut impl std::io::Read,
    bytes: u64,
    budget: &rrrah_core::MemoryBudget,
) -> RestoreResult<crate::ModelBuffer<PlyMesh>> {
    let mut r = reader.take(bytes);
    let mut magic = [0; 8];
    r.read_exact(&mut magic)?;
    if &magic != b"RRPLY001" {
        return Err(bad());
    }
    let encoding = match read_tag(&mut r)? {
        0 => PlyEncoding::Ascii,
        1 => PlyEncoding::LittleEndian,
        2 => PlyEncoding::BigEndian,
        _ => return Err(bad()),
    };
    let vertex_element = read_size(&mut r)?;
    let mut position_properties = [0; 3];
    for p in &mut position_properties {
        *p = read_size(&mut r)?;
    }
    let mut credit = budget.try_reserve(size_of::<PlyMesh>() as u64)?;
    let mut read_strings = |r: &mut std::io::Take<_>| -> RestoreResult<Vec<String>> {
        let n = read_count(r, 8)?;
        let mut values = reserve(n, &mut credit)?;
        for _ in 0..n {
            values.push(read_text(r, &mut credit)?);
        }
        Ok(values)
    };
    let comments = read_strings(&mut r)?;
    let object_info = read_strings(&mut r)?;
    let n = read_count(&mut r, 24)?;
    if n > 128 {
        return Err(bad());
    }
    let mut property_count = 0_usize;
    let mut elements = reserve(n, &mut credit)?;
    for _ in 0..n {
        let name = read_text(&mut r, &mut credit)?;
        let count = read_size(&mut r)?;
        let n = read_count(&mut r, 18)?;
        property_count = property_count
            .checked_add(n)
            .filter(|n| *n <= 4096)
            .ok_or_else(bad)?;
        let mut properties = reserve(n, &mut credit)?;
        for _ in 0..n {
            let name = read_text(&mut r, &mut credit)?;
            let values = match read_tag(&mut r)? {
                0 => {
                    let values = read_scalars(&mut r, &mut credit)?;
                    if values.len() != count {
                        return Err(bad());
                    }
                    PlyValues::Scalar(values)
                }
                1 => {
                    let count_type = read_type(&mut r)?;
                    if !integer(count_type) {
                        return Err(bad());
                    }
                    let n = read_count(&mut r, 4)?;
                    if count.checked_add(1) != Some(n) {
                        return Err(bad());
                    }
                    let mut offsets = reserve(n, &mut credit)?;
                    for _ in 0..n {
                        let mut raw = [0; 4];
                        r.read_exact(&mut raw)?;
                        offsets.push(u32::from_le_bytes(raw));
                    }
                    let values = read_scalars(&mut r, &mut credit)?;
                    if offsets.first() != Some(&0)
                        || offsets.last().copied().and_then(|v| usize::try_from(v).ok()) != Some(values.len())
                        || offsets.windows(2).any(|v| v[0] > v[1])
                    {
                        return Err(bad());
                    }
                    let max = match count_type {
                        PlyType::I8 => 127,
                        PlyType::U8 => 255,
                        PlyType::I16 => 32767,
                        PlyType::U16 => 65535,
                        PlyType::I32 => i32::MAX as u32,
                        PlyType::U32 => u32::MAX,
                        _ => return Err(bad()),
                    };
                    if offsets.windows(2).any(|v| v[1] - v[0] > max.min(1024 * 1024)) {
                        return Err(bad());
                    }
                    PlyValues::List {
                        count_type,
                        offsets,
                        values,
                    }
                }
                _ => return Err(bad()),
            };
            properties.push(crate::PlyProperty { name, values });
        }
        if elements
            .iter()
            .any(|element: &crate::PlyElement| element.name == name)
            || properties
                .iter()
                .enumerate()
                .any(|(i, p)| properties[..i].iter().any(|previous| previous.name == p.name))
        {
            return Err(bad());
        }
        elements.push(crate::PlyElement {
            name,
            count,
            properties,
        });
    }
    let vertex = elements
        .get(vertex_element)
        .filter(|e| e.name == "vertex")
        .ok_or_else(bad)?;
    for (axis, name) in ["x", "y", "z"].iter().enumerate() {
        let property = vertex
            .properties
            .get(position_properties[axis])
            .filter(|p| p.name == *name)
            .ok_or_else(bad)?;
        if !matches!(property.values, PlyValues::Scalar(_)) {
            return Err(bad());
        }
    }
    let vertex_count = vertex.count;
    let n = read_count(&mut r, 12)?;
    let mut triangles = reserve(n, &mut credit)?;
    for _ in 0..n {
        let mut triangle = [0; 3];
        for index in &mut triangle {
            let mut raw = [0; 4];
            r.read_exact(&mut raw)?;
            *index = u32::from_le_bytes(raw);
            if usize::try_from(*index).map_err(|_| bad())? >= vertex_count {
                return Err(bad());
            }
        }
        triangles.push(triangle);
    }
    let mut expected = 0_usize;
    if let Some(faces) = elements.iter().find(|e| e.name == "face") {
        let mut candidates = faces
            .properties
            .iter()
            .filter(|p| p.name == "vertex_indices" || p.name == "vertex_index");
        let property = candidates.next().ok_or_else(bad)?;
        if candidates.next().is_some() {
            return Err(bad());
        }
        let PlyValues::List { offsets, values, .. } = &property.values else {
            return Err(bad());
        };
        if !integer(values.kind()) {
            return Err(bad());
        }
        for range in offsets.windows(2) {
            let start = usize::try_from(range[0]).map_err(|_| bad())?;
            let end = usize::try_from(range[1]).map_err(|_| bad())?;
            if !(3..=1024).contains(&(end - start)) {
                return Err(bad());
            }
            expected = expected.checked_add(end - start - 2).ok_or_else(bad)?;
            for index in start..end {
                if values.integer_index(index).map_err(|_| bad())? >= vertex_count {
                    return Err(bad());
                }
            }
        }
    }
    if expected != triangles.len() || (vertex_count > 0 && expected == 0) || r.limit() != 0 {
        return Err(bad());
    }
    let mut mesh = PlyMesh {
        encoding,
        comments,
        object_info,
        elements,
        triangles,
        bounds: None,
        vertex_element,
        position_properties,
    };
    for index in 0..vertex_count {
        let point = mesh.vertex(u32::try_from(index).map_err(|_| bad())?);
        if point.iter().any(|value| !value.is_finite()) {
            return Err(bad());
        }
        let bounds = mesh.bounds.get_or_insert([point, point]);
        for axis in 0..3 {
            bounds[0][axis] = bounds[0][axis].min(point[axis]);
            bounds[1][axis] = bounds[1][axis].max(point[axis]);
        }
    }
    Ok(crate::ModelBuffer::from_reserved(mesh, credit))
}
