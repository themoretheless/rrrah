//! Shared source/model lifetime and publication boundary for pair portfolios.
use crate::{
    decode::DecodeSourceSnapshot,
    exact::{ContentSnapshot, SnapshotError},
    model_tiles::owners_cancelled,
};
use rrrah_decode::{DecodeRequest, GenerationToken};
use std::{cell::Cell, marker::PhantomData, path::Path};

pub(crate) trait PairError {
    fn cancelled() -> Self;
    fn source(error: SnapshotError) -> Self;
}
pub(crate) struct PairContext<'a, E> {
    first: &'a DecodeSourceSnapshot,
    second: &'a DecodeSourceSnapshot,
    pub model: &'a ContentSnapshot,
    pub cancel: &'a dyn Fn() -> bool,
    error: PhantomData<E>,
}
impl<E: PairError> PairContext<'_, E> {
    pub fn verify(&self) -> Result<(), E> {
        self.first.verify(self.cancel).map_err(E::source)?;
        self.second.verify(self.cancel).map_err(E::source)?;
        self.model.verify(self.cancel).map_err(E::source)
    }
}
/// Return an inconclusive stage outcome as a nested result, so final source and
/// generation observations still precede its publication by the adapter.
#[allow(clippy::too_many_arguments)]
pub(crate) fn guarded_pair<T, E: PairError>(
    left: &DecodeRequest,
    right: &DecodeRequest,
    weights: &Path,
    max_source_bytes: u64,
    max_model_bytes: u64,
    preflight: impl FnOnce(&dyn Fn() -> bool) -> Result<(), E>,
    run: impl FnOnce(&PairContext<'_, E>) -> Result<T, E>,
    cancel: impl Fn() -> bool,
) -> Result<T, E> {
    let latch = Cell::new(false);
    let cancelled = || {
        let stop = latch.get()
            || cancel()
            || [left, right]
                .iter()
                .any(|r| r.cancellation.as_ref().is_some_and(GenerationToken::is_cancelled));
        if stop {
            latch.set(true);
        }
        stop
    };
    let result = (|| {
        if cancelled() {
            return Err(E::cancelled());
        }
        preflight(&cancelled)?;
        let first =
            DecodeSourceSnapshot::read(&left.path, max_source_bytes, &cancelled).map_err(E::source)?;
        let second =
            DecodeSourceSnapshot::read(&right.path, max_source_bytes, &cancelled).map_err(E::source)?;
        let model = ContentSnapshot::read(weights, max_model_bytes, &cancelled).map_err(E::source)?;
        let value = run(&PairContext {
            first: &first,
            second: &second,
            model: &model,
            cancel: &cancelled,
            error: PhantomData,
        })?;
        Ok((value, first, second, model))
    })();
    if cancelled() {
        return Err(E::cancelled());
    }
    let (value, first, second, model) = result?;
    // Last caller checkpoint is above. Only sticky owner observations occur now.
    let owners = || owners_cancelled(left, right, &latch);
    let verified = (|| {
        first.verify(&owners).map_err(E::source)?;
        second.verify(&owners).map_err(E::source)?;
        model.verify(&owners).map_err(E::source)?;
        Ok(value)
    })();
    if owners() { Err(E::cancelled()) } else { verified }
}
