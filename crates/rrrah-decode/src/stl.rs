//! Native STL geometry loading. Units and coordinate axes remain source-defined.
use crate::{DecodeError, DecodeRequest, bounded_io::read_bounded};
use thiserror::Error;

const MAX_FACETS: usize = (512 * 1024 * 1024) / std::mem::size_of::<StlFacet>();

/// Source facet storage, without vertex welding or loss of sharp edges.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct StlFacet {
    pub normal: [f32; 3],
    pub vertices: [[f32; 3]; 3],
    /// Preserved raw extension field; ambiguous color conventions are not interpreted.
    pub attribute: u16,
}

#[derive(Debug, Clone)]
pub struct StlMesh {
    pub facets: Vec<StlFacet>,
    pub bounds: Option<[[f32; 3]; 2]>,
}

#[derive(Debug, Error)]
pub enum StlDecodeError {
    #[error(transparent)]
    Source(#[from] DecodeError),
    #[error("invalid STL: {0}")]
    Invalid(&'static str),
}

type Result<T> = std::result::Result<T, StlDecodeError>;

pub fn decode_stl(request: &DecodeRequest) -> Result<StlMesh> {
    request.check_cancelled()?;
    if request.image_index != 0 {
        return Err(StlDecodeError::Invalid("STL has no selectable image"));
    }
    let bytes = read_bounded(request)?;
    decode_bytes(&bytes, || request.check_cancelled().map_err(Into::into))
}

pub(crate) fn decode_stl_for_model(request: &DecodeRequest) -> Result<crate::model::ModelBuffer<StlMesh>> {
    request.check_cancelled()?;
    if request.image_index != 0 {
        return Err(StlDecodeError::Invalid("STL has no selectable image"));
    }
    let source = read_bounded(request)?;
    let count = facet_count(&source, &mut || request.check_cancelled().map_err(Into::into))?;
    let mut reservation = request
        .memory_budget
        .as_ref()
        .map(|budget| budget.try_reserve((count * size_of::<StlFacet>() + size_of::<StlMesh>()) as u64))
        .transpose()
        .map_err(DecodeError::Memory)?;
    let mesh = decode_bytes_with_count(&source, count, || request.check_cancelled().map_err(Into::into))?;
    drop(source);
    if let Some(reservation) = &mut reservation {
        reservation
            .ensure_bytes((mesh.facets.capacity() * size_of::<StlFacet>() + size_of::<StlMesh>()) as u64)
            .map_err(DecodeError::Memory)?;
    }
    Ok(match reservation {
        Some(reservation) => crate::model::ModelBuffer::from_reserved(mesh, reservation),
        None => std::sync::Arc::new(mesh).into(),
    })
}

pub(crate) fn append(mesh: &mut StlMesh, facet: StlFacet) -> Result<()> {
    let mut finite = true;
    for value in facet.normal {
        finite &= value.is_finite();
    }
    for vertex in facet.vertices {
        for value in vertex {
            finite &= value.is_finite();
        }
    }
    if !finite {
        return Err(StlDecodeError::Invalid("non-finite coordinate or normal"));
    }
    if mesh.facets.len() == MAX_FACETS {
        return Err(StlDecodeError::Invalid("facet budget exceeded"));
    }
    if mesh.facets.len() == mesh.facets.capacity() {
        return Err(StlDecodeError::Invalid("facet count exceeded preallocation"));
    }
    for vertex in facet.vertices {
        let bounds = mesh.bounds.get_or_insert([vertex, vertex]);
        for axis in 0..3 {
            bounds[0][axis] = bounds[0][axis].min(vertex[axis]);
            bounds[1][axis] = bounds[1][axis].max(vertex[axis]);
        }
    }
    mesh.facets.push(facet);
    Ok(())
}

fn facet_count(bytes: &[u8], cancel: &mut impl FnMut() -> Result<()>) -> Result<usize> {
    let binary = bytes
        .get(80..84)
        .map(|v| u32::from_le_bytes(v.try_into().unwrap()) as usize)
        .filter(|n| n.checked_mul(50).and_then(|v| v.checked_add(84)) == Some(bytes.len()));
    let count = if let Some(count) = binary {
        count
    } else {
        let text = std::str::from_utf8(bytes)
            .map_err(|_| StlDecodeError::Invalid("binary length or ASCII encoding"))?;
        let mut count = 0;
        for (index, line) in text.lines().enumerate() {
            if index % 4096 == 0 {
                cancel()?;
            }
            if line.split_whitespace().next() == Some("facet") {
                count += 1;
                if count > MAX_FACETS {
                    return Err(StlDecodeError::Invalid("facet budget exceeded"));
                }
            }
        }
        count
    };
    if count > MAX_FACETS {
        return Err(StlDecodeError::Invalid("facet budget exceeded"));
    }
    Ok(count)
}

fn decode_bytes(bytes: &[u8], mut cancel: impl FnMut() -> Result<()>) -> Result<StlMesh> {
    let count = facet_count(bytes, &mut cancel)?;
    decode_bytes_with_count(bytes, count, cancel)
}

fn decode_bytes_with_count(
    bytes: &[u8],
    count: usize,
    mut cancel: impl FnMut() -> Result<()>,
) -> Result<StlMesh> {
    let mut mesh = StlMesh {
        facets: Vec::new(),
        bounds: None,
    };
    mesh.facets
        .try_reserve_exact(count)
        .map_err(|_| StlDecodeError::Invalid("allocation failed"))?;
    // Exact binary length disambiguates headers beginning with "solid".
    let binary_count = bytes
        .get(80..84)
        .map(|v| u32::from_le_bytes(v.try_into().unwrap()) as usize);
    if let Some(count) =
        binary_count.filter(|n| n.checked_mul(50).and_then(|v| v.checked_add(84)) == Some(bytes.len()))
    {
        if count > MAX_FACETS {
            return Err(StlDecodeError::Invalid("facet budget exceeded"));
        }
        for (index, chunk) in bytes[84..].chunks_exact(50).enumerate() {
            if index % 4096 == 0 {
                cancel()?;
            }
            let mut values = [0.0; 12];
            for (value, raw) in values.iter_mut().zip(chunk[..48].chunks_exact(4)) {
                *value = f32::from_le_bytes(raw.try_into().unwrap());
            }
            append(
                &mut mesh,
                StlFacet {
                    normal: values[..3].try_into().unwrap(),
                    vertices: [
                        values[3..6].try_into().unwrap(),
                        values[6..9].try_into().unwrap(),
                        values[9..12].try_into().unwrap(),
                    ],
                    attribute: u16::from_le_bytes(chunk[48..50].try_into().unwrap()),
                },
            )?;
        }
    } else {
        let text = std::str::from_utf8(bytes)
            .map_err(|_| StlDecodeError::Invalid("binary length or ASCII encoding"))?;
        let mut lines = text.lines().map(str::trim).filter(|line| !line.is_empty());
        let first = lines.next().ok_or(StlDecodeError::Invalid("missing solid"))?;
        let name = keyword_tail(first, "solid")?;
        loop {
            cancel()?;
            let line = lines.next().ok_or(StlDecodeError::Invalid("missing endsolid"))?;
            if line.split_whitespace().next() == Some("endsolid") {
                if keyword_tail(line, "endsolid")? != name || lines.next().is_some() {
                    return Err(StlDecodeError::Invalid("solid name mismatch or trailing data"));
                }
                break;
            }
            let normal = vector(keyword_tail(keyword_tail(line, "facet")?, "normal")?)?;
            if lines
                .next()
                .map(|line| line.split_whitespace().eq(["outer", "loop"]))
                != Some(true)
            {
                return Err(StlDecodeError::Invalid("missing outer loop"));
            }
            let mut vertices = [[0.0; 3]; 3];
            for vertex in &mut vertices {
                *vertex = vector(keyword_tail(
                    lines.next().ok_or(StlDecodeError::Invalid("missing vertex"))?,
                    "vertex",
                )?)?;
            }
            if lines.next() != Some("endloop") || lines.next() != Some("endfacet") {
                return Err(StlDecodeError::Invalid("missing facet delimiter"));
            }
            append(
                &mut mesh,
                StlFacet {
                    normal,
                    vertices,
                    attribute: 0,
                },
            )?;
        }
    }
    cancel()?;
    Ok(mesh)
}

fn keyword_tail<'a>(line: &'a str, keyword: &str) -> Result<&'a str> {
    let tail = line
        .strip_prefix(keyword)
        .ok_or(StlDecodeError::Invalid("unexpected keyword"))?;
    if !tail.is_empty() && !tail.starts_with(char::is_whitespace) {
        return Err(StlDecodeError::Invalid("keyword boundary"));
    }
    Ok(tail.trim())
}

fn vector(text: &str) -> Result<[f32; 3]> {
    let mut tokens = text.split_whitespace();
    let mut values = [0.0; 3];
    for value in &mut values {
        *value = tokens
            .next()
            .and_then(|v| v.parse().ok())
            .ok_or(StlDecodeError::Invalid("invalid vector"))?;
    }
    if tokens.next().is_some() {
        return Err(StlDecodeError::Invalid("extra vector component"));
    }
    Ok(values)
}

#[cfg(test)]
mod tests {
    use super::*;
    const ASCII: &[u8] = b"solid triangle\nfacet normal 0 0 1\nouter loop\nvertex -1 0 0\nvertex 1 0 0\nvertex 0 2 0\nendloop\nendfacet\nendsolid triangle\n";
    fn binary() -> Vec<u8> {
        let mut bytes = vec![0; 80];
        bytes[..5].copy_from_slice(b"solid");
        bytes.extend_from_slice(&1_u32.to_le_bytes());
        for v in [0_f32, 0., 1., -1., 0., 0., 1., 0., 0., 0., 2., 0.] {
            bytes.extend_from_slice(&v.to_le_bytes());
        }
        bytes.extend_from_slice(&0_u16.to_le_bytes());
        bytes
    }
    #[test]
    fn binary_model_admits_before_facets_and_retains_reservation() {
        let bytes = binary();
        let path = std::env::temp_dir().join(format!("rrrah-stl-budget-{}.stl", std::process::id()));
        std::fs::write(&path, &bytes).unwrap();
        let geometry = (size_of::<StlMesh>() + size_of::<StlFacet>()) as u64;
        let total = bytes.len() as u64 + geometry;
        let mut request = DecodeRequest::new(&path);
        let tight = rrrah_core::MemoryBudget::new(total - 1);
        request.memory_budget = Some(tight.clone());
        assert!(matches!(
            crate::decode_model(&request),
            Err(crate::ModelDecodeError::Stl(StlDecodeError::Source(
                DecodeError::Memory(_)
            )))
        ));
        assert_eq!(tight.used(), 0);
        let budget = rrrah_core::MemoryBudget::new(total);
        request.memory_budget = Some(budget.clone());
        let managed = crate::decode_model(&request).unwrap();
        assert_eq!(budget.peak(), total);
        assert_eq!(budget.used(), geometry);
        assert_eq!(managed.triangle_count(), 1);
        assert_eq!(
            managed.triangles().next().unwrap(),
            [[-1., 0., 0.], [1., 0., 0.], [0., 2., 0.]]
        );
        let held = managed.clone();
        drop(managed);
        assert_eq!(budget.used(), geometry);
        drop(held);
        assert_eq!(budget.used(), 0);
        let mut corrupt = bytes;
        corrupt[84..88].copy_from_slice(&f32::NAN.to_le_bytes());
        std::fs::write(&path, corrupt).unwrap();
        request.memory_budget = Some(tight.clone());
        assert!(matches!(
            crate::decode_model(&request),
            Err(crate::ModelDecodeError::Stl(StlDecodeError::Source(
                DecodeError::Memory(_)
            )))
        ));
        request.memory_budget = Some(budget.clone());
        assert!(matches!(
            crate::decode_model(&request),
            Err(crate::ModelDecodeError::Stl(StlDecodeError::Invalid(
                "non-finite coordinate or normal"
            )))
        ));
        assert_eq!(budget.used(), 0);
        std::fs::remove_file(path).unwrap();
    }

    #[test]
    fn ascii_model_reserves_exact_facets_before_coordinate_decode() {
        let path = std::env::temp_dir().join(format!("rrrah-ascii-stl-budget-{}.stl", std::process::id()));
        std::fs::write(&path, ASCII).unwrap();
        let geometry = (size_of::<StlMesh>() + size_of::<StlFacet>()) as u64;
        let budget = rrrah_core::MemoryBudget::new(ASCII.len() as u64 + geometry);
        let mut request = DecodeRequest::new(&path);
        request.memory_budget = Some(budget.clone());
        let model = crate::decode_model(&request).unwrap();
        let crate::DecodedModel::Stl(mesh) = &model else {
            panic!()
        };
        assert_eq!(mesh.facets.capacity(), 1);
        assert!(mesh.is_managed());
        assert_eq!(budget.peak(), ASCII.len() as u64 + geometry);
        assert_eq!(budget.used(), geometry);
        drop(model);
        assert_eq!(budget.used(), 0);
        let corrupt = std::str::from_utf8(ASCII)
            .unwrap()
            .replace("normal 0", "normal NaN");
        std::fs::write(&path, &corrupt).unwrap();
        let tight = rrrah_core::MemoryBudget::new(corrupt.len() as u64 + geometry - 1);
        request.memory_budget = Some(tight.clone());
        assert!(matches!(
            crate::decode_model(&request),
            Err(crate::ModelDecodeError::Stl(StlDecodeError::Source(
                DecodeError::Memory(_)
            )))
        ));
        assert_eq!(tight.used(), 0);
        request.memory_budget = Some(rrrah_core::MemoryBudget::new(corrupt.len() as u64 + geometry));
        assert!(matches!(
            crate::decode_model(&request),
            Err(crate::ModelDecodeError::Stl(StlDecodeError::Invalid(
                "non-finite coordinate or normal"
            )))
        ));
        assert_eq!(request.memory_budget.as_ref().unwrap().used(), 0);
        assert!(matches!(
            facet_count(ASCII, &mut || Err(DecodeError::Cancelled.into())),
            Err(StlDecodeError::Source(DecodeError::Cancelled))
        ));
        std::fs::remove_file(path).unwrap();
    }

    #[test]
    fn public_file_api_and_cancellation_before_io() {
        use std::sync::{Arc, atomic::AtomicU64};
        let path = std::env::temp_dir().join(format!("rrrah-stl-{}.stl", std::process::id()));
        std::fs::write(&path, binary()).unwrap();
        let mut request = DecodeRequest::new(&path);
        assert_eq!(decode_stl(&request).unwrap().facets.len(), 1);
        std::fs::remove_file(&path).unwrap();
        request.cancellation = Some(crate::GenerationToken::new(Arc::new(AtomicU64::new(2)), 1));
        assert!(matches!(
            decode_stl(&request),
            Err(StlDecodeError::Source(DecodeError::Cancelled))
        ));
        request.cancellation = None;
        request.image_index = 1;
        assert!(matches!(decode_stl(&request), Err(StlDecodeError::Invalid(_))));
    }
    #[test]
    fn binary_and_ascii_preserve_geometry() {
        let a = decode_bytes(ASCII, || Ok(())).unwrap();
        let b = decode_bytes(&binary(), || Ok(())).unwrap();
        assert_eq!(a.facets, b.facets);
        let spaced = std::str::from_utf8(ASCII)
            .unwrap()
            .replace("outer loop", "outer\t  loop")
            .replace("facet normal", "facet\t normal");
        assert_eq!(
            decode_bytes(spaced.as_bytes(), || Ok(())).unwrap().facets,
            b.facets
        );
        assert_eq!(a.bounds, Some([[-1., 0., 0.], [1., 2., 0.]]));
    }
    #[test]
    fn rejects_truncation_nonfinite_and_trailing_data() {
        let bytes = binary();
        for end in 0..bytes.len() {
            assert!(decode_bytes(&bytes[..end], || Ok(())).is_err());
        }
        let mut invalid = bytes.clone();
        invalid[96..100].copy_from_slice(&f32::NAN.to_le_bytes());
        assert!(decode_bytes(&invalid, || Ok(())).is_err());
        invalid = bytes;
        invalid.push(0);
        assert!(decode_bytes(&invalid, || Ok(())).is_err());
        assert!(decode_bytes(&ASCII[..ASCII.len() - 5], || Ok(())).is_err());
    }
    #[test]
    fn preserves_attributes_empty_mesh_and_cancellation() {
        let mut bytes = binary();
        bytes[132..134].copy_from_slice(&0xdead_u16.to_le_bytes());
        assert_eq!(
            decode_bytes(&bytes, || Ok(())).unwrap().facets[0].attribute,
            0xdead
        );
        let empty = decode_bytes(b"solid\nendsolid\n", || Ok(())).unwrap();
        assert!(empty.facets.is_empty());
        assert!(empty.bounds.is_none());
        assert!(matches!(
            decode_bytes(&bytes, || Err(DecodeError::Cancelled.into())),
            Err(StlDecodeError::Source(DecodeError::Cancelled))
        ));
    }
}
