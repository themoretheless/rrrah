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
    index: DescriptorIndexStorage,
}
#[derive(Debug)]
enum DescriptorIndexStorage {
    Legacy(VisualIndex),
    #[cfg(feature = "decode")]
    Managed {
        index: crate::managed_visual::ManagedVisualIndex,
        budget: rrrah_core::MemoryBudget,
    },
}
enum DescriptorVisualHits {
    Legacy(Vec<crate::visual::VisualCandidate>),
    #[cfg(feature = "decode")]
    Managed(crate::managed_visual::ManagedVisualCandidates),
}
impl std::ops::Deref for DescriptorVisualHits {
    type Target = [crate::visual::VisualCandidate];
    fn deref(&self) -> &Self::Target {
        match self {
            Self::Legacy(v) => v,
            #[cfg(feature = "decode")]
            Self::Managed(v) => v,
        }
    }
}
impl DescriptorIndexStorage {
    fn search(
        &self,
        query: &Fingerprint,
        radius: u32,
        transforms: bool,
        cancel: &dyn Fn() -> bool,
    ) -> Result<DescriptorVisualHits, VisualError> {
        match self {
            Self::Legacy(index) => index
                .search(query, radius, transforms, cancel)
                .map(DescriptorVisualHits::Legacy),
            #[cfg(feature = "decode")]
            Self::Managed { index, budget } => index
                .search(query, radius, transforms, budget, cancel)
                .map(DescriptorVisualHits::Managed),
        }
    }
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
enum DescriptorRetrievalHits {
    Legacy(Vec<DescriptorCandidate>),
    #[cfg(feature = "decode")]
    Managed(rrrah_core::SharedBuffer<DescriptorCandidate>),
}
impl std::ops::Deref for DescriptorRetrievalHits {
    type Target = [DescriptorCandidate];
    fn deref(&self) -> &Self::Target {
        match self {
            Self::Legacy(v) => v,
            #[cfg(feature = "decode")]
            Self::Managed(v) => v,
        }
    }
}
impl LocalDescriptorIndex {
    fn search_retrieval(
        &self,
        query: &Feature,
        radius: u32,
        max_results: usize,
        cancel: impl Fn() -> bool,
    ) -> Result<DescriptorRetrievalHits, LocalError> {
        if cancel() {
            return Err(LocalError::Cancelled);
        }
        if self.recipe.is_some_and(|r| r != query.recipe) {
            return Err(LocalError::Invalid);
        }
        match &self.index {
            #[cfg(feature = "decode")]
            DescriptorIndexStorage::Managed { index, budget } => {
                crate::managed_descriptor_union::search(index, query, radius, max_results, budget, cancel)
                    .map(DescriptorRetrievalHits::Managed)
            }
            _ => self
                .search(query, radius, max_results, cancel)
                .map(DescriptorRetrievalHits::Legacy),
        }
    }
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
        Ok(Self {
            recipe,
            index: DescriptorIndexStorage::Legacy(index),
        })
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
                .iter()
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
    descriptor_file_pairs_with_slot_storage(
        files,
        max_distance,
        minimum_shared_features,
        budgets,
        cancel,
        |documents, _count, cancel| {
            let mut slots = Vec::new();
            for (id, features) in documents {
                for _ in features.iter() {
                    if cancel() {
                        return Err(LocalError::Cancelled);
                    }
                    reserve_slot(&mut slots, budgets.max_features)?;
                    slots.push(*id);
                }
            }
            Ok(slots)
        },
        |documents, count, cancel| build_descriptor_index(documents, count, budgets.max_features, cancel),
        |_| Ok(FileTargets::Legacy(HashSet::new())),
        |_| Ok(FilePairCounts::Legacy(HashMap::new())),
    )
}

/// Manage the owner table and temporary fingerprint construction staging.
/// Owner credit spans retrieval; staging credit spans native index construction.
/// Native tree/maps/query scratch remain independently bounded by entry/work caps.
#[cfg(feature = "decode")]
pub(crate) fn descriptor_file_pairs_shared_managed<B: std::ops::Deref<Target = [Feature]>>(
    files: impl IntoIterator<Item = (u64, B)>,
    max_distance: u32,
    minimum_shared_features: usize,
    budgets: FileFeatureBudgets,
    budget: &rrrah_core::MemoryBudget,
    cancel: impl Fn() -> bool,
) -> Result<DescriptorFilePairs, LocalError> {
    descriptor_file_pairs_with_slot_storage(
        files,
        max_distance,
        minimum_shared_features,
        budgets,
        cancel,
        |documents, count, cancel| {
            let bytes = count
                .checked_mul(std::mem::size_of::<u64>())
                .and_then(|n| u64::try_from(n).ok())
                .ok_or(LocalError::Budget)?;
            let credit = budget.try_reserve(bytes).map_err(|_| LocalError::Budget)?;
            let mut slots = Vec::new();
            slots.try_reserve_exact(count).map_err(|_| LocalError::Budget)?;
            for (id, features) in documents {
                for _ in features.iter() {
                    if cancel() {
                        return Err(LocalError::Cancelled);
                    }
                    slots.push(*id);
                }
            }
            credit.try_adopt(slots).map_err(|_| LocalError::Budget)
        },
        |documents, count, cancel| {
            let bytes = count
                .checked_mul(std::mem::size_of::<(u64, Fingerprint)>())
                .and_then(|n| u64::try_from(n).ok())
                .ok_or(LocalError::Budget)?;
            let mut credit = budget.try_reserve(bytes).map_err(|_| LocalError::Budget)?;
            let mut fingerprints = Vec::new();
            fingerprints
                .try_reserve_exact(count)
                .map_err(|_| LocalError::Budget)?;
            credit
                .ensure_bytes(
                    fingerprints
                        .capacity()
                        .checked_mul(std::mem::size_of::<(u64, Fingerprint)>())
                        .and_then(|n| u64::try_from(n).ok())
                        .ok_or(LocalError::Budget)?,
                )
                .map_err(|_| LocalError::Budget)?;
            let mut recipe = None;
            for (slot, feature) in documents
                .iter()
                .flat_map(|(_, features)| features.iter())
                .enumerate()
            {
                if cancel() {
                    return Err(LocalError::Cancelled);
                }
                if recipe.is_some_and(|r| r != feature.recipe) {
                    return Err(LocalError::Invalid);
                }
                recipe = Some(feature.recipe);
                fingerprints.push((
                    u64::try_from(slot).map_err(|_| LocalError::Budget)?,
                    as_fingerprint(feature),
                ));
            }
            // Fingerprint credit now transfers into the owned managed index.
            let index = crate::managed_visual::ManagedVisualIndex::new(fingerprints, credit, budget, cancel)
                .map_err(|e| mapped(&e))?;
            Ok(LocalDescriptorIndex {
                recipe,
                index: DescriptorIndexStorage::Managed {
                    index,
                    budget: budget.clone(),
                },
            })
        },
        |files| crate::managed_targets::ManagedTargets::new(files, budget).map(FileTargets::Managed),
        |files| {
            let maximum = files
                .checked_mul(files.saturating_sub(1))
                .map(|n| n / 2)
                .ok_or(LocalError::Budget)?;
            Ok(FilePairCounts::Managed(
                crate::managed_pair_counts::ManagedPairCounts::new(
                    maximum.min(budgets.max_pair_counts),
                    budget,
                ),
            ))
        },
    )
}

fn build_descriptor_index<B: std::ops::Deref<Target = [Feature]>>(
    documents: &[(u64, B)],
    count: usize,
    max_entries: usize,
    cancel: &dyn Fn() -> bool,
) -> Result<LocalDescriptorIndex, LocalError> {
    u64::try_from(count).map_err(|_| LocalError::Budget)?;
    let entries = documents
        .iter()
        .flat_map(|(_, features)| features.iter())
        .enumerate()
        .map(|(slot, feature)| (u64::try_from(slot).expect("validated feature count"), *feature));
    LocalDescriptorIndex::new(entries, max_entries, cancel)
}

enum FileTargets {
    Legacy(HashSet<u64>),
    #[cfg(feature = "decode")]
    Managed(crate::managed_targets::ManagedTargets),
}
enum FilePairCounts {
    Legacy(HashMap<(u64, u64), usize>),
    #[cfg(feature = "decode")]
    Managed(crate::managed_pair_counts::ManagedPairCounts),
}
enum CountIter<'a> {
    Legacy(std::collections::hash_map::Iter<'a, (u64, u64), usize>),
    #[cfg(feature = "decode")]
    Managed(crate::managed_pair_counts::PairCountIter<'a>),
}
impl Iterator for CountIter<'_> {
    type Item = ((u64, u64), usize);
    fn next(&mut self) -> Option<Self::Item> {
        match self {
            Self::Legacy(v) => v.next().map(|(pair, count)| (*pair, *count)),
            #[cfg(feature = "decode")]
            Self::Managed(v) => v.next(),
        }
    }
}
impl FilePairCounts {
    fn iter(&self) -> CountIter<'_> {
        match self {
            Self::Legacy(v) => CountIter::Legacy(v.iter()),
            #[cfg(feature = "decode")]
            Self::Managed(v) => CountIter::Managed(v.iter()),
        }
    }
}
fn descriptor_file_pairs_with_slot_storage<B, S>(
    files: impl IntoIterator<Item = (u64, B)>,
    max_distance: u32,
    minimum_shared_features: usize,
    budgets: FileFeatureBudgets,
    cancel: impl Fn() -> bool,
    build_slots: impl FnOnce(&[(u64, B)], usize, &dyn Fn() -> bool) -> Result<S, LocalError>,
    build_index: impl FnOnce(&[(u64, B)], usize, &dyn Fn() -> bool) -> Result<LocalDescriptorIndex, LocalError>,
    build_targets: impl FnOnce(usize) -> Result<FileTargets, LocalError>,
    build_counts: impl FnOnce(usize) -> Result<FilePairCounts, LocalError>,
) -> Result<DescriptorFilePairs, LocalError>
where
    B: std::ops::Deref<Target = [Feature]>,
    S: std::ops::Deref<Target = [u64]>,
{
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
    let mut insufficient = Vec::new();
    for (id, features) in &documents {
        let id = *id;
        if features.len() < minimum_shared_features {
            reserve_slot(&mut insufficient, budgets.max_files)?;
            insufficient.push(id);
        }
    }
    let slots = build_slots(&documents, count, &cancel)?;
    u64::try_from(count).map_err(|_| LocalError::Budget)?;
    let index = build_index(&documents, count, &cancel)?;
    let mut targets = build_targets(documents.len())?;
    let mut hits = 0usize;
    let mut counts = build_counts(documents.len())?;
    for (left, features) in &documents {
        let left = *left;
        for feature in features.iter() {
            let found = index.search_retrieval(
                feature,
                max_distance,
                budgets.max_hits.saturating_sub(hits),
                &cancel,
            )?;
            hits = hits.checked_add(found.len()).ok_or(LocalError::Budget)?;
            #[cfg(feature = "decode")]
            if let FileTargets::Managed(storage) = &mut targets {
                storage.begin()?;
            }
            for candidate in found.iter() {
                if cancel() {
                    return Err(LocalError::Cancelled);
                }
                let slot = usize::try_from(candidate.id).map_err(|_| LocalError::Budget)?;
                let right = slots[slot];
                if right > left {
                    match &mut targets {
                        FileTargets::Legacy(set) => {
                            if !set.contains(&right) {
                                set.try_reserve(1).map_err(|_| LocalError::Budget)?;
                                set.insert(right);
                            }
                        }
                        #[cfg(feature = "decode")]
                        FileTargets::Managed(storage) => {
                            let ordinal = documents
                                .binary_search_by_key(&right, |(id, _)| *id)
                                .map_err(|_| LocalError::Invalid)?;
                            storage.insert(ordinal, right)?;
                        }
                    }
                }
            }
            match &mut targets {
                FileTargets::Legacy(set) => {
                    count_targets(std::mem::take(set), left, &mut counts, budgets.max_pair_counts)?
                }
                #[cfg(feature = "decode")]
                FileTargets::Managed(storage) => count_ordered_targets(
                    storage.ordered().iter().copied(),
                    left,
                    &mut counts,
                    budgets.max_pair_counts,
                    &cancel,
                )?,
            }
        }
    }
    let mut pairs = Vec::new();
    for (pair, shared) in counts.iter() {
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
    counts: &mut FilePairCounts,
    max_pair_counts: usize,
) -> Result<(), LocalError> {
    let mut ordered_targets = Vec::new();
    ordered_targets
        .try_reserve_exact(targets.len())
        .map_err(|_| LocalError::Budget)?;
    ordered_targets.extend(targets);
    ordered_targets.sort_unstable();
    count_ordered_targets(ordered_targets, left, counts, max_pair_counts, || false)
}
fn count_ordered_targets(
    targets: impl IntoIterator<Item = u64>,
    left: u64,
    counts: &mut FilePairCounts,
    max_pair_counts: usize,
    cancel: impl Fn() -> bool,
) -> Result<(), LocalError> {
    for right in targets {
        match counts {
            #[cfg(feature = "decode")]
            FilePairCounts::Managed(storage) => {
                storage.add(left, right, &cancel)?;
            }
            FilePairCounts::Legacy(counts) => {
                if !counts.contains_key(&(left, right)) {
                    if counts.len() >= max_pair_counts {
                        return Err(LocalError::Budget);
                    }
                    counts.try_reserve(1).map_err(|_| LocalError::Budget)?;
                }
                let shared = counts.entry((left, right)).or_insert(0usize);
                *shared = shared.checked_add(1).ok_or(LocalError::Budget)?;
            }
        }
    }
    Ok(())
}

#[cfg(all(test, feature = "decode"))]
mod managed_owner_slots_tests {
    use super::*;
    use rrrah_core::MemoryBudget;
    use std::cell::Cell;
    #[test]
    fn binary_owner_table_parity_credit_lifetime_and_refusals() {
        let f = Feature {
            recipe: crate::local::FeatureRecipe::OrientedBriefV1,
            position: [0.; 2],
            descriptor: [7; 4],
            quarter_turns: [[7; 4]; 3],
        };
        let files = [30, 10, 20].map(|id| (id, vec![f; 2]));
        let limits = FileFeatureBudgets {
            max_files: 3,
            max_features: 6,
            max_hits: 36,
            max_pair_counts: 3,
            max_pairs: 3,
        };
        let expected = descriptor_file_pairs(files.clone(), 0, 1, limits, || false).unwrap();
        let required = 48 + 6 * std::mem::size_of::<(u64, Fingerprint)>() as u64;
        let budget = MemoryBudget::new(1024 * 1024);
        let observed = Cell::new(false);
        let calls = Cell::new(0usize);
        let actual = descriptor_file_pairs_shared_managed(files.clone(), 0, 1, limits, &budget, || {
            calls.set(calls.get() + 1);
            if budget.used() > 0 {
                observed.set(true);
            }
            false
        })
        .unwrap();
        assert_eq!(actual.pairs, expected.pairs);
        assert_eq!(actual.indexed_features, expected.indexed_features);
        assert_eq!(actual.descriptor_hits, expected.descriptor_hits);
        assert_eq!(actual.insufficient_features, expected.insufficient_features);
        assert!(observed.get());
        assert!(budget.peak() > required);
        assert_eq!(budget.used(), 0);
        let total = calls.get();
        let short = MemoryBudget::new(47);
        assert!(matches!(
            descriptor_file_pairs_shared_managed(files.clone(), 0, 1, limits, &short, || false),
            Err(LocalError::Budget)
        ));
        assert_eq!(short.peak(), 0);
        assert_eq!(short.used(), 0);
        let staging_short = MemoryBudget::new(required - 1);
        assert!(matches!(
            descriptor_file_pairs_shared_managed(files.clone(), 0, 1, limits, &staging_short, || false),
            Err(LocalError::Budget)
        ));
        assert_eq!(staging_short.peak(), 48);
        assert_eq!(staging_short.used(), 0);
        let mut mixed = files.clone();
        mixed[0].1[0].recipe = FeatureRecipe::OrientedScaleBriefV1;
        assert!(matches!(
            descriptor_file_pairs_shared_managed(mixed, 0, 1, limits, &budget, || false),
            Err(LocalError::Invalid)
        ));
        assert_eq!(budget.used(), 0);
        let exact = MemoryBudget::new(budget.peak());
        descriptor_file_pairs_shared_managed(files.clone(), 0, 1, limits, &exact, || false).unwrap();
        assert_eq!(exact.used(), 0);
        let index_short = MemoryBudget::new(budget.peak() - 1);
        assert!(matches!(
            descriptor_file_pairs_shared_managed(files.clone(), 0, 1, limits, &index_short, || false),
            Err(LocalError::Budget)
        ));
        assert_eq!(index_short.used(), 0);
        for stop in [1, total / 2, total] {
            calls.set(0);
            assert!(matches!(
                descriptor_file_pairs_shared_managed(files.clone(), 0, 1, limits, &budget, || {
                    calls.set(calls.get() + 1);
                    calls.get() == stop
                }),
                Err(LocalError::Cancelled)
            ));
            assert_eq!(budget.used(), 0);
        }
        let empty = MemoryBudget::new(0);
        let result = descriptor_file_pairs_shared_managed(
            Vec::<(u64, Vec<Feature>)>::new(),
            0,
            1,
            limits,
            &empty,
            || false,
        )
        .unwrap();
        assert!(result.pairs.is_empty());
        assert_eq!(empty.peak(), 0);
        let duplicate = MemoryBudget::new(0);
        assert!(matches!(
            descriptor_file_pairs_shared_managed(
                [(1, vec![f]), (1, vec![f])],
                0,
                1,
                limits,
                &duplicate,
                || false
            ),
            Err(LocalError::Invalid)
        ));
        assert_eq!(duplicate.peak(), 0);
    }
}
