#!/usr/bin/env python3
"""Independent TIFF page fixtures; run in an isolated Python environment.
Versions used are recorded in manifest.json. Rust tests consume saved files and
require no Python dependencies. Generated pixel data are dedicated to CC0.
"""
from pathlib import Path
import hashlib
import json
import numpy as np
import tifffile
import imagecodecs
import PIL
from PIL import ImageCms

root = Path(__file__).resolve().parents[1] / 'crates/rrrah-dedup/tests/fixtures/tiff'
root.mkdir(parents=True, exist_ok=True)
profile = ImageCms.ImageCmsProfile(ImageCms.createProfile('sRGB')).tobytes()
data = np.fromfunction(lambda page, y, x, c: (page * 53 + y * 37 + x * 19 + c * 71) % 256, (3, 4, 5, 3), dtype=int).astype(np.uint8)
entries = []
for big in [False, True]:
    for order, label in [('<', 'little'), ('>', 'big')]:
        for compression in [None, 'deflate', 'lzw', 'packbits']:
            for changed in [False, True]:
                pixels = data.copy()
                if changed:
                    pixels[-1, 2, 3, 0] ^= 128
                name = f'{"bigtiff" if big else "classic"}-{label}-{compression or "none"}-{"changed" if changed else "same"}.tif'
                path = root / name
                with tifffile.TiffWriter(path, bigtiff=big, byteorder=order) as writer:
                    for page in pixels:
                        writer.write(page, photometric='rgb', metadata=None, compression=compression, iccprofile=profile)
                decoded = tifffile.imread(path, key=range(3))
                assert np.array_equal(decoded, pixels)
                entries.append({'file': name, 'changed': changed, 'sha256': hashlib.sha256(path.read_bytes()).hexdigest(), 'pages': 3})
(root / 'manifest.json').write_text(json.dumps({'generator': {'tifffile': tifffile.__version__, 'numpy': np.__version__, 'pillow': PIL.__version__, 'imagecodecs': imagecodecs.__version__}, 'files': entries}, indent=2) + '\n')
print(f'Generated and independently decoded {len(entries)} fixtures')

# Preserve precision independently of the 8-bit container matrix above.
for dtype in ['uint16', 'float32']:
    original = (data.astype(np.uint16) * 257) if dtype == 'uint16' else (data.astype(np.float32) / 255)
    for order, label in [('<', 'little'), ('>', 'big')]:
        for changed in [False, True]:
            pixels = original.copy()
            if changed:
                if dtype == 'uint16':
                    pixels[-1, 2, 3, 0] ^= 1
                else:
                    pixels[-1, 2, 3, 0] = np.nextafter(pixels[-1, 2, 3, 0], np.float32(1))
            name = f'precision-{dtype}-{label}-{"changed" if changed else "same"}.tif'
            path = root / name
            with tifffile.TiffWriter(path, byteorder=order) as writer:
                for page in pixels:
                    writer.write(page, photometric='rgb', metadata=None, compression='deflate', iccprofile=profile)
            assert np.array_equal(tifffile.imread(path, key=range(3)), pixels)
            entries.append({'file': name, 'changed': changed, 'sha256': hashlib.sha256(path.read_bytes()).hexdigest(), 'pages': 3})

# Stored pixels are inverse-oriented so their displayed presentations agree.
for orientation in range(1, 9):
    name = f'orientation-{orientation}.tif'
    path = root / name
    with tifffile.TiffWriter(path) as writer:
        for page in data:
            stored = {
                1: page, 2: page[:, ::-1], 3: page[::-1, ::-1], 4: page[::-1],
                5: page.transpose(1, 0, 2), 6: np.rot90(page, 1),
                7: page[::-1, ::-1].transpose(1, 0, 2), 8: np.rot90(page, -1),
            }[orientation]
            writer.write(stored, photometric='rgb', metadata=None, iccprofile=profile, extratags=[(274, 'H', 1, orientation, False)])
    entries.append({'file': name, 'orientation': orientation, 'sha256': hashlib.sha256(path.read_bytes()).hexdigest(), 'pages': 3})
(root / 'manifest.json').write_text(json.dumps({'generator': {'tifffile': tifffile.__version__, 'numpy': np.__version__, 'pillow': PIL.__version__, 'imagecodecs': imagecodecs.__version__}, 'files': entries}, indent=2) + '\n')
print(f'Added precision/orientation coverage; {len(entries)} fixtures total')
