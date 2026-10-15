//! 3D polygon mesh decoding and managed swap serialization (OBJ, PLY, STL, OFF).
#![allow(
    clippy::pedantic,
    clippy::chunks_exact_to_as_chunks,
    clippy::collapsible_if,
    clippy::missing_errors_doc,
    clippy::cast_precision_loss,
    clippy::manual_is_multiple_of,
    clippy::manual_checked_ops,
    clippy::too_many_arguments,
    clippy::useless_conversion
)]

pub use rrrah_raw::{DecodeError, DecodeRequest, GenerationToken, bounded_io};

mod off_swap;
pub use off_swap::{OffSwapError, off_swap_payload_len, read_off_swap_payload, write_off_swap_payload};
mod off;
pub use off::{OffColor, OffDecodeError, OffEncoding, OffFace, OffMesh, decode_off};
mod ply_swap;
pub use ply_swap::{PlySwapError, ply_swap_payload_len, read_ply_swap_payload, write_ply_swap_payload};
mod ply;
pub use ply::{
    PlyDecodeError, PlyElement, PlyEncoding, PlyMesh, PlyProperty, PlyScalars, PlyType, PlyValues, decode_ply,
};
mod model;
pub use model::{DecodedModel, ModelBuffer, ModelDecodeError, decode_model, is_supported_model_path};
mod obj_swap;
pub use obj_swap::{ObjSwapError, obj_swap_payload_len, read_obj_swap_payload, write_obj_swap_payload};
mod obj;
pub use obj::{ObjCorner, ObjDecodeError, ObjFace, ObjMesh, ObjTriangle, decode_obj};
mod stl_swap;
pub use stl_swap::{StlSwapError, read_stl_swap_payload, stl_swap_payload_len, write_stl_swap_payload};
mod stl;
pub use stl::{StlDecodeError, StlFacet, StlMesh, decode_stl};
