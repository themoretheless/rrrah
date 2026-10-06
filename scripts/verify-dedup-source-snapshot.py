#!/usr/bin/env python3
"""Verify the full file set and hashes of an isolated qualification snapshot."""
import argparse
import hashlib
import json
import pathlib

parser = argparse.ArgumentParser()
parser.add_argument('snapshot', type=pathlib.Path)
parser.add_argument('root', type=pathlib.Path)
parser.add_argument('output', type=pathlib.Path)
args = parser.parse_args()
snapshot_bytes = args.snapshot.read_bytes()
snapshot = json.loads(snapshot_bytes)
expected = snapshot['source_sha256']
assert len(expected) == snapshot['files']
root = args.root.resolve()
actual = {}
for top in sorted({pathlib.PurePosixPath(name).parts[0] for name in expected}):
    path = root / top
    paths = path.rglob('*') if path.is_dir() else [path]
    for child in paths:
        relative = child.relative_to(root)
        if any(part in ('target', '.git') for part in relative.parts):
            continue
        if child.is_file():
            actual[relative.as_posix()] = hashlib.sha256(child.read_bytes()).hexdigest()
missing = sorted(expected.keys() - actual.keys())
added = sorted(actual.keys() - expected.keys())
changed = sorted(name for name in expected.keys() & actual.keys()
                 if expected[name] != actual[name])
result = {
    'snapshot_sha256': hashlib.sha256(snapshot_bytes).hexdigest(),
    'root': str(root), 'expected_files': len(expected),
    'actual_files': len(actual), 'missing': missing, 'added': added,
    'changed': changed, 'integrity_passed': not (missing or added or changed),
    'scope': 'Current file set and hashes under snapshot acquisition roots; '
             'build targets excluded. Does not prove historical ABA or test success.',
}
args.output.write_text(json.dumps(result, indent=2) + '\n')
print(result)
raise SystemExit(0 if result['integrity_passed'] else 1)
