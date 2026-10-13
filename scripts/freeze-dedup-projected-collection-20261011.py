#!/usr/bin/env python3
"""Freeze shared collection engine and projected HDR/alpha collection adapters."""
import hashlib
import json
from pathlib import Path
import shutil

ROOT = Path(__file__).resolve().parents[1]
OUT = ROOT / 'docs/research/dedup-projected-collection-snapshot-20261011.json'
DEST = Path('/Users/themoretheless/.codex/tmp/rrrah-projected-collection-snapshot-20261011')
CHANGES = ['crates/rrrah-dedup/src/collection.rs', 'crates/rrrah-dedup/src/portfolio_collection.rs', 'crates/rrrah-dedup/src/projected_collection.rs', 'crates/rrrah-dedup/src/model_projected_tiles.rs', 'crates/rrrah-dedup/src/model_loftr.rs', 'crates/rrrah-dedup/src/lib.rs', 'crates/rrrah-dedup/tests/projected_collection.rs', 'scripts/dedup-loftr-native-probe/src/bin/projected_collection_files.rs']

def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()

def main():
    assert not OUT.exists() and not DEST.exists()
    parent_path = ROOT / 'docs/research/dedup-projected-tiles-snapshot-20261011.json'
    parent = json.loads(parent_path.read_text())
    source = Path(parent['snapshot'])
    for name, digest in parent['file_hashes'].items():
        assert sha(source / name) == digest, name
        target = DEST / name
        target.parent.mkdir(parents=True, exist_ok=True)
        shutil.copy2(source / name, target)
    for name in CHANGES:
        shutil.copy2(ROOT / name, DEST / name)
    hashes = {name: sha(DEST / name) for name in dict.fromkeys(list(parent['file_hashes']) + CHANGES)}
    assert all(hashes[name] == digest for name, digest in parent['file_hashes'].items()
               if name not in CHANGES)
    assert all(sha(ROOT / name) == hashes[name] for name in CHANGES)
    report = {'snapshot': str(DEST), 'parent_manifest': str(parent_path),
              'parent_manifest_sha256': sha(parent_path), 'changes': CHANGES,
              'file_hashes': hashes,
              'scope': 'Frozen projected model tiles parent with shared exhaustive collection lifecycle, legacy compatibility adapter and projected collection generic/native APIs; qualification pending.',
              'producer': str(Path(__file__).resolve()), 'producer_sha256': sha(Path(__file__).resolve())}
    OUT.write_text(json.dumps(report, indent=2) + '\n')
    print('frozen', len(hashes), 'files; changes', len(CHANGES))

if __name__ == '__main__':
    main()
