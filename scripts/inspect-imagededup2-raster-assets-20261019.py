"""Decode every frame/ICO variant in pinned raster assets and create a review contact sheet."""
from pathlib import Path
import hashlib
import json
import subprocess
from PIL import Image, ImageDraw, ImageOps

R = Path(__file__).resolve().parents[1]
S = R / '.dedup-work/imagededup2-20261019/source'
W = R / '.dedup-work/imagededup2-raster-assets-20261019'
W.mkdir(exist_ok=False)
paths = subprocess.check_output(['git', '-C', str(S), 'ls-tree', '-r', '--name-only', 'HEAD'], text=True).splitlines()
assets = [n for n in paths if n.startswith(('tests/data/', 'mkdocs/docs/img/', 'readme_figures/'))
          and not n.endswith('.pkl') and n != 'mkdocs/docs/img/logo.svg']
tiles, rows = [], []
for asset_id, name in enumerate(assets, 1):
    path = S / name
    row = dict(id=asset_id, path=name, sha256=hashlib.sha256(path.read_bytes()).hexdigest(), frames=[])
    try:
        with Image.open(path) as image:
            row['format'] = image.format
            if image.format == 'ICO':
                frames = [(str(size), image.ico.getimage(size)) for size in sorted(image.ico.sizes())]
            else:
                frames = []
                for frame in range(getattr(image, 'n_frames', 1)):
                    image.seek(frame)
                    image.load()
                    frames.append((str(frame), image.copy()))
            for frame_id, frame in frames:
                frame.load()
                rgba = frame.convert('RGBA')
                row['frames'].append(dict(frame=frame_id, size=list(frame.size), mode=frame.mode,
                    rgba_sha256=hashlib.sha256(rgba.tobytes()).hexdigest()))
                display = Image.new('RGBA', rgba.size, 'white')
                display.alpha_composite(rgba)
                thumb = ImageOps.contain(display.convert('RGB'), (220, 140))
                tile = Image.new('RGB', (240, 180), 'white')
                tile.paste(thumb, ((240-thumb.width)//2, 5))
                ImageDraw.Draw(tile).text((4, 148), f'{asset_id}: {Path(name).name[:25]}\nframe {frame_id}', fill='black')
                tiles.append(tile)
        row['status'] = 'all_frames_decoded'
    except Exception as e:
        row.update(status='decode_failed', exception=type(e).__name__, message=str(e))
    rows.append(row)
sheet = Image.new('RGB', (6 * 240, ((len(tiles)+5)//6) * 180), '#dddddd')
for i, tile in enumerate(tiles):
    sheet.paste(tile, ((i % 6)*240, (i//6)*180))
sheet.save(W / 'contact.png')
result = dict(status='decoded_pending_visual_review', assets=len(rows),
    successful_assets=sum(r['status']=='all_frames_decoded' for r in rows),
    decoded_frames_or_ICO_variants=len(tiles), rows=rows,
    limits=['Full-frame decoder traversal and RGBA byte hashes, not original matching algorithm execution.',
            'Contact thumbnails generated for visual review; no visual-review credit until inspected.',
            'Mislabeled SVG fixture decoded by content as JPEG; actual SVG logo excluded pending vector review.',
            'No private media or production code copied; pHash native investigation deferred.'])
(W / 'result.json').write_text(json.dumps(result, indent=2)+'\n')
print(json.dumps({k:result[k] for k in ['assets','successful_assets','decoded_frames_or_ICO_variants']}))
