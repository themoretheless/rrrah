#!/usr/bin/env python3
"""Freeze exact-first projected portfolio and its regression controls."""
import hashlib
import json
from pathlib import Path
import shutil

ROOT = Path(__file__).resolve().parents[1]
OUT = ROOT / 'docs/research/dedup-exact-classes-snapshot-20261011.json'
DEST = Path('/Users/themoretheless/.codex/tmp/rrrah-exact-classes-snapshot-20261011')
CHANGES = ['crates/rrrah-dedup/src/collection.rs', 'crates/rrrah-dedup/tests/projected_portfolio.rs']

def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()

def main():
    assert not OUT.exists() and not DEST.exists()
    parent_path = ROOT / 'docs/research/dedup-exact-collection-scaling-baseline-snapshot-20261011.json'
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
              'scope': 'Managed operation-local exact equality certificates. All per-pair and final source/dependency/model guards retained; only repeated byte confirmation is cached. Benchmark analytical byte formula reflects certified spanning edges.',
              'producer': str(Path(__file__).resolve()), 'producer_sha256': sha(Path(__file__).resolve())}
    OUT.write_text(json.dumps(report, indent=2) + '\n')
    print('frozen', len(hashes), 'files; changes', len(CHANGES))

if __name__ == '__main__':
    main()
