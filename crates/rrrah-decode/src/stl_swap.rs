//! Versioned streaming STL swap payload. Integrity belongs to the swap store.
use crate::{ModelBuffer, StlFacet, StlMesh};
use rrrah_core::{BufferError, MemoryBudget};
use std::io::{Read, Write};

const MAGIC: [u8; 8] = *b"RRSTL001";
const HEADER_BYTES: u64 = 16;
const FACET_BYTES: u64 = 50;
const MAX_STORAGE: u64 = 512 * 1024 * 1024;

#[derive(Debug, thiserror::Error)]
pub enum StlSwapError {
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    Memory(#[from] BufferError),
    #[error("invalid STL swap payload: {0}")]
    Invalid(&'static str),
}

pub fn stl_swap_payload_len(mesh: &StlMesh) -> Result<u64, StlSwapError> {
    u64::try_from(mesh.facets.len())
        .ok()
        .and_then(|count| count.checked_mul(FACET_BYTES))
        .and_then(|bytes| bytes.checked_add(HEADER_BYTES))
        .ok_or(StlSwapError::Invalid("payload length overflow"))
}

/// Writes original float bits and raw attributes without cloning geometry.
pub fn write_stl_swap_payload(mesh: &StlMesh, writer: &mut impl Write) -> Result<(), StlSwapError> {
    let _ = stl_swap_payload_len(mesh)?;
    writer.write_all(&MAGIC)?;
    writer.write_all(
        &u64::try_from(mesh.facets.len())
            .map_err(|_| StlSwapError::Invalid("facet count overflow"))?
            .to_le_bytes(),
    )?;
    for facet in &mesh.facets {
        let mut word = [0_u8; 50];
        for (index, value) in facet
            .normal
            .iter()
            .chain(facet.vertices.iter().flatten())
            .enumerate()
        {
            if !value.is_finite() {
                return Err(StlSwapError::Invalid("non-finite scalar"));
            }
            word[index * 4..index * 4 + 4].copy_from_slice(&value.to_le_bytes());
        }
        word[48..].copy_from_slice(&facet.attribute.to_le_bytes());
        writer.write_all(&word)?;
    }
    Ok(())
}

/// Restores directly into admitted final storage. `bytes` is the store's exact
/// payload length; the reader can be bounded by the caller.
pub fn read_stl_swap_payload(
    reader: &mut impl Read,
    bytes: u64,
    budget: &MemoryBudget,
) -> Result<ModelBuffer<StlMesh>, StlSwapError> {
    if bytes < HEADER_BYTES {
        return Err(StlSwapError::Invalid("short header"));
    }
    let mut header = [0_u8; 16];
    reader.read_exact(&mut header)?;
    if header[..8] != MAGIC {
        return Err(StlSwapError::Invalid("magic or version"));
    }
    let count = u64::from_le_bytes(
        header[8..]
            .try_into()
            .map_err(|_| StlSwapError::Invalid("header"))?,
    );
    if count
        .checked_mul(FACET_BYTES)
        .and_then(|n| n.checked_add(HEADER_BYTES))
        != Some(bytes)
    {
        return Err(StlSwapError::Invalid("payload length"));
    }
    let storage = count
        .checked_mul(u64::try_from(size_of::<StlFacet>()).map_err(|_| StlSwapError::Invalid("facet size"))?)
        .and_then(|n| n.checked_add(size_of::<StlMesh>() as u64))
        .filter(|n| *n <= MAX_STORAGE)
        .ok_or(StlSwapError::Invalid("storage limit"))?;
    let count = usize::try_from(count).map_err(|_| StlSwapError::Invalid("address space"))?;
    let mut reservation = budget.try_reserve(storage)?;
    let mut mesh = StlMesh {
        facets: Vec::new(),
        bounds: None,
    };
    mesh.facets
        .try_reserve_exact(count)
        .map_err(|_| StlSwapError::Invalid("allocation failed"))?;
    let actual = mesh
        .facets
        .capacity()
        .checked_mul(size_of::<StlFacet>())
        .and_then(|n| n.checked_add(size_of::<StlMesh>()))
        .and_then(|n| u64::try_from(n).ok())
        .ok_or(StlSwapError::Invalid("capacity overflow"))?;
    reservation.ensure_bytes(actual)?;
    let mut block = [0_u8; 16 * 1024];
    while mesh.facets.len() < count {
        let facets = (count - mesh.facets.len()).min(block.len() / 50);
        let bytes = facets * 50;
        reader.read_exact(&mut block[..bytes])?;
        for word in block[..bytes].chunks_exact(50) {
            let mut values = [0_f32; 12];
            for (value, raw) in values.iter_mut().zip(word[..48].chunks_exact(4)) {
                *value = f32::from_le_bytes(raw.try_into().map_err(|_| StlSwapError::Invalid("scalar"))?);
            }
            let vertices = [
                [values[3], values[4], values[5]],
                [values[6], values[7], values[8]],
                [values[9], values[10], values[11]],
            ];
            crate::stl::append(
                &mut mesh,
                StlFacet {
                    normal: [values[0], values[1], values[2]],
                    vertices,
                    attribute: u16::from_le_bytes([word[48], word[49]]),
                },
            )
            .map_err(|error| match error {
                crate::StlDecodeError::Invalid(reason) => StlSwapError::Invalid(reason),
                error => StlSwapError::Io(std::io::Error::other(error)),
            })?;
        }
    }
    Ok(ModelBuffer::from_reserved(mesh, reservation))
}

#[cfg(test)]
mod tests {
    use super::*;
    fn fixture() -> StlMesh {
        StlMesh {
            facets: vec![StlFacet {
                normal: [-0.0, 0.0, 1.0],
                vertices: [[-0.0, 2.0, 3.0], [4.0, -5.0, 6.0], [7.0, 8.0, 9.0]],
                attribute: u16::MAX,
            }],
            bounds: None,
        }
    }
    #[test]
    fn exact_bits_and_last_owner_credit() {
        let mesh = fixture();
        let mut bytes = Vec::new();
        write_stl_swap_payload(&mesh, &mut bytes).unwrap();
        assert_eq!(bytes.len() as u64, stl_swap_payload_len(&mesh).unwrap());
        let budget = MemoryBudget::new(1024);
        let restored = read_stl_swap_payload(&mut bytes.as_slice(), bytes.len() as u64, &budget).unwrap();
        assert!(restored.is_managed());
        assert_eq!(restored.facets[0].attribute, u16::MAX);
        for (a, b) in mesh.facets[0]
            .normal
            .iter()
            .chain(mesh.facets[0].vertices.iter().flatten())
            .zip(
                restored.facets[0]
                    .normal
                    .iter()
                    .chain(restored.facets[0].vertices.iter().flatten()),
            )
        {
            assert_eq!(a.to_bits(), b.to_bits());
        }
        assert_eq!(restored.bounds, Some([[-0.0, -5.0, 3.0], [7.0, 8.0, 9.0]]));
        let clone = restored.clone();
        let used = budget.used();
        assert_eq!(
            used,
            (size_of::<StlMesh>() + restored.facets.capacity() * size_of::<StlFacet>()) as u64
        );
        drop(restored);
        assert_eq!(budget.used(), used);
        drop(clone);
        assert_eq!(budget.used(), 0);
    }
    #[test]
    fn forged_counts_fail_before_admission_and_empty_mesh_is_managed() {
        let budget = MemoryBudget::new(1024);
        for count in [u64::MAX, u64::MAX / FACET_BYTES, 20_000_000] {
            let mut header = MAGIC.to_vec();
            header.extend_from_slice(&count.to_le_bytes());
            let length = count
                .checked_mul(FACET_BYTES)
                .and_then(|n| n.checked_add(HEADER_BYTES))
                .unwrap_or(u64::MAX);
            assert!(read_stl_swap_payload(&mut header.as_slice(), length, &budget).is_err());
            assert_eq!(budget.peak(), 0);
        }
        let empty = StlMesh {
            facets: Vec::new(),
            bounds: None,
        };
        let mut payload = Vec::new();
        write_stl_swap_payload(&empty, &mut payload).unwrap();
        let restored = read_stl_swap_payload(&mut payload.as_slice(), HEADER_BYTES, &budget).unwrap();
        assert!(restored.facets.is_empty());
        assert!(restored.bounds.is_none());
        assert_eq!(budget.used(), size_of::<StlMesh>() as u64);
        drop(restored);
        assert_eq!(budget.used(), 0);
    }
    #[test]
    fn facet_block_boundaries_preserve_all_bits_and_attributes() {
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
        let mut mesh = fixture();
        mesh.facets = (0..655_u16)
            .map(|attribute| {
                let mut facet = mesh.facets[0];
                facet.attribute = attribute;
                facet
            })
            .collect();
        let mut encoded = Vec::new();
        write_stl_swap_payload(&mesh, &mut encoded).unwrap();
        let mut reader = Counter {
            data: std::io::Cursor::new(encoded.clone()),
            calls: 0,
        };
        let root = MemoryBudget::new(128 * 1024);
        let restored = read_stl_swap_payload(&mut reader, encoded.len() as u64, &root).unwrap();
        assert_eq!(reader.calls, 4); // Header plus three record-aligned blocks.
        let mut actual = Vec::new();
        write_stl_swap_payload(&restored, &mut actual).unwrap();
        assert_eq!(actual, encoded);
        drop(restored);
        assert_eq!(root.used(), 0);
        for end in [16 + 327 * 50 - 1, 16 + 327 * 50, 16 + 654 * 50 - 1, 16 + 654 * 50] {
            assert!(read_stl_swap_payload(&mut &encoded[..end], encoded.len() as u64, &root).is_err());
            assert_eq!(root.used(), 0);
        }
    }
    #[test]
    fn corrupt_truncated_and_pressure_cleanup() {
        let mut bytes = Vec::new();
        write_stl_swap_payload(&fixture(), &mut bytes).unwrap();
        let budget = MemoryBudget::new(1024);
        for end in 0..bytes.len() {
            assert!(read_stl_swap_payload(&mut &bytes[..end], bytes.len() as u64, &budget).is_err());
            assert_eq!(budget.used(), 0);
        }
        assert!(read_stl_swap_payload(&mut bytes.as_slice(), bytes.len() as u64 + 1, &budget).is_err());
        let short = MemoryBudget::new((size_of::<StlMesh>() + size_of::<StlFacet>()) as u64 - 1);
        assert!(matches!(
            read_stl_swap_payload(&mut bytes.as_slice(), bytes.len() as u64, &short),
            Err(StlSwapError::Memory(_))
        ));
        assert_eq!(short.used(), 0);
        let mut invalid = bytes.clone();
        invalid[0] ^= 1;
        assert!(read_stl_swap_payload(&mut invalid.as_slice(), invalid.len() as u64, &budget).is_err());
        for index in 0..12 {
            for value in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
                let mut invalid = bytes.clone();
                invalid[16 + index * 4..20 + index * 4].copy_from_slice(&value.to_le_bytes());
                assert!(matches!(
                    read_stl_swap_payload(&mut invalid.as_slice(), invalid.len() as u64, &budget),
                    Err(StlSwapError::Invalid(_))
                ));
                assert_eq!(budget.used(), 0);
            }
        }
        let mut invalid = bytes;
        invalid[16..20].copy_from_slice(&f32::NAN.to_le_bytes());
        assert!(read_stl_swap_payload(&mut invalid.as_slice(), invalid.len() as u64, &budget).is_err());
        assert_eq!(budget.used(), 0);
    }
}
