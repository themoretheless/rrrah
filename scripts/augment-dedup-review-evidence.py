#!/usr/bin/env python3
"""Refine source selection using cached pinned trees; never rerun upstream code."""
import concurrent.futures
import hashlib
import importlib.util
import json
import pathlib

spec = importlib.util.spec_from_file_location('fetch_evidence', pathlib.Path(__file__).with_name('fetch-dedup-review-evidence.py'))
fetch = importlib.util.module_from_spec(spec)
spec.loader.exec_module(fetch)

def augment(manifest):
    data = json.loads(manifest.read_text())
    tree_path = manifest.parent / 'tree.json'
    if not tree_path.exists():
        return data['repository'], 'missing-tree'
    tree = json.loads(tree_path.read_text())
    paths = [item['path'] for item in tree['tree'] if item['type'] == 'blob']
    selected, sources, tests = fetch.choose(paths)
    data['source_file_count'] = len(sources)
    data['test_file_count'] = len(tests)
    existing = {file['path'] for file in data['files']}
    for path in selected:
        if path in existing:
            continue
        try:
            content, truncated = fetch.raw(data['repository'], data['commit'], path)
            local = f"{len(data['files']):02d}-" + pathlib.PurePosixPath(path).name
            text = content.decode('utf-8', errors='replace')
            (manifest.parent / local).write_bytes(content)
            data['files'].append({'path': path, 'local': local,
                'url': f"https://github.com/{data['repository']}/blob/{data['commit']}/{path}",
                'sha256': hashlib.sha256(content).hexdigest(), 'truncated': truncated,
                'lines': len(text.splitlines()), 'source': path in sources, 'test': path in tests})
        except Exception as error:
            data.setdefault('augmentation_errors', []).append({'path': path, 'error': str(error)})
    for file in data['files']:
        file['source'] = file['path'] in sources
        file['test'] = file['path'] in tests
    data['selected_paths_v2'] = selected
    manifest.write_text(json.dumps(data, indent=2) + '\n')
    return data['repository'], 'augmented'

if __name__ == '__main__':
    manifests = sorted(fetch.EVIDENCE.glob('*/manifest.json'))
    with concurrent.futures.ThreadPoolExecutor(max_workers=8) as pool:
        for number, (name, status) in enumerate(pool.map(augment, manifests), 1):
            print(f'{number}/{len(manifests)} {name} {status}', flush=True)
