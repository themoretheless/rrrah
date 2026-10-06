//! Bounded exact-file discovery. Hash equality is always byte-confirmed.
//!
//! Results describe stable observations, not an atomic filesystem snapshot.
//! No files are removed. Identity is device/inode on Unix and volume/file index on Windows.
//! Other targets use canonical paths.

use std::{
    collections::{BTreeMap, BTreeSet},
    fs::{self, File, Metadata, OpenOptions},
    io::{self, Read, Seek, SeekFrom},
    path::{Path, PathBuf},
    time::SystemTime,
};

const BLOCK: usize = 64 * 1024;
const SAMPLE: usize = 4096;

/// Resource and traversal policy. Limit omissions appear in diagnostics.
#[derive(Debug, Clone)]
pub struct Options {
    pub follow_symlinks: bool,
    pub max_depth: usize,
    pub max_entries: usize,
    pub max_file_bytes: u64,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            follow_symlinks: false,
            max_depth: 256,
            max_entries: 1_000_000,
            max_file_bytes: u64::MAX,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IssueKind {
    Io(String),
    Changed,
    Limit,
    SymlinkSkipped,
    NotRegular,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Issue {
    pub path: PathBuf,
    pub kind: IssueKind,
}

/// Distinct paths that reference one physical file; not redundant file copies.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Aliases {
    pub paths: Vec<PathBuf>,
}

/// Distinct physical files confirmed equal by full byte comparison.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Group {
    pub bytes: u64,
    pub digest: [u8; 32],
    pub paths: Vec<PathBuf>,
}

#[derive(Debug, Default)]
pub struct Report {
    pub groups: Vec<Group>,
    pub aliases: Vec<Aliases>,
    pub issues: Vec<Issue>,
    pub cancelled: bool,
    pub files_observed: usize,
    /// Actual bytes read in sampling, hashing and verification.
    pub bytes_read: u64,
}

impl Report {
    pub fn complete(&self) -> bool {
        !self.cancelled
            && !self.issues.iter().any(|issue| {
                matches!(
                    issue.kind,
                    IssueKind::Io(_) | IssueKind::Changed | IssueKind::Limit
                )
            })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
enum Identity {
    #[cfg(unix)]
    Unix(u64, u64),
    #[cfg(windows)]
    Windows(u64, u64),
    #[cfg(not(any(unix, windows)))]
    Canonical(PathBuf),
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Stamp {
    identity: Identity,
    bytes: u64,
    modified: Option<SystemTime>,
    #[cfg(unix)]
    changed: (i64, i64),
}

impl Stamp {
    // Windows handle acquisition is fallible; keep one signature across targets.
    #[cfg_attr(not(windows), allow(clippy::unnecessary_wraps))]
    fn new(metadata: &Metadata, canonical: &Path) -> io::Result<Self> {
        #[cfg(windows)]
        {
            use std::os::windows::fs::OpenOptionsExt;
            let _ = metadata;
            // FILE_FLAG_BACKUP_SEMANTICS also permits directory handles.
            let file = OpenOptions::new()
                .read(true)
                .custom_flags(0x0200_0000)
                .open(canonical)?;
            Self::from_file(&file, canonical)
        }
        #[cfg(not(windows))]
        {
            #[cfg(unix)]
            use std::os::unix::fs::MetadataExt;
            let _ = canonical;
            Ok(Self {
                #[cfg(unix)]
                identity: Identity::Unix(metadata.dev(), metadata.ino()),
                #[cfg(not(any(unix, windows)))]
                identity: Identity::Canonical(canonical.to_path_buf()),
                bytes: metadata.len(),
                modified: metadata.modified().ok(),
                #[cfg(unix)]
                changed: (metadata.ctime(), metadata.ctime_nsec()),
            })
        }
    }

    fn from_file(file: &File, canonical: &Path) -> io::Result<Self> {
        let metadata = file.metadata()?;
        #[cfg(windows)]
        {
            let _ = canonical;
            let information = winapi_util::file::information(file)?;
            Ok(Self {
                identity: Identity::Windows(information.volume_serial_number(), information.file_index()),
                bytes: metadata.len(),
                modified: metadata.modified().ok(),
            })
        }
        #[cfg(not(windows))]
        Self::new(&metadata, canonical)
    }
}

#[derive(Debug)]
struct Entry {
    path: PathBuf,
    canonical: PathBuf,
    stamp: Stamp,
    aliases: BTreeSet<PathBuf>,
}

#[derive(Debug)]
enum Failure {
    Io(io::Error),
    Changed,
    Cancelled,
}

impl From<io::Error> for Failure {
    fn from(value: io::Error) -> Self {
        Self::Io(value)
    }
}

fn check(cancel: &impl Fn() -> bool) -> Result<(), Failure> {
    if cancel() { Err(Failure::Cancelled) } else { Ok(()) }
}

fn open(entry: &Entry) -> Result<File, Failure> {
    let mut options = OpenOptions::new();
    options.read(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        // Avoid blocking on a FIFO swapped into a formerly regular path.
        options.custom_flags(libc::O_NONBLOCK | libc::O_NOFOLLOW);
    }
    let file = options.open(&entry.canonical)?;
    validate(entry, &file)?;
    Ok(file)
}

fn validate(entry: &Entry, file: &File) -> Result<(), Failure> {
    let metadata = file.metadata()?;
    let path_metadata = fs::metadata(&entry.path)?;
    if !metadata.is_file()
        || Stamp::from_file(file, &entry.canonical)? != entry.stamp
        || Stamp::new(&path_metadata, &entry.path)? != entry.stamp
        || fs::canonicalize(&entry.path)? != entry.canonical
    {
        return Err(Failure::Changed);
    }
    Ok(())
}

fn read_block(file: &mut File, buffer: &mut [u8]) -> Result<usize, Failure> {
    loop {
        match file.read(buffer) {
            Err(error) if error.kind() == io::ErrorKind::Interrupted => {}
            value => return value.map_err(Failure::Io),
        }
    }
}

fn allocate_read_buffer(length: usize) -> Result<Vec<u8>, io::Error> {
    let mut bytes = Vec::new();
    bytes
        .try_reserve_exact(length)
        .map_err(|_| io::Error::from(io::ErrorKind::OutOfMemory))?;
    bytes.resize(length, 0);
    Ok(bytes)
}

fn hash(
    entry: &Entry,
    sampled: bool,
    cancel: &impl Fn() -> bool,
    bytes_read: &mut u64,
) -> Result<[u8; 32], Failure> {
    check(cancel)?;
    let mut file = open(entry)?;
    let mut hasher = blake3::Hasher::new();
    let mut buffer = allocate_read_buffer(BLOCK)?;
    if sampled {
        for offset in [0, entry.stamp.bytes.saturating_sub(SAMPLE as u64)] {
            check(cancel)?;
            file.seek(SeekFrom::Start(offset))?;
            let length = (entry.stamp.bytes - offset).min(SAMPLE as u64);
            let length = usize::try_from(length).expect("sample size fits usize");
            file.read_exact(&mut buffer[..length])?;
            *bytes_read += length as u64;
            hasher.update(&buffer[..length]);
        }
    } else {
        let mut total = 0_u64;
        loop {
            check(cancel)?;
            let count = read_block(&mut file, &mut buffer)?;
            if count == 0 {
                break;
            }
            total += count as u64;
            *bytes_read += count as u64;
            if total > entry.stamp.bytes {
                return Err(Failure::Changed);
            }
            hasher.update(&buffer[..count]);
        }
        if total != entry.stamp.bytes {
            return Err(Failure::Changed);
        }
    }
    check(cancel)?;
    validate(entry, &file)?;
    Ok(*hasher.finalize().as_bytes())
}

fn equal(
    left: &Entry,
    right: &Entry,
    cancel: &impl Fn() -> bool,
    bytes_read: &mut u64,
) -> Result<bool, (bool, Failure)> {
    check(cancel).map_err(|failure| (false, failure))?;
    let mut a = open(left).map_err(|failure| (true, failure))?;
    let mut b = open(right).map_err(|failure| (false, failure))?;
    let mut ab = allocate_read_buffer(BLOCK).map_err(|error| (true, Failure::Io(error)))?;
    let mut bb = allocate_read_buffer(BLOCK).map_err(|error| (false, Failure::Io(error)))?;
    let mut remaining = left.stamp.bytes;
    let mut identical = true;
    while remaining > 0 {
        check(cancel).map_err(|failure| (false, failure))?;
        let count = usize::try_from(remaining.min(BLOCK as u64)).expect("block fits usize");
        a.read_exact(&mut ab[..count])
            .map_err(|error| (true, Failure::Io(error)))?;
        b.read_exact(&mut bb[..count])
            .map_err(|error| (false, Failure::Io(error)))?;
        *bytes_read += (count as u64) * 2;
        if ab[..count] != bb[..count] {
            identical = false;
            break;
        }
        remaining -= count as u64;
    }
    validate(left, &a).map_err(|failure| (true, failure))?;
    validate(right, &b).map_err(|failure| (false, failure))?;
    check(cancel).map_err(|failure| (false, failure))?;
    Ok(identical)
}

fn record(report: &mut Report, path: &Path, failure: Failure) {
    match failure {
        Failure::Cancelled => report.cancelled = true,
        Failure::Changed => report.issues.push(Issue {
            path: path.to_path_buf(),
            kind: IssueKind::Changed,
        }),
        Failure::Io(error) => report.issues.push(Issue {
            path: path.to_path_buf(),
            kind: IssueKind::Io(error.to_string()),
        }),
    }
}

fn issue(report: &mut Report, path: &Path, kind: IssueKind) {
    report.issues.push(Issue {
        path: path.to_path_buf(),
        kind,
    });
}

fn collect(
    roots: &[PathBuf],
    options: &Options,
    cancel: &impl Fn() -> bool,
    report: &mut Report,
) -> Vec<Entry> {
    let mut stack: Vec<_> = roots
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>()
        .into_iter()
        .rev()
        .map(|path| (path, 0_usize))
        .collect();
    let mut directories = BTreeSet::new();
    let mut files: BTreeMap<Identity, Entry> = BTreeMap::new();
    let mut visited = 0_usize;
    while let Some((path, depth)) = stack.pop() {
        if cancel() {
            report.cancelled = true;
            break;
        }
        if visited >= options.max_entries {
            issue(report, &path, IssueKind::Limit);
            break;
        }
        visited += 1;
        if depth > options.max_depth {
            issue(report, &path, IssueKind::Limit);
            continue;
        }
        let issue_path = path.clone();
        let result = (|| -> Result<(), Failure> {
            let link_metadata = fs::symlink_metadata(&path)?;
            if link_metadata.is_symlink() && !options.follow_symlinks {
                issue(report, &path, IssueKind::SymlinkSkipped);
                return Ok(());
            }
            let canonical = fs::canonicalize(&path)?;
            let metadata = fs::metadata(&canonical)?;
            let stamp = Stamp::new(&metadata, &canonical)?;
            if metadata.is_dir() {
                if !directories.insert(stamp.identity) {
                    return Ok(());
                }
                let mut children = BTreeSet::new();
                for child in fs::read_dir(&canonical)? {
                    check(cancel)?;
                    if visited.saturating_add(stack.len()).saturating_add(children.len())
                        >= options.max_entries
                    {
                        issue(report, &path, IssueKind::Limit);
                        return Ok(());
                    }
                    children.insert(path.join(child?.file_name()));
                }
                stack.extend(children.into_iter().rev().map(|child| (child, depth + 1)));
            } else if metadata.is_file() {
                if stamp.bytes > options.max_file_bytes {
                    issue(report, &path, IssueKind::Limit);
                } else if let Some(entry) = files.get_mut(&stamp.identity) {
                    entry.aliases.insert(path);
                } else {
                    files.insert(
                        stamp.identity.clone(),
                        Entry {
                            aliases: BTreeSet::from([path.clone()]),
                            path,
                            canonical,
                            stamp,
                        },
                    );
                }
            } else {
                issue(report, &path, IssueKind::NotRegular);
            }
            Ok(())
        })();
        if let Err(failure) = result {
            record(report, &issue_path, failure);
            if report.cancelled {
                break;
            }
        }
    }
    let mut entries: Vec<_> = files.into_values().collect();
    entries.sort_by(|a, b| a.path.cmp(&b.path));
    report.files_observed = entries.len();
    report.aliases = entries
        .iter()
        .filter(|entry| entry.aliases.len() > 1)
        .map(|entry| Aliases {
            paths: entry.aliases.iter().cloned().collect(),
        })
        .collect();
    entries
}

/// Scan file/directory roots, preserving successful results alongside diagnostics.
///
/// Cancellation is checked during traversal and every bounded read. All groups
/// require full hashes and fresh byte confirmation. Aliases are reported
/// separately and cannot inflate the number of independent duplicate copies.
pub fn scan(roots: &[PathBuf], options: &Options, cancel: impl Fn() -> bool) -> Report {
    scan_with_digest(roots, options, cancel, |digest| digest)
}

/// Scan with full-content observations for every admitted physical file,
/// including files pruned by size or sampled hashes. Revalidate all observations
/// before publication and discard groups involving changed sources.
/// This costs at least two full reads of each successfully observed file.
/// Observations still do not form an atomic filesystem snapshot.
pub fn scan_verified(roots: &[PathBuf], options: &Options, cancel: impl Fn() -> bool) -> Report {
    let mut report = scan_observed(roots, options, cancel, |digest| digest, true);
    if report.cancelled {
        report.groups.clear();
    }
    report
}

// Private injection point permits collision qualification without exposing an
// unsafe custom-hash policy to consumers. All reads and validation still run.
fn scan_with_digest(
    roots: &[PathBuf],
    options: &Options,
    cancel: impl Fn() -> bool,
    digest_identity: impl Fn([u8; 32]) -> [u8; 32],
) -> Report {
    scan_observed(roots, options, cancel, digest_identity, false)
}

#[allow(clippy::too_many_lines)] // Keep staged collision partitioning and representative recovery together.
fn scan_observed(
    roots: &[PathBuf],
    options: &Options,
    cancel: impl Fn() -> bool,
    digest_identity: impl Fn([u8; 32]) -> [u8; 32],
    verify_all: bool,
) -> Report {
    let mut report = Report::default();
    let entries = collect(roots, options, &cancel, &mut report);
    if report.cancelled {
        return report;
    }
    let baseline = if verify_all {
        observe_contents(&entries, &cancel, &mut report)
    } else {
        BTreeMap::new()
    };
    if report.cancelled {
        return report;
    }
    let mut sizes: BTreeMap<u64, Vec<usize>> = BTreeMap::new();
    for (index, entry) in entries.iter().enumerate() {
        if !verify_all || baseline.contains_key(&index) {
            sizes.entry(entry.stamp.bytes).or_default().push(index);
        }
    }
    let mut digests: BTreeMap<(u64, [u8; 32]), Vec<usize>> = BTreeMap::new();
    let mut observed_content = BTreeMap::new();
    for (bytes, candidates) in sizes.into_iter().filter(|(_, indices)| indices.len() > 1) {
        let mut samples: BTreeMap<[u8; 32], Vec<usize>> = BTreeMap::new();
        for index in candidates {
            match hash(&entries[index], true, &cancel, &mut report.bytes_read) {
                Ok(digest) => samples.entry(digest_identity(digest)).or_default().push(index),
                Err(failure) => record(&mut report, &entries[index].path, failure),
            }
            if report.cancelled {
                return report;
            }
        }
        for indices in samples.into_values().filter(|indices| indices.len() > 1) {
            for index in indices {
                match hash(&entries[index], false, &cancel, &mut report.bytes_read) {
                    Ok(digest) => {
                        observed_content.insert(index, digest);
                        digests
                            .entry((bytes, digest_identity(digest)))
                            .or_default()
                            .push(index);
                    }
                    Err(failure) => record(&mut report, &entries[index].path, failure),
                }
                if report.cancelled {
                    return report;
                }
            }
        }
    }
    for ((bytes, digest), indices) in digests.into_iter().filter(|(_, indices)| indices.len() > 1) {
        let mut verified: Vec<Vec<usize>> = Vec::new();
        for index in indices {
            let mut matched = false;
            let mut failed = false;
            for group in &mut verified {
                while !group.is_empty() {
                    match equal(
                        &entries[group[0]],
                        &entries[index],
                        &cancel,
                        &mut report.bytes_read,
                    ) {
                        Ok(true) => {
                            group.push(index);
                            matched = true;
                            break;
                        }
                        Ok(false) => {
                            let representative = group[0];
                            if content_unchanged(
                                &entries[representative],
                                observed_content[&representative],
                                &cancel,
                                &mut report,
                            ) {
                                break;
                            }
                            group.remove(0);
                            if report.cancelled {
                                return report;
                            }
                        }
                        Err((left_failed, failure)) => {
                            let culprit = if left_failed { group.remove(0) } else { index };
                            record(&mut report, &entries[culprit].path, failure);
                            if !left_failed || report.cancelled {
                                failed = true;
                                break;
                            }
                            // Retry this healthy candidate against the next surviving representative.
                        }
                    }
                }
                if matched || failed {
                    break;
                }
            }
            if report.cancelled {
                return report;
            }
            if !matched && !failed {
                verified.push(vec![index]);
            }
        }
        admit_verified_groups(
            &entries,
            verified,
            &observed_content,
            (bytes, digest),
            &cancel,
            &mut report,
        );
        if report.cancelled {
            return report;
        }
    }
    if verify_all {
        validate_observations(&entries, &baseline, &cancel, &mut report);
    }
    report.groups.sort_by(|a, b| a.paths.cmp(&b.paths));
    report.issues.sort_by(|a, b| a.path.cmp(&b.path));
    report
}

fn observe_contents(
    entries: &[Entry],
    cancel: &impl Fn() -> bool,
    report: &mut Report,
) -> BTreeMap<usize, [u8; 32]> {
    let mut observed = BTreeMap::new();
    for (index, entry) in entries.iter().enumerate() {
        match hash(entry, false, cancel, &mut report.bytes_read) {
            Ok(digest) => {
                observed.insert(index, digest);
            }
            Err(failure) => record(report, &entry.path, failure),
        }
        if report.cancelled {
            break;
        }
    }
    observed
}

fn validate_observations(
    entries: &[Entry],
    observed: &BTreeMap<usize, [u8; 32]>,
    cancel: &impl Fn() -> bool,
    report: &mut Report,
) {
    let mut changed = BTreeSet::new();
    for (&index, &digest) in observed {
        if !content_unchanged(&entries[index], digest, cancel, report) {
            changed.insert(entries[index].path.clone());
        }
        if report.cancelled {
            report.groups.clear();
            return;
        }
    }
    for group in &mut report.groups {
        group.paths.retain(|path| !changed.contains(path));
    }
    report.groups.retain(|group| group.paths.len() > 1);
}

fn admit_verified_groups(
    entries: &[Entry],
    verified: Vec<Vec<usize>>,
    observed_content: &BTreeMap<usize, [u8; 32]>,
    key: (u64, [u8; 32]),
    cancel: &impl Fn() -> bool,
    report: &mut Report,
) {
    let (bytes, digest) = key;
    for indices in verified.into_iter().filter(|indices| indices.len() > 1) {
        let survivors = surviving_members(entries, indices, observed_content, cancel, report);
        if report.cancelled {
            return;
        }
        if survivors.len() > 1 {
            report.groups.push(Group {
                bytes,
                digest,
                paths: survivors
                    .iter()
                    .map(|&index| entries[index].path.clone())
                    .collect(),
            });
        }
    }
}

fn content_unchanged(
    entry: &Entry,
    expected: [u8; 32],
    cancel: &impl Fn() -> bool,
    report: &mut Report,
) -> bool {
    match hash(entry, false, cancel, &mut report.bytes_read) {
        Ok(digest) if digest == expected => true,
        Ok(_) => {
            record(report, &entry.path, Failure::Changed);
            false
        }
        Err(failure) => {
            record(report, &entry.path, failure);
            false
        }
    }
}

// Preserve healthy members when another member changes after comparison.
fn surviving_members(
    entries: &[Entry],
    indices: Vec<usize>,
    observed_content: &BTreeMap<usize, [u8; 32]>,
    cancel: &impl Fn() -> bool,
    report: &mut Report,
) -> Vec<usize> {
    let mut survivors = Vec::new();
    for index in indices {
        if cancel() {
            report.cancelled = true;
            break;
        }
        if content_unchanged(&entries[index], observed_content[&index], cancel, report) {
            survivors.push(index);
        }
    }
    survivors
}

/// Stable observed file identity plus a full streaming content digest. This is
/// an observation contract, not an atomic filesystem snapshot.
#[derive(Debug)]
pub struct ContentSnapshot {
    entry: Entry,
    digest: [u8; 32],
}

#[derive(Debug, thiserror::Error)]
pub enum SnapshotError {
    #[error(transparent)]
    Io(#[from] io::Error),
    #[error("file changed during processing")]
    Changed,
    #[error("operation cancelled")]
    Cancelled,
    #[error("source is not a regular file or exceeds the byte budget")]
    Policy,
}

impl From<Failure> for SnapshotError {
    fn from(failure: Failure) -> Self {
        match failure {
            Failure::Io(error) => Self::Io(error),
            Failure::Changed => Self::Changed,
            Failure::Cancelled => Self::Cancelled,
        }
    }
}

impl ContentSnapshot {
    /// Capture identity and full content, following the initial path's symlinks.
    ///
    /// # Errors
    /// Returns source, policy, mutation and cancellation errors.
    pub fn read(path: &Path, max_bytes: u64, cancel: impl Fn() -> bool) -> Result<Self, SnapshotError> {
        Self::read_observed(path, max_bytes, cancel, Stamp::new)
    }

    fn read_observed(
        path: &Path,
        max_bytes: u64,
        cancel: impl Fn() -> bool,
        observe: impl FnOnce(&Metadata, &Path) -> io::Result<Stamp>,
    ) -> Result<Self, SnapshotError> {
        check(&cancel)?;
        let canonical = fs::canonicalize(path)?;
        let metadata = fs::metadata(path)?;
        if !metadata.is_file() || metadata.len() > max_bytes {
            return Err(SnapshotError::Policy);
        }
        let stamp = observe(&metadata, &canonical)?;
        // Windows acquires fresh handle metadata; replacement may enlarge the file
        // after the earlier path-metadata policy check. Refuse before hashing.
        if stamp.bytes > max_bytes {
            return Err(SnapshotError::Policy);
        }
        let entry = Entry {
            path: path.to_path_buf(),
            stamp,
            canonical,
            aliases: BTreeSet::new(),
        };
        let digest = hash(&entry, false, &cancel, &mut 0)?;
        Ok(Self { entry, digest })
    }

    /// Read a small format prefix through the same validated, nonblocking
    /// regular-file handle used by the full digest.
    ///
    /// # Errors
    /// Rejects prefixes above 64 KiB, short reads, mutation and cancellation.
    pub fn read_prefix(&self, length: usize, cancel: impl Fn() -> bool) -> Result<Vec<u8>, SnapshotError> {
        check(&cancel)?;
        if length > BLOCK {
            return Err(SnapshotError::Policy);
        }
        let mut file = open(&self.entry)?;
        let mut prefix = allocate_read_buffer(length)?;
        file.read_exact(&mut prefix)?;
        check(&cancel)?;
        validate(&self.entry, &file)?;
        Ok(prefix)
    }

    pub fn byte_count(&self) -> u64 {
        self.entry.stamp.bytes
    }

    /// Stream a bounded source range through a validated regular-file handle.
    ///
    /// # Errors
    /// Rejects invalid ranges, source mutation, I/O failure or cancellation.
    pub fn copy_range(
        &self,
        offset: u64,
        length: u64,
        mut output: impl io::Write,
        cancel: impl Fn() -> bool,
    ) -> Result<(), SnapshotError> {
        check(&cancel)?;
        if offset > self.entry.stamp.bytes || length > self.entry.stamp.bytes - offset {
            return Err(SnapshotError::Policy);
        }
        let mut file = open(&self.entry)?;
        file.seek(SeekFrom::Start(offset))?;
        let mut remaining = length;
        let mut buffer = allocate_read_buffer(BLOCK)?;
        while remaining != 0 {
            check(&cancel)?;
            let length = usize::try_from(remaining.min(BLOCK as u64)).map_err(|_| SnapshotError::Policy)?;
            file.read_exact(&mut buffer[..length])?;
            output.write_all(&buffer[..length])?;
            remaining -= length as u64;
        }
        check(&cancel)?;
        validate(&self.entry, &file)?;
        Ok(())
    }

    pub fn digest(&self) -> [u8; 32] {
        self.digest
    }

    /// Re-read full content and validate original identity/time stamps. Caller
    /// must invoke this after decoding before admitting newly generated evidence.
    ///
    /// # Errors
    /// Rejects changed/replaced sources and cancellation, without admitting data.
    pub fn verify(&self, cancel: impl Fn() -> bool) -> Result<(), SnapshotError> {
        if hash(&self.entry, false, &cancel, &mut 0)? != self.digest {
            return Err(SnapshotError::Changed);
        }
        Ok(())
    }
}

#[cfg(test)]
mod collision_tests {
    use super::*;

    #[test]
    fn read_buffer_capacity_failure_is_explicit_and_retry_succeeds() {
        let error = allocate_read_buffer(usize::MAX).unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::OutOfMemory);
        assert_eq!(allocate_read_buffer(4).unwrap(), [0; 4]);
    }

    #[test]
    fn verified_observations_detect_metadata_invisible_pruned_mutations() {
        let dir = tempfile::tempdir().unwrap();
        let paths = ["unique-size", "sample-a", "sample-b"].map(|p| dir.path().join(p));
        fs::write(&paths[0], b"unique-size").unwrap();
        fs::write(&paths[1], vec![1; SAMPLE * 4]).unwrap();
        fs::write(&paths[2], vec![2; SAMPLE * 4]).unwrap();
        let mut report = Report::default();
        let mut entries = collect(&paths, &Options::default(), &|| false, &mut report);
        let baseline = observe_contents(&entries, &|| false, &mut report);
        let samples = entries
            .iter()
            .map(|e| hash(e, true, &|| false, &mut report.bytes_read).unwrap())
            .collect::<Vec<_>>();
        assert!(samples[0] != samples[1]);
        for entry in &mut entries {
            if entry.path == paths[0] || entry.path == paths[1] {
                let mut data = fs::read(&entry.path).unwrap();
                let middle = data.len() / 2;
                data[middle] ^= 8;
                fs::write(&entry.path, data).unwrap();
                entry.stamp = Stamp::new(&fs::metadata(&entry.path).unwrap(), &entry.canonical).unwrap();
            }
        }
        let sample_index = entries.iter().position(|e| e.path == paths[1]).unwrap();
        assert_eq!(
            hash(&entries[sample_index], true, &|| false, &mut report.bytes_read).unwrap(),
            samples[sample_index]
        );
        validate_observations(&entries, &baseline, &|| false, &mut report);
        assert_eq!(report.issues.len(), 2);
        assert!(report.issues.iter().all(|i| i.kind == IssueKind::Changed));
        assert!(!report.complete());
    }

    #[test]
    fn final_content_validation_rejects_change_even_when_metadata_matches() {
        let root = tempfile::tempdir().unwrap();
        let paths = ["a", "b", "c"].map(|name| root.path().join(name));
        for path in &paths {
            fs::write(path, b"original").unwrap();
        }
        let mut report = Report::default();
        let mut entries = collect(&paths, &Options::default(), &|| false, &mut report);
        let observed = entries
            .iter()
            .enumerate()
            .map(|(i, entry)| (i, hash(entry, false, &|| false, &mut report.bytes_read).unwrap()))
            .collect::<BTreeMap<_, _>>();
        fs::write(&entries[0].path, b"modified").unwrap();
        // Inject metadata-indistinguishable observation without depending on
        // filesystem timestamp resolution or an ability to reset Unix ctime.
        entries[0].stamp =
            Stamp::new(&fs::metadata(&entries[0].path).unwrap(), &entries[0].canonical).unwrap();
        assert!(open(&entries[0]).is_ok());
        let survivors = surviving_members(&entries, vec![0, 1, 2], &observed, &|| false, &mut report);
        assert_eq!(survivors, [1, 2]);
        assert_eq!(report.issues.len(), 1);
        assert_eq!(report.issues[0].path, entries[0].path);
        assert_eq!(report.issues[0].kind, IssueKind::Changed);
    }

    #[test]
    fn colliding_samples_and_full_hashes_still_partition_by_bytes() {
        let root = tempfile::tempdir().unwrap();
        let mut paths = Vec::new();
        // Both short/empty and multi-block files exercise EOF and streaming paths.
        for length in [0, 1, BLOCK * 2 + 1] {
            for (name, value) in [("a", 1), ("b", 2), ("c", 1), ("d", 2), ("e", 3)] {
                let path = root.path().join(format!("{length}-{name}"));
                fs::write(&path, vec![value; length]).unwrap();
                paths.push(path);
            }
        }
        let report = scan_with_digest(&paths, &Options::default(), || false, |_| [0; 32]);
        assert!(report.complete(), "{report:?}");
        let mut expected = Vec::new();
        expected.push(
            ["a", "b", "c", "d", "e"]
                .map(|name| root.path().join(format!("0-{name}")))
                .to_vec(),
        );
        for length in [1, BLOCK * 2 + 1] {
            for names in [["a", "c"], ["b", "d"]] {
                expected.push(
                    names
                        .map(|name| root.path().join(format!("{length}-{name}")))
                        .to_vec(),
                );
            }
        }
        expected.sort();
        assert_eq!(
            report
                .groups
                .iter()
                .map(|group| group.paths.clone())
                .collect::<Vec<_>>(),
            expected
        );
        assert!(report.groups.iter().all(|group| group.digest == [0; 32]));
        assert!(report.bytes_read > 5 * (BLOCK * 2 + 1) as u64);
    }
}

#[derive(Debug)]
pub struct Discovery {
    /// One deterministic path per observed physical file. No content reads.
    pub files: Vec<PathBuf>,
    pub aliases: Vec<Aliases>,
    pub issues: Vec<Issue>,
    pub cancelled: bool,
}

/// Enumerate regular files using the same bounded traversal as exact scanning.
/// Errors and limit omissions remain visible; identity aliases are separate.
/// Observations are not an atomic filesystem snapshot.
pub fn discover(roots: &[PathBuf], options: &Options, cancel: impl Fn() -> bool) -> Discovery {
    let mut report = Report::default();
    let entries = collect(roots, options, &cancel, &mut report);
    report.issues.sort_by(|a, b| a.path.cmp(&b.path));
    Discovery {
        files: entries.into_iter().map(|entry| entry.path).collect(),
        aliases: report.aliases,
        issues: report.issues,
        cancelled: report.cancelled,
    }
}

#[cfg(test)]
mod observed_snapshot_policy_tests {
    use super::*;
    use std::cell::Cell;

    #[test]
    fn replacement_after_initial_metadata_cannot_bypass_observed_size_limit() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("source");
        fs::write(&path, b"tiny").unwrap();
        let callbacks = Cell::new(0);
        let result = ContentSnapshot::read_observed(
            &path,
            8,
            || {
                callbacks.set(callbacks.get() + 1);
                false
            },
            |initial, canonical| {
                assert_eq!(initial.len(), 4);
                fs::rename(canonical, directory.path().join("retained-original"))?;
                fs::write(canonical, [7; 16])?;
                // Reproduce Windows fresh-handle acquisition on any host platform.
                Stamp::from_file(&File::open(canonical)?, canonical)
            },
        );
        assert!(matches!(result, Err(SnapshotError::Policy)));
        assert_eq!(callbacks.get(), 1, "no hashing callback after admission refusal");
        let retry = ContentSnapshot::read(&path, 16, || false).unwrap();
        assert_eq!(retry.byte_count(), 16);
        retry.verify(|| false).unwrap();
    }
}
