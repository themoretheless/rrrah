//! Inspect drag-and-drop paths without filesystem access on the UI thread.
use crossbeam_channel::{Receiver, Sender, TryRecvError, bounded};
use std::{
    path::{Path, PathBuf},
    sync::{
        Arc, Mutex,
        atomic::{AtomicU64, Ordering},
    },
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DropKind {
    File,
    Folder,
}

pub struct DropReady {
    pub generation: u64,
    pub path: PathBuf,
    pub kind: Result<DropKind, String>,
}

fn inspect(path: &Path) -> Result<DropKind, String> {
    let kind = std::fs::symlink_metadata(path)
        .map_err(|error| error.to_string())?
        .file_type();
    if kind.is_symlink() {
        return Err("symlinks are not opened".into());
    }
    if kind.is_dir() {
        return Ok(DropKind::Folder);
    }
    if kind.is_file() && crate::is_supported_image(path) {
        return Ok(DropKind::File);
    }
    Err("expected a supported image file or folder".into())
}

pub struct DropInspector {
    tx: Sender<(u64, PathBuf)>,
    pending: Receiver<(u64, PathBuf)>,
    ready: Receiver<DropReady>,
    generation: Arc<AtomicU64>,
    publication: Arc<Mutex<()>>,
}

impl DropInspector {
    pub fn new() -> Self {
        Self::with_inspector(inspect)
    }

    pub(super) fn with_inspector(
        inspect: impl Fn(&Path) -> Result<DropKind, String> + Send + 'static,
    ) -> Self {
        let (tx, jobs) = bounded::<(u64, PathBuf)>(1);
        let pending = jobs.clone();
        let (send, ready) = bounded(1);
        let generation = Arc::new(AtomicU64::new(0));
        let current = generation.clone();
        let publication = Arc::new(Mutex::new(()));
        let publish = publication.clone();
        std::thread::Builder::new()
            .name("rrrah-drop-inspect".into())
            .spawn(move || {
                while let Ok((generation, path)) = jobs.recv() {
                    if current.load(Ordering::Acquire) != generation {
                        continue;
                    }
                    let kind = inspect(&path);
                    let _guard = publish.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
                    if current.load(Ordering::Acquire) == generation {
                        let _ = send.try_send(DropReady {
                            generation,
                            path,
                            kind,
                        });
                    }
                }
            })
            .expect("spawn dropped-path inspector");
        Self {
            tx,
            pending,
            ready,
            generation,
            publication,
        }
    }

    fn cancel_locked(&self) -> u64 {
        let generation = self.generation.fetch_add(1, Ordering::AcqRel).wrapping_add(1);
        while self.pending.try_recv().is_ok() {}
        while self.ready.try_recv().is_ok() {}
        generation
    }

    pub fn cancel(&self) {
        let _guard = self
            .publication
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        self.cancel_locked();
    }

    pub fn submit(&self, path: PathBuf) -> u64 {
        let _guard = self
            .publication
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let generation = self.cancel_locked();
        let _ = self.tx.try_send((generation, path));
        generation
    }

    pub fn try_recv(&self) -> Result<Option<DropReady>, TryRecvError> {
        let _guard = self
            .publication
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        match self.ready.try_recv() {
            Ok(ready) => Ok(Some(ready)),
            Err(TryRecvError::Empty) => Ok(None),
            Err(error) => Err(error),
        }
    }
}

impl Drop for DropInspector {
    fn drop(&mut self) {
        self.cancel();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn real_paths_preserve_file_folder_and_refusal_policy() {
        let root = tempfile::tempdir().unwrap();
        let image = root.path().join("image.png");
        let model = root.path().join("model.obj");
        let unsupported = root.path().join("notes.txt");
        for path in [&image, &model, &unsupported] {
            std::fs::write(path, b"fixture").unwrap();
        }
        let inspector = DropInspector::new();
        for (path, expected) in [
            (image, Some(DropKind::File)),
            (model, Some(DropKind::File)),
            (root.path().to_path_buf(), Some(DropKind::Folder)),
            (unsupported, None),
            (root.path().join("missing.png"), None),
        ] {
            let generation = inspector.submit(path.clone());
            let result = inspector.ready.recv_timeout(Duration::from_secs(5)).unwrap();
            assert_eq!(result.generation, generation);
            assert_eq!(result.path, path);
            assert_eq!(result.kind.ok(), expected);
        }
    }

    #[cfg(unix)]
    #[test]
    fn symlinks_broken_links_and_special_files_are_rejected() {
        let root = tempfile::tempdir().unwrap();
        let file = root.path().join("image.png");
        std::fs::write(&file, b"fixture").unwrap();
        for (name, target) in [
            ("file.png", file),
            ("directory", root.path().to_path_buf()),
            ("broken.png", root.path().join("missing.png")),
        ] {
            let link = root.path().join(name);
            std::os::unix::fs::symlink(target, &link).unwrap();
            assert_eq!(inspect(&link).unwrap_err(), "symlinks are not opened");
        }
        let socket = root.path().join("socket.png");
        let _listener = std::os::unix::net::UnixListener::bind(&socket).unwrap();
        assert!(inspect(&socket).is_err());
    }

    #[test]
    fn blocked_inspection_does_not_block_latest_drop_or_publish_old_result() {
        let (started, observed) = bounded(1);
        let (release, gate) = bounded(1);
        let inspector = DropInspector::with_inspector(move |path| {
            if path == Path::new("blocked") {
                started.send(()).unwrap();
                gate.recv_timeout(Duration::from_secs(5)).unwrap();
            }
            Ok(DropKind::File)
        });
        inspector.submit("blocked".into());
        observed.recv_timeout(Duration::from_secs(5)).unwrap();
        assert!(inspector.try_recv().unwrap().is_none());
        for _ in 0..100 {
            inspector.submit("superseded".into());
        }
        let generation = inspector.submit("latest".into());
        assert!(inspector.pending.len() <= 1);
        release.send(()).unwrap();
        let ready = inspector.ready.recv_timeout(Duration::from_secs(5)).unwrap();
        assert_eq!(ready.generation, generation);
        assert_eq!(ready.path, PathBuf::from("latest"));
        assert!(inspector.try_recv().unwrap().is_none());
    }

    #[test]
    fn cancel_clears_published_result_and_drop_stops_idle_worker() {
        let inspector = DropInspector::with_inspector(|_| Ok(DropKind::Folder));
        inspector.submit("folder".into());
        let deadline = std::time::Instant::now() + Duration::from_secs(5);
        while inspector.ready.is_empty() {
            assert!(std::time::Instant::now() < deadline);
            std::thread::yield_now();
        }
        inspector.cancel();
        assert!(inspector.try_recv().unwrap().is_none());
        let ready = inspector.ready.clone();
        drop(inspector);
        assert!(matches!(
            ready.recv_timeout(Duration::from_secs(5)),
            Err(crossbeam_channel::RecvTimeoutError::Disconnected)
        ));
    }
}
