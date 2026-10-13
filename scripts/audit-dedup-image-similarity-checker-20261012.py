#!/usr/bin/env python3
"""Verify full upstream tree and the seven original-source observations."""
import hashlib
import json
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
DOC = ROOT / 'docs/research/dedup-500'


def sha(path):
    return hashlib.sha256(Path(path).read_bytes()).hexdigest()


def main():
    manifest = DOC / 'image-similarity-checker-full-source-20261012.json'
    review = DOC / 'image-similarity-checker-complete-source-review-20261012.json'
    runtime = DOC / 'image-similarity-checker-runtime-controls-20261012.json'
    inventory = DOC / 'evidence/Imranch4--image-similarity-checker/tree.json'
    data, read, execution, tree = [json.loads(path.read_text()) for path in [manifest, review, runtime, inventory]]
    source = Path(data['source'])
    blobs = {row['path']: row for row in tree['tree'] if row['type'] == 'blob'}
    assert not tree['truncated'] and len(blobs) == 4
    assert set(blobs) == set(read['fully_read_files']) == {row['path'] for row in data['files']}
    pins = dict(execution['pins'])
    for row in data['files']:
        path = source / row['path']
        content = path.read_bytes()
        assert len(content) == row['bytes'] and sha(path) == row['sha256']
        blob = hashlib.sha1(b'blob ' + str(len(content)).encode() + b'\0' + content).hexdigest()
        assert blob == row['git_blob_sha1'] == blobs[row['path']]['sha']
        pins[str(path)] = row['sha256']
    assert execution['status'] == 'complete_original_source7_controls'
    assert execution['upstream_source_modified'] is False
    assert execution['upstream_tests'] == 0 and execution['authored_controls'] == 7
    rows = {row['control']: row for row in execution['controls']}
    assert len(rows) == len(execution['controls']) == 7
    assert rows['alpha_difference_erased']['ssim'] == 1.0
    assert rows['alpha_difference_erased']['phash_distance'] == 0
    assert rows['different_rgb_same_decoded_gray']['ssim'] == 1.0
    assert rows['identical_flat_image_has_zero_orb_matches']['orb_matches'] == 0
    assert rows['missing_input_is_attribute_error']['exception'] == 'AttributeError'
    assert rows['tiny_identical_input_is_ssim_error']['exception'] == 'ValueError'
    assert rows['distinct_original_dimensions_erased']['ssim'] == 1.0
    cli = rows['documented_cli_flags_ignored']
    assert cli['returncode'] == 0 and cli['command'][-4:] == ['--img1', 'absent-left.jpg', '--img2', 'absent-right.jpg']
    assert "Comparing 'before.jpg' and 'after.jpg'" in cli['stdout']
    assert all(sha(path) == digest for path, digest in pins.items())
    for path in [manifest, review, runtime, inventory, Path(__file__)]:
        pins[str(path)] = sha(path)
    output = DOC / 'image-similarity-checker-complete-runtime-audit-20261012.json'
    assert not output.exists()
    output.write_text(json.dumps(dict(status='verified_complete4file_source_review_and7_original_source_controls',
        repository=data['repository'], commit=data['commit'], files=4,
        upstream_tests=0, authored_controls=7, packages=execution['packages'], pins=pins,
        scope=execution['scope']), indent=2) + '\n')
    print('verified4files7controls')


if __name__ == '__main__':
    main()
