//! Bounded, atomic sidecars and checksummed edit recovery snapshots.
use anyhow::{Context, Result, bail};
use rrrah_core::edit::{EDIT_SCHEMA, EditState};
use serde::{Deserialize, Serialize};
use std::{
    fs,
    io::{Read, Write},
    path::{Path, PathBuf},
};

struct Lock {
    // Keep the inode: unlinking it would let another writer lock a new inode
    // at the same path while this writer still owns the original lock.
    _file: fs::File,
}
impl Drop for Lock {
    fn drop(&mut self) {
        // Closing our descriptor alone can retain the lock while a duplicate
        // (including a transient descriptor inherited across fork) stays alive.
        // The transaction ends with this guard, regardless of descriptor lifetime.
        if let Err(error) = self._file.unlock() {
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
        .open(lock_path)?;
    file.try_lock()
        .context("edit writer is busy; no overwrite performed")?;
    Ok(Lock { _file: file })
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
    let before = stamp(path)?;
    let mut file = fs::File::open(path)?;
    let mut hash = blake3::Hasher::new();
    let mut buffer = [0u8; 16 * 1024];
    loop {
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
        Err(error) => return Err(error.into()),
    }
    let file = match fs::File::open(path) {
        Ok(file) => file,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(e.into()),
    };
    if file.metadata()?.len() > MAX_DOCUMENT_BYTES {
        bail!("edit document exceeds 256 KiB");
    }
    let mut bytes = Vec::new();
    file.take(MAX_DOCUMENT_BYTES + 1).read_to_end(&mut bytes)?;
    if bytes.len() as u64 > MAX_DOCUMENT_BYTES {
        bail!("edit document exceeds 256 KiB");
    }
    Ok(Some(bytes))
}
fn atomic_write(path: &Path, bytes: &[u8]) -> Result<()> {
    if bytes.len() as u64 > MAX_DOCUMENT_BYTES {
        bail!("edit document exceeds 256 KiB");
    }
    if fs::symlink_metadata(path).is_ok_and(|m| m.file_type().is_symlink()) {
        bail!("symbolic edit documents are not replaced");
    }
    let parent = path.parent().context("edit document has no parent")?;
    let mut temporary = tempfile::NamedTempFile::new_in(parent)?;
    temporary.write_all(bytes)?;
    temporary.as_file().sync_all()?;
    #[cfg(test)]
    crash_test_checkpoint("temporary_synced");
    temporary.persist(path).map_err(|e| e.error)?;
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
        bail!("legacy sidecar lock retained; resolve it before saving");
    }
    let guard = writer_lock(path)?;
    let original = bounded_read(path)?;
    if original.as_ref().map(|b| *blake3::hash(b).as_bytes()) != expected {
        bail!("sidecar changed externally; reload before saving");
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
pub fn write_recovery(path: &Path, source: SourceVersion, page: usize, state: EditState) -> Result<()> {
    fs::create_dir_all(path.parent().context("journal parent missing")?)?;
    let _guard = writer_lock(path)?;
    write_recovery_unlocked(path, source, page, &state)
}
fn write_recovery_unlocked(path: &Path, source: SourceVersion, page: usize, state: &EditState) -> Result<()> {
    state.validate()?;
    let payload = Recovery {
        schema: EDIT_SCHEMA,
        page,
        source,
        state: *state,
    };
    let checksum = *blake3::hash(&serde_json::to_vec(&payload)?).as_bytes();
    fs::create_dir_all(path.parent().context("journal parent missing")?)?;
    atomic_write(path, &serde_json::to_vec(&Envelope { payload, checksum })?)
}
pub fn read_recovery(path: &Path, source: &SourceVersion, page: usize) -> Result<Option<EditState>> {
    let Some(bytes) = bounded_read(path)? else {
        return Ok(None);
    };
    let envelope: Envelope = serde_json::from_slice(&bytes)?;
    let payload = envelope.payload;
    if envelope.checksum != *blake3::hash(&serde_json::to_vec(&payload)?).as_bytes()
        || payload.schema != EDIT_SCHEMA
        || &payload.source != source
        || payload.page != page
    {
        bail!("recovery identity/version/checksum mismatch; journal retained");
    }
    payload.state.validate()?;
    Ok(Some(payload.state))
}
pub fn recovery_digest(path: &Path) -> Result<Option<[u8; 32]>> {
    Ok(bounded_read(path)?.map(|bytes| *blake3::hash(&bytes).as_bytes()))
}
/// Explicitly start over only after durably retaining the exact refused bytes.
/// The backup is content-addressed and never overwrites an existing document.
pub fn restart_recovery(
    path: &Path,
    source_path: &Path,
    source: &SourceVersion,
    page: usize,
    expected: [u8; 32],
) -> Result<PathBuf> {
    verify_source(source_path, source)?;
    let _guard = writer_lock(path)?;
    let bytes = bounded_read(path)?.context("refused recovery disappeared; reload before choosing")?;
    if *blake3::hash(&bytes).as_bytes() != expected {
        bail!("refused recovery changed; no replacement performed");
    }
    let backup = path.with_extension(format!("json.{}.retained", blake3::Hash::from(expected).to_hex()));
    match bounded_read(&backup)? {
        Some(existing) if existing != bytes => bail!("recovery backup differs; no replacement performed"),
        Some(_) => {}
        None => {
            let parent = path.parent().context("recovery parent missing")?;
            let mut temporary = tempfile::NamedTempFile::new_in(parent)?;
            temporary.write_all(&bytes)?;
            temporary.as_file().sync_all()?;
            match temporary.persist_noclobber(&backup) {
                Ok(_) => {}
                Err(error) if error.error.kind() == std::io::ErrorKind::AlreadyExists => {
                    if bounded_read(&backup)?.as_deref() != Some(bytes.as_slice()) {
                        bail!("concurrent recovery backup differs; no replacement performed");
                    }
                }
                Err(error) => return Err(error.error.into()),
            }
        }
    }
    #[cfg(unix)]
    fs::File::open(path.parent().context("recovery parent missing")?)?.sync_all()?;
    #[cfg(test)]
    crash_test_checkpoint("backup_retained");
    verify_source(source_path, source)?;
    if recovery_digest(path)? != Some(expected) {
        bail!("recovery changed while retaining backup; no replacement performed");
    }
    write_recovery_unlocked(path, source.clone(), page, &EditState::default())?;
    Ok(backup)
}

#[cfg(test)]
mod tests {
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
        } else if kind == "journal" {
            super::write_recovery(
                &root.join("edits.json"),
                super::source_version(&source).unwrap(),
                0,
                state,
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
        for kind in ["journal", "xmp"] {
            for stage in ["temporary_synced", "persisted", "directory_synced"] {
                let dir = tempfile::tempdir().unwrap();
                let source = dir.path().join("source.raw");
                std::fs::write(&source, b"synthetic").unwrap();
                let version = super::source_version(&source).unwrap();
                let old = super::EditState::default();
                let journal = dir.path().join("edits.json");
                let xmp = dir.path().join("edits.xmp");
                super::write_recovery(&journal, version.clone(), 0, old).unwrap();
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
                let competing_write = if kind == "journal" {
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
                let state = if kind == "journal" {
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
                if kind == "journal" {
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
        let duplicate = guard._file.try_clone().unwrap();
        assert!(writer_lock(&path).is_err());
        drop(guard);
        let next = writer_lock(&path).expect("completed transaction must release lock independently of duplicated descriptors");
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
