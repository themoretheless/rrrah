//! Shared model entrypoint, distinct from sensor/raster decoding.
use crate::{DecodeRequest, ObjDecodeError, ObjMesh, StlDecodeError, StlMesh};
use std::sync::Arc;
use thiserror::Error;

/// Shared mesh storage whose admission follows every clone of the allocation.
#[derive(Debug)]
pub struct ModelBuffer<T>(ModelStorage<T>);
#[derive(Debug)]
enum ModelStorage<T> {
    Legacy(Arc<T>),
    Managed(Arc<ModelAllocation<T>>),
}
#[derive(Debug)]
struct ModelAllocation<T> {
    value: T,
    _reservation: rrrah_core::Reservation,
}
impl<T> Clone for ModelBuffer<T> {
    fn clone(&self) -> Self {
        Self(match &self.0 {
            ModelStorage::Legacy(value) => ModelStorage::Legacy(value.clone()),
            ModelStorage::Managed(value) => ModelStorage::Managed(value.clone()),
        })
    }
}
impl<T> From<Arc<T>> for ModelBuffer<T> {
    fn from(value: Arc<T>) -> Self {
        Self(ModelStorage::Legacy(value))
    }
}
impl<T> std::ops::Deref for ModelBuffer<T> {
    type Target = T;
    fn deref(&self) -> &T {
        match &self.0 {
            ModelStorage::Legacy(value) => value,
            ModelStorage::Managed(value) => &value.value,
        }
    }
}
impl<T> ModelBuffer<T> {
    pub(crate) fn from_reserved(value: T, reservation: rrrah_core::Reservation) -> Self {
        Self(ModelStorage::Managed(Arc::new(ModelAllocation {
            value,
            _reservation: reservation,
        })))
    }
    pub fn is_managed(&self) -> bool {
        matches!(self.0, ModelStorage::Managed(_))
    }
    pub fn ptr_eq(&self, other: &Self) -> bool {
        match (&self.0, &other.0) {
            (ModelStorage::Legacy(a), ModelStorage::Legacy(b)) => Arc::ptr_eq(a, b),
            (ModelStorage::Managed(a), ModelStorage::Managed(b)) => Arc::ptr_eq(a, b),
            _ => false,
        }
    }
    fn try_manage(
        self,
        budget: &rrrah_core::MemoryBudget,
        bytes: u64,
    ) -> Result<Self, rrrah_core::BufferError> {
        match self.0 {
            ModelStorage::Managed(_) => Ok(self),
            ModelStorage::Legacy(value) => {
                let value = Arc::try_unwrap(value).map_err(|_| rrrah_core::BufferError::SharedOwners)?;
                let reservation = budget.try_reserve(bytes)?;
                Ok(Self(ModelStorage::Managed(Arc::new(ModelAllocation {
                    value,
                    _reservation: reservation,
                }))))
            }
        }
    }
}

#[derive(Debug, Clone)]
pub enum DecodedModel {
    Stl(ModelBuffer<StlMesh>),
    Obj(ModelBuffer<ObjMesh>),
    Ply(ModelBuffer<crate::PlyMesh>),
    Off(ModelBuffer<crate::OffMesh>),
}
#[derive(Debug, Error)]
pub enum ModelDecodeError {
    #[error(transparent)]
    Source(#[from] crate::DecodeError),
    #[error(transparent)]
    Memory(#[from] rrrah_core::BufferError),
    #[error(transparent)]
    Stl(#[from] StlDecodeError),
    #[error(transparent)]
    Obj(#[from] ObjDecodeError),
    #[error(transparent)]
    Ply(#[from] crate::PlyDecodeError),
    #[error(transparent)]
    Off(#[from] crate::OffDecodeError),
    #[error("unsupported model extension")]
    Unsupported,
}
pub fn is_supported_model_path(path: &std::path::Path) -> bool {
    path.extension().and_then(|v| v.to_str()).is_some_and(|v| {
        ["stl", "obj", "ply", "off"]
            .iter()
            .any(|ext| v.eq_ignore_ascii_case(ext))
    })
}
pub fn decode_model(request: &DecodeRequest) -> Result<DecodedModel, ModelDecodeError> {
    request.check_cancelled()?;
    if request.image_index != 0 {
        return Err(crate::DecodeError::UnsupportedImageIndex {
            index: request.image_index,
        }
        .into());
    }
    let model = decode_model_inner(request)?;
    request.check_cancelled()?;
    match &request.memory_budget {
        Some(budget) => Ok(model.try_manage(budget)?),
        None => Ok(model),
    }
}
fn decode_model_inner(request: &DecodeRequest) -> Result<DecodedModel, ModelDecodeError> {
    let extension = request
        .path
        .extension()
        .and_then(|v| v.to_str())
        .unwrap_or_default();
    if extension.eq_ignore_ascii_case("stl") {
        Ok(DecodedModel::Stl(crate::stl::decode_stl_for_model(request)?))
    } else if extension.eq_ignore_ascii_case("obj") {
        Ok(DecodedModel::Obj(crate::obj::decode_obj_for_model(request)?))
    } else if extension.eq_ignore_ascii_case("ply") {
        Ok(DecodedModel::Ply(crate::ply::decode_ply_for_model(request)?))
    } else if extension.eq_ignore_ascii_case("off") {
        Ok(DecodedModel::Off(crate::off::decode_off_for_model(request)?))
    } else {
        Err(ModelDecodeError::Unsupported)
    }
}
impl DecodedModel {
    pub fn try_manage(self, budget: &rrrah_core::MemoryBudget) -> Result<Self, rrrah_core::BufferError> {
        let bytes = self.capacity_bytes();
        Ok(match self {
            Self::Stl(mesh) => Self::Stl(mesh.try_manage(budget, bytes)?),
            Self::Obj(mesh) => Self::Obj(mesh.try_manage(budget, bytes)?),
            Self::Ply(mesh) => Self::Ply(mesh.try_manage(budget, bytes)?),
            Self::Off(mesh) => Self::Off(mesh.try_manage(budget, bytes)?),
        })
    }
    /// Conservative retained-capacity weight for cache admission.
    /// Includes vectors and nested attributes/strings, excludes allocator and
    /// Arc bookkeeping. Shared OBJ face strings are charged per reference.
    pub fn capacity_bytes(&self) -> u64 {
        fn cap<T>(capacity: usize) -> u64 {
            capacity
                .checked_mul(size_of::<T>())
                .and_then(|v| u64::try_from(v).ok())
                .unwrap_or(u64::MAX)
        }
        fn sum(values: impl IntoIterator<Item = u64>) -> u64 {
            values.into_iter().fold(0, u64::saturating_add)
        }
        fn strings(values: &Vec<String>) -> u64 {
            sum(std::iter::once(cap::<String>(values.capacity()))
                .chain(values.iter().map(|s| s.capacity() as u64)))
        }
        fn scalars(values: &crate::ply::PlyScalars) -> u64 {
            use crate::ply::PlyScalars::{F32, F64, I8, I16, I32, U8, U16, U32};
            match values {
                I8(v) => cap::<i8>(v.capacity()),
                U8(v) => cap::<u8>(v.capacity()),
                I16(v) => cap::<i16>(v.capacity()),
                U16(v) => cap::<u16>(v.capacity()),
                I32(v) => cap::<i32>(v.capacity()),
                U32(v) => cap::<u32>(v.capacity()),
                F32(v) => cap::<f32>(v.capacity()),
                F64(v) => cap::<f64>(v.capacity()),
            }
        }
        match self {
            Self::Stl(mesh) => sum([
                size_of::<StlMesh>() as u64,
                cap::<crate::stl::StlFacet>(mesh.facets.capacity()),
            ]),
            Self::Obj(mesh) => {
                let vectors = sum([
                    size_of::<ObjMesh>() as u64,
                    cap::<[f32; 4]>(mesh.positions.capacity()),
                    cap::<[f32; 3]>(mesh.texcoords.capacity()),
                    cap::<[f32; 3]>(mesh.normals.capacity()),
                    cap::<crate::obj::ObjCorner>(mesh.corners.capacity()),
                    cap::<crate::obj::ObjFace>(mesh.faces.capacity()),
                    cap::<crate::obj::ObjTriangle>(mesh.triangles.capacity()),
                    strings(&mesh.material_libraries),
                ]);
                vectors.saturating_add(sum(mesh.faces.iter().map(|face| {
                    sum([
                        face.object.len() as u64,
                        face.material.as_ref().map_or(0, |m| m.len() as u64),
                        cap::<String>(face.groups.len()),
                        sum(face.groups.iter().map(|s| s.capacity() as u64)),
                    ])
                })))
            }
            Self::Off(mesh) => sum([
                size_of::<crate::OffMesh>() as u64,
                cap::<[f64; 3]>(mesh.positions.capacity()),
                cap::<[f64; 3]>(mesh.normals.capacity()),
                cap::<[f64; 4]>(mesh.colors.capacity()),
                cap::<[f64; 2]>(mesh.texcoords.capacity()),
                cap::<u32>(mesh.indices.capacity()),
                cap::<crate::off::OffFace>(mesh.faces.capacity()),
                cap::<[u32; 3]>(mesh.triangles.capacity()),
                sum(mesh.faces.iter().map(|face| match &face.color {
                    crate::off::OffColor::Bytes(v) => cap::<u8>(v.capacity()),
                    crate::off::OffColor::Floats(v) => cap::<f64>(v.capacity()),
                    _ => 0,
                })),
            ]),
            Self::Ply(mesh) => sum([
                size_of::<crate::PlyMesh>() as u64,
                strings(&mesh.comments),
                strings(&mesh.object_info),
                cap::<crate::ply::PlyElement>(mesh.elements.capacity()),
                cap::<[u32; 3]>(mesh.triangles.capacity()),
                sum(mesh.elements.iter().map(|element| {
                    sum([
                        element.name.capacity() as u64,
                        cap::<crate::ply::PlyProperty>(element.properties.capacity()),
                        sum(element.properties.iter().map(|property| {
                            (property.name.capacity() as u64).saturating_add(match &property.values {
                                crate::ply::PlyValues::Scalar(v) => scalars(v),
                                crate::ply::PlyValues::List { offsets, values, .. } => {
                                    cap::<u32>(offsets.capacity()).saturating_add(scalars(values))
                                }
                            })
                        })),
                    ])
                })),
            ]),
        }
    }

    pub fn format_name(&self) -> &'static str {
        match self {
            Self::Stl(_) => "STL",
            Self::Obj(_) => "OBJ",
            Self::Ply(_) => "PLY",
            Self::Off(_) => "OFF",
        }
    }
    pub fn triangle_count(&self) -> usize {
        match self {
            Self::Stl(v) => v.facets.len(),
            Self::Obj(v) => v.triangles.len(),
            Self::Ply(v) => v.triangles.len(),
            Self::Off(v) => v.triangles.len(),
        }
    }
    pub fn bounds(&self) -> Option<[[f64; 3]; 2]> {
        match self {
            Self::Stl(v) => v.bounds.map(|b| b.map(|p| p.map(f64::from))),
            Self::Obj(v) => v.bounds.map(|b| b.map(|p| p.map(f64::from))),
            Self::Ply(v) => v.bounds,
            Self::Off(v) => v.bounds,
        }
    }
    /// Iterates source model storage without materializing a second triangle array.
    pub fn triangles(&self) -> impl ExactSizeIterator<Item = [[f64; 3]; 3]> + Clone + '_ {
        (0..self.triangle_count()).map(move |index| match self {
            Self::Stl(v) => v.facets[index].vertices.map(|p| p.map(f64::from)),
            Self::Obj(v) => v.triangles[index]
                .corners
                .map(|corner| v.vertex(v.corners[corner as usize].position).map(f64::from)),
            Self::Ply(v) => v.triangles[index].map(|vertex| v.vertex(vertex)),
            Self::Off(v) => v.triangles[index].map(|vertex| v.positions[vertex as usize]),
        })
    }
}

#[cfg(test)]
mod ownership_tests {
    use super::*;
    #[test]
    fn model_clone_shares_original_geometry_without_reallocation() {
        let path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../tests/fixtures/models/off-attributes-binary.off");
        let model = decode_model(&DecodeRequest::new(path)).unwrap();
        let bounds = model.bounds();
        let triangles = model.triangles().collect::<Vec<_>>();
        let clone = model.clone();
        let (DecodedModel::Off(original), DecodedModel::Off(shared)) = (&model, &clone) else {
            panic!()
        };
        assert!(original.ptr_eq(shared));
        assert_eq!(original.positions.as_ptr(), shared.positions.as_ptr());
        drop(model);
        assert_eq!(clone.bounds(), bounds);
        assert_eq!(clone.triangles().collect::<Vec<_>>(), triangles);
    }
    #[test]
    fn model_weight_counts_reserved_geometry_even_without_triangles() {
        let mesh = StlMesh {
            facets: Vec::with_capacity(8),
            bounds: None,
        };
        let expected = size_of::<StlMesh>() + mesh.facets.capacity() * size_of::<crate::stl::StlFacet>();
        let model = DecodedModel::Stl(Arc::new(mesh).into());
        assert_eq!(model.triangle_count(), 0);
        assert_eq!(model.capacity_bytes(), expected as u64);
        assert_eq!(model.clone().capacity_bytes(), model.capacity_bytes());
        let mut mesh = ObjMesh::default();
        mesh.positions = Vec::with_capacity(32);
        mesh.material_libraries
            .push("retained-material-library.mtl".into());
        let minimum = size_of::<ObjMesh>()
            + mesh.positions.capacity() * size_of::<[f32; 4]>()
            + mesh.material_libraries.capacity() * size_of::<String>()
            + mesh.material_libraries[0].capacity();
        assert!(DecodedModel::Obj(Arc::new(mesh).into()).capacity_bytes() >= minimum as u64);
    }
}

#[cfg(test)]
mod managed_tests {
    #[test]
    fn nonzero_model_index_is_refused_before_source_io() {
        for extension in ["stl", "obj", "ply", "off"] {
            for index in [1, usize::MAX] {
                let mut request =
                    crate::DecodeRequest::new(format!("/rrrah-absent-indexed-model.{extension}"));
                request.image_index = index;
                assert!(matches!(super::decode_model(&request),
                    Err(super::ModelDecodeError::Source(crate::DecodeError::UnsupportedImageIndex { index: actual }))
                    if actual == index));
            }
        }
    }
    #[test]
    fn cancelled_model_request_precedes_source_io_and_format_resolution() {
        for extension in ["stl", "obj", "ply", "off", "unknown"] {
            let mut request = crate::DecodeRequest::new(format!("/rrrah-absent-cancelled-model.{extension}"));
            request.cancellation = Some(crate::GenerationToken::new(
                std::sync::Arc::new(std::sync::atomic::AtomicU64::new(2)),
                1,
            ));
            let budget = rrrah_core::MemoryBudget::new(0);
            request.memory_budget = Some(budget.clone());
            assert!(matches!(
                super::decode_model(&request),
                Err(super::ModelDecodeError::Source(crate::DecodeError::Cancelled))
            ));
            assert_eq!(budget.peak(), 0);
        }
    }
    use super::*;
    #[test]
    fn model_kinds_preserve_geometry_and_last_owner_admission() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/models");
        for name in [
            "triangle.stl",
            "obj-negative-quad.obj",
            "ply-offset64-be.ply",
            "off-attributes-binary.off",
        ] {
            let mut request = DecodeRequest::new(root.join(name));
            let expected = decode_model(&request).unwrap();
            let bytes = expected.capacity_bytes();
            let budget =
                rrrah_core::MemoryBudget::new(bytes + std::fs::metadata(&request.path).unwrap().len());
            request.memory_budget = Some(budget.clone());
            let actual = decode_model(&request).unwrap();
            assert_eq!(actual.format_name(), expected.format_name());
            assert_eq!(actual.bounds(), expected.bounds());
            assert_eq!(
                actual.triangles().collect::<Vec<_>>(),
                expected.triangles().collect::<Vec<_>>()
            );
            assert_eq!(actual.capacity_bytes(), bytes);
            assert_eq!(budget.used(), bytes, "{name}");
            let held = actual.clone();
            match (&actual, &held) {
                (DecodedModel::Stl(a), DecodedModel::Stl(b)) => assert!(a.is_managed() && a.ptr_eq(b)),
                (DecodedModel::Obj(a), DecodedModel::Obj(b)) => assert!(a.is_managed() && a.ptr_eq(b)),
                (DecodedModel::Ply(a), DecodedModel::Ply(b)) => assert!(a.is_managed() && a.ptr_eq(b)),
                (DecodedModel::Off(a), DecodedModel::Off(b)) => assert!(a.is_managed() && a.ptr_eq(b)),
                _ => panic!("model kind changed"),
            }
            drop(actual);
            assert_eq!(budget.used(), bytes);
            drop(held);
            assert_eq!(budget.used(), 0);
            let tight = rrrah_core::MemoryBudget::new(bytes - 1);
            request.memory_budget = Some(tight.clone());
            assert!(decode_model(&request).is_err());
            assert_eq!(tight.used(), 0);
        }
    }
    #[test]
    fn shared_legacy_mesh_is_rejected_and_managed_mesh_retains_original_budget() {
        let path =
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/models/triangle.stl");
        let model = decode_model(&DecodeRequest::new(path)).unwrap();
        let held = model.clone();
        let budget = rrrah_core::MemoryBudget::new(model.capacity_bytes());
        assert!(matches!(
            model.try_manage(&budget),
            Err(rrrah_core::BufferError::SharedOwners)
        ));
        assert_eq!(budget.used(), 0);
        let managed = held.try_manage(&budget).unwrap();
        let other = rrrah_core::MemoryBudget::new(0);
        let managed = managed.try_manage(&other).unwrap();
        assert_eq!(other.used(), 0);
        drop(managed);
        assert_eq!(budget.used(), 0);
    }
}
