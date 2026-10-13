//! Bounded background source checks for deferred UI work.
use crate::gallery::SourceStamp;
use crossbeam_channel::{Receiver, Sender, TryRecvError, bounded};
use std::{
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};

struct Job {
    path: PathBuf,
    expected: SourceStamp,
    cancelled: Arc<AtomicBool>,
    reply: Sender<bool>,
}

pub struct Check {
    cancelled: Arc<AtomicBool>,
    reply: Receiver<bool>,
}
impl Check {
    #[cfg(test)]
    pub(super) fn has_result(&self) -> bool {
        !self.reply.is_empty()
    }
    /// None means pending; disconnects fail closed.
    pub fn poll(&self) -> Option<bool> {
        match self.reply.try_recv() {
            Ok(value) => Some(value),
            Err(TryRecvError::Empty) => None,
            Err(TryRecvError::Disconnected) => Some(false),
        }
    }
}
impl Drop for Check {
    fn drop(&mut self) {
        self.cancelled.store(true, Ordering::Release);
    }
}

pub struct SourceValidator {
    jobs: Sender<Job>,
    pending: Receiver<Job>,
}
impl SourceValidator {
    pub fn new() -> Self {
        Self::with_reader(SourceStamp::read)
    }

    pub(super) fn with_reader(reader: impl Fn(&std::path::Path) -> SourceStamp + Send + 'static) -> Self {
        let (jobs, pending) = bounded::<Job>(1);
        let worker = pending.clone();
        std::thread::Builder::new()
            .name("rrrah-source-validation".into())
            .spawn(move || {
                while let Ok(job) = worker.recv() {
                    if job.cancelled.load(Ordering::Acquire) {
                        continue;
                    }
                    let actual = reader(&job.path);
                    let valid = actual.is_readable() && actual == job.expected;
                    if !job.cancelled.load(Ordering::Acquire) {
                        let _ = job.reply.try_send(valid);
                    }
                }
            })
            .expect("spawn source validator");
        Self { jobs, pending }
    }

    /// Keep at most one active and one queued source check. Replacing a queued
    /// check disconnects its reply, so its owner cannot accept stale evidence.
    pub fn submit(&self, path: PathBuf, expected: SourceStamp) -> Check {
        while self.pending.try_recv().is_ok() {}
        let (reply, receive) = bounded(1);
        let cancelled = Arc::new(AtomicBool::new(false));
        let _ = self.jobs.try_send(Job {
            path,
            expected,
            reply,
            cancelled: cancelled.clone(),
        });
        Check {
            cancelled,
            reply: receive,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    fn wait(check: &Check) -> bool {
        check.reply.recv_timeout(Duration::from_secs(5)).unwrap()
    }

    #[test]
    fn changed_and_missing_sources_fail_closed() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("cover.png");
        std::fs::write(&path, [1u8; 32]).unwrap();
        let stamp = SourceStamp::read(&path);
        let validator = SourceValidator::new();
        assert!(wait(&validator.submit(path.clone(), stamp.clone())));
        std::fs::write(&path, [2u8; 32]).unwrap();
        assert!(!wait(&validator.submit(path.clone(), stamp.clone())));
        std::fs::remove_file(&path).unwrap();
        assert!(!wait(&validator.submit(path, stamp)));
    }

    #[test]
    fn blocked_reader_does_not_block_submit_and_queue_stays_bounded() {
        let (started, observed) = bounded(8);
        let (release, gate) = bounded(1);
        let validator = SourceValidator::with_reader(move |path| {
            started.send(path.to_path_buf()).unwrap();
            if path == std::path::Path::new("active") {
                gate.recv_timeout(Duration::from_secs(5)).unwrap();
            }
            SourceStamp::default()
        });
        let active = validator.submit("active".into(), SourceStamp::default());
        assert_eq!(
            observed.recv_timeout(Duration::from_secs(5)).unwrap(),
            PathBuf::from("active")
        );
        assert_eq!(active.poll(), None);
        let replaced = validator.submit("replaced".into(), SourceStamp::default());
        let cancelled = validator.submit("cancelled".into(), SourceStamp::default());
        assert_eq!(replaced.poll(), Some(false));
        drop(cancelled);
        let latest = validator.submit("latest".into(), SourceStamp::default());
        drop(active);
        release.send(()).unwrap();
        assert!(!wait(&latest));
        assert_eq!(
            observed.recv_timeout(Duration::from_secs(5)).unwrap(),
            PathBuf::from("latest")
        );
        assert!(observed.try_recv().is_err());
    }

    #[test]
    fn expired_thumbnail_releases_pixels_while_source_reader_is_blocked() {
        use std::time::Instant;
        let (started, observed) = bounded(1);
        let (release, gate) = bounded(1);
        let validator = SourceValidator::with_reader(move |_| {
            started.send(()).unwrap();
            gate.recv_timeout(Duration::from_secs(5)).unwrap();
            SourceStamp::default()
        });
        let budget = rrrah_core::MemoryBudget::new(4);
        let now = Instant::now();
        let mut deferred = crate::DeferredThumbnail {
            ready: crate::gallery::ThumbnailReady {
                source_stamp: SourceStamp::default(),
                index: 0,
                width: 1,
                height: 1,
                pixels: budget.try_buffer(4, 255u8).unwrap().freeze().into(),
            },
            folder: "folder".into(),
            cover: "folder/cover.png".into(),
            retry_at: now,
            expires_at: now + Duration::from_secs(2),
            validation: None,
        };
        assert_eq!(deferred.poll_source(&validator), None);
        observed.recv_timeout(Duration::from_secs(5)).unwrap();
        assert_eq!(deferred.poll_source(&validator), None);
        assert_eq!(budget.used(), 4);
        assert!(deferred.expired(now + Duration::from_secs(2)));
        drop(deferred);
        assert_eq!(budget.used(), 0, "validation retains no decoded pixels");
        release.send(()).unwrap();
    }
}
