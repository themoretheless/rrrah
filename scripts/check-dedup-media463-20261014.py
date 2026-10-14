#!/usr/bin/env python3
"""Synthetic controls against unchanged pinned upstream modules; never user media."""
import hashlib
import importlib.util
import json
from pathlib import Path
import sys
import tempfile
import types

import cv2
import imagehash
import numpy as np
from PIL import Image

SOURCE = Path('/private/tmp/rrrah-media463-source-20261014')
RESULT = Path(__file__).resolve().parents[1] / 'docs/research/dedup-media463-controls-20261014.json'
assert not RESULT.exists()
pins = {p.name: hashlib.sha256(p.read_bytes()).hexdigest() for p in SOURCE.iterdir()}
sys.path.insert(0, str(SOURCE))
import tool

# Only the progress display is adapted; hashing, video, executor and CSV remain upstream.
progress = types.ModuleType('tqdm')
progress.tqdm = lambda iterable, **kwargs: iterable
sys.modules['tqdm'] = progress
spec = importlib.util.spec_from_file_location('upstream_media_main', SOURCE / 'main.py')
main = importlib.util.module_from_spec(spec)
spec.loader.exec_module(main)
checks = []

def check(name, condition):
    assert condition, name
    checks.append(name)

with tempfile.TemporaryDirectory(prefix='rrrah-media463-', dir='/private/tmp') as tmp:
    root = Path(tmp)
    black, white = root / 'black.png', root / 'white.png'
    Image.new('RGB', (32, 32), 'black').save(black)
    Image.new('RGB', (32, 32), 'white').save(white)
    hb, hw = tool.process_file(str(black))[1], tool.process_file(str(white))[1]
    check('real_phash_black_white_distance_one', hb - hw == 1)
    check('strict_threshold_excludes_equal_boundary', not tool.are_images_similar(hb, hw, 1))
    check('threshold_zero_rejects_identical_hash', not tool.are_images_similar(hb, hb, 0))
    check('default_threshold_merges_distinct_solid_images', tool.are_images_similar(hb, hw))
    output = root / 'output'
    unique = main.deduplicate_files([str(black), str(white)], str(output))
    check('real_executor_merges_two_distinct_images', len(unique) == 1)
    csv = output / 'similar_files.csv'
    check('first_call_records_similar_pair', len(csv.read_text().splitlines()) == 2)
    main.deduplicate_files([], str(output), is_video=True)
    check('subsequent_video_call_truncates_image_report', len(csv.read_text().splitlines()) == 1)
    bad = root / 'broken.png'
    bad.write_bytes(b'not an image')
    check('malformed_image_returns_none_hash', tool.process_file(str(bad)) == (str(bad), None))
    check('single_malformed_image_is_unique_with_errors_disabled', main.deduplicate_files([str(bad)], str(output)) == [str(bad)])
    try:
        main.deduplicate_files([str(bad), str(black)], str(output))
    except TypeError:
        check('mixed_valid_invalid_images_raise_type_error', True)
    else:
        raise AssertionError('Expected TypeError regardless of completion order')
    check('include_errors_preserves_bad_image', str(bad) in main.deduplicate_files([str(bad), str(black)], str(output), include_errors=True))
    a, b = root / 'a', root / 'b'
    a.mkdir(); b.mkdir()
    (a / 'same.png').write_bytes(b'first')
    (b / 'same.png').write_bytes(b'second')
    tool.copy_file(str(a / 'same.png'), str(output))
    tool.copy_file(str(b / 'same.png'), str(output))
    check('same_basename_copy_overwrites_existing_bytes', (output / 'same.png').read_bytes() == b'second')
    (a / 'arbitraryjpg').write_bytes(b'x')
    (a / 'UPPER.JPG').write_bytes(b'x')
    paths = tool.merge_target_files([str(a)], ['jpg'])
    check('extension_matching_accepts_missing_dot', str(a / 'arbitraryjpg') in paths)
    check('extension_matching_misses_uppercase', str(a / 'UPPER.JPG') not in paths)
    check('overlapping_roots_duplicate_paths', len(tool.merge_target_files([str(a), str(a)], ['jpg'])) == 2 * len(paths))
    videos = []
    for index in range(2):
        path = root / f'video{index}.avi'
        writer = cv2.VideoWriter(str(path), cv2.VideoWriter_fourcc(*'MJPG'), 5, (32, 32))
        assert writer.isOpened()
        for frame in range(8):
            value = 80 if frame == 4 else index * 255
            writer.write(np.full((32, 32, 3), value, dtype=np.uint8))
        writer.release()
        videos.append(path)
    check('actual_mjpg_videos_have_distinct_bytes', videos[0].read_bytes() != videos[1].read_bytes())
    hashes = [tool.process_file(str(p), is_video=True)[1] for p in videos]
    check('different_videos_with_same_fifth_frame_have_identical_hash', hashes[0] == hashes[1])
    check('real_video_dedup_merges_temporally_distinct_videos', len(main.deduplicate_files([str(p) for p in videos], str(output), is_video=True)) == 1)

assert pins == {p.name: hashlib.sha256(p.read_bytes()).hexdigest() for p in SOURCE.iterdir()}
RESULT.write_text(json.dumps(dict(repository='happy91512/MediaDeduplicationTool', commit='1cefb405db27263eda5d94fa5ea3502fde690841', source=str(SOURCE), source_sha256=pins, checks=checks, count=len(checks), status='passed', adapter='tqdm display replaced by identity iterable only; original __main__ not executed', dependencies=dict(python=sys.version, cv2=cv2.__version__, pillow=Image.__version__, imagehash=imagehash.__version__, numpy=np.__version__), scope='Synthetic local images and actual MJPG/AVI videos. Current installed dependencies; not original Python 3.9 environment, full CLI or user corpus. No upstream tests exist in six-file tree.'), indent=2) + '\n')
print(f'{len(checks)} synthetic controls passed')
