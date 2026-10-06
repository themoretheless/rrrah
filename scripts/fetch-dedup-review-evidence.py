#!/usr/bin/env python3
"""Fetch pinned source evidence for all registry entries, without executing it.

This prepares source review; it does not claim that a model/person reviewed it.
Reruns reuse completed evidence and retry incomplete acquisitions.
"""
import concurrent.futures
import hashlib
import json
import pathlib
import re
import subprocess
import time
import urllib.error
import urllib.parse
import urllib.request

ROOT = pathlib.Path('docs/research/dedup-500')
EVIDENCE = ROOT / 'evidence'
SOURCE_EXT = {'.rs', '.py', '.go', '.c', '.cpp', '.h', '.hpp', '.js', '.ts',
              '.cs', '.java', '.swift', '.kt', '.rb', '.php', '.sh', '.hs',
              '.clj', '.pl', '.ex', '.scala', '.jl', '.nim', '.zig', '.lisp',
              '.lua', '.sql', '.lean', '.vhd', '.groovy', '.ipynb', '.ps1', '.pyx', '.erl', '.tsx', '.jsx', '.svelte', '.jl', '.sp', '.j', '.tex', '.m', '.mm', '.dart', '.fut', '.ijs', '.html', '.css', '.eo', '.pyw', '.cljs', '.fs'}
SIGNAL = re.compile(r'dup|hash|similar|fingerprint|compare|bktree|bk_tree|scan|match|distance|retriev', re.I)
EXCLUDE = re.compile(r'(^|/)(node_modules|vendor|third_party|external|\.git|dist|build|docs)/', re.I)

def api(endpoint):
    last = None
    for attempt in range(3):
        proc = subprocess.run(['gh', 'api', endpoint], capture_output=True, text=True, timeout=90)
        if proc.returncode == 0:
            return json.loads(proc.stdout)
        last = proc.stderr[-600:]
        time.sleep(2 + attempt * 3)
    raise RuntimeError(last)

def raw(repo, commit, path):
    url = f'https://raw.githubusercontent.com/{repo}/{commit}/' + urllib.parse.quote(path)
    for attempt in range(3):
        try:
            request = urllib.request.Request(url, headers={'User-Agent': 'rrrah-source-review'})
            with urllib.request.urlopen(request, timeout=40) as response:
                data = response.read(262145)
            return data[:262144], len(data) > 262144
        except (OSError, urllib.error.URLError):
            if attempt == 2:
                raise
            time.sleep(2 + attempt)

def score(path):
    low = path.lower()
    basename = pathlib.PurePosixPath(low).name
    return (12 * bool(SIGNAL.search(basename)) + 4 * bool(SIGNAL.search(low))
            + 3 * ('/src/' in '/' + low) + 2 * ('/core/' in '/' + low)
            - low.count('/') - 5 * bool(is_test(low))
            - 25 * bool(re.search(r'__init__|setup\.|\.d\.ts$|extconf|version|config|formatter|converter|stateholder| copying\.', low))
            - 15 * bool(re.search(r'appdelegate|constants|\.h$|\.hpp$|worker|service|result|\.generated\.', basename))
            + 15 * bool(re.search(r'^((duplicate|dedup|similar|image|file)?(finder|detector|hasher|scanner|compare)|phash|dhash|hash|dedup|dup|compare)\.', basename)))

def is_test(path):
    return bool(re.search(r'(^|/)(tests?|specs?)(/|\.)|(_test|\.test|_spec|\.spec)\.', path, re.I))

def choose(paths):
    sources = [p for p in paths if (pathlib.PurePosixPath(p).suffix.lower() in SOURCE_EXT or
               (not pathlib.PurePosixPath(p).suffix and (SIGNAL.search(pathlib.PurePosixPath(p).name) or pathlib.PurePosixPath(p).name in {'cidrmerge'})))
               and not EXCLUDE.search(p)]
    tests = sorted([p for p in sources if is_test(p)], key=lambda p: (-score(p), p))
    algorithms = sorted([p for p in sources if p not in tests], key=lambda p: (-score(p), p))
    selected = algorithms[:3] + tests[:1]
    docs = sorted([p for p in paths if pathlib.PurePosixPath(p).name.lower().startswith('readme')], key=lambda p: (p.count('/'), p))[:1]
    licenses = sorted([p for p in paths if re.match(r'license|licence|copying', pathlib.PurePosixPath(p).name, re.I)], key=lambda p: (p.count('/'), p))[:2]
    return list(dict.fromkeys(docs + licenses + selected)), sources, tests

def fetch(repo):
    name = repo['name']
    destination = EVIDENCE / name.replace('/', '--')
    manifest = destination / 'manifest.json'
    if manifest.exists():
        old = json.loads(manifest.read_text())
        if old.get('acquisition_status') == 'ready':
            return name, 'cached'
    destination.mkdir(parents=True, exist_ok=True)
    result = {'repository': name, 'metadata': repo, 'review_status': 'awaiting-source-review', 'files': [], 'errors': []}
    try:
        commit = api(f"repos/{name}/commits/{urllib.parse.quote(repo['default_branch'], safe='')}")
        sha = commit['sha']
        result['commit'] = sha
        tree_sha = commit['commit']['tree']['sha']
        tree = api(f'repos/{name}/git/trees/{tree_sha}?recursive=1')
        result['tree_truncated'] = tree.get('truncated', False)
        paths = [item['path'] for item in tree['tree'] if item['type'] == 'blob']
        (destination / 'tree.json').write_text(json.dumps(tree, indent=2) + '\n')
        selected, sources, tests = choose(paths)
        result['source_file_count'] = len(sources)
        result['test_file_count'] = len(tests)
        for index, path in enumerate(selected):
            try:
                content, truncated = raw(name, sha, path)
                text = content.decode('utf-8', errors='replace')
                local = f'{index:02d}-' + pathlib.PurePosixPath(path).name
                (destination / local).write_bytes(content)
                result['files'].append({'path': path, 'local': local,
                    'url': f'https://github.com/{name}/blob/{sha}/{path}',
                    'sha256': hashlib.sha256(content).hexdigest(), 'truncated': truncated,
                    'lines': len(text.splitlines()), 'source': path in sources, 'test': path in tests})
            except Exception as error:
                result['errors'].append({'path': path, 'error': str(error)})
        result['acquisition_status'] = 'ready' if not result['errors'] else 'partial'
    except Exception as error:
        result['errors'].append({'error': str(error)})
        result['acquisition_status'] = 'failed'
    manifest.write_text(json.dumps(result, indent=2) + '\n')
    return name, result['acquisition_status']

def main():
    repositories = json.loads((ROOT / 'repositories.json').read_text())['repositories']
    EVIDENCE.mkdir(parents=True, exist_ok=True)
    counts = {}
    with concurrent.futures.ThreadPoolExecutor(max_workers=8) as executor:
        futures = [executor.submit(fetch, repo) for repo in repositories]
        for number, future in enumerate(concurrent.futures.as_completed(futures), 1):
            name, status = future.result()
            counts[status] = counts.get(status, 0) + 1
            print(f'{number}/500 {name} {status}', flush=True)
    print(json.dumps(counts), flush=True)

if __name__ == '__main__':
    main()
