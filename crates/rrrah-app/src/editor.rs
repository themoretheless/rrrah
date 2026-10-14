//! UI history and a bounded background persistence actor. No filesystem work on key events.
use crate::edit_store::{self, SourceVersion};
use crossbeam_channel::{Receiver, Sender, bounded};
use rrrah_core::{
    MemoryBudget,
    edit::{EditSession, EditState},
};
use std::{collections::BTreeMap, path::PathBuf, sync::Arc};

type Key = (PathBuf, usize);
#[derive(Clone)]
struct Context {
    source: SourceVersion,
    sidecar: Option<[u8; 32]>,
}
#[derive(Clone, Copy, PartialEq, Eq)]
enum IoState {
    Opening,
    Idle,
    Saving,
    Failed,
}
struct Entry {
    history: EditSession,
    revision: u64,
    used: u64,
    context: Option<Context>,
    io: IoState,
    dirty: bool,
    save_sidecar: bool,
}
enum Job {
    Open(Key),
    Save {
        key: Key,
        context: Context,
        state: EditState,
        revision: u64,
        sidecar: bool,
    },
    Stop,
}
enum Outcome {
    Open(Context, Option<EditState>),
    Saved {
        revision: u64,
        sidecar: Option<[u8; 32]>,
        saved_sidecar: bool,
    },
}
struct Reply {
    key: Key,
    result: anyhow::Result<Outcome>,
}
pub(crate) struct Editor {
    entries: BTreeMap<Key, Entry>,
    current: Option<Key>,
    budget: MemoryBudget,
    clock: u64,
    root: PathBuf,
    jobs: Option<Sender<Job>>,
    replies: Option<Receiver<Reply>>,
    thread: Option<std::thread::JoinHandle<()>>,
    wake: Arc<dyn Fn() + Send + Sync>,
}
impl Editor {
    pub fn new(budget: MemoryBudget, root: PathBuf) -> Self {
        Self {
            entries: BTreeMap::new(),
            current: None,
            budget,
            clock: 0,
            root,
            jobs: None,
            replies: None,
            thread: None,
            wake: Arc::new(|| {}),
        }
    }
    pub fn set_waker(&mut self, wake: impl Fn() + Send + Sync + 'static) {
        self.wake = Arc::new(wake);
    }
    fn start(&mut self) -> anyhow::Result<()> {
        if self.thread.is_some() {
            return Ok(());
        }
        let (tx, rx) = bounded(16);
        let (done, replies) = bounded(32);
        let root = self.root.clone();
        let wake = self.wake.clone();
        self.thread = Some(
            std::thread::Builder::new()
                .name("edit-store".into())
                .spawn(move || {
                    while let Ok(job) = rx.recv() {
                        let (key, result) = match job {
                            Job::Stop => break,
                            Job::Open(key) => {
                                let result = (|| {
                                    let source = edit_store::source_version(&key.0)?;
                                    let (sidecar_state, sidecar) =
                                        edit_store::read_sidecar(&key.0.with_extension("xmp"), key.1)?;
                                    let journal = edit_store::journal_path(&root, &key.0, key.1);
                                    let recovery = edit_store::read_recovery(&journal, &source, key.1)?;
                                    if let (Some(recovered), Some(saved)) = (recovery, sidecar_state) {
                                        if recovered != saved {
                                            anyhow::bail!(
                                                "recovery differs from XMP; both retained for explicit resolution"
                                            );
                                        }
                                    }
                                    let state = recovery.or(sidecar_state);
                                    if state.is_some_and(|s| s.exposure_stops.abs() > 10.) {
                                        anyhow::bail!(
                                            "exposure record retained but outside this viewer's -10..10 range"
                                        );
                                    }
                                    if state.is_some_and(|s| s.points() != EditState::default().points()) {
                                        anyhow::bail!(
                                            "curve edit retained but not supported by this viewer yet"
                                        );
                                    }
                                    Ok(Outcome::Open(Context { source, sidecar }, state))
                                })();
                                (key, result)
                            }
                            Job::Save {
                                key,
                                context,
                                state,
                                revision,
                                sidecar,
                            } => {
                                let result = (|| {
                                    edit_store::verify_source(&key.0, &context.source)?;
                                    let journal = edit_store::journal_path(&root, &key.0, key.1);
                                    edit_store::write_recovery(&journal, context.source, key.1, state)?;
                                    let digest = if sidecar {
                                        Some(edit_store::write_sidecar(
                                            &key.0.with_extension("xmp"),
                                            key.1,
                                            state,
                                            context.sidecar,
                                        )?)
                                    } else {
                                        context.sidecar
                                    };
                                    Ok(Outcome::Saved {
                                        revision,
                                        sidecar: digest,
                                        saved_sidecar: sidecar,
                                    })
                                })();
                                (key, result)
                            }
                        };
                        if done.send(Reply { key, result }).is_err() {
                            break;
                        }
                        wake();
                    }
                })?,
        );
        self.jobs = Some(tx);
        self.replies = Some(replies);
        Ok(())
    }
    pub fn activate(&mut self, path: PathBuf, page: usize) -> anyhow::Result<bool> {
        let key = (path, page);
        if self.current.as_ref() == Some(&key) {
            return Ok(false);
        }
        self.current = None;
        if !self.entries.contains_key(&key) {
            if self.entries.len() == 16 {
                let victim = self
                    .entries
                    .iter()
                    .filter(|(_, e)| !e.dirty && matches!(e.io, IoState::Idle | IoState::Failed))
                    .min_by_key(|(_, e)| e.used)
                    .map(|(k, _)| k.clone());
                if let Some(victim) = victim {
                    self.entries.remove(&victim);
                } else {
                    anyhow::bail!("edit history is busy; pending changes retained");
                }
            }
            let history = EditSession::new(EditState::default(), 64, &self.budget)?;
            self.start()?;
            self.jobs.as_ref().unwrap().try_send(Job::Open(key.clone()))?;
            self.entries.insert(
                key.clone(),
                Entry {
                    history,
                    revision: 0,
                    used: 0,
                    context: None,
                    io: IoState::Opening,
                    dirty: false,
                    save_sidecar: false,
                },
            );
        }
        self.clock = self.clock.wrapping_add(1);
        self.entries.get_mut(&key).unwrap().used = self.clock;
        self.current = Some(key);
        Ok(true)
    }
    pub fn current(&self) -> EditState {
        self.current
            .as_ref()
            .and_then(|k| self.entries.get(k))
            .map_or_else(EditState::default, |e| e.history.current())
    }
    pub fn is_current(&self, path: &std::path::Path, page: usize) -> bool {
        self.current
            .as_ref()
            .is_some_and(|key| key.0 == path && key.1 == page)
    }
    pub fn is_ready(&self) -> bool {
        self.current
            .as_ref()
            .and_then(|key| self.entries.get(key))
            .is_some_and(|entry| entry.context.is_some() && entry.io != IoState::Opening)
    }
    pub fn original(&self) -> EditState {
        self.current
            .as_ref()
            .and_then(|k| self.entries.get(k))
            .map_or_else(EditState::default, |e| e.history.original())
    }
    pub fn apply_exposure(&mut self, exposure: f32) -> anyhow::Result<()> {
        let Some(entry) = self.current.as_ref().and_then(|k| self.entries.get_mut(k)) else {
            anyhow::bail!("no edit session");
        };
        let mut state = entry.history.current();
        state.exposure_stops = exposure;
        if entry.history.apply(state)? {
            entry.revision += 1;
            entry.dirty = true;
            if entry.io == IoState::Failed {
                entry.io = IoState::Idle;
            }
        }
        self.flush();
        Ok(())
    }
    pub fn undo_redo(&mut self, redo: bool) -> bool {
        let Some(entry) = self.current.as_ref().and_then(|k| self.entries.get_mut(k)) else {
            return false;
        };
        let changed = if redo {
            entry.history.redo()
        } else {
            entry.history.undo()
        };
        if changed {
            entry.revision += 1;
            entry.dirty = true;
            if entry.io == IoState::Failed {
                entry.io = IoState::Idle;
            }
        }
        self.flush();
        changed
    }
    pub fn request_save(&mut self) -> bool {
        let Some(entry) = self.current.as_ref().and_then(|k| self.entries.get_mut(k)) else {
            return false;
        };
        if entry.io != IoState::Opening && entry.context.is_none() {
            return false;
        }
        entry.save_sidecar = true;
        if entry.io == IoState::Failed {
            entry.io = IoState::Idle;
        }
        self.flush();
        true
    }
    fn flush(&mut self) {
        let Some(jobs) = &self.jobs else {
            return;
        };
        for (key, entry) in &mut self.entries {
            if entry.io != IoState::Idle || !(entry.dirty || entry.save_sidecar) {
                continue;
            }
            let Some(context) = entry.context.clone() else {
                continue;
            };
            let job = Job::Save {
                key: key.clone(),
                context,
                state: entry.history.current(),
                revision: entry.revision,
                sidecar: entry.save_sidecar,
            };
            if jobs.try_send(job).is_ok() {
                entry.io = IoState::Saving;
                entry.save_sidecar = false;
            }
        }
    }
    fn accept(&mut self, reply: Reply) -> (bool, Option<String>) {
        let current = self.current.as_ref() == Some(&reply.key);
        let Some(entry) = self.entries.get_mut(&reply.key) else {
            return (false, None);
        };
        entry.io = IoState::Idle;
        match reply.result {
            Ok(Outcome::Open(context, state)) => {
                entry.context = Some(context);
                if entry.revision == 0 {
                    if let Some(state) = state {
                        if let Err(error) = entry.history.apply(state) {
                            return (false, Some(error.to_string()));
                        }
                        return (current, Some("edits recovered".into()));
                    }
                }
                (false, None)
            }
            Ok(Outcome::Saved {
                revision,
                sidecar,
                saved_sidecar,
            }) => {
                if let Some(context) = &mut entry.context {
                    context.sidecar = sidecar;
                }
                if revision == entry.revision {
                    entry.dirty = false;
                }
                (
                    false,
                    (current && saved_sidecar).then(|| "XMP edits saved".into()),
                )
            }
            Err(error) => {
                entry.io = IoState::Failed;
                (false, Some(format!("edits not saved/recovered: {error:#}")))
            }
        }
    }
    pub fn poll(&mut self) -> (bool, Option<String>) {
        let mut changed = false;
        let mut message = None;
        while let Some(reply) = self.replies.as_ref().and_then(|r| r.try_recv().ok()) {
            let (update, info) = self.accept(reply);
            changed |= update;
            if info.is_some() {
                message = info;
            }
        }
        self.flush();
        (changed, message)
    }
    pub fn shutdown(&mut self) {
        loop {
            self.poll();
            self.flush();
            if !self
                .entries
                .values()
                .any(|e| matches!(e.io, IoState::Opening | IoState::Saving))
            {
                break;
            }
            let Some(reply) = self.replies.as_ref().and_then(|r| r.recv().ok()) else {
                break;
            };
            let (_, message) = self.accept(reply);
            if let Some(message) = message {
                log::warn!("{message}");
            }
        }
        if let Some(jobs) = self.jobs.take() {
            let _ = jobs.send(Job::Stop);
        }
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}
impl Drop for Editor {
    fn drop(&mut self) {
        self.shutdown();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn conflicting_recovery_and_sidecar_are_retained_without_implicit_overwrite() {
        let dir = tempfile::tempdir().unwrap();
        let source = dir.path().join("image.raw");
        std::fs::write(&source, b"synthetic source").unwrap();
        let root = dir.path().join("journals");
        let mut saved = EditState::default();
        saved.exposure_stops = 0.5;
        edit_store::write_sidecar(&source.with_extension("xmp"), 0, saved, None).unwrap();
        let mut recovery = saved;
        recovery.exposure_stops = 1.5;
        let journal = edit_store::journal_path(&root, &source, 0);
        edit_store::write_recovery(
            &journal,
            edit_store::source_version(&source).unwrap(),
            0,
            recovery,
        )
        .unwrap();
        let sidecar_bytes = std::fs::read(source.with_extension("xmp")).unwrap();
        let journal_bytes = std::fs::read(&journal).unwrap();
        let mut editor = Editor::new(MemoryBudget::new(1024 * 1024), root);
        editor.activate(source.clone(), 0).unwrap();
        editor.shutdown();
        assert!(!editor.is_ready());
        assert!(!editor.request_save());
        assert_eq!(
            std::fs::read(source.with_extension("xmp")).unwrap(),
            sidecar_bytes
        );
        assert_eq!(std::fs::read(&journal).unwrap(), journal_bytes);
    }
    #[test]
    fn documents_keep_separate_history_and_latest_edit_recovers_after_restart() {
        let dir = tempfile::tempdir().unwrap();
        let source = dir.path().join("image.raw");
        std::fs::write(&source, b"synthetic source").unwrap();
        let root = dir.path().join("journals");
        let budget = MemoryBudget::new(1024 * 1024);
        {
            let mut editor = Editor::new(budget.clone(), root.clone());
            editor.activate(source.clone(), 0).unwrap();
            assert!(!editor.is_ready());
            editor.apply_exposure(1.5).unwrap(); // Before the asynchronous open completes.
            editor.activate(source.clone(), 1).unwrap();
            editor.apply_exposure(-0.5).unwrap();
            editor.activate(source.clone(), 0).unwrap();
            assert_eq!(editor.current().exposure_stops, 1.5);
            assert!(editor.undo_redo(false));
            assert_eq!(editor.current().exposure_stops, 0.);
            assert!(editor.undo_redo(true));
            editor.request_save();
            editor.shutdown();
            assert!(editor.is_ready());
            assert_eq!(
                edit_store::read_sidecar(&source.with_extension("xmp"), 0)
                    .unwrap()
                    .0
                    .unwrap()
                    .exposure_stops,
                1.5
            );
        }
        assert_eq!(budget.used(), 0);
        let mut restored = Editor::new(budget.clone(), root);
        restored.activate(source.clone(), 1).unwrap();
        restored.shutdown();
        assert_eq!(restored.current().exposure_stops, -0.5);
    }
}
