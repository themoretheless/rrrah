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
    sync::atomic::{AtomicBool, Ordering},
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
                identity: Identity::Windows(
                    information.volume_serial_number(),
                    information.file_index(),
                ),
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
    if cancel() {
        Err(Failure::Cancelled)
    } else {
        Ok(())
    }
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

fn read_block(
    reader: &mut impl Read,
    buffer: &mut [u8],
    cancel: &impl Fn() -> bool,
) -> Result<usize, Failure> {
    loop {
        check(cancel)?;
        match reader.read(buffer) {
            Err(error) if error.kind() == io::ErrorKind::Interrupted => {}
            value => return value.map_err(Failure::Io),
        }
    }
}

fn read_exact_cancelled(
    reader: &mut impl Read,
    mut buffer: &mut [u8],
    cancel: &impl Fn() -> bool,
) -> Result<(), Failure> {
    while !buffer.is_empty() {
        let count = read_block(reader, buffer, cancel)?;
        if count == 0 {
            return Err(Failure::Io(io::Error::from(io::ErrorKind::UnexpectedEof)));
        }
        buffer = &mut buffer[count..];
    }
    Ok(())
}

fn write_all_cancelled(
    writer: &mut impl io::Write,
    mut buffer: &[u8],
    cancel: &impl Fn() -> bool,
) -> Result<(), Failure> {
    while !buffer.is_empty() {
        check(cancel)?;
        match writer.write(buffer) {
            Ok(0) => return Err(Failure::Io(io::Error::from(io::ErrorKind::WriteZero))),
            Ok(count) => buffer = &buffer[count..],
            Err(error) if error.kind() == io::ErrorKind::Interrupted => {}
            Err(error) => return Err(Failure::Io(error)),
        }
    }
    Ok(())
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
            read_exact_cancelled(&mut file, &mut buffer[..length], cancel)?;
            *bytes_read += length as u64;
            hasher.update(&buffer[..length]);
        }
    } else {
        let mut total = 0_u64;
        loop {
            check(cancel)?;
            let count = read_block(&mut file, &mut buffer, cancel)?;
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
        read_exact_cancelled(&mut a, &mut ab[..count], cancel)
            .map_err(|failure| (true, failure))?;
        read_exact_cancelled(&mut b, &mut bb[..count], cancel)
            .map_err(|failure| (false, failure))?;
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
    // Admit root bookkeeping before cloning paths or opening any source. A root
    // list beyond the traversal limit is refused as a whole; duplicates do not
    // consume extra entries. Borrow roots while building the bounded index.
    let mut admitted = BTreeSet::new();
    for path in roots {
        if cancel() {
            report.cancelled = true;
            return Vec::new();
        }
        if admitted.contains(path) {
            continue;
        }
        if admitted.len() >= options.max_entries {
            issue(report, path, IssueKind::Limit);
            return Vec::new();
        }
        admitted.insert(path);
    }
    let mut stack: Vec<_> = admitted
        .into_iter()
        .rev()
        .map(|path| (path.clone(), 0_usize))
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
                    if visited
                        .saturating_add(stack.len())
                        .saturating_add(children.len())
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
/// The final publication pass does not invoke `cancel`; use
/// [`scan_with_cancel_flag`] for cancellation during that pass.
pub fn scan(roots: &[PathBuf], options: &Options, cancel: impl Fn() -> bool) -> Report {
    scan_with_digest(roots, options, cancel, |digest| digest)
}

/// Scan with full-content observations for every admitted physical file,
/// including files pruned by size or sampled hashes. Revalidate all observations
/// before publication and discard groups involving changed sources.
/// This costs at least three full reads of each successfully observed file.
/// Observations still do not form an atomic filesystem snapshot.
/// The final pass does not invoke `cancel`; [`scan_verified_with_cancel_flag`]
/// supports cancellation during its reads.
pub fn scan_verified(roots: &[PathBuf], options: &Options, cancel: impl Fn() -> bool) -> Report {
    let mut report = scan_observed(roots, options, cancel, |digest| digest, true, None);
    if report.cancelled {
        report.groups.clear();
    }
    report
}

/// Exact scan with an externally owned cancellation flag, including the final
/// publication reads. Cancellation discards all groups. Keep the flag set until
/// the call returns; an in-flight filesystem operation is not preempted.
pub fn scan_with_cancel_flag(roots: &[PathBuf], options: &Options, cancel: &AtomicBool) -> Report {
    scan_flagged(roots, options, cancel, false)
}

/// Fully observed exact scan with the same cancellation contract as
/// [`scan_with_cancel_flag`]. No user callback runs during publication.
pub fn scan_verified_with_cancel_flag(
    roots: &[PathBuf],
    options: &Options,
    cancel: &AtomicBool,
) -> Report {
    scan_flagged(roots, options, cancel, true)
}

fn scan_flagged(
    roots: &[PathBuf],
    options: &Options,
    cancel: &AtomicBool,
    verify_all: bool,
) -> Report {
    let mut report = scan_observed(
        roots,
        options,
        || cancel.load(Ordering::Acquire),
        |digest| digest,
        verify_all,
        Some(cancel),
    );
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
    scan_observed(roots, options, cancel, digest_identity, false, None)
}

#[allow(clippy::too_many_lines)] // Keep staged collision partitioning and representative recovery together.
fn scan_observed(
    roots: &[PathBuf],
    options: &Options,
    cancel: impl Fn() -> bool,
    digest_identity: impl Fn([u8; 32]) -> [u8; 32],
    verify_all: bool,
    publication_cancel: Option<&AtomicBool>,
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
                Ok(digest) => samples
                    .entry(digest_identity(digest))
                    .or_default()
                    .push(index),
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
    if !report.cancelled {
        // A later callback can mutate a member already validated above or in
        // an earlier group. Publication owns this last pass: no caller code
        // runs after an observation is checked.
        let publication_observations = if verify_all {
            &baseline
        } else {
            &observed_content
        };
        let publication_stopped =
            || publication_cancel.is_some_and(|flag| flag.load(Ordering::Acquire));
        validate_observations(
            &entries,
            publication_observations,
            &publication_stopped,
            &mut report,
        );
        if publication_stopped() {
            report.cancelled = true;
            report.groups.clear();
        }
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

/// Compare two regular files without decoding or loading a model.
///
/// `max_file_bytes` limits each file. A positive result requires streaming byte
/// confirmation, not just matching digests. Both positive and negative results
/// validate both original observations after the last caller checkpoint.
/// Symlinks are followed initially. This observes file bytes only, not decoded
/// pixels or external resources, and is not an atomic filesystem snapshot.
///
/// # Errors
/// Rejects nonregular/oversized sources, mutation, I/O failure and cancellation.
pub fn compare_files(
    left: &Path,
    right: &Path,
    max_file_bytes: u64,
    cancel: impl Fn() -> bool,
) -> Result<bool, SnapshotError> {
    let stopped = std::cell::Cell::new(false);
    let cancelled = || {
        let value = stopped.get() || cancel();
        stopped.set(value);
        value
    };
    let result: Result<_, SnapshotError> =
        (|| read_compared_pair(left, right, max_file_bytes, &cancelled))();
    // No caller callback after this point: it could mutate either source after
    // that source had already passed its final validation.
    if cancelled() {
        return Err(SnapshotError::Cancelled);
    }
    let (identical, first, second) = result?;
    first.verify(|| false)?;
    second.verify(|| false)?;
    Ok(identical)
}

// Stream initial hashing and byte comparison together. Final callback-free publication
// verification in compare_files stays intact; no digest-only identity decision.
fn read_compared_pair(
    left: &Path,
    right: &Path,
    max_bytes: u64,
    cancel: &impl Fn() -> bool,
) -> Result<(bool, ContentSnapshot, ContentSnapshot), SnapshotError> {
    let observe = |path: &Path| -> Result<Entry, SnapshotError> {
        check(cancel)?;
        let canonical = fs::canonicalize(path)?;
        let metadata = fs::metadata(path)?;
        if !metadata.is_file() || metadata.len() > max_bytes {
            return Err(SnapshotError::Policy);
        }
        let stamp = Stamp::new(&metadata, &canonical)?;
        if stamp.bytes > max_bytes {
            return Err(SnapshotError::Policy);
        }
        Ok(Entry {
            path: path.to_path_buf(),
            stamp,
            canonical,
            aliases: BTreeSet::new(),
        })
    };
    let first = observe(left)?;
    let second = observe(right)?;
    if first.stamp.bytes != second.stamp.bytes {
        let a = hash(&first, false, cancel, &mut 0)?;
        let b = hash(&second, false, cancel, &mut 0)?;
        return Ok((
            false,
            ContentSnapshot {
                entry: first,
                digest: a,
            },
            ContentSnapshot {
                entry: second,
                digest: b,
            },
        ));
    }
    let mut a = open(&first)?;
    let mut b = open(&second)?;
    let mut ab = allocate_read_buffer(BLOCK)?;
    let mut bb = allocate_read_buffer(BLOCK)?;
    let mut ah = blake3::Hasher::new();
    let mut bh = blake3::Hasher::new();
    let mut remaining = first.stamp.bytes;
    let mut identical = true;
    while remaining > 0 {
        check(cancel)?;
        let count = usize::try_from(remaining.min(BLOCK as u64)).expect("block fits usize");
        read_exact_cancelled(&mut a, &mut ab[..count], cancel)?;
        read_exact_cancelled(&mut b, &mut bb[..count], cancel)?;
        ah.update(&ab[..count]);
        bh.update(&bb[..count]);
        identical = identical && ab[..count] == bb[..count];
        remaining -= count as u64;
    }
    check(cancel)?;
    validate(&first, &a)?;
    validate(&second, &b)?;
    Ok((
        identical,
        ContentSnapshot {
            entry: first,
            digest: *ah.finalize().as_bytes(),
        },
        ContentSnapshot {
            entry: second,
            digest: *bh.finalize().as_bytes(),
        },
    ))
}

#[cfg(any(feature = "decode", test))]
pub(crate) fn compare_observed(
    first: &ContentSnapshot,
    second: &ContentSnapshot,
    cancel: &impl Fn() -> bool,
) -> Result<bool, SnapshotError> {
    compare_observed_tagged(first, second, cancel).map_err(|(_, error)| error)
}

/// The bool in an error identifies the left (true) or right (false) source.
#[cfg(any(feature = "decode", test))]
pub(crate) fn compare_observed_tagged(
    first: &ContentSnapshot,
    second: &ContentSnapshot,
    cancel: &impl Fn() -> bool,
) -> Result<bool, (bool, SnapshotError)> {
    check(cancel).map_err(|error| (true, SnapshotError::from(error)))?;
    if first.entry.stamp.bytes != second.entry.stamp.bytes || first.digest != second.digest {
        return Ok(false);
    }
    equal(&first.entry, &second.entry, cancel, &mut 0)
        .map_err(|(left, failure)| (left, SnapshotError::from(failure)))
}

impl ContentSnapshot {
    /// Read at most 64 prefix bytes from the same streaming observation that
    /// revalidates the full original digest. Unlike `read_prefix`, this binds the
    /// returned header to the original content even if metadata misses a rewrite.
    /// # Errors
    /// Policy, I/O, mutation or cancellation failure; no unverified prefix escapes.
    pub fn read_verified_prefix(
        &self,
        length: usize,
        cancel: impl Fn() -> bool,
    ) -> Result<Vec<u8>, SnapshotError> {
        check(&cancel)?;
        if length > 64 || length as u64 > self.byte_count() {
            return Err(SnapshotError::Policy);
        }
        let mut file = open(&self.entry)?;
        let mut prefix = allocate_read_buffer(length)?;
        let mut buffer = allocate_read_buffer(BLOCK)?;
        let mut remaining = self.byte_count();
        let mut hasher = blake3::Hasher::new();
        while remaining > 0 {
            check(&cancel)?;
            let count = usize::try_from(remaining.min(BLOCK as u64)).expect("block fits usize");
            read_exact_cancelled(&mut file, &mut buffer[..count], &cancel)?;
            if remaining == self.byte_count() {
                prefix.copy_from_slice(&buffer[..length]);
            }
            hasher.update(&buffer[..count]);
            remaining -= count as u64;
        }
        check(&cancel)?;
        validate(&self.entry, &file)?;
        if hasher.finalize().as_bytes() != &self.digest {
            return Err(SnapshotError::Changed);
        }
        Ok(prefix)
    }

    /// Capture identity and full content, following the initial path's symlinks.
    ///
    /// # Errors
    /// Returns source, policy, mutation and cancellation errors.
    pub fn read(
        path: &Path,
        max_bytes: u64,
        cancel: impl Fn() -> bool,
    ) -> Result<Self, SnapshotError> {
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
    pub fn read_prefix(
        &self,
        length: usize,
        cancel: impl Fn() -> bool,
    ) -> Result<Vec<u8>, SnapshotError> {
        check(&cancel)?;
        if length > BLOCK {
            return Err(SnapshotError::Policy);
        }
        let mut file = open(&self.entry)?;
        let mut prefix = allocate_read_buffer(length)?;
        read_exact_cancelled(&mut file, &mut prefix, &cancel)?;
        check(&cancel)?;
        validate(&self.entry, &file)?;
        Ok(prefix)
    }

    pub fn byte_count(&self) -> u64 {
        self.entry.stamp.bytes
    }

    /// Stream a bounded source range through a validated regular-file handle.
    /// Cancellation is checked between write attempts; failure may leave a written prefix.
    /// An individual blocking writer call cannot be preempted.
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
            let length =
                usize::try_from(remaining.min(BLOCK as u64)).map_err(|_| SnapshotError::Policy)?;
            read_exact_cancelled(&mut file, &mut buffer[..length], &cancel)?;
            write_all_cancelled(&mut output, &buffer[..length], &cancel)?;
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
    fn formerly_empty_entries_are_reopened_before_hash_or_equality() {
        let root = tempfile::tempdir().unwrap();
        let paths = ["a", "b", "healthy-a", "healthy-b"].map(|name| root.path().join(name));
        for path in &paths {
            fs::write(path, []).unwrap();
        }
        let mut report = Report::default();
        let entries = collect(&paths, &Options::default(), &|| false, &mut report);
        assert!(report.complete());
        let entry = |path: &Path| entries.iter().find(|entry| entry.path == path).unwrap();
        fs::write(&paths[0], b"AAAA").unwrap();
        fs::write(&paths[1], b"BBBB").unwrap();
        for sampled in [true, false] {
            for path in &paths[..2] {
                assert!(matches!(
                    hash(entry(path), sampled, &|| false, &mut report.bytes_read),
                    Err(Failure::Changed)
                ));
            }
            assert_eq!(
                hash(entry(&paths[2]), sampled, &|| false, &mut report.bytes_read).unwrap(),
                *blake3::hash(&[]).as_bytes()
            );
        }
        assert!(matches!(
            equal(
                entry(&paths[0]),
                entry(&paths[2]),
                &|| false,
                &mut report.bytes_read
            ),
            Err((true, Failure::Changed))
        ));
        assert!(matches!(
            equal(
                entry(&paths[2]),
                entry(&paths[1]),
                &|| false,
                &mut report.bytes_read
            ),
            Err((false, Failure::Changed))
        ));
        assert!(
            equal(
                entry(&paths[2]),
                entry(&paths[3]),
                &|| false,
                &mut report.bytes_read
            )
            .unwrap()
        );
        // A fresh public scan must retain the healthy empty pair and separate
        // the now nonempty files rather than reusing the zero-size observation.
        let fresh = scan_verified(&paths, &Options::default(), || false);
        assert!(fresh.complete());
        assert_eq!(fresh.groups.len(), 1);
        assert_eq!(fresh.groups[0].bytes, 0);
        assert_eq!(fresh.groups[0].paths, paths[2..]);
    }

    #[test]
    fn verified_prefix_binds_header_and_body_even_when_metadata_matches() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("source");
        let original = vec![7u8; BLOCK + 19];
        for offset in [0, BLOCK + 9] {
            fs::write(&path, &original).unwrap();
            let mut snapshot = ContentSnapshot::read(&path, u64::MAX, || false).unwrap();
            let mut changed = original.clone();
            changed[offset] ^= 8;
            fs::write(&path, &changed).unwrap();
            // Simulate a filesystem whose metadata cannot reveal the rewrite.
            snapshot.entry.stamp =
                Stamp::new(&fs::metadata(&path).unwrap(), &snapshot.entry.canonical).unwrap();
            assert_eq!(snapshot.read_prefix(64, || false).unwrap(), changed[..64]);
            assert!(matches!(
                snapshot.read_verified_prefix(64, || false),
                Err(SnapshotError::Changed)
            ));
        }
    }

    #[test]
    fn verified_prefix_policy_cancel_and_final_mutation_do_not_publish_headers() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("source");
        for bytes in [Vec::new(), vec![7u8], vec![9u8; 2 * BLOCK + 9]] {
            fs::write(&path, &bytes).unwrap();
            let snapshot = ContentSnapshot::read(&path, u64::MAX, || false).unwrap();
            let length = bytes.len().min(64);
            let calls = std::cell::Cell::new(0);
            assert_eq!(
                snapshot
                    .read_verified_prefix(length, || {
                        calls.set(calls.get() + 1);
                        false
                    })
                    .unwrap(),
                bytes[..length]
            );
            assert!(matches!(
                snapshot.read_verified_prefix(65, || false),
                Err(SnapshotError::Policy)
            ));
            for stop in 1..=calls.get() {
                let current = std::cell::Cell::new(0);
                assert!(matches!(
                    snapshot.read_verified_prefix(length, || {
                        current.set(current.get() + 1);
                        current.get() == stop
                    }),
                    Err(SnapshotError::Cancelled)
                ));
            }
            let last = calls.get();
            calls.set(0);
            assert!(matches!(
                snapshot.read_verified_prefix(length, || {
                    calls.set(calls.get() + 1);
                    if calls.get() == last {
                        let mut changed = bytes.clone();
                        changed.push(0);
                        fs::write(&path, changed).unwrap();
                    }
                    false
                }),
                Err(SnapshotError::Changed)
            ));
        }
    }

    #[test]
    fn pair_comparison_confirms_bytes_and_refuses_digest_collision() {
        let dir = tempfile::tempdir().unwrap();
        let a = dir.path().join("a");
        let b = dir.path().join("b");
        for data in [Vec::new(), vec![17; BLOCK * 2 + 7]] {
            fs::write(&a, &data).unwrap();
            fs::write(&b, &data).unwrap();
            assert!(compare_files(&a, &b, data.len() as u64, || false).unwrap());
            assert!(compare_files(&a, &a, data.len() as u64, || false).unwrap());
        }
        let mut data = fs::read(&b).unwrap();
        *data.last_mut().unwrap() ^= 1;
        fs::write(&b, &data).unwrap();
        let first = ContentSnapshot::read(&a, u64::MAX, || false).unwrap();
        let mut second = ContentSnapshot::read(&b, u64::MAX, || false).unwrap();
        second.digest = first.digest;
        assert!(!compare_observed(&first, &second, &|| false).unwrap());
        assert!(!compare_files(&a, &b, u64::MAX, || false).unwrap());
        fs::write(&b, b"short").unwrap();
        assert!(!compare_files(&a, &b, u64::MAX, || false).unwrap());
    }

    #[test]
    fn pair_comparison_checks_policy_and_cancellation_at_every_checkpoint() {
        let dir = tempfile::tempdir().unwrap();
        let a = dir.path().join("a");
        let b = dir.path().join("b");
        fs::write(&a, vec![1; BLOCK + 1]).unwrap();
        for different in [false, true] {
            fs::write(&b, vec![if different { 2 } else { 1 }; BLOCK + 1]).unwrap();
            let calls = std::cell::Cell::new(0);
            assert_eq!(
                compare_files(&a, &b, u64::MAX, || {
                    calls.set(calls.get() + 1);
                    false
                })
                .unwrap(),
                !different
            );
            for stop in 1..=calls.get() {
                let current = std::cell::Cell::new(0);
                assert!(
                    matches!(
                        compare_files(&a, &b, u64::MAX, || {
                            current.set(current.get() + 1);
                            current.get() == stop
                        }),
                        Err(SnapshotError::Cancelled)
                    ),
                    "checkpoint {stop}"
                );
            }
        }
        assert!(matches!(
            compare_files(&a, &b, 0, || false),
            Err(SnapshotError::Policy)
        ));
        assert!(matches!(
            compare_files(dir.path(), &b, u64::MAX, || false),
            Err(SnapshotError::Policy)
        ));
        assert!(matches!(
            compare_files(&a, &dir.path().join("absent"), u64::MAX, || false),
            Err(SnapshotError::Io(_))
        ));
    }

    #[test]
    fn pair_comparison_rejects_last_checkpoint_mutations_on_both_outcomes() {
        let dir = tempfile::tempdir().unwrap();
        let paths = [dir.path().join("a"), dir.path().join("b")];
        for different in [false, true] {
            for victim in &paths {
                fs::write(&paths[0], b"first").unwrap();
                fs::write(&paths[1], if different { b"other" } else { b"first" }).unwrap();
                let calls = std::cell::Cell::new(0);
                compare_files(&paths[0], &paths[1], 5, || {
                    calls.set(calls.get() + 1);
                    false
                })
                .unwrap();
                let final_checkpoint = calls.get();
                calls.set(0);
                let result = compare_files(&paths[0], &paths[1], 5, || {
                    calls.set(calls.get() + 1);
                    if calls.get() == final_checkpoint {
                        fs::write(victim, b"new!!").unwrap();
                    }
                    false
                });
                assert!(matches!(result, Err(SnapshotError::Changed)));
            }
        }
    }

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
                entry.stamp =
                    Stamp::new(&fs::metadata(&entry.path).unwrap(), &entry.canonical).unwrap();
            }
        }
        let sample_index = entries.iter().position(|e| e.path == paths[1]).unwrap();
        assert_eq!(
            hash(
                &entries[sample_index],
                true,
                &|| false,
                &mut report.bytes_read
            )
            .unwrap(),
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
            .map(|(i, entry)| {
                (
                    i,
                    hash(entry, false, &|| false, &mut report.bytes_read).unwrap(),
                )
            })
            .collect::<BTreeMap<_, _>>();
        fs::write(&entries[0].path, b"modified").unwrap();
        // Inject metadata-indistinguishable observation without depending on
        // filesystem timestamp resolution or an ability to reset Unix ctime.
        entries[0].stamp = Stamp::new(
            &fs::metadata(&entries[0].path).unwrap(),
            &entries[0].canonical,
        )
        .unwrap();
        assert!(open(&entries[0]).is_ok());
        let survivors =
            surviving_members(&entries, vec![0, 1, 2], &observed, &|| false, &mut report);
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
        assert_eq!(
            callbacks.get(),
            1,
            "no hashing callback after admission refusal"
        );
        let retry = ContentSnapshot::read(&path, 16, || false).unwrap();
        assert_eq!(retry.byte_count(), 16);
        retry.verify(|| false).unwrap();
    }
}

#[cfg(test)]
mod interrupted_read_tests {
    use super::*;
    use std::cell::Cell;
    struct InterruptedReader<'a> {
        calls: &'a Cell<usize>,
    }
    impl Read for InterruptedReader<'_> {
        fn read(&mut self, _: &mut [u8]) -> io::Result<usize> {
            self.calls.set(self.calls.get() + 1);
            Err(io::ErrorKind::Interrupted.into())
        }
    }
    #[test]
    fn interrupted_reads_observe_cancellation_before_retry() {
        let calls = Cell::new(0);
        let mut reader = InterruptedReader { calls: &calls };
        let result = read_exact_cancelled(&mut reader, &mut [0; 8], &|| calls.get() >= 3);
        assert!(matches!(result, Err(Failure::Cancelled)));
        assert_eq!(calls.get(), 3);
    }
    #[test]
    fn read_cancelled_before_first_attempt_does_not_touch_reader() {
        let calls = Cell::new(0);
        let mut reader = InterruptedReader { calls: &calls };
        assert!(matches!(
            read_block(&mut reader, &mut [0; 1], &|| true),
            Err(Failure::Cancelled)
        ));
        assert_eq!(calls.get(), 0);
    }
    #[test]
    fn short_reads_and_interruptions_preserve_content_and_eof_errors() {
        struct Chunked {
            calls: usize,
            data: io::Cursor<Vec<u8>>,
        }
        impl Read for Chunked {
            fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
                self.calls += 1;
                if self.calls % 2 == 1 {
                    return Err(io::ErrorKind::Interrupted.into());
                }
                let count = buffer.len().min(2);
                self.data.read(&mut buffer[..count])
            }
        }
        let mut reader = Chunked {
            calls: 0,
            data: io::Cursor::new(b"abcdefg".to_vec()),
        };
        let mut buffer = [0; 7];
        read_exact_cancelled(&mut reader, &mut buffer, &|| false).unwrap();
        assert_eq!(&buffer, b"abcdefg");
        assert!(
            matches!(read_exact_cancelled(&mut reader, &mut [0; 1], &|| false), Err(Failure::Io(e)) if e.kind() == io::ErrorKind::UnexpectedEof)
        );
        read_exact_cancelled(&mut reader, &mut [], &|| false).unwrap();
    }
}

#[cfg(test)]
mod cancelled_output_tests {
    use super::*;
    use std::cell::{Cell, RefCell};
    struct Writer<'a> {
        calls: &'a Cell<usize>,
        bytes: &'a RefCell<Vec<u8>>,
        mode: u8,
    }
    impl io::Write for Writer<'_> {
        fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
            self.calls.set(self.calls.get() + 1);
            match self.mode {
                0 if self.calls.get() <= 3 => return Err(io::ErrorKind::Interrupted.into()),
                2 if self.calls.get() % 2 == 1 => return Err(io::ErrorKind::Interrupted.into()),
                3 => return Ok(0),
                4 => return Err(io::ErrorKind::BrokenPipe.into()),
                _ => {}
            }
            let count = if self.mode == 0 {
                bytes.len()
            } else {
                bytes.len().min(2)
            };
            self.bytes.borrow_mut().extend_from_slice(&bytes[..count]);
            Ok(count)
        }
        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }
    fn snapshot() -> (tempfile::TempDir, ContentSnapshot) {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("source");
        fs::write(&path, b"0123456789").unwrap();
        let snapshot = ContentSnapshot::read(&path, 10, || false).unwrap();
        (root, snapshot)
    }
    #[test]
    fn interrupted_output_cancels_before_next_write_attempt() {
        let (_root, snapshot) = snapshot();
        let calls = Cell::new(0);
        let bytes = RefCell::new(Vec::new());
        let writer = Writer {
            calls: &calls,
            bytes: &bytes,
            mode: 0,
        };
        assert!(matches!(
            snapshot.copy_range(1, 8, writer, || calls.get() >= 3),
            Err(SnapshotError::Cancelled)
        ));
        assert_eq!(calls.get(), 3);
        assert!(bytes.borrow().is_empty());
    }
    #[test]
    fn partial_output_cancels_without_writing_remaining_bytes() {
        let (_root, snapshot) = snapshot();
        let calls = Cell::new(0);
        let bytes = RefCell::new(Vec::new());
        let writer = Writer {
            calls: &calls,
            bytes: &bytes,
            mode: 1,
        };
        assert!(matches!(
            snapshot.copy_range(1, 8, writer, || calls.get() >= 2),
            Err(SnapshotError::Cancelled)
        ));
        assert_eq!(calls.get(), 2);
        assert_eq!(&*bytes.borrow(), b"1234");
    }
    #[test]
    fn healthy_short_interrupted_output_preserves_exact_range() {
        let (_root, snapshot) = snapshot();
        let calls = Cell::new(0);
        let bytes = RefCell::new(Vec::new());
        let writer = Writer {
            calls: &calls,
            bytes: &bytes,
            mode: 2,
        };
        snapshot.copy_range(1, 8, writer, || false).unwrap();
        assert_eq!(&*bytes.borrow(), b"12345678");
        assert_eq!(calls.get(), 8);
    }
    #[test]
    fn zero_progress_and_output_errors_remain_typed_io_failures() {
        let (_root, snapshot) = snapshot();
        for (mode, expected) in [
            (3, io::ErrorKind::WriteZero),
            (4, io::ErrorKind::BrokenPipe),
        ] {
            let calls = Cell::new(0);
            let bytes = RefCell::new(Vec::new());
            let writer = Writer {
                calls: &calls,
                bytes: &bytes,
                mode,
            };
            assert!(
                matches!(snapshot.copy_range(1, 8, writer, || false), Err(SnapshotError::Io(e)) if e.kind() == expected)
            );
            assert_eq!(calls.get(), 1);
            assert!(bytes.borrow().is_empty());
        }
    }
}

#[cfg(test)]
mod cancellation_flag_tests {
    use super::*;
    use std::cell::Cell;

    #[test]
    fn final_publication_observes_flag_without_reentering_caller() {
        let directory = tempfile::tempdir().unwrap();
        let paths = [directory.path().join("a"), directory.path().join("b")];
        for path in &paths {
            fs::write(path, vec![91; BLOCK * 3]).unwrap();
        }
        for verified in [false, true] {
            let calls = Cell::new(0);
            let baseline = scan_observed(
                &paths,
                &Options::default(),
                || {
                    calls.set(calls.get() + 1);
                    false
                },
                |digest| digest,
                verified,
                None,
            );
            assert!(baseline.complete());
            assert_eq!(baseline.groups.len(), 1);
            let last = calls.get();
            calls.set(0);
            let flag = AtomicBool::new(false);
            let cancelled = scan_observed(
                &paths,
                &Options::default(),
                || {
                    calls.set(calls.get() + 1);
                    if calls.get() == last {
                        flag.store(true, Ordering::Release);
                    }
                    // Only the publication flag can observe this cancellation.
                    false
                },
                |digest| digest,
                verified,
                Some(&flag),
            );
            assert_eq!(calls.get(), last);
            assert!(cancelled.cancelled);
            assert!(cancelled.groups.is_empty());
            assert!(cancelled.bytes_read < baseline.bytes_read);
            flag.store(false, Ordering::Release);
            let retry = scan_flagged(&paths, &Options::default(), &flag, verified);
            assert!(retry.complete());
            assert_eq!(retry.groups, baseline.groups);
        }
    }
}

#[cfg(test)]
mod fused_pair_controls {
    use super::*;
    #[test]
    fn fused_pair_matches_independent_bytes_across_block_edges_and_mismatch_positions() {
        let dir = tempfile::tempdir().unwrap();
        let a = dir.path().join("a");
        let b = dir.path().join("b");
        for length in [0, 1, 2, BLOCK - 1, BLOCK, BLOCK + 1, BLOCK * 2 + 7] {
            let original: Vec<u8> = (0..length)
                .map(|i| (i.wrapping_mul(73) ^ (i >> 7)) as u8)
                .collect();
            fs::write(&a, &original).unwrap();
            let mut variants = vec![original.clone()];
            for position in [0, length / 2, length.saturating_sub(1)] {
                if length != 0 {
                    let mut changed = original.clone();
                    changed[position] ^= 1;
                    variants.push(changed);
                }
            }
            variants.push(vec![51; length + 1]);
            for value in variants {
                fs::write(&b, &value).unwrap();
                let expected = original == value;
                assert_eq!(compare_files(&a, &b, u64::MAX, || false).unwrap(), expected);
                assert_eq!(compare_files(&b, &a, u64::MAX, || false).unwrap(), expected);
            }
        }
    }
}
