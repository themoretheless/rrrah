#!/usr/bin/env python3
from pathlib import Path
import json,hashlib,shutil
R=Path(__file__).resolve().parents[1];D=R/'docs/research';F=Path('/private/tmp/rrrah-fused-exact-pair-20261014');F.mkdir(exist_ok=False);sha=lambda p:hashlib.sha256(Path(p).read_bytes()).hexdigest()
for v in ['baseline','candidate']:
 (F/v).mkdir();shutil.copytree(R/'crates/rrrah-dedup/src',F/v/'src');shutil.copyfile(R/'crates/rrrah-dedup/Cargo.toml',F/v/'Cargo.toml')
p=F/'candidate/src/exact.rs';s=p.read_text();old='''        let first = ContentSnapshot::read(left, max_file_bytes, &cancelled)?;
        let second = ContentSnapshot::read(right, max_file_bytes, &cancelled)?;
        let identical = compare_observed(&first, &second, &cancelled)?;
        Ok((identical, first, second))''';new='''        read_compared_pair(left, right, max_file_bytes, &cancelled)''';assert s.count(old)==1;s=s.replace(old,new)
helper='''
// Experimental fused first observation only. Final callback-free publication
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
        Ok(Entry { path: path.to_path_buf(), stamp, canonical, aliases: BTreeSet::new() })
    };
    let first = observe(left)?;
    let second = observe(right)?;
    if first.stamp.bytes != second.stamp.bytes {
        let a = hash(&first, false, cancel, &mut 0)?;
        let b = hash(&second, false, cancel, &mut 0)?;
        return Ok((false, ContentSnapshot { entry: first, digest: a }, ContentSnapshot { entry: second, digest: b }));
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
        identical &= ab[..count] == bb[..count];
        remaining -= count as u64;
    }
    check(cancel)?;
    validate(&first, &a)?;
    validate(&second, &b)?;
    Ok((identical, ContentSnapshot { entry: first, digest: *ah.finalize().as_bytes() }, ContentSnapshot { entry: second, digest: *bh.finalize().as_bytes() }))
}
'''
marker='pub(crate) fn compare_observed(';assert s.count(marker)==1;s=s.replace(marker,helper+'\n'+marker);p.write_text(s)
subprocess=None
O=D/'dedup-fused-exact-pair-experiment-20261014.json';assert not O.exists();O.write_text(json.dumps(dict(status='isolated_candidate_prepared_not_adopted',source=str(F),root_source_sha256={str(p.relative_to(R)):sha(p)for p in (R/'crates/rrrah-dedup/src').glob('*.rs')},variants={v:{str(p.relative_to(F/v)):sha(p)for p in (F/v/'src').glob('*.rs')}for v in ['baseline','candidate']},producer_sha256=sha(__file__),scope='Isolated current-root no-feature compare_files public API experiment. First fullhash and streaming byte comparison fused; final no-caller-callback full source verification unchanged. Different-size path still fully hashes both sources. No change to root, collection scanner, existing compare_observed collision proof, decode or native paths. Correctness, CPU, allocation and all cancellation/mutation gates pending; no adoption/speedup claim.'),indent=2)+'\n');print(F)
