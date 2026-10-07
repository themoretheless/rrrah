//! Versioned OBJ swap writer preserving adjacent shared metadata ownership.
//! Managed restoration preserves source arrays and adjacent metadata sharing.
use crate::{ObjFace, ObjMesh};
use std::{
    io::{self, Read, Write},
    sync::Arc,
};
fn uint(writer: &mut impl Write, value: usize) -> io::Result<()> {
    writer.write_all(
        &u64::try_from(value)
            .map_err(|_| io::Error::other("OBJ count overflow"))?
            .to_le_bytes(),
    )
}
fn string(writer: &mut impl Write, value: &str) -> io::Result<()> {
    uint(writer, value.len())?;
    writer.write_all(value.as_bytes())
}
fn optional_index(value: Option<u32>) -> [u8; 5] {
    let mut bytes = [0; 5];
    if let Some(value) = value {
        bytes[0] = 1;
        bytes[1..].copy_from_slice(&value.to_le_bytes());
    }
    bytes
}
fn same_object(face: &ObjFace, previous: Option<&ObjFace>) -> bool {
    previous.is_some_and(|previous| Arc::ptr_eq(&face.object, &previous.object))
}
fn same_groups(face: &ObjFace, previous: Option<&ObjFace>) -> bool {
    previous.is_some_and(|previous| Arc::ptr_eq(&face.groups, &previous.groups))
}
fn same_material(face: &ObjFace, previous: Option<&ObjFace>) -> bool {
    match (
        &face.material,
        previous.and_then(|previous| previous.material.as_ref()),
    ) {
        (Some(value), Some(previous)) => Arc::ptr_eq(value, previous),
        _ => false,
    }
}
fn arrays<const N: usize>(writer: &mut impl Write, values: &[[f32; N]]) -> io::Result<()> {
    uint(writer, values.len())?;
    crate::off_swap::scalar_blocks(writer, values.iter().flatten().map(|value| value.to_le_bytes()))
        .map_err(io::Error::other)
}
/// Writes original float32 bits, indexed corners, triangles and metadata.
/// No geometry or metadata arrays are cloned.
pub fn write_obj_swap_payload(mesh: &ObjMesh, writer: &mut impl Write) -> io::Result<()> {
    writer.write_all(b"RROBJ001")?;
    arrays(writer, &mesh.positions)?;
    arrays(writer, &mesh.texcoords)?;
    arrays(writer, &mesh.normals)?;
    uint(writer, mesh.corners.len())?;
    crate::off_swap::scalar_blocks(
        writer,
        mesh.corners.iter().map(|corner| {
            let mut record = [0; 14];
            record[..4].copy_from_slice(&corner.position.to_le_bytes());
            record[4..9].copy_from_slice(&optional_index(corner.texcoord));
            record[9..].copy_from_slice(&optional_index(corner.normal));
            record
        }),
    )
    .map_err(io::Error::other)?;
    uint(writer, mesh.triangles.len())?;
    crate::off_swap::scalar_blocks(
        writer,
        mesh.triangles.iter().map(|triangle| {
            let mut record = [0; 16];
            for (bytes, index) in record
                .chunks_exact_mut(4)
                .zip(triangle.corners.into_iter().chain([triangle.face]))
            {
                bytes.copy_from_slice(&index.to_le_bytes());
            }
            record
        }),
    )
    .map_err(io::Error::other)?;
    uint(writer, mesh.faces.len())?;
    let mut previous = None;
    for face in &mesh.faces {
        uint(writer, face.corners.start)?;
        uint(writer, face.corners.end)?;
        if same_object(face, previous) {
            writer.write_all(&[0])?;
        } else {
            writer.write_all(&[1])?;
            string(writer, &face.object)?;
        }
        if same_groups(face, previous) {
            writer.write_all(&[0])?;
        } else {
            writer.write_all(&[1])?;
            uint(writer, face.groups.len())?;
            for group in face.groups.iter() {
                string(writer, group)?;
            }
        }
        match &face.material {
            None => writer.write_all(&[0])?,
            Some(_) if same_material(face, previous) => writer.write_all(&[1])?,
            Some(material) => {
                writer.write_all(&[2])?;
                string(writer, material)?;
            }
        }
        writer.write_all(&optional_index(face.smoothing_group))?;
        previous = Some(face);
    }
    uint(writer, mesh.material_libraries.len())?;
    for library in &mesh.material_libraries {
        string(writer, library)?;
    }
    Ok(())
}
/// Computes length from container sizes and metadata references; never scans
/// coordinate, corner or triangle values.
pub fn obj_swap_payload_len(mesh: &ObjMesh) -> io::Result<u64> {
    fn add(total: &mut u64, count: usize, stride: u64) -> io::Result<()> {
        *total = u64::try_from(count)
            .ok()
            .and_then(|count| count.checked_mul(stride))
            .and_then(|n| total.checked_add(n))
            .ok_or_else(|| io::Error::other("OBJ payload length overflow"))?;
        Ok(())
    }
    fn text(total: &mut u64, value: &str) -> io::Result<()> {
        add(total, 1, 8)?;
        add(total, value.len(), 1)
    }
    let mut bytes = 8 + 7 * 8;
    for (count, stride) in [
        (mesh.positions.len(), 16),
        (mesh.texcoords.len(), 12),
        (mesh.normals.len(), 12),
        (mesh.corners.len(), 14),
        (mesh.triangles.len(), 16),
        (mesh.faces.len(), 24),
    ] {
        add(&mut bytes, count, stride)?;
    }
    let mut previous = None;
    for face in &mesh.faces {
        if !same_object(face, previous) {
            text(&mut bytes, &face.object)?;
        }
        if !same_groups(face, previous) {
            add(&mut bytes, 1, 8)?;
            for group in face.groups.iter() {
                text(&mut bytes, group)?;
            }
        }
        if let Some(material) = &face.material {
            if !same_material(face, previous) {
                text(&mut bytes, material)?;
            }
        }
        previous = Some(face);
    }
    for library in &mesh.material_libraries {
        text(&mut bytes, library)?;
    }
    Ok(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn numeric_arrays_cross_block_boundary_and_release_truncated_allocations() {
        use std::io::Read;
        struct Count<'a> {
            input: &'a [u8],
            calls: usize,
        }
        impl Read for Count<'_> {
            fn read(&mut self, out: &mut [u8]) -> io::Result<usize> {
                self.calls += 1;
                self.input.read(out)
            }
        }
        let source = vec![[-0.0_f32, 1.25, -2.5]; 2731];
        let mut bytes = Vec::new();
        arrays(&mut bytes, &source).unwrap();
        let budget = rrrah_core::MemoryBudget::new(128 * 1024);
        let mut credit = budget.try_reserve(0).unwrap();
        let mut reader = Count {
            input: &bytes,
            calls: 0,
        }
        .take(bytes.len() as u64);
        let decoded = read_arrays::<3, _>(&mut reader, &mut credit).unwrap();
        assert_eq!(reader.get_ref().calls, 4); // count plus three record blocks
        for value in decoded {
            assert_eq!(value.map(f32::to_bits), source[0].map(f32::to_bits));
        }
        drop(credit);
        assert_eq!(budget.used(), 0);
        let mut credit = budget.try_reserve(0).unwrap();
        let mut truncated = (&bytes[..bytes.len() - 1]).take(bytes.len() as u64);
        assert!(read_arrays::<3, _>(&mut truncated, &mut credit).is_err());
        drop(credit);
        assert_eq!(budget.used(), 0);
        for stride in [14, 16] {
            let count = 16 * 1024 / stride * 2 + 1;
            let bytes: Vec<_> = (0..count * stride).map(|i| (i % 251) as u8).collect();
            let mut reader = Count {
                input: &bytes,
                calls: 0,
            };
            let mut offset = 0;
            read_records(&mut reader, count, stride, |record| {
                assert_eq!(record, &bytes[offset..offset + stride]);
                offset += stride;
                Ok(())
            })
            .unwrap();
            assert_eq!(offset, bytes.len());
            assert_eq!(reader.calls, 3);
            let mut truncated = &bytes[..bytes.len() - 1];
            assert!(read_records(&mut truncated, count, stride, |_| Ok(())).is_err());
        }
    }
    #[test]
    fn empty_mesh_has_independently_specified_wire_layout() {
        let mesh = ObjMesh::default();
        let mut actual = Vec::new();
        write_obj_swap_payload(&mesh, &mut actual).unwrap();
        let mut expected = b"RROBJ001".to_vec();
        expected.extend_from_slice(&[0; 56]);
        assert_eq!(actual, expected);
        assert_eq!(obj_swap_payload_len(&mesh).unwrap(), 64);
    }
    #[test]
    fn native_obj_corpus_preserves_geometry_and_attribute_references() {
        for name in [
            "obj-negative-quad.obj",
            "obj-triangle.obj",
            "obj-clockwise.obj",
            "obj-concave-u.obj",
            "obj-weight.obj",
            "obj-yz.obj",
            "obj-shared-metadata.obj",
        ] {
            let path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("../../tests/fixtures/models")
                .join(name);
            let mesh = crate::decode_obj(&crate::DecodeRequest::new(path)).unwrap();
            let mut bytes = Vec::new();
            write_obj_swap_payload(&mesh, &mut bytes).unwrap();
            let root = rrrah_core::MemoryBudget::new(1024 * 1024);
            let restored = read_obj_swap_payload(&mut bytes.as_slice(), bytes.len() as u64, &root).unwrap();
            let mut encoded = Vec::new();
            write_obj_swap_payload(&restored, &mut encoded).unwrap();
            assert_eq!(encoded, bytes, "{name}");
            assert_eq!(restored.bounds, mesh.bounds);
            assert_eq!(restored.corners, mesh.corners);
            assert_eq!(restored.triangles, mesh.triangles);
            assert_eq!(restored.faces.len(), mesh.faces.len());
            for (actual, expected) in restored.faces.iter().zip(&mesh.faces) {
                assert_eq!(actual.corners, expected.corners);
                assert_eq!(actual.object, expected.object);
                assert_eq!(actual.groups, expected.groups);
                assert_eq!(actual.material, expected.material);
                assert_eq!(actual.smoothing_group, expected.smoothing_group);
            }
            assert_eq!(
                restored.material_libraries, mesh.material_libraries,
                "{name}: material libraries"
            );
            assert_eq!(
                restored
                    .positions
                    .iter()
                    .flatten()
                    .map(|v| v.to_bits())
                    .collect::<Vec<_>>(),
                mesh.positions
                    .iter()
                    .flatten()
                    .map(|v| v.to_bits())
                    .collect::<Vec<_>>(),
                "{name}: homogeneous positions"
            );
            for (restored_values, source_values) in [
                (&restored.texcoords, &mesh.texcoords),
                (&restored.normals, &mesh.normals),
            ] {
                assert_eq!(
                    restored_values
                        .iter()
                        .flatten()
                        .map(|v| v.to_bits())
                        .collect::<Vec<_>>(),
                    source_values
                        .iter()
                        .flatten()
                        .map(|v| v.to_bits())
                        .collect::<Vec<_>>(),
                    "{name}: UV/normal bits"
                );
            }
            assert_eq!(
                root.used(),
                crate::DecodedModel::Obj(restored.clone()).capacity_bytes()
            );
            drop(restored);
            assert_eq!(root.used(), 0);
        }
    }
    #[test]
    fn shared_metadata_uses_previous_references_and_length_is_exact() {
        let source=b"mtllib first.mtl second.mtl\no shared object\ng one two\nusemtl shared material\ns 7\nv -0 0 0\nv 1 0 0\nv 0 1 0\nf 1 2 3\nf 1 2 3\n";
        let path = std::env::temp_dir().join(format!("rrrah-obj-swap-{}.obj", std::process::id()));
        std::fs::write(&path, source).unwrap();
        let mesh = crate::decode_obj(&crate::DecodeRequest::new(&path)).unwrap();
        std::fs::remove_file(path).unwrap();
        let mut bytes = Vec::new();
        write_obj_swap_payload(&mesh, &mut bytes).unwrap();
        assert_eq!(obj_swap_payload_len(&mesh).unwrap(), bytes.len() as u64);
        let root = rrrah_core::MemoryBudget::new(1024 * 1024);
        let restored = read_obj_swap_payload(&mut bytes.as_slice(), bytes.len() as u64, &root).unwrap();
        let mut roundtrip = Vec::new();
        write_obj_swap_payload(&restored, &mut roundtrip).unwrap();
        assert_eq!(roundtrip, bytes);
        assert_eq!(restored.bounds, mesh.bounds);
        assert!(Arc::ptr_eq(&restored.faces[0].object, &restored.faces[1].object));
        assert!(Arc::ptr_eq(&restored.faces[0].groups, &restored.faces[1].groups));
        assert!(Arc::ptr_eq(
            restored.faces[0].material.as_ref().unwrap(),
            restored.faces[1].material.as_ref().unwrap()
        ));
        let retained = crate::DecodedModel::Obj(restored.clone()).capacity_bytes();
        assert_eq!(root.used(), retained);
        let clone = restored.clone();
        drop(restored);
        assert_eq!(root.used(), retained);
        drop(clone);
        assert_eq!(root.used(), 0);
        let short = rrrah_core::MemoryBudget::new(retained - 1);
        assert!(matches!(
            read_obj_swap_payload(&mut bytes.as_slice(), bytes.len() as u64, &short),
            Err(ObjSwapError::Memory(_))
        ));
        assert_eq!(short.used(), 0);
        for end in 0..bytes.len() {
            assert!(read_obj_swap_payload(&mut &bytes[..end], bytes.len() as u64, &root).is_err());
            assert_eq!(root.used(), 0);
        }
        let first_corner =
            8 + 3 * 8 + mesh.positions.len() * 16 + mesh.texcoords.len() * 12 + mesh.normals.len() * 12 + 8;
        let mut bad_index = bytes.clone();
        bad_index[first_corner..first_corner + 4].copy_from_slice(&u32::MAX.to_le_bytes());
        assert!(read_obj_swap_payload(&mut bad_index.as_slice(), bad_index.len() as u64, &root).is_err());
        assert_eq!(root.used(), 0);
        let first_object = first_corner + mesh.corners.len() * 14 + 8 + mesh.triangles.len() * 16 + 8 + 16;
        let mut bad_reference = bytes.clone();
        bad_reference[first_object] = 0;
        assert!(
            read_obj_swap_payload(&mut bad_reference.as_slice(), bad_reference.len() as u64, &root).is_err()
        );
        assert_eq!(root.used(), 0);
        assert!(same_object(&mesh.faces[1], Some(&mesh.faces[0])));
        assert!(same_groups(&mesh.faces[1], Some(&mesh.faces[0])));
        assert!(same_material(&mesh.faces[1], Some(&mesh.faces[0])));
        let mut expanded = mesh.clone();
        expanded.faces[1].object = Arc::from(&*mesh.faces[1].object);
        expanded.faces[1].groups = Arc::from(mesh.faces[1].groups.to_vec());
        expanded.faces[1].material = Some(Arc::from(&**mesh.faces[1].material.as_ref().unwrap()));
        let mut expanded_bytes = Vec::new();
        write_obj_swap_payload(&expanded, &mut expanded_bytes).unwrap();
        assert_eq!(
            obj_swap_payload_len(&expanded).unwrap(),
            expanded_bytes.len() as u64
        );
        let metadata = 8
            + mesh.faces[1].object.len()
            + 8
            + mesh.faces[1].groups.iter().map(|g| 8 + g.len()).sum::<usize>()
            + 8
            + mesh.faces[1].material.as_ref().unwrap().len();
        assert_eq!(expanded_bytes.len() - bytes.len(), metadata);
    }
}

#[derive(Debug, thiserror::Error)]
pub enum ObjSwapError {
    #[error(transparent)]
    Io(#[from] io::Error),
    #[error(transparent)]
    Memory(#[from] rrrah_core::BufferError),
    #[error("invalid OBJ swap payload: {0}")]
    Invalid(&'static str),
}
type RestoreResult<T> = std::result::Result<T, ObjSwapError>;
fn bad() -> ObjSwapError {
    ObjSwapError::Invalid("layout or value")
}
fn charge(credit: &mut rrrah_core::Reservation, bytes: usize) -> RestoreResult<()> {
    let next = u64::try_from(bytes)
        .ok()
        .and_then(|n| credit.bytes().checked_add(n))
        .filter(|n| *n <= 512 * 1024 * 1024)
        .ok_or_else(bad)?;
    credit.ensure_bytes(next)?;
    Ok(())
}
fn reserve<T>(n: usize, credit: &mut rrrah_core::Reservation) -> RestoreResult<Vec<T>> {
    let bytes = n.checked_mul(size_of::<T>()).ok_or_else(bad)?;
    charge(credit, bytes)?;
    let mut values = Vec::new();
    values.try_reserve_exact(n).map_err(|_| bad())?;
    charge(
        credit,
        values
            .capacity()
            .checked_mul(size_of::<T>())
            .and_then(|actual| actual.checked_sub(bytes))
            .ok_or_else(bad)?,
    )?;
    Ok(values)
}
fn read_u64(r: &mut impl std::io::Read) -> RestoreResult<u64> {
    let mut raw = [0; 8];
    r.read_exact(&mut raw)?;
    Ok(u64::from_le_bytes(raw))
}
fn read_count<R: std::io::Read>(r: &mut std::io::Take<R>, stride: u64) -> RestoreResult<usize> {
    let n = read_u64(r)?;
    if n > r.limit() / stride {
        return Err(bad());
    }
    usize::try_from(n).map_err(|_| bad())
}
fn read_tag(r: &mut impl std::io::Read) -> RestoreResult<u8> {
    let mut raw = [0];
    r.read_exact(&mut raw)?;
    Ok(raw[0])
}
fn read_optional(r: &mut impl std::io::Read) -> RestoreResult<Option<u32>> {
    let tag = read_tag(r)?;
    let mut raw = [0; 4];
    r.read_exact(&mut raw)?;
    match tag {
        0 if raw == [0; 4] => Ok(None),
        1 => Ok(Some(u32::from_le_bytes(raw))),
        _ => Err(bad()),
    }
}
fn read_arrays<const N: usize, R: std::io::Read>(
    r: &mut std::io::Take<R>,
    credit: &mut rrrah_core::Reservation,
) -> RestoreResult<Vec<[f32; N]>> {
    let n = read_count(r, (N * 4) as u64)?;
    let mut values = reserve(n, credit)?;
    let stride = N
        .checked_mul(4)
        .filter(|s| *s > 0 && *s <= 16 * 1024)
        .ok_or_else(bad)?;
    let mut block = [0_u8; 16 * 1024];
    while values.len() < n {
        let records = (n - values.len()).min(block.len() / stride);
        let bytes = records * stride;
        r.read_exact(&mut block[..bytes])?;
        for record in block[..bytes].chunks_exact(stride) {
            let mut value = [0.; N];
            for (scalar, raw) in value.iter_mut().zip(record.chunks_exact(4)) {
                *scalar = f32::from_le_bytes(raw.try_into().map_err(|_| bad())?);
                if !scalar.is_finite() {
                    return Err(bad());
                }
            }
            values.push(value);
        }
    }
    Ok(values)
}
fn read_records(
    reader: &mut impl std::io::Read,
    count: usize,
    stride: usize,
    mut accept: impl FnMut(&[u8]) -> RestoreResult<()>,
) -> RestoreResult<()> {
    let mut block = [0_u8; 16 * 1024];
    if stride == 0 || stride > block.len() {
        return Err(bad());
    }
    let mut remaining = count;
    while remaining > 0 {
        let records = remaining.min(block.len() / stride);
        let bytes = records * stride;
        reader.read_exact(&mut block[..bytes])?;
        for record in block[..bytes].chunks_exact(stride) {
            accept(record)?;
        }
        remaining -= records;
    }
    Ok(())
}
fn read_string<R: std::io::Read>(
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
fn read_arc<R: std::io::Read>(
    r: &mut std::io::Take<R>,
    credit: &mut rrrah_core::Reservation,
    budget: &rrrah_core::MemoryBudget,
) -> RestoreResult<Arc<str>> {
    let mut scratch = budget.try_reserve(0)?;
    let text = read_string(r, &mut scratch)?;
    charge(credit, text.len())?;
    let out = Arc::from(text.as_str());
    drop(text);
    drop(scratch);
    Ok(out)
}
/// Restores original indexed geometry and shared metadata directly into managed
/// native storage. Integrity and cancellation are provided by the swap store.
pub fn read_obj_swap_payload(
    reader: &mut impl std::io::Read,
    bytes: u64,
    budget: &rrrah_core::MemoryBudget,
) -> RestoreResult<crate::ModelBuffer<ObjMesh>> {
    let mut r = reader.take(bytes);
    let mut magic = [0; 8];
    r.read_exact(&mut magic)?;
    if &magic != b"RROBJ001" {
        return Err(bad());
    }
    let mut credit = budget.try_reserve(size_of::<ObjMesh>() as u64)?;
    let positions: Vec<[f32; 4]> = read_arrays(&mut r, &mut credit)?;
    let texcoords = read_arrays(&mut r, &mut credit)?;
    let normals = read_arrays(&mut r, &mut credit)?;
    let count = read_count(&mut r, 14)?;
    let mut corners = reserve(count, &mut credit)?;
    read_records(&mut r, count, 14, |record| {
        let position = u32::from_le_bytes(record[..4].try_into().map_err(|_| bad())?);
        let mut attributes = &record[4..];
        let texcoord = read_optional(&mut attributes)?;
        let normal = read_optional(&mut attributes)?;
        if usize::try_from(position).map_err(|_| bad())? >= positions.len()
            || texcoord.is_some_and(|i| usize::try_from(i).map_or(true, |i| i >= texcoords.len()))
            || normal.is_some_and(|i| usize::try_from(i).map_or(true, |i| i >= normals.len()))
        {
            return Err(bad());
        }
        corners.push(crate::ObjCorner {
            position,
            texcoord,
            normal,
        });
        Ok(())
    })?;
    let count = read_count(&mut r, 16)?;
    let mut triangles = reserve(count, &mut credit)?;
    read_records(&mut r, count, 16, |raw| {
        let mut record = [0_u32; 4];
        for (index, bytes) in record.iter_mut().zip(raw.chunks_exact(4)) {
            *index = u32::from_le_bytes(bytes.try_into().map_err(|_| bad())?);
        }
        if record[..3]
            .iter()
            .any(|i| usize::try_from(*i).map_or(true, |i| i >= corners.len()))
        {
            return Err(bad());
        }
        triangles.push(crate::ObjTriangle {
            corners: [record[0], record[1], record[2]],
            face: record[3],
        });
        Ok(())
    })?;
    let count = read_count(&mut r, 24)?;
    let mut faces: Vec<ObjFace> = reserve(count, &mut credit)?;
    let mut previous_end = 0;
    let mut expected_triangles = 0_usize;
    for _ in 0..count {
        let start = usize::try_from(read_u64(&mut r)?).map_err(|_| bad())?;
        let end = usize::try_from(read_u64(&mut r)?).map_err(|_| bad())?;
        if start != previous_end || end < start || end > corners.len() || !(3..=1024).contains(&(end - start))
        {
            return Err(bad());
        }
        previous_end = end;
        expected_triangles = expected_triangles.checked_add(end - start - 2).ok_or_else(bad)?;
        let object = match read_tag(&mut r)? {
            0 => {
                let value = &faces.last().ok_or_else(bad)?.object;
                charge(&mut credit, value.len())?;
                value.clone()
            }
            1 => read_arc(&mut r, &mut credit, budget)?,
            _ => return Err(bad()),
        };
        let groups = match read_tag(&mut r)? {
            0 => {
                let value = &faces.last().ok_or_else(bad)?.groups;
                charge(
                    &mut credit,
                    value.len().checked_mul(size_of::<String>()).ok_or_else(bad)?,
                )?;
                for group in value.iter() {
                    charge(&mut credit, group.capacity())?;
                }
                value.clone()
            }
            1 => {
                let n = read_count(&mut r, 8)?;
                if n > 65536 {
                    return Err(bad());
                }
                let mut values = reserve::<String>(n, &mut credit)?;
                for _ in 0..n {
                    values.push(read_string(&mut r, &mut credit)?);
                }
                let overlap = budget.try_reserve((values.capacity() * size_of::<String>()) as u64)?;
                let values: Arc<[String]> = values.into();
                drop(overlap);
                values
            }
            _ => return Err(bad()),
        };
        let material = match read_tag(&mut r)? {
            0 => None,
            1 => {
                let value = faces
                    .last()
                    .and_then(|face| face.material.as_ref())
                    .ok_or_else(bad)?;
                charge(&mut credit, value.len())?;
                Some(value.clone())
            }
            2 => Some(read_arc(&mut r, &mut credit, budget)?),
            _ => return Err(bad()),
        };
        let smoothing_group = read_optional(&mut r)?;
        faces.push(ObjFace {
            corners: start..end,
            object,
            groups,
            material,
            smoothing_group,
        });
    }
    if previous_end != corners.len() || expected_triangles != triangles.len() {
        return Err(bad());
    }
    for triangle in &triangles {
        let face = faces
            .get(usize::try_from(triangle.face).map_err(|_| bad())?)
            .ok_or_else(bad)?;
        if triangle
            .corners
            .iter()
            .any(|i| usize::try_from(*i).map_or(true, |i| !face.corners.contains(&i)))
        {
            return Err(bad());
        }
    }
    let count = read_count(&mut r, 8)?;
    let mut material_libraries = reserve(count, &mut credit)?;
    for _ in 0..count {
        material_libraries.push(read_string(&mut r, &mut credit)?);
    }
    if r.limit() != 0 {
        return Err(bad());
    }
    let mut bounds: Option<[[f32; 3]; 2]> = None;
    for p in &positions {
        let point = [p[0], p[1], p[2]];
        let b = bounds.get_or_insert([point, point]);
        for axis in 0..3 {
            b[0][axis] = b[0][axis].min(point[axis]);
            b[1][axis] = b[1][axis].max(point[axis]);
        }
    }
    Ok(crate::ModelBuffer::from_reserved(
        ObjMesh {
            positions,
            texcoords,
            normals,
            corners,
            faces,
            triangles,
            material_libraries,
            bounds,
        },
        credit,
    ))
}
