//! UI history and a bounded background persistence actor. No filesystem work on key events.
use crate::edit_store::{self, SourceVersion};
use anyhow::Context as _;
use crossbeam_channel::{Receiver, Sender, bounded};
use rrrah_core::{
    MemoryBudget,
    edit::{EditSession, EditState},
};
use std::{
    collections::BTreeMap,
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, Instant},
};

// A new decode must not display a previous file version's history while the
// persistence actor establishes the full content identity asynchronously.
type Key = (PathBuf, usize, u64);
#[derive(Clone)]
struct Context {
    source: SourceVersion,
    sidecar: Option<[u8; 32]>,
    recovery: Option<[u8; 32]>,
    own_recovery: bool,
    foreign_recovery: bool,
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
    carried: bool,
}
enum Job {
    Open(Key, Option<Result<SourceVersion, String>>),
    Retain {
        key: Key,
        source: SourceVersion,
        state: EditState,
        revision: u64,
        reason: String,
    },
    Restart {
        key: Key,
        context: Context,
        expected: [u8; 32],
        draft: Option<(SourceVersion, EditState)>,
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
fn remember_recovery(
    written: &mut BTreeMap<PathBuf, (SourceVersion, [u8; 32])>,
    path: PathBuf,
    source: SourceVersion,
    digest: [u8; 32],
) {
    if written.len() >= 32 && !written.contains_key(&path) {
        written.pop_first();
    }
    written.insert(path, (source, digest));
}

fn retain_entry(key: &Key, entry: &mut Entry, jobs: &Sender<Job>, reason: String) {
    entry.io = IoState::Failed;
    entry.dirty = true;
    entry.save_sidecar = false;
    if let Some(context) = &entry.context {
        let job = Job::Retain {
            key: key.clone(),
            source: context.source.clone(),
            state: entry.history.current(),
            revision: entry.revision,
            reason,
        };
        if jobs.try_send(job).is_ok() {
            entry.context = None;
            entry.carried = false;
            entry.io = IoState::Saving;
        }
    }
}

enum Outcome {
    Open(Context, Option<EditState>, Option<EditState>),
    Conflict(Context, EditState, EditState),
    ExternalConflict(Context, Option<EditState>),
    RetainedDraft {
        revision: u64,
        backup: PathBuf,
        reason: String,
    },
    RefusedRecovery(Context, [u8; 32], String),
    Restarted(Context, PathBuf),
    Saved {
        revision: u64,
        sidecar: Option<[u8; 32]>,
        saved_sidecar: bool,
        recovery: [u8; 32],
    },
}
struct Reply {
    key: Key,
    result: anyhow::Result<Outcome>,
}
fn open_document(
    root: &std::path::Path,
    key: &Key,
    admitted: Option<&SourceVersion>,
) -> anyhow::Result<Outcome> {
    let source = if let Some(source) = admitted {
        edit_store::verify_source_stamp(&key.0, source)?;
        source.clone()
    } else {
        edit_store::source_version(&key.0)?
    };
    let (sidecar_state, sidecar) = edit_store::read_sidecar(&key.0.with_extension("xmp"), key.1)?;
    let journal = edit_store::journal_path(root, &key.0, key.1);
    let (recovery, recovery_digest) = match edit_store::read_recovery_record(&journal, &source, key.1)? {
        edit_store::RecoveryRead::Missing => (None, None),
        edit_store::RecoveryRead::Valid(state, digest) => (Some(state), Some(digest)),
        edit_store::RecoveryRead::Refused(error, digest) => {
            return Ok(Outcome::RefusedRecovery(
                Context {
                    source,
                    sidecar,
                    recovery: Some(digest),
                    own_recovery: false,
                    foreign_recovery: false,
                },
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
    let context = Context {
        source,
        sidecar,
        recovery: recovery_digest,
        own_recovery: false,
        foreign_recovery: false,
    };
    if let (Some(recovered), Some(saved)) = (recovery, sidecar_state) {
        if recovered != saved {
            return Ok(Outcome::Conflict(context, recovered, saved));
        }
    }
    Ok(Outcome::Open(context, recovery.or(sidecar_state), sidecar_state))
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
    stop: Arc<AtomicBool>,
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
            stop: Arc::new(AtomicBool::new(false)),
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
        let stop = self.stop.clone();
        self.thread = Some(
            std::thread::Builder::new()
                .name("edit-store".into())
                .spawn(move || {
                    let mut written:BTreeMap<PathBuf,(SourceVersion,[u8;32])>=BTreeMap::new();
                    while let Ok(job) = rx.recv() {
                        if stop.load(Ordering::Acquire) {
                            break;
                        }
                        let (key, result) = match job {
                            Job::Stop => break,
                            Job::Retain{key,source,state,revision,reason} => {
                                let result=edit_store::retain_draft(&edit_store::journal_path(&root,&key.0,key.1),source,key.1,state).map(|backup|Outcome::RetainedDraft{revision,backup,reason});
                                (key,result)
                            }
                            Job::Open(key,admitted) => {
                                let mut result = match admitted {
                                    Some(Ok(source)) => open_document(&root,&key,Some(&source)),
                                    Some(Err(error)) => Err(anyhow::anyhow!("source edit identity unavailable: {error}")),
                                    None => open_document(&root,&key,None),
                                };
                                if let Ok(Outcome::Open(c,_,_)|Outcome::Conflict(c,_,_)|Outcome::RefusedRecovery(c,_,_))=&mut result {
                                    let journal=edit_store::journal_path(&root,&key.0,key.1);
                                    c.own_recovery=c.recovery.is_some() && written.get(&journal).is_some_and(|(source,digest)|source==&c.source && Some(*digest)==c.recovery);
                                    if !c.own_recovery {written.remove(&journal);}
                                }
                                (key, result)
                            }
                            Job::Restart {
                                key,
                                context,
                                expected,
                                draft,
                            } => {
                                let result = (|| {
                                    if let Some((source,state))=draft {edit_store::retain_draft(&edit_store::journal_path(&root,&key.0,key.1),source,key.1,state)?;}
                                    let journal=edit_store::journal_path(&root,&key.0,key.1);
                                    let (backup,recovery)=edit_store::restart_recovery_with_digest(&journal,&key.0,&context.source,key.1,expected)?;
                                    remember_recovery(&mut written,journal,context.source.clone(),recovery);
                                    let mut context=context;context.recovery=Some(recovery);context.own_recovery=true;
                                    Ok(Outcome::Restarted(context,backup))
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
                                    let journal = edit_store::journal_path(&root, &key.0, key.1);
                                    if let Err(error)=edit_store::verify_source(&key.0,&context.source) {
                                        let backup=edit_store::retain_draft(&journal,context.source.clone(),key.1,state)?;
                                        return Ok(Outcome::RetainedDraft{revision,backup,reason:error.to_string()});
                                    }
                                    let recovery=match edit_store::write_recovery_checked(&journal,context.source.clone(),key.1,state,context.recovery,written.get(&journal).filter(|(source,_)|source==&context.source).map(|(_,digest)|*digest)) {
                                        Ok(digest)=>{remember_recovery(&mut written,journal.clone(),context.source.clone(),digest);digest},
                                        Err(error) if matches!(error.downcast_ref::<edit_store::StoreError>(),Some(edit_store::StoreError::RecoveryChanged))=>{
                                            let backup=edit_store::retain_draft(&journal,context.source.clone(),key.1,state)?;
                                            return Ok(Outcome::RetainedDraft{revision,backup,reason:error.to_string()});
                                        },
                                        Err(error)=>return Err(error),
                                    };
                                    let digest = if sidecar {
                                        match edit_store::write_sidecar(
                                            &key.0.with_extension("xmp"),
                                            key.1,
                                            state,
                                            context.sidecar,
                                        ) {
                                            Ok(digest) => Some(digest),
                                            Err(error)
                                                if matches!(
                                                    error.downcast_ref::<edit_store::StoreError>(),
                                                    Some(edit_store::StoreError::SidecarChanged)
                                                ) =>
                                            {
                                                let (saved, digest) = edit_store::read_sidecar(
                                                    &key.0.with_extension("xmp"),
                                                    key.1,
                                                )?;
                                                if saved.is_some_and(|s|s.exposure_stops.abs()>10.) {anyhow::bail!("external exposure retained but outside viewer -10..10 range");}
                                                return Ok(Outcome::ExternalConflict(
                                                    Context {
                                                        source: context.source,
                                                        sidecar: digest,
                                                        recovery:Some(recovery),own_recovery:true,foreign_recovery:false,
                                                    },
                                                    saved,
                                                ));
                                            }
                                            Err(error) => return Err(error),
                                        }
                                    } else {
                                        context.sidecar
                                    };
                                    Ok(Outcome::Saved {
                                        revision,
                                        sidecar: digest,
                                        saved_sidecar: sidecar,
                                        recovery,
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
    #[cfg(test)]
    pub fn activate_version(&mut self, path: PathBuf, page: usize, generation: u64) -> anyhow::Result<bool> {
        self.activate_inner(path, page, generation, None)
    }
    pub fn activate_admitted(
        &mut self,
        path: PathBuf,
        page: usize,
        generation: u64,
        source: Result<SourceVersion, String>,
    ) -> anyhow::Result<bool> {
        self.activate_inner(path, page, generation, Some(source))
    }
    fn activate_inner(
        &mut self,
        path: PathBuf,
        page: usize,
        generation: u64,
        admitted: Option<Result<SourceVersion, String>>,
    ) -> anyhow::Result<bool> {
        let key = (path, page, generation);
        if self.current.as_ref() == Some(&key) {
            return Ok(false);
        }
        self.end_curve_gesture();
        self.current = None;
        if !self.entries.contains_key(&key) {
            let carry = admitted
                .as_ref()
                .and_then(|v| v.as_ref().ok())
                .and_then(|version| {
                    self.entries.iter().find_map(|(old, e)| {
                        (old.0 == key.0
                            && old.1 == key.1
                            && (e.dirty || e.io == IoState::Saving)
                            && e.conflict.is_none()
                            && e.refused_recovery.is_none()
                            && e.context.as_ref().is_some_and(|c| &c.source == version))
                        .then(|| old.clone())
                    })
                });
            if self.entries.len() == 16 && carry.is_none() {
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
            let history = if carry.is_none() {
                Some(EditSession::new(EditState::default(), 64, &self.budget)?)
            } else {
                None
            };
            self.start()?;
            self.jobs
                .as_ref()
                .unwrap()
                .try_send(Job::Open(key.clone(), admitted))?;
            let entry = if let Some(old) = carry {
                let mut entry = self.entries.remove(&old).unwrap();
                entry.io = IoState::Opening;
                entry.dirty = true;
                entry.save_sidecar = false;
                entry.carried = true;
                entry
            } else {
                Entry {
                    history: history.unwrap(),
                    revision: 0,
                    used: 0,
                    context: None,
                    io: IoState::Opening,
                    dirty: false,
                    save_sidecar: false,
                    conflict: None,
                    refused_recovery: None,
                    carried: false,
                }
            };
            self.entries.insert(key.clone(), entry);
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
                    && !entry.carried
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
            "Local edits differ from a saved version. Ctrl/Cmd+Shift+R uses local edits; Ctrl/Cmd+Shift+X uses the saved version."
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
    pub fn begin_curve_gesture(&mut self) {
        if let Some(entry) = self.current.as_ref().and_then(|k| self.entries.get_mut(k)) {
            entry.history.begin_gesture();
        }
    }
    pub fn end_curve_gesture(&mut self) {
        if let Some(entry) = self.current.as_ref().and_then(|k| self.entries.get_mut(k)) {
            entry.history.end_gesture();
        }
    }
    pub fn apply_curve(&mut self, points: &[[f64; 2]]) -> anyhow::Result<()> {
        if !self.is_ready() {
            anyhow::bail!("resolve edits before changing the curve");
        }
        let entry = self
            .current
            .as_ref()
            .and_then(|k| self.entries.get_mut(k))
            .context("no edit session")?;
        let state = entry.history.current().with_curve(points)?;
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
    pub fn apply_exposure(&mut self, exposure: f32) -> anyhow::Result<()> {
        let Some(entry) = self.current.as_ref().and_then(|k| self.entries.get_mut(k)) else {
            anyhow::bail!("no edit session");
        };
        if entry.carried
            || entry.context.is_none() && entry.io != IoState::Opening
            || entry.conflict.is_some()
            || entry.refused_recovery.is_some()
        {
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
        if entry.context.is_none() {
            return Ok(false);
        }
        let Some((recovery, saved)) = entry.conflict else {
            return Ok(false);
        };
        entry.history.apply(if use_recovery { recovery } else { saved })?;
        entry.conflict = None;
        if let Some(context) = &mut entry.context {
            context.foreign_recovery = false;
        }
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
            draft: self.entries.get(&key).filter(|e| e.carried).and_then(|e| {
                e.context
                    .as_ref()
                    .map(|c| (c.source.clone(), e.history.current()))
            }),
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
                || (entry.conflict.is_some() && entry.context.as_ref().is_none_or(|c| c.foreign_recovery))
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
                sidecar: entry.save_sidecar && entry.conflict.is_none(),
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
        if let Ok(Outcome::Open(context, state, _)) = &reply.result {
            if self.reuse_history(&reply.key, context, state.as_ref()) {
                return (current, None);
            }
        }
        let Some(entry) = self.entries.get_mut(&reply.key) else {
            return (false, None);
        };
        entry.io = IoState::Idle;
        if entry.carried {
            let context = match &reply.result {
                Ok(Outcome::Open(c, _, _) | Outcome::Conflict(c, _, _)) => Some(c),
                _ => None,
            };
            if context.is_some_and(|context| {
                entry.context.as_ref().is_some_and(|old| {
                    old.source == context.source
                        && old.sidecar == context.sidecar
                        && (old.recovery == context.recovery || context.own_recovery)
                })
            }) {
                entry.context = context.cloned();
                entry.carried = false;
                entry.dirty = true;
                return (
                    current,
                    current.then(|| "latest local draft and undo history retained across reload".into()),
                );
            }
        }
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
                entry.carried = false;
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
            Ok(Outcome::ExternalConflict(context, saved)) => {
                entry.context = Some(context);
                entry.conflict = Some((entry.history.current(), saved.unwrap_or_default()));
                (false, current.then(|| "XMP changed externally. Choose local edits (Ctrl/Cmd+Shift+R) or external XMP (Ctrl/Cmd+Shift+X). No XMP overwritten.".into()))
            }
            Ok(Outcome::Conflict(mut context, recovered, saved)) => {
                let recovery_changed = entry.carried
                    && entry
                        .context
                        .as_ref()
                        .is_some_and(|old| old.recovery != context.recovery)
                    && !context.own_recovery;
                context.foreign_recovery = recovery_changed;
                entry.context = Some(context);
                let local = if entry.carried {
                    entry.history.current()
                } else {
                    recovered
                };
                entry.carried = false;
                entry.conflict = Some((local, if recovery_changed { recovered } else { saved }));
                if recovery_changed {
                    return(false,current.then(||"Recovery changed externally; choose local edits or saved recovery. XMP unchanged until Save.".into()));
                }
                (false, current.then(|| "Recovery differs from XMP. Ctrl/Cmd+Shift+R: use recovery; Ctrl/Cmd+Shift+X: use XMP. Both retained until choice.".into()))
            }
            Ok(Outcome::Open(mut context, state, saved)) => {
                let recovery_changed = entry.carried
                    && entry
                        .context
                        .as_ref()
                        .is_some_and(|old| old.recovery != context.recovery)
                    && !context.own_recovery;
                context.foreign_recovery = recovery_changed;
                entry.context = Some(context);
                if entry.carried {
                    entry.carried = false;
                    entry.conflict = Some((
                        entry.history.current(),
                        if recovery_changed {
                            state.unwrap_or_default()
                        } else {
                            saved.unwrap_or_default()
                        },
                    ));
                    return (
                        false,
                        current.then(|| {
                            "Saved edits changed during reload; choose local draft or saved version before saving".into()
                        }),
                    );
                }

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
            Ok(Outcome::RetainedDraft {
                revision,
                backup,
                reason,
            }) => {
                if revision == entry.revision {
                    entry.dirty = false;
                    entry.io = IoState::Failed;
                    entry.context = None;
                } else {
                    entry.dirty = true;
                    entry.io = IoState::Idle;
                }
                (
                    false,
                    current.then(|| {
                        format!(
                            "{reason}; old-version edits retained in {}; reload before editing",
                            backup.display()
                        )
                    }),
                )
            }
            Ok(Outcome::Saved {
                revision,
                sidecar,
                saved_sidecar,
                recovery,
            }) => {
                if let Some(context) = &mut entry.context {
                    context.sidecar = sidecar;
                    context.recovery = Some(recovery);
                    context.own_recovery = true;
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
                if entry.carried {
                    if let Some(jobs) = &self.jobs {
                        retain_entry(
                            &reply.key,
                            entry,
                            jobs,
                            format!("source/recovery open refused: {error}"),
                        );
                    }
                    return(current,current.then(||format!("source/recovery open refused: {error}; retaining the local draft separately; XMP and refused journal unchanged")));
                }
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
    pub fn has_conflict(&self) -> bool {
        self.current
            .as_ref()
            .and_then(|k| self.entries.get(k))
            .is_some_and(|e| e.conflict.is_some())
    }
    pub fn shutdown(&mut self) {
        self.shutdown_with_timeout(Duration::from_secs(2));
    }
    fn shutdown_with_timeout(&mut self, timeout: Duration) {
        if self.thread.is_none() && self.jobs.is_none() {
            return;
        }
        let deadline = Instant::now() + timeout;
        loop {
            self.poll();
            if let Some(jobs) = &self.jobs {
                for (key, entry) in &mut self.entries {
                    if entry.dirty
                        && entry.context.is_some()
                        && (entry.carried || entry.conflict.is_some() || entry.refused_recovery.is_some())
                        && !matches!(entry.io, IoState::Opening | IoState::Saving)
                    {
                        retain_entry(key, entry, jobs, "closed with a refused recovery document".into());
                    }
                }
            }
            self.flush();
            if !self
                .entries
                .values()
                .any(|e| matches!(e.io, IoState::Opening | IoState::Saving))
            {
                break;
            }
            let remaining = deadline.saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                break;
            }
            let Some(reply) = self.replies.as_ref().and_then(|r| r.recv_timeout(remaining).ok()) else {
                break;
            };
            let (_, message) = self.accept(reply);
            if let Some(message) = message {
                log::warn!("{message}");
            }
        }
        if self
            .entries
            .values()
            .any(|e| e.dirty || matches!(e.io, IoState::Opening | IoState::Saving))
        {
            log::warn!(
                "Edit shutdown deadline reached or edits unresolved: only completed atomic snapshots are durable; pending edits may be unsaved."
            );
        }
        self.stop.store(true, Ordering::Release);
        if let Some(jobs) = self.jobs.take() {
            let _ = jobs.try_send(Job::Stop);
        }
        // A blocked filesystem call cannot be cancelled safely. Detach the actor
        // at the deadline; it owns its data and may finish its atomic transaction.
        if let Some(thread) = self.thread.take() {
            while !thread.is_finished() && Instant::now() < deadline {
                std::thread::sleep(Duration::from_millis(1));
            }
            if thread.is_finished() {
                let _ = thread.join();
            }
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
    fn foreign_recovery_is_not_overwritten_and_latest_local_draft_is_retained() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("image.raw");
        std::fs::write(&path, b"original").unwrap();
        let root = dir.path().join("journal");
        let version = edit_store::source_version(&path).unwrap();
        let mut editor = Editor::new(MemoryBudget::new(1024 * 1024), root.clone());
        editor.activate_admitted(path.clone(), 0, 1, Ok(version)).unwrap();
        drain(&mut editor);
        editor.apply_exposure(1.).unwrap();
        drain(&mut editor);
        let journal = edit_store::journal_path(&root, &path, 0);
        std::fs::write(&journal, b"foreign recovery").unwrap();
        editor.apply_exposure(2.).unwrap();
        drain(&mut editor);
        assert_eq!(std::fs::read(&journal).unwrap(), b"foreign recovery");
        assert!(!path.with_extension("xmp").exists());
        assert!(!editor.is_ready());
        let backups: Vec<_> = std::fs::read_dir(journal.parent().unwrap())
            .unwrap()
            .flatten()
            .filter(|e| e.file_name().to_string_lossy().contains("retained"))
            .collect();
        assert_eq!(backups.len(), 1);
        let source = edit_store::source_version(&path).unwrap();
        assert_eq!(
            edit_store::read_recovery(&backups[0].path(), &source, 0)
                .unwrap()
                .unwrap()
                .exposure_stops,
            2.
        );
    }
    #[test]
    fn admitted_reload_keeps_latest_draft_while_an_older_save_is_in_flight() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("image.raw");
        std::fs::write(&path, b"original").unwrap();
        let version = edit_store::source_version(&path).unwrap();
        let bytes = u64::try_from(64 * std::mem::size_of::<EditState>()).unwrap();
        let mut editor = Editor::new(MemoryBudget::new(bytes), dir.path().join("journal"));
        editor
            .activate_admitted(path.clone(), 0, 1, Ok(version.clone()))
            .unwrap();
        drain(&mut editor);
        editor.apply_exposure(1.).unwrap();
        editor.apply_exposure(2.).unwrap();
        editor.activate_admitted(path, 0, 2, Ok(version)).unwrap();
        drain(&mut editor);
        assert_eq!(editor.current().exposure_stops, 2.);
        assert!(editor.undo_redo(false));
        assert_eq!(editor.current().exposure_stops, 1.);
    }
    #[test]
    fn carried_latest_draft_still_requires_choice_for_external_xmp_changes() {
        for local in [true, false] {
            let dir = tempfile::tempdir().unwrap();
            let path = dir.path().join("image.raw");
            std::fs::write(&path, b"original").unwrap();
            let source = edit_store::source_version(&path).unwrap();
            let mut editor = Editor::new(MemoryBudget::new(1024 * 1024), dir.path().join("journal"));
            editor
                .activate_admitted(path.clone(), 0, 1, Ok(source.clone()))
                .unwrap();
            drain(&mut editor);
            editor.apply_exposure(1.).unwrap();
            editor.apply_exposure(2.).unwrap();
            let mut external = EditState::default();
            external.exposure_stops = -1.;
            let xmp = path.with_extension("xmp");
            edit_store::write_sidecar(&xmp, 0, external, None).unwrap();
            let bytes = std::fs::read(&xmp).unwrap();
            editor.activate_admitted(path, 0, 2, Ok(source)).unwrap();
            drain(&mut editor);
            assert!(editor.has_conflict());
            assert_eq!(editor.current().exposure_stops, 2.);
            assert!(editor.resolve_conflict(local).unwrap());
            drain(&mut editor);
            assert_eq!(editor.current().exposure_stops, if local { 2. } else { -1. });
            assert_eq!(std::fs::read(xmp).unwrap(), bytes);
        }
    }
    #[test]
    fn refused_recovery_or_xmp_never_overwrites_foreign_bytes_and_keeps_latest_draft() {
        for mode in ["close", "clean", "bad_xmp"] {
            let dir = tempfile::tempdir().unwrap();
            let path = dir.path().join("image.raw");
            std::fs::write(&path, b"original").unwrap();
            let version = edit_store::source_version(&path).unwrap();
            let root = dir.path().join("journal");
            let journal = edit_store::journal_path(&root, &path, 0);
            let mut editor = Editor::new(MemoryBudget::new(1024 * 1024), root.clone());
            editor
                .activate_admitted(path.clone(), 0, 1, Ok(version.clone()))
                .unwrap();
            drain(&mut editor);
            editor.apply_exposure(1.).unwrap();
            drain(&mut editor);
            // Model backpressure: the newest UI draft has not reached the actor.
            let entry = editor.entries.get_mut(editor.current.as_ref().unwrap()).unwrap();
            let mut state = entry.history.current();
            state.exposure_stops = 2.;
            entry.history.apply(state).unwrap();
            entry.revision += 1;
            entry.dirty = true;
            entry.io = IoState::Saving;
            std::fs::write(&journal, b"foreign future recovery bytes").unwrap();
            let xmp = path.with_extension("xmp");
            if mode == "bad_xmp" {
                std::fs::write(&xmp, b"<malformed XMP").unwrap();
            }
            editor.activate_admitted(path, 0, 2, Ok(version.clone())).unwrap();
            drain(&mut editor);
            assert!(!editor.is_ready());
            assert_eq!(std::fs::read(&journal).unwrap(), b"foreign future recovery bytes");
            if mode == "clean" {
                assert!(editor.request_restart_recovery().unwrap());
                drain(&mut editor);
                assert!(editor.is_ready());
            } else {
                editor.shutdown();
                assert_eq!(std::fs::read(&journal).unwrap(), b"foreign future recovery bytes");
            }
            if mode == "bad_xmp" {
                assert_eq!(std::fs::read(xmp).unwrap(), b"<malformed XMP");
            }
            let states: Vec<_> = std::fs::read_dir(root)
                .unwrap()
                .map(|e| e.unwrap().path())
                .filter(|p| p.extension().is_some_and(|e| e == "retained"))
                .filter_map(|p| edit_store::read_recovery(&p, &version, 0).ok().flatten())
                .collect();
            assert!(
                states.iter().any(|s| s.exposure_stops == 2.),
                "latest draft must survive even when canonical recovery is refused"
            );
        }
    }
    #[test]
    fn decode_admission_identity_refuses_changed_source_before_any_persistence() {
        for preserve_stamp in [false, true] {
            let dir = tempfile::tempdir().unwrap();
            let path = dir.path().join("image.raw");
            std::fs::write(&path, b"original").unwrap();
            let admitted = edit_store::source_version(&path).unwrap();
            let modified = std::fs::metadata(&path).unwrap().modified().unwrap();
            std::fs::write(
                &path,
                if preserve_stamp {
                    b"replaced".as_slice()
                } else {
                    b"a new longer source".as_slice()
                },
            )
            .unwrap();
            if preserve_stamp {
                std::fs::File::options()
                    .write(true)
                    .open(&path)
                    .unwrap()
                    .set_times(std::fs::FileTimes::new().set_modified(modified))
                    .unwrap();
            }
            let root = dir.path().join("journal");
            let mut editor = Editor::new(MemoryBudget::new(1024 * 1024), root.clone());
            editor
                .activate_admitted(path.clone(), 0, 1, Ok(admitted.clone()))
                .unwrap();
            drain(&mut editor);
            if preserve_stamp {
                assert!(editor.is_ready());
                editor.apply_exposure(1.).unwrap();
                drain(&mut editor);
                assert!(editor.entries.get(editor.current.as_ref().unwrap()).unwrap().io == IoState::Failed);
            } else {
                assert!(!editor.is_ready());
                assert!(!editor.request_save());
            }
            assert!(!path.with_extension("xmp").exists());
            assert!(!edit_store::journal_path(&root, &path, 0).exists());
            if preserve_stamp {
                let backups: Vec<_> = std::fs::read_dir(&root)
                    .unwrap()
                    .map(|e| e.unwrap().path())
                    .filter(|p| p.extension().is_some_and(|e| e == "retained"))
                    .collect();
                assert_eq!(backups.len(), 1);
                let state = edit_store::read_recovery(&backups[0], &admitted, 0)
                    .unwrap()
                    .unwrap();
                assert_eq!(state.exposure_stops, 1.);
                assert_eq!(std::fs::read(&path).unwrap(), b"replaced");
            }
        }
    }
    #[test]
    fn unavailable_decode_identity_is_read_only_and_never_falls_back_to_current_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("image.raw");
        std::fs::write(&path, b"new file at same path").unwrap();
        let mut editor = Editor::new(MemoryBudget::new(1024 * 1024), dir.path().join("journal"));
        editor
            .activate_admitted(path.clone(), 0, 8, Err("admission failed".into()))
            .unwrap();
        drain(&mut editor);
        assert!(!editor.is_ready());
        assert!(!editor.request_save());
        assert!(editor.apply_curve(EditState::default().points()).is_err());
        assert!(!path.with_extension("xmp").exists());
    }
    #[test]
    fn curve_drag_is_one_undo_and_restores_across_document_reload() {
        let dir = tempfile::tempdir().unwrap();
        let source = dir.path().join("image.raw");
        std::fs::write(&source, b"source").unwrap();
        let root = dir.path().join("journal");
        let mut editor = Editor::new(MemoryBudget::new(1024 * 1024), root.clone());
        editor.activate_version(source.clone(), 0, 1).unwrap();
        drain(&mut editor);
        editor.begin_curve_gesture();
        for y in [0.4, 0.6, 0.7] {
            editor.apply_curve(&[[0., 0.], [0.5, y], [1., 1.]]).unwrap();
        }
        editor.end_curve_gesture();
        drain(&mut editor);
        let edited = editor.current();
        assert_eq!(edited.points()[1], [0.5, 0.7]);
        assert!(editor.undo_redo(false));
        assert_eq!(editor.current(), EditState::default());
        assert!(!editor.undo_redo(false));
        assert!(editor.undo_redo(true));
        assert_eq!(editor.current(), edited);
        drain(&mut editor);
        assert!(editor.request_save());
        drain(&mut editor);
        editor.shutdown();
        drop(editor);
        let mut next = Editor::new(MemoryBudget::new(1024 * 1024), root);
        next.activate_version(source, 0, 2).unwrap();
        drain(&mut next);
        assert!(next.is_ready());
        assert_eq!(next.current(), edited);
    }
    #[test]
    fn external_xmp_choice_retains_local_state_until_explicit_resolution() {
        for choose_local in [true, false] {
            let dir = tempfile::tempdir().unwrap();
            let source = dir.path().join("image.raw");
            std::fs::write(&source, b"source").unwrap();
            let xmp = source.with_extension("xmp");
            let mut editor = Editor::new(MemoryBudget::new(1024 * 1024), dir.path().join("journal"));
            editor.activate_version(source.clone(), 0, 1).unwrap();
            drain(&mut editor);
            editor.apply_exposure(1.25).unwrap();
            drain(&mut editor);
            let mut external = EditState::default();
            external.exposure_stops = -2.;
            edit_store::write_sidecar(&xmp, 0, external, None).unwrap();
            let bytes = std::fs::read(&xmp).unwrap();
            assert!(editor.request_save());
            drain(&mut editor);
            assert!(editor.has_conflict());
            assert!(!editor.is_ready());
            assert_eq!(editor.current().exposure_stops, 1.25);
            assert_eq!(std::fs::read(&xmp).unwrap(), bytes);
            assert!(!editor.request_save());
            assert!(editor.resolve_conflict(choose_local).unwrap());
            drain(&mut editor);
            assert_eq!(std::fs::read(&xmp).unwrap(), bytes);
            assert_eq!(
                editor.current().exposure_stops,
                if choose_local { 1.25 } else { -2. }
            );
            assert!(editor.request_save());
            drain(&mut editor);
            assert!(!editor.has_conflict());
            assert_eq!(
                edit_store::read_sidecar(&xmp, 0).unwrap().0,
                Some(editor.current())
            );
        }
    }
    #[test]
    fn shutdown_deadline_bounds_stalled_actor_without_joining_it() {
        for pending in [false, true] {
            let dir = tempfile::tempdir().unwrap();
            let source = dir.path().join("image.raw");
            std::fs::write(&source, b"source").unwrap();
            let mut editor = Editor::new(MemoryBudget::new(1024 * 1024), dir.path().join("journal"));
            editor.activate_version(source, 0, 1).unwrap();
            drain(&mut editor);
            editor.shutdown();
            if pending {
                editor.entries.values_mut().next().unwrap().io = IoState::Opening;
                let (_sender, receiver) = bounded(1);
                editor.replies = Some(receiver);
                // Keep the synthetic sender alive until after shutdown.
                let (release, wait) = bounded::<()>(1);
                let (done, exited) = bounded(1);
                editor.thread = Some(std::thread::spawn(move || {
                    wait.recv().unwrap();
                    done.send(()).unwrap();
                }));
                let now = Instant::now();
                editor.shutdown_with_timeout(Duration::from_millis(20));
                assert!(now.elapsed() < Duration::from_millis(250));
                let again = Instant::now();
                editor.shutdown_with_timeout(Duration::from_secs(2));
                assert!(again.elapsed() < Duration::from_millis(250));
                release.send(()).unwrap();
                exited.recv_timeout(Duration::from_secs(2)).unwrap();
            } else {
                let (release, wait) = bounded::<()>(1);
                let (done, exited) = bounded(1);
                editor.thread = Some(std::thread::spawn(move || {
                    wait.recv().unwrap();
                    done.send(()).unwrap();
                }));
                let now = Instant::now();
                editor.shutdown_with_timeout(Duration::from_millis(20));
                assert!(now.elapsed() < Duration::from_millis(250));
                let again = Instant::now();
                editor.shutdown_with_timeout(Duration::from_secs(2));
                assert!(again.elapsed() < Duration::from_millis(250));
                release.send(()).unwrap();
                exited.recv_timeout(Duration::from_secs(2)).unwrap();
            }
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
            recovery: None,
            own_recovery: false,
            foreign_recovery: false,
        };
        let mut state = EditState::default();
        state.exposure_stops = 2.;
        let (changed, message) = editor.accept(Reply {
            key: (source.clone(), 0, 1),
            result: Ok(Outcome::Open(context, Some(state), None)),
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
        assert!(editor.has_conflict());
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
