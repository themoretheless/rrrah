#!/usr/bin/env python3
"""Reproducible public, non-fork repository discovery with GitHub metadata.

Metadata discovery is not a source-code audit or license compliance conclusion.
"""
import datetime
import json
import pathlib
import re
import subprocess
import time

OUT = pathlib.Path('docs/research/dedup-500')
QUERIES = [
    'duplicate in:name,description',
    'duplicates in:name,description',
    'dedup in:name,description',
    'dupe in:name,description',
    'perceptual hash in:name,description',
    'image deduplication in:name,description',
    'near duplicate image in:name,description',
    'image similarity in:name,description',
    'content hashing in:name,description',
    'hamming distance in:name,description',
    'image retrieval in:name,description',
    'phash in:name,description',
    'dhash in:name,description',
    'imagehash in:name,description',
    'similar image in:name,description',
    'duplicate photo in:name,description',
    'duplicate file in:name,description',
    'near duplicate in:name,description',
    'content deduplication in:name,description',
    'image hashing in:name,description',
    'photo similarity in:name,description',
    'duplicate images in:name,description',
    'duplicate files in:name,description',
    'deduplication in:name,description',
    'fdupes in:name,description',
    'jdupes in:name,description',
    'photo dedup in:name,description',
    'image fingerprint in:name,description',
]

def main():
    OUT.mkdir(parents=True, exist_ok=True)
    collected = {}
    for qi, query in enumerate(QUERIES):
        for page in range(1, 4):
            snapshot = OUT / f'targeted-query-{qi:02d}-page-{page}.json'
            if snapshot.exists():
                data = json.loads(snapshot.read_text())
            else:
                result = subprocess.run([
                    'gh', 'api', '-X', 'GET', 'search/repositories',
                    '-f', 'q=' + query + ' fork:false is:public',
                    '-f', 'sort=stars', '-f', 'order=desc',
                    '-f', 'per_page=100', '-f', f'page={page}',
                ], capture_output=True, text=True)
                if result.returncode:
                    print(result.stderr, flush=True)
                    return 1
                data = json.loads(result.stdout)
                if data.get('incomplete_results'):
                    raise RuntimeError('Incomplete GitHub search; rerun query')
                snapshot.write_text(json.dumps(data, indent=2) + '\n')
                time.sleep(2.2)
            for repo in data['items']:
                context = (repo['full_name'] + ' ' + (repo.get('description') or '')).lower()
                if not re.search(r'file|image|photo|picture|perceptual|hash|folder|directory|media|storage|dedup|dupe|fingerprint|hamming', context):
                    continue
                if re.search(r'awesome|curated|interview|leetcode|system.design|tutorial|course|cheatsheet', context):
                    continue
                license_info = repo.get('license') or {}
                spdx = license_info.get('spdx_id')
                if not spdx or spdx == 'NOASSERTION':
                    continue
                name = repo['full_name']
                if name in collected:
                    collected[name]['queries'].append(query)
                    continue
                collected[name] = {
                    'name': name, 'url': repo['html_url'],
                    'description': repo['description'], 'language': repo['language'],
                    'stars': repo['stargazers_count'], 'license': spdx,
                    'archived': repo['archived'], 'pushed_at': repo['pushed_at'],
                    'default_branch': repo['default_branch'], 'queries': [query],
                    'review_status': 'metadata-only',
                }
            ordered = list(collected.values())
            payload = {
                'retrieved_at': datetime.datetime.now(datetime.timezone.utc).isoformat(),
                'method': 'GitHub search; public non-forks with recognized SPDX metadata',
                'count': len(ordered[:500]), 'repositories': ordered[:500],
            }
            (OUT / 'repositories.json').write_text(json.dumps(payload, indent=2) + '\n')
            print(f'query={qi} page={page} unique licensed repositories={len(collected)}', flush=True)
            if len(collected) >= 500:
                return 0
            if len(data['items']) < 100:
                break
    raise RuntimeError(f'Only {len(collected)} repositories; extend targeted queries')

if __name__ == '__main__':
    raise SystemExit(main())
