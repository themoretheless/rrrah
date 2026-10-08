//! Ownership-preserving adoption of returned geometric file evidence.
use crate::{
    geometry::ProjectiveTransform, local::LocalError, local_scan::ProjectivePhotometricFileEvidence,
};
use rrrah_core::{MemoryBudget, SharedBuffer};

#[derive(Debug, Clone)]
pub struct ManagedProjectiveGeometry {
    pub transform: ProjectiveTransform,
    pub inliers: SharedBuffer<usize>,
    pub squared_error: f64,
    pub hypotheses: u64,
}

/// Vector payload credit follows each buffer's last shared owner. Fixed fields
/// stay on the stack; allocator/control-block overhead and caller inputs are
/// excluded. Adoption does not retroactively account construction scratch.
#[derive(Debug, Clone)]
pub struct ManagedProjectivePhotometricFileEvidence {
    pub correspondences: SharedBuffer<crate::geometry::Correspondence>,
    pub geometry: Option<ManagedProjectiveGeometry>,
    pub registered_transform: Option<ProjectiveTransform>,
    pub pixels: Option<crate::warp::ProjectivePhotometricEvidence>,
    pub fit_failure: Option<crate::warp::PhotometricFitFailure>,
    pub unfitted: Option<crate::warp::ProjectiveFilteredEvidence>,
    pub candidate: bool,
}

/// Transfer existing vector payloads without copying. Both credits are admitted
/// before adoption using actual vector capacities. Cancellation or refusal drops
/// all transferred evidence and reservations; no partial result is published.
/// This is a result ownership API, not an end-to-end file allocation bound.
/// # Errors
/// Checked capacity/memory refusal or cancellation.
pub fn manage_projective_file_evidence(
    evidence: ProjectivePhotometricFileEvidence,
    budget: &MemoryBudget,
    cancel: impl Fn() -> bool,
) -> Result<ManagedProjectivePhotometricFileEvidence, LocalError> {
    if cancel() {
        return Err(LocalError::Cancelled);
    }
    let bytes = |capacity: usize, size: usize| {
        capacity
            .checked_mul(size)
            .and_then(|n| u64::try_from(n).ok())
            .ok_or(LocalError::Budget)
    };
    let correspondence_bytes = bytes(
        evidence.correspondences.capacity(),
        std::mem::size_of::<crate::geometry::Correspondence>(),
    )?;
    let inlier_bytes = evidence.geometry.as_ref().map_or(Ok(0), |g| {
        bytes(g.inliers.capacity(), std::mem::size_of::<usize>())
    })?;
    let correspondence_credit = budget
        .try_reserve(correspondence_bytes)
        .map_err(|_| LocalError::Budget)?;
    let inlier_credit = budget.try_reserve(inlier_bytes).map_err(|_| LocalError::Budget)?;
    if cancel() {
        return Err(LocalError::Cancelled);
    }
    let correspondences = correspondence_credit
        .try_adopt(evidence.correspondences)
        .map_err(|_| LocalError::Budget)?;
    if cancel() {
        return Err(LocalError::Cancelled);
    }
    let geometry = if let Some(g) = evidence.geometry {
        let inliers = inlier_credit
            .try_adopt(g.inliers)
            .map_err(|_| LocalError::Budget)?;
        Some(ManagedProjectiveGeometry {
            transform: g.transform,
            inliers,
            squared_error: g.squared_error,
            hypotheses: g.hypotheses,
        })
    } else {
        drop(inlier_credit);
        None
    };
    if cancel() {
        return Err(LocalError::Cancelled);
    }
    Ok(ManagedProjectivePhotometricFileEvidence {
        correspondences,
        geometry,
        registered_transform: evidence.registered_transform,
        pixels: evidence.pixels,
        fit_failure: evidence.fit_failure,
        unfitted: evidence.unfitted,
        candidate: evidence.candidate,
    })
}
