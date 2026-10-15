//! Lossless streaming payload for native OFF geometry and attributes.
use crate::{ModelBuffer, OffColor, OffEncoding, OffFace, OffMesh};
use rrrah_core::{MemoryBudget, Reservation};
use std::io::{Read, Write};
const MAGIC: &[u8; 8] = b"RROFF001";
const LIMIT: u64 = 512 * 1024 * 1024;
#[derive(Debug, thiserror::Error)]
pub enum OffSwapError {
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    Memory(#[from] rrrah_core::BufferError),
    #[error("invalid OFF swap payload: {0}")]
    Invalid(&'static str),
}
type Result<T> = std::result::Result<T, OffSwapError>;
fn invalid() -> OffSwapError {
    OffSwapError::Invalid("layout or value")
}
fn uint(w: &mut impl Write, n: usize) -> Result<()> {
    w.write_all(&u64::try_from(n).map_err(|_| invalid())?.to_le_bytes())?;
    Ok(())
}
fn read_u64(r: &mut impl Read) -> Result<u64> {
    let mut b = [0; 8];
    r.read_exact(&mut b)?;
    Ok(u64::from_le_bytes(b))
}
fn read_size(r: &mut impl Read) -> Result<usize> {
    usize::try_from(read_u64(r)?).map_err(|_| invalid())
}
fn read_count<R: Read>(r: &mut std::io::Take<R>, stride: u64) -> Result<usize> {
    let n = read_u64(r)?;
    if n > r.limit() / stride {
        return Err(invalid());
    }
    usize::try_from(n).map_err(|_| invalid())
}
pub(crate) fn scalar_blocks<const N: usize>(
    w: &mut impl Write,
    words: impl IntoIterator<Item = [u8; N]>,
) -> Result<()> {
    let mut block = [0_u8; 4096];
    let mut used = 0;
    for word in words {
        if used + N > block.len() {
            w.write_all(&block[..used])?;
            used = 0;
        }
        block[used..used + N].copy_from_slice(&word);
        used += N;
    }
    if used > 0 {
        w.write_all(&block[..used])?;
    }
    Ok(())
}
fn arrays<const N: usize>(w: &mut impl Write, values: &[[f64; N]]) -> Result<()> {
    uint(w, values.len())?;
    scalar_blocks(w, values.iter().flatten().map(|value| value.to_le_bytes()))
}
fn reserve<T>(n: usize, credit: &mut Reservation) -> Result<Vec<T>> {
    let bytes = n
        .checked_mul(size_of::<T>())
        .and_then(|n| u64::try_from(n).ok())
        .ok_or_else(invalid)?;
    let next = credit
        .bytes()
        .checked_add(bytes)
        .filter(|n| *n <= LIMIT)
        .ok_or_else(invalid)?;
    credit.ensure_bytes(next)?;
    let mut out = Vec::new();
    out.try_reserve_exact(n).map_err(|_| invalid())?;
    let actual = out
        .capacity()
        .checked_mul(size_of::<T>())
        .and_then(|n| u64::try_from(n).ok())
        .ok_or_else(invalid)?;
    credit.ensure_bytes(
        next.checked_add(actual.checked_sub(bytes).ok_or_else(invalid)?)
            .filter(|n| *n <= LIMIT)
            .ok_or_else(invalid)?,
    )?;
    Ok(out)
}
fn read_records<T, R: Read>(
    r: &mut R,
    count: usize,
    stride: usize,
    credit: &mut Reservation,
    mut decode: impl FnMut(&[u8]) -> Result<T>,
) -> Result<Vec<T>> {
    if stride == 0 || stride > 16 * 1024 {
        return Err(invalid());
    }
    let mut out = reserve(count, credit)?;
    let mut block = [0_u8; 16 * 1024];
    while out.len() < count {
        let n = (count - out.len()).min(block.len() / stride);
        let bytes = n * stride;
        r.read_exact(&mut block[..bytes])?;
        for record in block[..bytes].chunks_exact(stride) {
            out.push(decode(record)?);
        }
    }
    Ok(out)
}
fn read_arrays<const N: usize, R: Read>(
    r: &mut std::io::Take<R>,
    credit: &mut Reservation,
) -> Result<Vec<[f64; N]>> {
    let stride = N
        .checked_mul(8)
        .filter(|n| *n > 0 && *n <= 16 * 1024)
        .ok_or_else(invalid)?;
    let count = read_count(r, u64::try_from(stride).map_err(|_| invalid())?)?;
    read_records(r, count, stride, credit, |record| {
        let mut value = [0.; N];
        for (scalar, raw) in value.iter_mut().zip(record.chunks_exact(8)) {
            *scalar = f64::from_le_bytes(raw.try_into().map_err(|_| invalid())?);
            if !scalar.is_finite() {
                return Err(invalid());
            }
        }
        Ok(value)
    })
}
pub fn write_off_swap_payload(mesh: &OffMesh, w: &mut impl Write) -> Result<()> {
    w.write_all(MAGIC)?;
    w.write_all(&[match mesh.encoding {
        OffEncoding::Ascii => 0,
        OffEncoding::Binary => 1,
    }])?;
    w.write_all(&mesh.declared_edges.to_le_bytes())?;
    arrays(w, &mesh.positions)?;
    arrays(w, &mesh.normals)?;
    arrays(w, &mesh.colors)?;
    arrays(w, &mesh.texcoords)?;
    uint(w, mesh.indices.len())?;
    scalar_blocks(w, mesh.indices.iter().map(|value| value.to_le_bytes()))?;
    uint(w, mesh.triangles.len())?;
    scalar_blocks(
        w,
        mesh.triangles.iter().flatten().map(|value| value.to_le_bytes()),
    )?;
    uint(w, mesh.faces.len())?;
    for face in &mesh.faces {
        uint(w, face.indices.start)?;
        uint(w, face.indices.end)?;
        match &face.color {
            OffColor::Default => w.write_all(&[0])?,
            OffColor::Map(n) => {
                w.write_all(&[1])?;
                w.write_all(&n.to_le_bytes())?;
            }
            OffColor::Bytes(v) => {
                w.write_all(&[2])?;
                uint(w, v.len())?;
                w.write_all(v)?;
            }
            OffColor::Floats(v) => {
                w.write_all(&[3])?;
                uint(w, v.len())?;
                scalar_blocks(w, v.iter().map(|value| value.to_le_bytes()))?;
            }
        }
    }
    Ok(())
}
pub fn off_swap_payload_len(mesh: &OffMesh) -> Result<u64> {
    fn add(total: &mut u64, count: usize, stride: u64) -> Result<()> {
        *total = u64::try_from(count)
            .ok()
            .and_then(|count| count.checked_mul(stride))
            .and_then(|bytes| total.checked_add(bytes))
            .ok_or_else(invalid)?;
        Ok(())
    }
    // Magic, encoding, edge count, and seven vector count fields.
    let mut bytes = 13 + 7 * 8;
    for (count, stride) in [
        (mesh.positions.len(), 24),
        (mesh.normals.len(), 24),
        (mesh.colors.len(), 32),
        (mesh.texcoords.len(), 16),
        (mesh.indices.len(), 4),
        (mesh.triangles.len(), 12),
        (mesh.faces.len(), 17),
    ] {
        add(&mut bytes, count, stride)?;
    }
    for face in &mesh.faces {
        match &face.color {
            OffColor::Default => {}
            OffColor::Map(_) => add(&mut bytes, 1, 4)?,
            OffColor::Bytes(values) => {
                add(&mut bytes, 1, 8)?;
                add(&mut bytes, values.len(), 1)?;
            }
            OffColor::Floats(values) => {
                add(&mut bytes, 1, 8)?;
                add(&mut bytes, values.len(), 8)?;
            }
        }
    }
    Ok(bytes)
}
pub fn read_off_swap_payload(
    reader: &mut impl Read,
    bytes: u64,
    budget: &MemoryBudget,
) -> Result<ModelBuffer<OffMesh>> {
    let mut r = reader.take(bytes);
    let mut magic = [0; 8];
    r.read_exact(&mut magic)?;
    if &magic != MAGIC {
        return Err(invalid());
    }
    let mut tag = [0];
    r.read_exact(&mut tag)?;
    let encoding = match tag[0] {
        0 => OffEncoding::Ascii,
        1 => OffEncoding::Binary,
        _ => return Err(invalid()),
    };
    let mut word = [0; 4];
    r.read_exact(&mut word)?;
    let declared_edges = u32::from_le_bytes(word);
    let mut credit = budget.try_reserve(size_of::<OffMesh>() as u64)?;
    let positions = read_arrays(&mut r, &mut credit)?;
    let normals = read_arrays(&mut r, &mut credit)?;
    let colors: Vec<[f64; 4]> = read_arrays(&mut r, &mut credit)?;
    let texcoords = read_arrays(&mut r, &mut credit)?;
    if (!normals.is_empty() && normals.len() != positions.len())
        || (!colors.is_empty() && colors.len() != positions.len())
        || (!texcoords.is_empty() && texcoords.len() != positions.len())
        || colors.iter().flatten().any(|n| !(0.0..=1.0).contains(n))
    {
        return Err(invalid());
    }
    let count = read_count(&mut r, 4)?;
    let mut decode_index = |raw: &[u8]| -> Result<u32> {
        let index = u32::from_le_bytes(raw.try_into().map_err(|_| invalid())?);
        if usize::try_from(index).map_err(|_| invalid())? >= positions.len() {
            return Err(invalid());
        }
        Ok(index)
    };
    let indices = read_records(&mut r, count, 4, &mut credit, &mut decode_index)?;
    let count = read_count(&mut r, 12)?;
    let triangles = read_records(&mut r, count, 12, &mut credit, |record| {
        let mut triangle = [0; 3];
        for (index, raw) in triangle.iter_mut().zip(record.chunks_exact(4)) {
            *index = decode_index(raw)?;
        }
        Ok(triangle)
    })?;
    let count = read_count(&mut r, 17)?;
    let mut faces = reserve(count, &mut credit)?;
    let mut previous = 0;
    let mut expected_triangles = 0_usize;
    for _ in 0..count {
        let start = read_size(&mut r)?;
        let end = read_size(&mut r)?;
        if start != previous || end < start || end > indices.len() || !(3..=1024).contains(&(end - start)) {
            return Err(invalid());
        }
        previous = end;
        expected_triangles = expected_triangles
            .checked_add(end - start - 2)
            .ok_or_else(invalid)?;
        r.read_exact(&mut tag)?;
        let color = match tag[0] {
            0 => OffColor::Default,
            1 => {
                r.read_exact(&mut word)?;
                OffColor::Map(i32::from_le_bytes(word))
            }
            2 => {
                let n = read_size(&mut r)?;
                if !matches!(n, 3 | 4) {
                    return Err(invalid());
                }
                let mut v = reserve(n, &mut credit)?;
                for _ in 0..n {
                    r.read_exact(&mut tag)?;
                    v.push(tag[0]);
                }
                OffColor::Bytes(v)
            }
            3 => {
                let n = read_size(&mut r)?;
                if !matches!(n, 3 | 4) {
                    return Err(invalid());
                }
                let mut v = reserve(n, &mut credit)?;
                for _ in 0..n {
                    let f = f64::from_bits(read_u64(&mut r)?);
                    if !(0.0..=1.0).contains(&f) {
                        return Err(invalid());
                    }
                    v.push(f);
                }
                OffColor::Floats(v)
            }
            _ => return Err(invalid()),
        };
        faces.push(OffFace {
            indices: start..end,
            color,
        });
    }
    if previous != indices.len() || expected_triangles != triangles.len() || r.limit() != 0 {
        return Err(invalid());
    }
    let mut bounds: Option<[[f64; 3]; 2]> = None;
    for position in &positions {
        let b = bounds.get_or_insert([*position, *position]);
        for axis in 0..3 {
            b[0][axis] = b[0][axis].min(position[axis]);
            b[1][axis] = b[1][axis].max(position[axis]);
        }
    }
    Ok(ModelBuffer::from_reserved(
        OffMesh {
            encoding,
            positions,
            normals,
            colors,
            texcoords,
            indices,
            faces,
            triangles,
            bounds,
            declared_edges,
        },
        credit,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn block_writes_preserve_scalar_bits_across_boundaries() {
        struct Sink {
            bytes: Vec<u8>,
            calls: usize,
        }
        impl Write for Sink {
            fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
                self.calls += 1;
                self.bytes.extend_from_slice(bytes);
                Ok(bytes.len())
            }
            fn flush(&mut self) -> std::io::Result<()> {
                Ok(())
            }
        }
        let values = (0..1500_u32)
            .map(|n| [f64::from(n), -0.0, -f64::from(n)])
            .collect::<Vec<_>>();
        let mut sink = Sink {
            bytes: Vec::new(),
            calls: 0,
        };
        arrays(&mut sink, &values).unwrap();
        let mut expected = (values.len() as u64).to_le_bytes().to_vec();
        for value in values.iter().flatten() {
            expected.extend_from_slice(&value.to_le_bytes());
        }
        assert_eq!(sink.bytes, expected);
        assert_eq!(sink.calls, 1 + (values.len() * 24).div_ceil(4096));
        let mut empty = Sink {
            bytes: Vec::new(),
            calls: 0,
        };
        scalar_blocks(&mut empty, std::iter::empty::<[u8; 4]>()).unwrap();
        assert_eq!(empty.calls, 0);
    }
    #[test]
    fn blocked_arrays_preserve_records_across_non_aligned_boundaries() {
        struct Counter {
            data: std::io::Cursor<Vec<u8>>,
            calls: usize,
        }
        impl Read for Counter {
            fn read(&mut self, bytes: &mut [u8]) -> std::io::Result<usize> {
                self.calls += 1;
                self.data.read(bytes)
            }
        }
        let values = (0..1367_u32)
            .map(|n| [f64::from(n), -0.0, -f64::from(n)])
            .collect::<Vec<_>>();
        let mut encoded = Vec::new();
        arrays(&mut encoded, &values).unwrap();
        let mut reader = Counter {
            data: std::io::Cursor::new(encoded.clone()),
            calls: 0,
        };
        let mut input = (&mut reader).take(encoded.len() as u64);
        let root = MemoryBudget::new((values.len() * size_of::<[f64; 3]>()) as u64);
        let mut credit = root.try_reserve(0).unwrap();
        let restored = read_arrays::<3, _>(&mut input, &mut credit).unwrap();
        assert_eq!(input.limit(), 0);
        assert_eq!(reader.calls, 4); // Count plus three record-aligned blocks.
        let mut actual = Vec::new();
        arrays(&mut actual, &restored).unwrap();
        assert_eq!(actual, encoded);
        assert_eq!(root.used(), (restored.capacity() * size_of::<[f64; 3]>()) as u64);
        drop(restored);
        drop(credit);
        assert_eq!(root.used(), 0);
    }
    #[test]
    fn native_attributes_and_polygon_storage_roundtrip_exactly() {
        for name in ["off-attributes-binary.off", "off-concave-u.off"] {
            let path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("../../tests/fixtures/models")
                .join(name);
            let mesh = crate::decode_off(&crate::DecodeRequest::new(path)).unwrap();
            let mut bytes = Vec::new();
            write_off_swap_payload(&mesh, &mut bytes).unwrap();
            assert_eq!(off_swap_payload_len(&mesh).unwrap(), bytes.len() as u64);
            let root = MemoryBudget::new(1024 * 1024);
            let restored = read_off_swap_payload(&mut bytes.as_slice(), bytes.len() as u64, &root).unwrap();
            let mut encoded = Vec::new();
            write_off_swap_payload(&restored, &mut encoded).unwrap();
            assert_eq!(encoded, bytes);
            assert_eq!(restored.bounds, mesh.bounds);
            assert_eq!(restored.encoding, mesh.encoding);
            assert_eq!(restored.declared_edges, mesh.declared_edges);
            assert_eq!(restored.indices, mesh.indices);
            assert_eq!(restored.triangles, mesh.triangles);
            assert_eq!(restored.faces.len(), mesh.faces.len());
            for (actual, expected) in restored.faces.iter().zip(&mesh.faces) {
                assert_eq!(actual.indices, expected.indices);
                assert_eq!(actual.color, expected.color);
            }
            for (actual, expected) in [
                (
                    restored.positions.iter().flatten().collect::<Vec<_>>(),
                    mesh.positions.iter().flatten().collect::<Vec<_>>(),
                ),
                (
                    restored.normals.iter().flatten().collect::<Vec<_>>(),
                    mesh.normals.iter().flatten().collect::<Vec<_>>(),
                ),
                (
                    restored.colors.iter().flatten().collect::<Vec<_>>(),
                    mesh.colors.iter().flatten().collect::<Vec<_>>(),
                ),
                (
                    restored.texcoords.iter().flatten().collect::<Vec<_>>(),
                    mesh.texcoords.iter().flatten().collect::<Vec<_>>(),
                ),
            ] {
                assert_eq!(actual.len(), expected.len());
                assert!(
                    actual
                        .iter()
                        .zip(expected)
                        .all(|(a, e)| a.to_bits() == e.to_bits())
                );
            }
            let expected = crate::DecodedModel::Off(restored.clone()).capacity_bytes();
            assert_eq!(root.used(), expected);
            let clone = restored.clone();
            drop(restored);
            assert_eq!(root.used(), expected);
            drop(clone);
            assert_eq!(root.used(), 0);
            let short = MemoryBudget::new(expected - 1);
            assert!(matches!(
                read_off_swap_payload(&mut bytes.as_slice(), bytes.len() as u64, &short),
                Err(OffSwapError::Memory(_))
            ));
            assert_eq!(short.used(), 0);
            for end in 0..bytes.len() {
                assert!(read_off_swap_payload(&mut &bytes[..end], bytes.len() as u64, &root).is_err());
                assert_eq!(root.used(), 0);
            }
            let mut nonfinite = bytes.clone();
            nonfinite[21..29].copy_from_slice(&f64::NAN.to_le_bytes());
            assert!(read_off_swap_payload(&mut nonfinite.as_slice(), nonfinite.len() as u64, &root).is_err());
            assert_eq!(root.used(), 0);
            let index_start = 13
                + 4 * 8
                + mesh.positions.len() * 24
                + mesh.normals.len() * 24
                + mesh.colors.len() * 32
                + mesh.texcoords.len() * 16
                + 8;
            let mut bad_index = bytes.clone();
            bad_index[index_start..index_start + 4].copy_from_slice(&u32::MAX.to_le_bytes());
            assert!(read_off_swap_payload(&mut bad_index.as_slice(), bad_index.len() as u64, &root).is_err());
            assert_eq!(root.used(), 0);
            let mut forged = bytes.clone();
            forged[13..21].copy_from_slice(&u64::MAX.to_le_bytes());
            assert!(read_off_swap_payload(&mut forged.as_slice(), forged.len() as u64, &root).is_err());
            assert_eq!(root.used(), 0);
            let mut trailing = bytes.clone();
            trailing.push(0);
            assert!(read_off_swap_payload(&mut trailing.as_slice(), trailing.len() as u64, &root).is_err());
            assert_eq!(root.used(), 0);
        }
    }
    #[test]
    fn every_native_face_color_kind_is_preserved() {
        let mut mesh = crate::decode_off(&crate::DecodeRequest::new(
            std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("../../tests/fixtures/models/off-concave-u.off"),
        ))
        .unwrap();
        for color in [
            OffColor::Default,
            OffColor::Map(i32::MIN),
            OffColor::Bytes(vec![0, 128, 255]),
            OffColor::Bytes(vec![0, 128, 255, 42]),
            OffColor::Floats(vec![-0.0, 0.5, 1.0]),
            OffColor::Floats(vec![0.0, 0.5, 1.0, 0.25]),
        ] {
            mesh.faces[0].color = color;
            let mut bytes = Vec::new();
            write_off_swap_payload(&mesh, &mut bytes).unwrap();
            assert_eq!(off_swap_payload_len(&mesh).unwrap(), bytes.len() as u64);
            let root = MemoryBudget::new(1024 * 1024);
            let restored = read_off_swap_payload(&mut bytes.as_slice(), bytes.len() as u64, &root).unwrap();
            let mut encoded = Vec::new();
            write_off_swap_payload(&restored, &mut encoded).unwrap();
            assert_eq!(encoded, bytes);
            assert_eq!(
                root.used(),
                crate::DecodedModel::Off(restored.clone()).capacity_bytes()
            );
            drop(restored);
            assert_eq!(root.used(), 0);
        }
    }
}
