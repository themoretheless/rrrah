//! Guarded exhaustive pair collection and managed result ownership.
use crate::{
    decode::{DecodeDependenciesSnapshot, DecodeSourceSnapshot},
    exact::{ContentSnapshot, SnapshotError},
};
use rrrah_core::{BufferError, MemoryBudget, Reservation, SharedBuffer};
use rrrah_decode::DecodeRequest;
use std::{cell::Cell, path::Path};

#[derive(Debug, Clone, Copy)]
pub(crate) struct CollectionSettings {
    pub max_files: usize,
    pub max_pairs: usize,
    pub max_model_bytes: u64,
    pub max_file_bytes: u64,
}
#[derive(Debug)]
pub struct CollectionPair<T> {
    pub left: u64,
    pub right: u64,
    pub evidence: T,
}
#[derive(Debug)]
pub struct CollectionReport<T> {
    ids: SharedBuffer<u64>,
    pairs: Slots<CollectionPair<T>>,
}
impl<T> CollectionReport<T> {
    pub fn file_ids(&self) -> &[u64] {
        &self.ids
    }
    /// Every unordered pair in lexicographic ID order, including unsupported
    /// results. Visual support is nontransitive; no duplicate groups are inferred.
    pub fn pairs(&self) -> &[CollectionPair<T>] {
        &self.pairs.values
    }
}
#[derive(Debug, thiserror::Error)]
pub enum CollectionError<E> {
    #[error("collection search cancelled")]
    Cancelled,
    #[error("collection file limit exceeded")]
    Files,
    #[error("complete collection pair enumeration exceeds pair limit")]
    Pairs,
    #[error("duplicate application id {0}")]
    DuplicateId(u64),
    #[error("collection cumulative work refusal")]
    Work,
    #[error(transparent)]
    Policy(E),
    #[error(transparent)]
    Memory(#[from] BufferError),
    #[error("model source: {0}")]
    ModelSource(#[source] SnapshotError),
    #[error("file {id}: {error}")]
    Source {
        id: u64,
        #[source]
        error: SnapshotError,
    },
    #[error("pair ({left}, {right}): {error}")]
    Pair {
        left: u64,
        right: u64,
        #[source]
        error: E,
    },
}

// Private owned slots keep credit inseparable from non-Copy evidence. Returning
// only a borrowed slice prevents moving the Vec out while dropping its credit.
#[derive(Debug)]
struct Slots<T> {
    values: Vec<T>,
    limit: usize,
    _credit: Reservation,
}
fn bytes<T>(capacity: usize) -> Result<u64, BufferError> {
    capacity
        .checked_mul(std::mem::size_of::<T>())
        .and_then(|v| u64::try_from(v).ok())
        .ok_or(BufferError::Overflow)
}
impl<T> Slots<T> {
    fn new(limit: usize, budget: &MemoryBudget) -> Result<Self, BufferError> {
        let mut credit = budget.try_reserve(bytes::<T>(limit)?)?;
        let mut values = Vec::new();
        values.try_reserve_exact(limit).map_err(BufferError::Allocate)?;
        credit.ensure_bytes(bytes::<T>(values.capacity())?)?;
        Ok(Self {
            values,
            limit,
            _credit: credit,
        })
    }
    fn push<E>(&mut self, value: T) -> Result<(), CollectionError<E>> {
        if self.values.len() >= self.limit {
            return Err(CollectionError::Work);
        }
        self.values.push(value);
        Ok(())
    }
}

fn stopped(requests: &[(u64, DecodeRequest)], cancel: &dyn Fn() -> bool, latch: &Cell<bool>) -> bool {
    let stop = latch.get()
        || cancel()
        || requests.iter().any(|(_, r)| {
            r.cancellation
                .as_ref()
                .is_some_and(rrrah_decode::GenerationToken::is_cancelled)
        });
    latch.set(stop);
    stop
}
fn owners_stopped(requests: &[(u64, DecodeRequest)], latch: &Cell<bool>) -> bool {
    let stop = latch.get()
        || requests.iter().any(|(_, r)| {
            r.cancellation
                .as_ref()
                .is_some_and(rrrah_decode::GenerationToken::is_cancelled)
        });
    latch.set(stop);
    stop
}

trait Observation: Sized {
    const EARLY_MODEL: bool;
    fn read(path: &Path, max_bytes: u64, cancel: &dyn Fn() -> bool) -> Result<Self, SnapshotError>;
    fn verify(&self, cancel: &dyn Fn() -> bool) -> Result<(), SnapshotError>;
    fn identical(&self, other: &Self, cancel: &dyn Fn() -> bool) -> Result<bool, (bool, SnapshotError)>;
    fn verified_identical(&self, other: &Self, cancel: &dyn Fn() -> bool) -> Result<bool, (bool, SnapshotError)> {
        self.verify(cancel).map_err(|e| (true, e))?;
        other.verify(cancel).map_err(|e| (false, e))?;
        self.identical(other, cancel)
    }
    fn activate(
        &mut self,
        path: &Path,
        max_bytes: u64,
        cancel: &dyn Fn() -> bool,
    ) -> Result<(), SnapshotError>;
}
impl Observation for DecodeSourceSnapshot {
    const EARLY_MODEL: bool = true;
    fn read(path: &Path, max_bytes: u64, cancel: &dyn Fn() -> bool) -> Result<Self, SnapshotError> {
        Self::read(path, max_bytes, cancel)
    }
    fn verify(&self, cancel: &dyn Fn() -> bool) -> Result<(), SnapshotError> {
        self.verify(cancel)
    }
    fn identical(&self, _: &Self, _: &dyn Fn() -> bool) -> Result<bool, (bool, SnapshotError)> {
        Ok(false)
    }
    fn activate(&mut self, _: &Path, _: u64, _: &dyn Fn() -> bool) -> Result<(), SnapshotError> {
        Ok(())
    }
}
struct ExactObservation {
    original: ContentSnapshot,
    visual: Option<DecodeDependenciesSnapshot>,
}
impl Observation for ExactObservation {
    const EARLY_MODEL: bool = false;
    fn read(path: &Path, max_bytes: u64, cancel: &dyn Fn() -> bool) -> Result<Self, SnapshotError> {
        Ok(Self {
            original: ContentSnapshot::read(path, max_bytes, cancel)?,
            visual: None,
        })
    }
    fn verify(&self, cancel: &dyn Fn() -> bool) -> Result<(), SnapshotError> {
        self.original.verify(cancel)?;
        if let Some(visual) = &self.visual {
            visual.verify(cancel)?;
        }
        Ok(())
    }
    fn identical(&self, other: &Self, cancel: &dyn Fn() -> bool) -> Result<bool, (bool, SnapshotError)> {
        crate::exact::compare_observed_tagged(&self.original, &other.original, &cancel)
    }
    fn verified_identical(&self, other: &Self, cancel: &dyn Fn() -> bool) -> Result<bool, (bool, SnapshotError)> {
        #[cfg(test)]
        if !BENCH_SELECTIVE_PREVERIFY.with(Cell::get) {
            self.verify(cancel).map_err(|e| (true, e))?;
            other.verify(cancel).map_err(|e| (false, e))?;
            return self.identical(other, cancel);
        }
        // Preserve dependency verification order once visual decoding has activated.
        if self.visual.is_none() && other.visual.is_none() {
            crate::exact::selective_verified_pair(&self.original, &other.original, cancel)
        } else {
            self.verify(cancel).map_err(|e| (true, e))?;
            other.verify(cancel).map_err(|e| (false, e))?;
            self.identical(other, cancel)
        }
    }
    fn activate(
        &mut self,
        path: &Path,
        max_bytes: u64,
        cancel: &dyn Fn() -> bool,
    ) -> Result<(), SnapshotError> {
        if self.visual.is_none() {
            self.visual = Some(DecodeDependenciesSnapshot::read_verified(
                &self.original,
                path,
                max_bytes,
                cancel,
            )?);
        }
        Ok(())
    }
}
fn verify_file<S: Observation, E>(
    snapshots: &Slots<S>,
    requests: &Slots<(u64, DecodeRequest)>,
    index: usize,
    cancel: &dyn Fn() -> bool,
) -> Result<(), CollectionError<E>> {
    snapshots.values[index]
        .verify(cancel)
        .map_err(|error| CollectionError::Source {
            id: requests.values[index].0,
            error,
        })
}

/// Common engine: adapters preflight every constituent before iterator consumption.
/// Every pair and all sources are guarded; errors discard all earlier evidence.
#[allow(clippy::too_many_arguments)]
pub(crate) fn scan_guarded_collection<T, E>(
    files: impl IntoIterator<Item = (u64, DecodeRequest)>,
    weights: &Path,
    preflight: impl FnOnce(&dyn Fn() -> bool) -> Result<CollectionSettings, CollectionError<E>>,
    budget: &MemoryBudget,
    compare: impl FnMut(&DecodeRequest, &DecodeRequest, &dyn Fn() -> bool) -> Result<T, E>,
    cancel: impl Fn() -> bool,
) -> Result<CollectionReport<T>, CollectionError<E>> {
    scan_guarded_collection_mode::<_, _, DecodeSourceSnapshot>(
        files, weights, preflight, budget, compare, None, None, cancel,
    )
}

/// Exact collections defer dependency and weight observations until visual work.
#[allow(clippy::too_many_arguments)]
pub(crate) fn scan_guarded_exact_collection<T, E>(
    files: impl IntoIterator<Item = (u64, DecodeRequest)>,
    weights: &Path,
    preflight: impl FnOnce(&dyn Fn() -> bool) -> Result<CollectionSettings, CollectionError<E>>,
    budget: &MemoryBudget,
    compare: impl FnMut(&DecodeRequest, &DecodeRequest, &dyn Fn() -> bool) -> Result<T, E>,
    exact: impl Fn() -> T,
    cancel: impl Fn() -> bool,
) -> Result<CollectionReport<T>, CollectionError<E>> {
    scan_guarded_collection_mode::<_, _, ExactObservation>(
        files,
        weights,
        preflight,
        budget,
        compare,
        Some(&exact),
        None,
        cancel,
    )
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn scan_guarded_collection_ordered<T, E>(
    files: impl IntoIterator<Item = (u64, DecodeRequest)>,
    weights: &Path,
    preflight: impl FnOnce(&dyn Fn() -> bool) -> Result<CollectionSettings, CollectionError<E>>,
    budget: &MemoryBudget,
    compare: impl FnMut(&DecodeRequest, &DecodeRequest, &dyn Fn() -> bool) -> Result<T, E>,
    order: Option<&crate::pair_schedule::CompletePairSchedule>,
    cancel: impl Fn() -> bool,
) -> Result<CollectionReport<T>, CollectionError<E>> {
    scan_guarded_collection_mode::<_, _, DecodeSourceSnapshot>(
        files, weights, preflight, budget, compare, None, order, cancel,
    )
}

#[allow(clippy::too_many_arguments)]
fn scan_guarded_collection_mode<T, E, S: Observation>(
    files: impl IntoIterator<Item = (u64, DecodeRequest)>,
    weights: &Path,
    preflight: impl FnOnce(&dyn Fn() -> bool) -> Result<CollectionSettings, CollectionError<E>>,
    budget: &MemoryBudget,
    mut compare: impl FnMut(&DecodeRequest, &DecodeRequest, &dyn Fn() -> bool) -> Result<T, E>,
    exact: Option<&dyn Fn() -> T>,
    order: Option<&crate::pair_schedule::CompletePairSchedule>,
    cancel: impl Fn() -> bool,
) -> Result<CollectionReport<T>, CollectionError<E>> {
    let latch = Cell::new(false);
    let global = || stopped(&[], &cancel, &latch);
    let admitted = (|| {
        if global() {
            return Err(CollectionError::Cancelled);
        }
        let policy = preflight(&global)?;
        let requests = Slots::new(policy.max_files, budget)?;
        let snapshots = Slots::<S>::new(policy.max_files, budget)?;
        let pairs = Slots::new(policy.max_pairs, budget)?;
        Ok((policy, requests, snapshots, pairs))
    })();
    let (policy, mut requests, snapshots, pairs) = match admitted {
        Ok(v) => v,
        Err(error) => {
            return if global() {
                Err(CollectionError::Cancelled)
            } else {
                Err(error)
            };
        }
    };
    if global() {
        return Err(CollectionError::Cancelled);
    }
    let mut iterator = files.into_iter();
    let admission = admit(&mut iterator, &mut requests, policy.max_files, &global);
    drop(iterator);
    if stopped(&requests.values, &global, &latch) {
        return Err(CollectionError::Cancelled);
    }
    admission?;
    // Erase provider/callback types before the heavy pair pipeline.
    scan(
        weights,
        policy,
        budget,
        &mut compare,
        exact,
        order,
        &global,
        requests,
        snapshots,
        pairs,
    )
}

fn admit<E>(
    files: &mut dyn Iterator<Item = (u64, DecodeRequest)>,
    requests: &mut Slots<(u64, DecodeRequest)>,
    max_files: usize,
    cancel: &dyn Fn() -> bool,
) -> Result<(), CollectionError<E>> {
    let latch = Cell::new(false);
    loop {
        if stopped(&requests.values, cancel, &latch) {
            return Err(CollectionError::Cancelled);
        }
        let next = files.next();
        if stopped(&requests.values, cancel, &latch) {
            return Err(CollectionError::Cancelled);
        }
        let Some(request) = next else {
            return Ok(());
        };
        if requests.values.len() >= max_files {
            return Err(CollectionError::Files);
        }
        requests.push(request)?;
    }
}

#[allow(clippy::too_many_arguments)]
fn scan<T, E, S: Observation>(
    weights: &Path,
    policy: CollectionSettings,
    budget: &MemoryBudget,
    compare: &mut dyn FnMut(&DecodeRequest, &DecodeRequest, &dyn Fn() -> bool) -> Result<T, E>,
    exact: Option<&dyn Fn() -> T>,
    order: Option<&crate::pair_schedule::CompletePairSchedule>,
    cancel: &dyn Fn() -> bool,
    mut requests: Slots<(u64, DecodeRequest)>,
    mut snapshots: Slots<S>,
    mut pairs: Slots<CollectionPair<T>>,
) -> Result<CollectionReport<T>, CollectionError<E>> {
    let latch = Cell::new(false);
    if stopped(&requests.values, cancel, &latch) {
        return Err(CollectionError::Cancelled);
    }
    // Sorting uses no extra heap; cancellation is checked on either side.
    requests.values.sort_unstable_by_key(|(id, _)| *id);
    let cancelled = || stopped(&requests.values, cancel, &latch);
    let result = (|| {
        if cancelled() {
            return Err(CollectionError::Cancelled);
        }
        for pair in requests.values.windows(2) {
            if cancelled() {
                return Err(CollectionError::Cancelled);
            }
            if pair[0].0 == pair[1].0 {
                return Err(CollectionError::DuplicateId(pair[0].0));
            }
        }
        let n = requests.values.len();
        let count = n
            .checked_mul(n.saturating_sub(1))
            .map(|v| v / 2)
            .ok_or(CollectionError::Pairs)?;
        if count > policy.max_pairs {
            return Err(CollectionError::Pairs);
        }
        if let Some(schedule) = order {
            if schedule.file_ids().len() != n || schedule.total_pairs() != count as u64 {
                return Err(CollectionError::Pairs);
            }
            for (id, (request_id, _)) in schedule.file_ids().iter().zip(&requests.values) {
                if cancelled() {
                    return Err(CollectionError::Cancelled);
                }
                if id != request_id {
                    return Err(CollectionError::Pairs);
                }
            }
        }
        let mut model = if S::EARLY_MODEL {
            Some(
                ContentSnapshot::read(weights, policy.max_model_bytes, &cancelled)
                    .map_err(CollectionError::ModelSource)?,
            )
        } else {
            None
        };
        let max_bytes = policy.max_file_bytes;
        let mut ids = budget.try_buffer(n, 0u64)?;
        for (index, (id, request)) in requests.values.iter().enumerate() {
            if cancelled() {
                return Err(CollectionError::Cancelled);
            }
            snapshots.push(
                S::read(&request.path, max_bytes, &cancelled)
                    .map_err(|error| CollectionError::Source { id: *id, error })?,
            )?;
            ids[index] = *id;
        }
        for i in 0..n {
            verify_file(&snapshots, &requests, i, &cancelled)?;
        }
        let callback: &dyn Fn() -> bool = &cancelled;
        let mut ordinary = (0..n).flat_map(|i| (i + 1..n).map(move |j| Ok((i, j))));
        let mut scheduled = order.map(|schedule| {
            schedule.pairs(callback).map(|pair| {
                let pair = pair.map_err(|_| CollectionError::Cancelled)?;
                let i = requests
                    .values
                    .binary_search_by_key(&pair.left, |v| v.0)
                    .map_err(|_| CollectionError::Pairs)?;
                let j = requests
                    .values
                    .binary_search_by_key(&pair.right, |v| v.0)
                    .map_err(|_| CollectionError::Pairs)?;
                Ok((i, j))
            })
        });
        let iterator: &mut dyn Iterator<Item = Result<(usize, usize), CollectionError<E>>> =
            if let Some(scheduled) = &mut scheduled {
                scheduled
            } else {
                &mut ordinary
            };
        for pair in iterator {
            let (i, j) = pair?;
            let (left, l) = &requests.values[i];
            let (right, r) = &requests.values[j];
            let identical = snapshots.values[i]
                .verified_identical(&snapshots.values[j], &cancelled)
                .map_err(|(is_left, error)| CollectionError::Source {
                    id: if is_left { *left } else { *right },
                    error,
                })?;
            let evidence = if identical {
                exact.expect("exact observation has an evidence factory")()
            } else {
                if model.is_none() {
                    // Guard all files, including idle files, before provider code.
                    for (index, (_, request)) in requests.values.iter().enumerate() {
                        snapshots.values[index]
                            .activate(&request.path, max_bytes, &cancelled)
                            .map_err(|error| CollectionError::Source {
                                id: requests.values[index].0,
                                error,
                            })?;
                    }
                    model = Some(
                        ContentSnapshot::read(weights, policy.max_model_bytes, &cancelled)
                            .map_err(CollectionError::ModelSource)?,
                    );
                    for k in 0..n {
                        verify_file(&snapshots, &requests, k, &cancelled)?;
                    }
                }
                model
                    .as_ref()
                    .expect("visual weights")
                    .verify(&cancelled)
                    .map_err(CollectionError::ModelSource)?;
                compare(l, r, callback).map_err(|error| CollectionError::Pair {
                    left: *left,
                    right: *right,
                    error,
                })?
            };
            verify_file(&snapshots, &requests, i, &cancelled)?;
            verify_file(&snapshots, &requests, j, &cancelled)?;
            if let Some(model) = &model {
                model.verify(&cancelled).map_err(CollectionError::ModelSource)?;
            }
            pairs.push(CollectionPair {
                left: *left,
                right: *right,
                evidence,
            })?;
        }
        if order.is_some() {
            pairs.values.sort_unstable_by_key(|p| (p.left, p.right));
        }
        for i in 0..n {
            verify_file(&snapshots, &requests, i, &cancelled)?;
        }
        if let Some(model) = &model {
            model.verify(&cancelled).map_err(CollectionError::ModelSource)?;
        }
        Ok((
            CollectionReport {
                ids: ids.freeze(),
                pairs,
            },
            model,
        ))
    })();
    // Last user-code checkpoint is the commit boundary. Revalidate without
    // invoking user callbacks, which could otherwise mutate an already-checked
    // source immediately before publication. Request generations stay live.
    if cancelled() {
        return Err(CollectionError::Cancelled);
    }
    let (report, model) = result?;
    let owner_cancelled = || owners_stopped(&requests.values, &latch);
    let verified = (|| {
        for (index, snapshot) in snapshots.values.iter().enumerate() {
            snapshot
                .verify(&owner_cancelled)
                .map_err(|error| CollectionError::Source {
                    id: requests.values[index].0,
                    error,
                })?;
        }
        if let Some(model) = model {
            model
                .verify(&owner_cancelled)
                .map_err(CollectionError::ModelSource)?;
        }
        Ok(report)
    })();
    if owner_cancelled() {
        Err(CollectionError::Cancelled)
    } else {
        verified
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    };
    #[test]
    fn final_generation_observation_stays_cancelled_if_owner_resets_counter() {
        for owner in 0..3 {
            let generation = Arc::new(AtomicU64::new(1));
            let mut requests: Vec<_> = (0..3).map(|id| (id, DecodeRequest::new("unused"))).collect();
            requests[owner].1.cancellation = Some(rrrah_decode::GenerationToken::new(generation.clone(), 1));
            let latch = Cell::new(false);
            assert!(!owners_stopped(&requests, &latch));
            generation.store(2, Ordering::Release);
            assert!(owners_stopped(&requests, &latch));
            generation.store(1, Ordering::Release);
            assert!(owners_stopped(&requests, &latch));
        }
    }
}

#[cfg(test)]
mod scheduled_guard_tests {
    use super::*;
    use crate::pair_schedule::CompletePairSchedule;
    fn run(
        files: Vec<(u64, DecodeRequest)>,
        weights: &Path,
        schedule: &CompletePairSchedule,
        budget: &MemoryBudget,
        compare: impl FnMut(&DecodeRequest, &DecodeRequest, &dyn Fn() -> bool) -> Result<(), std::io::Error>,
        cancel: impl Fn() -> bool,
    ) -> Result<CollectionReport<()>, CollectionError<std::io::Error>> {
        scan_guarded_collection_ordered(
            files,
            weights,
            |_| {
                Ok(CollectionSettings {
                    max_files: 3,
                    max_pairs: 3,
                    max_model_bytes: 1024,
                    max_file_bytes: 1024,
                })
            },
            budget,
            compare,
            Some(schedule),
            cancel,
        )
    }
    #[test]
    fn proposal_order_and_final_idle_file_guards_remain_authoritative() {
        let dir = tempfile::tempdir().unwrap();
        let budget = MemoryBudget::new(65536);
        let files: Vec<_> = [10, 20, 30]
            .into_iter()
            .map(|id| {
                let p = dir.path().join(format!("{id}.bin"));
                std::fs::write(&p, vec![id as u8; 64]).unwrap();
                (id, DecodeRequest::new(p))
            })
            .collect();
        let weights = dir.path().join("weights");
        std::fs::write(&weights, b"weights").unwrap();
        let schedule =
            CompletePairSchedule::new(&[30, 10, 20], &[(20, 30)], 3, 3, 1, &budget, || false).unwrap();
        let baseline = budget.used();
        for mutate in [false, true] {
            let mut seen = Vec::new();
            let result = run(
                files.clone(),
                &weights,
                &schedule,
                &budget,
                |left, right, _| {
                    let id = |request: &DecodeRequest| {
                        request
                            .path
                            .file_stem()
                            .unwrap()
                            .to_str()
                            .unwrap()
                            .parse::<u64>()
                            .unwrap()
                    };
                    seen.push((id(left), id(right)));
                    if mutate && seen.len() == 3 {
                        std::fs::write(&files[1].1.path, [99u8; 64])?;
                    }
                    Ok(())
                },
                || false,
            );
            assert_eq!(seen, vec![(20, 30), (10, 20), (10, 30)]);
            if mutate {
                assert!(matches!(result, Err(CollectionError::Source { id: 20, .. })));
            } else {
                let report = result.unwrap();
                assert_eq!(
                    report
                        .pairs()
                        .iter()
                        .map(|p| (p.left, p.right))
                        .collect::<Vec<_>>(),
                    vec![(10, 20), (10, 30), (20, 30)]
                );
                drop(report);
            }
            assert_eq!(budget.used(), baseline);
        }
        drop(schedule);
        assert_eq!(budget.used(), 0);
    }
    #[test]
    fn every_scheduled_collection_cancel_checkpoint_discards_and_retries() {
        let dir = tempfile::tempdir().unwrap();
        let budget = MemoryBudget::new(65536);
        let files: Vec<_> = [1, 2, 3]
            .into_iter()
            .map(|id| {
                let p = dir.path().join(format!("{id}.bin"));
                std::fs::write(&p, [id as u8; 8]).unwrap();
                (id, DecodeRequest::new(p))
            })
            .collect();
        let weights = dir.path().join("weights");
        std::fs::write(&weights, b"weights").unwrap();
        let schedule = CompletePairSchedule::new(&[1, 2, 3], &[(2, 3)], 3, 3, 1, &budget, || false).unwrap();
        let baseline = budget.used();
        let calls = Cell::new(0);
        drop(
            run(
                files.clone(),
                &weights,
                &schedule,
                &budget,
                |_, _, _| Ok(()),
                || {
                    calls.set(calls.get() + 1);
                    false
                },
            )
            .unwrap(),
        );
        for stop in 1..=calls.get() {
            let seen = Cell::new(0);
            let result = run(
                files.clone(),
                &weights,
                &schedule,
                &budget,
                |_, _, _| Ok(()),
                || {
                    seen.set(seen.get() + 1);
                    seen.get() == stop
                },
            );
            assert!(
                matches!(result, Err(CollectionError::Cancelled)),
                "checkpoint {stop}"
            );
            assert_eq!(budget.used(), baseline);
        }
        drop(run(files, &weights, &schedule, &budget, |_, _, _| Ok(()), || false).unwrap());
        assert_eq!(budget.used(), baseline);
        drop(schedule);
        assert_eq!(budget.used(), 0);
    }
}

#[cfg(test)]
std::thread_local! { static BENCH_SELECTIVE_PREVERIFY: Cell<bool> = const { Cell::new(true) }; }
#[cfg(test)]
mod actual_collection_benchmark {
    use super::*;
    #[test]
    #[ignore = "manual complete collection engine benchmark with synthetic provider"]
    fn complete_guarded_engine() {
        let variant=std::env::var("DEDUP_BENCH_VARIANT").unwrap();
        assert!(["baseline","candidate"].contains(&variant.as_str()));
        BENCH_SELECTIVE_PREVERIFY.with(|flag|flag.set(variant=="candidate"));
        let n: usize = std::env::var("DEDUP_BENCH_FILES").unwrap().parse().unwrap();
        let size: usize = std::env::var("DEDUP_BENCH_BYTES").unwrap().parse().unwrap();
        let iterations: usize = std::env::var("DEDUP_BENCH_ITERATIONS").unwrap().parse().unwrap();
        let mode = std::env::var("DEDUP_BENCH_CONTENT").unwrap();
        assert!((2..=24).contains(&n) && size <= 2*1024*1024 && (1..=1000).contains(&iterations));
        assert!(["equal", "early", "late", "sizes", "mixed"].contains(&mode.as_str()));
        assert!(mode == "equal" || size >= 128);
        let dir = tempfile::tempdir().unwrap();
        let contents: Vec<Vec<u8>> = (0..n).map(|i| {
            let mut bytes: Vec<u8> = (0..size).map(|j| (j.wrapping_mul(37)%251) as u8).collect();
            // Public synthetic BMP-like header; dependency observation only,
            // no assertion of real pixel decoding or provider matching quality.
            if size >= 128 {
                bytes[..54].fill(0); bytes[..2].copy_from_slice(b"BM");
                bytes[2..6].copy_from_slice(&(size as u32).to_le_bytes());
                bytes[10..14].copy_from_slice(&54u32.to_le_bytes());
                bytes[14..18].copy_from_slice(&40u32.to_le_bytes());
                bytes[18..22].copy_from_slice(&1i32.to_le_bytes());
                bytes[22..26].copy_from_slice(&1i32.to_le_bytes());
                bytes[26..28].copy_from_slice(&1u16.to_le_bytes());
                bytes[28..30].copy_from_slice(&24u16.to_le_bytes());
            }
            match mode.as_str() {
                "early" => bytes[54] ^= (i+1) as u8,
                "late" => bytes[size-1] ^= (i+1) as u8,
                "sizes" => bytes.extend(std::iter::repeat_n(71,i)),
                "mixed" => bytes[54] ^= (i%3) as u8,
                _ => {},
            }
            bytes
        }).collect();
        let oracle: Vec<Vec<bool>> = contents.iter().map(|a|contents.iter().map(|b|a==b).collect()).collect();
        let files: Vec<_> = contents.iter().enumerate().map(|(i,b)| {
            let p=dir.path().join(format!("{i}.bmp")); std::fs::write(&p,b).unwrap();
            (i as u64,DecodeRequest::new(p))
        }).collect();
        let weights=dir.path().join("weights");std::fs::write(&weights,b"synthetic provider").unwrap();
        let pair_count=n*(n-1)/2;
        let expected_visual: usize=(0..n).map(|i|(i+1..n).filter(|&j|!oracle[i][j]).count()).sum();
        let budget=MemoryBudget::new(64*1024*1024);
        let calls=Cell::new(0u64);let callback=||{calls.set(calls.get()+1);false};
        let cancel:&dyn Fn()->bool=std::hint::black_box(&callback);
        let timer=std::time::Instant::now();let mut accepted=0usize;let mut visual_calls=0usize;
        for _ in 0..iterations {
            let report=scan_guarded_exact_collection(files.clone(),&weights,
                |_|Ok(CollectionSettings{max_files:n,max_pairs:pair_count,max_model_bytes:1024,max_file_bytes:u64::MAX}),
                &budget,|a,b,_|->Result<bool,std::io::Error>{
                    let index=|r:&DecodeRequest|r.path.file_stem().unwrap().to_str().unwrap().parse::<usize>().unwrap();
                    visual_calls+=1;Ok(oracle[index(a)][index(b)])
                },||true,cancel).unwrap();
            assert_eq!(report.pairs().len(),pair_count);
            for pair in report.pairs(){assert_eq!(pair.evidence,oracle[pair.left as usize][pair.right as usize]);accepted+=1;}
            drop(report);assert_eq!(budget.used(),0);
        }
        assert_eq!(accepted,iterations*pair_count);assert_eq!(visual_calls,iterations*expected_visual);
        println!("DEDUP_ENGINE_NS={} ACCEPTED={accepted} VISUAL_CALLS={visual_calls} CHECKPOINTS={}",timer.elapsed().as_nanos(),calls.get());
    }
}
