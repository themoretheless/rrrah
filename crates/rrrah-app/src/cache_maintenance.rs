//! One monotonic deadline shared by idle waits and busy neighbour processing.
use std::time::{Duration, Instant};

pub(crate) struct CacheMaintenance {
    interval: Option<Duration>,
    deadline: Option<Instant>,
}

impl CacheMaintenance {
    pub(crate) fn new(interval: Option<Duration>, now: Instant) -> Self {
        Self {
            interval,
            deadline: interval.and_then(|period| now.checked_add(period)),
        }
    }

    pub(crate) fn due(&mut self, now: Instant) -> bool {
        if self.deadline.is_some_and(|deadline| now >= deadline) {
            self.completed(now);
            true
        } else {
            false
        }
    }

    pub(crate) fn completed(&mut self, now: Instant) {
        self.deadline = self.interval.and_then(|period| now.checked_add(period));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn continuous_neighbours_do_not_postpone_expiry_and_pins_survive() {
        use rrrah_cache::{CacheLimits, RasterRamCache};
        use rrrah_core::{DecodedRaster, MemoryBudget, RasterColorSpace, RasterPixels};
        let budget = MemoryBudget::new(8);
        let frame = || {
            DecodedRaster::new(
                1,
                1,
                RasterPixels::Rgba8(std::sync::Arc::new(vec![0, 0, 0, 255]).into()),
                RasterColorSpace::Srgb,
            )
            .unwrap()
            .try_manage_pixels(&budget)
            .unwrap()
        };
        let mut cache = RasterRamCache::new(CacheLimits {
            max_bytes: 8,
            max_entries: Some(2),
            ttl: Some(Duration::ZERO),
        });
        assert!(cache.insert_visible(0u8, frame()));
        assert!(cache.insert(1, frame()));
        assert_eq!(budget.used(), 8);
        let started = Instant::now();
        let mut maintenance = CacheMaintenance::new(Some(Duration::from_secs(1)), started);
        let mut runs = 0;
        // Simulated uninterrupted neighbour activity: no idle receive or cache lookup.
        for milliseconds in 0..=3100 {
            if maintenance.due(started + Duration::from_millis(milliseconds)) {
                runs += 1;
                cache.spill_expired();
            }
        }
        assert_eq!(runs, 3);
        assert_eq!(cache.len(), 1);
        assert_eq!(budget.used(), 4);
        // TTL forbids a fresh lookup; the owner's visible pin still retains
        // the existing allocation until the viewer releases it.
        assert!(!cache.set_limits_and_spill(CacheLimits::bytes(0)));
        drop(cache);
        assert_eq!(budget.used(), 0);
    }

    #[test]
    fn disabled_and_idle_completion_do_not_create_extra_sweeps() {
        let now = Instant::now();
        assert!(!CacheMaintenance::new(None, now).due(now + Duration::from_secs(3600)));
        let mut timer = CacheMaintenance::new(Some(Duration::from_secs(1)), now);
        timer.completed(now + Duration::from_secs(1));
        assert!(!timer.due(now + Duration::from_millis(1999)));
        assert!(timer.due(now + Duration::from_secs(2)));
        assert!(!timer.due(now + Duration::from_secs(2)));
    }
}
