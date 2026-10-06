#!/usr/bin/env python3
"""Verify all 500 review records and the bytes of their archived source evidence.

Integrity/coverage checking does not certify the correctness of review findings.
"""
import collections
import hashlib
import json
import pathlib
import re

ROOT = pathlib.Path('docs/research/dedup-500')

def verify():
    registry = json.loads((ROOT / 'repositories.json').read_text())
    repositories = registry['repositories']
    assert registry['count'] == len(repositories) == 500
    assert len({repo['name'].lower() for repo in repositories}) == 500
    assert len(list((ROOT / 'reviews').glob('*.json'))) == 500
    result = collections.Counter()
    for number, repository in enumerate(repositories, 1):
        directory = ROOT / 'evidence' / repository['name'].replace('/', '--')
        manifest = json.loads((directory / 'manifest.json').read_text())
        review = json.loads((ROOT / 'reviews' / f'{number:03d}.json').read_text())
        assert review['index'] == number
        assert review['repository'] == manifest['repository'] == repository['name']
        assert review['commit'] == manifest['commit']
        assert re.fullmatch('[0-9a-f]{40}', review['commit'])
        assert review['finding'] and review['review_scope'] and review['decision']
        assert review['upstream_tests_run'] is False
        assert manifest['acquisition_status'] == 'ready'
        assert not manifest['errors']
        assert not manifest.get('augmentation_errors')
        assert (directory / 'tree.json').exists()
        result['repositories'] += 1
        result['reviews'] += 1
        result['source_absent'] += manifest['source_file_count'] == 0
        result['truncated_trees'] += manifest.get('tree_truncated', False)
        for file in manifest['files']:
            local = directory / file['local']
            assert local.parent == directory
            assert hashlib.sha256(local.read_bytes()).hexdigest() == file['sha256'], str(local)
            assert manifest['commit'] in file['url']
            result['files'] += 1
            result['truncated_files'] += file['truncated']
    return dict(result)

if __name__ == '__main__':
    print(json.dumps(verify(), indent=2))
