//! Sparse managed union of the exact per-variant visual retrieval results.
use crate::{
    local::{Feature, LocalError},
    local_index::DescriptorCandidate,
    managed_visual::ManagedVisualIndex,
    pixels::Fingerprint,
    visual::VisualError,
};
use rrrah_core::{MemoryBudget, SharedBuffer};

fn mapped(error: VisualError) -> LocalError {
    match error {
        VisualError::Cancelled => LocalError::Cancelled,
        VisualError::Budget => LocalError::Budget,
        VisualError::DuplicateId => LocalError::Invalid,
    }
}
/// Keeps the minimum distance for each opaque ID, then sorts by (distance,id).
/// Allocates only for actual hits, with credit retained by returned storage.
pub(crate) fn search(
    index: &ManagedVisualIndex,
    query: &Feature,
    radius: u32,
    max_results: usize,
    budget: &MemoryBudget,
    cancel: impl Fn() -> bool,
) -> Result<SharedBuffer<DescriptorCandidate>, LocalError> {
    if cancel() {
        return Err(LocalError::Cancelled);
    }
    let mut credit = budget.try_reserve(0).map_err(|_| LocalError::Budget)?;
    let mut union: Vec<DescriptorCandidate> = Vec::new();
    let descriptors = std::iter::once(&query.descriptor).chain(query.quarter_turns.iter().take(
        if matches!(
            query.recipe,
            crate::local::FeatureRecipe::OrientedScaleBriefV1
                | crate::local::FeatureRecipe::RankOrientedScaleBriefV1
        ) {
            3
        } else {
            0
        },
    ));
    for descriptor in descriptors {
        let fingerprint = Fingerprint {
            variants: [*descriptor; 8],
            mean_linear_rgb: [0.; 3],
            luminance_stddev: 1.,
        };
        let hits = index
            .search(&fingerprint, radius, true, budget, &cancel)
            .map_err(mapped)?;
        let length = union.len().checked_add(hits.len()).ok_or(LocalError::Budget)?;
        let bytes = length
            .checked_mul(std::mem::size_of::<DescriptorCandidate>())
            .and_then(|n| u64::try_from(n).ok())
            .ok_or(LocalError::Budget)?;
        credit.ensure_bytes(bytes).map_err(|_| LocalError::Budget)?;
        union
            .try_reserve_exact(hits.len())
            .map_err(|_| LocalError::Budget)?;
        let actual = union
            .capacity()
            .checked_mul(std::mem::size_of::<DescriptorCandidate>())
            .and_then(|n| u64::try_from(n).ok())
            .ok_or(LocalError::Budget)?;
        credit.ensure_bytes(actual).map_err(|_| LocalError::Budget)?;
        for hit in hits.iter() {
            if cancel() {
                return Err(LocalError::Cancelled);
            }
            union.push(DescriptorCandidate {
                id: hit.id,
                distance: hit.evidence.distance,
            });
        }
    }
    union.sort_unstable_by_key(|v| (v.id, v.distance));
    let mut unique = 0;
    for read in 0..union.len() {
        if cancel() {
            return Err(LocalError::Cancelled);
        }
        if unique != 0 && union[read].id == union[unique - 1].id {
            continue;
        }
        if unique == max_results {
            return Err(LocalError::Budget);
        }
        union[unique] = union[read];
        unique += 1;
    }
    union.truncate(unique);
    union.sort_unstable_by_key(|v| (v.distance, v.id));
    if cancel() {
        return Err(LocalError::Cancelled);
    }
    credit.try_adopt(union).map_err(|_| LocalError::Budget)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::local::FeatureRecipe;
    use std::cell::Cell;
    #[test]
    fn sparse_union_equals_independent_minimum_distance_and_releases() {
        let features: Vec<_> = (0u64..20)
            .map(|i| {
                let hash = i.wrapping_mul(0x9e3779b97f4a7c15);
                Feature {
                    recipe: FeatureRecipe::OrientedScaleBriefV1,
                    position: [0.; 2],
                    descriptor: [hash; 4],
                    quarter_turns: [[hash.rotate_left(9); 4], [hash ^ 255; 4], [!hash; 4]],
                }
            })
            .collect();
        let entries: Vec<_> = features
            .iter()
            .enumerate()
            .map(|(i, f)| {
                let mut variants = [f.descriptor; 8];
                variants[1..4].copy_from_slice(&f.quarter_turns);
                (
                    if i == 19 { u64::MAX } else { i as u64 * 3 },
                    Fingerprint {
                        variants,
                        mean_linear_rgb: [0.; 3],
                        luminance_stddev: 1.,
                    },
                )
            })
            .collect();
        let budget = MemoryBudget::new(1024 * 1024);
        let credit = budget
            .try_reserve((entries.capacity() * std::mem::size_of::<(u64, Fingerprint)>()) as u64)
            .unwrap();
        let index = ManagedVisualIndex::new(entries, credit, &budget, || false).unwrap();
        let retained = budget.used();
        for query in features.iter().step_by(4) {
            for radius in [0, 1, 64, 128, 256, u32::MAX] {
                let mut expected: Vec<_> = features
                    .iter()
                    .enumerate()
                    .filter_map(|(i, other)| {
                        let distance = std::iter::once(query.descriptor)
                            .chain(query.quarter_turns)
                            .flat_map(|q| {
                                std::iter::once(other.descriptor)
                                    .chain(other.quarter_turns)
                                    .map(move |r| {
                                        q.iter().zip(r).map(|(a, b)| (a ^ b).count_ones()).sum::<u32>()
                                    })
                            })
                            .min()
                            .unwrap();
                        (distance <= radius.min(256)).then_some(DescriptorCandidate {
                            id: if i == 19 { u64::MAX } else { i as u64 * 3 },
                            distance,
                        })
                    })
                    .collect();
                expected.sort_unstable_by_key(|v| (v.distance, v.id));
                let actual = search(&index, query, radius, 20, &budget, || false).unwrap();
                assert_eq!(&*actual, expected.as_slice());
                let shared = actual.clone();
                drop(actual);
                assert!(budget.used() > retained);
                drop(shared);
                assert_eq!(budget.used(), retained);
                if !expected.is_empty() {
                    assert!(matches!(
                        search(&index, query, radius, expected.len() - 1, &budget, || false),
                        Err(LocalError::Budget)
                    ));
                    assert_eq!(budget.used(), retained);
                }
            }
        }
        let calls = Cell::new(0usize);
        drop(
            search(&index, &features[0], 256, 20, &budget, || {
                calls.set(calls.get() + 1);
                false
            })
            .unwrap(),
        );
        let total = calls.get();
        for stop in 1..=total {
            calls.set(0);
            assert!(matches!(
                search(&index, &features[0], 256, 20, &budget, || {
                    calls.set(calls.get() + 1);
                    calls.get() == stop
                }),
                Err(LocalError::Cancelled)
            ));
            assert_eq!(budget.used(), retained);
        }
        let empty = MemoryBudget::new(0);
        assert!(matches!(
            search(&index, &features[0], 256, 20, &empty, || false),
            Err(LocalError::Budget)
        ));
        assert_eq!(empty.used(), 0);
        drop(index);
        assert_eq!(budget.used(), 0);
    }
}
