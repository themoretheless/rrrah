"""Independent Pillow derivatives of pinned CC0 camera JPEG previews.
Input RAW files must already exist; no native rrrah decoder is used here.
"""
from pathlib import Path
import argparse, hashlib, io, json
from PIL import Image, ImageEnhance, __version__

parser = argparse.ArgumentParser()
parser.add_argument('--corpus', type=Path, default=Path('/tmp/rrrah-raw-corpus'))
parser.add_argument('--ids', type=int, nargs='+', default=[830, 898, 1084, 1294])
parser.add_argument('--output', type=Path)
args = parser.parse_args()
root = Path(__file__).resolve().parents[1]
out = args.output if args.output is not None else root / 'crates/rrrah-dedup/tests/fixtures/photos'
out.mkdir(parents=True, exist_ok=True)
manifest = json.loads((root / 'tests/fixtures/raw-color-corpus.json').read_text())
records = []
for wanted in args.ids:
    case = next(c for c in manifest['cases'] if c['id'] == wanted)
    data = (args.corpus / case['filename']).read_bytes()
    assert hashlib.sha256(data).hexdigest() == case['sha256']
    previews, start = [], 0
    while True:
        start = data.find(b'\xff\xd8\xff', start)
        if start < 0:
            break
        end = data.find(b'\xff\xd9', start + 3)
        if end >= 0:
            encoded = data[start:end + 2]
            try:
                image = Image.open(io.BytesIO(encoded))
                if image.width * image.height <= 32_000_000:
                    image.load()
                    previews.append((image.width * image.height, start, len(encoded), image.convert('RGB'), hashlib.sha256(encoded).hexdigest()))
            except (OSError, ValueError):
                pass
        start += 3
    if not previews:
        raise ValueError(f'no bounded Pillow-decodable JPEG preview in {case["filename"]}')
    _, offset, length, image, preview_hash = max(previews, key=lambda p: p[0])
    original = image.size
    image.thumbnail((320, 320), Image.Resampling.LANCZOS)
    w, h = image.size
    variants = {
        'base': image,
        'resize': image.resize((round(w * .75), round(h * .75)), Image.Resampling.LANCZOS),
        'crop': image.crop((w // 8, h // 8, w - w // 8, h - h // 8)),
        'rotate': image.rotate(17, Image.Resampling.BICUBIC, expand=True),
        'brightness': ImageEnhance.Brightness(image).enhance(.7),
        'jpeg': image,
    }
    files = []
    for name, variant in variants.items():
        path = out / f'{wanted}-{name}.{ "jpg" if name == "jpeg" else "png"}'
        if name == 'jpeg':
            variant.save(path, quality=60, subsampling=2)
        else:
            variant.save(path)
        files.append({'variant': name, 'file': path.name, 'dimensions': variant.size, 'sha256': hashlib.sha256(path.read_bytes()).hexdigest()})
    records.append({'source': case, 'preview_offset': offset, 'preview_length': length, 'preview_sha256': preview_hash, 'preview_original_dimensions': original, 'files': files})
(out / 'manifest.json').write_text(json.dumps({'generator': Path(__file__).name, 'pillow': __version__, 'scope': 'Embedded rendered camera JPEG derivatives; not full RAW development. Brightness multiplies encoded RGB values, not scene-linear exposure.', 'cases': records}, indent=2) + '\n')
print(f'{len(records)} source previews, {sum(len(c["files"]) for c in records)} files')
