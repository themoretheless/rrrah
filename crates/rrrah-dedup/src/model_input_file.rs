//! File-backed model input with retained source/dependency observations.
use crate::{
    animated::AnimationBudget,
    decode::{decode_selected_frame_bounded, CachedError, DecodeSourceSnapshot},
    exact::SnapshotError,
    model_input::{
        prepare_gray_model_region_input_with_projection, GrayModelInput, ModelInputError, ModelInputPolicy,
    },
    model_projection::ModelInputProjection,
    raster::NormalizedRaster,
};
use rrrah_core::MemoryBudget;
use rrrah_decode::{DecodeRequest, GenerationToken};
use std::cell::Cell;

#[derive(Debug, thiserror::Error)]
pub enum ModelFileError {
    #[error("model-file preparation cancelled")]
    Cancelled,
    #[error("invalid model-file policy")]
    Policy,
    #[error(transparent)]
    Source(#[from] SnapshotError),
    #[error(transparent)]
    Decode(Box<CachedError>),
    #[error(transparent)]
    Input(#[from] ModelInputError),
}

/// Input plus original source and external decoder dependency observations.
/// Call [`Self::verify_source`] after inference and before publishing evidence.
/// The public input is candidate data only; this type does not prove a duplicate.
#[derive(Debug)]
pub struct PreparedModelFile {
    input: GrayModelInput,
    original: [u32; 2],
    rectangle: [u32; 4],
    source: DecodeSourceSnapshot,
    cancellation: Option<GenerationToken>,
    projection: ModelInputProjection,
}
impl PreparedModelFile {
    /// Borrow prepared pixels and their original/resized domains.
    pub fn input(&self) -> &GrayModelInput {
        &self.input
    }
    /// Exact proposal projection parameters, which must be included in cache keys.
    pub fn projection(&self) -> ModelInputProjection {
        self.projection
    }
    /// Full oriented source dimensions, independent of the inference crop.
    pub fn original_dimensions(&self) -> [u32; 2] {
        self.original
    }
    /// Inference rectangle in original zero-based pixel coordinates.
    pub fn rectangle(&self) -> [u32; 4] {
        self.rectangle
    }
    /// Recheck complete content/identity of the source and decoder dependencies,
    /// and the original request generation token. Cancellation is sticky within
    /// this call. Changed/missing files never turn into an ordinary non-match.
    ///
    /// # Errors
    /// Source/dependency mutation, I/O or cancellation.
    pub fn verify_source(&self, cancel: impl Fn() -> bool) -> Result<(), ModelFileError> {
        let latch = Cell::new(false);
        let cancelled = || {
            let stop = latch.get()
                || cancel()
                || self
                    .cancellation
                    .as_ref()
                    .is_some_and(GenerationToken::is_cancelled);
            latch.set(stop);
            stop
        };
        if cancelled() {
            return Err(ModelFileError::Cancelled);
        }
        let result = self.source.verify(cancelled);
        if cancelled() {
            return Err(ModelFileError::Cancelled);
        }
        result.map_err(ModelFileError::Source)?;
        // The final caller checkpoint above may have changed a source. Recheck
        // without caller code, while still observing the retained owner token.
        let owners = || owner_cancelled(&self.cancellation, &latch);
        let verified = self.source.verify(&owners).map_err(ModelFileError::Source);
        if owners() {
            Err(ModelFileError::Cancelled)
        } else {
            verified
        }
    }
}

fn owner_cancelled(token: &Option<GenerationToken>, latch: &Cell<bool>) -> bool {
    let stopped = latch.get() || token.as_ref().is_some_and(GenerationToken::is_cancelled);
    latch.set(stopped);
    stopped
}

/// Decode and prepare managed model input under one source/dependency lifecycle.
/// Policy preflight occurs before source I/O. Source and dependency observations
/// span decoding and preparation; the result retains them for post-inference
/// verification. Global and request-generation cancellation are sticky through
/// all stages, including the final publication check. No model runtime is needed.
///
/// # Errors
/// Invalid policy, source/dependency mutation or I/O, decode/input refusal,
/// memory/work admission or cancellation. Failures retain no image payload.
pub fn prepare_gray_model_file(
    request: &DecodeRequest,
    limits: AnimationBudget,
    policy: ModelInputPolicy,
    budget: &MemoryBudget,
    cancel: impl Fn() -> bool,
) -> Result<PreparedModelFile, ModelFileError> {
    prepare_file(request, None, limits, policy, budget, &cancel)
}

/// Prepare an explicitly supplied original-pixel rectangle with the same whole
/// source/dependency guards as [`prepare_gray_model_file`]. No crop file or
/// full-size grayscale scratch is created. The full decoded source remains
/// subject to the decode/source-pixel caps; the input domain is the crop extent.
///
/// # Errors
/// Invalid rectangle/policy, source mutation, decode/input/work/memory refusal,
/// or cancellation. Original frame/page/orientation selection is preserved.
pub fn prepare_gray_model_region_file(
    request: &DecodeRequest,
    rectangle: [u32; 4],
    limits: AnimationBudget,
    policy: ModelInputPolicy,
    budget: &MemoryBudget,
    cancel: impl Fn() -> bool,
) -> Result<PreparedModelFile, ModelFileError> {
    prepare_file(request, Some(rectangle), limits, policy, budget, &cancel)
}

/// Prepare projected candidate pixels while retaining original source guards.
/// Projected equality is not original-pixel evidence. The existing rank model
/// path remains opaque SDR; use original linear RGBA verification for alpha/HDR.
/// Projection policy is checked before I/O; no full-resolution scratch is made.
///
/// # Errors
/// Invalid policy/projection, source mutation, decode/input refusal or cancellation.
pub fn prepare_gray_model_file_with_projection(
    request: &DecodeRequest,
    limits: AnimationBudget,
    policy: ModelInputPolicy,
    projection: ModelInputProjection,
    budget: &MemoryBudget,
    cancel: impl Fn() -> bool,
) -> Result<PreparedModelFile, ModelFileError> {
    prepare_file_with_projection(request, None, limits, policy, projection, budget, &cancel)
}

/// Prepare a projected original-pixel rectangle under the complete file lifecycle.
///
/// # Errors
/// Invalid rectangle/projection, source mutation, decode/input refusal or cancellation.
pub fn prepare_gray_model_region_file_with_projection(
    request: &DecodeRequest,
    rectangle: [u32; 4],
    limits: AnimationBudget,
    policy: ModelInputPolicy,
    projection: ModelInputProjection,
    budget: &MemoryBudget,
    cancel: impl Fn() -> bool,
) -> Result<PreparedModelFile, ModelFileError> {
    prepare_file_with_projection(
        request,
        Some(rectangle),
        limits,
        policy,
        projection,
        budget,
        &cancel,
    )
}

pub(crate) fn prepare_file(
    request: &DecodeRequest,
    rectangle: Option<[u32; 4]>,
    limits: AnimationBudget,
    policy: ModelInputPolicy,
    budget: &MemoryBudget,
    cancel: &dyn Fn() -> bool,
) -> Result<PreparedModelFile, ModelFileError> {
    prepare_file_with_projection(
        request,
        rectangle,
        limits,
        policy,
        ModelInputProjection::OpaqueSdr,
        budget,
        cancel,
    )
}

pub(crate) fn prepare_file_with_projection(
    request: &DecodeRequest,
    rectangle: Option<[u32; 4]>,
    limits: AnimationBudget,
    policy: ModelInputPolicy,
    projection: ModelInputProjection,
    budget: &MemoryBudget,
    cancel: &dyn Fn() -> bool,
) -> Result<PreparedModelFile, ModelFileError> {
    prepare_file_core(
        request, rectangle, limits, policy, projection, budget, cancel, false,
    )
    .map(|(input, _)| input)
}

/// Retain the same decoded originals for subsequent pixel confirmation.
#[allow(clippy::too_many_arguments)]
pub(crate) fn prepare_file_retaining_original(
    request: &DecodeRequest,
    limits: AnimationBudget,
    policy: ModelInputPolicy,
    projection: ModelInputProjection,
    budget: &MemoryBudget,
    cancel: &dyn Fn() -> bool,
) -> Result<(PreparedModelFile, NormalizedRaster), ModelFileError> {
    let (input, original) =
        prepare_file_core(request, None, limits, policy, projection, budget, cancel, true)?;
    Ok((input, original.expect("retained original requested")))
}

#[allow(clippy::too_many_arguments)]
fn prepare_file_core(
    request: &DecodeRequest,
    rectangle: Option<[u32; 4]>,
    limits: AnimationBudget,
    policy: ModelInputPolicy,
    projection: ModelInputProjection,
    budget: &MemoryBudget,
    cancel: &dyn Fn() -> bool,
    retain_original: bool,
) -> Result<(PreparedModelFile, Option<NormalizedRaster>), ModelFileError> {
    let latch = Cell::new(false);
    let cancelled = || {
        let stop = latch.get()
            || cancel()
            || request
                .cancellation
                .as_ref()
                .is_some_and(GenerationToken::is_cancelled);
        latch.set(stop);
        stop
    };
    if cancelled() {
        return Err(ModelFileError::Cancelled);
    }
    if policy.max_side < 8
        || policy.max_source_pixels == 0
        || policy.max_output_pixels == 0
        || limits.max_file_bytes == 0
        || limits.max_frames == 0
        || limits.max_pixels == 0
    {
        return Err(ModelFileError::Policy);
    }
    if policy.max_pixel_reads == 0 {
        return Err(ModelInputError::Budget.into());
    }
    if rectangle.is_some_and(|[x, y, w, h]| {
        w == 0 || h == 0 || x.checked_add(w).is_none() || y.checked_add(h).is_none()
    }) {
        return Err(ModelInputError::Invalid.into());
    }
    projection.validate()?;
    let result: Result<(PreparedModelFile, Option<NormalizedRaster>), ModelFileError> = (|| {
        let source = DecodeSourceSnapshot::read(&request.path, limits.max_file_bytes, cancelled)?;
        let limits = AnimationBudget {
            max_pixels: limits.max_pixels.min(policy.max_source_pixels),
            ..limits
        };
        let image =
            decode_selected_frame_bounded(request, limits, budget, cancelled).map_err(|e| match e {
                CachedError::Source(source) => ModelFileError::Source(source),
                other => ModelFileError::Decode(Box::new(other)),
            })?;
        let view = image
            .view(cancelled)
            .map_err(|e| ModelFileError::Decode(Box::new(CachedError::Normalize(e))))?;
        let (width, height) = view.dimensions();
        let original = [width, height];
        let rectangle = rectangle.unwrap_or([0, 0, width, height]);
        let input = prepare_gray_model_region_input_with_projection(
            &view, rectangle, policy, projection, budget, cancelled,
        )?;
        let retained = if retain_original {
            Some(image)
        } else {
            drop(image);
            None
        };
        source.verify(cancelled)?;
        Ok((
            PreparedModelFile {
                input,
                original,
                rectangle,
                source,
                cancellation: request.cancellation.clone(),
                projection,
            },
            retained,
        ))
    })();
    if cancelled() {
        return Err(ModelFileError::Cancelled);
    }
    let (prepared, original) = result?;
    let owners = || owner_cancelled(&request.cancellation, &latch);
    let verified = prepared.source.verify(&owners).map_err(ModelFileError::Source);
    if owners() {
        Err(ModelFileError::Cancelled)
    } else {
        verified?;
        Ok((prepared, original))
    }
}
