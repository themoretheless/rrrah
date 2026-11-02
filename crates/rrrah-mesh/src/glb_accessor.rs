//! Borrowed GLB embedded-buffer accessors; no expanded dense/sparse arrays.
use crate::{
    DecodeRequest,
    glb::{GlbDocument, GlbError, JsonKind, JsonValue},
};

pub const MAX_ACCESSOR_ELEMENTS: usize = 16_777_216;
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GlbComponent {
    I8,
    U8,
    I16,
    U16,
    U32,
    F32,
}
impl GlbComponent {
    fn parse(value: u64) -> Result<Self, GlbError> {
        Ok(match value {
            5120 => Self::I8,
            5121 => Self::U8,
            5122 => Self::I16,
            5123 => Self::U16,
            5125 => Self::U32,
            5126 => Self::F32,
            _ => return Err(GlbError::Invalid("accessor componentType")),
        })
    }
    fn size(self) -> usize {
        match self {
            Self::I8 | Self::U8 => 1,
            Self::I16 | Self::U16 => 2,
            Self::U32 | Self::F32 => 4,
        }
    }
    fn unsigned(self) -> bool {
        matches!(self, Self::U8 | Self::U16 | Self::U32)
    }
    fn value(self, bytes: &[u8], normalized: bool) -> Result<f64, GlbError> {
        let value = match self {
            Self::I8 => f64::from(bytes[0] as i8),
            Self::U8 => f64::from(bytes[0]),
            Self::I16 => f64::from(i16::from_le_bytes(bytes[..2].try_into().unwrap())),
            Self::U16 => f64::from(u16::from_le_bytes(bytes[..2].try_into().unwrap())),
            Self::U32 => f64::from(u32::from_le_bytes(bytes[..4].try_into().unwrap())),
            Self::F32 => f64::from(f32::from_le_bytes(bytes[..4].try_into().unwrap())),
        };
        if !value.is_finite() {
            return Err(GlbError::Invalid("non-finite accessor component"));
        }
        Ok(if normalized {
            match self {
                Self::I8 => (value / 127.).max(-1.),
                Self::U8 => value / 255.,
                Self::I16 => (value / 32767.).max(-1.),
                Self::U16 => value / 65535.,
                _ => unreachable!(),
            }
        } else {
            value
        })
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GlbShape {
    Scalar,
    Vec2,
    Vec3,
    Vec4,
    Mat2,
    Mat3,
    Mat4,
}
impl GlbShape {
    fn parse(value: JsonValue<'_>) -> Result<Self, GlbError> {
        for (name, shape) in [
            ("SCALAR", Self::Scalar),
            ("VEC2", Self::Vec2),
            ("VEC3", Self::Vec3),
            ("VEC4", Self::Vec4),
            ("MAT2", Self::Mat2),
            ("MAT3", Self::Mat3),
            ("MAT4", Self::Mat4),
        ] {
            if value.string_eq(name) {
                return Ok(shape);
            }
        }
        Err(GlbError::Invalid("accessor type"))
    }
    pub fn components(self) -> usize {
        match self {
            Self::Scalar => 1,
            Self::Vec2 => 2,
            Self::Vec3 => 3,
            Self::Vec4 | Self::Mat2 => 4,
            Self::Mat3 => 9,
            Self::Mat4 => 16,
        }
    }
    fn matrix_rows(self) -> Option<usize> {
        match self {
            Self::Mat2 => Some(2),
            Self::Mat3 => Some(3),
            Self::Mat4 => Some(4),
            _ => None,
        }
    }
    fn offset(self, component: usize, size: usize) -> usize {
        if let Some(rows) = self.matrix_rows() {
            (component / rows) * ((rows * size + 3) & !3) + (component % rows) * size
        } else {
            component * size
        }
    }
    fn extent(self, size: usize) -> usize {
        self.offset(self.components() - 1, size) + size
    }
    fn packed_stride(self, size: usize) -> usize {
        self.matrix_rows()
            .map_or(self.components() * size, |rows| rows * ((rows * size + 3) & !3))
    }
}
#[derive(Debug, Clone, Copy)]
struct View<'a> {
    bytes: &'a [u8],
    offset: usize,
    stride: Option<usize>,
    target: Option<usize>,
}
#[derive(Debug, Clone, Copy)]
struct Sparse<'a> {
    indices: &'a [u8],
    values: &'a [u8],
    count: usize,
    component: GlbComponent,
}
impl Sparse<'_> {
    fn index(self, at: usize) -> usize {
        let at = at * self.component.size();
        match self.component {
            GlbComponent::U8 => usize::from(self.indices[at]),
            GlbComponent::U16 => {
                usize::from(u16::from_le_bytes(self.indices[at..at + 2].try_into().unwrap()))
            }
            GlbComponent::U32 => u32::from_le_bytes(self.indices[at..at + 4].try_into().unwrap()) as usize,
            _ => unreachable!(),
        }
    }
    fn find(self, index: usize) -> Option<usize> {
        let (mut low, mut high) = (0, self.count);
        while low < high {
            let mid = low + (high - low) / 2;
            let value = self.index(mid);
            if value < index {
                low = mid + 1;
            } else {
                high = mid;
            }
        }
        (low < self.count && self.index(low) == index).then_some(low)
    }
}
/// Layout-validated accessor borrowing immutable source bytes. Float finiteness
/// is checked on sample access. Source metadata/min/max remain in the document;
/// their agreement with data and usage-specific geometry rules are later checks.
#[derive(Debug, Clone, Copy)]
pub struct GlbAccessor<'a> {
    base: Option<&'a [u8]>,
    sparse: Option<Sparse<'a>>,
    count: usize,
    stride: usize,
    component: GlbComponent,
    shape: GlbShape,
    normalized: bool,
}
impl GlbAccessor<'_> {
    pub fn len(self) -> usize {
        self.count
    }
    pub fn is_empty(self) -> bool {
        self.count == 0
    }
    pub fn component_type(self) -> GlbComponent {
        self.component
    }
    pub fn shape(self) -> GlbShape {
        self.shape
    }
    pub fn normalized(self) -> bool {
        self.normalized
    }
    pub fn component(self, index: usize, component: usize) -> Result<f64, GlbError> {
        if index >= self.count || component >= self.shape.components() {
            return Err(GlbError::Invalid("accessor sample index"));
        }
        let offset = self.shape.offset(component, self.component.size());
        if let Some(sparse) = self.sparse {
            if let Some(at) = sparse.find(index) {
                return self.component.value(
                    &sparse.values[at * self.shape.packed_stride(self.component.size()) + offset..],
                    self.normalized,
                );
            }
        }
        match self.base {
            Some(base) => self
                .component
                .value(&base[index * self.stride + offset..], self.normalized),
            None => Ok(0.),
        }
    }
    /// Raw unnormalized unsigned scalar. Primitive index range/restart checks
    /// belong to the geometry consumer; u32 converts to f64 without loss.
    pub fn unsigned(self, index: usize) -> Result<u32, GlbError> {
        if self.shape != GlbShape::Scalar || !self.component.unsigned() || self.normalized {
            return Err(GlbError::Invalid("unsigned index accessor type"));
        }
        Ok(self.component(index, 0)? as u32)
    }
}
fn required<'a>(
    object: JsonValue<'a>,
    name: &str,
    request: &DecodeRequest,
) -> Result<JsonValue<'a>, GlbError> {
    object
        .field(name, request)?
        .ok_or(GlbError::Invalid("missing accessor/buffer property"))
}
fn integer(value: JsonValue<'_>) -> Result<usize, GlbError> {
    value
        .as_u64()
        .and_then(|v| usize::try_from(v).ok())
        .ok_or(GlbError::Invalid("nonnegative integer property"))
}
fn optional_integer(
    object: JsonValue<'_>,
    name: &str,
    request: &DecodeRequest,
) -> Result<Option<usize>, GlbError> {
    object.field(name, request)?.map(integer).transpose()
}
fn array_item<'a>(
    object: JsonValue<'a>,
    name: &str,
    index: usize,
    request: &DecodeRequest,
) -> Result<JsonValue<'a>, GlbError> {
    let array = required(object, name, request)?;
    let array = array
        .array()
        .ok_or(GlbError::Invalid("expected accessor/buffer array"))?;
    // Check cancellation during traversal, not only once for a large nth().
    for (at, value) in array.enumerate() {
        request.check_cancelled()?;
        if at == index {
            return Ok(value);
        }
    }
    Err(GlbError::Invalid("accessor/buffer index out of range"))
}
fn fit<'a>(
    view: View<'a>,
    offset: usize,
    count: usize,
    stride: usize,
    extent: usize,
    alignment: usize,
) -> Result<&'a [u8], GlbError> {
    if offset % alignment != 0 || view.offset.checked_add(offset).is_none_or(|v| v % alignment != 0) {
        return Err(GlbError::Invalid("accessor alignment"));
    }
    let length = (count - 1)
        .checked_mul(stride)
        .and_then(|v| v.checked_add(extent))
        .ok_or(GlbError::Invalid("accessor span overflow"))?;
    let end = offset
        .checked_add(length)
        .ok_or(GlbError::Invalid("accessor span overflow"))?;
    view.bytes
        .get(offset..end)
        .ok_or(GlbError::Invalid("accessor exceeds bufferView"))
}
impl GlbDocument {
    fn embedded_buffer(&self, request: &DecodeRequest) -> Result<&[u8], GlbError> {
        let buffer = array_item(self.root(), "buffers", 0, request)?;
        if buffer.field("uri", request)?.is_some() {
            return Err(GlbError::Unsupported("external/data URI buffers"));
        }
        let length = integer(required(buffer, "byteLength", request)?)?;
        if length == 0 {
            return Err(GlbError::Invalid("empty buffer"));
        }
        let bin = self
            .source()
            .bin_bytes()
            .ok_or(GlbError::Invalid("missing BIN buffer"))?;
        if length > bin.len() || bin.len() - length > 3 || bin[length..].iter().any(|b| *b != 0) {
            return Err(GlbError::Invalid("BIN buffer length/padding"));
        }
        Ok(&bin[..length])
    }
    fn view(&self, index: usize, request: &DecodeRequest) -> Result<View<'_>, GlbError> {
        let view = array_item(self.root(), "bufferViews", index, request)?;
        if integer(required(view, "buffer", request)?)? != 0 {
            return Err(GlbError::Unsupported("non-embedded buffer"));
        }
        let offset = optional_integer(view, "byteOffset", request)?.unwrap_or(0);
        let length = integer(required(view, "byteLength", request)?)?;
        if length == 0 {
            return Err(GlbError::Invalid("empty bufferView"));
        }
        let end = offset
            .checked_add(length)
            .ok_or(GlbError::Invalid("bufferView overflow"))?;
        let bytes = self
            .embedded_buffer(request)?
            .get(offset..end)
            .ok_or(GlbError::Invalid("bufferView exceeds buffer"))?;
        let stride = optional_integer(view, "byteStride", request)?;
        if stride.is_some_and(|v| !(4..=252).contains(&v) || v % 4 != 0) {
            return Err(GlbError::Invalid("bufferView byteStride"));
        }
        let target = optional_integer(view, "target", request)?;
        if target.is_some_and(|v| v != 34962 && v != 34963) || (target == Some(34963) && stride.is_some()) {
            return Err(GlbError::Invalid("bufferView target/stride"));
        }
        Ok(View {
            bytes,
            offset,
            stride,
            target,
        })
    }
    /// Resolve one embedded GLB accessor without materializing its values.
    /// Sparse index validation is linear/cancellable; random samples use binary
    /// search. No view/sample operation creates an additional heap allocation.
    pub fn accessor(&self, index: usize, request: &DecodeRequest) -> Result<GlbAccessor<'_>, GlbError> {
        request.check_cancelled()?;
        let object = array_item(self.root(), "accessors", index, request)?;
        if object.kind() != JsonKind::Object {
            return Err(GlbError::Invalid("accessor object"));
        }
        let count = integer(required(object, "count", request)?)?;
        if count == 0 {
            return Err(GlbError::Invalid("empty accessor"));
        }
        if count > MAX_ACCESSOR_ELEMENTS {
            return Err(GlbError::Limit("accessor element count"));
        }
        let component = GlbComponent::parse(
            required(object, "componentType", request)?
                .as_u64()
                .ok_or(GlbError::Invalid("componentType integer"))?,
        )?;
        let shape = GlbShape::parse(required(object, "type", request)?)?;
        let normalized = object
            .field("normalized", request)?
            .map(|v| v.as_bool().ok_or(GlbError::Invalid("normalized boolean")))
            .transpose()?
            .unwrap_or(false);
        if normalized && matches!(component, GlbComponent::F32 | GlbComponent::U32) {
            return Err(GlbError::Invalid("normalized componentType"));
        }
        let offset = optional_integer(object, "byteOffset", request)?;
        let base_view = optional_integer(object, "bufferView", request)?;
        let packed = shape.packed_stride(component.size());
        let alignment = if shape.matrix_rows().is_some() {
            4
        } else {
            component.size()
        };
        let (base, stride) = if let Some(view) = base_view {
            let view = self.view(view, request)?;
            let stride = view.stride.unwrap_or(packed);
            if stride < packed || stride % component.size() != 0 {
                return Err(GlbError::Invalid("accessor stride"));
            }
            if view.target == Some(34962)
                && (offset.unwrap_or(0) % 4 != 0
                    || stride % 4 != 0
                    || (component.size() < 4 && view.stride.is_none()))
            {
                return Err(GlbError::Invalid("vertex attribute alignment"));
            }
            (
                Some(fit(
                    view,
                    offset.unwrap_or(0),
                    count,
                    stride,
                    shape.extent(component.size()),
                    alignment,
                )?),
                stride,
            )
        } else {
            if offset.is_some() {
                return Err(GlbError::Invalid("byteOffset without bufferView"));
            }
            (None, packed)
        };
        let sparse = if let Some(sparse) = object.field("sparse", request)? {
            let sparse_count = integer(required(sparse, "count", request)?)?;
            if sparse_count == 0 || sparse_count > count {
                return Err(GlbError::Invalid("sparse count"));
            }
            let indices = required(sparse, "indices", request)?;
            let index_component = GlbComponent::parse(
                required(indices, "componentType", request)?
                    .as_u64()
                    .ok_or(GlbError::Invalid("sparse componentType integer"))?,
            )?;
            if !index_component.unsigned() {
                return Err(GlbError::Invalid("sparse index componentType"));
            }
            let view = self.view(integer(required(indices, "bufferView", request)?)?, request)?;
            if view.stride.is_some() || view.target.is_some() {
                return Err(GlbError::Invalid("sparse bufferView stride/target"));
            }
            let index_bytes = fit(
                view,
                optional_integer(indices, "byteOffset", request)?.unwrap_or(0),
                sparse_count,
                index_component.size(),
                index_component.size(),
                index_component.size(),
            )?;
            let values = required(sparse, "values", request)?;
            let view = self.view(integer(required(values, "bufferView", request)?)?, request)?;
            if view.stride.is_some() || view.target.is_some() {
                return Err(GlbError::Invalid("sparse bufferView stride/target"));
            }
            let values = fit(
                view,
                optional_integer(values, "byteOffset", request)?.unwrap_or(0),
                sparse_count,
                packed,
                shape.extent(component.size()),
                alignment,
            )?;
            let sparse = Sparse {
                indices: index_bytes,
                values,
                count: sparse_count,
                component: index_component,
            };
            let mut previous = None;
            for at in 0..sparse_count {
                if at % 1024 == 0 {
                    request.check_cancelled()?;
                }
                let index = sparse.index(at);
                if index >= count || previous.is_some_and(|old| index <= old) {
                    return Err(GlbError::Invalid("sparse indices order/range"));
                }
                previous = Some(index);
            }
            Some(sparse)
        } else {
            None
        };
        request.check_cancelled()?;
        Ok(GlbAccessor {
            base,
            sparse,
            count,
            stride,
            component,
            shape,
            normalized,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::DecodeError;
    use rrrah_core::MemoryBudget;
    use std::sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    };
    fn fixture(name: &str) -> std::path::PathBuf {
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../tests/fixtures/glb")
            .join(name)
    }
    fn document(data: &[u8], budget: &MemoryBudget) -> GlbDocument {
        static SERIAL: AtomicU64 = AtomicU64::new(0);
        let path = std::env::temp_dir().join(format!(
            "rrrah-glb-accessor-{}-{}.glb",
            std::process::id(),
            SERIAL.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::write(&path, data).unwrap();
        let mut request = DecodeRequest::new(&path);
        request.memory_budget = Some(budget.clone());
        let result = GlbDocument::read(&request);
        std::fs::remove_file(path).unwrap();
        result.unwrap()
    }
    fn hex(text: &str) -> Vec<u8> {
        text.as_bytes()
            .chunks_exact(2)
            .map(|b| u8::from_str_radix(std::str::from_utf8(b).unwrap(), 16).unwrap())
            .collect()
    }
    #[test]
    fn original_box_and_authored_native_samples_match_independent_struct_oracles() {
        for (model, oracle, expected_samples) in [
            ("Box.glb", "Box-accessors.tsv", 180),
            ("accessor-controls.glb", "accessor-controls.tsv", 714),
        ] {
            let budget = MemoryBudget::new(1024 * 1024);
            let mut request = DecodeRequest::new(fixture(model));
            request.memory_budget = Some(budget.clone());
            let document = GlbDocument::read(&request).unwrap();
            let used = budget.used();
            let peak = budget.peak();
            let rows = std::fs::read_to_string(fixture(oracle)).unwrap();
            let mut samples = 0;
            for line in rows.lines() {
                let mut fields = line.split('\t');
                let a = fields.next().unwrap().parse().unwrap();
                let r = fields.next().unwrap().parse().unwrap();
                let c = fields.next().unwrap().parse().unwrap();
                let expected: f64 = fields.next().unwrap().parse().unwrap();
                let accessor = document.accessor(a, &request).unwrap();
                let actual = accessor.component(r, c).unwrap();
                assert_eq!(
                    actual.to_bits(),
                    expected.to_bits(),
                    "{model}: accessor{a} row{r} component{c}"
                );
                if accessor.shape() == GlbShape::Scalar
                    && accessor.component_type().unsigned()
                    && !accessor.normalized()
                {
                    assert_eq!(accessor.unsigned(r).unwrap(), expected as u32);
                }
                assert_eq!(budget.used(), used);
                assert_eq!(budget.peak(), peak);
                samples += 1;
            }
            assert_eq!(samples, expected_samples);
            assert_eq!(
                document.accessor(usize::MAX, &request).unwrap_err().to_string(),
                "invalid GLB container: accessor/buffer index out of range"
            );
            let first = document.accessor(0, &request).unwrap();
            assert!(first.component(first.len(), 0).is_err());
            assert!(first.component(0, first.shape().components()).is_err());
            drop(document);
            assert_eq!(budget.used(), 0);
        }
    }
    #[test]
    fn malformed_bounds_alignment_types_and_sparse_indices_refuse_without_new_credit() {
        let budget = MemoryBudget::new(1024 * 1024);
        let request = DecodeRequest::new("unused.glb");
        let rows = std::fs::read_to_string(fixture("accessor-invalid.tsv")).unwrap();
        let mut cases = 0;
        for line in rows.lines() {
            let (name, data) = line.split_once('\t').unwrap();
            let document = document(&hex(data), &budget);
            let used = budget.used();
            let peak = budget.peak();
            assert!(document.accessor(0, &request).is_err(), "{name}");
            assert_eq!(budget.used(), used);
            assert_eq!(budget.peak(), peak);
            drop(document);
            assert_eq!(budget.used(), 0);
            cases += 1;
        }
        assert_eq!(cases, 43);
    }
    #[test]
    fn cancelled_accessor_resolution_keeps_document_for_retry_and_releases_last_owner() {
        let generation = Arc::new(AtomicU64::new(1));
        let budget = MemoryBudget::new(4304);
        let mut request = DecodeRequest::new(fixture("Box.glb"));
        request.memory_budget = Some(budget.clone());
        request.cancellation = Some(crate::GenerationToken::new(generation.clone(), 1));
        let document = GlbDocument::read(&request).unwrap();
        generation.store(2, Ordering::Release);
        assert!(matches!(
            document.accessor(0, &request),
            Err(GlbError::Source(DecodeError::Cancelled))
        ));
        assert_eq!(budget.used(), 4304);
        request.cancellation = Some(crate::GenerationToken::new(generation, 2));
        assert_eq!(document.accessor(0, &request).unwrap().unsigned(0).unwrap(), 0);
        let clone = document.clone();
        drop(document);
        assert_eq!(budget.used(), 4304);
        drop(clone);
        assert_eq!(budget.used(), 0);
    }
    #[test]
    fn bin_zero_padding_is_not_addressable_and_decimal_integer_fields_are_accepted() {
        fn binary_container(text: &str, binary: &[u8]) -> Vec<u8> {
            let mut text = text.as_bytes().to_vec();
            while text.len() % 4 != 0 {
                text.push(b' ');
            }
            let mut binary = binary.to_vec();
            while binary.len() % 4 != 0 {
                binary.push(0);
            }
            let mut bytes = b"glTF\x02\0\0\0".to_vec();
            bytes.extend_from_slice(&((12 + 8 + text.len() + 8 + binary.len()) as u32).to_le_bytes());
            for (kind, chunk) in [(0x4e4f534au32, text), (0x004e4942u32, binary)] {
                bytes.extend_from_slice(&(chunk.len() as u32).to_le_bytes());
                bytes.extend_from_slice(&kind.to_le_bytes());
                bytes.extend_from_slice(&chunk);
            }
            bytes
        }
        let budget = MemoryBudget::new(8192);
        let request = DecodeRequest::new("unused.glb");
        for length in 1..=4 {
            let text = format!(
                r#"{{"asset":{{"version":"2.0"}},"buffers":[{{"byteLength":{length}.0}}],"bufferViews":[{{"buffer":0e0,"byteLength":{length}}}],"accessors":[{{"bufferView":0.0,"componentType":5121.0,"count":{length}e0,"type":"SCALAR"}}]}}"#
            );
            let doc = document(&binary_container(&text, &vec![123; length]), &budget);
            let accessor = doc.accessor(0, &request).unwrap();
            assert_eq!(accessor.len(), length);
            assert_eq!(accessor.unsigned(length - 1).unwrap(), 123);
            drop(doc);
            assert_eq!(budget.used(), 0);
        }
        let text = r#"{"asset":{"version":"2.0"},"buffers":[{"byteLength":1}],"bufferViews":[{"buffer":0,"byteLength":4}],"accessors":[{"bufferView":0,"componentType":5121,"count":4,"type":"SCALAR"}]}"#;
        let doc = document(&binary_container(text, &[123]), &budget);
        assert!(matches!(
            doc.accessor(0, &request),
            Err(GlbError::Invalid("bufferView exceeds buffer"))
        ));
        drop(doc);
        assert_eq!(budget.used(), 0);
    }

    #[test]
    fn nonfinite_float_samples_refuse_and_integer_api_does_not_reinterpret_attributes() {
        let original = std::fs::read(fixture("Box.glb")).unwrap();
        for bits in [
            f32::NAN.to_bits(),
            f32::INFINITY.to_bits(),
            f32::NEG_INFINITY.to_bits(),
        ] {
            let mut bad = original.clone();
            // Independent Box framing: BIN begins1016, normal accessor at0.
            bad[1016..1020].copy_from_slice(&bits.to_le_bytes());
            let budget = MemoryBudget::new(4304);
            let document = document(&bad, &budget);
            let request = DecodeRequest::new("unused.glb");
            let accessor = document.accessor(1, &request).unwrap();
            assert!(matches!(
                accessor.component(0, 0),
                Err(GlbError::Invalid("non-finite accessor component"))
            ));
            assert!(accessor.unsigned(0).is_err());
            drop(document);
            assert_eq!(budget.used(), 0);
        }
    }
}
