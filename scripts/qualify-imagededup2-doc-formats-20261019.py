"""Compare pinned format documentation with original loader and real fixture decoding."""
from pathlib import Path
from types import SimpleNamespace
import ast
import hashlib
import json
import numpy as np
from PIL import Image

R = Path(__file__).resolve().parents[1]
S = R / '.dedup-work/imagededup2-20261019/source'
W = R / '.dedup-work/imagededup2-doc-formats-v2-20261019'
W.mkdir(exist_ok=False)
source = S / 'imagededup/utils/image_utils.py'
tree = ast.parse(source.read_text())
tree.body = [n for n in tree.body if not (isinstance(n, ast.ImportFrom) and n.module == 'imagededup.utils.logger')]
env = {'return_logger': lambda *args: SimpleNamespace(warning=lambda *args: None)}
exec(compile(tree, str(source), 'exec'), env)
rows = []
for path in sorted((S / 'tests/data/formats_images').iterdir()):
    try:
        with Image.open(path) as image:
            actual_format = image.format
            frames = getattr(image, 'n_frames', 1)
    except Exception as e:
        actual_format = None
        frames = None
    loaded = env['load_image'](path)
    rows.append(dict(path=str(path.relative_to(S)), sha256=hashlib.sha256(path.read_bytes()).hexdigest(),
                     pillow_format=actual_format, frame_count=frames,
                     loaded_shape=list(loaded.shape) if loaded is not None else None))
svg = next(r for r in rows if r['path'].endswith('.svg'))
assert svg['pillow_format'] == 'JPEG' and svg['loaded_shape'] is not None
real_svg = W / 'real.svg'
real_svg.write_text('<svg xmlns="http://www.w3.org/2000/svg" width="4" height="4"><rect width="4" height="4" fill="red"/></svg>')
assert env['load_image'](real_svg) is None
assert all(r['pillow_format'] == 'PPM' and r['loaded_shape'] is not None
           for r in rows if r['path'].endswith(('.pbm', '.pgm', '.ppm')))
result = dict(status='passed_documented_format_characterization',
    loader_sha256=hashlib.sha256(source.read_bytes()).hexdigest(), fixture_results=rows,
    findings=['Encoding guide lists SVG support; tracked SVG-named fixture is actually JPEG, so its success does not qualify SVG. A real synthetic SVG returns None in this Pillow environment.',
              'PGM/PBM fixtures decode under Pillow PPM format identifier, so missing separate PGM/PBM IMG_FORMATS entries does not mean unsupported.',
              'README explicitly limits published benchmarks to <=v0.2.2; pinned package0.3.3-post2 must not inherit historical timing/quality claims.',
              'Removal guide already documents order-dependent chain heuristic; prior runtime finding is documented behavior, not newly discovered contract violation.',
              'Evaluation guide example spells f1-score; implementation returns f1_score.',
              'Encoding guide describes576-dimensional CNN output; custom-model guide permits other feature dimensions.'],
    limits=['Original image_utils module unchanged except logger import routing; real Pillow/NumPy decode on tracked format fixtures.',
            'No visual review, all-frame decode, platform matrix, benchmark reproduction or production change.',
            'Documentation read as source material; download/shell/publish instructions were not executed.',
            'SVG result applies to this installed Pillow environment, not every optional external decoder.'])
(W / 'result.json').write_text(json.dumps(result, indent=2) + '\n')
print(json.dumps({'fixtures': len(rows), 'SVG_named_fixture_actual_format': svg['pillow_format'], 'real_SVG_loaded': False}))
