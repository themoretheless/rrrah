#!/usr/bin/env python3
"""Exact mask/selection qualification against the pinned GIMP test fixture."""
import argparse
import hashlib
import json
from pathlib import Path
import subprocess
import tempfile

SOURCE_SHA = 'a1557a22fd7ba5d8185447c932a708bd2369910203081590e45e3a3cabe46636'
COMMIT = '025352c5745884086d7095969bc5942bc6fbff49'


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('source', type=Path)
    parser.add_argument('--dump', type=Path, required=True)
    parser.add_argument('--report', type=Path, required=True)
    args = parser.parse_args()
    source = args.source.resolve()
    if hashlib.sha256(source.read_bytes()).hexdigest() != SOURCE_SHA:
        raise SystemExit('Pinned GIMP fixture hash mismatch; no decoding performed')
    binary = args.dump.resolve()
    with tempfile.TemporaryDirectory(prefix='rrrah-xcf-gimp-oracle-') as directory:
        run = subprocess.run([str(binary), str(source), directory],
                             capture_output=True, text=True, check=True)
        records = [json.loads(line) for line in run.stdout.splitlines()]
        expected_selection = bytes(255 if 5 <= x < 12 and 6 <= y < 14 else 0
                                   for y in range(90) for x in range(100))
        rows = []
        for name, expected in [('mask-0.bin', bytes(25 * 251)),
                               ('channel-1.bin', expected_selection)]:
            actual = (Path(directory) / name).read_bytes()
            rows.append({'plane': name, 'expected_bytes': len(expected),
                         'actual_bytes': len(actual), 'exact': actual == expected,
                         'different_bytes': sum(a != b for a, b in zip(actual, expected))
                                            + abs(len(actual) - len(expected)),
                         'actual_sha256': hashlib.sha256(actual).hexdigest()})
    metadata = []
    for selector, index, expected in [
        ('layer_attributes', 0, {'visible': True, 'opacity': 0,
                                'offset_x': 0, 'offset_y': 0, 'apply_mask': True}),
        ('layer_attributes', 1, {'visible': True, 'opacity': 1,
                                'offset_x': 0, 'offset_y': 0, 'apply_mask': False}),
        ('layer_composition', 0, {'blend_mode': 3}),
        ('layer_composition', 1, {'blend_mode': 0}),
        ('layer', 0, {'width': 25, 'height': 251, 'kind': 0}),
        ('layer', 1, {'width': 50, 'height': 51, 'kind': 1}),
    ]:
        matches = [record for record in records if record.get(selector) == index]
        actual = matches[0] if len(matches) == 1 else None
        exact = actual is not None and all(actual.get(key) == value
                                           for key, value in expected.items())
        metadata.append({'selector': selector, 'index': index,
                         'expected': expected, 'actual': actual, 'exact': exact})
    report = {'source_sha256': SOURCE_SHA, 'commit': COMMIT,
              'source_url': f'https://raw.githubusercontent.com/GNOME/gimp/{COMMIT}/app/tests/files/gimp-2-6-file.xcf',
              'oracle_source_url': f'https://github.com/GNOME/gimp/blob/{COMMIT}/app/tests/test-xcf.c',
              'oracle': 'GIMP_ADD_MASK_BLACK and unfeathered rectangle selection x=5,y=6,w=7,h=8 from GIMP test construction',
              'decoder_sha256': hashlib.sha256(binary.read_bytes()).hexdigest(),
              'records': records, 'comparisons': rows, 'metadata_comparisons': metadata,
              'scope': 'Full native byte planes of one black layer mask and one selection channel; no flattened rendering, blend, color, precision or other XCF qualification.'}
    args.report.parent.mkdir(parents=True, exist_ok=True)
    args.report.write_text(json.dumps(report, indent=2) + '\n')
    if (not all(row['exact'] for row in rows + metadata)
            or records[-1].get('managed_used') != 0):
        raise SystemExit(1)
    print('2/2 native planes and 6/6 layer metadata checks exact; managed usage zero')


if __name__ == '__main__':
    main()
