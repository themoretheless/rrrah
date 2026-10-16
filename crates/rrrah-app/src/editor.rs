//! UI history and a bounded background persistence actor. No filesystem work on key events.
use crate::edit_store::{self, SourceVersion};
use anyhow::Context as _;
use crossbeam_channel::{Receiver, Sender, bounded};
use rrrah_core::{
    MemoryBudget,
    edit::{EditSession, EditState},
};
use std::{collections::BTreeMap, path::PathBuf, sync::Arc};

// A new decode must not display a previous file version's history while the
// persistence actor establishes the full content identity asynchronously.
type Key = (PathBuf, usize, u64);
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
    conflict: Option<(EditState, EditState)>,
    refused_recovery: Option<[u8; 32]>,
}
enum Job {
    Open(Key),
    Restart {
        key: Key,
        context: Context,
        expected: [u8; 32],
    },
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
    Conflict(Context, EditState, EditState),
    RefusedRecovery(Context, [u8; 32], String),
    Restarted(Context, PathBuf),
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
fn open_document(root: &std::path::Path, key: &Key) -> anyhow::Result<Outcome> {
    let source = edit_store::source_version(&key.0)?;
    let (sidecar_state, sidecar) = edit_store::read_sidecar(&key.0.with_extension("xmp"), key.1)?;
    let journal = edit_store::journal_path(root, &key.0, key.1);
    let recovery = match edit_store::read_recovery(&journal, &source, key.1) {
        Ok(state) => state,
        Err(error) => {
            let digest = edit_store::recovery_digest(&journal)?
                .context("refused recovery disappeared; reload before choosing")?;
            return Ok(Outcome::RefusedRecovery(
                Context { source, sidecar },
                digest,
                error.to_string(),
            ));
        }
    };
    if [recovery, sidecar_state]
        .into_iter()
        .flatten()
        .any(|s| s.exposure_stops.abs() > 10.)
    {
        anyhow::bail!("exposure record retained but outside this viewer's -10..10 range");
    }
    if [recovery, sidecar_state]
        .into_iter()
        .flatten()
        .any(|s| s.points() != EditState::default().points())
    {
        anyhow::bail!("curve edit retained but not supported by this viewer yet");
    }
    let context = Context { source, sidecar };
    if let (Some(recovered), Some(saved)) = (recovery, sidecar_state) {
        if recovered != saved {
            return Ok(Outcome::Conflict(context, recovered, saved));
        }
    }
    Ok(Outcome::Open(context, recovery.or(sidecar_state)))
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
                                let result = open_document(&root, &key);
                                (key, result)
                            }
                            Job::Restart {
                                key,
                                context,
                                expected,
                            } => {
                                let result = edit_store::restart_recovery(
                                    &edit_store::journal_path(&root, &key.0, key.1),
                                    &key.0,
                                    &context.source,
                                    key.1,
                                    expected,
                                )
                                .map(|backup| Outcome::Restarted(context, backup));
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
    #[cfg(test)]
    pub fn activate(&mut self, path: PathBuf, page: usize) -> anyhow::Result<bool> {
        self.activate_version(path, page, 0)
    }
    pub fn activate_version(&mut self, path: PathBuf, page: usize, generation: u64) -> anyhow::Result<bool> {
        let key = (path, page, generation);
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
                    conflict: None,
                    refused_recovery: None,
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
    pub fn is_current_version(&self, path: &std::path::Path, page: usize, generation: u64) -> bool {
        self.current
            .as_ref()
            .is_some_and(|key| key.0 == path && key.1 == page && key.2 == generation)
    }
    pub fn is_ready(&self) -> bool {
        self.current
            .as_ref()
            .and_then(|key| self.entries.get(key))
            .is_some_and(|entry| {
                entry.context.is_some()
                    && entry.conflict.is_none()
                    && entry.refused_recovery.is_none()
                    && entry.io != IoState::Opening
            })
    }
    pub fn blocked_message(&self) -> &'static str {
        let entry = self.current.as_ref().and_then(|key| self.entries.get(key));
        if entry.is_some_and(|e| e.refused_recovery.is_some()) {
            "Recovery retained. Ctrl/Cmd+Shift+N starts clean after retaining a backup; XMP stays unchanged."
        } else if entry.is_some_and(|e| e.conflict.is_some()) {
            "Recovery differs from XMP. Ctrl/Cmd+Shift+R uses recovery; Ctrl/Cmd+Shift+X uses XMP."
        } else {
            "Wait for edit recovery or resolve the source/recovery error before editing."
        }
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
        if entry.conflict.is_some() || entry.refused_recovery.is_some() {
            anyhow::bail!("resolve retained recovery or XMP conflict before editing");
        }
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
        if entry.conflict.is_some() || entry.refused_recovery.is_some() {
            return false;
        }
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
        if entry.conflict.is_some() || entry.refused_recovery.is_some() {
            return false;
        }
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
    /// An explicit choice replaces only the recovery snapshot; XMP still
    /// requires the normal Save action and its external-change check.
    pub fn resolve_conflict(&mut self, use_recovery: bool) -> anyhow::Result<bool> {
        let Some(entry) = self.current.as_ref().and_then(|key| self.entries.get_mut(key)) else {
            return Ok(false);
        };
        let Some((recovery, saved)) = entry.conflict else {
            return Ok(false);
        };
        entry.history.apply(if use_recovery { recovery } else { saved })?;
        entry.conflict = None;
        entry.revision += 1;
        entry.dirty = true;
        entry.io = IoState::Idle;
        self.flush();
        Ok(true)
    }
    pub fn request_restart_recovery(&mut self) -> anyhow::Result<bool> {
        let request = self.current.as_ref().and_then(|key| {
            let entry = self.entries.get(key)?;
            if matches!(entry.io, IoState::Opening | IoState::Saving) {
                return None;
            }
            Some((key.clone(), entry.context.clone()?, entry.refused_recovery?))
        });
        let Some((key, context, expected)) = request else {
            return Ok(false);
        };
        self.start()?;
        self.jobs.as_ref().unwrap().try_send(Job::Restart {
            key: key.clone(),
            context,
            expected,
        })?;
        self.entries.get_mut(&key).unwrap().io = IoState::Saving;
        Ok(true)
    }
    fn flush(&mut self) {
        let Some(jobs) = &self.jobs else {
            return;
        };
        for (key, entry) in &mut self.entries {
            if entry.io != IoState::Idle
                || entry.conflict.is_some()
                || entry.refused_recovery.is_some()
                || !(entry.dirty || entry.save_sidecar)
            {
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
    fn reuse_history(&mut self, incoming: &Key, context: &Context, state: Option<&EditState>) -> bool {
        let reusable = self.entries.iter().find_map(|(key, entry)| {
            (key != incoming
                && key.0 == incoming.0
                && key.1 == incoming.1
                && !entry.dirty
                && entry.io == IoState::Idle
                && entry.history.current() == state.copied().unwrap_or_default()
                && entry
                    .context
                    .as_ref()
                    .is_some_and(|old| old.source == context.source && old.sidecar == context.sidecar))
            .then(|| key.clone())
        });
        if let Some(old_key) = reusable {
            if self
                .entries
                .get(incoming)
                .is_some_and(|entry| entry.revision == 0)
            {
                let mut old = self.entries.remove(&old_key).unwrap();
                let entry = self.entries.get_mut(incoming).unwrap();
                std::mem::swap(&mut entry.history, &mut old.history);
                entry.revision = old.revision;
                entry.context = Some(context.clone());
                entry.io = IoState::Idle;
                return true;
            }
        }
        false
    }
    fn accept(&mut self, reply: Reply) -> (bool, Option<String>) {
        let current = self.current.as_ref() == Some(&reply.key);
        // Reuse undo/redo only once a full content digest has matched. Replies
        // carry the decode generation, so an older open cannot initialize the
        // newly selected document. Busy/dirty histories are never discarded.
        if let Ok(Outcome::Open(context, state)) = &reply.result {
            if self.reuse_history(&reply.key, context, state.as_ref()) {
                return (current, None);
            }
        }
        let Some(entry) = self.entries.get_mut(&reply.key) else {
            return (false, None);
        };
        entry.io = IoState::Idle;
        match reply.result {
            Ok(Outcome::RefusedRecovery(context, digest, error)) => {
                entry.context = Some(context);
                entry.refused_recovery = Some(digest);
                (false, current.then(|| format!("Recovery retained: {error}. Ctrl/Cmd+Shift+N starts clean with a retained backup; XMP stays unchanged.")))
            }
            Ok(Outcome::Restarted(context, backup)) => {
                if let Err(error) = entry.history.reset(EditState::default()) {
                    entry.io = IoState::Failed;
                    return (
                        false,
                        current.then(|| {
                            format!("clean recovery written but history unavailable: {error}; reload")
                        }),
                    );
                }
                entry.context = Some(context);
                entry.refused_recovery = None;
                entry.conflict = None;
                entry.revision = 0;
                entry.dirty = false;
                entry.save_sidecar = false;
                (
                    current,
                    current.then(|| {
                        format!(
                            "Recovery retained in {}; clean edits ready. Ctrl/Cmd+S saves XMP.",
                            backup.display()
                        )
                    }),
                )
            }
            Ok(Outcome::Conflict(context, recovered, saved)) => {
                entry.context = Some(context);
                entry.conflict = Some((recovered, saved));
                (false, current.then(|| "Recovery differs from XMP. Ctrl/Cmd+Shift+R: use recovery; Ctrl/Cmd+Shift+X: use XMP. Both retained until choice.".into()))
            }
            Ok(Outcome::Open(context, state)) => {
                entry.context = Some(context);
                if entry.revision == 0 {
                    if let Some(state) = state {
                        if let Err(error) = entry.history.apply(state) {
                            return (false, current.then(|| error.to_string()));
                        }
                        return (current, current.then(|| "edits recovered".into()));
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
                let message = format!("edits not saved/recovered: {error:#}");
                if !current {
                    log::warn!("{}: {message}", reply.key.0.display());
                }
                (false, current.then_some(message))
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
    fn drain(editor: &mut Editor) {
        while editor
            .entries
            .values()
            .any(|entry| matches!(entry.io, IoState::Opening | IoState::Saving))
        {
            let reply = editor
                .replies
                .as_ref()
                .unwrap()
                .recv_timeout(std::time::Duration::from_secs(10))
                .unwrap();
            editor.accept(reply);
            editor.flush();
        }
    }
    #[test]
    fn reload_reuses_history_only_after_matching_content_and_persisted_state() {
        let dir = tempfile::tempdir().unwrap();
        let source = dir.path().join("image.raw");
        std::fs::write(&source, b"original").unwrap();
        let modified = std::fs::metadata(&source).unwrap().modified().unwrap();
        let mut editor = Editor::new(MemoryBudget::new(1024 * 1024), dir.path().join("journal"));
        editor.activate_version(source.clone(), 0, 1).unwrap();
        drain(&mut editor);
        let history_bytes = editor.budget.used();
        editor.apply_exposure(1.5).unwrap();
        drain(&mut editor);
        editor.activate_version(source.clone(), 0, 2).unwrap();
        assert_eq!(editor.current(), EditState::default());
        assert!(!editor.is_ready());
        drain(&mut editor);
        assert_eq!(editor.current().exposure_stops.to_bits(), (1.5_f32).to_bits());
        assert_eq!(editor.budget.used(), history_bytes);
        assert!(editor.undo_redo(false));
        drain(&mut editor);
        assert_eq!(editor.current().exposure_stops.to_bits(), (0.0_f32).to_bits());

        let journal = edit_store::journal_path(&editor.root, &source, 0);
        let bytes = std::fs::read(&journal).unwrap();
        std::fs::write(&source, b"replaced").unwrap();
        std::fs::File::options()
            .write(true)
            .open(&source)
            .unwrap()
            .set_times(std::fs::FileTimes::new().set_modified(modified))
            .unwrap();
        editor.activate_version(source.clone(), 0, 3).unwrap();
        drain(&mut editor);
        assert_eq!(editor.current(), EditState::default());
        assert!(!editor.is_ready());
        assert!(!editor.request_save());
        assert!(!editor.undo_redo(true));
        assert_eq!(std::fs::read(journal).unwrap(), bytes);
    }
    #[test]
    fn stale_open_reply_cannot_initialize_new_decode_generation() {
        let dir = tempfile::tempdir().unwrap();
        let source = dir.path().join("image.raw");
        std::fs::write(&source, b"original").unwrap();
        let mut editor = Editor::new(MemoryBudget::new(1024 * 1024), dir.path().join("journal"));
        editor.activate_version(source.clone(), 0, 1).unwrap();
        editor.activate_version(source.clone(), 0, 2).unwrap();
        let context = Context {
            source: edit_store::source_version(&source).unwrap(),
            sidecar: None,
        };
        let mut state = EditState::default();
        state.exposure_stops = 2.;
        let (changed, message) = editor.accept(Reply {
            key: (source.clone(), 0, 1),
            result: Ok(Outcome::Open(context, Some(state))),
        });
        assert!(!changed);
        assert!(message.is_none());
        assert!(!editor.is_ready());
        assert_eq!(editor.current(), EditState::default());
        let (changed, message) = editor.accept(Reply {
            key: (source, 0, 1),
            result: Err(anyhow::anyhow!("old document failure")),
        });
        assert!(!changed);
        assert!(message.is_none());
    }
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
    fn explicit_conflict_choices_update_recovery_but_require_save_for_xmp() {
        for use_recovery in [false, true] {
            let dir = tempfile::tempdir().unwrap();
            let source = dir.path().join("image.raw");
            std::fs::write(&source, b"synthetic source").unwrap();
            let root = dir.path().join("journals");
            let mut saved = EditState::default();
            saved.exposure_stops = 0.5;
            edit_store::write_sidecar(&source.with_extension("xmp"), 0, saved, None).unwrap();
            let mut recovered = saved;
            recovered.exposure_stops = 1.5;
            let version = edit_store::source_version(&source).unwrap();
            let journal = edit_store::journal_path(&root, &source, 0);
            edit_store::write_recovery(&journal, version.clone(), 0, recovered).unwrap();
            let xmp = std::fs::read(source.with_extension("xmp")).unwrap();
            let mut editor = Editor::new(MemoryBudget::new(1024 * 1024), root);
            editor.activate(source.clone(), 0).unwrap();
            drain(&mut editor);
            assert!(!editor.is_ready());
            assert!(editor.apply_exposure(2.).is_err());
            assert!(!editor.undo_redo(false));
            assert!(!editor.request_save());
            assert!(editor.resolve_conflict(use_recovery).unwrap());
            let chosen = if use_recovery { recovered } else { saved };
            assert_eq!(editor.current(), chosen);
            drain(&mut editor);
            assert!(editor.is_ready());
            assert_eq!(
                edit_store::read_recovery(&journal, &version, 0).unwrap(),
                Some(chosen)
            );
            assert_eq!(std::fs::read(source.with_extension("xmp")).unwrap(), xmp);
            assert!(editor.request_save());
            drain(&mut editor);
            assert_eq!(
                edit_store::read_sidecar(&source.with_extension("xmp"), 0)
                    .unwrap()
                    .0,
                Some(chosen)
            );
            assert!(!editor.resolve_conflict(use_recovery).unwrap());
        }
    }
    #[test]
    fn conflict_choice_does_not_authorize_overwriting_a_later_external_xmp_edit() {
        let dir = tempfile::tempdir().unwrap();
        let source = dir.path().join("image.raw");
        std::fs::write(&source, b"synthetic source").unwrap();
        let root = dir.path().join("journals");
        let xmp = source.with_extension("xmp");
        let saved = EditState::default();
        let digest = edit_store::write_sidecar(&xmp, 0, saved, None).unwrap();
        let mut recovered = saved;
        recovered.exposure_stops = 1.5;
        edit_store::write_recovery(
            &edit_store::journal_path(&root, &source, 0),
            edit_store::source_version(&source).unwrap(),
            0,
            recovered,
        )
        .unwrap();
        let mut editor = Editor::new(MemoryBudget::new(1024 * 1024), root);
        editor.activate(source, 0).unwrap();
        drain(&mut editor);
        let mut external = saved;
        external.exposure_stops = -1.;
        edit_store::write_sidecar(&xmp, 0, external, Some(digest)).unwrap();
        let bytes = std::fs::read(&xmp).unwrap();
        assert!(editor.resolve_conflict(true).unwrap());
        drain(&mut editor);
        assert!(editor.request_save());
        drain(&mut editor);
        assert_eq!(std::fs::read(xmp).unwrap(), bytes);
        assert!(editor.entries.get(editor.current.as_ref().unwrap()).unwrap().io == IoState::Failed);
        assert_eq!(editor.current(), recovered);
    }
    #[test]
    fn refused_recovery_requires_explicit_restart_and_retains_exact_bytes() {
        for reason in ["changed_source", "unknown_schema", "corrupt"] {
            let dir = tempfile::tempdir().unwrap();
            let source = dir.path().join("image.raw");
            std::fs::write(&source, b"original").unwrap();
            let root = dir.path().join("journals");
            let journal = edit_store::journal_path(&root, &source, 0);
            let mut state = EditState::default();
            state.exposure_stops = 1.5;
            let digest = edit_store::write_sidecar(&source.with_extension("xmp"), 0, state, None).unwrap();
            edit_store::write_recovery(&journal, edit_store::source_version(&source).unwrap(), 0, state)
                .unwrap();
            match reason {
                "changed_source" => std::fs::write(&source, b"replacement").unwrap(),
                "unknown_schema" => {
                    let canonical = std::fs::read_to_string(&journal).unwrap().replacen(
                        "\"schema\":1,",
                        "\"schema\":999,",
                        1,
                    );
                    let start = canonical.find("{\"schema\":999,").unwrap();
                    let end = canonical.rfind(",\"checksum\":").unwrap();
                    let checksum = blake3::hash(&canonical.as_bytes()[start..end]);
                    let mut json: serde_json::Value = serde_json::from_str(&canonical).unwrap();
                    json["checksum"] = serde_json::to_value(checksum.as_bytes()).unwrap();
                    std::fs::write(&journal, serde_json::to_vec(&json).unwrap()).unwrap();
                }
                _ => std::fs::write(&journal, b"unparseable original recovery").unwrap(),
            }
            let bytes = std::fs::read(&journal).unwrap();
            let xmp = std::fs::read(source.with_extension("xmp")).unwrap();
            let mut editor = Editor::new(MemoryBudget::new(1024 * 1024), root.clone());
            editor.activate(source.clone(), 0).unwrap();
            drain(&mut editor);
            assert!(!editor.is_ready());
            assert!(!editor.request_save());
            assert!(editor.apply_exposure(2.).is_err());
            assert_eq!(std::fs::read(&journal).unwrap(), bytes);
            assert!(editor.request_restart_recovery().unwrap());
            assert!(!editor.request_restart_recovery().unwrap());
            drain(&mut editor);
            assert!(editor.is_ready());
            assert_eq!(editor.current(), EditState::default());
            assert!(!editor.undo_redo(false));
            assert_eq!(
                edit_store::read_recovery(&journal, &edit_store::source_version(&source).unwrap(), 0)
                    .unwrap(),
                Some(EditState::default())
            );
            let backups: Vec<_> = std::fs::read_dir(&root)
                .unwrap()
                .flatten()
                .filter(|entry| entry.path().extension().is_some_and(|e| e == "retained"))
                .collect();
            assert_eq!(backups.len(), 1);
            assert_eq!(std::fs::read(backups[0].path()).unwrap(), bytes);
            assert_eq!(std::fs::read(source.with_extension("xmp")).unwrap(), xmp);
            assert_eq!(
                edit_store::read_sidecar(&source.with_extension("xmp"), 0)
                    .unwrap()
                    .1,
                Some(digest)
            );
        }
    }
    #[test]
    fn restart_succeeds_when_existing_history_uses_the_entire_budget() {
        let dir = tempfile::tempdir().unwrap();
        let source = dir.path().join("source.raw");
        std::fs::write(&source, b"synthetic source").unwrap();
        let root = dir.path().join("journals");
        std::fs::create_dir_all(&root).unwrap();
        let journal = edit_store::journal_path(&root, &source, 0);
        std::fs::write(&journal, b"refused original record").unwrap();
        let bytes = u64::try_from(64 * std::mem::size_of::<EditState>()).unwrap();
        let budget = MemoryBudget::new(bytes);
        let mut editor = Editor::new(budget.clone(), root);
        editor.activate(source, 0).unwrap();
        drain(&mut editor);
        assert_eq!(budget.used(), bytes);
        assert!(editor.request_restart_recovery().unwrap());
        drain(&mut editor);
        assert!(editor.is_ready());
        assert_eq!(budget.used(), bytes);
        assert_eq!(editor.current(), EditState::default());
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
            assert_eq!(editor.current().exposure_stops.to_bits(), (1.5_f32).to_bits());
            assert!(editor.undo_redo(false));
            assert_eq!(editor.current().exposure_stops.to_bits(), (0.0_f32).to_bits());
            assert!(editor.undo_redo(true));
            editor.request_save();
            editor.shutdown();
            assert!(editor.is_ready());
            assert_eq!(
                edit_store::read_sidecar(&source.with_extension("xmp"), 0)
                    .unwrap()
                    .0
                    .unwrap()
                    .exposure_stops
                    .to_bits(),
                (1.5_f32).to_bits()
            );
        }
        assert_eq!(budget.used(), 0);
        let mut restored = Editor::new(budget.clone(), root);
        restored.activate(source.clone(), 1).unwrap();
        restored.shutdown();
        assert_eq!(restored.current().exposure_stops.to_bits(), (-0.5_f32).to_bits());
    }
}
