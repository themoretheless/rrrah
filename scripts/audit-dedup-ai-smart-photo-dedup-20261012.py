#!/usr/bin/env python3
"""Verify full source review and explicitly bounded compatible-class controls."""
import hashlib
import json
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
DOC = ROOT / 'docs/research/dedup-500'


def sha(path):
    return hashlib.sha256(Path(path).read_bytes()).hexdigest()


def main():
    paths = [DOC / name for name in ['ai-smart-photo-dedup-full-source-20261012.json',
        'ai-smart-photo-dedup-complete-source-review-20261012.json',
        'ai-smart-photo-dedup-runtime-controls-20261012.json',
        'ai-smart-photo-dedup-upstream-runtime-preparation-20261012.json']]
    manifest, review, runtime, preparation = [json.loads(path.read_text()) for path in paths]
    tree_path = DOC / 'evidence/maweidong--ai-smart-photo-dedup/tree.json'
    tree = json.loads(tree_path.read_text())
    blobs = {row['path']: row for row in tree['tree'] if row['type'] == 'blob'}
    assert not tree['truncated'] and len(blobs) == 4
    assert set(blobs) == set(review['fully_read_files']) == {row['path'] for row in manifest['files']}
    pins = dict(runtime['pins'])
    for row in manifest['files']:
        path = Path(manifest['source']) / row['path']
        content = path.read_bytes()
        assert len(content) == row['bytes'] and sha(path) == row['sha256']
        assert hashlib.sha1(b'blob ' + str(len(content)).encode() + b'\0' + content).hexdigest() == row['git_blob_sha1'] == blobs[row['path']]['sha']
        pins[str(path)] = row['sha256']
    assert runtime['status'] == 'complete_original_class8_compatible_dependency_controls'
    assert runtime['upstream_tests'] == 0 and runtime['authored_controls'] == 8
    controls = {row['control']: row for row in runtime['controls']}
    assert len(controls) == len(runtime['controls']) == 8
    assert controls['pair_edges_counted_as_duplicate_images']['edge_count'] == 3
    assert controls['pair_edges_counted_as_duplicate_images']['unique_right_paths'] == 2
    assert controls['repeated_scan_accumulates_edges']['second_edges'] == 2
    assert controls['prior_returned_report_aliases_mutable_lists']['prior_returned_edges_now'] == 2
    assert not Path(controls['removed_file_remains_in_next_scan_report']['missing_path']).exists()
    assert controls['missing_root_is_successful_empty_report']['report'] == {}
    assert controls['decode_failure_is_dropped_from_result']['report'] == {}
    assert controls['opaque_black_white_classified_duplicate']['accepted_edges'] == 1
    assert controls['invalid_threshold_accepted_and_hides_identical_copies']['threshold'] == 2.0
    assert preparation['status'] == 'terminal_original_requirements_failure' and preparation['returncode'] != 0
    dependency_log = Path(preparation['log'])
    assert 'No matching distribution found for imagehash==5.3.0' in dependency_log.read_text()
    assert runtime['packages'] == {'ImageHash': '4.3.2', 'Pillow': '11.3.0'}
    assert all(sha(path) == digest for path, digest in pins.items())
    for path in [*paths, tree_path, dependency_log, Path(__file__)]:
        pins[str(path)] = sha(path)
    output = DOC / 'ai-smart-photo-dedup-complete-runtime-audit-20261012.json'
    assert not output.exists()
    output.write_text(json.dumps(dict(status='verified_complete4file_review_and8_compatible_class_controls',
        repository=manifest['repository'], commit=manifest['commit'], files=4,
        exact_upstream_environment_qualified=False, original_requirement_install_returncode=preparation['returncode'],
        upstream_tests=0, authored_controls=8, extraction=runtime['extraction'],
        pins=pins, scope=runtime['scope']), indent=2) + '\n')
    print('verified4files8compatible_class_controls; original requirements unavailable')


if __name__ == '__main__':
    main()
