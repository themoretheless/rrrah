//! Foreground-priority admission for expensive full-RAW work.
//!
//! Native entropy decoding is cooperatively cancelled at row boundaries. This
//! gate still prevents a foreground decode and the
//! speculative cache warmer from running concurrently, while keeping every
//! wait off the winit thread.  Foreground intent is published before its
//! worker waits for the permit, so queued prefetch can never jump the line.

use std::{
    sync::{
        Arc, Condvar, Mutex, MutexGuard,
        atomic::{AtomicU64, Ordering},
    },
    time::{Duration, Instant},
};

pub const PREFETCH_IDLE_DEBOUNCE: Duration = Duration::from_millis(200);
const CANCELLATION_POLL: Duration = Duration::from_millis(10);

#[derive(Debug)]
pub struct DecodeGate {
    state: Mutex<GateState>,
    changed: Condvar,
    speculative_generation: Arc<AtomicU64>,
    idle_debounce: Duration,
}

#[derive(Debug)]
struct GateState {
    busy: bool,
    foreground_epoch: u64,
    foreground_pending: bool,
    idle_since: Option<Instant>,
}

impl DecodeGate {
    pub fn new() -> Self {
        Self::with_idle_debounce(PREFETCH_IDLE_DEBOUNCE)
    }

    fn with_idle_debounce(idle_debounce: Duration) -> Self {
        Self {
            state: Mutex::new(GateState {
                busy: false,
                foreground_epoch: 0,
                foreground_pending: false,
                idle_since: None,
            }),
            changed: Condvar::new(),
            speculative_generation: Arc::new(AtomicU64::new(0)),
            idle_debounce,
        }
    }

    /// Publish foreground intent without waiting.  Dropping the returned
    /// ticket clears the priority only if no newer foreground request exists.
    pub fn request_foreground(self: &Arc<Self>) -> ForegroundTicket {
        let mut state = self.lock_state();
        state.foreground_epoch = state.foreground_epoch.wrapping_add(1);
        state.foreground_pending = true;
        let epoch = state.foreground_epoch;

        // Raw prefetch uses this same generation for its decoder cancellation
        // token.  Advancing it here makes ForegroundLoader independently able
        // to cancel speculative work; correctness does not depend on the UI
        // calling RawPrefetcher::begin_foreground first.
        self.speculative_generation.fetch_add(1, Ordering::AcqRel);
        drop(state);
        self.changed.notify_all();

        ForegroundTicket {
            gate: Arc::clone(self),
            epoch,
        }
    }

    pub fn speculative_generation(&self) -> Arc<AtomicU64> {
        Arc::clone(&self.speculative_generation)
    }

    /// Restart the quiet-period clock after the UI has observed completion.
    /// This complements the ticket's RAII fallback and guarantees that a
    /// cache warmer submitted from the completion event waits a full 200 ms.
    pub fn defer_prefetch(&self) {
        let mut state = self.lock_state();
        state.idle_since = Some(Instant::now());
        drop(state);
        self.changed.notify_all();
    }

    pub fn acquire_prefetch<F>(self: &Arc<Self>, mut cancelled: F) -> Option<DecodePermit>
    where
        F: FnMut() -> bool,
    {
        let mut state = self.lock_state();
        loop {
            if cancelled() {
                return None;
            }

            if !state.foreground_pending && !state.busy {
                let debounce_remaining = state
                    .idle_since
                    .and_then(|idle_since| self.idle_debounce.checked_sub(idle_since.elapsed()));
                if debounce_remaining.is_none() {
                    state.busy = true;
                    return Some(DecodePermit {
                        gate: Arc::clone(self),
                    });
                }
            }

            let wait = state
                .idle_since
                .and_then(|idle_since| self.idle_debounce.checked_sub(idle_since.elapsed()))
                .unwrap_or(CANCELLATION_POLL)
                .min(CANCELLATION_POLL);
            state = self.wait_timeout(state, wait);
        }
    }

    fn acquire_foreground<F>(self: &Arc<Self>, epoch: u64, mut cancelled: F) -> Option<DecodePermit>
    where
        F: FnMut() -> bool,
    {
        let mut state = self.lock_state();
        loop {
            if cancelled() || state.foreground_epoch != epoch {
                return None;
            }
            if !state.busy {
                state.busy = true;
                return Some(DecodePermit {
                    gate: Arc::clone(self),
                });
            }
            state = self.wait_timeout(state, CANCELLATION_POLL);
        }
    }

    fn finish_foreground(&self, epoch: u64) {
        let mut state = self.lock_state();
        if state.foreground_epoch == epoch {
            state.foreground_pending = false;
            state.idle_since = Some(Instant::now());
        }
        drop(state);
        self.changed.notify_all();
    }

    fn release(&self) {
        let mut state = self.lock_state();
        debug_assert!(state.busy, "decode permit released while gate was idle");
        state.busy = false;
        drop(state);
        self.changed.notify_all();
    }

    fn lock_state(&self) -> MutexGuard<'_, GateState> {
        self.state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    fn wait_timeout<'a>(
        &self,
        state: MutexGuard<'a, GateState>,
        duration: Duration,
    ) -> MutexGuard<'a, GateState> {
        self.changed
            .wait_timeout(state, duration)
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .0
    }
}

#[derive(Debug)]
pub struct ForegroundTicket {
    gate: Arc<DecodeGate>,
    epoch: u64,
}

impl ForegroundTicket {
    pub fn acquire_decode<F>(&self, cancelled: F) -> Option<DecodePermit>
    where
        F: FnMut() -> bool,
    {
        self.gate.acquire_foreground(self.epoch, cancelled)
    }
}

impl Drop for ForegroundTicket {
    fn drop(&mut self) {
        self.gate.finish_foreground(self.epoch);
    }
}

#[derive(Debug)]
pub struct DecodePermit {
    gate: Arc<DecodeGate>,
}

impl Drop for DecodePermit {
    fn drop(&mut self) {
        self.gate.release();
    }
}

#[cfg(test)]
mod tests {
    use std::{
        sync::atomic::{AtomicBool, AtomicUsize},
        thread,
    };

    use crossbeam_channel::bounded;

    use super::*;

    #[test]
    fn production_prefetch_debounce_is_two_hundred_milliseconds() {
        assert_eq!(PREFETCH_IDLE_DEBOUNCE, Duration::from_millis(200));
    }

    #[test]
    fn foreground_intent_is_nonblocking_and_invalidates_prefetch_generation() {
        let gate = Arc::new(DecodeGate::new());
        let generation = gate.speculative_generation();
        let expected = generation.load(Ordering::Acquire);
        let _prefetch = gate.acquire_prefetch(|| false).unwrap();

        let started = Instant::now();
        let ticket = gate.request_foreground();
        assert!(started.elapsed() < Duration::from_millis(50));
        assert_ne!(generation.load(Ordering::Acquire), expected);
        drop(ticket);
    }

    #[test]
    fn foreground_jumps_a_waiting_prefetch_after_current_permit() {
        let gate = Arc::new(DecodeGate::with_idle_debounce(Duration::ZERO));
        let active = gate.acquire_prefetch(|| false).unwrap();
        let (order_tx, order_rx) = bounded(2);
        let cancel_prefetch = Arc::new(AtomicBool::new(false));

        let waiting_gate = Arc::clone(&gate);
        let waiting_cancel = Arc::clone(&cancel_prefetch);
        let prefetch_tx = order_tx.clone();
        let prefetch = thread::spawn(move || {
            let permit = waiting_gate.acquire_prefetch(|| waiting_cancel.load(Ordering::Acquire));
            if let Some(permit) = permit {
                prefetch_tx.send("prefetch").unwrap();
                drop(permit);
            }
        });

        let ticket = gate.request_foreground();
        let foreground_tx = order_tx.clone();
        let foreground = thread::spawn(move || {
            let permit = ticket.acquire_decode(|| false).unwrap();
            foreground_tx.send("foreground").unwrap();
            drop(permit);
            drop(ticket);
        });

        drop(active);
        assert_eq!(
            order_rx.recv_timeout(Duration::from_secs(1)).unwrap(),
            "foreground"
        );
        cancel_prefetch.store(true, Ordering::Release);
        gate.changed.notify_all();
        foreground.join().unwrap();
        prefetch.join().unwrap();
    }

    #[test]
    fn cancelled_prefetch_waiter_returns_none_while_permit_is_busy() {
        let gate = Arc::new(DecodeGate::with_idle_debounce(Duration::ZERO));
        let active = gate.acquire_prefetch(|| false).unwrap();
        let cancelled = Arc::new(AtomicBool::new(false));
        let (polled_tx, polled_rx) = bounded(1);
        let (result_tx, result_rx) = bounded(1);

        let waiting_gate = Arc::clone(&gate);
        let waiting_cancelled = Arc::clone(&cancelled);
        let waiter = thread::spawn(move || {
            let result = waiting_gate.acquire_prefetch(|| {
                let _ = polled_tx.try_send(());
                waiting_cancelled.load(Ordering::Acquire)
            });
            result_tx.send(result.is_some()).unwrap();
        });

        polled_rx.recv_timeout(Duration::from_secs(1)).unwrap();
        cancelled.store(true, Ordering::Release);
        gate.changed.notify_all();
        assert!(!result_rx.recv_timeout(Duration::from_secs(1)).unwrap());
        drop(active);
        waiter.join().unwrap();
    }

    #[test]
    fn superseded_ticket_does_not_clear_newer_foreground_priority() {
        let gate = Arc::new(DecodeGate::with_idle_debounce(Duration::ZERO));
        let old = gate.request_foreground();
        let current = gate.request_foreground();

        drop(old);
        {
            let state = gate.lock_state();
            assert!(state.foreground_pending);
            assert_eq!(state.foreground_epoch, current.epoch);
        }

        drop(current);
        assert!(!gate.lock_state().foreground_pending);
    }

    #[test]
    fn dropping_current_ticket_unblocks_prefetch() {
        let gate = Arc::new(DecodeGate::with_idle_debounce(Duration::ZERO));
        let ticket = gate.request_foreground();
        let (polled_tx, polled_rx) = bounded(1);
        let (acquired_tx, acquired_rx) = bounded(1);

        let waiting_gate = Arc::clone(&gate);
        let waiter = thread::spawn(move || {
            let permit = waiting_gate.acquire_prefetch(|| {
                let _ = polled_tx.try_send(());
                false
            });
            acquired_tx.send(permit.is_some()).unwrap();
        });

        polled_rx.recv_timeout(Duration::from_secs(1)).unwrap();
        assert!(acquired_rx.try_recv().is_err());
        drop(ticket);
        assert!(acquired_rx.recv_timeout(Duration::from_secs(1)).unwrap());
        waiter.join().unwrap();
    }

    #[test]
    fn permits_serialize_foreground_and_prefetch_work() {
        let gate = Arc::new(DecodeGate::with_idle_debounce(Duration::ZERO));
        let active = Arc::new(AtomicUsize::new(0));
        let maximum = Arc::new(AtomicUsize::new(0));
        let (prefetch_started_tx, prefetch_started_rx) = bounded(0);
        let (release_prefetch_tx, release_prefetch_rx) = bounded(0);
        let (foreground_started_tx, foreground_started_rx) = bounded(0);

        let prefetch_gate = Arc::clone(&gate);
        let prefetch_active = Arc::clone(&active);
        let prefetch_maximum = Arc::clone(&maximum);
        let prefetch = thread::spawn(move || {
            let _permit = prefetch_gate.acquire_prefetch(|| false).unwrap();
            enter(&prefetch_active, &prefetch_maximum);
            prefetch_started_tx.send(()).unwrap();
            release_prefetch_rx.recv().unwrap();
            leave(&prefetch_active);
        });
        prefetch_started_rx.recv().unwrap();

        let ticket = gate.request_foreground();
        let foreground_active = Arc::clone(&active);
        let foreground_maximum = Arc::clone(&maximum);
        let foreground = thread::spawn(move || {
            let _permit = ticket.acquire_decode(|| false).unwrap();
            enter(&foreground_active, &foreground_maximum);
            foreground_started_tx.send(()).unwrap();
            leave(&foreground_active);
        });

        assert!(foreground_started_rx.try_recv().is_err());
        release_prefetch_tx.send(()).unwrap();
        foreground_started_rx
            .recv_timeout(Duration::from_secs(1))
            .unwrap();
        prefetch.join().unwrap();
        foreground.join().unwrap();
        assert_eq!(maximum.load(Ordering::Acquire), 1);
    }

    #[test]
    fn prefetch_waits_for_idle_debounce() {
        let debounce = Duration::from_millis(40);
        let gate = Arc::new(DecodeGate::with_idle_debounce(debounce));
        let ticket = gate.request_foreground();
        drop(ticket);

        let started = Instant::now();
        let _permit = gate.acquire_prefetch(|| false).unwrap();
        assert!(started.elapsed() >= debounce.saturating_sub(Duration::from_millis(5)));
    }

    fn enter(active: &AtomicUsize, maximum: &AtomicUsize) {
        let now = active.fetch_add(1, Ordering::AcqRel) + 1;
        maximum.fetch_max(now, Ordering::AcqRel);
    }

    fn leave(active: &AtomicUsize) {
        active.fetch_sub(1, Ordering::AcqRel);
    }
}
