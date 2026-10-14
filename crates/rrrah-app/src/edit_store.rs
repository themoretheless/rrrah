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
    path: PathBuf,
    file: Option<fs::File>,
}
impl Drop for Lock {
    fn drop(&mut self) {
        drop(self.file.take());
        let _ = fs::remove_file(&self.path);
    }
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
    temporary.persist(path).map_err(|e| e.error)?;
    #[cfg(unix)]
    fs::File::open(parent)?.sync_all()?;
    Ok(())
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
    let lock_path = path.with_extension("xmp.rrrah-lock");
    let lock = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&lock_path)
        .context("sidecar writer is busy; no overwrite performed")?;
    let guard = Lock {
        path: lock_path,
        file: Some(lock),
    };
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
    state.validate()?;
    let payload = Recovery {
        schema: EDIT_SCHEMA,
        page,
        source,
        state,
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

#[cfg(test)]
mod tests {
    use super::*;
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
        fs::write(&path, vec![b' '; MAX_DOCUMENT_BYTES as usize + 1]).unwrap();
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
