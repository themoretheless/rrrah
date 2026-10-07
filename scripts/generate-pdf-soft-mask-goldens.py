#!/usr/bin/env python3
"""Generate independent Poppler references for authored black-foreground masks."""
import hashlib
import json
from pathlib import Path
import subprocess
import tempfile

root = Path(__file__).resolve().parents[1] / 'tests/fixtures/pdf'
rows = []
with tempfile.TemporaryDirectory(prefix='rrrah-mask-oracle-') as directory:
    for name, width in [('soft-mask-device-rgb-bands', 24),
                        ('soft-mask-shared-group', 24),
                        ('soft-mask-alpha-without-group-cs', 16)]:
        source = root / (name + '.pdf')
        prefix = Path(directory) / name
        result = subprocess.run(['pdftoppm', '-r', '72', '-singlefile',
                                 str(source), str(prefix)], capture_output=True, check=True)
        if result.stderr.strip():
            raise ValueError(result.stderr.decode())
        ppm = prefix.with_suffix('.ppm').read_bytes()
        header, rgb = ppm.split(b'\n255\n', 1)
        assert header == f'P6\n{width} 8'.encode()
        assert len(rgb) == width * 8 * 3
        assert all(r == g == b for r, g, b in zip(rgb[0::3], rgb[1::3], rgb[2::3]))
        # These authored PDFs paint only black. Compositing over white gives
        # white = 255 - alpha. RGB behind zero alpha is canonically black.
        rgba = b''.join(bytes([0, 0, 0, 255 - value]) for value in rgb[0::3])
        source.with_suffix('.rgba').write_bytes(rgba)
        rows.append({'source': source.name, 'source_sha256': hashlib.sha256(source.read_bytes()).hexdigest(),
                     'ppm_sha256': hashlib.sha256(ppm).hexdigest(),
                     'rgba_sha256': hashlib.sha256(rgba).hexdigest(), 'dimensions': [width, 8]})
report = {'oracle': subprocess.run(['pdftoppm', '-v'], capture_output=True, check=True).stderr.decode().strip(),
          'dpi': 72, 'scope': 'Black foreground over white; recover alpha by 255 minus oracle gray.', 'fixtures': rows}
(root / 'soft-mask-poppler-goldens.json').write_text(json.dumps(report, indent=2) + '\n')
