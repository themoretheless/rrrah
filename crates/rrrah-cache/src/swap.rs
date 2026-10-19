//! Asynchronous spill tier for immutable, reproducible decoded images.
use crate::{CacheKey, decode_mosaic_payload_v1_with_budget, prepare_mosaic_payload_v1};
use rrrah_core::{DecodedMosaic, DecodedRaster};
use rrrah_memory::{CacheLimits, MemoryBudget, Reservation, WeightedLru};
use rrrah_swap::{SwapHandle, SwapLimits, SwapStore};
use std::{
    collections::HashMap,
    hash::Hash,
    io::{Read, Write},
};
use std::{
    path::Path,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicU64, Ordering},
        mpsc::{self, SyncSender},
    },
};

/// Immediate admission result; `Queued` does not guarantee a successful disk write.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpillAdmission {
    Queued,
    AlreadyPresent,
    AlreadyPending,
    Disabled,
    TooLarge,
    Full,
    Busy,
    WorkerStopped,
}

/// Errors from a payload codec, preserving transient memory admission failures.
#[derive(Debug)]
pub enum SwapPayloadError {
    Memory(rrrah_memory::BufferError),
    Invalid(std::io::Error),
}
impl std::fmt::Display for SwapPayloadError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Memory(e) => e.fmt(f),
            Self::Invalid(e) => e.fmt(f),
        }
    }
}
impl std::error::Error for SwapPayloadError {}
/// Streaming codec for an immutable reproducible object. The store supplies
/// integrity and cancellation; implementations must budget restored allocations.
pub trait SwapPayload: Send + 'static + Sized {
    /// Opt in only when allocations made after stream reads begin are bounded
    /// by unread wire bytes. Compressed/custom payloads remain serial by default.
    fn permits_hash_scratch() -> bool {
        false
    }
    fn capacity_bytes(&self) -> u64;
    /// Optional guaranteed lower bound on restored RAM-cache weight.
    /// Unknown/compressed codecs must not guess from serialized byte length.
    fn minimum_restore_weight(&self) -> Option<u64> {
        None
    }
    fn payload_len(&self) -> std::io::Result<u64>;
    fn write_payload(&self, writer: &mut impl Write) -> std::io::Result<()>;
    fn read_payload(
        reader: &mut impl Read,
        bytes: u64,
        budget: &MemoryBudget,
    ) -> Result<Self, SwapPayloadError>;
}
impl SwapPayload for DecodedMosaic {
    fn capacity_bytes(&self) -> u64 {
        self.pixels.capacity_bytes()
    }
    fn minimum_restore_weight(&self) -> Option<u64> {
        u64::try_from(self.pixels.len()).ok()?.checked_mul(2)
    }
    fn payload_len(&self) -> std::io::Result<u64> {
        prepare_mosaic_payload_v1(self)
            .map(|p| p.stats().payload_bytes)
            .map_err(std::io::Error::other)
    }
    fn write_payload(&self, writer: &mut impl Write) -> std::io::Result<()> {
        prepare_mosaic_payload_v1(self)
            .map_err(std::io::Error::other)?
            .encode(writer)
            .map_err(std::io::Error::other)
    }
    fn read_payload(
        reader: &mut impl Read,
        bytes: u64,
        budget: &MemoryBudget,
    ) -> Result<Self, SwapPayloadError> {
        decode_mosaic_payload_v1_with_budget(reader, bytes, budget).map_err(|e| match e {
            crate::MosaicPayloadError::Memory(e) => SwapPayloadError::Memory(e),
            e => SwapPayloadError::Invalid(std::io::Error::other(e)),
        })
    }
}
impl SwapPayload for DecodedRaster {
    fn permits_hash_scratch() -> bool {
        true
    }
    fn capacity_bytes(&self) -> u64 {
        self.capacity_bytes()
    }
    fn minimum_restore_weight(&self) -> Option<u64> {
        let (length, bytes) = match self.pixels() {
            rrrah_core::RasterPixels::Rgba8(v) => (v.len(), 1),
            rrrah_core::RasterPixels::Rgba16(v) => (v.len(), 2),
            rrrah_core::RasterPixels::Rgba32Float(v) => (v.len(), 4),
        };
        u64::try_from(length).ok()?.checked_mul(bytes)
    }
    fn payload_len(&self) -> std::io::Result<u64> {
        crate::raster_payload_len(self).map_err(std::io::Error::other)
    }
    fn write_payload(&self, writer: &mut impl Write) -> std::io::Result<()> {
        crate::write_raster_payload(writer, self).map_err(std::io::Error::other)
    }
    fn read_payload(
        reader: &mut impl Read,
        bytes: u64,
        budget: &MemoryBudget,
    ) -> Result<Self, SwapPayloadError> {
        crate::read_raster_payload_with_length(reader, bytes, budget).map_err(|e| match e {
            crate::RasterPayloadError::Memory(e) => SwapPayloadError::Memory(e),
            e => SwapPayloadError::Invalid(std::io::Error::other(e)),
        })
    }
}
pub type MosaicSwapCache = ImageSwapCache<CacheKey, DecodedMosaic>;
pub type RasterSwapCache<K> = ImageSwapCache<K, DecodedRaster>;
pub type MosaicSwapConfig = ImageSwapConfig;
pub type MosaicSwapStats = ImageSwapStats;

/// Hint and handle are published/replaced/expired together in the same index entry.
#[derive(Debug, Clone)]
struct StoredPayload {
    handle: SwapHandle,
    minimum_restore_weight: Option<u64>,
}
impl std::ops::Deref for StoredPayload {
    type Target = SwapHandle;
    fn deref(&self) -> &SwapHandle {
        &self.handle
    }
}
impl From<SwapHandle> for StoredPayload {
    fn from(handle: SwapHandle) -> Self {
        Self {
            handle,
            minimum_restore_weight: None,
        }
    }
}

/// Bound channel bookkeeping independently of payload byte admission.
pub const MAX_SWAP_QUEUE_COUNT: usize = 1024;

#[derive(Debug, Clone, Copy)]
pub struct ImageSwapConfig {
    pub limits: CacheLimits,
    pub queue_bytes: u64,
    /// Maximum waiting writes, excluding the active worker. Zero disables spills.
    /// Values above `MAX_SWAP_QUEUE_COUNT` are rejected before store creation.
    pub queue_count: usize,
    /// Pixel capacity held by all live restored frames, independent of cache membership.
    pub restore_bytes: u64,
}
#[derive(Debug, Default)]
struct Counters {
    writes: AtomicU64,
    reads: AtomicU64,
    errors: AtomicU64,
    dropped: AtomicU64,
}
#[derive(Debug, Clone, Copy)]
pub struct ImageSwapStats {
    pub writes: u64,
    pub reads: u64,
    /// Codec, corruption or filesystem failures; expected quota refusals are excluded.
    pub errors: u64,
    /// Writes refused by admission, quota, cancellation or publication policy.
    pub dropped: u64,
    pub queued_bytes: u64,
}
enum Command<K, V> {
    Spill(K, V, Reservation, u64, Box<dyn Send>),
    Barrier(mpsc::Sender<()>),
}
struct PendingSpill<K: Eq + Hash> {
    keys: Arc<Mutex<HashMap<K, u64>>>,
    key: K,
    generation: u64,
}
impl<K: Eq + Hash> Drop for PendingSpill<K> {
    fn drop(&mut self) {
        let mut keys = self
            .keys
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        // An older cancelled command must not remove a newer request's marker.
        if keys.get(&self.key) == Some(&self.generation) {
            keys.remove(&self.key);
        }
    }
}
#[derive(Debug)]
pub struct ImageSwapCache<K, V> {
    tx: SyncSender<Command<K, V>>,
    worker: Mutex<Option<std::thread::JoinHandle<()>>>,
    index: Arc<Mutex<WeightedLru<K, StoredPayload>>>,
    store: SwapStore,
    queue_budget: MemoryBudget,
    restore_budget: MemoryBudget,
    // Original caller admission root; scratch must not consume pixel-only caps.
    scratch_budget: MemoryBudget,
    restore_limit: u64,
    retired_restore_budgets: Vec<MemoryBudget>,
    restore_admission: Mutex<()>,
    cancelled: Arc<AtomicBool>,
    write_generation: Arc<AtomicU64>,
    pending: Arc<Mutex<HashMap<K, u64>>>,
    queue_count: usize,
    counters: Arc<Counters>,
}
impl<K: Clone + Eq + Hash + Send + 'static, V: SwapPayload> ImageSwapCache<K, V> {
    pub fn new(parent: &Path, config: ImageSwapConfig) -> std::io::Result<Self> {
        Self::new_with_budgets(
            parent,
            config,
            MemoryBudget::new(config.queue_bytes),
            MemoryBudget::new(config.restore_bytes),
        )
    }
    /// The queue budget measures occupancy; use an independent budget when pixels
    /// already retain reservations in a shared allocation root. Restore admission
    /// may be a child of that root. Configured local byte caps are enforced
    /// with child budgets even when the supplied budgets have larger limits.
    pub fn new_with_budgets(
        parent: &Path,
        config: ImageSwapConfig,
        queue_budget: MemoryBudget,
        restore_budget: MemoryBudget,
    ) -> std::io::Result<Self> {
        if config.queue_count > MAX_SWAP_QUEUE_COUNT {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                format!("swap queue count exceeds {MAX_SWAP_QUEUE_COUNT}"),
            ));
        }
        let queue_budget = queue_budget.child(config.queue_bytes);
        let scratch_budget = restore_budget.clone();
        let restore_budget = restore_budget.child(config.restore_bytes);
        let store = SwapStore::new(
            parent,
            SwapLimits {
                max_bytes: config.limits.max_bytes,
                max_objects: config.limits.max_entries,
            },
        )
        .map_err(std::io::Error::other)?;
        let index = Arc::new(Mutex::new(WeightedLru::<K, StoredPayload>::with_limits(
            config.limits,
        )));
        let cancelled = Arc::new(AtomicBool::new(false));
        let write_generation = Arc::new(AtomicU64::new(0));
        let counters = Arc::new(Counters::default());
        let (tx, commands) = mpsc::sync_channel::<Command<K, V>>(config.queue_count);
        let worker_index = index.clone();
        let worker_store = store.clone();
        let worker_cancel = cancelled.clone();
        let worker_generation = write_generation.clone();
        let worker_counters = counters.clone();
        let worker = std::thread::Builder::new()
            .name("rrrah-swap-write".into())
            .spawn(move || {
                let maintenance_period = std::time::Duration::from_secs(1);
                let mut next_maintenance = std::time::Instant::now() + maintenance_period;
                loop {
                    if worker_cancel.load(Ordering::Acquire) {
                        break;
                    }
                    let command = if config.limits.ttl.is_some() {
                        // A busy command stream must not keep resetting TTL
                        // maintenance. In particular, barriers do not touch LRU.
                        let now = std::time::Instant::now();
                        if now >= next_maintenance {
                            if let Ok(mut index) = worker_index.lock() {
                                index.prune_expired();
                            }
                            next_maintenance = now + maintenance_period;
                        }
                        let wait = next_maintenance.saturating_duration_since(std::time::Instant::now());
                        match commands.recv_timeout(wait) {
                            Ok(command) => command,
                            Err(mpsc::RecvTimeoutError::Timeout) => continue,
                            Err(mpsc::RecvTimeoutError::Disconnected) => break,
                        }
                    } else {
                        match commands.recv() {
                            Ok(command) => command,
                            Err(_) => break,
                        }
                    };
                    match command {
                        Command::Barrier(done) => {
                            let _ = done.send(());
                        }
                        Command::Spill(key, mosaic, _reservation, generation, _pending) => {
                            let cancelled = || {
                                worker_cancel.load(Ordering::Acquire)
                                    || worker_generation.load(Ordering::Acquire) != generation
                            };
                            if cancelled() {
                                worker_counters.dropped.fetch_add(1, Ordering::Relaxed);
                                continue;
                            }
                            let result = (|| -> Result<(), Box<dyn std::error::Error>> {
                                let bytes = mosaic.payload_len()?;
                                if bytes > config.limits.max_bytes || config.limits.max_entries == Some(0) {
                                    return Err(rrrah_swap::SwapError::Quota.into());
                                }
                                {
                                    let mut index = worker_index.lock().map_err(|_| "swap index poisoned")?;
                                    if cancelled() {
                                        return Err(rrrah_swap::SwapError::Cancelled.into());
                                    }
                                    if index.get(&key).is_some() {
                                        return Ok(());
                                    }
                                    index.prune_expired();
                                    loop {
                                        if cancelled() {
                                            return Err(rrrah_swap::SwapError::Cancelled.into());
                                        }
                                        let usage = worker_store.usage()?;
                                        let full_bytes =
                                            bytes > config.limits.max_bytes.saturating_sub(usage.bytes);
                                        let full_count = config
                                            .limits
                                            .max_entries
                                            .is_some_and(|limit| usage.objects >= limit);
                                        if !full_bytes && !full_count {
                                            break;
                                        }
                                        if index.take_lru().is_none() {
                                            break;
                                        }
                                    }
                                }
                                let minimum_restore_weight = mosaic.minimum_restore_weight();
                                let handle = worker_store.write_stream(bytes, cancelled, |mut writer| {
                                    mosaic.write_payload(&mut writer)
                                })?;
                                if cancelled() {
                                    worker_counters.dropped.fetch_add(1, Ordering::Relaxed);
                                    return Ok(());
                                }
                                let mut index = worker_index.lock().map_err(|_| "swap index poisoned")?;
                                if cancelled() {
                                    return Err(rrrah_swap::SwapError::Cancelled.into());
                                }
                                if index.insert_prefetch(
                                    key,
                                    StoredPayload {
                                        handle,
                                        minimum_restore_weight,
                                    },
                                    bytes,
                                ) {
                                    worker_counters.writes.fetch_add(1, Ordering::Relaxed);
                                } else {
                                    worker_counters.dropped.fetch_add(1, Ordering::Relaxed);
                                }
                                Ok(())
                            })();
                            if let Err(error) = result {
                                // Region backing shares the hard quota but is owned
                                // outside the LRU. Refusal while it is live is normal
                                // bounded admission, not a codec or filesystem failure.
                                if cancelled()
                                    || matches!(
                                        error.downcast_ref::<rrrah_swap::SwapError>(),
                                        Some(rrrah_swap::SwapError::Quota)
                                    )
                                {
                                    worker_counters.dropped.fetch_add(1, Ordering::Relaxed);
                                } else {
                                    worker_counters.errors.fetch_add(1, Ordering::Relaxed);
                                }
                            }
                        }
                    }
                }
            })?;
        Ok(Self {
            tx,
            worker: Mutex::new(Some(worker)),
            index,
            store,
            queue_budget,
            restore_budget,
            scratch_budget,
            restore_limit: config.restore_bytes,
            retired_restore_budgets: Vec::new(),
            restore_admission: Mutex::new(()),
            cancelled,
            write_generation,
            pending: Arc::new(Mutex::new(HashMap::new())),
            queue_count: config.queue_count,
            counters,
        })
    }
    /// Nonblocking opportunistic spill. Queue limits cover retained pixel capacity, not total process memory.
    pub fn enqueue(&self, key: K, mosaic: V) {
        let _ = self.try_enqueue(key, mosaic);
    }
    /// Nonblocking admission. Rejected payload ownership is released before return.
    /// A queued write may evict completed LRU objects to satisfy the hard disk
    /// quota before serialization starts. Failure or cancellation removes the
    /// new partial object, but does not restore those evicted cache entries.
    pub fn try_enqueue(&self, key: K, mosaic: V) -> SpillAdmission {
        if self.queue_count == 0 {
            self.counters.dropped.fetch_add(1, Ordering::Relaxed);
            return SpillAdmission::Disabled;
        }
        let generation = self.write_generation.load(Ordering::Acquire);
        if let Ok(mut index) = self.index.try_lock() {
            if index.get(&key).is_some() {
                return SpillAdmission::AlreadyPresent;
            }
        }
        let pending = {
            let Ok(mut keys) = self.pending.try_lock() else {
                self.counters.dropped.fetch_add(1, Ordering::Relaxed);
                return SpillAdmission::Busy;
            };
            if keys.get(&key) == Some(&generation) {
                return SpillAdmission::AlreadyPending;
            }
            keys.insert(key.clone(), generation);
            PendingSpill {
                keys: self.pending.clone(),
                key: key.clone(),
                generation,
            }
        };
        if mosaic.capacity_bytes() > self.queue_budget.limit() {
            self.counters.dropped.fetch_add(1, Ordering::Relaxed);
            return SpillAdmission::TooLarge;
        }
        let Ok(reservation) = self.queue_budget.try_reserve(mosaic.capacity_bytes()) else {
            self.counters.dropped.fetch_add(1, Ordering::Relaxed);
            return SpillAdmission::Full;
        };
        match self.tx.try_send(Command::Spill(
            key,
            mosaic,
            reservation,
            generation,
            Box::new(pending),
        )) {
            Ok(()) => SpillAdmission::Queued,
            Err(error) => {
                self.counters.dropped.fetch_add(1, Ordering::Relaxed);
                match error {
                    mpsc::TrySendError::Full(_) => SpillAdmission::Full,
                    mpsc::TrySendError::Disconnected(_) => SpillAdmission::WorkerStopped,
                }
            }
        }
    }
    /// Cancels outstanding spills without deleting completed disk objects.
    /// Nonblocking: retained buffers are released when the worker observes the
    /// generation change. A codec must return to stream I/O to observe cancellation.
    /// New enqueues use the new generation and remain eligible for writing.
    pub fn discard_pending_writes(&self) {
        self.write_generation.fetch_add(1, Ordering::AcqRel);
    }
    /// Share the same disk quota for a visible image's segmented region backing.
    /// The returned handle remains charged until its last owner drops.
    pub fn region_store(&self) -> SwapStore {
        self.store.clone()
    }

    pub fn restore_budget(&self) -> &MemoryBudget {
        &self.restore_budget
    }
    /// Configures the shared budget for restored pixel allocations.
    pub fn set_restore_budget(&mut self, budget: MemoryBudget) {
        self.retired_restore_budgets.retain(|old| old.used() != 0);
        if self.restore_budget.used() != 0 {
            self.retired_restore_budgets.push(self.restore_budget.clone());
        }
        self.scratch_budget = budget.clone();
        self.restore_budget = budget.child(self.restore_limit);
    }
    /// Index-only lookup; no payload read, decode or restore-budget reservation.
    /// Like other lookups this updates recency and removes expired membership.
    /// A hint is not an integrity check; actual restoration still verifies the object.
    pub fn minimum_restore_weight(&self, key: &K) -> Option<u64> {
        self.index.lock().ok()?.get(key)?.minimum_restore_weight
    }
    pub fn get(&self, key: &K, cancelled: impl FnMut() -> bool) -> Option<V> {
        self.try_get(key, cancelled).ok().flatten()
    }
    /// Distinguishes transient allocation admission from a cache miss.
    /// Admission failure retains the immutable swap entry for later retry.
    pub fn try_get(
        &self,
        key: &K,
        mut cancelled: impl FnMut() -> bool,
    ) -> Result<Option<V>, rrrah_memory::BufferError> {
        // Serialize admission so simultaneous restores cannot each spend the
        // same remaining allowance from retired budget generations.
        let _admission = loop {
            if cancelled() {
                return Ok(None);
            }
            match self.restore_admission.try_lock() {
                Ok(guard) => break guard,
                Err(std::sync::TryLockError::Poisoned(error)) => break error.into_inner(),
                Err(std::sync::TryLockError::WouldBlock) => {
                    std::thread::sleep(std::time::Duration::from_millis(1));
                }
            }
        };
        let Some(handle) = self
            .index
            .lock()
            .ok()
            .and_then(|mut index| index.get(key).cloned())
        else {
            return Ok(None);
        };
        let mut memory_pressure = None;
        let budgets = std::iter::once(&self.restore_budget).chain(self.retired_restore_budgets.iter());
        // An ancestor's usage already includes all descendant reservations.
        // Sum only outermost budgets; identical handles count once as well.
        let live = budgets
            .clone()
            .enumerate()
            .filter(|(position, budget)| {
                !budgets.clone().enumerate().any(|(other_position, other)| {
                    other_position != *position
                        && budget.is_descendant_of(other)
                        && (!other.is_descendant_of(budget) || other_position < *position)
                })
            })
            .fold(0u64, |total, (_, budget)| total.saturating_add(budget.used()));
        let allowance = self.restore_limit.saturating_sub(live);
        let restore = self.restore_budget.child(allowance);
        let decode = |mut reader: &mut dyn Read| {
            let decoded = V::read_payload(&mut reader, handle.byte_len(), &restore);
            match decoded {
                Err(SwapPayloadError::Memory(error)) => {
                    // Report aggregate occupancy rather than a temporary zero
                    // allowance, so foreground caches can release unpinned
                    // old-generation owners. A truly smaller parent cap still
                    // reports an impossible allocation and avoids futile eviction.
                    memory_pressure = Some(match error {
                        rrrah_memory::BufferError::Capacity {
                            requested,
                            used,
                            limit,
                        } if live != 0 && limit == allowance => rrrah_memory::BufferError::Capacity {
                            requested,
                            used: live.saturating_add(used),
                            limit: self.restore_budget.allocation_limit(),
                        },
                        other => other,
                    });
                    Err(std::io::Error::other("restore allocation admission rejected"))
                }
                result => result.map_err(std::io::Error::other),
            }
        };
        let result = if V::permits_hash_scratch() {
            self.store
                .read_stream_with_scratch(&handle, &mut cancelled, decode, &self.scratch_budget)
        } else {
            self.store.read_stream(&handle, &mut cancelled, decode)
        };
        match result {
            Ok(mosaic) => {
                self.counters.reads.fetch_add(1, Ordering::Relaxed);
                Ok(Some(mosaic))
            }
            Err(rrrah_swap::SwapError::Cancelled) => Ok(None),
            Err(_) if memory_pressure.is_some() => Err(memory_pressure.unwrap()),
            Err(_) => {
                self.counters.errors.fetch_add(1, Ordering::Relaxed);
                if let Ok(mut index) = self.index.lock() {
                    if index.get(key).is_some_and(|current| current.ptr_eq(&handle)) {
                        index.remove(key);
                    }
                }
                Ok(None)
            }
        }
    }
    /// Removes all expired index entries without requiring a new spill or lookup.
    /// Call from background maintenance, not the render thread: removal performs
    /// filesystem cleanup. In-flight restores retain their handles and quota until
    /// completion. Returns the number removed from the index, not deleted files.
    pub fn prune_expired(&self) -> std::io::Result<usize> {
        let mut index = self
            .index
            .lock()
            .map_err(|_| std::io::Error::other("swap index poisoned"))?;
        Ok(index.prune_expired())
    }
    pub fn stats(&self) -> ImageSwapStats {
        ImageSwapStats {
            writes: self.counters.writes.load(Ordering::Acquire),
            reads: self.counters.reads.load(Ordering::Acquire),
            errors: self.counters.errors.load(Ordering::Acquire),
            dropped: self.counters.dropped.load(Ordering::Acquire),
            queued_bytes: self.queue_budget.used(),
        }
    }
    /// Disk quota charged by completed blobs and active writes, including blobs
    /// retained by an in-flight restore after their index entry was removed.
    pub fn disk_usage(&self) -> std::io::Result<rrrah_swap::SwapUsage> {
        self.store.usage().map_err(std::io::Error::other)
    }

    /// Synchronization hook for tests/benchmarks; never call this from the render thread.
    /// Terminal owner shutdown. Cancels queued writes and joins the worker so
    /// temporary storage owners are released before process exit. Never use on UI.
    pub fn shutdown(&self) {
        self.cancelled.store(true, Ordering::Release);
        let (wake, _) = mpsc::channel();
        let _ = self.tx.try_send(Command::Barrier(wake));
        if let Some(worker) = self
            .worker
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .take()
        {
            let _ = worker.join();
        }
    }

    pub fn wait_idle(&self) -> std::io::Result<()> {
        let (tx, rx) = mpsc::channel();
        self.tx
            .send(Command::Barrier(tx))
            .map_err(|_| std::io::Error::other("swap worker disconnected"))?;
        rx.recv().map_err(std::io::Error::other)
    }
}
impl<K, V> Drop for ImageSwapCache<K, V> {
    fn drop(&mut self) {
        self.cancelled.store(true, Ordering::Release);
    }
}

#[cfg(test)]
mod write_cancellation_tests {
    use super::*;
    struct Payload {
        buffer: rrrah_memory::SharedBuffer<u8>,
        pause: Option<(mpsc::Sender<()>, mpsc::Receiver<()>, bool)>,
    }
    impl SwapPayload for Payload {
        fn capacity_bytes(&self) -> u64 {
            self.buffer.capacity_bytes()
        }
        fn payload_len(&self) -> std::io::Result<u64> {
            if let Some((started, resume, true)) = &self.pause {
                started.send(()).unwrap();
                resume.recv().unwrap();
            }
            Ok(self.buffer.len() as u64)
        }
        fn write_payload(&self, writer: &mut impl Write) -> std::io::Result<()> {
            if let Some((started, resume, false)) = &self.pause {
                started.send(()).unwrap();
                resume.recv().unwrap();
            }
            writer.write_all(&self.buffer)
        }
        fn read_payload(
            reader: &mut impl Read,
            bytes: u64,
            budget: &MemoryBudget,
        ) -> Result<Self, SwapPayloadError> {
            let mut buffer = budget
                .try_buffer(bytes as usize, 0_u8)
                .map_err(SwapPayloadError::Memory)?;
            reader
                .read_exact(&mut buffer)
                .map_err(SwapPayloadError::Invalid)?;
            Ok(Self {
                buffer: buffer.freeze(),
                pause: None,
            })
        }
    }
    #[test]
    fn terminal_shutdown_joins_idle_worker_and_releases_temporary_directory() {
        for ttl in [None, Some(std::time::Duration::from_secs(3600))] {
            let directory = tempfile::tempdir().unwrap();
            let root = MemoryBudget::new(3);
            let swap = ImageSwapCache::<u8, Payload>::new(
                directory.path(),
                ImageSwapConfig {
                    limits: CacheLimits {
                        max_bytes: 3,
                        max_entries: Some(1),
                        ttl,
                    },
                    queue_bytes: 3,
                    queue_count: 1,
                    restore_bytes: 3,
                },
            )
            .unwrap();
            swap.enqueue(
                1,
                Payload {
                    buffer: root.try_buffer(3, 7).unwrap().freeze(),
                    pause: None,
                },
            );
            swap.wait_idle().unwrap();
            assert_eq!(swap.disk_usage().unwrap().objects, 1);
            swap.shutdown();
            swap.shutdown();
            assert!(swap.worker.lock().unwrap().is_none());
            drop(swap);
            assert_eq!(root.used(), 0);
            assert_eq!(std::fs::read_dir(directory.path()).unwrap().count(), 0);
        }
    }

    #[test]
    fn cancelling_active_and_queued_spills_releases_owners_and_allows_new_writes() {
        let directory = tempfile::tempdir().unwrap();
        let root = MemoryBudget::new(3);
        let swap = ImageSwapCache::<u8, Payload>::new(
            directory.path(),
            ImageSwapConfig {
                limits: CacheLimits::bytes(3),
                queue_bytes: 3,
                queue_count: 4,
                restore_bytes: 3,
            },
        )
        .unwrap();
        let (started, started_rx) = mpsc::channel();
        let (resume, resume_rx) = mpsc::channel();
        swap.enqueue(
            1,
            Payload {
                buffer: root.try_buffer(1, 1).unwrap().freeze(),
                pause: Some((started, resume_rx, false)),
            },
        );
        started_rx
            .recv_timeout(std::time::Duration::from_secs(5))
            .unwrap();
        swap.enqueue(
            2,
            Payload {
                buffer: root.try_buffer(1, 2).unwrap().freeze(),
                pause: None,
            },
        );
        assert_eq!(root.used(), 2);
        assert_eq!(swap.stats().queued_bytes, 2);
        swap.enqueue(
            1,
            Payload {
                buffer: root.try_buffer(1, 99).unwrap().freeze(),
                pause: None,
            },
        );
        assert_eq!(root.used(), 2);
        assert_eq!(swap.stats().queued_bytes, 2);
        swap.discard_pending_writes();
        swap.enqueue(
            3,
            Payload {
                buffer: root.try_buffer(1, 3).unwrap().freeze(),
                pause: None,
            },
        );
        resume.send(()).unwrap();
        swap.wait_idle().unwrap();
        assert_eq!(root.used(), 0);
        assert_eq!(swap.stats().queued_bytes, 0);
        assert_eq!(swap.stats().errors, 0);
        assert_eq!(swap.stats().dropped, 2);
        assert_eq!(swap.stats().writes, 1);
        assert!(swap.get(&1, || false).is_none());
        assert!(swap.get(&2, || false).is_none());
        assert_eq!(&*swap.get(&3, || false).unwrap().buffer, &[3]);
        swap.discard_pending_writes();
        assert_eq!(&*swap.get(&3, || false).unwrap().buffer, &[3]);
    }
    #[test]
    fn cancelled_payload_preparation_does_not_evict_completed_disk_object() {
        let directory = tempfile::tempdir().unwrap();
        let root = MemoryBudget::new(1);
        let swap = ImageSwapCache::<u8, Payload>::new(
            directory.path(),
            ImageSwapConfig {
                limits: CacheLimits {
                    max_bytes: 1,
                    max_entries: Some(1),
                    ttl: None,
                },
                queue_bytes: 1,
                queue_count: 4,
                restore_bytes: 1,
            },
        )
        .unwrap();
        swap.enqueue(
            1,
            Payload {
                buffer: root.try_buffer(1, 41).unwrap().freeze(),
                pause: None,
            },
        );
        swap.wait_idle().unwrap();
        let (started, started_rx) = mpsc::channel();
        let (resume, resume_rx) = mpsc::channel();
        swap.enqueue(
            2,
            Payload {
                buffer: root.try_buffer(1, 42).unwrap().freeze(),
                pause: Some((started, resume_rx, true)),
            },
        );
        started_rx
            .recv_timeout(std::time::Duration::from_secs(5))
            .unwrap();
        swap.discard_pending_writes();
        resume.send(()).unwrap();
        swap.wait_idle().unwrap();
        assert_eq!(root.used(), 0);
        assert_eq!(swap.stats().queued_bytes, 0);
        assert_eq!(swap.stats().dropped, 1);
        assert_eq!(swap.stats().errors, 0);
        assert_eq!(swap.stats().writes, 1);
        assert!(swap.get(&2, || false).is_none());
        assert_eq!(&*swap.get(&1, || false).unwrap().buffer, &[41]);
    }
    #[test]
    fn transient_restore_cancellation_retains_entry_and_releases_allocated_pixels() {
        let directory = tempfile::tempdir().unwrap();
        let root = MemoryBudget::new(1);
        let swap = ImageSwapCache::<u8, Payload>::new_with_budgets(
            directory.path(),
            ImageSwapConfig {
                limits: CacheLimits::bytes(1),
                queue_bytes: 1,
                queue_count: 1,
                restore_bytes: 1,
            },
            MemoryBudget::new(1),
            root.clone(),
        )
        .unwrap();
        swap.enqueue(
            1,
            Payload {
                buffer: root.try_buffer(1, 41).unwrap().freeze(),
                pause: None,
            },
        );
        swap.wait_idle().unwrap();
        assert_eq!(root.used(), 0);
        let mut polls = 0;
        let mut allocation_seen = false;
        assert!(
            swap.try_get(&1, || {
                polls += 1;
                if polls == 3 {
                    allocation_seen = root.used() == 1;
                }
                polls == 3
            })
            .unwrap()
            .is_none()
        );
        assert!(allocation_seen);
        assert_eq!(root.used(), 0);
        assert_eq!(swap.stats().errors, 0);
        let retry = swap.try_get(&1, || false).unwrap().unwrap();
        assert_eq!(&*retry.buffer, &[41]);
        assert_eq!(root.used(), 1);
        drop(retry);
        assert_eq!(root.used(), 0);
        assert_eq!(swap.stats().reads, 1);
    }
    #[test]
    fn cancelled_restore_waiter_returns_before_active_admission_releases() {
        let directory = tempfile::tempdir().unwrap();
        let root = MemoryBudget::new(4);
        let swap = ImageSwapCache::<u8, Payload>::new_with_budgets(
            directory.path(),
            ImageSwapConfig {
                limits: CacheLimits::bytes(32),
                queue_bytes: 1,
                queue_count: 1,
                restore_bytes: 1,
            },
            MemoryBudget::new(1),
            root.clone(),
        )
        .unwrap();
        swap.enqueue(
            1,
            Payload {
                buffer: root.try_buffer(1, 7u8).unwrap().freeze(),
                pause: None,
            },
        );
        swap.wait_idle().unwrap();
        for cancel_after in [1, 3] {
            let admission = swap.restore_admission.lock().unwrap();
            std::thread::scope(|scope| {
                let (sent, received) = mpsc::channel();
                let cache = &swap;
                let worker = scope.spawn(move || {
                    let mut calls = 0;
                    let result = cache.try_get(&1, || {
                        calls += 1;
                        calls >= cancel_after
                    });
                    sent.send(matches!(result, Ok(None))).unwrap();
                });
                let before_release = received.recv_timeout(std::time::Duration::from_secs(2));
                drop(admission);
                worker.join().unwrap();
                assert_eq!(before_release.unwrap(), true);
            });
            assert_eq!(root.used(), 0);
            assert_eq!(swap.stats().reads, 0);
        }
        let retry = swap.try_get(&1, || false).unwrap().unwrap();
        assert_eq!(&*retry.buffer, &[7]);
        drop(retry);
        assert_eq!(root.used(), 0);
    }
    #[test]
    fn concurrent_restores_cannot_double_spend_retired_generation_allowance() {
        let directory = tempfile::tempdir().unwrap();
        let old = MemoryBudget::new(32);
        let current = MemoryBudget::new(32);
        let mut swap = ImageSwapCache::<u8, Payload>::new_with_budgets(
            directory.path(),
            ImageSwapConfig {
                limits: CacheLimits::bytes(32),
                queue_bytes: 2,
                queue_count: 1,
                restore_bytes: 3,
            },
            MemoryBudget::new(2),
            old.clone(),
        )
        .unwrap();
        swap.enqueue(
            1,
            Payload {
                buffer: old.try_buffer(1, 7u8).unwrap().freeze(),
                pause: None,
            },
        );
        swap.wait_idle().unwrap();
        swap.enqueue(
            2,
            Payload {
                buffer: old.try_buffer(2, 8u8).unwrap().freeze(),
                pause: None,
            },
        );
        swap.wait_idle().unwrap();
        let previous = swap.try_get(&1, || false).unwrap().unwrap();
        swap.set_restore_budget(current.clone());
        let start = std::sync::Barrier::new(8);
        let swap = &swap;
        let results = std::thread::scope(|scope| {
            let handles: Vec<_> = (0..8)
                .map(|_| {
                    let start = &start;
                    scope.spawn(move || {
                        start.wait();
                        swap.try_get(&2, || false)
                    })
                })
                .collect();
            handles
                .into_iter()
                .map(|handle| handle.join().unwrap())
                .collect::<Vec<_>>()
        });
        assert_eq!(
            results
                .iter()
                .filter(|result| matches!(result, Ok(Some(_))))
                .count(),
            1
        );
        assert_eq!(
            results
                .iter()
                .filter(|result| matches!(result, Err(rrrah_memory::BufferError::Capacity { .. })))
                .count(),
            7
        );
        assert_eq!(old.used(), 1);
        assert_eq!(current.used(), 2);
        drop(results);
        drop(previous);
        assert_eq!(old.used(), 0);
        assert_eq!(current.used(), 0);
        let retry = swap.try_get(&2, || false).unwrap().unwrap();
        assert_eq!(&*retry.buffer, &[8; 2]);
        drop(retry);
        assert_eq!(current.used(), 0);
    }
    #[test]
    fn restore_rebinding_preserves_aggregate_cap_until_last_old_owner_releases() {
        let directory = tempfile::tempdir().unwrap();
        let original = MemoryBudget::new(32);
        let replacement = MemoryBudget::new(32);
        let mut swap = ImageSwapCache::<u8, Payload>::new_with_budgets(
            directory.path(),
            ImageSwapConfig {
                limits: CacheLimits::bytes(32),
                queue_bytes: 4,
                queue_count: 1,
                restore_bytes: 4,
            },
            MemoryBudget::new(4),
            original.clone(),
        )
        .unwrap();
        swap.enqueue(
            1,
            Payload {
                buffer: original.try_buffer(4, 7u8).unwrap().freeze(),
                pause: None,
            },
        );
        swap.wait_idle().unwrap();
        let first = swap.try_get(&1, || false).unwrap().unwrap();
        let alias = first.buffer.clone();
        swap.set_restore_budget(original.clone());
        assert!(swap.try_get(&1, || false).is_err());
        swap.set_restore_budget(replacement.clone());
        drop(first);
        assert_eq!(original.used(), 4);
        assert_eq!(replacement.used(), 0);
        assert!(swap.try_get(&1, || false).is_err());
        drop(alias);
        assert_eq!(original.used(), 0);
        let retry = swap.try_get(&1, || false).unwrap().unwrap();
        assert_eq!(&*retry.buffer, &[7; 4]);
        assert_eq!(replacement.used(), 4);
        drop(retry);
        assert_eq!(replacement.used(), 0);
    }
    #[test]
    fn restore_rebinding_to_existing_budget_counts_nested_owners_once() {
        let directory = tempfile::tempdir().unwrap();
        let source = MemoryBudget::new(8);
        let mut swap = ImageSwapCache::<u8, Payload>::new(
            directory.path(),
            ImageSwapConfig {
                limits: CacheLimits::bytes(8),
                queue_bytes: 8,
                queue_count: 2,
                restore_bytes: 4,
            },
        )
        .unwrap();
        for (key, bytes) in [(1, 1), (2, 2)] {
            swap.enqueue(
                key,
                Payload {
                    buffer: source.try_buffer(bytes, key).unwrap().freeze(),
                    pause: None,
                },
            );
        }
        swap.wait_idle().unwrap();
        let first = swap.try_get(&1, || false).unwrap().unwrap();
        let original = swap.restore_budget().clone();
        swap.set_restore_budget(original.clone());
        let second = swap.try_get(&2, || false).unwrap().unwrap();
        assert_eq!(original.used(), 3);
        let third = swap.try_get(&1, || false).unwrap().unwrap();
        assert_eq!(original.used(), 4);
        assert!(swap.try_get(&1, || false).is_err());
        drop(first);
        let fourth = swap.try_get(&1, || false).unwrap().unwrap();
        assert_eq!(original.used(), 4);
        drop((second, third, fourth));
        assert_eq!(original.used(), 0);
        assert_eq!(source.used(), 0);
    }
    #[test]
    fn failed_queue_admission_releases_key_for_retry() {
        let directory = tempfile::tempdir().unwrap();
        let root = MemoryBudget::new(1);
        let queue = MemoryBudget::new(1);
        let swap = ImageSwapCache::<u8, Payload>::new_with_budgets(
            directory.path(),
            ImageSwapConfig {
                limits: CacheLimits::bytes(1),
                queue_bytes: 1,
                queue_count: 4,
                restore_bytes: 1,
            },
            queue.clone(),
            MemoryBudget::new(1),
        )
        .unwrap();
        let blocker = queue.try_reserve(1).unwrap();
        swap.enqueue(
            1,
            Payload {
                buffer: root.try_buffer(1, 7).unwrap().freeze(),
                pause: None,
            },
        );
        assert_eq!(root.used(), 0);
        assert_eq!(swap.stats().dropped, 1);
        drop(blocker);
        swap.enqueue(
            1,
            Payload {
                buffer: root.try_buffer(1, 8).unwrap().freeze(),
                pause: None,
            },
        );
        swap.wait_idle().unwrap();
        assert_eq!(swap.stats().writes, 1);
        assert_eq!(&*swap.get(&1, || false).unwrap().buffer, &[8]);
        assert_eq!(root.used(), 0);
        assert!(swap.pending.lock().unwrap().is_empty());
    }
    #[test]
    fn cancelled_generation_cleanup_preserves_new_generation_marker() {
        let keys = Arc::new(Mutex::new(HashMap::from([(7_u8, 1_u64)])));
        let old = PendingSpill {
            keys: keys.clone(),
            key: 7,
            generation: 1,
        };
        keys.lock().unwrap().insert(7, 2);
        let new = PendingSpill {
            keys: keys.clone(),
            key: 7,
            generation: 2,
        };
        drop(old);
        assert_eq!(keys.lock().unwrap().get(&7), Some(&2));
        drop(new);
        assert!(keys.lock().unwrap().is_empty());
    }
    #[test]
    fn stalled_writer_keeps_producer_nonblocking_and_bounds_count_bytes_and_owners() {
        let mut cases = 0;
        for queue_count in [1usize, 2, 4] {
            for queue_bytes in [16u64, 32, 64] {
                for cancel in [false, true] {
                    let directory = tempfile::tempdir().unwrap();
                    let root = MemoryBudget::new(64 + queue_bytes + 16);
                    let visible = root.try_buffer(64, 171u8).unwrap().freeze();
                    let queue = MemoryBudget::new(queue_bytes);
                    let swap = ImageSwapCache::<u8, Payload>::new_with_budgets(
                        directory.path(),
                        ImageSwapConfig {
                            limits: CacheLimits {
                                max_bytes: 4096,
                                max_entries: Some(16),
                                ttl: None,
                            },
                            queue_bytes,
                            queue_count,
                            restore_bytes: root.limit(),
                        },
                        queue.clone(),
                        root.clone(),
                    )
                    .unwrap();
                    let (started, started_rx) = mpsc::channel();
                    let (resume, resume_rx) = mpsc::channel();
                    assert_eq!(
                        swap.try_enqueue(
                            0,
                            Payload {
                                buffer: root.try_buffer(16, 0).unwrap().freeze(),
                                pause: Some((started, resume_rx, false)),
                            }
                        ),
                        SpillAdmission::Queued
                    );
                    started_rx
                        .recv_timeout(std::time::Duration::from_secs(5))
                        .unwrap();
                    let waiting = queue_count.min((queue_bytes / 16 - 1) as usize);
                    std::thread::scope(|scope| {
                        let (done, completed) = mpsc::channel();
                        let swap = &swap;
                        let root = &root;
                        let producer = scope.spawn(move || {
                            let results: Vec<_> = (1..=64u8)
                                .map(|key| {
                                    swap.try_enqueue(
                                        key,
                                        Payload {
                                            buffer: root.try_buffer(16, key).unwrap().freeze(),
                                            pause: None,
                                        },
                                    )
                                })
                                .collect();
                            done.send(results).unwrap();
                        });
                        // Completion while the writer is still gated proves that
                        // producer admission does not wait for filesystem progress.
                        let results = completed.recv_timeout(std::time::Duration::from_secs(5));
                        if let Ok(results) = &results {
                            assert_eq!(
                                results.iter().filter(|r| **r == SpillAdmission::Queued).count(),
                                waiting
                            );
                            assert_eq!(
                                results.iter().filter(|r| **r == SpillAdmission::Full).count(),
                                64 - waiting
                            );
                            assert_eq!(root.used(), 64 + (waiting as u64 + 1) * 16);
                            assert_eq!(queue.used(), (waiting as u64 + 1) * 16);
                            assert_eq!(swap.stats().queued_bytes, queue.used());
                            assert!(visible.iter().all(|v| *v == 171));
                        }
                        if cancel {
                            swap.discard_pending_writes();
                        }
                        // Release even after an observation timeout so the scoped
                        // thread can finish; the timeout remains a test failure.
                        resume.send(()).unwrap();
                        producer.join().unwrap();
                        assert!(results.is_ok(), "producer waited for the stalled writer");
                    });
                    swap.wait_idle().unwrap();
                    assert_eq!((root.used(), queue.used()), (64, 0));
                    assert_eq!(swap.stats().queued_bytes, 0);
                    assert_eq!(swap.stats().errors, 0);
                    assert_eq!(swap.stats().writes, if cancel { 0 } else { waiting as u64 + 1 });
                    assert_eq!(
                        swap.stats().dropped,
                        if cancel { 65 } else { 64 - waiting as u64 }
                    );
                    assert_eq!(
                        swap.try_enqueue(
                            250,
                            Payload {
                                buffer: root.try_buffer(16, 250).unwrap().freeze(),
                                pause: None,
                            }
                        ),
                        SpillAdmission::Queued
                    );
                    swap.wait_idle().unwrap();
                    let restored = swap.get(&250, || false).unwrap();
                    assert_eq!(&*restored.buffer, &[250; 16]);
                    assert_eq!(root.used(), 80);
                    drop(restored);
                    assert!(visible.iter().all(|v| *v == 171));
                    swap.shutdown();
                    drop(swap);
                    assert_eq!(std::fs::read_dir(directory.path()).unwrap().count(), 0);
                    assert_eq!(root.used(), 64);
                    drop(visible);
                    assert_eq!((root.used(), queue.used()), (0, 0));
                    cases += 1;
                }
            }
        }
        assert_eq!(cases, 18);
        eprintln!(
            "STALLED_SWAP_QUEUE cases={cases} producer_admissions={} recovery_restores={cases} final=0",
            cases * 64
        );
    }
    #[test]
    fn full_channel_releases_rejected_payload_and_accepts_same_key_later() {
        let directory = tempfile::tempdir().unwrap();
        let root = MemoryBudget::new(6);
        let swap = ImageSwapCache::<u8, Payload>::new(
            directory.path(),
            ImageSwapConfig {
                limits: CacheLimits::bytes(6),
                queue_bytes: 6,
                queue_count: 2,
                restore_bytes: 6,
            },
        )
        .unwrap();
        let (started, started_rx) = mpsc::channel();
        let (resume, resume_rx) = mpsc::channel();
        swap.enqueue(
            0,
            Payload {
                buffer: root.try_buffer(1, 0).unwrap().freeze(),
                pause: Some((started, resume_rx, false)),
            },
        );
        started_rx
            .recv_timeout(std::time::Duration::from_secs(5))
            .unwrap();
        for key in 1..=5 {
            swap.enqueue(
                key,
                Payload {
                    buffer: root.try_buffer(1, key).unwrap().freeze(),
                    pause: None,
                },
            );
        }
        assert_eq!(swap.stats().dropped, 3);
        assert_eq!(root.used(), 3);
        assert_eq!(swap.stats().queued_bytes, 3);
        assert!(!swap.pending.lock().unwrap().contains_key(&5));
        resume.send(()).unwrap();
        swap.wait_idle().unwrap();
        assert_eq!(root.used(), 0);
        swap.enqueue(
            5,
            Payload {
                buffer: root.try_buffer(1, 5).unwrap().freeze(),
                pause: None,
            },
        );
        swap.wait_idle().unwrap();
        assert_eq!(swap.stats().writes, 4);
        assert_eq!(&*swap.get(&5, || false).unwrap().buffer, &[5]);
        assert_eq!(root.used(), 0);
        assert_eq!(swap.stats().queued_bytes, 0);
    }
    struct FailingPayload(Payload, bool);
    #[test]
    fn admission_reports_oversize_busy_and_completed_duplicate_without_leaking() {
        let directory = tempfile::tempdir().unwrap();
        let root = MemoryBudget::new(8);
        let swap = ImageSwapCache::<u8, Payload>::new(
            directory.path(),
            ImageSwapConfig {
                limits: CacheLimits::bytes(8),
                queue_bytes: 1,
                queue_count: 2,
                restore_bytes: 8,
            },
        )
        .unwrap();
        let payload = |len| Payload {
            buffer: root.try_buffer(len, 7).unwrap().freeze(),
            pause: None,
        };
        assert_eq!(swap.try_enqueue(1, payload(2)), SpillAdmission::TooLarge);
        assert_eq!(root.used(), 0);
        assert!(swap.pending.lock().unwrap().is_empty());
        {
            let _guard = swap.pending.lock().unwrap();
            assert_eq!(swap.try_enqueue(1, payload(1)), SpillAdmission::Busy);
        }
        assert_eq!(root.used(), 0);
        let credit = swap.queue_budget.try_reserve(1).unwrap();
        assert_eq!(swap.try_enqueue(1, payload(1)), SpillAdmission::Full);
        assert!(swap.pending.lock().unwrap().is_empty());
        assert_eq!(root.used(), 0);
        drop(credit);
        assert_eq!(swap.try_enqueue(1, payload(1)), SpillAdmission::Queued);
        swap.wait_idle().unwrap();
        assert_eq!(swap.try_enqueue(1, payload(1)), SpillAdmission::AlreadyPresent);
        assert_eq!(swap.stats().writes, 1);
        assert_eq!(root.used(), 0);
        assert_eq!(swap.stats().queued_bytes, 0);
    }
    #[test]
    fn zero_queue_count_disables_spills_without_retaining_memory() {
        let directory = tempfile::tempdir().unwrap();
        let root = MemoryBudget::new(1);
        let swap = ImageSwapCache::<u8, Payload>::new(
            directory.path(),
            ImageSwapConfig {
                limits: CacheLimits::bytes(1),
                queue_bytes: 1,
                queue_count: 0,
                restore_bytes: 1,
            },
        )
        .unwrap();
        let admission = swap.try_enqueue(
            1,
            Payload {
                buffer: root.try_buffer(1, 7).unwrap().freeze(),
                pause: None,
            },
        );
        assert_eq!(admission, SpillAdmission::Disabled);
        swap.wait_idle().unwrap();
        assert_eq!(root.used(), 0);
        assert_eq!(swap.stats().queued_bytes, 0);
        assert_eq!(swap.stats().writes, 0);
        assert_eq!(swap.stats().dropped, 1);
        assert!(swap.pending.lock().unwrap().is_empty());
    }
    #[test]
    fn oversized_queue_is_rejected_before_store_creation() {
        let directory = tempfile::tempdir().unwrap();
        for count in [MAX_SWAP_QUEUE_COUNT + 1, usize::MAX] {
            let result = ImageSwapCache::<u8, Payload>::new(
                directory.path(),
                ImageSwapConfig {
                    limits: CacheLimits::bytes(1),
                    queue_bytes: 1,
                    queue_count: count,
                    restore_bytes: 1,
                },
            );
            let Err(error) = result else {
                panic!("oversized queue admitted")
            };
            assert_eq!(error.kind(), std::io::ErrorKind::InvalidInput);
            assert_eq!(std::fs::read_dir(directory.path()).unwrap().count(), 0);
        }
    }
    #[test]
    fn external_budgets_cannot_bypass_local_queue_or_restore_caps() {
        let directory = tempfile::tempdir().unwrap();
        let pixels = MemoryBudget::new(8);
        let queue = MemoryBudget::new(8);
        let restore = MemoryBudget::new(8);
        let mut swap = ImageSwapCache::<u8, Payload>::new_with_budgets(
            directory.path(),
            ImageSwapConfig {
                limits: CacheLimits::bytes(8),
                queue_bytes: 1,
                queue_count: 4,
                restore_bytes: 0,
            },
            queue.clone(),
            restore.clone(),
        )
        .unwrap();
        let (started, started_rx) = mpsc::channel();
        let (resume, resume_rx) = mpsc::channel();
        swap.enqueue(
            1,
            Payload {
                buffer: pixels.try_buffer(1, 7).unwrap().freeze(),
                pause: Some((started, resume_rx, false)),
            },
        );
        started_rx
            .recv_timeout(std::time::Duration::from_secs(5))
            .unwrap();
        swap.enqueue(
            2,
            Payload {
                buffer: pixels.try_buffer(1, 8).unwrap().freeze(),
                pause: None,
            },
        );
        assert_eq!(pixels.used(), 1);
        assert_eq!(queue.used(), 1);
        assert_eq!(swap.stats().dropped, 1);
        resume.send(()).unwrap();
        swap.wait_idle().unwrap();
        assert_eq!(queue.used(), 0);
        assert_eq!(pixels.used(), 0);
        assert_eq!(swap.restore_budget().limit(), 0);
        assert!(matches!(
            swap.try_get(&1, || false),
            Err(rrrah_memory::BufferError::Capacity { limit: 0, .. })
        ));
        assert_eq!(restore.used(), 0);
        let replacement = MemoryBudget::new(1024);
        swap.set_restore_budget(replacement.clone());
        assert_eq!(swap.restore_budget().limit(), 0);
        assert!(matches!(
            swap.try_get(&1, || false),
            Err(rrrah_memory::BufferError::Capacity { limit: 0, .. })
        ));
        assert_eq!(replacement.used(), 0);
        assert!(swap.index.lock().unwrap().get(&1).is_some());
        assert_eq!(swap.store.usage().unwrap().objects, 1);
    }
    impl SwapPayload for FailingPayload {
        fn capacity_bytes(&self) -> u64 {
            self.0.capacity_bytes()
        }
        fn payload_len(&self) -> std::io::Result<u64> {
            self.0.payload_len()
        }
        fn write_payload(&self, writer: &mut impl Write) -> std::io::Result<()> {
            self.0.write_payload(writer)?;
            if self.1 {
                return Err(std::io::Error::other("injected codec failure after write"));
            }
            Ok(())
        }
        fn read_payload(
            reader: &mut impl Read,
            bytes: u64,
            budget: &MemoryBudget,
        ) -> Result<Self, SwapPayloadError> {
            Payload::read_payload(reader, bytes, budget).map(|value| Self(value, false))
        }
    }
    #[test]
    fn failed_replacement_obeys_hard_quota_and_exposes_lru_eviction_boundary() {
        for limit in [1u64, 2] {
            for cancel in [false, true] {
                let directory = tempfile::tempdir().unwrap();
                let root = MemoryBudget::new(1);
                let swap = ImageSwapCache::<u8, FailingPayload>::new(
                    directory.path(),
                    ImageSwapConfig {
                        limits: CacheLimits::bytes(limit),
                        queue_bytes: 1,
                        queue_count: 1,
                        restore_bytes: 1,
                    },
                )
                .unwrap();
                let payload = |value, fail| {
                    FailingPayload(
                        Payload {
                            buffer: root.try_buffer(1, value).unwrap().freeze(),
                            pause: None,
                        },
                        fail,
                    )
                };
                assert_eq!(swap.try_enqueue(1, payload(7, false)), SpillAdmission::Queued);
                swap.wait_idle().unwrap();
                assert_eq!(swap.disk_usage().unwrap().bytes, 1);
                let (started, started_rx) = mpsc::channel();
                let (resume, resume_rx) = mpsc::channel();
                let mut replacement = payload(9, !cancel);
                if cancel {
                    replacement.0.pause = Some((started, resume_rx, false));
                }
                assert_eq!(swap.try_enqueue(2, replacement), SpillAdmission::Queued);
                if cancel {
                    started_rx
                        .recv_timeout(std::time::Duration::from_secs(5))
                        .unwrap();
                    swap.discard_pending_writes();
                    resume.send(()).unwrap();
                }
                swap.wait_idle().unwrap();
                assert_eq!((root.used(), swap.stats().queued_bytes), (0, 0));
                assert_eq!(
                    (swap.stats().writes, swap.stats().errors),
                    (1, if cancel { 0 } else { 1 })
                );
                assert_eq!(swap.stats().dropped, if cancel { 1 } else { 0 });
                assert!(swap.pending.lock().unwrap().is_empty());
                assert!(swap.get(&2, || false).is_none());
                let previous = swap.get(&1, || false);
                // Hard quota requires eviction before staging a new object. A codec
                // failure rolls back its own file, but cannot resurrect that victim.
                assert_eq!(previous.is_some(), limit == 2);
                if let Some(previous) = previous {
                    assert_eq!(&*previous.0.buffer, &[7]);
                }
                assert_eq!(root.used(), 0);
                assert_eq!(swap.disk_usage().unwrap().bytes, if limit == 2 { 1 } else { 0 });
                assert_eq!(swap.try_enqueue(2, payload(11, false)), SpillAdmission::Queued);
                swap.wait_idle().unwrap();
                assert_eq!(&*swap.get(&2, || false).unwrap().0.buffer, &[11]);
                assert_eq!(swap.stats().writes, 2);
                assert!(swap.disk_usage().unwrap().bytes <= limit);
                swap.shutdown();
                drop(swap);
                assert_eq!(root.used(), 0);
                assert_eq!(std::fs::read_dir(directory.path()).unwrap().count(), 0);
            }
        }
    }
    #[test]
    fn codec_failure_after_write_rolls_back_disk_memory_and_pending_key() {
        let directory = tempfile::tempdir().unwrap();
        let root = MemoryBudget::new(1);
        let swap = ImageSwapCache::<u8, FailingPayload>::new(
            directory.path(),
            ImageSwapConfig {
                limits: CacheLimits::bytes(1),
                queue_bytes: 1,
                queue_count: 4,
                restore_bytes: 1,
            },
        )
        .unwrap();
        swap.enqueue(
            1,
            FailingPayload(
                Payload {
                    buffer: root.try_buffer(1, 9).unwrap().freeze(),
                    pause: None,
                },
                true,
            ),
        );
        swap.wait_idle().unwrap();
        assert_eq!(swap.stats().errors, 1);
        assert_eq!(swap.stats().writes, 0);
        assert_eq!(root.used(), 0);
        assert_eq!(swap.stats().queued_bytes, 0);
        assert_eq!(swap.store.usage().unwrap(), rrrah_swap::SwapUsage::default());
        assert!(swap.pending.lock().unwrap().is_empty());
        assert!(swap.get(&1, || false).is_none());
        swap.enqueue(
            1,
            FailingPayload(
                Payload {
                    buffer: root.try_buffer(1, 10).unwrap().freeze(),
                    pause: None,
                },
                false,
            ),
        );
        swap.wait_idle().unwrap();
        assert_eq!(swap.stats().writes, 1);
        assert_eq!(&*swap.get(&1, || false).unwrap().0.buffer, &[10]);
        assert_eq!(root.used(), 0);
    }
}

#[cfg(test)]
mod raster_tests {
    use super::*;
    use rrrah_core::{RasterColorSpace, RasterPixels};
    use std::time::Duration;
    fn frame(budget: &MemoryBudget) -> DecodedRaster {
        DecodedRaster::new(
            1,
            1,
            RasterPixels::Rgba32Float(Arc::new(vec![8.0, -0.0, f32::from_bits(0x7fc01234), 1.0]).into()),
            RasterColorSpace::Icc(vec![1, 2, 3]),
        )
        .unwrap()
        .with_sample_scale(2.5)
        .unwrap()
        .with_hotspot(Some((0, 0)))
        .unwrap()
        .with_image_selection(1, 3)
        .unwrap()
        .try_manage_pixels(budget)
        .unwrap()
    }
    fn config(limits: CacheLimits) -> ImageSwapConfig {
        ImageSwapConfig {
            limits,
            queue_bytes: 19,
            queue_count: 4,
            restore_bytes: 19,
        }
    }
    fn assert_frame(actual: &DecodedRaster) {
        let RasterPixels::Rgba32Float(p) = actual.pixels() else {
            panic!()
        };
        assert!(p.is_managed());
        assert_eq!(
            p.iter().map(|x| x.to_bits()).collect::<Vec<_>>(),
            [
                8.0f32.to_bits(),
                (-0.0f32).to_bits(),
                0x7fc01234,
                1.0f32.to_bits()
            ]
        );
        assert_eq!(actual.color_space(), &RasterColorSpace::Icc(vec![1, 2, 3]));
        assert_eq!(actual.sample_scale(), 2.5);
        assert_eq!(actual.hotspot(), Some((0, 0)));
        assert_eq!((actual.image_index(), actual.image_count()), (1, 3));
    }
    fn stored_path(parent: &Path) -> std::path::PathBuf {
        let directory = std::fs::read_dir(parent).unwrap().next().unwrap().unwrap().path();
        std::fs::read_dir(directory)
            .unwrap()
            .next()
            .unwrap()
            .unwrap()
            .path()
    }
    #[test]
    fn corrupt_and_truncated_raster_restore_release_memory_and_remove_entry() {
        for truncate in [false, true] {
            let parent = tempfile::tempdir().unwrap();
            let budget = MemoryBudget::new(19);
            let swap = RasterSwapCache::new_with_budgets(
                parent.path(),
                config(CacheLimits::bytes(1024)),
                MemoryBudget::new(19),
                budget.clone(),
            )
            .unwrap();
            swap.enqueue(1u32, frame(&budget));
            swap.wait_idle().unwrap();
            let path = stored_path(parent.path());
            let mut bytes = std::fs::read(&path).unwrap();
            if truncate {
                bytes.pop();
            } else {
                let last = bytes.len() - 1;
                bytes[last] ^= 1;
            }
            std::fs::write(path, bytes).unwrap();
            assert!(swap.try_get(&1, || false).unwrap().is_none());
            assert_eq!(budget.used(), 0);
            assert_eq!(budget.peak(), 19);
            assert_eq!(swap.stats().errors, 1);
            assert_eq!(swap.store.usage().unwrap().bytes, 0);
            assert!(swap.index.lock().unwrap().is_empty());
            assert!(swap.try_get(&1, || false).unwrap().is_none());
            assert_eq!(swap.stats().errors, 1);
            swap.enqueue(1, frame(&budget));
            swap.wait_idle().unwrap();
            assert_frame(&swap.try_get(&1, || false).unwrap().unwrap());
            assert_eq!(budget.used(), 0);
        }
    }
    fn qualify_wire_corruption(animated: bool) {
        let parent = tempfile::tempdir().unwrap();
        let budget = MemoryBudget::new(19);
        let swap = RasterSwapCache::new_with_budgets(
            parent.path(),
            config(CacheLimits::bytes(1024)),
            MemoryBudget::new(19),
            budget.clone(),
        )
        .unwrap();
        let timing = animated.then(|| rrrah_core::RasterAnimationTiming::new(1000, 60, 3).unwrap());
        let make = || frame(&budget).with_animation_timing(timing);
        let check = |frame: &DecodedRaster| {
            assert_frame(frame);
            assert_eq!(frame.animation_timing(), timing);
        };
        swap.enqueue(1u32, make());
        swap.wait_idle().unwrap();
        let first_path = stored_path(parent.path());
        let directory = first_path.parent().unwrap();
        let original = std::fs::read(&first_path).unwrap();
        swap.enqueue(2u32, make());
        swap.wait_idle().unwrap();
        let neighbour_path = std::fs::read_dir(directory)
            .unwrap()
            .map(|entry| entry.unwrap().path())
            .find(|path| *path != first_path)
            .unwrap();
        let neighbour_usage = original.len() as u64;
        let mut cases = 0;
        // Mutations exercise descriptor, profile and arbitrary float payload bits.
        // The independent expected result is rejection for every changed wire object.
        let mutations = (0..original.len() * 8)
            .map(|bit| {
                let mut bytes = original.clone();
                bytes[bit / 8] ^= 1 << (bit % 8);
                bytes
            })
            .chain((0..original.len()).map(|end| original[..end].to_vec()))
            .chain([{
                let mut bytes = original.clone();
                bytes.push(0);
                bytes
            }]);
        for bytes in mutations {
            let path = std::fs::read_dir(directory)
                .unwrap()
                .map(|entry| entry.unwrap().path())
                .find(|path| *path != neighbour_path)
                .unwrap();
            std::fs::write(path, bytes).unwrap();
            assert!(swap.try_get(&1, || false).unwrap().is_none(), "case {cases}");
            assert_eq!(budget.used(), 0, "case {cases}");
            assert_eq!(swap.disk_usage().unwrap().bytes, neighbour_usage, "case {cases}");
            assert_eq!(swap.disk_usage().unwrap().objects, 1);
            let errors = swap.stats().errors;
            assert_eq!(errors, cases + 1);
            assert!(swap.try_get(&1, || false).unwrap().is_none());
            assert_eq!(swap.stats().errors, errors);
            check(&swap.try_get(&2, || false).unwrap().unwrap());
            assert_eq!(budget.used(), 0);
            swap.enqueue(1u32, make());
            swap.wait_idle().unwrap();
            check(&swap.try_get(&1, || false).unwrap().unwrap());
            assert_eq!(budget.used(), 0);
            assert_eq!(swap.stats().queued_bytes, 0);
            cases += 1;
        }
        assert_eq!(cases as usize, original.len() * 9 + 1);
        assert_eq!(swap.stats().reads, cases * 2);
        assert_eq!(budget.peak(), 19);
        drop(swap);
        assert_eq!(budget.used(), 0);
        eprintln!(
            "RASTER_SWAP_CORRUPTION animated={animated} cases={cases} neighbouring_object_preserved=true retries={cases} final=0"
        );
    }
    #[test]
    fn every_wire_bit_truncation_and_tail_failure_preserves_neighbour_and_retry() {
        qualify_wire_corruption(false);
    }
    #[test]
    fn every_animation_v5_wire_mutation_preserves_neighbour_timing_and_retry() {
        qualify_wire_corruption(true);
    }
    #[test]
    fn cancelled_stream_restore_retains_disk_entry_and_releases_partial_allocation() {
        let parent = tempfile::tempdir().unwrap();
        let budget = MemoryBudget::new(19);
        let swap = RasterSwapCache::new_with_budgets(
            parent.path(),
            config(CacheLimits::bytes(1024)),
            MemoryBudget::new(19),
            budget.clone(),
        )
        .unwrap();
        swap.enqueue(1u32, frame(&budget));
        swap.wait_idle().unwrap();
        // Cancel on sample read after header/profile parsing and pixel admission.
        let mut polls = 0;
        let mut allocation_live_at_cancel = false;
        assert!(
            swap.try_get(&1, || {
                polls += 1;
                allocation_live_at_cancel |= budget.used() == 19;
                allocation_live_at_cancel
            })
            .unwrap()
            .is_none()
        );
        assert!(polls > 1);
        assert!(allocation_live_at_cancel);
        assert_eq!(budget.used(), 0);
        assert_eq!(swap.stats().errors, 0);
        assert_eq!(swap.store.usage().unwrap().bytes, 83);
        assert_frame(&swap.try_get(&1, || false).unwrap().unwrap());
        assert_eq!(budget.used(), 0);
    }
    #[test]
    fn corrupt_old_restore_cannot_remove_replacement_for_same_key() {
        let parent = tempfile::tempdir().unwrap();
        let budget = MemoryBudget::new(19);
        let swap = RasterSwapCache::new_with_budgets(
            parent.path(),
            config(CacheLimits::bytes(1024)),
            MemoryBudget::new(19),
            budget.clone(),
        )
        .unwrap();
        swap.enqueue(1u32, frame(&budget));
        swap.wait_idle().unwrap();
        let old_path = stored_path(parent.path());
        let mut corrupt = std::fs::read(&old_path).unwrap();
        let last = corrupt.len() - 1;
        corrupt[last] ^= 1;
        std::fs::write(old_path, corrupt).unwrap();
        let fresh = frame(&budget);
        let replacement = swap
            .store
            .write_stream(
                crate::raster_payload_len(&fresh).unwrap(),
                || false,
                |mut writer| crate::write_raster_payload(&mut writer, &fresh).map_err(std::io::Error::other),
            )
            .unwrap();
        drop(fresh);
        let mut polls = 0;
        assert!(
            swap.try_get(&1, || {
                polls += 1;
                if polls == 2 {
                    // old handle already selected, before its read
                    swap.index.lock().unwrap().insert_prefetch(
                        1,
                        replacement.clone().into(),
                        replacement.byte_len(),
                    );
                }
                false
            })
            .unwrap()
            .is_none()
        );
        assert_eq!(swap.stats().errors, 1);
        assert_eq!(budget.used(), 0);
        assert_frame(&swap.try_get(&1, || false).unwrap().unwrap());
        assert_eq!(swap.stats().reads, 1);
        assert_eq!(budget.used(), 0);
    }
    #[test]
    #[ignore = "manual 60-second collection swap pressure soak"]
    fn sustained_collection_restores_under_full_memory_pressure() {
        let count: u32 = std::env::var("RRRAH_SWAP_SOAK_OBJECTS")
            .map(|value| value.parse().expect("RRRAH_SWAP_SOAK_OBJECTS must be u32"))
            .unwrap_or(256);
        let seconds: u64 = std::env::var("RRRAH_SWAP_SOAK_SECONDS")
            .map(|value| value.parse().expect("RRRAH_SWAP_SOAK_SECONDS must be u64"))
            .unwrap_or(60);
        assert!((1..=65536).contains(&count));
        assert!(seconds > 0);
        const BYTES: u64 = 128 * 128 * 4;
        let parent = tempfile::tempdir().unwrap();
        let budget = MemoryBudget::new(BYTES * 2);
        let swap = RasterSwapCache::new_with_budgets(
            parent.path(),
            ImageSwapConfig {
                limits: CacheLimits {
                    max_bytes: (BYTES + 64) * u64::from(count),
                    max_entries: Some(count as usize),
                    ttl: None,
                },
                queue_bytes: BYTES,
                queue_count: 1,
                restore_bytes: BYTES * 2,
            },
            MemoryBudget::new(BYTES),
            budget.child(BYTES * 2),
        )
        .unwrap();
        for key in 0..count {
            let mut values = budget.try_buffer(BYTES as usize, 0u8).unwrap();
            for (i, pixel) in values.as_chunks_mut::<4>().0.iter_mut().enumerate() {
                pixel.copy_from_slice(&[key as u8, (key >> 8) as u8, i as u8, 255]);
            }
            let frame = DecodedRaster::new(
                128,
                128,
                RasterPixels::Rgba8(values.freeze().into()),
                RasterColorSpace::Srgb,
            )
            .unwrap();
            swap.enqueue(key, frame);
            swap.wait_idle().unwrap();
            assert_eq!(budget.used(), 0);
        }
        assert_eq!(swap.stats().writes, u64::from(count));
        let disk = swap.disk_usage().unwrap();
        assert_eq!(disk.objects, count as usize);
        assert_eq!(disk.bytes, (BYTES + 64) * u64::from(count));
        let pressure = budget.try_reserve(BYTES).unwrap();
        let start = std::time::Instant::now();
        let mut cycles = 0u64;
        let mut last_progress = start;
        while start.elapsed() < Duration::from_secs(seconds) {
            let key = (cycles % u64::from(count)) as u32;
            let restored = swap.try_get(&key, || false).unwrap().unwrap();
            let RasterPixels::Rgba8(values) = restored.pixels() else {
                panic!()
            };
            for (i, pixel) in values.as_chunks::<4>().0.iter().enumerate() {
                assert_eq!(*pixel, [key as u8, (key >> 8) as u8, i as u8, 255]);
            }
            assert_eq!(budget.used(), BYTES * 2);
            let held = restored.clone();
            drop(restored);
            let next = (key + 1) % count;
            assert!(matches!(
                swap.try_get(&next, || false),
                Err(rrrah_memory::BufferError::Capacity { .. })
            ));
            assert_eq!(budget.used(), BYTES * 2);
            drop(held);
            assert_eq!(budget.used(), BYTES);
            assert!(swap.try_get(&next, || true).unwrap().is_none());
            let retry = swap.try_get(&next, || false).unwrap().unwrap();
            let RasterPixels::Rgba8(values) = retry.pixels() else {
                panic!()
            };
            for (i, pixel) in values.as_chunks::<4>().0.iter().enumerate() {
                assert_eq!(*pixel, [next as u8, (next >> 8) as u8, i as u8, 255]);
            }
            drop(retry);
            assert_eq!(budget.used(), BYTES);
            assert_eq!(swap.disk_usage().unwrap(), disk);
            cycles += 1;
            if cycles.is_multiple_of(1024) && last_progress.elapsed() >= Duration::from_secs(10) {
                eprintln!(
                    "RASTER_SWAP_SOAK_PROGRESS seconds={:.1} objects={count} cycles={cycles} reads={} used={} peak={}",
                    start.elapsed().as_secs_f64(),
                    swap.stats().reads,
                    budget.used(),
                    budget.peak()
                );
                last_progress = std::time::Instant::now();
            }
        }
        assert!(cycles >= u64::from(count));
        assert_eq!(swap.stats().reads, cycles * 2);
        assert_eq!((swap.stats().errors, swap.stats().queued_bytes), (0, 0));
        drop(pressure);
        assert_eq!((budget.used(), budget.peak()), (0, BYTES * 2));
        swap.shutdown();
        drop(swap);
        assert_eq!(std::fs::read_dir(parent.path()).unwrap().count(), 0);
        eprintln!(
            "RASTER_SWAP_SOAK seconds={} objects={count} cycles={cycles} verified_pixel_bytes={} pressure_refusals={cycles} retries={cycles} peak={} final=0",
            start.elapsed().as_secs_f64(),
            cycles * 2 * BYTES,
            BYTES * 2
        );
    }
    #[test]
    fn raster_swap_preserves_hdr_metadata_and_retries_memory_pressure() {
        let root = MemoryBudget::new(19);
        let swap = RasterSwapCache::new_with_budgets(
            &std::env::temp_dir(),
            config(CacheLimits::bytes(1024)),
            MemoryBudget::new(19),
            root.child(19),
        )
        .unwrap();
        let original = frame(&root);
        assert_eq!(swap.minimum_restore_weight(&1), None);
        let expected_floor = original.minimum_restore_weight().unwrap();
        swap.enqueue(1u32, original.clone()); // full allocation root must not prevent spill
        swap.wait_idle().unwrap();
        assert_eq!(swap.stats().writes, 1);
        assert_eq!(swap.stats().queued_bytes, 0);
        // The index hint stays available under exhausted restore credit.
        assert_eq!(swap.minimum_restore_weight(&1), Some(expected_floor));
        assert_eq!(swap.stats().reads, 0);
        assert_eq!(root.used(), 19);
        assert!(matches!(
            swap.try_get(&1, || false),
            Err(rrrah_memory::BufferError::Capacity { .. })
        ));
        assert_eq!(swap.stats().errors, 0);
        assert!(swap.try_get(&1, || true).unwrap().is_none());
        drop(original);
        let restored = swap.try_get(&1, || false).unwrap().unwrap();
        assert_frame(&restored);
        assert_eq!(root.used(), 19);
        let held = restored.clone();
        drop(restored);
        drop(swap);
        assert_eq!(root.used(), 19);
        drop(held);
        assert_eq!(root.used(), 0);
    }
    #[test]
    fn raster_disk_byte_count_ttl_and_queue_limits_are_independent() {
        for limits in [
            CacheLimits::bytes(83),
            CacheLimits {
                max_bytes: 1024,
                max_entries: Some(1),
                ttl: None,
            },
        ] {
            let source = MemoryBudget::new(19);
            let swap = RasterSwapCache::new(&std::env::temp_dir(), config(limits)).unwrap();
            swap.enqueue(1u32, frame(&source));
            swap.wait_idle().unwrap();
            swap.enqueue(2, frame(&source));
            swap.wait_idle().unwrap();
            assert_eq!(swap.stats().writes, 2);
            assert!(swap.get(&1, || false).is_none());
            assert_frame(&swap.get(&2, || false).unwrap());
            assert_eq!(source.used(), 0);
        }
        let source = MemoryBudget::new(19);
        let swap = RasterSwapCache::new(
            &std::env::temp_dir(),
            config(CacheLimits {
                max_bytes: 1024,
                max_entries: None,
                ttl: Some(Duration::ZERO),
            }),
        )
        .unwrap();
        swap.enqueue(1u32, frame(&source));
        swap.wait_idle().unwrap();
        assert!(swap.get(&1, || false).is_none());
        let swap = RasterSwapCache::new(
            &std::env::temp_dir(),
            ImageSwapConfig {
                queue_bytes: 0,
                queue_count: 4,
                ..config(CacheLimits::bytes(1024))
            },
        )
        .unwrap();
        swap.enqueue(1u32, frame(&source));
        swap.wait_idle().unwrap();
        assert_eq!(swap.stats().dropped, 1);
        assert_eq!(swap.stats().writes, 0);
        assert_eq!(source.used(), 0);
    }
    #[test]
    fn idle_ttl_maintenance_releases_files_and_quota_but_keeps_live_handles() {
        let source = MemoryBudget::new(19);
        let swap = RasterSwapCache::new(
            &std::env::temp_dir(),
            config(CacheLimits {
                max_bytes: 1024,
                max_entries: Some(2),
                ttl: Some(Duration::ZERO),
            }),
        )
        .unwrap();
        swap.enqueue(1u32, frame(&source));
        swap.wait_idle().unwrap();
        assert_eq!(source.used(), 0);
        assert_eq!(swap.store.usage().unwrap().objects, 1);
        // A restore can retain a handle while maintenance removes membership.
        let held = swap.index.lock().unwrap().remove(&1).unwrap();
        let bytes = held.byte_len();
        assert!(swap.index.lock().unwrap().insert_prefetch(1, held.clone(), bytes));
        assert_eq!(swap.prune_expired().unwrap(), 1);
        assert!(swap.get(&1, || false).is_none());
        assert_eq!(swap.store.usage().unwrap().objects, 1);
        assert_eq!(swap.store.usage().unwrap().bytes, bytes);
        assert_eq!(swap.prune_expired().unwrap(), 0);
        drop(held);
        assert_eq!(swap.store.usage().unwrap().objects, 0);
        assert_eq!(swap.store.usage().unwrap().bytes, 0);
        // No lookup of this key is needed to release an unleased expired file.
        swap.enqueue(2, frame(&source));
        swap.wait_idle().unwrap();
        assert_eq!(swap.store.usage().unwrap().objects, 1);
        assert_eq!(swap.prune_expired().unwrap(), 1);
        assert_eq!(swap.store.usage().unwrap().objects, 0);
        assert_eq!(swap.store.usage().unwrap().bytes, 0);
        assert_eq!(swap.stats().errors, 0);
    }
    #[test]
    fn ttl_worker_reclaims_idle_swap_without_foreground_calls() {
        let source = MemoryBudget::new(19);
        let swap = RasterSwapCache::new(
            &std::env::temp_dir(),
            config(CacheLimits {
                max_bytes: 1024,
                max_entries: Some(1),
                ttl: Some(Duration::ZERO),
            }),
        )
        .unwrap();
        swap.enqueue(1u32, frame(&source));
        swap.wait_idle().unwrap();
        assert_eq!(swap.stats().writes, 1);
        assert_eq!(source.used(), 0);
        let deadline = std::time::Instant::now() + Duration::from_secs(5);
        // Observe physical store accounting, never triggering index maintenance.
        while swap.store.usage().unwrap().objects != 0 {
            assert!(
                std::time::Instant::now() < deadline,
                "idle TTL worker did not reclaim swap"
            );
            std::thread::sleep(Duration::from_millis(10));
        }
        assert_eq!(swap.store.usage().unwrap().bytes, 0);
        assert!(swap.index.lock().unwrap().is_empty());
        assert_eq!(swap.stats().errors, 0);
    }
    #[test]
    fn nonzero_ttl_keeps_fresh_swap_then_reclaims_without_lookup() {
        let source = MemoryBudget::new(19);
        let ttl = Duration::from_secs(2);
        let swap = RasterSwapCache::new(
            &std::env::temp_dir(),
            config(CacheLimits {
                max_bytes: 1024,
                max_entries: Some(1),
                ttl: Some(ttl),
            }),
        )
        .unwrap();
        swap.enqueue(1u32, frame(&source));
        swap.wait_idle().unwrap();
        assert_eq!(swap.stats().writes, 1);
        assert_eq!(source.used(), 0);
        // Explicit maintenance must retain a newly committed, nonzero-TTL entry.
        assert_eq!(swap.prune_expired().unwrap(), 0);
        let initial = swap.store.usage().unwrap();
        assert_eq!(initial.objects, 1);
        assert!(initial.bytes > 0);
        let deadline = std::time::Instant::now() + Duration::from_secs(6);
        // Only observe physical accounting: no get, barrier, or prune can cause
        // expiration here. The background worker must release the stored file.
        while swap.store.usage().unwrap().objects != 0 {
            assert!(
                std::time::Instant::now() < deadline,
                "nonzero TTL worker did not reclaim idle swap"
            );
            std::thread::sleep(Duration::from_millis(10));
        }
        assert_eq!(swap.store.usage().unwrap().bytes, 0);
        assert!(swap.index.lock().unwrap().is_empty());
        assert_eq!(swap.stats().errors, 0);
    }
    #[test]
    fn ttl_worker_reclaims_swap_under_continuous_barriers() {
        let source = MemoryBudget::new(19);
        let swap = RasterSwapCache::new(
            &std::env::temp_dir(),
            config(CacheLimits {
                max_bytes: 1024,
                max_entries: Some(1),
                ttl: Some(Duration::ZERO),
            }),
        )
        .unwrap();
        swap.enqueue(1u32, frame(&source));
        swap.wait_idle().unwrap();
        assert_eq!(swap.stats().writes, 1);
        let deadline = std::time::Instant::now() + Duration::from_secs(3);
        // Each barrier wakes the worker well before its old one-second timeout.
        // Observe store usage without lookup or explicit expiry pruning.
        while swap.store.usage().unwrap().objects != 0 {
            assert!(
                std::time::Instant::now() < deadline,
                "continuous barriers starved TTL maintenance"
            );
            swap.wait_idle().unwrap();
            std::thread::sleep(Duration::from_millis(10));
        }
        assert_eq!(swap.store.usage().unwrap().bytes, 0);
        assert!(swap.index.lock().unwrap().is_empty());
        assert_eq!(source.used(), 0);
        assert_eq!(swap.stats().errors, 0);
    }
    #[test]
    fn raster_ram_eviction_spills_and_pressure_restore_preserves_visible_frame() {
        let root = MemoryBudget::new(38);
        let swap = RasterSwapCache::new_with_budgets(
            &std::env::temp_dir(),
            config(CacheLimits::bytes(1024)),
            MemoryBudget::new(38),
            root.child(38),
        )
        .unwrap();
        swap.enqueue(1u32, frame(&root));
        swap.wait_idle().unwrap();
        let mut ram = crate::RasterRamCache::new(CacheLimits::bytes(38));
        ram.enable_swap(swap);
        assert!(ram.insert_visible(2, frame(&root)));
        assert!(ram.insert(3, frame(&root)));
        assert_eq!(root.used(), 38);
        assert!(ram.get_background_with_cancel(&1, || false).is_none());
        assert_eq!(ram.len(), 2);
        assert_eq!(root.used(), 38);
        assert_eq!(ram.swap().unwrap().stats().errors, 0);
        let restored = ram.get_with_cancel(&1, || false).unwrap();
        assert_frame(&restored);
        assert!(ram.get(&2).is_some()); // pressure did not discard the old visible frame
        assert_eq!(ram.len(), 2);
        drop(restored);
        drop(ram);
        assert_eq!(root.used(), 0);

        let root = MemoryBudget::new(38);
        let swap = RasterSwapCache::new_with_budgets(
            &std::env::temp_dir(),
            config(CacheLimits::bytes(1024)),
            MemoryBudget::new(38),
            root.child(38),
        )
        .unwrap();
        let mut ram = crate::RasterRamCache::new(CacheLimits::bytes(19));
        ram.enable_swap(swap);
        assert!(ram.insert_visible(1u32, frame(&root)));
        assert!(ram.insert_visible(2, frame(&root)));
        ram.swap().unwrap().wait_idle().unwrap();
        assert_eq!(ram.swap().unwrap().stats().writes, 1);
        let restored = ram.get(&1).unwrap();
        assert_frame(&restored);
        ram.swap().unwrap().wait_idle().unwrap();
        drop(restored);
        drop(ram);
        assert_eq!(root.used(), 0);
    }
}
