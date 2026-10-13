#!/usr/bin/env python3
"""Verify the pinned external inputs for manual Metal camera qualification."""
import argparse
import hashlib
import json
from pathlib import Path


def main():
    root = Path(__file__).resolve().parents[1]
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--manifest', type=Path, default=root / 'docs/research/metal-current-manual-followups-2026-10-11.json')
    parser.add_argument('--output', type=Path)
    parser.add_argument('--input-root', type=Path, help='Resolve pinned input basenames under a recovered local corpus')
    args = parser.parse_args()
    manifest = json.loads(args.manifest.read_text())
    results = []
    for name, entry in manifest['external_followups']['inputs'].items():
        original = Path(entry['path'])
        path = args.input_root / original.name if args.input_root else original
        result = {'name': name, 'path': str(path), 'expected_sha256': entry['sha256']}
        try:
            with path.open('rb') as source:
                digest = hashlib.sha256()
                for block in iter(lambda: source.read(1024 * 1024), b''):
                    digest.update(block)
                actual = digest.hexdigest()
            result.update(actual_sha256=actual, status='verified' if actual == entry['sha256'] else 'mismatch')
        except FileNotFoundError:
            result['status'] = 'missing'
        except OSError as error:
            result.update(status='unreadable', error=str(error))
        results.append(result)
    report = {'manifest': str(args.manifest.resolve()), 'verified': all(item['status'] == 'verified' for item in results), 'inputs': results}
    encoded = json.dumps(report, indent=2) + '\n'
    if args.output:
        args.output.write_text(encoded)
    print(encoded, end='')
    return 0 if report['verified'] else 1


if __name__ == '__main__':
    raise SystemExit(main())
