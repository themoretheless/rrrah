#!/usr/bin/env python3
"""Create a verified, read-only source/fixture snapshot for native validation."""
import argparse
import hashlib
import json
import os
import shutil
from pathlib import Path

p = argparse.ArgumentParser()
p.add_argument('destination', type=Path)
a = p.parse_args()
root = Path(__file__).resolve().parent.parent
destination = a.destination.resolve()
assert not destination.exists(), 'Use a fresh validation directory.'
files = []
for name in ('crates', 'vendor', 'tests', '.cargo'):
    for current, directories, names in os.walk(root / name, followlinks=False):
        for child in [*directories, *names]:
            assert not (Path(current) / child).is_symlink(), 'Snapshot must be self-contained; inspect symlink before copying.'
        files.extend(Path(current) / name for name in names)
for name in ('Cargo.toml', 'Cargo.lock', 'rust-toolchain.toml', 'rustfmt.toml', 'build.rs', 'README.md', 'LICENSE', 'LICENSE-MIT', 'LICENSE-APACHE'):
    if (root / name).is_file():
        files.append(root / name)
files.sort()
digest = lambda path: hashlib.sha256(path.read_bytes()).hexdigest()
before = {str(path.relative_to(root)): digest(path) for path in files}
destination.mkdir(parents=True)
for path in files:
    relative = path.relative_to(root)
    target = destination / relative
    target.parent.mkdir(parents=True, exist_ok=True)
    shutil.copyfile(path, target)
    assert digest(target) == before[str(relative)], str(relative)
assert {str(path.relative_to(root)): digest(path) for path in files} == before, 'Source changed during snapshot; preserve this failed snapshot and retry in a fresh directory.'
manifest = {'status': 'verified_self_contained_source_snapshot', 'source': str(root),
            'files': before, 'total_bytes': sum(path.stat().st_size for path in files),
            'scope': 'Copied source/fixture inputs only; no compilation or runtime qualification.'}
(destination / 'validation-snapshot.json').write_text(json.dumps(manifest, indent=2) + '\n')
for current, directories, names in os.walk(destination):
    for name in names:
        (Path(current) / name).chmod(0o444)
    Path(current).chmod(0o555)
print(json.dumps({'destination': str(destination), 'files': len(before),
                  'total_bytes': manifest['total_bytes'], 'manifest_sha256': digest(destination / 'validation-snapshot.json')}))
