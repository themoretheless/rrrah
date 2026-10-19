"""Decode every saved PNG output in remaining notebooks without executing cells."""
from pathlib import Path
import base64
import hashlib
import io
import json
from PIL import Image, ImageDraw, ImageOps

R = Path(__file__).resolve().parents[1]
S = R / '.dedup-work/imagededup2-20261019/source'
W = R / '.dedup-work/imagededup2-notebook-images-v2-20261019'
W.mkdir(exist_ok=False)
rows, tiles = [], []
names = ['Encoding_generation.ipynb', 'Finding_duplicates.ipynb', 'CIFAR10_duplicates.ipynb']
notebooks = []
for name in names:
    p = S / 'examples' / name
    notebook = json.loads(p.read_text())
    notebooks.append(dict(path=str(p.relative_to(S)), sha256=hashlib.sha256(p.read_bytes()).hexdigest(),
                          cells=len(notebook['cells'])))
    for cell_id, cell in enumerate(notebook['cells']):
        assert not cell.get('attachments')
        for output_id, out in enumerate(cell.get('outputs', [])):
            for mime in out.get('data', {}):
                assert mime in ['text/plain', 'image/png'], mime
            encoded = out.get('data', {}).get('image/png')
            if encoded is None:
                continue
            raw = base64.b64decode(''.join(''.join(encoded).split()), validate=True)
            output = W / f'{p.stem}-cell{cell_id}-output{output_id}.png'
            output.write_bytes(raw)
            with Image.open(io.BytesIO(raw)) as image:
                image.load()
                rgb = image.convert('RGB')
                rows.append(dict(notebook=name, cell=cell_id, output=output_id,
                    artifact=output.name, sha256=hashlib.sha256(raw).hexdigest(), size=list(image.size),
                    RGB_sha256=hashlib.sha256(rgb.tobytes()).hexdigest()))
                tile = Image.new('RGB', (720, 520), 'white')
                thumb = ImageOps.contain(rgb, (710, 490))
                tile.paste(thumb, (5, 5))
                ImageDraw.Draw(tile).text((5, 500), f'{name} cell{cell_id}', fill='black')
                tiles.append(tile)
for start in range(0, len(tiles), 4):
    sheet = Image.new('RGB', (1440, 1040), '#dddddd')
    for j, tile in enumerate(tiles[start:start+4]):
        sheet.paste(tile, ((j%2)*720, (j//2)*520))
    sheet.save(W / f'contact-{start//4}.png')
result = dict(status='all_saved_PNG_outputs_decoded_pending_visual_review', notebooks=notebooks,
    saved_images=rows, findings=['All remaining notebook source and text outputs read separately.',
    'Historical CIFAR10 output explicitly uses imagededup0.0.4/TensorFlow2.0.0/Python3.6; no current-model timing or quality qualification.',
    'CIFAR example copies train and test into a flat directory, then filters by basename; namespace overlap must be checked before generalizing to other datasets.',
    'Embedding array displays contain ellipses; cannot reconstruct full vectors or verify original model output from saved text.'],
    limits=['No notebook cells, shell commands, models or downloads executed.',
            'PNG decoding is not visual review until sheets inspected; no matching-quality proof.'])
(W / 'result.json').write_text(json.dumps(result, indent=2)+'\n')
print(json.dumps({'notebooks':len(notebooks),'saved_images':len(rows),'contact_sheets':(len(tiles)+3)//4}))
