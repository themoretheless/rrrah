#!/usr/bin/env python3
"""Bounded original-class controls; imports/CLI and progress UI are excluded."""
import ast
import contextlib
import hashlib
import importlib.metadata
import io
import json
from collections import defaultdict
from pathlib import Path

import imagehash
from PIL import Image

ROOT = Path(__file__).resolve().parents[1]
DOC = ROOT / 'docs/research/dedup-500'
TMP = Path('/Users/themoretheless/.codex/tmp')


def sha(path):
    return hashlib.sha256(Path(path).read_bytes()).hexdigest()


def main():
    manifest = DOC / 'ai-smart-photo-dedup-full-source-20261012.json'
    data = json.loads(manifest.read_text())
    source = Path(data['source'])
    assert all(sha(source / row['path']) == row['sha256'] for row in data['files'])
    original = source / 'dedup.py'
    parsed = ast.parse(original.read_text())
    classes = [node for node in parsed.body if isinstance(node, ast.ClassDef)]
    assert len(classes) == 1 and classes[0].name == 'PhotoDeduplicator'
    namespace = dict(defaultdict=defaultdict, Path=Path, imagehash=imagehash,
                     Image=Image, tqdm=lambda values, **_: values)
    exec(compile(ast.Module(body=classes, type_ignores=[]), str(original), 'exec'), namespace)
    Dedup = namespace['PhotoDeduplicator']
    directory = TMP / 'rrrah-ai-smart-photo-dedup-controls-20261012'
    output = DOC / 'ai-smart-photo-dedup-runtime-controls-20261012.json'
    assert not directory.exists() and not output.exists()
    directory.mkdir()
    rows = []
    transcript = io.StringIO()

    def fixture(name, colors):
        folder = directory / name
        folder.mkdir()
        for filename, color in colors:
            Image.new('RGB', (32, 32), color).save(folder / filename)
        return folder

    with contextlib.redirect_stdout(transcript):
        triple = fixture('triple', [('a.png', 'white'), ('b.png', 'white'), ('c.png', 'white')])
        report = Dedup().find_duplicates(triple)
        pair_count = sum(map(len, report.values()))
        unique_right_paths = {path for paths in report.values() for path in paths}
        assert pair_count == 3 and len(unique_right_paths) == 2
        rows.append(dict(control='pair_edges_counted_as_duplicate_images', edge_count=3, unique_right_paths=2))
        copies = fixture('repeated', [('a.png', 'white'), ('b.png', 'white')])
        instance = Dedup()
        first = instance.find_duplicates(copies)
        assert sum(map(len, first.values())) == 1
        second = instance.find_duplicates(copies)
        assert sum(map(len, second.values())) == 2
        rows.append(dict(control='repeated_scan_accumulates_edges', first_edges=1, second_edges=2))
        assert sum(map(len, first.values())) == 2
        rows.append(dict(control='prior_returned_report_aliases_mutable_lists', prior_returned_edges_now=2))
        referenced = next(iter(second.values()))[0]
        Path(referenced).unlink()
        third = instance.find_duplicates(copies)
        assert any(referenced in paths for paths in third.values()) and not Path(referenced).exists()
        rows.append(dict(control='removed_file_remains_in_next_scan_report', missing_path=referenced))
        missing = Dedup().find_duplicates(directory / 'absent-root')
        assert missing == {}
        rows.append(dict(control='missing_root_is_successful_empty_report', report=missing))
        corrupt = directory / 'corrupt'
        corrupt.mkdir()
        (corrupt / 'broken.png').write_bytes(b'not an image')
        assert Dedup().find_duplicates(corrupt) == {}
        rows.append(dict(control='decode_failure_is_dropped_from_result', report={}))
        flat = fixture('flat', [('black.png', 'black'), ('white.png', 'white')])
        assert Image.open(flat / 'black.png').tobytes() != Image.open(flat / 'white.png').tobytes()
        flat_result = Dedup().find_duplicates(flat)
        assert sum(map(len, flat_result.values())) == 1
        rows.append(dict(control='opaque_black_white_classified_duplicate', accepted_edges=1))
        invalid_result = Dedup(similarity_threshold=2.0).find_duplicates(triple)
        assert invalid_result == {}
        rows.append(dict(control='invalid_threshold_accepted_and_hides_identical_copies', threshold=2.0, report={}))
    log = directory / 'controls.stdout'
    log.write_text(transcript.getvalue())
    pins = {str(path): sha(path) for path in [manifest, original, Path(__file__), log]}
    for path in directory.rglob('*'):
        if path.is_file():
            pins[str(path)] = sha(path)
    assert len(rows) == 8
    output.write_text(json.dumps(dict(status='complete_original_class8_compatible_dependency_controls',
        controls=rows, upstream_tests=0, authored_controls=8,
        packages={name: importlib.metadata.version(name) for name in ['ImageHash', 'Pillow']},
        extraction='Unmodified original ClassDef AST; module imports/main excluded; tqdm replaced by identity progress adapter.',
        pins=pins, scope='Original class logic exercised with ImageHash4.3.2/Pillow11.3.0, not original requirement versions. No full-module/CLI or exact-upstream-environment qualification, no general precision estimate or adoption.'), indent=2) + '\n')
    print('8 original-class compatible-dependency controls passed')


if __name__ == '__main__':
    main()
