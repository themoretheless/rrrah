#!/usr/bin/env python3
"""Exercise unchanged upstream functions on explicit source distinctions."""
import hashlib
import importlib.metadata
import json
import runpy
import subprocess
import sys
from pathlib import Path

import cv2
import numpy as np
from PIL import Image

ROOT = Path(__file__).resolve().parents[1]
DOC = ROOT / 'docs/research/dedup-500'
TMP = Path('/Users/themoretheless/.codex/tmp')


def sha(path):
    return hashlib.sha256(Path(path).read_bytes()).hexdigest()


def main():
    manifest = DOC / 'image-similarity-checker-full-source-20261012.json'
    data = json.loads(manifest.read_text())
    source = Path(data['source'])
    assert all(sha(source / row['path']) == row['sha256'] for row in data['files'])
    original = source / 'image-similarity.py'
    functions = runpy.run_path(str(original), run_name='upstream_controls')
    directory = TMP / 'rrrah-image-similarity-checker-controls-20261012'
    output = DOC / 'image-similarity-checker-runtime-controls-20261012.json'
    assert not directory.exists() and not output.exists()
    directory.mkdir()

    def picture(name, color, size=(32, 32), mode='RGBA'):
        path = directory / name
        Image.new(mode, size, color).save(path)
        return str(path)

    opaque = picture('opaque.png', (255, 0, 0, 255))
    transparent = picture('transparent.png', (255, 0, 0, 0))
    assert Image.open(opaque).tobytes() != Image.open(transparent).tobytes()
    alpha_ssim = functions['ssim_compare'](opaque, transparent)
    alpha_phash = functions['phash_compare'](opaque, transparent)
    assert alpha_ssim == 1.0 and alpha_phash == 0
    rows = [dict(control='alpha_difference_erased', ssim=alpha_ssim, phash_distance=alpha_phash)]
    green = picture('green.png', (0, 130, 0, 255))
    assert Image.open(opaque).tobytes() != Image.open(green).tobytes()
    assert np.array_equal(cv2.imread(opaque, cv2.IMREAD_GRAYSCALE), cv2.imread(green, cv2.IMREAD_GRAYSCALE))
    gray_ssim = functions['ssim_compare'](opaque, green)
    assert gray_ssim == 1.0
    rows.append(dict(control='different_rgb_same_decoded_gray', ssim=gray_ssim))
    same_orb = functions['orb_match'](opaque, opaque)
    assert same_orb == 0
    rows.append(dict(control='identical_flat_image_has_zero_orb_matches', orb_matches=same_orb))
    for name, left, right, expected in [
        ('missing_input_is_attribute_error', str(directory / 'missing.png'), opaque, AttributeError),
        ('tiny_identical_input_is_ssim_error', picture('tiny.png', (255, 0, 0, 255), (3, 3)),
         str(directory / 'tiny.png'), ValueError),
    ]:
        try:
            functions['ssim_compare'](left, right)
        except expected as error:
            rows.append(dict(control=name, exception=type(error).__name__, message=str(error)))
        else:
            raise AssertionError(name)
    wide = picture('wide.png', (255, 0, 0, 255), (64, 16))
    tall = picture('tall.png', (255, 0, 0, 255), (16, 64))
    aspect_ssim = functions['ssim_compare'](wide, tall)
    assert aspect_ssim == 1.0
    rows.append(dict(control='distinct_original_dimensions_erased', left_dimensions=[64, 16],
                     right_dimensions=[16, 64], ssim=aspect_ssim))
    picture('before.jpg', (200, 20, 10), mode='RGB')
    picture('after.jpg', (200, 20, 10), mode='RGB')
    command = [sys.executable, str(original), '--img1', 'absent-left.jpg', '--img2', 'absent-right.jpg']
    cli = subprocess.run(command, cwd=directory, capture_output=True, text=True)
    assert cli.returncode == 0 and "Comparing 'before.jpg' and 'after.jpg'" in cli.stdout
    rows.append(dict(control='documented_cli_flags_ignored', command=command, returncode=0,
                     stdout=cli.stdout, stderr=cli.stderr))
    assert len(rows) == 7
    pins = {str(path): sha(path) for path in [manifest, original, Path(__file__), *directory.iterdir()]}
    packages = {name: importlib.metadata.version(name) for name in
                ['Pillow', 'ImageHash', 'opencv-python-headless', 'numpy', 'scikit-image', 'scipy']}
    output.write_text(json.dumps(dict(status='complete_original_source7_controls',
        upstream_source_modified=False, upstream_tests=0, authored_controls=7,
        controls=rows, packages=packages, interpreter=sys.executable, pins=pins,
        scope='Observed seven bounded scorer/CLI behaviors in original pinned source. Does not estimate general precision or assert these scores are upstream duplicate decisions; source has no duplicate classifier. No code adopted.'), indent=2) + '\n')
    print('7 original-source controls passed')


if __name__ == '__main__':
    main()
