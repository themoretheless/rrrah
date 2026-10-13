#!/usr/bin/env python3
"""Original Python2 function bodies with explicit Python3 and hash adapters."""
import ast
import contextlib
import hashlib
import io
import itertools
import json
import logging
import os
import pickle
import select
import shutil
import signal
import sys
from pathlib import Path
from types import SimpleNamespace

ROOT = Path(__file__).resolve().parents[1]
DOC = ROOT / 'docs/research/dedup-500'
TMP = Path('/Users/themoretheless/.codex/tmp')


class LegacyDict(dict):
    def viewitems(self):
        return self.items()

    def viewvalues(self):
        return self.values()


class HashAdapter:
    def __init__(self):
        self.calls = []

    def dct_imagehash(self, path):
        self.calls.append(path)
        data = Path(path).read_bytes()
        if data == b'bad':
            return None
        if data in [b'1', b'3', b'15']:
            return int(data)
        return int.from_bytes(hashlib.sha256(data).digest()[:8], 'big')

    def hamming_distance(self, left, right):
        return bin(left ^ right).count('1')


def sha(path):
    return hashlib.sha256(Path(path).read_bytes()).hexdigest()


def main():
    manifest = DOC / 'image-deduplication-tool-full-source-20261012.json'
    data = json.loads(manifest.read_text())
    source = Path(data['source'])
    assert all(sha(source / row['path']) == row['sha256'] for row in data['files'])
    original = source / 'image_matcher.py'
    parsed = ast.parse(original.read_text())
    functions = [node for node in parsed.body if isinstance(node, ast.FunctionDef)]
    assert {node.name for node in functions} == {'update_dcts', 'sort_by_similarity', 'main'}
    provider = HashAdapter()
    namespace = dict(os=os, sys=sys, pickle=pickle, select=select, signal=signal,
        dict=LegacyDict, xrange=range, phash=provider, pHash=HashAdapter,
        log=logging.getLogger('bounded-upstream-controls'),
        op=__import__('operator'), it=SimpleNamespace(imap=map, chain=itertools.chain,
            combinations=itertools.combinations, groupby=itertools.groupby))
    exec(compile(ast.Module(body=functions, type_ignores=[]), str(original), 'exec'), namespace)
    update, ordered = namespace['update_dcts'], namespace['sort_by_similarity']
    directory = TMP / 'rrrah-image-deduplication-tool-controls-v2-20261012'
    output = DOC / 'image-deduplication-tool-runtime-controls-20261012.json'
    assert not directory.exists() and not output.exists()
    directory.mkdir()
    folder = directory / 'images'
    folder.mkdir()
    image = folder / 'a.bin'
    image.write_bytes(b'A')
    cache = LegacyDict()
    update(cache, [str(folder)])
    before, calls = cache[str(image)], len(provider.calls)
    image.write_bytes(b'B')
    update(cache, [str(folder)])
    assert cache[str(image)] == before and len(provider.calls) == calls
    assert before != HashAdapter().dct_imagehash(str(image))
    rows = [dict(control='changed_bytes_reuse_old_path_hash', hash_calls=calls)]
    image.write_bytes(b'bad')
    failed = LegacyDict()
    update(failed, [str(folder)])
    calls = len(provider.calls)
    assert failed[str(image)] is None
    image.write_bytes(b'repaired')
    update(failed, [str(folder)])
    assert failed[str(image)] is None and len(provider.calls) == calls
    rows.append(dict(control='failed_hash_is_not_retried_after_repair'))
    direct = LegacyDict({str(image): 1})
    update(direct, [str(image)])
    assert direct == {}
    rows.append(dict(control='documented_direct_file_input_is_ignored_and_evicted'))
    missing = LegacyDict({str(image): 1})
    update(missing, [str(directory / 'missing-root')])
    assert missing == {}
    rows.append(dict(control='missing_root_succeeds_and_clears_cache'))
    overlap = LegacyDict()
    calls = len(provider.calls)
    update(overlap, [str(folder), str(folder)])
    assert len(provider.calls) == calls + 1 and len(overlap) == 1
    rows.append(dict(control='overlapping_roots_hash_each_path_once'))
    image.unlink()
    update(overlap, [str(folder)])
    assert overlap == {}
    rows.append(dict(control='deleted_paths_are_evicted'))
    assert list(ordered(LegacyDict({'a': 0, 'b': 0}))) == []
    rows.append(dict(control='zero_hash_pairs_are_skipped'))
    for name, content in [('a.bin', b'1'), ('b.bin', b'3'), ('c.bin', b'15')]:
        (folder / name).write_bytes(content)
    previous_argv = sys.argv
    transcript = io.StringIO()
    try:
        sys.argv = [str(original), str(folder), '-p', '1', '-n', '1', '--hash-db', str(directory / 'hashes.db')]
        with contextlib.redirect_stdout(transcript):
            namespace['main']()
    finally:
        sys.argv = previous_argv
    lines = transcript.getvalue().splitlines()
    assert len(lines) == 3 and [int(line.rsplit(' ', 1)[1]) for line in lines] == [1, 2, 3]
    rows.append(dict(control='top_n1_prints_three_distance_tiers', lines=lines))
    assert len(rows) == 8
    (directory / 'controls.stdout').write_text(transcript.getvalue())
    pins = {str(path): sha(path) for path in [manifest, original, Path(__file__), *directory.rglob('*')] if path.is_file()}
    output.write_text(json.dumps(dict(status='complete_original_functions8_python3_hash_adapter_controls',
        authored_controls=8, upstream_tests=0, controls=rows, pins=pins,
        python2_on_path=shutil.which('python2'),
        extraction='Unchanged3FunctionDef bodies; Python3 imap/xrange/viewitems/viewvalues adapters, mocked hash provider, serial mode. No native hashing/fork/viewer execution.',
        scope='Bounded path-cache, enumeration and CLI behavior with original function bodies and adapters. No exact Python2/libpHash environment, image-hash precision, asynchronous child lifecycle or feh/delete qualification. No adoption.'), indent=2) + '\n')
    print('8 original-function adapter controls passed')


if __name__ == '__main__':
    main()
