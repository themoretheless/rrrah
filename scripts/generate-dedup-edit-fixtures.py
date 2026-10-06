"""Independent Pillow overlays on the existing CC0 seeded texture."""
from pathlib import Path
import hashlib, json
from PIL import Image, ImageDraw, __version__
root = Path(__file__).resolve().parent.parent / 'crates/rrrah-dedup/tests/fixtures'
source = root / 'rotation/base.png'
out = root / 'edits'
out.mkdir(exist_ok=True)
cases = []
for name, box, accepted in [('small-watermark', (5, 5, 44, 24), True), ('large-edit', (40, 40, 119, 119), False)]:
    image = Image.open(source).convert('RGBA')
    ImageDraw.Draw(image).rectangle(box, fill=(255, 0, 0, 255))
    path = out / (name + '.png')
    image.save(path)
    cases.append({'file': path.name, 'rectangle_inclusive': box, 'changed_pixels': (box[2]-box[0]+1)*(box[3]-box[1]+1), 'candidate_at_90_percent_pixel_policy': accepted, 'sha256': hashlib.sha256(path.read_bytes()).hexdigest()})
(out / 'manifest.json').write_text(json.dumps({'license': 'CC0-1.0', 'pillow': __version__, 'source_sha256': hashlib.sha256(source.read_bytes()).hexdigest(), 'cases': cases}, indent=2)+'\n')
