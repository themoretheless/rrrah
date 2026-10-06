#!/usr/bin/env python3
"""Print selected pinned source excerpts for sequential manual/model review."""
import json
import pathlib
import re
import sys

root = pathlib.Path('docs/research/dedup-500')
repositories = json.loads((root / 'repositories.json').read_text())['repositories']
start = int(sys.argv[1]) if len(sys.argv) > 1 else 1
count = int(sys.argv[2]) if len(sys.argv) > 2 else 20
signature = re.compile(r'^\s*(pub\s+)?(async\s+)?(fn|def|class|func|function|struct|enum)\b|hamming|sha256|blake3|md5|phash|dhash|cosine|memcmp|HashAlgorithm|MessageDigest|hashlib|xxhash|murmur|Similarity|simhash|LSH', re.I)
for index in range(start - 1, min(start - 1 + count, len(repositories))):
    repo = repositories[index]
    directory = root / 'evidence' / repo['name'].replace('/', '--')
    manifest = directory / 'manifest.json'
    print(f"\n[{index+1}] {repo['name']} | {repo['language']} | {repo['license']} | {(repo['description'] or '')[:95]}")
    if not manifest.exists():
        print('EVIDENCE NOT READY')
        continue
    data = json.loads(manifest.read_text())
    if len(sys.argv) <= 3: print('commit', data.get('commit'), 'status', data['acquisition_status'], 'sources', data.get('source_file_count'), 'tests', data.get('test_file_count'), 'tree-truncated', data.get('tree_truncated'))
    if data['errors']:
        print('errors', data['errors'])
    if len(sys.argv) > 3 and sys.argv[3] == 'compact':
        print('tests', data.get('test_file_count'), 'files', ', '.join(f['path'] for f in data['files'] if f['source'])[:270])
        for file in [f for f in data['files'] if f['source'] and not f['test']][:3]:
            lines = (directory / file['local']).read_text(errors='replace').splitlines()
            logic = [(n, line) for n,line in enumerate(lines,1) if re.search(r'blake3|hashlib|md5|sha256|phash|dhash|cosine|memcmp|xxhash|bit_count|popcount|count_ones|hamming|faiss|cv2|read_exact|MessageDigest|HashData|ComputeHash',line,re.I) and not re.match(r'\s*(#|//|\*|import |from |use )',line)]
            if not logic:
                logic = [(n,line) for n,line in enumerate(lines,1) if re.search(r'\b(if|return|foreach|while|let|const|var|public|private|def|fn|func)\b',line) and not re.match(r'\s*(#|//|\*|import |from |use )',line)]
            for n,line in logic[:1]: print(file['path']+':'+str(n)+': '+line.strip()[:170])
        continue
    for file in data['files']:
        if not file['source']:
            continue
        print('FILE', file['path'], 'lines', file['lines'], 'truncated', file['truncated'], 'test', file['test'])
        lines = (directory / file['local']).read_text(errors='replace').splitlines()
        hits = [(number, line) for number, line in enumerate(lines, 1) if signature.search(line)]
        bodies = [(n, line) for n, line in enumerate(lines, 1) if re.search(r'\b(if|return|foreach|while|let|const|var|public|private)\b', line) and not re.match(r'\s*(#|//|\*|import |from |use |package |namespace)', line)]
        selected = hits[:4] if hits else bodies[:4]
        for number, line in selected:
            print(f'{number}: {line[:95]}')
        logic = [(n, line) for n, line in enumerate(lines, 1) if re.search(r'blake3|hashlib|md5|sha256|phash|dhash|cosine|memcmp|xxhash|murmur|bit_count|popcount|count_ones|hamming|torch|faiss|cv2|read_exact', line, re.I) and not re.match(r'\s*(#|//|\*|from |import |use |def |class |fn )', line)]
        for n, line in logic[:3]:
            if n not in {v[0] for v in selected}: print(f'{n}: {line[:95]}')
