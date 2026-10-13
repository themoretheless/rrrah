#!/usr/bin/env python3
"""Freeze the real later-recipe controls with the captured pre-fix native implementation."""
import hashlib
import json
from pathlib import Path
import shutil
ROOT = Path(__file__).resolve().parents[1]
OUT = ROOT / 'docs/research/dedup-native-later-recovery-before-snapshot-20261011.json'
DEST = Path('/Users/themoretheless/.codex/tmp/rrrah-native-later-recovery-before-snapshot-20261011')
OLD = Path('/Users/themoretheless/.codex/tmp/rrrah-native-fallback-before-20261011/anchor_rank_file.rs')
NAME = 'crates/rrrah-dedup/src/anchor_rank_file.rs'
def sha(p):
    return hashlib.sha256(p.read_bytes()).hexdigest()
def main():
    assert not OUT.exists() and not DEST.exists()
    parent_path = ROOT / 'docs/research/dedup-native-later-recovery-snapshot-20261011.json'
    parent = json.loads(parent_path.read_text()); source = Path(parent['snapshot'])
    hashes = {}
    for name, digest in parent['file_hashes'].items():
        assert sha(source / name) == digest
        target = DEST / name; target.parent.mkdir(parents=True, exist_ok=True)
        shutil.copy2(source / name, target)
        hashes[name] = sha(target)
    shutil.copy2(OLD, DEST / NAME); hashes[NAME] = sha(DEST / NAME)
    assert all(hashes[name] == digest for name, digest in parent['file_hashes'].items() if name != NAME)
    report = {'snapshot': str(DEST), 'parent_manifest': str(parent_path),
              'parent_manifest_sha256': sha(parent_path), 'changes': [NAME],
              'before_source': str(OLD), 'before_source_sha256': sha(OLD), 'file_hashes': hashes,
              'scope': 'Same controls and frozen dependencies; only native fallback implementation reverted to captured pre-fix source.',
              'producer': str(Path(__file__).resolve()), 'producer_sha256': sha(Path(__file__).resolve())}
    OUT.write_text(json.dumps(report, indent=2)+'\n')
    print('before snapshot', len(hashes))
if __name__ == '__main__': main()
