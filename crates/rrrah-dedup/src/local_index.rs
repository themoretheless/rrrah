//! Exact metric retrieval for BRIEF feature variants, separate from geometry proof.
use crate::{
    local::{Feature, FeatureRecipe, LocalError, reserve_slot},
    pixels::Fingerprint,
    visual::{VisualError, VisualIndex},
};
use std::collections::{HashMap, HashSet};
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DescriptorCandidate {
    pub id: u64,
    pub distance: u32,
}
#[derive(Debug)]
pub struct LocalDescriptorIndex {
    recipe: Option<FeatureRecipe>,
    index: VisualIndex,
}
fn mapped(error: &VisualError) -> LocalError {
    match error {
        VisualError::Budget => LocalError::Budget,
        VisualError::Cancelled => LocalError::Cancelled,
        VisualError::DuplicateId => LocalError::Invalid,
    }
}
fn as_fingerprint(feature: &Feature) -> Fingerprint {
    let mut variants = [feature.descriptor; 8];
    variants[1..4].copy_from_slice(&feature.quarter_turns);
    // The remaining repeats do not change minimum distance. Pixel-information
    // fields are unused: this internal adapter indexes descriptors, not images.
    Fingerprint {
        variants,
        mean_linear_rgb: [0.0; 3],
        luminance_stddev: 1.0,
    }
}
impl LocalDescriptorIndex {
    /// Index one compatible feature recipe under a finite feature-entry limit.
    /// Application IDs are opaque, including `u64::MAX`. All stored right-side
    /// descriptor variants participate, exactly as in `match_features`.
    ///
    /// # Errors
    /// Mixed recipes, duplicate IDs, entry budget or cancellation.
    pub fn new(
        entries: impl IntoIterator<Item = (u64, Feature)>,
        max_entries: usize,
        cancel: impl Fn() -> bool,
    ) -> Result<Self, LocalError> {
        let mut recipe = None;
        let mut fingerprints = Vec::new();
        for (id, feature) in entries {
            if cancel() {
                return Err(LocalError::Cancelled);
            }
            if fingerprints.len() >= max_entries {
                return Err(LocalError::Budget);
            }
            if recipe.is_some_and(|r| r != feature.recipe) {
                return Err(LocalError::Invalid);
            }
            recipe = Some(feature.recipe);
            reserve_slot(&mut fingerprints, max_entries)?;
            fingerprints.push((id, as_fingerprint(&feature)));
        }
        let index = VisualIndex::new(fingerprints, max_entries, cancel).map_err(|error| mapped(&error))?;
        Ok(Self { recipe, index })
    }
    /// Retrieve exact minimum 256-bit distance over allowed recipe variants.
    /// Quarter-turn/oriented queries use their canonical descriptor; multiscale
    /// queries use all four, matching the exhaustive feature-pair metric.
    /// Results are ordered by (distance,id). They do not establish mutual nearest
    /// matching, ambiguity ratio, geometric overlap or duplicate-image identity.
    ///
    /// # Errors
    /// Incompatible query recipe, result limit or cancellation; no partial list.
    pub fn search(
        &self,
        query: &Feature,
        radius: u32,
        max_results: usize,
        cancel: impl Fn() -> bool,
    ) -> Result<Vec<DescriptorCandidate>, LocalError> {
        if cancel() {
            return Err(LocalError::Cancelled);
        }
        if self.recipe.is_some_and(|r| r != query.recipe) {
            return Err(LocalError::Invalid);
        }
        let variants = std::iter::once(&query.descriptor).chain(
            query
                .quarter_turns
                .iter()
                .take(if query.recipe.has_scale_variants() { 3 } else { 0 }),
        );
        let mut found = HashMap::new();
        for descriptor in variants {
            let fingerprint = Fingerprint {
                variants: [*descriptor; 8],
                mean_linear_rgb: [0.0; 3],
                luminance_stddev: 1.0,
            };
            for hit in self
                .index
                .search(&fingerprint, radius, true, &cancel)
                .map_err(|error| mapped(&error))?
            {
                if cancel() {
                    return Err(LocalError::Cancelled);
                }
                if let Some(distance) = found.get_mut(&hit.id) {
                    *distance = u32::min(*distance, hit.evidence.distance);
                } else {
                    if found.len() >= max_results {
                        return Err(LocalError::Budget);
                    }
                    found.try_reserve(1).map_err(|_| LocalError::Budget)?;
                    found.insert(hit.id, hit.evidence.distance);
                }
            }
        }
        let mut results = Vec::new();
        results
            .try_reserve_exact(found.len())
            .map_err(|_| LocalError::Budget)?;
        results.extend(
            found
                .into_iter()
                .map(|(id, distance)| DescriptorCandidate { id, distance }),
        );
        results.sort_unstable_by_key(|c| (c.distance, c.id));
        if cancel() {
            return Err(LocalError::Cancelled);
        }
        Ok(results)
    }
}

#[derive(Debug, Clone, Copy)]
pub struct IndexedMatchPolicy {
    pub max_features_per_side: usize,
    /// Total exact-radius hits across forward and reverse queries, not metric-node visits.
    pub max_hits: usize,
    pub max_distance: u32,
}
/// Mutual unique nearest matching with the same strict 3/4 ambiguity ratio as
/// exhaustive matching. The search radius includes every possible ambiguity
/// challenger: `floor(4*max_distance/3)`, capped by the 256-bit metric diameter.
/// Missing farther neighbors cannot change acceptance. Directed recipe semantics
/// are preserved even for supplied non-symmetric descriptor variants.
///
/// # Errors
/// Mixed recipes, feature/hit budgets or cancellation without partial correspondences.
pub fn match_features_indexed(
    left: &[Feature],
    right: &[Feature],
    policy: IndexedMatchPolicy,
    cancel: impl Fn() -> bool,
) -> Result<Vec<crate::geometry::Correspondence>, LocalError> {
    if cancel() {
        return Err(LocalError::Cancelled);
    }
    if left.is_empty() || right.is_empty() {
        return Ok(Vec::new());
    }
    if left.len() > policy.max_features_per_side || right.len() > policy.max_features_per_side {
        return Err(LocalError::Budget);
    }
    let recipe = left[0].recipe;
    for feature in left.iter().chain(right) {
        if cancel() {
            return Err(LocalError::Cancelled);
        }
        if feature.recipe != recipe {
            return Err(LocalError::Invalid);
        }
    }
    let entries = right
        .iter()
        .enumerate()
        .map(|(i, f)| {
            if cancel() {
                return Err(LocalError::Cancelled);
            }
            Ok((u64::try_from(i).map_err(|_| LocalError::Budget)?, *f))
        })
        .collect::<Result<Vec<_>, LocalError>>()?;
    let forward = LocalDescriptorIndex::new(entries, policy.max_features_per_side, &cancel)?;
    // Reverse queries must use the transpose of the original metric, rather than
    // assuming quarter-turn arrays form an exact rotation group.
    let entries = left
        .iter()
        .enumerate()
        .map(|(i, f)| {
            if cancel() {
                return Err(LocalError::Cancelled);
            }
            let mut feature = *f;
            feature.recipe = FeatureRecipe::OrientedScaleBriefV1;
            if !recipe.has_scale_variants() {
                feature.quarter_turns = [feature.descriptor; 3];
            }
            Ok((u64::try_from(i).map_err(|_| LocalError::Budget)?, feature))
        })
        .collect::<Result<Vec<_>, LocalError>>()?;
    let reverse = LocalDescriptorIndex::new(entries, policy.max_features_per_side, &cancel)?;
    let radius = ((policy.max_distance.min(256) * 4) / 3).min(256);
    let mut hits = 0usize;
    let rows = nearest_rows(&forward, left, false, radius, policy.max_hits, &mut hits, &cancel)?;
    let columns = nearest_rows(&reverse, right, true, radius, policy.max_hits, &mut hits, &cancel)?;
    let mut matches = Vec::new();
    for (i, &(distance, second, j)) in rows.iter().enumerate() {
        if cancel() {
            return Err(LocalError::Cancelled);
        }
        if distance > policy.max_distance.min(256) || u64::from(distance) * 4 >= u64::from(second) * 3 {
            continue;
        }
        let (reverse, next, index) = columns[j];
        if index == i && u64::from(reverse) * 4 < u64::from(next) * 3 {
            matches.push(crate::geometry::Correspondence {
                source: left[i].position,
                target: right[j].position,
            });
        }
    }
    if cancel() {
        return Err(LocalError::Cancelled);
    }
    Ok(matches)
}
fn nearest_rows(
    index: &LocalDescriptorIndex,
    queries: &[Feature],
    transpose: bool,
    radius: u32,
    max_hits: usize,
    hits: &mut usize,
    cancel: impl Fn() -> bool,
) -> Result<Vec<(u32, u32, usize)>, LocalError> {
    let mut rows = Vec::new();
    for query in queries {
        if cancel() {
            return Err(LocalError::Cancelled);
        }
        let mut query = *query;
        if transpose {
            query.recipe = FeatureRecipe::OrientedScaleBriefV1;
        }
        let found = index.search(&query, radius, max_hits.saturating_sub(*hits), &cancel)?;
        *hits = hits.checked_add(found.len()).ok_or(LocalError::Budget)?;
        let row = if let Some(first) = found.first() {
            (
                first.distance,
                found.get(1).map_or(u32::MAX, |c| c.distance),
                usize::try_from(first.id).map_err(|_| LocalError::Budget)?,
            )
        } else {
            (u32::MAX, u32::MAX, 0)
        };
        rows.push(row);
    }
    Ok(rows)
}

#[derive(Debug, Clone, Copy)]
pub struct FileFeatureBudgets {
    pub max_files: usize,
    pub max_features: usize,
    pub max_hits: usize,
    /// Temporary pair counters, including pairs not reaching the shared-feature gate.
    pub max_pair_counts: usize,
    pub max_pairs: usize,
}
#[derive(Debug)]
pub struct DescriptorFilePairs {
    pub pairs: Vec<(u64, u64)>,
    pub indexed_features: usize,
    pub descriptor_hits: usize,
    pub insufficient_features: Vec<u64>,
}
/// Retrieve file pairs with enough distinct left-side feature queries having a
/// within-radius right-side descriptor. This is necessary, not sufficient, for
/// that many mutual matches. Lower file ID defines the left side, preserving
/// the directed exhaustive metric. Geometry and pixels must still be verified.
///
/// # Errors
/// Duplicate IDs, mixed recipes, zero shared-feature threshold, entry/hit/pair
/// limits or cancellation without partial pairs.
pub fn descriptor_file_pairs(
    files: impl IntoIterator<Item = (u64, Vec<Feature>)>,
    max_distance: u32,
    minimum_shared_features: usize,
    budgets: FileFeatureBudgets,
    cancel: impl Fn() -> bool,
) -> Result<DescriptorFilePairs, LocalError> {
    descriptor_file_pairs_shared(files, max_distance, minimum_shared_features, budgets, cancel)
}

pub(crate) fn descriptor_file_pairs_shared<B: std::ops::Deref<Target = [Feature]>>(
    files: impl IntoIterator<Item = (u64, B)>,
    max_distance: u32,
    minimum_shared_features: usize,
    budgets: FileFeatureBudgets,
    cancel: impl Fn() -> bool,
) -> Result<DescriptorFilePairs, LocalError> {
    if minimum_shared_features == 0 {
        return Err(LocalError::Invalid);
    }
    let mut documents = Vec::new();
    let mut seen = HashSet::new();
    let mut count = 0usize;
    for (id, features) in files {
        if cancel() {
            return Err(LocalError::Cancelled);
        }
        if seen.contains(&id) {
            return Err(LocalError::Invalid);
        }
        if documents.len() >= budgets.max_files {
            return Err(LocalError::Budget);
        }
        count = count.checked_add(features.len()).ok_or(LocalError::Budget)?;
        if count > budgets.max_features {
            return Err(LocalError::Budget);
        }
        reserve_slot(&mut documents, budgets.max_files)?;
        seen.try_reserve(1).map_err(|_| LocalError::Budget)?;
        seen.insert(id);
        documents.push((id, features));
    }
    documents.sort_unstable_by_key(|(id, _)| *id);
    drop(seen);
    let mut slots = Vec::new();
    let mut insufficient = Vec::new();
    for (id, features) in &documents {
        let id = *id;
        if features.len() < minimum_shared_features {
            reserve_slot(&mut insufficient, budgets.max_files)?;
            insufficient.push(id);
        }
        for _ in features.iter() {
            if cancel() {
                return Err(LocalError::Cancelled);
            }
            reserve_slot(&mut slots, budgets.max_features)?;
            slots.push(id);
        }
    }
    u64::try_from(count).map_err(|_| LocalError::Budget)?;
    let entries = documents
        .iter()
        .flat_map(|(_, features)| features.iter())
        .enumerate()
        .map(|(slot, feature)| (u64::try_from(slot).expect("validated feature count"), *feature));
    let index = LocalDescriptorIndex::new(entries, budgets.max_features, &cancel)?;
    let mut hits = 0usize;
    let mut counts = HashMap::new();
    for (left, features) in &documents {
        let left = *left;
        for feature in features.iter() {
            let found = index.search(
                feature,
                max_distance,
                budgets.max_hits.saturating_sub(hits),
                &cancel,
            )?;
            hits = hits.checked_add(found.len()).ok_or(LocalError::Budget)?;
            let mut targets = HashSet::new();
            for candidate in found {
                if cancel() {
                    return Err(LocalError::Cancelled);
                }
                let slot = usize::try_from(candidate.id).map_err(|_| LocalError::Budget)?;
                let right = slots[slot];
                if right > left && !targets.contains(&right) {
                    targets.try_reserve(1).map_err(|_| LocalError::Budget)?;
                    targets.insert(right);
                }
            }
            count_targets(targets, left, &mut counts, budgets.max_pair_counts)?;
        }
    }
    let mut pairs = Vec::new();
    for (pair, shared) in counts {
        if cancel() {
            return Err(LocalError::Cancelled);
        }
        if shared >= minimum_shared_features {
            reserve_slot(&mut pairs, budgets.max_pairs)?;
            pairs.push(pair);
        }
    }
    if cancel() {
        return Err(LocalError::Cancelled);
    }
    pairs.sort_unstable();
    Ok(DescriptorFilePairs {
        pairs,
        indexed_features: count,
        descriptor_hits: hits,
        insufficient_features: insufficient,
    })
}

fn count_targets(
    targets: HashSet<u64>,
    left: u64,
    counts: &mut HashMap<(u64, u64), usize>,
    max_pair_counts: usize,
) -> Result<(), LocalError> {
    let mut ordered_targets = Vec::new();
    ordered_targets
        .try_reserve_exact(targets.len())
        .map_err(|_| LocalError::Budget)?;
    ordered_targets.extend(targets);
    ordered_targets.sort_unstable();
    for right in ordered_targets {
        if !counts.contains_key(&(left, right)) {
            if counts.len() >= max_pair_counts {
                return Err(LocalError::Budget);
            }
            counts.try_reserve(1).map_err(|_| LocalError::Budget)?;
        }
        let shared = counts.entry((left, right)).or_insert(0usize);
        *shared = shared.checked_add(1).ok_or(LocalError::Budget)?;
    }
    Ok(())
}
