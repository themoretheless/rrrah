"""Regenerate CC0 synthetic raster fixtures with the independent Pillow codec.
Run with a Python environment containing Pillow. Oracles come from Pillow's
read-back, not Rrrah. Lossy formats permit a small decoder rounding tolerance.
"""
from pathlib import Path
import hashlib
from PIL import Image, ImageCms, __version__

root = Path(__file__).resolve().parent.parent / 'tests/fixtures/raster'
root.mkdir(parents=True, exist_ok=True)
rgb = Image.new('RGB', (16, 16))
rgb.putdata([(x * 17, y * 17, ((x + y) % 16) * 17) for y in range(16) for x in range(16)])
rgba = rgb.convert('RGBA')
rgba.putalpha(Image.frombytes('L', (16, 16), bytes((x * 17 for y in range(16) for x in range(16)))))
formats = [
    ('rgb', 'SGI', rgb, {}), ('rgba', 'SGI', rgba, {}),
    ('png', 'PNG', rgba, {}), ('jpg', 'JPEG', rgb, {'quality':95, 'subsampling':0}),
    ('webp', 'WEBP', rgba, {'lossless':True}), ('bmp', 'BMP', rgb, {}),
    ('gif', 'GIF', rgb, {}), ('ico', 'ICO', rgba, {'sizes':[(16,16)]}),
    ('tga', 'TGA', rgba, {}), ('ppm', 'PPM', rgb, {}),
    ('tif', 'TIFF', rgba, {'compression':'tiff_lzw'}),
    ('progressive.jpg', 'JPEG', rgb, {'quality':95, 'subsampling':0, 'progressive':True}),
    ('bitmap.ico', 'ICO', rgba, {'sizes':[(16,16)], 'bitmap_format':'bmp'}),
    ('indexed.png', 'PNG', rgb.quantize(colors=16), {}),
    ('profiled.png', 'PNG', rgba, {'icc_profile': ImageCms.ImageCmsProfile(ImageCms.createProfile('sRGB')).tobytes()}),
]
manifest = []
for ext, fmt, source, options in formats:
    name = 'pattern.' + ext
    path = root / name
    source.save(path, format=fmt, **options)
    with Image.open(path) as decoded:
        decoded.load()
        reference = decoded.convert('RGBA').tobytes()
        width, height = decoded.size
    oracle = name + '.rgba'
    (root / oracle).write_bytes(reference)
    tolerance = 2 if ext.endswith('jpg') else 0
    manifest.append(f'{name}\t{width}\t{height}\t{tolerance}\t{oracle}\t{hashlib.sha256(path.read_bytes()).hexdigest()}')
(root / 'manifest.tsv').write_text('\n'.join(manifest) + '\n')
(root / 'README.md').write_text(f'''# Independent raster corpus

Synthetic coordinate/color/alpha patterns authored for Rrrah and dedicated to
CC0. Generated with Pillow {__version__} by `scripts/generate-raster-fixtures.py`.
No third-party photographs or assets are included. Each `.rgba` oracle is the
independent Pillow decoder's row-major straight RGBA8 output. JPEG allows two
sample levels for decoder rounding; lossless formats require exact equality.
The manifest records file name, dimensions, tolerance, oracle, and source SHA256.
This is baseline pixel qualification, not a complete format variant corpus.
''')
