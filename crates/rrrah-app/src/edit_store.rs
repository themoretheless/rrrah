//! Bounded atomic sidecars, legacy snapshots and versioned recovery command logs.
use anyhow::{Context, Result, bail};
use rrrah_core::edit::{EDIT_SCHEMA, EditState};
use serde::{Deserialize, Serialize};
use std::{
    fs,
    io::{Read, Write},
    path::{Path, PathBuf},
};

/// Stable error categories survive anyhow context and cross the persistence actor.
#[derive(Debug, thiserror::Error)]
pub(crate) enum StoreError {
    #[error("sidecar changed externally; choose local edits or reload XMP before saving")]
    SidecarChanged,
    #[error("recovery changed externally; reload before writing edits")]
    RecoveryChanged,
    #[error("legacy sidecar lock retained; resolve it before saving")]
    LegacyLock,
    #[error("edit writer is busy; no overwrite performed")]
    Busy,
    #[error("{operation} failed for {path}: {source}")]
    Io {
        operation: &'static str,
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("retained recovery or temporary-file limit reached; inspect retained files before retrying")]
    RetentionLimit,
}
fn io_error(operation: &'static str, path: &Path, source: std::io::Error) -> StoreError {
    StoreError::Io {
        operation,
        path: path.to_owned(),
        source,
    }
}

struct Lock {
    // Keep the inode: unlinking it would let another writer lock a new inode
    // at the same path while this writer still owns the original lock.
    file: fs::File,
}
impl Drop for Lock {
    fn drop(&mut self) {
        // Closing our descriptor alone can retain the lock while a duplicate
        // (including a transient descriptor inherited across fork) stays alive.
        // The transaction ends with this guard, regardless of descriptor lifetime.
        if let Err(error) = self.file.unlock() {
            log::warn!("failed to release edit writer lock: {error}");
        }
    }
}
fn writer_lock(path: &Path) -> Result<Lock> {
    let mut name = path.as_os_str().to_os_string();
    name.push(".rrrah-lock-v2");
    let lock_path = PathBuf::from(name);
    if fs::symlink_metadata(&lock_path).is_ok_and(|m| !m.is_file()) {
        bail!("edit lock is not a regular file");
    }
    let file = fs::OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(&lock_path)
        .map_err(|e| io_error("open writer lock", &lock_path, e))?;
    file.try_lock().map_err(|e| match e {
        std::fs::TryLockError::WouldBlock => StoreError::Busy,
        std::fs::TryLockError::Error(error) => io_error("lock edit document", &lock_path, error),
    })?;
    Ok(Lock { file })
}
const MAX_DOCUMENT_BYTES: u64 = 256 * 1024;
const NS: &str = "https://rrrah.dev/ns/edit/1";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SourceVersion {
    len: u64,
    modified: u128,
    digest: [u8; 32],
}
fn stamp(path: &Path) -> Result<(u64, u128)> {
    let m = fs::symlink_metadata(path)?;
    if !m.is_file() {
        bail!("source is not a regular file");
    }
    Ok((
        m.len(),
        m.modified()?.duration_since(std::time::UNIX_EPOCH)?.as_nanos(),
    ))
}
pub fn source_version(path: &Path) -> Result<SourceVersion> {
    source_version_with_cancel(path, || false)
}
pub fn source_version_with_cancel(path: &Path, mut cancelled: impl FnMut() -> bool) -> Result<SourceVersion> {
    if cancelled() {
        bail!("source identity cancelled");
    }
    let before = stamp(path)?;
    let mut file = fs::File::open(path)?;
    let mut hash = blake3::Hasher::new();
    let mut buffer = [0u8; 16 * 1024];
    loop {
        if cancelled() {
            bail!("source identity cancelled");
        }
        let n = file.read(&mut buffer)?;
        if n == 0 {
            break;
        }
        hash.update(&buffer[..n]);
    }
    if stamp(path)? != before {
        bail!("source changed during identity check");
    }
    Ok(SourceVersion {
        len: before.0,
        modified: before.1,
        digest: *hash.finalize().as_bytes(),
    })
}
/// Cheap post-decode check. Full content identity is checked again before every write.
pub fn verify_source_stamp(path: &Path, version: &SourceVersion) -> Result<()> {
    if stamp(path)? != (version.len, version.modified) {
        bail!("source changed after decode admission; reload before editing");
    }
    Ok(())
}
pub fn verify_source(path: &Path, version: &SourceVersion) -> Result<()> {
    if stamp(path)? != (version.len, version.modified) {
        bail!("source changed; edits were not written");
    }
    if &source_version(path)? != version {
        bail!("source content changed; edits were not written");
    }
    Ok(())
}
fn bounded_read(path: &Path) -> Result<Option<Vec<u8>>> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if !metadata.is_file() => bail!("edit document is not a regular file"),
        Ok(_) => {}
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(io_error("inspect edit document", path, error).into()),
    }
    let file = match fs::File::open(path) {
        Ok(file) => file,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(io_error("open edit document", path, e).into()),
    };
    if file.metadata()?.len() > MAX_DOCUMENT_BYTES {
        bail!("edit document exceeds 256 KiB");
    }
    let mut bytes = Vec::new();
    file.take(MAX_DOCUMENT_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| io_error("read edit document", path, e))?;
    if bytes.len() as u64 > MAX_DOCUMENT_BYTES {
        bail!("edit document exceeds 256 KiB");
    }
    Ok(Some(bytes))
}
const MAX_RETAINED: usize = 32;
fn temporary_prefix(path: &Path) -> String {
    format!(
        ".rrrah-edit-{}-",
        &blake3::hash(path.as_os_str().as_encoded_bytes()).to_hex()[..16]
    )
}
// Called only inside this document's writer transaction. Never discard an orphan
// containing different (possibly newer) edits, a symlink, or an unknown file.
fn clean_duplicate_temporaries(path: &Path, prefix: &str) -> Result<()> {
    let committed = bounded_read(path)?;
    let parent = path.parent().context("edit document has no parent")?;
    let mut retained = 0;
    // Unrelated photos do not consume this document's retained-file quota.
    for entry in fs::read_dir(parent)? {
        let entry = entry?;
        if !entry.file_name().to_string_lossy().starts_with(prefix) {
            continue;
        }
        let temporary = entry.path();
        if fs::symlink_metadata(&temporary)?.is_file()
            && committed.is_some()
            && bounded_read(&temporary).ok().flatten().as_ref() == committed.as_ref()
        {
            fs::remove_file(&temporary)
                .map_err(|e| io_error("remove committed duplicate temporary", &temporary, e))?;
        } else {
            retained += 1;
            if retained >= MAX_RETAINED {
                return Err(StoreError::RetentionLimit.into());
            }
        }
    }
    Ok(())
}
fn check_backup_limit(path: &Path) -> Result<()> {
    let parent = path.parent().context("recovery parent missing")?;
    let prefix = format!(
        "{}.",
        path.file_name()
            .context("recovery filename missing")?
            .to_string_lossy()
    );
    let mut count = 0;
    // Unrelated photos do not consume this document's retained-file quota.
    for entry in fs::read_dir(parent)? {
        let name = entry?.file_name();
        let name = name.to_string_lossy();
        if name.starts_with(&prefix) && name.ends_with(".retained") {
            count += 1;
            if count >= MAX_RETAINED {
                return Err(StoreError::RetentionLimit.into());
            }
        }
    }
    Ok(())
}

fn atomic_write(path: &Path, bytes: &[u8]) -> Result<()> {
    if bytes.len() as u64 > MAX_DOCUMENT_BYTES {
        bail!("edit document exceeds 256 KiB");
    }
    if fs::symlink_metadata(path).is_ok_and(|m| m.file_type().is_symlink()) {
        bail!("symbolic edit documents are not replaced");
    }
    let parent = path.parent().context("edit document has no parent")?;
    let prefix = temporary_prefix(path);
    clean_duplicate_temporaries(path, &prefix)?;
    let mut temporary = tempfile::Builder::new()
        .prefix(&prefix)
        .tempfile_in(parent)
        .map_err(|e| io_error("create edit temporary", path, e))?;
    temporary
        .write_all(bytes)
        .map_err(|e| io_error("write edit temporary", path, e))?;
    temporary
        .as_file()
        .sync_all()
        .map_err(|e| io_error("sync edit temporary", path, e))?;
    #[cfg(test)]
    crash_test_checkpoint("temporary_synced");
    temporary
        .persist(path)
        .map_err(|e| io_error("replace edit document", path, e.error))?;
    #[cfg(test)]
    crash_test_checkpoint("persisted");
    #[cfg(unix)]
    fs::File::open(parent)?.sync_all()?;
    #[cfg(test)]
    crash_test_checkpoint("directory_synced");
    Ok(())
}
#[cfg(test)]
fn crash_test_checkpoint(stage: &str) {
    // These variables are set only on the isolated child test process.
    if std::env::var("RRRAH_TEST_EDIT_CRASH_STAGE").as_deref() == Ok(stage) {
        let marker = std::env::var_os("RRRAH_TEST_EDIT_CRASH_MARKER").unwrap();
        fs::write(marker, stage).unwrap();
        loop {
            std::thread::park();
        }
    }
}
#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct StoredEdit {
    schema: u32,
    page: usize,
    state: EditState,
}
fn parse_edit(text: &str, page: usize) -> Result<Option<EditState>> {
    let doc = roxmltree::Document::parse(text)?;
    let mut edits = doc.descendants().filter(|node| {
        node.has_tag_name((NS, "Edit"))
            && node.attribute("page").and_then(|s| s.parse::<usize>().ok()) == Some(page)
    });
    let Some(node) = edits.next() else {
        return Ok(None);
    };
    if edits.next().is_some() {
        bail!("duplicate rrrah edit record");
    }
    let edit: StoredEdit = serde_json::from_str(node.text().context("empty edit record")?)?;
    if edit.schema != EDIT_SCHEMA || edit.page != page {
        bail!("unsupported edit schema or page");
    }
    edit.state.validate()?;
    Ok(Some(edit.state))
}
pub fn read_sidecar(path: &Path, page: usize) -> Result<(Option<EditState>, Option<[u8; 32]>)> {
    let Some(bytes) = bounded_read(path)? else {
        return Ok((None, None));
    };
    let state = parse_edit(std::str::from_utf8(&bytes)?, page)?;
    Ok((state, Some(*blake3::hash(&bytes).as_bytes())))
}
pub fn write_sidecar(
    path: &Path,
    page: usize,
    state: EditState,
    expected: Option<[u8; 32]>,
) -> Result<[u8; 32]> {
    state.validate()?;
    // Cooperative writers serialize the read/compare/replace transaction. An
    // external editor that ignores this lock can still race the final rename.
    if fs::symlink_metadata(path.with_extension("xmp.rrrah-lock")).is_ok() {
        return Err(StoreError::LegacyLock.into());
    }
    let guard = writer_lock(path)?;
    let original = bounded_read(path)?;
    if original.as_ref().map(|b| *blake3::hash(b).as_bytes()) != expected {
        return Err(StoreError::SidecarChanged.into());
    }
    let text = original.as_ref().map(|b| std::str::from_utf8(b)).transpose()?.unwrap_or(
        "<x:xmpmeta xmlns:x=\"adobe:ns:meta/\"><rdf:RDF xmlns:rdf=\"http://www.w3.org/1999/02/22-rdf-syntax-ns#\"></rdf:RDF></x:xmpmeta>");
    let doc = roxmltree::Document::parse(text)?;
    let json = serde_json::to_string(&StoredEdit {
        schema: EDIT_SCHEMA,
        page,
        state,
    })?
    .replace('&', "&amp;")
    .replace('<', "&lt;")
    .replace('>', "&gt;");
    let replacement = format!("<rrrah:Edit xmlns:rrrah=\"{NS}\" page=\"{page}\">{json}</rrrah:Edit>");
    let existing: Vec<_> = doc
        .descendants()
        .filter(|n| {
            n.has_tag_name((NS, "Edit"))
                && n.attribute("page").and_then(|s| s.parse::<usize>().ok()) == Some(page)
        })
        .collect();
    if existing.len() > 1 {
        bail!("duplicate rrrah edit record");
    }
    let output = if let Some(node) = existing.first() {
        let mut output = text.to_owned();
        output.replace_range(node.range(), &replacement);
        output
    } else {
        let rdf = doc
            .descendants()
            .find(|n| n.has_tag_name(("http://www.w3.org/1999/02/22-rdf-syntax-ns#", "RDF")))
            .context("XMP has no RDF root")?;
        let range = rdf.range();
        let rdf_text = &text[range.clone()];
        let mut output = text.to_owned();
        let description = format!(
            "<rdf:Description xmlns:rdf=\"http://www.w3.org/1999/02/22-rdf-syntax-ns#\" rdf:about=\"\">{replacement}</rdf:Description>"
        );
        if rdf_text.ends_with("/>") {
            let tag = rdf_text[1..]
                .split(|c: char| c.is_whitespace() || c == '/' || c == '>')
                .next()
                .context("RDF tag is missing")?;
            output.replace_range(range.end - 2..range.end, &format!(">{description}</{tag}>"));
        } else {
            let at = range.start + rdf_text.rfind("</").context("RDF end tag missing")?;
            output.insert_str(at, &description);
        }
        output
    };
    parse_edit(&output, page)?.context("written edit record is missing")?;
    atomic_write(path, output.as_bytes())?;
    drop(guard);
    Ok(*blake3::hash(output.as_bytes()).as_bytes())
}
#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Recovery {
    schema: u32,
    page: usize,
    source: SourceVersion,
    state: EditState,
}
#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Envelope {
    payload: Recovery,
    checksum: [u8; 32],
}
pub fn journal_path(root: &Path, source: &Path, page: usize) -> PathBuf {
    let mut hash = blake3::Hasher::new();
    hash.update(source.as_os_str().as_encoded_bytes());
    hash.update(&(page as u64).to_le_bytes());
    root.join(format!("{}.json", hash.finalize()))
}
#[cfg(test)]
pub fn write_recovery(path: &Path, source: SourceVersion, page: usize, state: EditState) -> Result<()> {
    fs::create_dir_all(path.parent().context("journal parent missing")?)?;
    let _guard = writer_lock(path)?;
    write_recovery_unlocked(path, source, page, &state).map(|_| ())
}
#[cfg(test)]
fn write_recovery_unlocked(
    path: &Path,
    source: SourceVersion,
    page: usize,
    state: &EditState,
) -> Result<[u8; 32]> {
    let bytes = recovery_bytes(source, page, state)?;
    atomic_write(path, &bytes)?;
    Ok(*blake3::hash(&bytes).as_bytes())
}
pub fn write_recovery_checked(
    path: &Path,
    source: SourceVersion,
    page: usize,
    state: EditState,
    expected: Option<[u8; 32]>,
    own: Option<[u8; 32]>,
) -> Result<[u8; 32]> {
    fs::create_dir_all(path.parent().context("journal parent missing")?)?;
    let _guard = writer_lock(path)?;
    let actual = recovery_digest(path)?;
    if actual != expected && own.is_none_or(|digest| actual != Some(digest)) {
        return Err(StoreError::RecoveryChanged.into());
    }
    let mut journal = match bounded_read(path)? {
        Some(bytes) => match crate::edit_journal::decode(&bytes, &source, page)? {
            Some(journal) => journal,
            None => crate::edit_journal::Journal::new(
                source.clone(),
                page,
                parse_recovery(&bytes, &source, page)?,
            )?,
        },
        None => crate::edit_journal::Journal::new(source, page, EditState::default())?,
    };
    journal.append(state)?;
    let bytes = journal.encode()?;
    if bytes.len() as u64 > MAX_DOCUMENT_BYTES {
        bail!("recovery command log exceeds 256 KiB");
    }
    atomic_write(path, &bytes)?;
    Ok(*blake3::hash(&bytes).as_bytes())
}
fn recovery_bytes(source: SourceVersion, page: usize, state: &EditState) -> Result<Vec<u8>> {
    state.validate()?;
    let payload = Recovery {
        schema: EDIT_SCHEMA,
        page,
        source,
        state: *state,
    };
    let checksum = *blake3::hash(&serde_json::to_vec(&payload)?).as_bytes();
    Ok(serde_json::to_vec(&Envelope { payload, checksum })?)
}

pub enum RecoveryRead {
    Missing,
    Valid(EditState, [u8; 32]),
    Refused(anyhow::Error, [u8; 32]),
}
pub fn read_recovery_record(path: &Path, source: &SourceVersion, page: usize) -> Result<RecoveryRead> {
    let Some(bytes) = bounded_read(path)? else {
        return Ok(RecoveryRead::Missing);
    };
    let digest = *blake3::hash(&bytes).as_bytes();
    Ok(match parse_recovery(&bytes, source, page) {
        Ok(state) => RecoveryRead::Valid(state, digest),
        Err(error) => RecoveryRead::Refused(error, digest),
    })
}
#[cfg(test)]
pub fn read_recovery(path: &Path, source: &SourceVersion, page: usize) -> Result<Option<EditState>> {
    match read_recovery_record(path, source, page)? {
        RecoveryRead::Missing => Ok(None),
        RecoveryRead::Valid(state, _) => Ok(Some(state)),
        RecoveryRead::Refused(error, _) => Err(error),
    }
}
fn parse_recovery(bytes: &[u8], source: &SourceVersion, page: usize) -> Result<EditState> {
    if let Some(journal) = crate::edit_journal::decode(bytes, source, page)? {
        return journal.replay();
    }
    let envelope: Envelope = serde_json::from_slice(bytes)?;
    let payload = envelope.payload;
    if envelope.checksum != *blake3::hash(&serde_json::to_vec(&payload)?).as_bytes()
        || payload.schema != EDIT_SCHEMA
        || &payload.source != source
        || payload.page != page
    {
        bail!("recovery identity/version/checksum mismatch; journal retained");
    }
    payload.state.validate()?;
    Ok(payload.state)
}

pub fn recovery_digest(path: &Path) -> Result<Option<[u8; 32]>> {
    Ok(bounded_read(path)?.map(|bytes| *blake3::hash(&bytes).as_bytes()))
}
/// Explicitly start over only after durably retaining the exact refused bytes.
/// The backup is content-addressed and never overwrites an existing document.
#[cfg(test)]
pub fn restart_recovery(
    path: &Path,
    source_path: &Path,
    source: &SourceVersion,
    page: usize,
    expected: [u8; 32],
) -> Result<PathBuf> {
    restart_recovery_with_digest(path, source_path, source, page, expected).map(|(backup, _)| backup)
}
pub fn restart_recovery_with_digest(
    path: &Path,
    source_path: &Path,
    source: &SourceVersion,
    page: usize,
    expected: [u8; 32],
) -> Result<(PathBuf, [u8; 32])> {
    verify_source(source_path, source)?;
    let _guard = writer_lock(path)?;
    let bytes = bounded_read(path)?.context("refused recovery disappeared; reload before choosing")?;
    if *blake3::hash(&bytes).as_bytes() != expected {
        bail!("refused recovery changed; no replacement performed");
    }
    let backup = retain_bytes_unlocked(path, &bytes)?;
    #[cfg(test)]
    crash_test_checkpoint("backup_retained");
    verify_source(source_path, source)?;
    if recovery_digest(path)? != Some(expected) {
        bail!("recovery changed while retaining backup; no replacement performed");
    }
    let bytes = crate::edit_journal::Journal::new(source.clone(), page, EditState::default())?.encode()?;
    atomic_write(path, &bytes)?;
    let digest = *blake3::hash(&bytes).as_bytes();
    Ok((backup, digest))
}

/// Preserve an old-version draft without replacing the current journal or XMP.
pub fn retain_draft(path: &Path, source: SourceVersion, page: usize, state: EditState) -> Result<PathBuf> {
    fs::create_dir_all(path.parent().context("journal parent missing")?)?;
    let _guard = writer_lock(path)?;
    retain_bytes_unlocked(path, &recovery_bytes(source, page, &state)?)
}
fn retain_bytes_unlocked(path: &Path, bytes: &[u8]) -> Result<PathBuf> {
    let expected = *blake3::hash(bytes).as_bytes();
    let backup = path.with_extension(format!("json.{}.retained", blake3::Hash::from(expected).to_hex()));
    match bounded_read(&backup)? {
        Some(existing) if existing != bytes => bail!("recovery backup differs; no replacement performed"),
        Some(_) => {}
        None => {
            check_backup_limit(path)?;
            let parent = path.parent().context("recovery parent missing")?;
            let prefix = temporary_prefix(path);
            clean_duplicate_temporaries(path, &prefix)?;
            let mut temporary = tempfile::Builder::new().prefix(&prefix).tempfile_in(parent)?;
            temporary.write_all(bytes)?;
            temporary.as_file().sync_all()?;
            match temporary.persist_noclobber(&backup) {
                Ok(_) => {}
                Err(error) if error.error.kind() == std::io::ErrorKind::AlreadyExists => {
                    if bounded_read(&backup)?.as_deref() != Some(bytes) {
                        bail!("concurrent recovery backup differs; no replacement performed");
                    }
                }
                Err(error) => return Err(error.error.into()),
            }
        }
    }
    #[cfg(unix)]
    fs::File::open(path.parent().context("recovery parent missing")?)?.sync_all()?;
    Ok(backup)
}

#[cfg(test)]
mod tests {
    #[test]
    fn legacy_snapshot_migrates_only_on_checked_save_and_round_trips_command_log() {
        use super::*;
        let dir = tempfile::tempdir().unwrap();
        let image = dir.path().join("image.raw");
        fs::write(&image, b"original").unwrap();
        let source = source_version(&image).unwrap();
        let path = dir.path().join("edits.json");
        let mut state = EditState::default();
        state.exposure_stops = 1.;
        write_recovery(&path, source.clone(), 2, state).unwrap();
        let original = fs::read(&path).unwrap();
        assert_eq!(read_recovery(&path, &source, 2).unwrap(), Some(state));
        assert_eq!(fs::read(&path).unwrap(), original);
        let mut expected = recovery_digest(&path).unwrap();
        for i in 0..200 {
            state.exposure_stops = (i % 20) as f32 - 10.;
            state = state
                .with_curve(&[[0., 0.], [0.5, if i % 2 == 0 { 0.25 } else { 0.75 }], [1., 1.]])
                .unwrap();
            expected = Some(write_recovery_checked(&path, source.clone(), 2, state, expected, None).unwrap());
            assert_eq!(read_recovery(&path, &source, 2).unwrap(), Some(state));
        }
        let bytes = fs::read(&path).unwrap();
        assert!(bytes.len() < MAX_DOCUMENT_BYTES as usize);
        let value: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(value["payload"]["schema"], 2);
        assert!(value["payload"]["commands"].as_array().unwrap().len() <= crate::edit_journal::MAX_COMMANDS);
        assert!(value["payload"].get("state").is_none());
    }
    #[test]
    fn recovery_compare_exchange_preserves_external_bytes_and_allows_known_owned_revision() {
        use super::*;
        let dir = tempfile::tempdir().unwrap();
        let image = dir.path().join("source.raw");
        fs::write(&image, b"original").unwrap();
        let source = source_version(&image).unwrap();
        let path = dir.path().join("journal.json");
        let state = EditState::default();
        let first = write_recovery_checked(&path, source.clone(), 0, state, None, None).unwrap();
        let second = write_recovery_checked(&path, source.clone(), 0, state, None, Some(first)).unwrap();
        fs::write(&path, b"foreign unknown journal").unwrap();
        let error =
            write_recovery_checked(&path, source.clone(), 0, state, Some(first), Some(second)).unwrap_err();
        assert!(matches!(
            error.downcast_ref::<StoreError>(),
            Some(StoreError::RecoveryChanged)
        ));
        assert_eq!(fs::read(&path).unwrap(), b"foreign unknown journal");
        fs::remove_file(&path).unwrap();
        assert!(write_recovery_checked(&path, source, 0, state, Some(first), None).is_err());
        assert!(!path.exists());
    }
    #[test]
    fn source_hash_cancellation_does_not_open_obsolete_paths_or_publish_partial_identity() {
        use super::*;
        let dir = tempfile::tempdir().unwrap();
        let missing = dir.path().join("missing");
        assert!(
            source_version_with_cancel(&missing, || true)
                .unwrap_err()
                .to_string()
                .contains("cancelled")
        );
        let path = dir.path().join("large");
        fs::write(&path, [7u8; 65536]).unwrap();
        let mut checks = 0;
        assert!(
            source_version_with_cancel(&path, || {
                checks += 1;
                checks >= 3
            })
            .is_err()
        );
        assert_eq!(fs::metadata(&path).unwrap().len(), 65536);
    }
    #[test]
    fn large_photo_directory_does_not_consume_the_owned_retention_limit() {
        use super::*;
        let dir = tempfile::tempdir().unwrap();
        for i in 0..4100 {
            fs::write(dir.path().join(format!("photo-{i}.raw")), []).unwrap();
        }
        let path = dir.path().join("photo.xmp");
        write_sidecar(&path, 0, EditState::default(), None).unwrap();
        assert_eq!(read_sidecar(&path, 0).unwrap().0, Some(EditState::default()));
    }
    #[test]
    fn errors_keep_typed_conflict_and_io_categories() {
        use super::*;
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("image.xmp");
        write_sidecar(&path, 0, EditState::default(), None).unwrap();
        let error = write_sidecar(&path, 0, EditState::default(), None).unwrap_err();
        assert!(matches!(
            error.downcast_ref::<StoreError>(),
            Some(StoreError::SidecarChanged)
        ));
        let error = write_sidecar(
            &dir.path().join("missing/image.xmp"),
            0,
            EditState::default(),
            None,
        )
        .unwrap_err();
        assert!(
            matches!(error.downcast_ref::<StoreError>(), Some(StoreError::Io { source, .. }) if source.kind() == std::io::ErrorKind::NotFound)
        );
    }
    #[test]
    fn orphan_cleanup_removes_only_committed_duplicates_and_caps_unique_files() {
        use super::*;
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("edits.json");
        fs::write(&path, b"committed").unwrap();
        let prefix = temporary_prefix(&path);
        let duplicate = dir.path().join(format!("{prefix}duplicate"));
        let pending = dir.path().join(format!("{prefix}pending"));
        let unrelated = dir.path().join("other.tmp");
        fs::write(&duplicate, b"committed").unwrap();
        fs::write(&pending, b"newer unsaved edits").unwrap();
        fs::write(&unrelated, b"unrelated").unwrap();
        let _lock = writer_lock(&path).unwrap();
        atomic_write(&path, b"replacement").unwrap();
        assert!(!duplicate.exists());
        assert_eq!(fs::read(&pending).unwrap(), b"newer unsaved edits");
        assert_eq!(fs::read(&unrelated).unwrap(), b"unrelated");
        for i in 1..MAX_RETAINED {
            fs::write(dir.path().join(format!("{prefix}{i}")), b"unique orphan").unwrap();
        }
        let error = atomic_write(&path, b"must not replace").unwrap_err();
        assert!(matches!(
            error.downcast_ref::<StoreError>(),
            Some(StoreError::RetentionLimit)
        ));
        assert_eq!(fs::read(&path).unwrap(), b"replacement");
    }
    #[test]
    fn backup_limit_preserves_refused_journal_and_existing_backups() {
        use super::*;
        let dir = tempfile::tempdir().unwrap();
        let source = dir.path().join("image");
        fs::write(&source, b"source").unwrap();
        let path = dir.path().join("recovery.json");
        fs::write(&path, b"refused edits").unwrap();
        let version = source_version(&source).unwrap();
        for i in 0..MAX_RETAINED {
            fs::write(
                dir.path().join(format!("recovery.json.{i}.retained")),
                b"retained",
            )
            .unwrap();
        }
        let error = restart_recovery(
            &path,
            &source,
            &version,
            0,
            *blake3::hash(b"refused edits").as_bytes(),
        )
        .unwrap_err();
        assert!(matches!(
            error.downcast_ref::<StoreError>(),
            Some(StoreError::RetentionLimit)
        ));
        assert_eq!(fs::read(&path).unwrap(), b"refused edits");
        assert_eq!(
            fs::read(dir.path().join("recovery.json.0.retained")).unwrap(),
            b"retained"
        );
    }

    #[test]
    #[ignore = "subprocess helper; invoked by atomic_documents_survive_process_kill"]
    fn atomic_write_crash_helper() {
        let root = std::path::PathBuf::from(std::env::var_os("RRRAH_TEST_EDIT_CRASH_ROOT").unwrap());
        let source = root.join("source.raw");
        let mut state = super::EditState::default();
        state.exposure_stops = 1.5;
        let kind = std::env::var("RRRAH_TEST_EDIT_CRASH_KIND").unwrap();
        if kind == "restart" {
            let path = root.join("edits.json");
            super::restart_recovery(
                &path,
                &source,
                &super::source_version(&source).unwrap(),
                0,
                super::recovery_digest(&path).unwrap().unwrap(),
            )
            .unwrap();
        } else if matches!(kind.as_str(), "journal" | "commands" | "compaction") {
            super::write_recovery_checked(
                &root.join("edits.json"),
                super::source_version(&source).unwrap(),
                0,
                state,
                super::recovery_digest(&root.join("edits.json")).unwrap(),
                None,
            )
            .unwrap();
        } else {
            let path = root.join("edits.xmp");
            let (_, digest) = super::read_sidecar(&path, 0).unwrap();
            super::write_sidecar(&path, 0, state, digest).unwrap();
        }
        panic!("child did not stop at its requested checkpoint");
    }
    #[test]
    fn atomic_documents_survive_process_kill() {
        use std::{
            process::{Command, Stdio},
            time::{Duration, Instant},
        };
        for kind in ["journal", "commands", "compaction", "xmp"] {
            for stage in ["temporary_synced", "persisted", "directory_synced"] {
                let dir = tempfile::tempdir().unwrap();
                let source = dir.path().join("source.raw");
                std::fs::write(&source, b"synthetic").unwrap();
                let version = super::source_version(&source).unwrap();
                let old = super::EditState::default();
                let journal = dir.path().join("edits.json");
                let xmp = dir.path().join("edits.xmp");
                super::write_recovery(&journal, version.clone(), 0, old).unwrap();
                if matches!(kind, "commands" | "compaction") {
                    let count = if kind == "compaction" {
                        crate::edit_journal::MAX_COMMANDS
                    } else {
                        4
                    };
                    let mut digest = super::recovery_digest(&journal).unwrap();
                    for i in 0..count {
                        let mut state = old;
                        state.exposure_stops = if i % 2 == 0 { 0.5 } else { 0. };
                        digest = Some(
                            super::write_recovery_checked(&journal, version.clone(), 0, state, digest, None)
                                .unwrap(),
                        );
                    }
                }
                super::write_sidecar(&xmp, 0, old, None).unwrap();
                let marker = dir.path().join("checkpoint");
                let mut child = Command::new(std::env::current_exe().unwrap())
                    .args([
                        "--exact",
                        "edit_store::tests::atomic_write_crash_helper",
                        "--ignored",
                    ])
                    .env("RRRAH_TEST_EDIT_CRASH_ROOT", dir.path())
                    .env("RRRAH_TEST_EDIT_CRASH_KIND", kind)
                    .env("RRRAH_TEST_EDIT_CRASH_STAGE", stage)
                    .env("RRRAH_TEST_EDIT_CRASH_MARKER", &marker)
                    .stdout(Stdio::null())
                    .stderr(Stdio::inherit())
                    .spawn()
                    .unwrap();
                let deadline = Instant::now() + Duration::from_secs(10);
                while !marker.exists() && Instant::now() < deadline {
                    if child.try_wait().unwrap().is_some() {
                        break;
                    }
                    std::thread::sleep(Duration::from_millis(5));
                }
                let reached = marker.exists();
                let competing_write = if kind != "xmp" {
                    super::write_recovery(&journal, version.clone(), 0, old)
                } else {
                    let digest = super::recovery_digest(&xmp).unwrap();
                    super::write_sidecar(&xmp, 0, old, digest).map(|_| ())
                };
                let killed = child.kill();
                let status = child.wait().unwrap();
                assert!(reached, "child missed {kind}/{stage}: {status}");
                killed.unwrap();
                assert!(!status.success());
                assert!(
                    competing_write.is_err(),
                    "active writer lock was bypassed: {kind}/{stage}"
                );
                let state = if kind != "xmp" {
                    super::read_recovery(&journal, &version, 0).unwrap().unwrap()
                } else {
                    super::read_sidecar(&xmp, 0).unwrap().0.unwrap()
                };
                let expected: f32 = if stage == "temporary_synced" { 0. } else { 1.5 };
                assert_eq!(
                    state.exposure_stops.to_bits(),
                    expected.to_bits(),
                    "{kind}/{stage}"
                );
                // Death releases the OS lock; its persistent inode must not
                // prevent a subsequent save of the recovered document.
                if kind != "xmp" {
                    super::write_recovery(&journal, version.clone(), 0, state).unwrap();
                } else {
                    let (_, digest) = super::read_sidecar(&xmp, 0).unwrap();
                    super::write_sidecar(&xmp, 0, state, digest).unwrap();
                }
            }
        }
    }
    use super::*;
    #[test]
    fn restart_preserves_exact_backup_and_reuses_it_without_overwriting() {
        let dir = tempfile::tempdir().unwrap();
        let source = dir.path().join("source.raw");
        fs::write(&source, b"synthetic").unwrap();
        let version = source_version(&source).unwrap();
        let path = dir.path().join("edits.json");
        let original = b"unsupported original recovery bytes";
        fs::write(&path, original).unwrap();
        let digest = recovery_digest(&path).unwrap().unwrap();
        let backup = restart_recovery(&path, &source, &version, 0, digest).unwrap();
        assert_eq!(fs::read(&backup).unwrap(), original);
        assert_eq!(
            read_recovery(&path, &version, 0).unwrap(),
            Some(EditState::default())
        );
        fs::write(&path, original).unwrap();
        assert_eq!(
            restart_recovery(&path, &source, &version, 0, digest).unwrap(),
            backup
        );
        assert_eq!(fs::read(&backup).unwrap(), original);
    }
    #[test]
    fn restart_refuses_changed_journal_source_and_backup_collision() {
        let dir = tempfile::tempdir().unwrap();
        let source = dir.path().join("source.raw");
        fs::write(&source, b"synthetic").unwrap();
        let version = source_version(&source).unwrap();
        let path = dir.path().join("edits.json");
        fs::write(&path, b"original refused recovery").unwrap();
        let digest = recovery_digest(&path).unwrap().unwrap();
        fs::write(&path, b"external recovery edit").unwrap();
        assert!(restart_recovery(&path, &source, &version, 0, digest).is_err());
        assert_eq!(fs::read(&path).unwrap(), b"external recovery edit");
        let current = recovery_digest(&path).unwrap().unwrap();
        let backup = path.with_extension(format!("json.{}.retained", blake3::Hash::from(current).to_hex()));
        fs::write(&backup, b"foreign backup").unwrap();
        assert!(restart_recovery(&path, &source, &version, 0, current).is_err());
        assert_eq!(fs::read(&backup).unwrap(), b"foreign backup");
        assert_eq!(fs::read(&path).unwrap(), b"external recovery edit");
        fs::remove_file(&backup).unwrap();
        fs::write(&source, b"source changed after choosing").unwrap();
        assert!(restart_recovery(&path, &source, &version, 0, current).is_err());
        assert!(!backup.exists());
        assert_eq!(fs::read(&path).unwrap(), b"external recovery edit");
    }
    #[test]
    fn restart_process_kill_keeps_both_refused_record_and_retained_backup() {
        use std::{
            process::{Command, Stdio},
            time::{Duration, Instant},
        };
        let dir = tempfile::tempdir().unwrap();
        let source = dir.path().join("source.raw");
        fs::write(&source, b"synthetic").unwrap();
        let path = dir.path().join("edits.json");
        let bytes = b"refused original recovery";
        fs::write(&path, bytes).unwrap();
        let marker = dir.path().join("checkpoint");
        let mut child = Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "edit_store::tests::atomic_write_crash_helper",
                "--ignored",
            ])
            .env("RRRAH_TEST_EDIT_CRASH_ROOT", dir.path())
            .env("RRRAH_TEST_EDIT_CRASH_KIND", "restart")
            .env("RRRAH_TEST_EDIT_CRASH_STAGE", "backup_retained")
            .env("RRRAH_TEST_EDIT_CRASH_MARKER", &marker)
            .stdout(Stdio::null())
            .stderr(Stdio::inherit())
            .spawn()
            .unwrap();
        let deadline = Instant::now() + Duration::from_secs(10);
        while !marker.exists() && Instant::now() < deadline {
            if child.try_wait().unwrap().is_some() {
                break;
            }
            std::thread::sleep(Duration::from_millis(5));
        }
        let reached = marker.exists();
        let killed = child.kill();
        let status = child.wait().unwrap();
        assert!(reached, "child did not retain backup: {status}");
        killed.unwrap();
        assert!(!status.success());
        assert_eq!(fs::read(&path).unwrap(), bytes);
        let backups: Vec<_> = fs::read_dir(dir.path())
            .unwrap()
            .flatten()
            .filter(|e| e.path().extension().is_some_and(|e| e == "retained"))
            .collect();
        assert_eq!(backups.len(), 1);
        assert_eq!(fs::read(backups[0].path()).unwrap(), bytes);
        // Retrying after a crash reuses the durable backup and releases the
        // recovered writer lock automatically; it cannot duplicate the backup.
        assert_eq!(
            restart_recovery(
                &path,
                &source,
                &source_version(&source).unwrap(),
                0,
                recovery_digest(&path).unwrap().unwrap()
            )
            .unwrap(),
            backups[0].path()
        );
    }
    #[test]
    fn completed_writer_releases_lock_with_duplicated_descriptor_alive() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("edit.json");
        let guard = writer_lock(&path).unwrap();
        let duplicate = guard.file.try_clone().unwrap();
        assert!(writer_lock(&path).is_err());
        drop(guard);
        let next = writer_lock(&path)
            .expect("completed transaction must release lock independently of duplicated descriptors");
        drop(next);
        drop(duplicate);
        assert!(writer_lock(&path).is_ok());
    }

    #[test]
    fn concurrent_writers_are_refused_and_persistent_lock_is_reusable() {
        let dir = tempfile::tempdir().unwrap();
        let source = dir.path().join("source.raw");
        fs::write(&source, b"synthetic").unwrap();
        let version = source_version(&source).unwrap();
        let state = EditState::default();
        for sidecar in [false, true] {
            let path = dir.path().join(if sidecar { "edit.xmp" } else { "edit.json" });
            let digest = if sidecar {
                Some(write_sidecar(&path, 0, state, None).unwrap())
            } else {
                write_recovery(&path, version.clone(), 0, state).unwrap();
                None
            };
            let original = fs::read(&path).unwrap();
            let guard = writer_lock(&path).unwrap();
            let result = if sidecar {
                write_sidecar(&path, 0, state, digest).map(|_| ())
            } else {
                write_recovery(&path, version.clone(), 0, state)
            };
            assert!(result.is_err());
            assert_eq!(fs::read(&path).unwrap(), original);
            drop(guard);
            let mut lock_name = path.as_os_str().to_os_string();
            lock_name.push(".rrrah-lock-v2");
            assert_eq!(fs::metadata(lock_name).unwrap().len(), 0);
            assert!(writer_lock(&path).is_ok());
        }
    }
    #[test]
    fn legacy_lock_is_never_stolen_and_non_regular_v2_locks_are_refused() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("edit.xmp");
        let legacy = path.with_extension("xmp.rrrah-lock");
        fs::write(&legacy, b"possibly active old writer").unwrap();
        assert!(write_sidecar(&path, 0, EditState::default(), None).is_err());
        assert_eq!(fs::read(legacy).unwrap(), b"possibly active old writer");
        let journal = dir.path().join("edit.json");
        fs::create_dir(dir.path().join("edit.json.rrrah-lock-v2")).unwrap();
        assert!(writer_lock(&journal).is_err());
        assert!(!journal.exists());
    }
    #[test]
    fn curve_values_round_trip_exactly_before_recovery_checksum_verification() {
        let mut seed = 0x1234_5678_9abc_def0u64;
        for _ in 0..1024 {
            seed = seed.wrapping_mul(6_364_136_223_846_793_005).wrapping_add(1);
            let x = f64::from_bits(0x3fe0_0000_0000_0000 | (seed & 0x000f_ffff_ffff_ffff));
            if 1. - x < 1e-6 {
                continue;
            }
            let state = EditState::default()
                .with_curve(&[[0., 0.], [x, x], [1., 1.]])
                .unwrap();
            let encoded = serde_json::to_vec(&state).unwrap();
            let restored: EditState = serde_json::from_slice(&encoded).unwrap();
            assert_eq!(
                restored.points()[1][0].to_bits(),
                x.to_bits(),
                "{x} must survive checksummed recovery"
            );
            assert_eq!(serde_json::to_vec(&restored).unwrap(), encoded);
        }
        // The first pre-fix failing value must also survive both persistence formats.
        let x = f64::from_bits(4_607_175_173_822_920_608);
        let state = EditState::default()
            .with_curve(&[[0., 0.], [x, x], [1., 1.]])
            .unwrap();
        let dir = tempfile::tempdir().unwrap();
        let source = dir.path().join("image.raw");
        fs::write(&source, b"synthetic source").unwrap();
        let version = source_version(&source).unwrap();
        let journal = journal_path(dir.path(), &source, 0);
        write_recovery(&journal, version.clone(), 0, state).unwrap();
        assert_eq!(read_recovery(&journal, &version, 0).unwrap(), Some(state));
        let sidecar = source.with_extension("xmp");
        write_sidecar(&sidecar, 0, state, None).unwrap();
        assert_eq!(read_sidecar(&sidecar, 0).unwrap().0, Some(state));
    }
    #[test]
    fn oversized_and_non_regular_sidecars_are_refused() {
        let dir = tempfile::tempdir().unwrap();
        assert!(read_sidecar(dir.path(), 0).is_err());
        let path = dir.path().join("image.xmp");
        fs::write(
            &path,
            vec![b' '; usize::try_from(MAX_DOCUMENT_BYTES).unwrap() + 1],
        )
        .unwrap();
        assert!(read_sidecar(&path, 0).is_err());
        #[cfg(unix)]
        {
            let link = dir.path().join("linked.xmp");
            std::os::unix::fs::symlink(&path, &link).unwrap();
            assert!(read_sidecar(&link, 0).is_err());
        }
    }
    #[test]
    fn source_content_change_with_same_length_and_mtime_is_refused() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("source");
        fs::write(&path, b"original").unwrap();
        let modified = fs::metadata(&path).unwrap().modified().unwrap();
        let version = source_version(&path).unwrap();
        verify_source(&path, &version).unwrap();
        fs::write(&path, b"modified").unwrap();
        fs::OpenOptions::new()
            .write(true)
            .open(&path)
            .unwrap()
            .set_times(fs::FileTimes::new().set_modified(modified))
            .unwrap();
        assert_eq!(stamp(&path).unwrap(), (version.len, version.modified));
        assert!(verify_source(&path, &version).is_err());
    }
    #[test]
    fn xmp_preserves_unknown_fields_and_pages_and_refuses_external_change() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("image.xmp");
        let xml = "<r:RDF xmlns:r=\"http://www.w3.org/1999/02/22-rdf-syntax-ns#\"><r:Description xmlns:custom=\"urn:test\" custom:keep=\"yes\"><custom:Data>untouched &amp; preserved</custom:Data></r:Description></r:RDF>";
        fs::write(&path, xml).unwrap();
        let (_, digest) = read_sidecar(&path, 0).unwrap();
        let mut first = EditState::default();
        first.exposure_stops = 1.5;
        let digest = write_sidecar(&path, 0, first, digest).unwrap();
        write_sidecar(&path, 1, EditState::default(), Some(digest)).unwrap();
        assert_eq!(read_sidecar(&path, 0).unwrap().0.unwrap(), first);
        assert!(fs::read_to_string(&path).unwrap().contains("custom:keep=\"yes\""));
        assert!(write_sidecar(&path, 0, first, Some(digest)).is_err());
        fs::write(&path, "broken XML").unwrap();
        assert!(read_sidecar(&path, 0).is_err());
    }
    #[test]
    fn recovery_refuses_corruption_new_source_and_unknown_version_without_deleting() {
        let dir = tempfile::tempdir().unwrap();
        let source = dir.path().join("image");
        fs::write(&source, b"original").unwrap();
        let version = source_version(&source).unwrap();
        let path = journal_path(dir.path(), &source, 0);
        write_recovery(&path, version.clone(), 0, EditState::default()).unwrap();
        assert!(read_recovery(&path, &version, 0).unwrap().is_some());
        fs::write(&source, b"different").unwrap();
        assert!(read_recovery(&path, &source_version(&source).unwrap(), 0).is_err());
        let mut future: Envelope = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
        future.payload.schema = EDIT_SCHEMA + 1;
        future.checksum = *blake3::hash(&serde_json::to_vec(&future.payload).unwrap()).as_bytes();
        fs::write(&path, serde_json::to_vec(&future).unwrap()).unwrap();
        assert!(read_recovery(&path, &version, 0).is_err());
        write_recovery(&path, version.clone(), 0, EditState::default()).unwrap();
        let mut bytes = fs::read(&path).unwrap();
        bytes.truncate(bytes.len() / 2);
        fs::write(&path, bytes).unwrap();
        assert!(read_recovery(&path, &version, 0).is_err());
        assert!(path.exists());
    }
}
