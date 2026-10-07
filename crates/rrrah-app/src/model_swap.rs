use rrrah_cache::{SwapPayload, SwapPayloadError};
use rrrah_core::MemoryBudget;
use rrrah_decode::{DecodedModel, StlSwapError};
use std::io::{Read, Write};
fn codec_error(error: StlSwapError) -> SwapPayloadError {
    match error {
        StlSwapError::Memory(error) => SwapPayloadError::Memory(error),
        error => SwapPayloadError::Invalid(std::io::Error::other(error)),
    }
}
/// Native payload for all currently supported model variants.
#[derive(Clone)]
pub(crate) struct ModelPayload(pub(crate) DecodedModel);
impl ModelPayload {
    pub(crate) fn from_model(model: DecodedModel) -> Option<Self> {
        Some(Self(model))
    }
}
impl SwapPayload for ModelPayload {
    fn capacity_bytes(&self) -> u64 {
        self.0.capacity_bytes()
    }
    fn payload_len(&self) -> std::io::Result<u64> {
        match &self.0 {
            DecodedModel::Stl(mesh) => {
                rrrah_decode::stl_swap_payload_len(mesh).map_err(std::io::Error::other)
            }
            DecodedModel::Off(mesh) => {
                rrrah_decode::off_swap_payload_len(mesh).map_err(std::io::Error::other)
            }
            DecodedModel::Obj(mesh) => rrrah_decode::obj_swap_payload_len(mesh),
            DecodedModel::Ply(mesh) => rrrah_decode::ply_swap_payload_len(mesh),
        }
    }
    fn write_payload(&self, writer: &mut impl Write) -> std::io::Result<()> {
        match &self.0 {
            DecodedModel::Stl(mesh) => {
                rrrah_decode::write_stl_swap_payload(mesh, writer).map_err(std::io::Error::other)
            }
            DecodedModel::Off(mesh) => {
                rrrah_decode::write_off_swap_payload(mesh, writer).map_err(std::io::Error::other)
            }
            DecodedModel::Obj(mesh) => rrrah_decode::write_obj_swap_payload(mesh, writer),
            DecodedModel::Ply(mesh) => rrrah_decode::write_ply_swap_payload(mesh, writer),
        }
    }
    fn read_payload(
        reader: &mut impl Read,
        bytes: u64,
        budget: &MemoryBudget,
    ) -> Result<Self, SwapPayloadError> {
        if bytes < 8 {
            return Err(SwapPayloadError::Invalid(std::io::Error::other(
                "short model payload",
            )));
        }
        let mut magic = [0; 8];
        reader.read_exact(&mut magic).map_err(SwapPayloadError::Invalid)?;
        let mut stream = std::io::Cursor::new(magic).chain(reader);
        match &magic {
            b"RRSTL001" => rrrah_decode::read_stl_swap_payload(&mut stream, bytes, budget)
                .map(|mesh| Self(DecodedModel::Stl(mesh)))
                .map_err(codec_error),
            b"RROFF001" => rrrah_decode::read_off_swap_payload(&mut stream, bytes, budget)
                .map(|mesh| Self(DecodedModel::Off(mesh)))
                .map_err(|error| match error {
                    rrrah_decode::OffSwapError::Memory(error) => SwapPayloadError::Memory(error),
                    error => SwapPayloadError::Invalid(std::io::Error::other(error)),
                }),
            b"RROBJ001" => rrrah_decode::read_obj_swap_payload(&mut stream, bytes, budget)
                .map(|mesh| Self(DecodedModel::Obj(mesh)))
                .map_err(|error| match error {
                    rrrah_decode::ObjSwapError::Memory(error) => SwapPayloadError::Memory(error),
                    error => SwapPayloadError::Invalid(std::io::Error::other(error)),
                }),
            b"RRPLY001" => rrrah_decode::read_ply_swap_payload(&mut stream, bytes, budget)
                .map(|mesh| Self(DecodedModel::Ply(mesh)))
                .map_err(|error| match error {
                    rrrah_decode::PlySwapError::Memory(error) => SwapPayloadError::Memory(error),
                    error => SwapPayloadError::Invalid(std::io::Error::other(error)),
                }),
            _ => Err(SwapPayloadError::Invalid(std::io::Error::other(
                "model payload magic or version",
            ))),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn malformed_all_model_codecs_refuse_geometry_without_large_reservations() {
        let mut stl = b"RRSTL001".to_vec();
        stl.extend(u64::MAX.to_le_bytes());
        let mut obj = b"RROBJ001".to_vec();
        obj.extend(u64::MAX.to_le_bytes());
        let mut off = b"RROFF001".to_vec();
        off.push(0); // ASCII metadata
        off.extend(0u32.to_le_bytes());
        off.extend(u64::MAX.to_le_bytes());
        let mut ply = b"RRPLY001".to_vec();
        ply.push(0); // ASCII metadata
        ply.extend(0u64.to_le_bytes()); // vertex element
        for _ in 0..3 {
            ply.extend(0u64.to_le_bytes());
        }
        ply.extend(u64::MAX.to_le_bytes()); // comment count
        for bytes in [stl, obj, off, ply] {
            let root = MemoryBudget::new(1024 * 1024);
            assert!(matches!(
                ModelPayload::read_payload(&mut bytes.as_slice(), bytes.len() as u64, &root),
                Err(SwapPayloadError::Invalid(_))
            ));
            assert_eq!(root.used(), 0);
            assert!(
                root.peak() < 4096,
                "only bounded model bookkeeping may be reserved"
            );
            for end in 0..8 {
                let empty = MemoryBudget::new(0);
                assert!(matches!(
                    ModelPayload::read_payload(&mut &bytes[..end], end as u64, &empty),
                    Err(SwapPayloadError::Invalid(_))
                ));
                assert_eq!(empty.peak(), 0);
            }
        }
    }
}
