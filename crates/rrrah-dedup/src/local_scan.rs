//! Bounded selected-frame local candidates, with geometry and pixel residuals.
//! Candidate thresholds are caller policy, never exact content identity.
use crate::{
    animated::AnimationBudget,
    decode::DecodeSourceSnapshot as ContentSnapshot,
    decode::{CachedError, decode_selected_frame_bounded},
    exact::SnapshotError,
    geometry::{
        Correspondence, GeometryError, GeometryEvidence, GeometryPolicy, verify_similarity_candidates,
    },
    local::{LocalError, LocalPolicy, MatchPolicy, extract_multiscale_oriented, match_features},
    scan::ScanError,
    warp::{
        BidirectionalEvidence, BidirectionalPhotometricEvidence, ColorFilterPolicy, FilterPolicy,
        FilteredPhotometricEvidence, PhotometricFitFailure, PhotometricPolicy, WarpError, WarpPolicy,
        verify_bidirectional, verify_filtered_photometric_with_color, verify_photometric_bidirectional,
    },
};
use rrrah_core::MemoryBudget;
use rrrah_decode::DecodeRequest;
use std::{
    cell::Cell,
    collections::{BTreeMap, BTreeSet},
};

#[derive(Debug, Clone, Copy)]
pub struct LocalFilePolicy {
    pub decode: AnimationBudget,
    pub extract: LocalPolicy,
    pub matching: MatchPolicy,
    pub geometry: GeometryPolicy,
    pub pixels: WarpPolicy,
    pub minimum_compared_pixels: u64,
    /// Minimum overlap on each image, allowing explicitly bounded crop coverage.
    pub minimum_coverage_fraction: f64,
    pub minimum_matched_fraction: f64,
}
/// Explicit comparison signal shared by single-pair and collection searches.
#[derive(Debug, Clone, Copy)]
pub enum LocalComparisonMode {
    Strict,
    Photometric(PhotometricPolicy),
    RangePhotometric {
        photometric: PhotometricPolicy,
        range: crate::warp::FitSampleRange,
    },
    /// Display-range agreement with strict original evidence retained.
    DisplayProjection {
        photometric: PhotometricPolicy,
        range: crate::warp::FitSampleRange,
    },
    Filtered {
        photometric: PhotometricPolicy,
        filter: ColorFilterPolicy,
        fit: crate::warp::PhotometricFitMode,
    },
}
#[derive(Debug, Clone, Copy)]
pub struct LocalSearchPolicy {
    pub local: LocalFilePolicy,
    pub comparison: LocalComparisonMode,
}
impl From<LocalFilePolicy> for LocalSearchPolicy {
    fn from(local: LocalFilePolicy) -> Self {
        Self {
            local,
            comparison: LocalComparisonMode::Strict,
        }
    }
}
pub(crate) fn validate_search(policy: LocalSearchPolicy) -> Result<(), ScanError> {
    validate(policy.local).map_err(|_| ScanError::InvalidPolicy)?;
    match policy.comparison {
        LocalComparisonMode::Strict => {}
        LocalComparisonMode::Photometric(p) => {
            crate::warp::validate_photometric_policy(p).map_err(|_| ScanError::InvalidPolicy)?;
        }
        LocalComparisonMode::RangePhotometric { photometric, range }
        | LocalComparisonMode::DisplayProjection { photometric, range } => {
            crate::warp::validate_photometric_policy(photometric).map_err(|_| ScanError::InvalidPolicy)?;
            crate::warp::validate_fit_range(range).map_err(|_| ScanError::InvalidPolicy)?;
        }
        LocalComparisonMode::Filtered {
            photometric, filter, ..
        } => {
            crate::warp::validate_photometric_policy(photometric).map_err(|_| ScanError::InvalidPolicy)?;
            crate::warp::validate_filter_policy(filter.filter).map_err(|_| ScanError::InvalidPolicy)?;
        }
    }
    Ok(())
}
/// Compare selected frames using the same explicit signal policy as collection scans.
///
/// # Errors
/// Invalid policy, source changes, decode/geometry/resource errors or cancellation.
pub fn compare_local_files_with_policy(
    left: &DecodeRequest,
    right: &DecodeRequest,
    policy: LocalSearchPolicy,
    budget: &MemoryBudget,
    cancel: impl Fn() -> bool,
) -> Result<LocalFileEvidence, LocalFileError> {
    compare_local_files_selected(
        left,
        right,
        policy.local,
        comparison_signals(policy.comparison),
        None,
        budget,
        cancel,
    )
}
#[derive(Debug, thiserror::Error)]
pub enum LocalFileError {
    #[error("invalid local candidate acceptance policy")]
    InvalidPolicy,
    #[error("local file comparison cancelled")]
    Cancelled,
    #[error(transparent)]
    Source(#[from] SnapshotError),
    #[error(transparent)]
    Decode(#[from] CachedError),
    #[error(transparent)]
    Features(#[from] LocalError),
    #[error(transparent)]
    Geometry(#[from] GeometryError),
    #[error(transparent)]
    Pixels(#[from] WarpError),
}
#[derive(Debug)]
pub struct LocalFileEvidence {
    /// Original pixel coordinates; geometry inlier indices refer to this list.
    pub correspondences: Vec<Correspondence>,
    pub geometry: Option<GeometryEvidence>,
    pub pixels: Option<BidirectionalEvidence>,
    /// Optional fitted-color residuals; strict residuals remain separately exposed.
    pub photometric: Option<BidirectionalPhotometricEvidence>,
    /// Explicit fitting domain; residuals still cover all overlapping pixels.
    pub photometric_fit_range: Option<crate::warp::FitSampleRange>,
    /// True only for explicitly selected display-range projected residuals.
    pub display_projection: bool,
    /// Auxiliary linear fitting can be inconclusive while explicitly selected
    /// filtered-space verification succeeds. The reason is retained here.
    pub photometric_failure: Option<PhotometricFitFailure>,
    /// Optional box-filtered visual residuals, separately exposed from strict pixels.
    pub filtered: Option<FilteredPhotometricEvidence>,
    /// Selected-frame visual candidate only; not file/pixel/container identity.
    pub candidate: bool,
}
fn validate(policy: LocalFilePolicy) -> Result<(), LocalFileError> {
    if !policy.extract.minimum_corner_score.is_finite()
        || policy.extract.minimum_corner_score < 0.0
        || !policy.geometry.tolerance.is_finite()
        || policy.geometry.tolerance <= 0.0
        || !(policy.geometry.tolerance * policy.geometry.tolerance).is_finite()
        || policy.geometry.min_inliers < 3
        || !policy.pixels.tolerance.is_finite()
        || policy.pixels.tolerance < 0.0
        || policy.minimum_compared_pixels == 0
        || !policy.minimum_coverage_fraction.is_finite()
        || !(0.0..=1.0).contains(&policy.minimum_coverage_fraction)
        || !policy.minimum_matched_fraction.is_finite()
        || !(0.0..=1.0).contains(&policy.minimum_matched_fraction)
        || policy.minimum_matched_fraction == 0.0
    {
        return Err(LocalFileError::InvalidPolicy);
    }
    Ok(())
}
#[allow(clippy::cast_precision_loss)] // Native decoded pixel counts are bounded by decoder admission.
fn accepted(evidence: &BidirectionalEvidence, policy: LocalFilePolicy) -> bool {
    [&evidence.forward, &evidence.reverse].into_iter().all(|p| {
        p.compared_pixels >= policy.minimum_compared_pixels
            && p.compared_pixels as f64 >= p.source_pixels as f64 * policy.minimum_coverage_fraction
            && p.matched_pixels as f64 >= p.compared_pixels as f64 * policy.minimum_matched_fraction
    })
}
/// Decode, extract fixed multiscale oriented features, match, fit geometry and
/// verify both pixel grids. Both original source snapshots are revalidated after
/// the complete pair, including no-geometry/residual-rejected outcomes.
///
/// # Errors
/// Returns invalid policy, changed/invalid sources, explicit resource errors or
/// cancellation. Unknown color and unsupported RAW development are not previews.
pub fn compare_local_files(
    left: &DecodeRequest,
    right: &DecodeRequest,
    policy: LocalFilePolicy,
    budget: &MemoryBudget,
    cancel: impl Fn() -> bool,
) -> Result<LocalFileEvidence, LocalFileError> {
    compare_local_files_impl(left, right, policy, None, None, budget, cancel)
}
/// Compare with an explicitly requested global affine linear-RGB fit. Strict
/// residuals remain exposed; candidate acceptance uses fitted residuals in both
/// directions. No automatic fallback or exact-equality relaxation occurs.
///
/// # Errors
/// Same source/resource errors as strict comparison, plus degenerate or out-of-
/// bounds photometric fitting. Selected-frame candidates only.
pub fn compare_local_files_photometric(
    left: &DecodeRequest,
    right: &DecodeRequest,
    policy: LocalFilePolicy,
    photometric: PhotometricPolicy,
    budget: &MemoryBudget,
    cancel: impl Fn() -> bool,
) -> Result<LocalFileEvidence, LocalFileError> {
    compare_local_files_impl(left, right, policy, Some(photometric), None, budget, cancel)
}
/// Explicitly compare averaged corresponding windows with bounded global color
/// fitting. Strict and unfiltered fitted residuals remain in the evidence.
/// Candidate acceptance uses filtered residuals and their eroded coverage.
/// High-frequency edits can be suppressed; this is visual similarity only.
///
/// # Errors
/// Source/decode/resource/cancellation errors, invalid or degenerate fitting.
pub fn compare_local_files_filtered(
    left: &DecodeRequest,
    right: &DecodeRequest,
    policy: LocalFilePolicy,
    photometric: PhotometricPolicy,
    filter: FilterPolicy,
    budget: &MemoryBudget,
    cancel: impl Fn() -> bool,
) -> Result<LocalFileEvidence, LocalFileError> {
    compare_local_files_impl(
        left,
        right,
        policy,
        Some(photometric),
        Some((
            filter.into(),
            crate::warp::PhotometricFitMode::RejectOutsidePolicy,
        )),
        budget,
        cancel,
    )
}
/// Explicit filtered signal-space selection. Strict linear pixels and original
/// linear photometric residuals remain exposed; filtered tolerances and fitted
/// coefficients are interpreted in the selected space. No automatic fallback.
///
/// # Errors
/// Same errors as filtered comparison, with source observations and cancellation.
pub fn compare_local_files_with_color_filter(
    left: &DecodeRequest,
    right: &DecodeRequest,
    policy: LocalFilePolicy,
    photometric: PhotometricPolicy,
    filter: ColorFilterPolicy,
    budget: &MemoryBudget,
    cancel: impl Fn() -> bool,
) -> Result<LocalFileEvidence, LocalFileError> {
    compare_local_files_impl(
        left,
        right,
        policy,
        Some(photometric),
        Some((filter, crate::warp::PhotometricFitMode::RejectOutsidePolicy)),
        budget,
        cancel,
    )
}
/// Compare with the least-squares color fit inside the declared coefficient bounds.
/// Evidence records which channels required a boundary solution. Insufficient
/// samples and low variance remain inconclusive; acceptance still checks residuals.
///
/// # Errors
/// Source, resource, cancellation, invalid policy or insufficient fitting data.
pub fn compare_local_files_with_constrained_filter(
    left: &DecodeRequest,
    right: &DecodeRequest,
    policy: LocalFilePolicy,
    photometric: PhotometricPolicy,
    filter: ColorFilterPolicy,
    budget: &MemoryBudget,
    cancel: impl Fn() -> bool,
) -> Result<LocalFileEvidence, LocalFileError> {
    compare_local_files_impl(
        left,
        right,
        policy,
        Some(photometric),
        Some((filter, crate::warp::PhotometricFitMode::ConstrainedLeastSquares)),
        budget,
        cancel,
    )
}
fn accepted_photometric(p: &BidirectionalPhotometricEvidence, policy: LocalFilePolicy) -> bool {
    accepted(
        &BidirectionalEvidence {
            forward: p.forward.pixels,
            reverse: p.reverse.pixels,
        },
        policy,
    )
}
fn compare_local_files_impl(
    left: &DecodeRequest,
    right: &DecodeRequest,
    policy: LocalFilePolicy,
    photometric_policy: Option<PhotometricPolicy>,
    filter_policy: Option<(ColorFilterPolicy, crate::warp::PhotometricFitMode)>,
    budget: &MemoryBudget,
    cancel: impl Fn() -> bool,
) -> Result<LocalFileEvidence, LocalFileError> {
    compare_local_files_selected(
        left,
        right,
        policy,
        (photometric_policy, filter_policy, None, false),
        None,
        budget,
        cancel,
    )
}
type ComparisonSignals = (
    Option<PhotometricPolicy>,
    Option<(ColorFilterPolicy, crate::warp::PhotometricFitMode)>,
    Option<crate::warp::FitSampleRange>,
    bool,
);

fn comparison_signals(mode: LocalComparisonMode) -> ComparisonSignals {
    match mode {
        LocalComparisonMode::Strict => (None, None, None, false),
        LocalComparisonMode::Photometric(p) => (Some(p), None, None, false),
        LocalComparisonMode::RangePhotometric { photometric, range } => {
            (Some(photometric), None, Some(range), false)
        }
        LocalComparisonMode::DisplayProjection { photometric, range } => {
            (Some(photometric), None, Some(range), true)
        }
        LocalComparisonMode::Filtered {
            photometric,
            filter,
            fit,
        } => (Some(photometric), Some((filter, fit)), None, false),
    }
}

/// Explicit spatial feature admission. This changes selection, not descriptors
/// or acceptance thresholds; final source/pixel checks remain mandatory.
#[derive(Debug, Clone, Copy)]
pub struct SpatialFeaturePolicy {
    pub columns: usize,
    pub rows: usize,
    pub max_per_cell: usize,
}
/// Compare with spatial quotas and the same strict/fitted pixel verification.
///
/// # Errors
/// Invalid grid/search policy, resource limits, source changes or cancellation.
pub fn compare_local_files_spatial_with_policy(
    left: &DecodeRequest,
    right: &DecodeRequest,
    search: LocalSearchPolicy,
    spatial: SpatialFeaturePolicy,
    budget: &MemoryBudget,
    cancel: impl Fn() -> bool,
) -> Result<LocalFileEvidence, LocalFileError> {
    validate_spatial(spatial, search.local.extract.max_features)?;
    let signals = comparison_signals(search.comparison);
    compare_local_files_selected(left, right, search.local, signals, Some(spatial), budget, cancel)
}
pub(crate) fn validate_spatial(
    spatial: SpatialFeaturePolicy,
    max_features: usize,
) -> Result<(), LocalFileError> {
    if spatial.columns == 0 || spatial.rows == 0 || spatial.max_per_cell == 0 {
        return Err(LocalFileError::InvalidPolicy);
    }
    let cells = spatial
        .columns
        .checked_mul(spatial.rows)
        .ok_or(LocalError::Budget)?;
    if cells > max_features {
        return Err(LocalError::Budget.into());
    }
    Ok(())
}
pub(crate) fn extract_search_features(
    view: &crate::linear::LinearRgbaView<'_>,
    policy: LocalPolicy,
    spatial: Option<SpatialFeaturePolicy>,
    budget: &MemoryBudget,
    cancel: impl Fn() -> bool,
) -> Result<rrrah_core::SharedBuffer<crate::local::Feature>, LocalError> {
    extract_search_features_selected(view, policy, spatial, budget, cancel, false)
}
pub(crate) fn extract_search_features_selected(
    view: &crate::linear::LinearRgbaView<'_>,
    policy: LocalPolicy,
    spatial: Option<SpatialFeaturePolicy>,
    budget: &MemoryBudget,
    cancel: impl Fn() -> bool,
    reflected: bool,
) -> Result<rrrah_core::SharedBuffer<crate::local::Feature>, LocalError> {
    let (w, h) = view.dimensions();
    let pixels = u64::from(w) * u64::from(h);
    if pixels > policy.max_pixels {
        return Err(LocalError::Budget);
    }
    // Detector and oriented-descriptor grayscale planes are sequential. Keep
    // one reservation alive across both stages, including cancellation/errors.
    let bytes = pixels.checked_mul(8).ok_or(LocalError::Budget)?;
    let _gray = budget.try_reserve(bytes).map_err(|_| LocalError::Budget)?;

    let feature_bytes = policy
        .max_features
        .checked_mul(std::mem::size_of::<crate::local::Feature>())
        .and_then(|n| u64::try_from(n).ok())
        .ok_or(LocalError::Budget)?;
    let retained = budget
        .try_reserve(feature_bytes)
        .map_err(|_| LocalError::Budget)?;
    // Detector features and oriented output coexist during descriptor construction.
    let _feature_work = budget
        .try_reserve(feature_bytes)
        .map_err(|_| LocalError::Budget)?;
    let features = if reflected {
        if let Some(grid) = spatial {
            crate::local::extract_reflected_spatial_oriented(
                view,
                policy,
                grid.columns,
                grid.rows,
                grid.max_per_cell,
                cancel,
            )?
        } else {
            crate::local::extract_reflected_multiscale_oriented(view, policy, cancel)?
        }
    } else if let Some(grid) = spatial {
        crate::local::extract_spatial_oriented(
            view,
            policy,
            grid.columns,
            grid.rows,
            grid.max_per_cell,
            cancel,
        )?
    } else {
        extract_multiscale_oriented(view, policy, cancel)?
    };
    retained.try_adopt(features).map_err(|_| LocalError::Budget)
}
fn validate_selected_policy(
    policy: LocalFilePolicy,
    signals: ComparisonSignals,
) -> Result<(), LocalFileError> {
    let (photometric_policy, filter_policy, fit_range, _) = signals;
    if let Some(range) = fit_range {
        crate::warp::validate_fit_range(range)?;
    }
    validate(policy)?;
    if let Some(p) = photometric_policy {
        crate::warp::validate_photometric_policy(p)?;
    }
    if let Some(p) = filter_policy {
        crate::warp::validate_filter_policy(p.0.filter)?;
    }
    Ok(())
}
fn verify_selected_photometric(
    source: &crate::linear::LinearRgbaView<'_>,
    target: &crate::linear::LinearRgbaView<'_>,
    transform: crate::geometry::Transform,
    signals: ComparisonSignals,
    cancel: impl Fn() -> bool,
) -> Result<BidirectionalPhotometricEvidence, WarpError> {
    let policy = signals.0.ok_or(WarpError::Invalid)?;
    let range = signals.2;
    let display_projection = signals.3;
    if display_projection {
        let range = range.ok_or(WarpError::Invalid)?;
        crate::warp::verify_photometric_display_projection(source, target, transform, policy, range, cancel)
            .map(|value| value.projected.evidence)
    } else if let Some(range) = range {
        crate::warp::verify_photometric_bidirectional_in_range(
            source, target, transform, policy, range, cancel,
        )
        .map(|v| v.evidence)
    } else {
        verify_photometric_bidirectional(source, target, transform, policy, cancel)
    }
}
fn compare_local_files_selected(
    left: &DecodeRequest,
    right: &DecodeRequest,
    policy: LocalFilePolicy,
    signals: ComparisonSignals,
    spatial: Option<SpatialFeaturePolicy>,
    budget: &MemoryBudget,
    cancel: impl Fn() -> bool,
) -> Result<LocalFileEvidence, LocalFileError> {
    compare_local_file_views(
        left,
        right,
        policy,
        signals,
        spatial,
        budget,
        cancel,
        |av, bv, matches, cancelled| {
            let models = verify_similarity_candidates(&matches, policy.geometry, cancelled)?;
            confirm_local_models(av, bv, matches, models, policy, signals, cancelled)
        },
    )
}

#[derive(Debug)]
pub struct ProjectiveFileEvidence {
    /// Pixel-refined mapping, separate from original keypoint evidence.
    pub registered_transform: Option<crate::geometry::ProjectiveTransform>,
    /// Present only when an explicit low-pass comparison was requested.
    pub filtered: Option<crate::warp::ProjectiveFilteredEvidence>,
    pub correspondences: Vec<Correspondence>,
    pub geometry: Option<crate::geometry::ProjectiveEvidence>,
    pub pixels: Option<BidirectionalEvidence>,
    /// Selected-frame planar visual candidate, never file or exact-pixel identity.
    pub candidate: bool,
}

/// Compare selected files with local features and planar projective geometry.
/// Uses the existing source/dependency snapshots, native decode, resource limits,
/// cancellation latch and final source checks. Both pixel grids must qualify.
///
/// # Errors
/// Invalid policy (including fewer than four inliers), source changes,
/// decode/geometry/work refusal or cancellation, with no partial candidate.
pub fn compare_local_files_projective(
    left: &DecodeRequest,
    right: &DecodeRequest,
    policy: LocalFilePolicy,
    budget: &MemoryBudget,
    cancel: impl Fn() -> bool,
) -> Result<ProjectiveFileEvidence, LocalFileError> {
    if policy.geometry.min_inliers < 4 {
        return Err(LocalFileError::InvalidPolicy);
    }
    compare_local_file_views(
        left,
        right,
        policy,
        (None, None, None, false),
        None,
        budget,
        cancel,
        |av, bv, correspondences, cancelled| {
            let geometry = crate::geometry::verify_projective(&correspondences, policy.geometry, cancelled)?;
            let pixels = geometry
                .as_ref()
                .map(|model| {
                    crate::warp::verify_projective_bidirectional(
                        av,
                        bv,
                        model.transform,
                        policy.pixels,
                        cancelled,
                    )
                })
                .transpose()?;
            let candidate = pixels.as_ref().is_some_and(|evidence| accepted(evidence, policy));
            Ok(ProjectiveFileEvidence {
                registered_transform: None,
                filtered: None,
                correspondences,
                geometry,
                pixels,
                candidate,
            })
        },
    )
}

#[allow(clippy::too_many_arguments)] // Shared lifecycle preserves all comparison-mode source/resource checks.
fn compare_local_file_views<T>(
    left: &DecodeRequest,
    right: &DecodeRequest,
    policy: LocalFilePolicy,
    signals: ComparisonSignals,
    spatial: Option<SpatialFeaturePolicy>,
    budget: &MemoryBudget,
    cancel: impl Fn() -> bool,
    verify: impl FnOnce(
        &crate::linear::LinearRgbaView<'_>,
        &crate::linear::LinearRgbaView<'_>,
        Vec<Correspondence>,
        &dyn Fn() -> bool,
    ) -> Result<T, LocalFileError>,
) -> Result<T, LocalFileError> {
    compare_local_file_views_with_extractor(
        left,
        right,
        policy,
        signals,
        budget,
        cancel,
        |view, cancel| extract_search_features(view, policy.extract, spatial, budget, cancel),
        verify,
    )
}

#[allow(clippy::too_many_arguments)] // All feature recipes share the same atomic file lifecycle.
fn compare_local_file_views_with_extractor<T>(
    left: &DecodeRequest,
    right: &DecodeRequest,
    policy: LocalFilePolicy,
    signals: ComparisonSignals,
    budget: &MemoryBudget,
    cancel: impl Fn() -> bool,
    extract: impl Fn(
        &crate::linear::LinearRgbaView<'_>,
        &dyn Fn() -> bool,
    ) -> Result<rrrah_core::SharedBuffer<crate::local::Feature>, LocalError>,
    verify: impl FnOnce(
        &crate::linear::LinearRgbaView<'_>,
        &crate::linear::LinearRgbaView<'_>,
        Vec<Correspondence>,
        &dyn Fn() -> bool,
    ) -> Result<T, LocalFileError>,
) -> Result<T, LocalFileError> {
    validate_selected_policy(policy, signals)?;
    let latch = Cell::new(false);
    let cancelled = || {
        let value = latch.get()
            || cancel()
            || [left, right].iter().any(|r| {
                r.cancellation
                    .as_ref()
                    .is_some_and(rrrah_decode::GenerationToken::is_cancelled)
            });
        latch.set(value);
        value
    };
    let result = (|| {
        let first = ContentSnapshot::read(&left.path, policy.decode.max_file_bytes, cancelled)?;
        let second = ContentSnapshot::read(&right.path, policy.decode.max_file_bytes, cancelled)?;
        let a = decode_selected_frame_bounded(left, policy.decode, budget, cancelled)?;
        let b = decode_selected_frame_bounded(right, policy.decode, budget, cancelled)?;
        let av = a
            .view(cancelled)
            .map_err(crate::decode::FileError::from)
            .map_err(CachedError::from)?;
        let bv = b
            .view(cancelled)
            .map_err(crate::decode::FileError::from)
            .map_err(CachedError::from)?;
        let af = extract(&av, &cancelled)?;
        let bf = extract(&bv, &cancelled)?;
        let matches = match_features(&af, &bf, policy.matching, cancelled)?;
        let evidence = verify(&av, &bv, matches, &cancelled)?;
        first.verify(cancelled)?;
        second.verify(cancelled)?;
        Ok(evidence)
    })();
    if cancelled() {
        Err(LocalFileError::Cancelled)
    } else {
        result
    }
}
#[derive(Debug)]
pub struct LocalPair<E = LocalFileEvidence> {
    pub left: u64,
    pub right: u64,
    pub evidence: E,
}
#[derive(Debug)]
pub struct LocalFileReport<E = LocalFileEvidence> {
    /// All stable successful comparisons, including rejected candidates.
    pub pairs: Vec<LocalPair<E>>,
    pub issues: Vec<(u64, u64, LocalFileError)>,
    pub source_issues: Vec<(u64, SnapshotError)>,
}
impl<E> Default for LocalFileReport<E> {
    fn default() -> Self {
        Self {
            pairs: Vec::new(),
            issues: Vec::new(),
            source_issues: Vec::new(),
        }
    }
}
fn verify_selected_filter(
    source: &crate::linear::LinearRgbaView<'_>,
    target: &crate::linear::LinearRgbaView<'_>,
    transform: crate::geometry::Transform,
    signals: ComparisonSignals,
    cancel: impl Fn() -> bool,
) -> Result<Option<FilteredPhotometricEvidence>, WarpError> {
    signals
        .0
        .zip(signals.1)
        .map(|(policy, (filter, mode))| match mode {
            crate::warp::PhotometricFitMode::RejectOutsidePolicy => {
                verify_filtered_photometric_with_color(source, target, transform, policy, filter, &cancel)
            }
            crate::warp::PhotometricFitMode::ConstrainedLeastSquares => {
                crate::warp::verify_filtered_photometric_constrained(
                    source, target, transform, policy, filter, &cancel,
                )
            }
        })
        .transpose()
}

fn confirm_local_models(
    source: &crate::linear::LinearRgbaView<'_>,
    target: &crate::linear::LinearRgbaView<'_>,
    correspondences: Vec<Correspondence>,
    models: [Option<GeometryEvidence>; 2],
    policy: LocalFilePolicy,
    signals: ComparisonSignals,
    cancel: impl Fn() -> bool,
) -> Result<LocalFileEvidence, LocalFileError> {
    let mut geometry = None;
    let mut pixels = None;
    let mut photometric = None;
    let mut photometric_failure = None;
    let mut filtered = None;
    let mut candidate = false;
    let mut first_fit_failure = None;
    // Try the capped-residual model first, then the count-first model. All
    // accepted residuals refer to the returned model and original decoded views.
    for model in models.into_iter().rev().flatten() {
        let strict = verify_bidirectional(source, target, model.transform, policy.pixels, &cancel)?;
        let fit_result = signals
            .0
            .map(|_| verify_selected_photometric(source, target, model.transform, signals, &cancel))
            .transpose();
        let mut auxiliary_failure = None;
        let fitted = match fit_result {
            Ok(value) => value,
            Err(WarpError::Fit(reason)) if signals.1.is_some() => {
                auxiliary_failure = Some(reason);
                None
            }
            Err(WarpError::Fit(reason)) => {
                first_fit_failure.get_or_insert(reason);
                continue;
            }
            Err(error) => return Err(error.into()),
        };
        let selected_filter = match verify_selected_filter(source, target, model.transform, signals, &cancel)
        {
            Ok(value) => value,
            Err(WarpError::Fit(reason)) => {
                first_fit_failure.get_or_insert(reason);
                continue;
            }
            Err(error) => return Err(error.into()),
        };
        let accepted_model = if let Some(value) = &selected_filter {
            accepted_photometric(&value.evidence, policy)
        } else if let Some(value) = &fitted {
            accepted_photometric(value, policy)
        } else {
            accepted(&strict, policy)
        };
        if geometry.is_none() || accepted_model {
            geometry = Some(model);
            pixels = Some(strict);
            photometric = fitted;
            photometric_failure = auxiliary_failure;
            filtered = selected_filter;
        }
        if accepted_model {
            candidate = true;
            break;
        }
    }
    if !candidate && let Some(reason) = first_fit_failure {
        return Err(WarpError::Fit(reason).into());
    }
    if cancel() {
        return Err(LocalFileError::Cancelled);
    }
    Ok(LocalFileEvidence {
        correspondences,
        geometry,
        pixels,
        photometric,
        photometric_fit_range: signals.2,
        display_projection: signals.3,
        photometric_failure,
        filtered,
        candidate,
    })
}

/// Exhaustively compare bounded input pairs without a whole-image hash prefilter
/// (which can miss crops). Decode buffers are held for one pair at a time.
/// Initial/final batch snapshots discard every result involving changed sources.
/// Cost is quadratic and includes repeated decode/extraction; no scalable index
/// or whole-animation/page equivalence is claimed.
///
/// # Errors
/// Rejects duplicate ids, file/pair budgets, invalid policy or cancellation;
/// per-source/per-pair failures are retained, not interpreted as unique files.
pub fn scan_local_files(
    files: impl IntoIterator<Item = (u64, DecodeRequest)>,
    policy: LocalFilePolicy,
    max_files: usize,
    max_pairs: usize,
    budget: &MemoryBudget,
    cancel: impl Fn() -> bool,
) -> Result<LocalFileReport, ScanError> {
    scan_local_files_with_policy(files, policy.into(), max_files, max_pairs, budget, cancel)
}
/// Collection comparison with an explicit strict, fitted or filtered signal.
/// Admission validates all policies before reading sources or consuming inputs.
///
/// # Errors
/// Same admission, source-isolation and cancellation contract as `scan_local_files`.
pub fn scan_local_files_with_policy(
    files: impl IntoIterator<Item = (u64, DecodeRequest)>,
    search: LocalSearchPolicy,
    max_files: usize,
    max_pairs: usize,
    budget: &MemoryBudget,
    cancel: impl Fn() -> bool,
) -> Result<LocalFileReport, ScanError> {
    validate_search(search)?;
    let policy = search.local;
    let mut requests = BTreeMap::new();
    for (id, request) in files {
        if cancel() {
            return Err(ScanError::Cancelled);
        }
        if requests.contains_key(&id) {
            return Err(ScanError::DuplicateId);
        }
        if requests.len() >= max_files {
            return Err(ScanError::Budget);
        }
        requests.insert(id, request);
    }
    let count = requests.len();
    let pairs = count
        .checked_mul(count.saturating_sub(1))
        .ok_or(ScanError::Budget)?
        / 2;
    if pairs > max_pairs {
        return Err(ScanError::Budget);
    }
    let latch = Cell::new(false);
    let cancelled = || {
        let value = latch.get()
            || cancel()
            || requests.values().any(|r| {
                r.cancellation
                    .as_ref()
                    .is_some_and(rrrah_decode::GenerationToken::is_cancelled)
            });
        latch.set(value);
        value
    };
    let mut report = LocalFileReport::default();
    let mut snapshots = BTreeMap::new();
    for (&id, request) in &requests {
        let result = ContentSnapshot::read(&request.path, policy.decode.max_file_bytes, cancelled);
        if cancelled() {
            return Err(ScanError::Cancelled);
        }
        match result {
            Ok(source) => {
                snapshots.insert(id, source);
            }
            Err(error) => report.source_issues.push((id, error)),
        }
    }
    let ids = requests.keys().copied().collect::<Vec<_>>();
    for (index, &left) in ids.iter().enumerate() {
        for &right in &ids[index + 1..] {
            if cancelled() {
                return Err(ScanError::Cancelled);
            }
            if !snapshots.contains_key(&left) || !snapshots.contains_key(&right) {
                continue;
            }
            let result = compare_local_files_with_policy(
                &requests[&left],
                &requests[&right],
                search,
                budget,
                cancelled,
            );
            if cancelled() || matches!(result, Err(LocalFileError::Cancelled)) {
                return Err(ScanError::Cancelled);
            }
            match result {
                Ok(evidence) => report.pairs.push(LocalPair {
                    left,
                    right,
                    evidence,
                }),
                Err(error) => report.issues.push((left, right, error)),
            }
        }
    }
    let mut invalid = BTreeSet::new();
    for (id, snapshot) in snapshots {
        let result = snapshot.verify(cancelled);
        if cancelled() {
            return Err(ScanError::Cancelled);
        }
        if let Err(error) = result {
            invalid.insert(id);
            report.source_issues.push((id, error));
        }
    }
    report
        .pairs
        .retain(|pair| !invalid.contains(&pair.left) && !invalid.contains(&pair.right));
    report.source_issues.sort_by_key(|(id, _)| *id);
    if cancelled() {
        return Err(ScanError::Cancelled);
    }
    Ok(report)
}

#[derive(Debug)]
pub struct DirectoryLocalReport {
    pub files: Vec<(u64, std::path::PathBuf)>,
    pub aliases: Vec<crate::exact::Aliases>,
    pub traversal_issues: Vec<crate::exact::Issue>,
    pub local: LocalFileReport,
}
/// Discover recursive roots with the shared exact traversal policy, then search
/// selected-frame local candidates. Aliases and traversal failures remain visible.
/// Default decode requests preserve strict unknown-color behavior; use explicit
/// requests with `scan_local_files` for per-file frame/color/generation settings.
///
/// # Errors
/// Returns file/pair admission or cancellation; traversal and decode omissions
/// are diagnostics, never proof that the omitted files are unique.
pub fn scan_local_roots(
    roots: &[std::path::PathBuf],
    traversal: &crate::exact::Options,
    policy: LocalFilePolicy,
    max_files: usize,
    max_pairs: usize,
    budget: &MemoryBudget,
    cancel: impl Fn() -> bool,
) -> Result<DirectoryLocalReport, ScanError> {
    scan_local_roots_with_policy(
        roots,
        traversal,
        policy.into(),
        max_files,
        max_pairs,
        budget,
        cancel,
    )
}
/// Recursive selected-frame search using an explicitly selected comparison signal.
///
/// # Errors
/// Same traversal/source/admission diagnostics as `scan_local_roots`.
pub fn scan_local_roots_with_policy(
    roots: &[std::path::PathBuf],
    traversal: &crate::exact::Options,
    search: LocalSearchPolicy,
    max_files: usize,
    max_pairs: usize,
    budget: &MemoryBudget,
    cancel: impl Fn() -> bool,
) -> Result<DirectoryLocalReport, ScanError> {
    validate_search(search)?;
    let discovery = crate::exact::discover(roots, traversal, &cancel);
    if discovery.cancelled || cancel() {
        return Err(ScanError::Cancelled);
    }
    if discovery.files.len() > max_files {
        return Err(ScanError::Budget);
    }
    let files = discovery
        .files
        .into_iter()
        .enumerate()
        .map(|(index, path)| Ok((u64::try_from(index).map_err(|_| ScanError::Budget)?, path)))
        .collect::<Result<Vec<_>, ScanError>>()?;
    let local = scan_local_files_with_policy(
        files.iter().map(|(id, path)| (*id, DecodeRequest::new(path))),
        search,
        max_files,
        max_pairs,
        budget,
        &cancel,
    )?;
    if cancel() {
        return Err(ScanError::Cancelled);
    }
    Ok(DirectoryLocalReport {
        files,
        aliases: discovery.aliases,
        traversal_issues: discovery.issues,
        local,
    })
}

#[cfg(test)]
mod feature_memory_tests {
    use super::*;
    #[test]
    fn grayscale_admission_accounts_for_other_owners_and_releases_on_cancel() {
        let pixels = [0.5, 0.5, 0.5, 1.0].repeat(400);
        let view = crate::linear::LinearRgbaView::new(20, 20, &pixels, 400, || false).unwrap();
        let policy = LocalPolicy {
            max_pixels: 400,
            max_candidates: 400,
            max_features: 8,
            minimum_corner_score: 0.01,
        };
        let budget = MemoryBudget::new(10000);
        let owner = budget.try_reserve(7001).unwrap();
        assert!(matches!(
            extract_search_features(&view, policy, None, &budget, || false),
            Err(LocalError::Budget)
        ));
        assert_eq!(budget.used(), 7001);
        drop(owner);
        let owner = budget.try_reserve(1000).unwrap();
        let grid = SpatialFeaturePolicy {
            columns: 2,
            rows: 2,
            max_per_cell: 2,
        };
        assert!(
            extract_search_features(&view, policy, Some(grid), &budget, || false)
                .unwrap()
                .is_empty()
        );
        assert_eq!(budget.used(), 1000);
        assert!(budget.peak() >= 4200 && budget.peak() <= 10000);
        assert!(matches!(
            extract_search_features(&view, policy, Some(grid), &budget, || true),
            Err(LocalError::Cancelled)
        ));
        assert_eq!(budget.used(), 1000);
        drop(owner);
        assert_eq!(budget.used(), 0);
    }
    #[test]
    fn retained_feature_storage_keeps_credit_until_last_owner() {
        let mut seed = 1u32;
        let pixels = (0..4096)
            .flat_map(|_| {
                seed ^= seed << 13;
                seed ^= seed >> 17;
                seed ^= seed << 5;
                let v = f32::from(seed.to_le_bytes()[3]) / 255.0;
                [v, v, v, 1.0]
            })
            .collect::<Vec<_>>();
        let view = crate::linear::LinearRgbaView::new(64, 64, &pixels, 4096, || false).unwrap();
        let policy = LocalPolicy {
            max_pixels: 4096,
            max_candidates: 4096,
            max_features: 32,
            minimum_corner_score: 0.0001,
        };
        let budget = MemoryBudget::new(1024 * 1024);
        let features = extract_search_features(&view, policy, None, &budget, || false).unwrap();
        assert!(!features.is_empty());
        assert!(features.capacity_bytes() > 0);
        let retained = budget.used();
        assert!(
            retained >= u64::try_from(features.len() * std::mem::size_of::<crate::local::Feature>()).unwrap()
        );
        let shared = features.clone();
        drop(features);
        assert_eq!(budget.used(), retained);
        drop(shared);
        assert_eq!(budget.used(), 0);
    }
}

/// Selected-frame reflected candidate evidence. It never proves file identity.
#[derive(Debug)]
pub struct ReflectedLocalFileEvidence {
    pub correspondences: Vec<Correspondence>,
    pub geometry: Option<crate::geometry::ReflectedGeometryEvidence>,
    pub pixels: Option<BidirectionalEvidence>,
    /// Optional fitted-color residuals; original strict pixel residuals retained.
    pub photometric: Option<BidirectionalPhotometricEvidence>,
    pub photometric_fit_range: Option<crate::warp::FitSampleRange>,
    pub display_projection: bool,
    /// Auxiliary linear-fit failure when selected filtering remains qualified.
    pub photometric_failure: Option<PhotometricFitFailure>,
    /// Explicit filtered residuals; strict original pixels remain separately exposed.
    pub filtered: Option<FilteredPhotometricEvidence>,
    pub candidate: bool,
}

pub(crate) type ReflectedSignals = ComparisonSignals;

pub(crate) fn reflected_signals(mode: LocalComparisonMode) -> ReflectedSignals {
    comparison_signals(mode)
}

/// Reflected selected-frame comparison using an explicit strict, photometric
/// or filtered policy and optional spatial quotas on both images. Filtering
/// retains original strict evidence and any auxiliary linear fitting failure.
/// Range fitting and display projection are explicit modes, never pixel identity.
///
/// # Errors
/// Invalid/unsupported policy or grid before source access, source/decode/resource
/// failures, inconclusive fitting, or latched cancellation without evidence.
pub fn compare_reflected_local_files_with_policy(
    left: &DecodeRequest,
    right: &DecodeRequest,
    policy: LocalSearchPolicy,
    spatial: Option<SpatialFeaturePolicy>,
    budget: &MemoryBudget,
    cancel: impl Fn() -> bool,
) -> Result<ReflectedLocalFileEvidence, LocalFileError> {
    if let Some(grid) = spatial {
        validate_spatial(grid, policy.local.extract.max_features)?;
    }
    compare_reflected_local_files_with_signals(
        left,
        right,
        policy.local,
        spatial,
        reflected_signals(policy.comparison),
        budget,
        cancel,
    )
}

/// Compare a reflected selected-frame pair with strict scene-linear residuals.
/// Uses managed multiscale features and revalidates both content snapshots.
/// Reflection is explicit and uses the same acceptance thresholds as ordinary
/// strict local comparison. At most two geometric models share one bounded
/// hypothesis search and are independently confirmed against the original
/// pixels; color fitting and filtering use explicit policy entrypoints.
///
/// # Errors
/// Invalid policy, resource/decode/source errors or cancellation without evidence.
pub fn compare_reflected_local_files(
    left: &DecodeRequest,
    right: &DecodeRequest,
    policy: LocalFilePolicy,
    budget: &MemoryBudget,
    cancel: impl Fn() -> bool,
) -> Result<ReflectedLocalFileEvidence, LocalFileError> {
    compare_reflected_local_files_selected(left, right, policy, None, None, budget, cancel)
}

/// Reflected strict file comparison with spatial feature quotas on both images.
///
/// # Errors
/// Invalid grid/policy, source/decode/resource failure or cancellation.
pub fn compare_reflected_local_files_spatial(
    left: &DecodeRequest,
    right: &DecodeRequest,
    policy: LocalFilePolicy,
    spatial: SpatialFeaturePolicy,
    budget: &MemoryBudget,
    cancel: impl Fn() -> bool,
) -> Result<ReflectedLocalFileEvidence, LocalFileError> {
    validate_spatial(spatial, policy.extract.max_features)?;
    compare_reflected_local_files_selected(left, right, policy, Some(spatial), None, budget, cancel)
}
pub(crate) fn compare_reflected_local_files_selected(
    left: &DecodeRequest,
    right: &DecodeRequest,
    policy: LocalFilePolicy,
    spatial: Option<SpatialFeaturePolicy>,
    photometric_policy: Option<PhotometricPolicy>,
    budget: &MemoryBudget,
    cancel: impl Fn() -> bool,
) -> Result<ReflectedLocalFileEvidence, LocalFileError> {
    compare_reflected_local_files_with_signals(
        left,
        right,
        policy,
        spatial,
        (photometric_policy, None, None, false),
        budget,
        cancel,
    )
}

pub(crate) fn compare_reflected_local_files_with_signals(
    left: &DecodeRequest,
    right: &DecodeRequest,
    policy: LocalFilePolicy,
    spatial: Option<SpatialFeaturePolicy>,
    signals: ReflectedSignals,
    budget: &MemoryBudget,
    cancel: impl Fn() -> bool,
) -> Result<ReflectedLocalFileEvidence, LocalFileError> {
    let (photometric_policy, filter_policy, fit_range, _) = signals;
    validate(policy)?;
    if let Some(range) = fit_range {
        crate::warp::validate_fit_range(range)?;
    }
    if let Some(fit) = photometric_policy {
        crate::warp::validate_photometric_policy(fit)?;
    }
    if let Some((filter, _)) = filter_policy {
        if photometric_policy.is_none() {
            return Err(LocalFileError::InvalidPolicy);
        }
        crate::warp::validate_filter_policy(filter.filter)?;
    }

    let latch = Cell::new(false);
    let cancelled = || {
        let value = latch.get()
            || cancel()
            || [left, right].iter().any(|request| {
                request
                    .cancellation
                    .as_ref()
                    .is_some_and(rrrah_decode::GenerationToken::is_cancelled)
            });
        latch.set(value);
        value
    };
    let result = (|| {
        let first = ContentSnapshot::read(&left.path, policy.decode.max_file_bytes, cancelled)?;
        let second = ContentSnapshot::read(&right.path, policy.decode.max_file_bytes, cancelled)?;
        let a = decode_selected_frame_bounded(left, policy.decode, budget, cancelled)?;
        let b = decode_selected_frame_bounded(right, policy.decode, budget, cancelled)?;
        let av = a
            .view(cancelled)
            .map_err(crate::decode::FileError::from)
            .map_err(CachedError::from)?;
        let bv = b
            .view(cancelled)
            .map_err(crate::decode::FileError::from)
            .map_err(CachedError::from)?;
        let af = extract_search_features(&av, policy.extract, spatial, budget, cancelled)?;
        let bf = extract_search_features_selected(&bv, policy.extract, spatial, budget, cancelled, true)?;
        let correspondences = match_features(&af, &bf, policy.matching, cancelled)?;
        let models = crate::geometry::verify_reflected_similarity_candidates(
            &correspondences,
            policy.geometry,
            cancelled,
        )?;
        let evidence = confirm_reflected_models_with_signals(
            &av,
            &bv,
            correspondences,
            models,
            policy,
            signals,
            cancelled,
        )?;
        first.verify(cancelled)?;
        second.verify(cancelled)?;
        Ok(evidence)
    })();
    if cancelled() {
        Err(LocalFileError::Cancelled)
    } else {
        result
    }
}

// Source observations are checked by the file caller after this bounded
// confirmation. Fitting failures can be specific to one model; resource,
// invalid-input and cancellation errors abort without trying another model.
#[cfg(test)]
fn confirm_reflected_models(
    source: &crate::linear::LinearRgbaView<'_>,
    target: &crate::linear::LinearRgbaView<'_>,
    correspondences: Vec<Correspondence>,
    models: [Option<crate::geometry::ReflectedGeometryEvidence>; 2],
    policy: LocalFilePolicy,
    photometric_policy: Option<PhotometricPolicy>,
    cancel: impl Fn() -> bool,
) -> Result<ReflectedLocalFileEvidence, LocalFileError> {
    confirm_reflected_models_with_signals(
        source,
        target,
        correspondences,
        models,
        policy,
        (photometric_policy, None, None, false),
        cancel,
    )
}

fn verify_reflected_filter(
    source: &crate::linear::LinearRgbaView<'_>,
    target: &crate::linear::LinearRgbaView<'_>,
    transform: crate::geometry::Transform,
    signals: ReflectedSignals,
    cancel: impl Fn() -> bool,
) -> Result<Option<FilteredPhotometricEvidence>, WarpError> {
    let (photometric_policy, filter_policy, _, _) = signals;
    photometric_policy
        .zip(filter_policy)
        .map(|(fit, (filter, mode))| match mode {
            crate::warp::PhotometricFitMode::RejectOutsidePolicy => {
                crate::warp::verify_reflected_filtered_photometric(
                    source, target, transform, fit, filter, &cancel,
                )
            }
            crate::warp::PhotometricFitMode::ConstrainedLeastSquares => {
                crate::warp::verify_reflected_filtered_photometric_constrained(
                    source, target, transform, fit, filter, &cancel,
                )
            }
        })
        .transpose()
}

#[allow(clippy::too_many_lines)] // Preserve model selection and returned evidence in one transaction.
fn confirm_reflected_models_with_signals(
    source: &crate::linear::LinearRgbaView<'_>,
    target: &crate::linear::LinearRgbaView<'_>,
    correspondences: Vec<Correspondence>,
    models: [Option<crate::geometry::ReflectedGeometryEvidence>; 2],
    policy: LocalFilePolicy,
    signals: ReflectedSignals,
    cancel: impl Fn() -> bool,
) -> Result<ReflectedLocalFileEvidence, LocalFileError> {
    let (photometric_policy, filter_policy, fit_range, _) = signals;
    let mut geometry = None;
    let mut pixels = None;
    let mut photometric = None;
    let mut filtered = None;
    let mut photometric_failure = None;
    let mut candidate = false;
    let mut first_fit_failure = None;
    // Prefer the capped-residual model, then confirm the count-first model
    // if needed. Both use the same decoded pixels and unchanged thresholds.
    // Keep the first tested model's negative evidence if neither succeeds.
    for model in models.into_iter().rev().flatten() {
        let strict = crate::warp::verify_reflected_bidirectional(
            source,
            target,
            model.similarity.transform,
            policy.pixels,
            &cancel,
        )?;
        let fitted_result = photometric_policy
            .map(|fit| {
                let transform = model.similarity.transform;
                if signals.3 {
                    crate::warp::verify_reflected_photometric_display_projection(
                        source,
                        target,
                        transform,
                        fit,
                        fit_range.ok_or(WarpError::Invalid)?,
                        &cancel,
                    )
                    .map(|value| value.projected.evidence)
                } else if let Some(range) = fit_range {
                    crate::warp::verify_reflected_photometric_bidirectional_in_range(
                        source, target, transform, fit, range, &cancel,
                    )
                    .map(|value| value.evidence)
                } else {
                    crate::warp::verify_reflected_photometric_bidirectional(
                        source, target, transform, fit, &cancel,
                    )
                }
            })
            .transpose();
        let mut auxiliary_failure = None;
        let fitted = match fitted_result {
            Ok(value) => value,
            Err(WarpError::Fit(failure)) if filter_policy.is_some() => {
                auxiliary_failure = Some(failure);
                None
            }
            Err(WarpError::Fit(failure)) => {
                first_fit_failure.get_or_insert(failure);
                continue;
            }
            Err(error) => return Err(error.into()),
        };
        let filtered_result =
            verify_reflected_filter(source, target, model.similarity.transform, signals, &cancel);
        let filtered_model = match filtered_result {
            Ok(value) => value,
            Err(WarpError::Fit(failure)) => {
                first_fit_failure.get_or_insert(failure);
                continue;
            }
            Err(error) => return Err(error.into()),
        };
        let accepted_model = if let Some(value) = &filtered_model {
            accepted_photometric(&value.evidence, policy)
        } else {
            fitted.as_ref().map_or_else(
                || accepted(&strict, policy),
                |fit| accepted_photometric(fit, policy),
            )
        };
        if geometry.is_none() || accepted_model {
            geometry = Some(model);
            pixels = Some(strict);
            photometric = fitted;
            filtered = filtered_model;
            photometric_failure = auxiliary_failure;
        }
        if accepted_model {
            candidate = true;
            break;
        }
    }
    if !candidate && let Some(failure) = first_fit_failure {
        return Err(WarpError::Fit(failure).into());
    }
    if cancel() {
        return Err(LocalFileError::Cancelled);
    }
    Ok(ReflectedLocalFileEvidence {
        correspondences,
        geometry,
        pixels,
        photometric,
        photometric_fit_range: signals.2,
        display_projection: signals.3,
        photometric_failure,
        filtered,
        candidate,
    })
}

/// Explicit fitted-color reflected file comparison retaining strict pixel evidence.
/// Per-channel gain/offset is fitted in linear sRGB; alpha is never fitted.
///
/// # Errors
/// Invalid policy, fit/source/decode/resource failure or latched cancellation.
pub fn compare_reflected_local_files_photometric(
    left: &DecodeRequest,
    right: &DecodeRequest,
    policy: LocalFilePolicy,
    photometric: PhotometricPolicy,
    budget: &MemoryBudget,
    cancel: impl Fn() -> bool,
) -> Result<ReflectedLocalFileEvidence, LocalFileError> {
    compare_reflected_local_files_selected(left, right, policy, None, Some(photometric), budget, cancel)
}

/// Explicit reflected linear color fitting with spatial descriptor quotas.
/// Strict and fitted residuals remain separately exposed.
///
/// # Errors
/// Invalid grid/fit/policy, source/decode/resource failure or cancellation.
pub fn compare_reflected_local_files_spatial_photometric(
    left: &DecodeRequest,
    right: &DecodeRequest,
    policy: LocalFilePolicy,
    spatial: SpatialFeaturePolicy,
    photometric: PhotometricPolicy,
    budget: &MemoryBudget,
    cancel: impl Fn() -> bool,
) -> Result<ReflectedLocalFileEvidence, LocalFileError> {
    validate_spatial(spatial, policy.extract.max_features)?;
    compare_reflected_local_files_selected(
        left,
        right,
        policy,
        Some(spatial),
        Some(photometric),
        budget,
        cancel,
    )
}

#[cfg(test)]
mod reflected_model_confirmation_tests {
    use super::*;
    use crate::geometry::{ReflectedGeometryEvidence, Transform};

    fn fixture() -> (Vec<f32>, Vec<f32>) {
        let mut source = Vec::new();
        let mut target = Vec::new();
        for y in 0_u16..32 {
            for x in 0_u16..32 {
                let v = f32::from(x + 2 * y) / 128.;
                source.extend([v, 0.1 + 0.7 * v, 0.2 + 0.4 * v, 1.]);
                let v = f32::from(31 - x + 2 * y) / 128.;
                target.extend([
                    0.5 * v + 0.02,
                    0.5 * (0.1 + 0.7 * v) + 0.02,
                    0.5 * (0.2 + 0.4 * v) + 0.02,
                    1.,
                ]);
            }
        }
        (source, target)
    }
    fn policies() -> (LocalFilePolicy, PhotometricPolicy) {
        let pixels = WarpPolicy {
            tolerance: 0.001,
            max_source_pixels: 1024,
        };
        (
            LocalFilePolicy {
                decode: AnimationBudget {
                    max_frames: 1,
                    max_pixels: 1024,
                    max_file_bytes: 1024 * 1024,
                },
                extract: LocalPolicy {
                    max_pixels: 1024,
                    max_candidates: 1024,
                    max_features: 16,
                    minimum_corner_score: 0.0001,
                },
                matching: MatchPolicy {
                    max_comparisons: 256,
                    max_distance: 64,
                },
                geometry: GeometryPolicy {
                    tolerance: 2.,
                    min_inliers: 4,
                    max_points: 16,
                    max_hypotheses: 120,
                },
                pixels,
                minimum_compared_pixels: 800,
                minimum_coverage_fraction: 0.9,
                minimum_matched_fraction: 0.99,
            },
            PhotometricPolicy {
                residual: pixels,
                minimum_samples: 800,
                minimum_variance: 1e-5,
                minimum_gain: 0.2,
                maximum_gain: 5.,
                maximum_offset: 0.2,
            },
        )
    }
    fn model(translation_x: f64) -> ReflectedGeometryEvidence {
        ReflectedGeometryEvidence {
            similarity: GeometryEvidence {
                transform: Transform {
                    a: 1.,
                    b: 0.,
                    translation: [translation_x, 0.],
                },
                inliers: vec![0, 1, 2, 3],
                squared_error: 0.,
            },
        }
    }

    #[test]
    fn insufficient_fit_support_in_first_model_does_not_hide_second_verified_model() {
        let (source, target) = fixture();
        let source = crate::linear::LinearRgbaView::new(32, 32, &source, 1024, || false).unwrap();
        let target = crate::linear::LinearRgbaView::new(32, 32, &target, 1024, || false).unwrap();
        let (policy, fit) = policies();
        let first = model(10.);
        assert_eq!(
            crate::warp::verify_reflected_photometric_bidirectional(
                &source,
                &target,
                first.similarity.transform,
                fit,
                || false,
            ),
            Err(WarpError::Fit(PhotometricFitFailure::InsufficientSamples {
                observed: 352,
                required: 800
            }))
        );
        let good = model(31.);
        let result = confirm_reflected_models(
            &source,
            &target,
            Vec::new(),
            [Some(good.clone()), Some(first)],
            policy,
            Some(fit),
            || false,
        )
        .unwrap();
        assert!(result.candidate);
        assert_eq!(result.geometry, Some(good));
        let strict = result.pixels.unwrap();
        assert!(!accepted(&strict, policy));
        let fitted = result.photometric.unwrap();
        for (direction, expected_gain) in [(&fitted.forward, 0.5), (&fitted.reverse, 2.)] {
            assert_eq!(direction.pixels.compared_pixels, 1024);
            assert_eq!(direction.pixels.matched_pixels, 1024);
            for gain in direction.gain {
                assert!((gain - expected_gain).abs() < 1e-5);
            }
        }
    }

    #[test]
    fn coefficient_bounds_and_low_variance_failures_are_also_model_specific() {
        for low_variance in [false, true] {
            let (mut source_pixels, mut target_pixels) = fixture();
            if low_variance {
                // Vertical stripes: the first model sees only constant source
                // x=0, while the whole-image mirror has ample variance.
                for y in 0..32 {
                    for x in 0..32 {
                        let from = x * 4;
                        let to = (y * 32 + x) * 4;
                        for channel in 0..4 {
                            source_pixels[to + channel] = source_pixels[from + channel];
                            target_pixels[to + channel] = target_pixels[from + channel];
                        }
                    }
                }
            }
            let source = crate::linear::LinearRgbaView::new(32, 32, &source_pixels, 1024, || false).unwrap();
            let target = crate::linear::LinearRgbaView::new(32, 32, &target_pixels, 1024, || false).unwrap();
            let (policy, fit) = policies();
            let fit = PhotometricPolicy {
                minimum_samples: 20,
                maximum_offset: 0.05,
                ..fit
            };
            let first_fit = crate::warp::verify_reflected_photometric_bidirectional(
                &source,
                &target,
                model(0.).similarity.transform,
                fit,
                || false,
            );
            let expected = if low_variance {
                PhotometricFitFailure::LowVariance { channel: 0 }
            } else {
                PhotometricFitFailure::OutsidePolicy { channel: 0 }
            };
            assert_eq!(first_fit, Err(WarpError::Fit(expected)));
            let result = confirm_reflected_models(
                &source,
                &target,
                Vec::new(),
                [Some(model(31.)), Some(model(0.))],
                policy,
                Some(fit),
                || false,
            )
            .unwrap();
            assert!(result.candidate);
            assert_eq!(result.geometry, Some(model(31.)));
        }
    }

    #[test]
    fn a_rejected_second_model_does_not_convert_first_fit_failure_to_a_negative() {
        let (source_pixels, mut target_pixels) = fixture();
        for pixel in target_pixels.chunks_exact_mut(4).step_by(7) {
            pixel[0] += 0.1;
        }
        let source = crate::linear::LinearRgbaView::new(32, 32, &source_pixels, 1024, || false).unwrap();
        let target = crate::linear::LinearRgbaView::new(32, 32, &target_pixels, 1024, || false).unwrap();
        let (policy, fit) = policies();
        let second = crate::warp::verify_reflected_photometric_bidirectional(
            &source,
            &target,
            model(31.).similarity.transform,
            fit,
            || false,
        )
        .unwrap();
        assert!(!accepted_photometric(&second, policy));
        assert!(matches!(
            confirm_reflected_models(
                &source,
                &target,
                Vec::new(),
                [Some(model(31.)), Some(model(10.))],
                policy,
                Some(fit),
                || false
            ),
            Err(LocalFileError::Pixels(WarpError::Fit(
                PhotometricFitFailure::InsufficientSamples {
                    observed: 352,
                    required: 800
                }
            )))
        ));
    }

    #[test]
    fn selected_encoded_filter_retains_auxiliary_linear_failure() {
        let mut source_pixels = Vec::new();
        let mut target_pixels = Vec::new();
        for y in 0_u16..32 {
            for x in 0_u16..32 {
                let v = f32::from(x + 2 * y) / 128. * 0.005;
                source_pixels.extend([v, 0.0001 + 0.7 * v, 0.0002 + 0.4 * v, 1.]);
                let v = f32::from(31 - x + 2 * y) / 128. * 0.005;
                target_pixels.extend([v, 0.0001 + 0.7 * v, 0.0002 + 0.4 * v, 1.]);
            }
        }
        let source = crate::linear::LinearRgbaView::new(32, 32, &source_pixels, 1024, || false).unwrap();
        let target = crate::linear::LinearRgbaView::new(32, 32, &target_pixels, 1024, || false).unwrap();
        let (policy, fit) = policies();
        let policy = LocalFilePolicy {
            minimum_compared_pixels: 500,
            minimum_coverage_fraction: 0.6,
            ..policy
        };
        let fit = PhotometricPolicy {
            minimum_samples: 500,
            ..fit
        };
        let filter = ColorFilterPolicy {
            filter: FilterPolicy {
                radius: 3,
                max_sample_pairs: 200_704,
            },
            color_space: crate::warp::FilterColorSpace::EncodedSrgb,
        };
        let result = confirm_reflected_models_with_signals(
            &source,
            &target,
            Vec::new(),
            [Some(model(31.)), None],
            policy,
            (
                Some(fit),
                Some((filter, crate::warp::PhotometricFitMode::RejectOutsidePolicy)),
                None,
                false,
            ),
            || false,
        )
        .unwrap();
        assert!(result.candidate);
        assert!(result.pixels.is_some());
        assert!(result.photometric.is_none());
        assert_eq!(
            result.photometric_failure,
            Some(PhotometricFitFailure::LowVariance { channel: 0 })
        );
        let filtered = result.filtered.unwrap();
        for direction in [&filtered.evidence.forward, &filtered.evidence.reverse] {
            assert_eq!(direction.pixels.compared_pixels, 676);
            assert_eq!(direction.pixels.matched_pixels, 676);
        }
    }

    #[test]
    #[allow(clippy::too_many_lines)] // Keep recovery and terminal-error checks on the same two models.
    fn ordinary_alternative_models_recover_photometric_range_and_display_fits() {
        let (source_pixels, mirrored) = fixture();
        let target_pixels = mirrored
            .as_chunks::<128>()
            .0
            .iter()
            .flat_map(|row| row.as_chunks::<4>().0.iter().rev().flatten().copied())
            .collect::<Vec<_>>();
        let source = crate::linear::LinearRgbaView::new(32, 32, &source_pixels, 1024, || false).unwrap();
        let target = crate::linear::LinearRgbaView::new(32, 32, &target_pixels, 1024, || false).unwrap();
        let (policy, fit) = policies();
        let good = model(0.).similarity;
        let bad = model(21.).similarity;
        let range = crate::warp::FitSampleRange {
            minimum: 0.,
            maximum: 1.,
        };
        for signals in [
            (Some(fit), None, None, false),
            (Some(fit), None, Some(range), false),
            (Some(fit), None, Some(range), true),
        ] {
            let result = confirm_local_models(
                &source,
                &target,
                Vec::new(),
                [Some(good.clone()), Some(bad.clone())],
                policy,
                signals,
                || false,
            )
            .unwrap();
            assert!(result.candidate);
            assert_eq!(result.geometry, Some(good.clone()));
            assert_eq!(result.photometric_fit_range, signals.2);
            assert_eq!(result.display_projection, signals.3);
            assert!(result.pixels.is_some() && result.photometric.is_some());
            assert!(result.photometric_failure.is_none() && result.filtered.is_none());
        }
        let checks = Cell::new(0);
        let signals = (Some(fit), None, Some(range), true);
        let expected = confirm_local_models(
            &source,
            &target,
            Vec::new(),
            [Some(good.clone()), Some(bad.clone())],
            policy,
            signals,
            || {
                checks.set(checks.get() + 1);
                false
            },
        )
        .unwrap();
        for checkpoint in [1, 100, checks.get() / 2, checks.get() - 1, checks.get()] {
            let count = Cell::new(0);
            assert!(matches!(
                confirm_local_models(
                    &source,
                    &target,
                    Vec::new(),
                    [Some(good.clone()), Some(bad.clone())],
                    policy,
                    signals,
                    || {
                        count.set(count.get() + 1);
                        count.get() == checkpoint
                    }
                ),
                Err(LocalFileError::Pixels(WarpError::Cancelled) | LocalFileError::Cancelled)
            ));
        }
        let retry = confirm_local_models(
            &source,
            &target,
            Vec::new(),
            [Some(good.clone()), Some(bad.clone())],
            policy,
            signals,
            || false,
        )
        .unwrap();
        assert_eq!(retry.geometry, expected.geometry);
        assert_eq!(retry.pixels, expected.pixels);
        assert_eq!(retry.photometric, expected.photometric);
        let too_small = PhotometricPolicy {
            residual: WarpPolicy {
                max_source_pixels: 1023,
                ..fit.residual
            },
            ..fit
        };
        assert!(matches!(
            confirm_local_models(
                &source,
                &target,
                Vec::new(),
                [Some(good), Some(bad)],
                policy,
                (Some(too_small), None, None, false),
                || false
            ),
            Err(LocalFileError::Pixels(WarpError::Budget))
        ));
    }

    #[test]
    fn reflected_ranged_models_recover_fit_and_preserve_selected_domain() {
        let (source, target) = fixture();
        let source = crate::linear::LinearRgbaView::new(32, 32, &source, 1024, || false).unwrap();
        let target = crate::linear::LinearRgbaView::new(32, 32, &target, 1024, || false).unwrap();
        let (policy, fit) = policies();
        let range = crate::warp::FitSampleRange {
            minimum: 0.,
            maximum: 1.,
        };
        for display in [false, true] {
            let result = confirm_reflected_models_with_signals(
                &source,
                &target,
                Vec::new(),
                [Some(model(31.)), Some(model(10.))],
                policy,
                (Some(fit), None, Some(range), display),
                || false,
            )
            .unwrap();
            assert!(result.candidate);
            assert_eq!(result.geometry, Some(model(31.)));
            assert_eq!(result.photometric_fit_range, Some(range));
            assert_eq!(result.display_projection, display);
            assert!(result.pixels.is_some() && result.photometric.is_some());
        }
    }

    #[test]
    fn exhausted_model_fits_preserve_first_inconclusive_reason() {
        let (source, target) = fixture();
        let source = crate::linear::LinearRgbaView::new(32, 32, &source, 1024, || false).unwrap();
        let target = crate::linear::LinearRgbaView::new(32, 32, &target, 1024, || false).unwrap();
        let (policy, fit) = policies();
        let result = confirm_reflected_models(
            &source,
            &target,
            Vec::new(),
            [Some(model(20.)), Some(model(10.))],
            policy,
            Some(fit),
            || false,
        );
        assert!(matches!(
            result,
            Err(LocalFileError::Pixels(WarpError::Fit(
                PhotometricFitFailure::InsufficientSamples {
                    observed: 352,
                    required: 800
                }
            )))
        ));
    }

    #[test]
    fn alternative_confirmation_cancel_and_work_limits_remain_terminal() {
        let (source, target) = fixture();
        let source = crate::linear::LinearRgbaView::new(32, 32, &source, 1024, || false).unwrap();
        let target = crate::linear::LinearRgbaView::new(32, 32, &target, 1024, || false).unwrap();
        let (policy, fit) = policies();
        let calls = Cell::new(0);
        let expected = confirm_reflected_models(
            &source,
            &target,
            Vec::new(),
            [Some(model(31.)), Some(model(10.))],
            policy,
            Some(fit),
            || {
                calls.set(calls.get() + 1);
                false
            },
        )
        .unwrap();
        for checkpoint in [1, 100, calls.get() / 2, calls.get() - 1, calls.get()] {
            let count = Cell::new(0);
            let result = confirm_reflected_models(
                &source,
                &target,
                Vec::new(),
                [Some(model(31.)), Some(model(10.))],
                policy,
                Some(fit),
                || {
                    count.set(count.get() + 1);
                    count.get() == checkpoint
                },
            );
            assert!(matches!(
                result,
                Err(LocalFileError::Pixels(WarpError::Cancelled) | LocalFileError::Cancelled)
            ));
        }
        let retry = confirm_reflected_models(
            &source,
            &target,
            Vec::new(),
            [Some(model(31.)), Some(model(10.))],
            policy,
            Some(fit),
            || false,
        )
        .unwrap();
        assert_eq!(retry.geometry, expected.geometry);
        assert_eq!(retry.pixels, expected.pixels);
        assert_eq!(retry.photometric, expected.photometric);
        let limited_fit = PhotometricPolicy {
            residual: WarpPolicy {
                max_source_pixels: 1023,
                ..fit.residual
            },
            ..fit
        };
        assert!(matches!(
            confirm_reflected_models(
                &source,
                &target,
                Vec::new(),
                [Some(model(31.)), Some(model(10.))],
                policy,
                Some(limited_fit),
                || false
            ),
            Err(LocalFileError::Pixels(WarpError::Budget))
        ));
    }
}

/// Compare planar visual candidates using explicit low-pass windows.
/// Strict residuals remain available; acceptance does not establish pixel equality.
///
/// # Errors
/// Invalid policy, resource refusal, source changes, geometry failure or cancellation.
pub fn compare_local_files_projective_filtered(
    left: &DecodeRequest,
    right: &DecodeRequest,
    policy: LocalFilePolicy,
    filter: crate::warp::ColorFilterPolicy,
    budget: &MemoryBudget,
    cancel: impl Fn() -> bool,
) -> Result<ProjectiveFileEvidence, LocalFileError> {
    compare_projective_registered_selected(
        left, right, policy, filter, None, None, None, None, budget, cancel,
    )
}
/// Compare planar candidates after explicitly bounded pixel registration.
///
/// # Errors
/// Source/resource failures, invalid policy or cancellation yield no candidate.
pub fn compare_local_files_projective_registered(
    left: &DecodeRequest,
    right: &DecodeRequest,
    policy: LocalFilePolicy,
    filter: crate::warp::ColorFilterPolicy,
    registration: crate::warp::ProjectiveRegistrationPolicy,
    budget: &MemoryBudget,
    cancel: impl Fn() -> bool,
) -> Result<ProjectiveFileEvidence, LocalFileError> {
    crate::warp::validate_registration_policy(registration)?;
    compare_projective_registered_selected(
        left,
        right,
        policy,
        filter,
        Some(registration),
        None,
        None,
        None,
        budget,
        cancel,
    )
}
#[allow(clippy::too_many_arguments)] // Explicit comparison and registration budgets share file lifecycle.
fn compare_projective_registered_selected(
    left: &DecodeRequest,
    right: &DecodeRequest,
    policy: LocalFilePolicy,
    filter: crate::warp::ColorFilterPolicy,
    registration: Option<crate::warp::ProjectiveRegistrationPolicy>,
    spatial: Option<SpatialFeaturePolicy>,
    sampling: Option<crate::geometry::ProjectiveSamplingPolicy>,
    anchored: Option<crate::warp::ProjectiveRegistrationTrustPolicy>,
    budget: &MemoryBudget,
    cancel: impl Fn() -> bool,
) -> Result<ProjectiveFileEvidence, LocalFileError> {
    if let Some(trust) = anchored {
        crate::warp::validate_registration_trust_policy(trust)?;
    }
    if let Some(sample) = sampling {
        if sample.seed == 0 || sample.trials == 0 {
            return Err(LocalFileError::InvalidPolicy);
        }
        if sample.trials > policy.geometry.max_hypotheses {
            return Err(crate::geometry::GeometryError::Budget.into());
        }
    }
    if let Some(grid) = spatial {
        validate_spatial(grid, policy.extract.max_features)?;
    }
    crate::warp::validate_filter_policy(filter.filter)?;
    if policy.geometry.min_inliers < 4 {
        return Err(LocalFileError::InvalidPolicy);
    }
    compare_local_file_views(
        left,
        right,
        policy,
        (None, None, None, false),
        spatial,
        budget,
        cancel,
        |a, b, correspondences, cancel| {
            let geometry = match sampling {
                Some(sample) => crate::geometry::verify_projective_sampled(
                    &correspondences,
                    policy.geometry,
                    sample,
                    cancel,
                )?,
                None => crate::geometry::verify_projective(&correspondences, policy.geometry, cancel)?,
            };
            let registered_transform = geometry
                .as_ref()
                .zip(registration)
                .map(|(g, r)| match anchored {
                    Some(trust) => {
                        crate::warp::refine_projective_pixels_anchored(a, b, g.transform, trust, cancel)
                    }
                    None => crate::warp::refine_projective_pixels(a, b, g.transform, r, cancel),
                })
                .transpose()?;
            let filtered = geometry
                .as_ref()
                .map(|g| {
                    crate::warp::verify_projective_filtered(
                        a,
                        b,
                        registered_transform.unwrap_or(g.transform),
                        policy.pixels,
                        filter,
                        cancel,
                    )
                })
                .transpose()?;
            let pixels = filtered.as_ref().map(|e| e.strict);
            let candidate = filtered.as_ref().is_some_and(|e| accepted(&e.filtered, policy));
            Ok(ProjectiveFileEvidence {
                registered_transform,
                filtered,
                correspondences,
                geometry,
                pixels,
                candidate,
            })
        },
    )
}

/// Planar registration with explicit spatial quotas for feature selection.
/// Changes selection only; descriptors and final acceptance remain unchanged.
///
/// # Errors
/// Invalid spatial/search policy, source/work refusal or cancellation.
#[allow(clippy::too_many_arguments)] // Explicit grid, comparison and registration policies share one lifecycle.
pub fn compare_local_files_projective_registered_spatial(
    left: &DecodeRequest,
    right: &DecodeRequest,
    policy: LocalFilePolicy,
    filter: crate::warp::ColorFilterPolicy,
    registration: crate::warp::ProjectiveRegistrationPolicy,
    spatial: SpatialFeaturePolicy,
    budget: &MemoryBudget,
    cancel: impl Fn() -> bool,
) -> Result<ProjectiveFileEvidence, LocalFileError> {
    crate::warp::validate_registration_policy(registration)?;
    compare_projective_registered_selected(
        left,
        right,
        policy,
        filter,
        Some(registration),
        Some(spatial),
        None,
        None,
        budget,
        cancel,
    )
}

/// Explicit sampled planar search configuration; sampling is approximate.
#[derive(Debug, Clone, Copy)]
pub struct ProjectiveSampledFilePolicy {
    pub local: LocalFilePolicy,
    pub filter: crate::warp::ColorFilterPolicy,
    pub registration: crate::warp::ProjectiveRegistrationPolicy,
    pub spatial: Option<SpatialFeaturePolicy>,
    pub sampling: crate::geometry::ProjectiveSamplingPolicy,
}

/// Sampled planar geometry with the same source/decode/registration/pixel lifecycle.
/// Fixed trial count does not certify confidence; final residual checks still apply.
///
/// # Errors
/// Invalid policy, changed source, work/resource refusal or cancellation.
pub fn compare_local_files_projective_sampled(
    left: &DecodeRequest,
    right: &DecodeRequest,
    policy: ProjectiveSampledFilePolicy,
    budget: &MemoryBudget,
    cancel: impl Fn() -> bool,
) -> Result<ProjectiveFileEvidence, LocalFileError> {
    crate::warp::validate_registration_policy(policy.registration)?;
    compare_projective_registered_selected(
        left,
        right,
        policy.local,
        policy.filter,
        Some(policy.registration),
        policy.spatial,
        Some(policy.sampling),
        None,
        budget,
        cancel,
    )
}

/// Explicit geometry trust region and nuisance-color objective for planar files.
#[derive(Debug, Clone, Copy)]
pub struct ProjectiveAnchoredFilePolicy {
    pub local: LocalFilePolicy,
    pub filter: crate::warp::ColorFilterPolicy,
    pub registration: crate::warp::ProjectiveRegistrationTrustPolicy,
    pub spatial: Option<SpatialFeaturePolicy>,
    pub sampling: Option<crate::geometry::ProjectiveSamplingPolicy>,
}
/// Anchored planar registration with unchanged full bidirectional pixel admission.
/// Color fitting affects geometry search only; it never grants visual acceptance.
///
/// # Errors
/// Invalid policy, source/work/resource refusal or cancellation.
pub fn compare_local_files_projective_anchored(
    left: &DecodeRequest,
    right: &DecodeRequest,
    policy: ProjectiveAnchoredFilePolicy,
    budget: &MemoryBudget,
    cancel: impl Fn() -> bool,
) -> Result<ProjectiveFileEvidence, LocalFileError> {
    compare_projective_registered_selected(
        left,
        right,
        policy.local,
        policy.filter,
        Some(policy.registration.registration),
        policy.spatial,
        policy.sampling,
        Some(policy.registration),
        budget,
        cancel,
    )
}

/// Explicit perspective and bounded color comparison, using the anchored
/// geometry policy and the caller's unchanged overlap/matched-pixel thresholds.
#[derive(Debug, Clone, Copy)]
pub struct ProjectivePhotometricFilePolicy {
    pub search: ProjectiveAnchoredFilePolicy,
    pub photometric: PhotometricPolicy,
    pub fit_mode: crate::warp::PhotometricFitMode,
}
#[derive(Debug)]
pub struct ProjectivePhotometricFileEvidence {
    pub correspondences: Vec<Correspondence>,
    pub geometry: Option<crate::geometry::ProjectiveEvidence>,
    pub registered_transform: Option<crate::geometry::ProjectiveTransform>,
    pub pixels: Option<crate::warp::ProjectivePhotometricEvidence>,
    /// Fitting refusal remains explicit; it cannot grant a candidate.
    pub fit_failure: Option<PhotometricFitFailure>,
    /// Retained on successful fitting, independently of the selected mode.
    pub unfitted: Option<crate::warp::ProjectiveFilteredEvidence>,
    pub candidate: bool,
}
/// Compare fixed selected frames with explicit perspective and color fitting.
/// Source snapshots, dependency validation, cancellation and managed decode
/// lifetimes use the same path as the other local file modes.
///
/// # Errors
/// Invalid policy, source/decode/work refusal or cancellation. A photometric
/// fitting refusal is returned as rejected evidence without retrying pixel work.
pub fn compare_local_files_projective_photometric(
    left: &DecodeRequest,
    right: &DecodeRequest,
    policy: ProjectivePhotometricFilePolicy,
    budget: &MemoryBudget,
    cancel: impl Fn() -> bool,
) -> Result<ProjectivePhotometricFileEvidence, LocalFileError> {
    let p = policy.search;
    crate::warp::validate_registration_trust_policy(p.registration)?;
    crate::warp::validate_photometric_policy(policy.photometric)?;
    crate::warp::validate_filter_policy(p.filter.filter)?;
    if p.local.geometry.min_inliers < 4 {
        return Err(LocalFileError::InvalidPolicy);
    }
    if let Some(grid) = p.spatial {
        validate_spatial(grid, p.local.extract.max_features)?;
    }
    if let Some(sampling) = p.sampling {
        if sampling.seed == 0 || sampling.trials == 0 {
            return Err(LocalFileError::InvalidPolicy);
        }
        if sampling.trials > p.local.geometry.max_hypotheses {
            return Err(GeometryError::Budget.into());
        }
    }
    compare_local_file_views(
        left,
        right,
        p.local,
        (None, None, None, false),
        p.spatial,
        budget,
        cancel,
        |a, b, correspondences, cancel| {
            let geometry = match p.sampling {
                Some(sampling) => crate::geometry::verify_projective_sampled(
                    &correspondences,
                    p.local.geometry,
                    sampling,
                    cancel,
                )?,
                None => crate::geometry::verify_projective(&correspondences, p.local.geometry, cancel)?,
            };
            let registered_transform = geometry
                .as_ref()
                .map(|g| {
                    crate::warp::refine_projective_pixels_anchored(a, b, g.transform, p.registration, cancel)
                })
                .transpose()?;
            let mut pixels = None;
            let mut fit_failure = None;
            let mut unfitted = None;
            if let Some(transform) = registered_transform {
                match crate::warp::verify_projective_photometric_filtered(
                    a,
                    b,
                    transform,
                    policy.photometric,
                    p.filter,
                    policy.fit_mode,
                    cancel,
                ) {
                    Ok(evidence) => {
                        unfitted = Some(evidence.unfitted);
                        pixels = Some(evidence);
                    }
                    Err(WarpError::Fit(reason)) => {
                        fit_failure = Some(reason);
                    }
                    Err(error) => return Err(error.into()),
                }
            }
            let candidate = pixels
                .as_ref()
                .is_some_and(|e| accepted_photometric(&e.fitted, p.local));
            Ok(ProjectivePhotometricFileEvidence {
                correspondences,
                geometry,
                registered_transform,
                pixels,
                fit_failure,
                unfitted,
                candidate,
            })
        },
    )
}

/// Explicit bounded scale-pyramid search with independently selected color fitting.
#[derive(Debug, Clone, Copy)]
pub struct ProjectivePyramidPhotometricFilePolicy {
    pub local: LocalFilePolicy,
    pub max_levels: usize,
    pub max_total_pixels: u64,
    pub max_total_features: usize,
    pub sampling: crate::geometry::ProjectiveSamplingPolicy,
    pub photometric: PhotometricPolicy,
    pub filter: ColorFilterPolicy,
    pub fit_mode: crate::warp::PhotometricFitMode,
}

/// Compare selected files with managed scale-pyramid features and an explicit
/// bounded color fit. Geometry is estimated once; no registration or implicit
/// color assumption is applied. Evidence keeps strict and fitted residuals.
///
/// # Errors
/// Policy, source, decode, memory/work and cancellation refusals return no result.
/// A bounded fitting refusal is explicit rejected evidence.
pub fn compare_local_files_projective_pyramid_photometric(
    left: &DecodeRequest,
    right: &DecodeRequest,
    policy: ProjectivePyramidPhotometricFilePolicy,
    budget: &MemoryBudget,
    cancel: impl Fn() -> bool,
) -> Result<ProjectivePhotometricFileEvidence, LocalFileError> {
    validate_projective_pyramid_file_policy(&policy)?;
    let p = policy.local;
    let pyramid = crate::pyramid::PyramidPolicy {
        local: p.extract,
        max_levels: policy.max_levels,
        max_total_pixels: policy.max_total_pixels,
        max_total_features: policy.max_total_features,
    };
    compare_local_file_views_with_extractor(
        left,
        right,
        p,
        (None, None, None, false),
        budget,
        cancel,
        |view, cancel| crate::pyramid::extract_oriented_pyramid_managed(view, pyramid, budget, cancel),
        |a, b, correspondences, cancel| {
            verify_projective_pyramid_views(a, b, correspondences, policy, cancel)
        },
    )
}
/// Two explicit pixel filters share one feature search and one geometry estimate.
#[derive(Debug, Clone, Copy)]
pub struct ProjectivePyramidFilterPortfolioPolicy {
    pub primary: ProjectivePyramidPhotometricFilePolicy,
    pub secondary: ColorFilterPolicy,
    /// Sum of both admitted filtered-pixel work caps.
    pub max_total_sample_pairs: u64,
}
#[derive(Debug)]
pub struct ProjectivePyramidFilterPortfolioEvidence {
    pub primary: ProjectivePhotometricFileEvidence,
    pub secondary_pixels: Option<crate::warp::ProjectivePhotometricEvidence>,
    pub secondary_fit_failure: Option<PhotometricFitFailure>,
    pub accepted_filters: [bool; 2],
    pub candidate: bool,
}
/// Both filters finish inside the shared source/dependency lifecycle. No filter
/// may hide another filter's cancellation or resource error. A fit refusal is
/// retained as rejected evidence; it is never interpreted as successful work.
///
/// # Errors
/// Invalid policy, cumulative/phase budget, decode, source changes or cancellation.
pub fn compare_local_files_projective_pyramid_filter_portfolio(
    left: &DecodeRequest,
    right: &DecodeRequest,
    policy: ProjectivePyramidFilterPortfolioPolicy,
    budget: &MemoryBudget,
    cancel: impl Fn() -> bool,
) -> Result<ProjectivePyramidFilterPortfolioEvidence, LocalFileError> {
    let p = policy.primary;
    validate_projective_pyramid_file_policy(&p)?;
    crate::warp::validate_filter_policy(policy.secondary.filter)?;
    let total = p
        .filter
        .filter
        .max_sample_pairs
        .checked_add(policy.secondary.filter.max_sample_pairs)
        .ok_or(WarpError::Budget)?;
    if total > policy.max_total_sample_pairs {
        return Err(WarpError::Budget.into());
    }
    let pyramid = crate::pyramid::PyramidPolicy {
        local: p.local.extract,
        max_levels: p.max_levels,
        max_total_pixels: p.max_total_pixels,
        max_total_features: p.max_total_features,
    };
    compare_local_file_views_with_extractor(
        left,
        right,
        p.local,
        (None, None, None, false),
        budget,
        cancel,
        |view, cancel| crate::pyramid::extract_oriented_pyramid_managed(view, pyramid, budget, cancel),
        |a, b, correspondences, cancel| {
            let primary = verify_projective_pyramid_views(a, b, correspondences, p, cancel)?;
            let mut secondary_pixels = None;
            let mut secondary_fit_failure = None;
            if let Some(g) = &primary.geometry {
                match crate::warp::verify_projective_photometric_filtered(
                    a,
                    b,
                    g.transform,
                    p.photometric,
                    policy.secondary,
                    p.fit_mode,
                    cancel,
                ) {
                    Ok(e) => secondary_pixels = Some(e),
                    Err(WarpError::Fit(reason)) => secondary_fit_failure = Some(reason),
                    Err(error) => return Err(error.into()),
                }
            }
            let accepted_filters = [
                primary.candidate,
                secondary_pixels
                    .as_ref()
                    .is_some_and(|e| accepted_photometric(&e.fitted, p.local)),
            ];
            Ok(ProjectivePyramidFilterPortfolioEvidence {
                primary,
                secondary_pixels,
                secondary_fit_failure,
                accepted_filters,
                candidate: accepted_filters.into_iter().any(|x| x),
            })
        },
    )
}

/// Explicit region confirmations under the same native pyramid geometry.
#[derive(Debug, Clone, Copy)]
pub struct ProjectivePyramidRegionsFilePolicy {
    pub search: ProjectivePyramidPhotometricFilePolicy,
    pub max_regions: usize,
    /// Primary verification cap plus one complete region verification cap per domain pair.
    pub max_total_sample_pairs: u64,
}
#[derive(Debug, Clone, Copy)]
pub struct ProjectiveRegionFileEvidence {
    pub domains: [crate::warp::PixelRectangle; 2],
    pub pixels: Option<crate::warp::ProjectiveRegionPhotometricEvidence>,
    pub fit_failure: Option<PhotometricFitFailure>,
}
#[derive(Debug)]
pub struct ProjectivePyramidRegionsFileEvidence {
    /// Whole-image evidence and candidate remain unchanged by regional matches.
    pub whole: ProjectivePhotometricFileEvidence,
    /// No geometry means no regional pixel verification was attempted.
    /// The reservation follows shared ownership until the final owner is dropped.
    pub regions: rrrah_core::SharedBuffer<ProjectiveRegionFileEvidence>,
}
/// Confirm declared regions inside the shared selected-frame source lifecycle.
/// Regional fits never promote a whole-image candidate. Every domain is validated
/// even when geometry is unavailable; fit refusals remain explicit per region.
/// Work/cancellation/source failures discard all previously computed evidence.
///
/// # Errors
/// Invalid region/policy, cumulative or phase work/memory, decode/source changes or cancellation.
pub fn compare_local_files_projective_pyramid_regions(
    left: &DecodeRequest,
    right: &DecodeRequest,
    policy: ProjectivePyramidRegionsFilePolicy,
    domains: &[[crate::warp::PixelRectangle; 2]],
    budget: &MemoryBudget,
    cancel: impl Fn() -> bool,
) -> Result<ProjectivePyramidRegionsFileEvidence, LocalFileError> {
    let p = policy.search;
    validate_projective_pyramid_file_policy(&p)?;
    if domains.is_empty() || policy.max_regions == 0 {
        return Err(LocalFileError::InvalidPolicy);
    }
    if domains.len() > policy.max_regions {
        return Err(WarpError::Budget.into());
    }
    for pair in domains {
        for region in pair {
            crate::warp::validate_rectangle(*region, (u32::MAX, u32::MAX))?;
        }
    }
    let phases = u64::try_from(domains.len())
        .ok()
        .and_then(|n| n.checked_add(1))
        .ok_or(WarpError::Budget)?;
    let work = p
        .filter
        .filter
        .max_sample_pairs
        .checked_mul(phases)
        .ok_or(WarpError::Budget)?;
    if work > policy.max_total_sample_pairs {
        return Err(WarpError::Budget.into());
    }
    let bytes = domains
        .len()
        .checked_mul(std::mem::size_of::<ProjectiveRegionFileEvidence>())
        .and_then(|n| u64::try_from(n).ok())
        .ok_or(LocalError::Budget)?;
    let pyramid = crate::pyramid::PyramidPolicy {
        local: p.local.extract,
        max_levels: p.max_levels,
        max_total_pixels: p.max_total_pixels,
        max_total_features: p.max_total_features,
    };
    compare_local_file_views_with_extractor(
        left,
        right,
        p.local,
        (None, None, None, false),
        budget,
        cancel,
        |view, cancel| crate::pyramid::extract_oriented_pyramid_managed(view, pyramid, budget, cancel),
        |a, b, correspondences, cancel| {
            for pair in domains {
                crate::warp::validate_rectangle(pair[0], a.dimensions())?;
                crate::warp::validate_rectangle(pair[1], b.dimensions())?;
            }
            let retained = budget.try_reserve(bytes).map_err(|_| LocalError::Budget)?;
            let whole = verify_projective_pyramid_views(a, b, correspondences, p, cancel)?;
            let mut regions = Vec::new();
            regions
                .try_reserve_exact(domains.len())
                .map_err(|_| LocalError::Budget)?;
            for pair in domains {
                if cancel() {
                    return Err(LocalFileError::Cancelled);
                }
                let mut pixels = None;
                let mut fit_failure = None;
                if let Some(g) = &whole.geometry {
                    match crate::warp::verify_projective_regions_photometric_filtered(
                        a,
                        b,
                        g.transform,
                        *pair,
                        p.photometric,
                        p.filter,
                        p.fit_mode,
                        cancel,
                    ) {
                        Ok(e) => pixels = Some(e),
                        Err(WarpError::Fit(reason)) => fit_failure = Some(reason),
                        Err(error) => return Err(error.into()),
                    }
                }
                regions.push(ProjectiveRegionFileEvidence {
                    domains: *pair,
                    pixels,
                    fit_failure,
                });
            }
            let regions = retained.try_adopt(regions).map_err(|_| LocalError::Budget)?;
            Ok(ProjectivePyramidRegionsFileEvidence { whole, regions })
        },
    )
}

fn verify_projective_pyramid_views(
    a: &crate::linear::LinearRgbaView<'_>,
    b: &crate::linear::LinearRgbaView<'_>,
    correspondences: Vec<Correspondence>,
    policy: ProjectivePyramidPhotometricFilePolicy,
    cancel: &dyn Fn() -> bool,
) -> Result<ProjectivePhotometricFileEvidence, LocalFileError> {
    let p = policy.local;
    let geometry =
        crate::geometry::verify_projective_sampled(&correspondences, p.geometry, policy.sampling, cancel)?;
    let mut pixels = None;
    let mut unfitted = None;
    let mut fit_failure = None;
    if let Some(g) = &geometry {
        match crate::warp::verify_projective_photometric_filtered(
            a,
            b,
            g.transform,
            policy.photometric,
            policy.filter,
            policy.fit_mode,
            cancel,
        ) {
            Ok(e) => {
                unfitted = Some(e.unfitted);
                pixels = Some(e);
            }
            Err(WarpError::Fit(reason)) => {
                fit_failure = Some(reason);
            }
            Err(error) => return Err(error.into()),
        }
    }
    let candidate = pixels
        .as_ref()
        .is_some_and(|e| accepted_photometric(&e.fitted, p));
    Ok(ProjectivePhotometricFileEvidence {
        correspondences,
        geometry,
        registered_transform: None,
        pixels,
        fit_failure,
        unfitted,
        candidate,
    })
}

pub(crate) fn validate_projective_pyramid_file_policy(
    policy: &ProjectivePyramidPhotometricFilePolicy,
) -> Result<(), LocalFileError> {
    validate(policy.local)?;
    crate::warp::validate_photometric_policy(policy.photometric)?;
    crate::warp::validate_filter_policy(policy.filter.filter)?;
    if policy.max_levels == 0
        || policy.local.geometry.min_inliers < 4
        || policy.sampling.seed == 0
        || policy.sampling.trials == 0
    {
        return Err(LocalFileError::InvalidPolicy);
    }
    if policy.sampling.trials > policy.local.geometry.max_hypotheses {
        return Err(GeometryError::Budget.into());
    }
    Ok(())
}

/// Fixed feature search and two separately verified registration hypotheses.
#[derive(Debug, Clone, Copy)]
pub struct ProjectivePortfolioFilePolicy {
    pub local: LocalFilePolicy,
    pub filter: ColorFilterPolicy,
    pub registration: crate::warp::ProjectiveRegistrationPortfolioPolicy,
    pub spatial: Option<SpatialFeaturePolicy>,
    pub sampling: Option<crate::geometry::ProjectiveSamplingPolicy>,
}
#[derive(Debug)]
pub struct ProjectivePortfolioFileEvidence {
    pub correspondences: Vec<Correspondence>,
    pub geometry: Option<crate::geometry::ProjectiveEvidence>,
    pub pixels: Option<crate::warp::ProjectiveCandidatePixelEvidence>,
    /// Either complete bidirectional verification passed the caller's policy.
    /// This remains selected-frame visual evidence, never exact identity.
    /// Independent final admission of anchored and unanchored lanes.
    pub accepted_lanes: [bool; 2],
    pub candidate: bool,
}
/// Compare local files through two independently verified registration lanes.
/// Both required lanes must complete within their admitted cumulative caps.
/// Source/dependency checks and cancellation use the existing shared lifecycle.
///
/// # Errors
/// Invalid policy, source/decode/model/work refusal, or cancellation discards the
/// result. A partial successful lane cannot mask an error in the other lane.
pub fn compare_local_files_projective_portfolio(
    left: &DecodeRequest,
    right: &DecodeRequest,
    policy: ProjectivePortfolioFilePolicy,
    budget: &MemoryBudget,
    cancel: impl Fn() -> bool,
) -> Result<ProjectivePortfolioFileEvidence, LocalFileError> {
    validate_projective_portfolio_file_policy(&policy)?;
    compare_local_file_views(
        left,
        right,
        policy.local,
        (None, None, None, false),
        policy.spatial,
        budget,
        cancel,
        |a, b, correspondences, cancel| {
            verify_projective_portfolio_views(a, b, correspondences, policy, cancel)
        },
    )
}
fn verify_projective_portfolio_views(
    a: &crate::linear::LinearRgbaView<'_>,
    b: &crate::linear::LinearRgbaView<'_>,
    correspondences: Vec<Correspondence>,
    policy: ProjectivePortfolioFilePolicy,
    cancel: &dyn Fn() -> bool,
) -> Result<ProjectivePortfolioFileEvidence, LocalFileError> {
    let geometry = match policy.sampling {
        Some(sample) => crate::geometry::verify_projective_sampled(
            &correspondences,
            policy.local.geometry,
            sample,
            cancel,
        )?,
        None => crate::geometry::verify_projective(&correspondences, policy.local.geometry, cancel)?,
    };
    let pixels = geometry
        .as_ref()
        .map(|g| {
            let models = crate::warp::refine_projective_pixels_candidates(
                a,
                b,
                g.transform,
                policy.registration,
                cancel,
            )?;
            crate::warp::verify_projective_candidates_filtered(
                a,
                b,
                models,
                policy.local.pixels,
                policy.filter,
                cancel,
            )
        })
        .transpose()?;
    let accepted_lanes = pixels.as_ref().map_or([false; 2], |e| {
        [
            accepted(&e.anchored.filtered, policy.local),
            accepted(&e.unanchored.filtered, policy.local),
        ]
    });
    let candidate = accepted_lanes.into_iter().any(|accepted| accepted);
    Ok(ProjectivePortfolioFileEvidence {
        correspondences,
        geometry,
        pixels,
        accepted_lanes,
        candidate,
    })
}

pub(crate) fn validate_projective_portfolio_file_policy(
    policy: &ProjectivePortfolioFilePolicy,
) -> Result<(), LocalFileError> {
    crate::warp::validate_registration_portfolio_policy(policy.registration)?;
    crate::warp::validate_filter_policy(policy.filter.filter)?;
    if policy.local.geometry.min_inliers < 4 {
        return Err(LocalFileError::InvalidPolicy);
    }
    if let Some(grid) = policy.spatial {
        validate_spatial(grid, policy.local.extract.max_features)?;
    }
    if let Some(sample) = policy.sampling {
        if sample.seed == 0 || sample.trials == 0 {
            return Err(LocalFileError::InvalidPolicy);
        }
        if sample.trials > policy.local.geometry.max_hypotheses {
            return Err(GeometryError::Budget.into());
        }
    }
    validate_selected_policy(policy.local, (None, None, None, false))?;
    Ok(())
}

/// Fixed complementary searches on the same selected decoded frames.
#[derive(Debug, Clone, Copy)]
pub struct ProjectiveComplementaryFilePolicy {
    pub registration: ProjectivePortfolioFilePolicy,
    pub pyramid: ProjectivePyramidPhotometricFilePolicy,
    /// Sum of both admitted feature-pair comparison caps.
    pub max_total_comparisons: u64,
    /// Sum of both admitted model trial caps.
    pub max_total_hypotheses: u64,
    /// Registration objectives plus both searches' filtered-pixel work caps.
    pub max_total_sample_pairs: u64,
}
#[derive(Debug)]
pub struct ProjectiveComplementaryFileEvidence {
    pub registration: ProjectivePortfolioFileEvidence,
    pub pyramid: ProjectivePhotometricFileEvidence,
    /// Registration portfolio, then pyramid/color confirmation.
    pub accepted_searches: [bool; 2],
    pub candidate: bool,
}
/// Compare two independently specified searches against the same decoded views.
/// Both searches must finish before the shared source/dependency/cancellation
/// lifecycle admits the result. Every residual and refusal remains in evidence;
/// a successful search cannot hide another search's work or cancellation error.
/// Pixel-grid and extraction limits remain separately enforced by each search.
///
/// # Errors
/// Invalid/incompatible policies, cumulative or phase work/memory limits,
/// decode/source changes or cancellation discard the entire result.
pub fn compare_local_files_projective_complementary(
    left: &DecodeRequest,
    right: &DecodeRequest,
    policy: ProjectiveComplementaryFilePolicy,
    budget: &MemoryBudget,
    cancel: impl Fn() -> bool,
) -> Result<ProjectiveComplementaryFileEvidence, LocalFileError> {
    compare_local_files_projective_complementary_inner(left, right, policy, None, budget, cancel)
        .map(|e| e.searches)
}

/// Registration and two explicit pyramid filters, sharing selected decoded views.
#[derive(Debug, Clone, Copy)]
pub struct ProjectiveComplementaryFilterPortfolioPolicy {
    pub searches: ProjectiveComplementaryFilePolicy,
    pub secondary: ColorFilterPolicy,
    /// Admitted base portfolio cap plus the secondary filtered-pixel work cap.
    pub max_total_sample_pairs: u64,
}
#[derive(Debug)]
pub struct ProjectiveComplementaryFilterPortfolioEvidence {
    /// Original registration/pyramid evidence and its unchanged acceptance.
    pub searches: ProjectiveComplementaryFileEvidence,
    pub secondary_pixels: Option<crate::warp::ProjectivePhotometricEvidence>,
    pub secondary_fit_failure: Option<PhotometricFitFailure>,
    /// Registration, primary pyramid filter, secondary pyramid filter.
    pub accepted_searches: [bool; 3],
    pub candidate: bool,
}
/// Estimate each feature family's geometry once, then verify all explicit pixel
/// hypotheses. Every phase must complete before any result is admitted.
///
/// # Errors
/// Invalid policy, checked cumulative/phase limits, decode/source changes or cancellation.
pub fn compare_local_files_projective_complementary_filter_portfolio(
    left: &DecodeRequest,
    right: &DecodeRequest,
    policy: ProjectiveComplementaryFilterPortfolioPolicy,
    budget: &MemoryBudget,
    cancel: impl Fn() -> bool,
) -> Result<ProjectiveComplementaryFilterPortfolioEvidence, LocalFileError> {
    validate_projective_complementary_filter_portfolio_policy(&policy)?;
    compare_local_files_projective_complementary_inner(
        left,
        right,
        policy.searches,
        Some(policy.secondary),
        budget,
        cancel,
    )
}
pub(crate) fn validate_projective_complementary_filter_portfolio_policy(
    policy: &ProjectiveComplementaryFilterPortfolioPolicy,
) -> Result<(), LocalFileError> {
    validate_projective_complementary_file_policy(&policy.searches)?;
    crate::warp::validate_filter_policy(policy.secondary.filter)?;
    let total = policy
        .searches
        .max_total_sample_pairs
        .checked_add(policy.secondary.filter.max_sample_pairs)
        .ok_or(WarpError::Budget)?;
    if total > policy.max_total_sample_pairs {
        return Err(WarpError::Budget.into());
    }
    Ok(())
}
fn compare_local_files_projective_complementary_inner(
    left: &DecodeRequest,
    right: &DecodeRequest,
    policy: ProjectiveComplementaryFilePolicy,
    secondary: Option<ColorFilterPolicy>,
    budget: &MemoryBudget,
    cancel: impl Fn() -> bool,
) -> Result<ProjectiveComplementaryFilterPortfolioEvidence, LocalFileError> {
    validate_projective_complementary_file_policy(&policy)?;
    let p = policy.pyramid;
    let pyramid = crate::pyramid::PyramidPolicy {
        local: p.local.extract,
        max_levels: p.max_levels,
        max_total_pixels: p.max_total_pixels,
        max_total_features: p.max_total_features,
    };
    compare_local_file_views_with_extractor(
        left,
        right,
        p.local,
        (None, None, None, false),
        budget,
        cancel,
        |view, cancel| crate::pyramid::extract_oriented_pyramid_managed(view, pyramid, budget, cancel),
        |a, b, correspondences, cancel| {
            let pyramid = verify_projective_pyramid_views(a, b, correspondences, p, cancel)?;
            let r = policy.registration;
            let af = extract_search_features(a, r.local.extract, r.spatial, budget, cancel)?;
            let bf = extract_search_features(b, r.local.extract, r.spatial, budget, cancel)?;
            let matches = match_features(&af, &bf, r.local.matching, cancel)?;
            let registration = verify_projective_portfolio_views(a, b, matches, r, cancel)?;
            let accepted_searches = [registration.candidate, pyramid.candidate];
            let candidate = accepted_searches.into_iter().any(|accepted| accepted);
            let mut secondary_pixels = None;
            let mut secondary_fit_failure = None;
            if let (Some(filter), Some(g)) = (secondary, &pyramid.geometry) {
                match crate::warp::verify_projective_photometric_filtered(
                    a,
                    b,
                    g.transform,
                    p.photometric,
                    filter,
                    p.fit_mode,
                    cancel,
                ) {
                    Ok(e) => secondary_pixels = Some(e),
                    Err(WarpError::Fit(reason)) => secondary_fit_failure = Some(reason),
                    Err(error) => return Err(error.into()),
                }
            }
            let secondary_accepted = secondary_pixels
                .as_ref()
                .is_some_and(|e| accepted_photometric(&e.fitted, p.local));
            let all = [accepted_searches[0], accepted_searches[1], secondary_accepted];
            Ok(ProjectiveComplementaryFilterPortfolioEvidence {
                searches: ProjectiveComplementaryFileEvidence {
                    registration,
                    pyramid,
                    accepted_searches,
                    candidate,
                },
                secondary_pixels,
                secondary_fit_failure,
                accepted_searches: all,
                candidate: all.into_iter().any(|x| x),
            })
        },
    )
}
pub(crate) fn validate_projective_complementary_file_policy(
    policy: &ProjectiveComplementaryFilePolicy,
) -> Result<(), LocalFileError> {
    validate_projective_portfolio_file_policy(&policy.registration)?;
    validate_projective_pyramid_file_policy(&policy.pyramid)?;
    let a = policy.registration.local.decode;
    let b = policy.pyramid.local.decode;
    if a.max_frames != b.max_frames || a.max_pixels != b.max_pixels || a.max_file_bytes != b.max_file_bytes {
        return Err(LocalFileError::InvalidPolicy);
    }
    let r = policy.registration;
    let p = policy.pyramid;
    let comparisons = r
        .local
        .matching
        .max_comparisons
        .checked_add(p.local.matching.max_comparisons)
        .ok_or(LocalError::Budget)?;
    let hypotheses = r
        .sampling
        .map_or(r.local.geometry.max_hypotheses, |s| s.trials)
        .checked_add(p.sampling.trials)
        .ok_or(GeometryError::Budget)?;
    let sample_pairs = r
        .registration
        .max_sample_pairs
        .checked_add(r.filter.filter.max_sample_pairs)
        .and_then(|n| n.checked_add(p.filter.filter.max_sample_pairs))
        .ok_or(WarpError::Budget)?;
    if comparisons > policy.max_total_comparisons {
        return Err(LocalError::Budget.into());
    }
    if hypotheses > policy.max_total_hypotheses {
        return Err(GeometryError::Budget.into());
    }
    if sample_pairs > policy.max_total_sample_pairs {
        return Err(WarpError::Budget.into());
    }
    Ok(())
}
