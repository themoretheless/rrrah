//! One active and one replaceable histogram; never retain pixels after completion.
use crossbeam_channel::{Receiver, Sender, bounded};
use rrrah_core::{
    RasterPixels,
    histogram::{Histogram, MAX_SAMPLES},
};
use rrrah_decode::GenerationToken;
use std::sync::{
    Arc,
    atomic::{AtomicU64, Ordering},
};
struct Job {
    id: u64,
    pixels: RasterPixels,
    token: GenerationToken,
}
struct Done {
    id: u64,
    token: GenerationToken,
    histogram: Histogram,
}
pub(crate) struct HistogramWorker {
    revision: Arc<AtomicU64>,
    sender: Option<Sender<Job>>,
    pending: Option<Receiver<Job>>,
    completed: Option<Receiver<Done>>,
    thread: Option<std::thread::JoinHandle<()>>,
    wake: Arc<dyn Fn() + Send + Sync>,
}
impl Default for HistogramWorker {
    fn default() -> Self {
        Self {
            revision: Arc::new(AtomicU64::new(0)),
            sender: None,
            pending: None,
            completed: None,
            thread: None,
            wake: Arc::new(|| {}),
        }
    }
}
impl HistogramWorker {
    pub fn set_waker(&mut self, wake: impl Fn() + Send + Sync + 'static) {
        self.wake = Arc::new(wake);
    }
    pub fn submit(&mut self, pixels: RasterPixels, token: GenerationToken) -> std::io::Result<()> {
        if self.thread.is_none() {
            let (tx, rx) = bounded::<Job>(1);
            let (done, completed) = bounded::<Done>(1);
            let discard = completed.clone();
            let revision = self.revision.clone();
            let wake = self.wake.clone();
            let jobs = rx.clone();
            self.thread = Some(
                std::thread::Builder::new()
                    .name("histogram".into())
                    .spawn(move || {
                        while let Ok(job) = jobs.recv() {
                            let cancelled =
                                || revision.load(Ordering::Acquire) != job.id || job.token.is_cancelled();
                            let histogram = match &job.pixels {
                                RasterPixels::Rgba32Float(values) => {
                                    Histogram::linear_rgba(values, MAX_SAMPLES, 0., cancelled)
                                }
                                _ => continue, // Encoded samples must not be relabelled as linear RGB.
                            };
                            drop(job.pixels);
                            if let Ok(histogram) = histogram {
                                if revision.load(Ordering::Acquire) == job.id && !job.token.is_cancelled() {
                                    let _ = discard.try_recv();
                                    if done
                                        .try_send(Done {
                                            id: job.id,
                                            token: job.token,
                                            histogram,
                                        })
                                        .is_ok()
                                    {
                                        wake();
                                    }
                                }
                            }
                        }
                    })?,
            );
            self.sender = Some(tx);
            self.pending = Some(rx);
            self.completed = Some(completed);
        }
        let id = self.revision.fetch_add(1, Ordering::AcqRel) + 1;
        if let Some(pending) = &self.pending {
            let _ = pending.try_recv();
        }
        self.sender
            .as_ref()
            .unwrap()
            .try_send(Job { id, pixels, token })
            .map_err(std::io::Error::other)
    }
    pub fn poll(&self) -> Option<Histogram> {
        let done = self.completed.as_ref()?.try_recv().ok()?;
        (done.id == self.revision.load(Ordering::Acquire) && !done.token.is_cancelled())
            .then_some(done.histogram)
    }
    pub fn cancel(&self) {
        self.revision.fetch_add(1, Ordering::AcqRel);
        if let Some(pending) = &self.pending {
            let _ = pending.try_recv();
        }
    }
}
impl Drop for HistogramWorker {
    fn drop(&mut self) {
        self.cancel();
        self.sender.take();
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn stale_generation_is_refused_and_pixels_release_before_wake() {
        let budget = rrrah_core::MemoryBudget::new(1024);
        let generation = Arc::new(AtomicU64::new(1));
        let mut worker = HistogramWorker::default();
        let (wake, ready) = bounded(1);
        worker.set_waker(move || {
            let _ = wake.try_send(());
        });
        let pixels = RasterPixels::Rgba32Float(budget.try_buffer(16, 1f32).unwrap().freeze().into());
        worker
            .submit(pixels, GenerationToken::new(generation.clone(), 1))
            .unwrap();
        ready.recv_timeout(std::time::Duration::from_secs(5)).unwrap();
        assert_eq!(budget.used(), 0);
        assert_eq!(worker.poll().unwrap().sampled, 4);
        generation.store(2, Ordering::Release);
        let old = RasterPixels::Rgba32Float(budget.try_buffer(16, 1f32).unwrap().freeze().into());
        worker
            .submit(old, GenerationToken::new(generation.clone(), 1))
            .unwrap();
        let current = RasterPixels::Rgba32Float(budget.try_buffer(32, 1f32).unwrap().freeze().into());
        worker
            .submit(current, GenerationToken::new(generation, 2))
            .unwrap();
        ready.recv_timeout(std::time::Duration::from_secs(5)).unwrap();
        assert_eq!(worker.poll().unwrap().sampled, 8);
        assert_eq!(budget.used(), 0);
    }
}
