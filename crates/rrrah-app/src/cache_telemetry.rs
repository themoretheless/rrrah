//! Lock-free cache telemetry shared by the foreground loader, write-back
//! worker, RAW prefetcher and the timing window.
//!
//! The render thread only takes an atomic snapshot. Potentially slow disk
//! accounting is performed by cache workers and published here afterwards.

use rrrah_cache::DiskCacheUsage;
use std::{
    sync::atomic::{AtomicBool, AtomicU8, AtomicU64, Ordering},
    time::Duration,
};

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LookupPhase {
    Disabled,
    Waiting,
    Reading,
    Hit,
    Miss,
    Error,
}

impl LookupPhase {
    fn from_raw(value: u8) -> Self {
        match value {
            1 => Self::Waiting,
            2 => Self::Reading,
            3 => Self::Hit,
            4 => Self::Miss,
            5 => Self::Error,
            _ => Self::Disabled,
        }
    }

    fn label(self) -> &'static str {
        match self {
            Self::Disabled => "DISABLED",
            Self::Waiting => "WAITING",
            Self::Reading => "READING",
            Self::Hit => "HIT",
            Self::Miss => "MISS",
            Self::Error => "ERROR",
        }
    }
}

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WriterPhase {
    Disabled,
    Idle,
    Debounce,
    Writing,
    Error,
}

impl WriterPhase {
    fn from_raw(value: u8) -> Self {
        match value {
            1 => Self::Idle,
            2 => Self::Debounce,
            3 => Self::Writing,
            4 => Self::Error,
            _ => Self::Disabled,
        }
    }

    fn label(self) -> &'static str {
        match self {
            Self::Disabled => "DISABLED",
            Self::Idle => "IDLE",
            Self::Debounce => "QUEUED",
            Self::Writing => "WRITING",
            Self::Error => "ERROR",
        }
    }
}

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PrefetchPhase {
    Disabled,
    Idle,
    Queued,
    Checking,
    Decoding,
    Writing,
    Paused,
    Complete,
    Error,
}

impl PrefetchPhase {
    fn from_raw(value: u8) -> Self {
        match value {
            1 => Self::Idle,
            2 => Self::Queued,
            3 => Self::Checking,
            4 => Self::Decoding,
            5 => Self::Writing,
            6 => Self::Paused,
            7 => Self::Complete,
            8 => Self::Error,
            _ => Self::Disabled,
        }
    }

    fn label(self) -> &'static str {
        match self {
            Self::Disabled => "DISABLED",
            Self::Idle => "IDLE",
            Self::Queued => "QUEUED",
            Self::Checking => "CHECKING",
            Self::Decoding => "DECODING",
            Self::Writing => "WRITING",
            Self::Paused => "PAUSED",
            Self::Complete => "READY",
            Self::Error => "ERROR",
        }
    }
}

#[derive(Debug)]
pub struct CacheTelemetry {
    enabled: AtomicBool,
    disk_limit_bytes: u64,
    lookup_generation: AtomicU64,
    lookup_phase: AtomicU8,
    foreground_hits: AtomicU64,
    foreground_misses: AtomicU64,
    read_errors: AtomicU64,
    bytes_served: AtomicU64,
    last_read_ns: AtomicU64,
    disk_resident_bytes: AtomicU64,
    disk_entries: AtomicU64,
    disk_scan_errors: AtomicU64,
    writer_phase: AtomicU8,
    writer_pending_bytes: AtomicU64,
    writer_active_bytes: AtomicU64,
    writes_committed: AtomicU64,
    writes_superseded: AtomicU64,
    write_failures: AtomicU64,
    bytes_written: AtomicU64,
    last_write_ns: AtomicU64,
    prefetch_generation: AtomicU64,
    prefetch_phase: AtomicU8,
    prefetch_planned: AtomicU64,
    prefetch_completed: AtomicU64,
    prefetch_cached: AtomicU64,
    prefetch_stored: AtomicU64,
    prefetch_failures: AtomicU64,
    prefetch_cancelled: AtomicU64,
    prefetch_bytes: AtomicU64,
}

impl CacheTelemetry {
    pub fn new(enabled: bool, disk_limit_bytes: u64) -> Self {
        let lookup_phase = if enabled {
            LookupPhase::Waiting
        } else {
            LookupPhase::Disabled
        };
        let worker_phase = if enabled {
            WriterPhase::Idle
        } else {
            WriterPhase::Disabled
        };
        let prefetch_phase = if enabled {
            PrefetchPhase::Idle
        } else {
            PrefetchPhase::Disabled
        };
        Self {
            enabled: AtomicBool::new(enabled),
            disk_limit_bytes,
            lookup_generation: AtomicU64::new(0),
            lookup_phase: AtomicU8::new(lookup_phase as u8),
            foreground_hits: AtomicU64::new(0),
            foreground_misses: AtomicU64::new(0),
            read_errors: AtomicU64::new(0),
            bytes_served: AtomicU64::new(0),
            last_read_ns: AtomicU64::new(0),
            disk_resident_bytes: AtomicU64::new(0),
            disk_entries: AtomicU64::new(0),
            disk_scan_errors: AtomicU64::new(0),
            writer_phase: AtomicU8::new(worker_phase as u8),
            writer_pending_bytes: AtomicU64::new(0),
            writer_active_bytes: AtomicU64::new(0),
            writes_committed: AtomicU64::new(0),
            writes_superseded: AtomicU64::new(0),
            write_failures: AtomicU64::new(0),
            bytes_written: AtomicU64::new(0),
            last_write_ns: AtomicU64::new(0),
            prefetch_generation: AtomicU64::new(0),
            prefetch_phase: AtomicU8::new(prefetch_phase as u8),
            prefetch_planned: AtomicU64::new(0),
            prefetch_completed: AtomicU64::new(0),
            prefetch_cached: AtomicU64::new(0),
            prefetch_stored: AtomicU64::new(0),
            prefetch_failures: AtomicU64::new(0),
            prefetch_cancelled: AtomicU64::new(0),
            prefetch_bytes: AtomicU64::new(0),
        }
    }

    pub fn begin_lookup(&self, generation: u64) {
        if self.enabled.load(Ordering::Relaxed) {
            self.lookup_generation.store(generation, Ordering::Release);
            self.lookup_phase
                .store(LookupPhase::Reading as u8, Ordering::Release);
        }
    }

    pub fn record_hit(&self, generation: u64, elapsed: Duration, bytes: u64) {
        if self.lookup_generation.load(Ordering::Acquire) != generation {
            return;
        }
        self.foreground_hits.fetch_add(1, Ordering::Relaxed);
        self.bytes_served.fetch_add(bytes, Ordering::Relaxed);
        self.last_read_ns.store(duration_ns(elapsed), Ordering::Relaxed);
        self.lookup_phase.store(LookupPhase::Hit as u8, Ordering::Release);
    }

    pub fn record_miss(&self, generation: u64) {
        if self.lookup_generation.load(Ordering::Acquire) != generation {
            return;
        }
        self.foreground_misses.fetch_add(1, Ordering::Relaxed);
        self.lookup_phase
            .store(LookupPhase::Miss as u8, Ordering::Release);
    }

    pub fn record_read_error(&self, generation: u64) {
        if self.lookup_generation.load(Ordering::Acquire) != generation {
            return;
        }
        self.read_errors.fetch_add(1, Ordering::Relaxed);
        self.lookup_phase
            .store(LookupPhase::Error as u8, Ordering::Release);
    }

    pub fn update_disk_usage(&self, usage: DiskCacheUsage) {
        self.disk_resident_bytes
            .store(usage.resident_bytes, Ordering::Relaxed);
        self.disk_entries.store(usage.entries, Ordering::Relaxed);
    }

    pub fn record_disk_scan_error(&self) {
        self.disk_scan_errors.fetch_add(1, Ordering::Relaxed);
    }

    pub fn writer_queued(&self, bytes: u64) {
        self.writer_pending_bytes.store(bytes, Ordering::Release);
    }

    pub fn writer_superseded(&self) {
        self.writer_pending_bytes.store(0, Ordering::Release);
        self.writes_superseded.fetch_add(1, Ordering::Relaxed);
    }

    pub fn writer_dequeued(&self, bytes: u64) {
        self.writer_pending_bytes.store(0, Ordering::Release);
        self.writer_active_bytes.store(bytes, Ordering::Release);
        self.writer_phase
            .store(WriterPhase::Debounce as u8, Ordering::Release);
    }

    pub fn writer_cancelled(&self) {
        self.writer_active_bytes.store(0, Ordering::Release);
        self.writes_superseded.fetch_add(1, Ordering::Relaxed);
        self.finish_writer_phase();
    }

    pub fn writer_started(&self) {
        self.writer_phase
            .store(WriterPhase::Writing as u8, Ordering::Release);
    }

    pub fn writer_finished(&self, bytes: u64, elapsed: Duration, success: bool) {
        self.writer_active_bytes.store(0, Ordering::Release);
        self.last_write_ns.store(duration_ns(elapsed), Ordering::Relaxed);
        if success {
            self.writes_committed.fetch_add(1, Ordering::Relaxed);
            self.bytes_written.fetch_add(bytes, Ordering::Relaxed);
        } else {
            self.write_failures.fetch_add(1, Ordering::Relaxed);
            self.writer_phase
                .store(WriterPhase::Error as u8, Ordering::Release);
            return;
        }
        self.finish_writer_phase();
    }

    fn finish_writer_phase(&self) {
        let next = if self.writer_pending_bytes.load(Ordering::Acquire) == 0 {
            WriterPhase::Idle
        } else {
            WriterPhase::Debounce
        };
        self.writer_phase.store(next as u8, Ordering::Release);
    }

    pub fn pause_prefetch(&self) {
        if !self.enabled.load(Ordering::Relaxed) {
            return;
        }
        let planned = self.prefetch_planned.load(Ordering::Acquire);
        let completed = self.prefetch_completed.load(Ordering::Acquire);
        if planned > completed {
            self.prefetch_cancelled.fetch_add(1, Ordering::Relaxed);
        }
        self.prefetch_phase
            .store(PrefetchPhase::Paused as u8, Ordering::Release);
    }

    pub fn idle_prefetch(&self) {
        if self.enabled.load(Ordering::Relaxed) {
            self.prefetch_phase
                .store(PrefetchPhase::Idle as u8, Ordering::Release);
        }
    }

    pub fn disable_prefetch(&self) {
        self.prefetch_phase
            .store(PrefetchPhase::Disabled as u8, Ordering::Release);
    }

    pub fn begin_prefetch(&self, generation: u64, planned: usize) {
        self.prefetch_generation.store(generation, Ordering::Release);
        self.prefetch_planned.store(planned as u64, Ordering::Relaxed);
        self.prefetch_completed.store(0, Ordering::Relaxed);
        self.prefetch_cached.store(0, Ordering::Relaxed);
        self.prefetch_stored.store(0, Ordering::Relaxed);
        self.prefetch_failures.store(0, Ordering::Relaxed);
        self.prefetch_bytes.store(0, Ordering::Relaxed);
        let phase = if planned == 0 {
            PrefetchPhase::Complete
        } else {
            PrefetchPhase::Queued
        };
        self.prefetch_phase.store(phase as u8, Ordering::Release);
    }

    pub fn set_prefetch_phase(&self, generation: u64, phase: PrefetchPhase) {
        if self.prefetch_generation.load(Ordering::Acquire) == generation {
            self.prefetch_phase.store(phase as u8, Ordering::Release);
        }
    }

    pub fn record_prefetch_cached(&self, generation: u64) {
        if self.prefetch_generation.load(Ordering::Acquire) == generation {
            self.prefetch_cached.fetch_add(1, Ordering::Relaxed);
            self.prefetch_completed.fetch_add(1, Ordering::Relaxed);
        }
    }

    pub fn record_prefetch_stored(&self, generation: u64, bytes: u64) {
        if self.prefetch_generation.load(Ordering::Acquire) == generation {
            self.prefetch_stored.fetch_add(1, Ordering::Relaxed);
            self.prefetch_bytes.fetch_add(bytes, Ordering::Relaxed);
            self.prefetch_completed.fetch_add(1, Ordering::Relaxed);
        }
    }

    pub fn record_prefetch_failure(&self, generation: u64) {
        if self.prefetch_generation.load(Ordering::Acquire) == generation {
            self.prefetch_failures.fetch_add(1, Ordering::Relaxed);
            self.prefetch_completed.fetch_add(1, Ordering::Relaxed);
            self.prefetch_phase
                .store(PrefetchPhase::Error as u8, Ordering::Release);
        }
    }

    pub fn finish_prefetch(&self, generation: u64) {
        self.set_prefetch_phase(generation, PrefetchPhase::Complete);
    }

    pub fn snapshot(&self) -> CacheTelemetrySnapshot {
        CacheTelemetrySnapshot {
            enabled: self.enabled.load(Ordering::Relaxed),
            disk_limit_bytes: self.disk_limit_bytes,
            lookup_phase: LookupPhase::from_raw(self.lookup_phase.load(Ordering::Acquire)),
            foreground_hits: self.foreground_hits.load(Ordering::Relaxed),
            foreground_misses: self.foreground_misses.load(Ordering::Relaxed),
            read_errors: self.read_errors.load(Ordering::Relaxed),
            bytes_served: self.bytes_served.load(Ordering::Relaxed),
            last_read_ns: self.last_read_ns.load(Ordering::Relaxed),
            disk_resident_bytes: self.disk_resident_bytes.load(Ordering::Relaxed),
            disk_entries: self.disk_entries.load(Ordering::Relaxed),
            disk_scan_errors: self.disk_scan_errors.load(Ordering::Relaxed),
            writer_phase: WriterPhase::from_raw(self.writer_phase.load(Ordering::Acquire)),
            writer_pending_bytes: self.writer_pending_bytes.load(Ordering::Acquire),
            writer_active_bytes: self.writer_active_bytes.load(Ordering::Acquire),
            writes_committed: self.writes_committed.load(Ordering::Relaxed),
            writes_superseded: self.writes_superseded.load(Ordering::Relaxed),
            write_failures: self.write_failures.load(Ordering::Relaxed),
            bytes_written: self.bytes_written.load(Ordering::Relaxed),
            last_write_ns: self.last_write_ns.load(Ordering::Relaxed),
            prefetch_phase: PrefetchPhase::from_raw(self.prefetch_phase.load(Ordering::Acquire)),
            prefetch_planned: self.prefetch_planned.load(Ordering::Relaxed),
            prefetch_completed: self.prefetch_completed.load(Ordering::Relaxed),
            prefetch_cached: self.prefetch_cached.load(Ordering::Relaxed),
            prefetch_stored: self.prefetch_stored.load(Ordering::Relaxed),
            prefetch_failures: self.prefetch_failures.load(Ordering::Relaxed),
            prefetch_cancelled: self.prefetch_cancelled.load(Ordering::Relaxed),
            prefetch_bytes: self.prefetch_bytes.load(Ordering::Relaxed),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CacheTelemetrySnapshot {
    pub enabled: bool,
    pub disk_limit_bytes: u64,
    pub lookup_phase: LookupPhase,
    pub foreground_hits: u64,
    pub foreground_misses: u64,
    pub read_errors: u64,
    pub bytes_served: u64,
    pub last_read_ns: u64,
    pub disk_resident_bytes: u64,
    pub disk_entries: u64,
    pub disk_scan_errors: u64,
    pub writer_phase: WriterPhase,
    pub writer_pending_bytes: u64,
    pub writer_active_bytes: u64,
    pub writes_committed: u64,
    pub writes_superseded: u64,
    pub write_failures: u64,
    pub bytes_written: u64,
    pub last_write_ns: u64,
    pub prefetch_phase: PrefetchPhase,
    pub prefetch_planned: u64,
    pub prefetch_completed: u64,
    pub prefetch_cached: u64,
    pub prefetch_stored: u64,
    pub prefetch_failures: u64,
    pub prefetch_cancelled: u64,
    pub prefetch_bytes: u64,
}

impl CacheTelemetrySnapshot {
    pub fn format_hud(self) -> String {
        if !self.enabled {
            return "CACHE L2 DISK\nSTATUS DISABLED\nRAM HOT CACHE OFF\nPREFETCH DISABLED".into();
        }
        let lookups = self.foreground_hits.saturating_add(self.foreground_misses);
        let hit_rate = self
            .foreground_hits
            .saturating_mul(100)
            .checked_div(lookups)
            .map_or_else(|| "--".into(), |percent| format!("{percent}%"));
        let disk_percent = if self.disk_limit_bytes == 0 {
            0
        } else {
            self.disk_resident_bytes
                .saturating_mul(100)
                .checked_div(self.disk_limit_bytes)
                .unwrap_or(0)
                .min(100)
        };
        let filled = usize::try_from(disk_percent / 5).unwrap_or(20).min(20);
        let disk_bar = format!("{}{}", "=".repeat(filled), "-".repeat(20 - filled));
        let current = if self.lookup_phase == LookupPhase::Hit {
            format!(
                "CURRENT {} {:.2} MS",
                self.lookup_phase.label(),
                nanos_ms(self.last_read_ns)
            )
        } else {
            format!("CURRENT {}", self.lookup_phase.label())
        };
        let writer_bytes = self.writer_active_bytes.max(self.writer_pending_bytes);
        format!(
            "CACHE L2 DISK\n{}\nSESSION {} HIT {} MISS {} ERR RATE {}\nSERVED {:.2} MB\nDISK {:.2}/{:.2} MB {} FILES\nDISK BAR {} {}% SNAP ERR {}\nWRITER {} {:.2} MB LAST {:.2} MS\nWRITES {} OK {} ERR {} DROP\nWROTE {:.2} MB\nPREFETCH {} {}/{}\nPREFETCH {} PRESENT {} STORED {} ERR\nPREFETCH DATA {:.2} MB CANCEL {}\nRAM HOT CACHE OFF",
            current,
            self.foreground_hits,
            self.foreground_misses,
            self.read_errors,
            hit_rate,
            megabytes(self.bytes_served),
            megabytes(self.disk_resident_bytes),
            megabytes(self.disk_limit_bytes),
            self.disk_entries,
            disk_bar,
            disk_percent,
            self.disk_scan_errors,
            self.writer_phase.label(),
            megabytes(writer_bytes),
            nanos_ms(self.last_write_ns),
            self.writes_committed,
            self.write_failures,
            self.writes_superseded,
            megabytes(self.bytes_written),
            self.prefetch_phase.label(),
            self.prefetch_completed,
            self.prefetch_planned,
            self.prefetch_cached,
            self.prefetch_stored,
            self.prefetch_failures,
            megabytes(self.prefetch_bytes),
            self.prefetch_cancelled,
        )
    }
}

fn duration_ns(duration: Duration) -> u64 {
    u64::try_from(duration.as_nanos()).unwrap_or(u64::MAX)
}

fn nanos_ms(ns: u64) -> f64 {
    ns as f64 / 1_000_000.0
}

fn megabytes(bytes: u64) -> f64 {
    bytes as f64 / (1024.0 * 1024.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn disabled_hud_does_not_claim_cache_activity() {
        let telemetry = CacheTelemetry::new(false, 8 * 1024 * 1024 * 1024);
        let hud = telemetry.snapshot().format_hud();
        assert!(hud.contains("STATUS DISABLED"));
        assert!(hud.contains("RAM HOT CACHE OFF"));
    }

    #[test]
    fn hit_rate_and_disk_bar_are_stable_at_boundaries() {
        let telemetry = CacheTelemetry::new(true, 100);
        telemetry.begin_lookup(1);
        telemetry.record_miss(1);
        telemetry.begin_lookup(2);
        telemetry.record_hit(2, Duration::from_millis(12), 25);
        telemetry.update_disk_usage(DiskCacheUsage {
            resident_bytes: 50,
            entries: 2,
        });
        let hud = telemetry.snapshot().format_hud();
        assert!(hud.contains("RATE 50%"));
        assert!(hud.contains("DISK BAR ==========---------- 50%"));
        assert!(hud.contains("CURRENT HIT 12.00 MS"));
    }

    #[test]
    fn stale_prefetch_generation_cannot_overwrite_new_plan() {
        let telemetry = CacheTelemetry::new(true, 100);
        telemetry.begin_prefetch(2, 7);
        telemetry.record_prefetch_stored(1, 50);
        telemetry.set_prefetch_phase(1, PrefetchPhase::Error);
        let snapshot = telemetry.snapshot();
        assert_eq!(snapshot.prefetch_completed, 0);
        assert_eq!(snapshot.prefetch_phase, PrefetchPhase::Queued);
    }

    #[test]
    fn stale_foreground_generation_does_not_change_hit_rate() {
        let telemetry = CacheTelemetry::new(true, 100);
        telemetry.begin_lookup(2);
        telemetry.record_hit(1, Duration::from_millis(1), 50);
        telemetry.record_miss(1);
        let snapshot = telemetry.snapshot();
        assert_eq!(snapshot.lookup_phase, LookupPhase::Reading);
        assert_eq!(snapshot.foreground_hits, 0);
        assert_eq!(snapshot.foreground_misses, 0);
    }

    #[test]
    fn writer_snapshot_tracks_pending_active_and_result() {
        let telemetry = CacheTelemetry::new(true, 100);
        telemetry.writer_queued(40);
        telemetry.writer_dequeued(40);
        assert_eq!(telemetry.snapshot().writer_active_bytes, 40);
        telemetry.writer_started();
        telemetry.writer_finished(40, Duration::from_millis(2), true);
        let snapshot = telemetry.snapshot();
        assert_eq!(snapshot.writer_phase, WriterPhase::Idle);
        assert_eq!(snapshot.writes_committed, 1);
        assert_eq!(snapshot.bytes_written, 40);
    }

    #[test]
    fn cache_panel_fits_the_telemetry_window_contract() {
        let telemetry = CacheTelemetry::new(true, 8 * 1024 * 1024 * 1024);
        telemetry.begin_lookup(1);
        telemetry.record_hit(1, Duration::from_millis(123), 120 * 1024 * 1024);
        telemetry.update_disk_usage(DiskCacheUsage {
            resident_bytes: 4 * 1024 * 1024 * 1024,
            entries: 1234,
        });
        telemetry.writer_queued(120 * 1024 * 1024);
        telemetry.begin_prefetch(3, 7);
        telemetry.record_prefetch_cached(3);
        telemetry.record_prefetch_stored(3, 120 * 1024 * 1024);
        let hud = telemetry.snapshot().format_hud();
        assert!(hud.lines().count() <= 13);
        assert!(
            hud.lines().all(|line| line.chars().count() <= 49),
            "cache HUD line exceeds the 780 px window: {hud}"
        );
    }
}
