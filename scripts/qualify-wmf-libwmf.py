#!/usr/bin/env python3
"""Compare authored WMF fixtures against an externally built libwmf renderer."""
import argparse
import hashlib
import json
import pathlib
import subprocess
import tempfile

parser = argparse.ArgumentParser()
parser.add_argument('--wmf2gd', required=True)
parser.add_argument('--ffmpeg', required=True)
parser.add_argument('--source-archive', required=True)
parser.add_argument('--report', required=True)
args = parser.parse_args()
root = pathlib.Path(__file__).resolve().parents[1] / 'tests/fixtures/wmf'
rows = []
with tempfile.TemporaryDirectory(prefix='rrrah-wmf-reference-') as directory:
    for name in ['red', 'green', 'blue', 'polygon', 'saved-context', 'object-reuse']:
        source = root / (name + '.wmf')
        png = pathlib.Path(directory) / (name + '.png')
        result = subprocess.run([args.wmf2gd, '--maxwidth=10', '--maxheight=10', '--maxsize', '-o', str(png), str(source)], capture_output=True)
        if result.returncode or b'ERROR:' in result.stderr or not png.exists():
            raise RuntimeError(f'{name}: external renderer failed: {result.stderr!r}')
        pixels = subprocess.check_output([args.ffmpeg, '-v', 'error', '-i', str(png), '-f', 'rawvideo', '-pix_fmt', 'rgba', '-'])
        expected = (root / (name + '.wmf.rgba')).read_bytes()
        if pixels != expected:
            raise RuntimeError(f'{name}: independent pixels differ')
        rows.append(dict(file=source.name, wmf_sha256=hashlib.sha256(source.read_bytes()).hexdigest(), rgba_sha256=hashlib.sha256(pixels).hexdigest(), sample_bytes=len(pixels), exact=True))
report = dict(renderer='libwmf 0.2.16 wmf2gd bundled GD', source_url='https://github.com/caolanm/libwmf/releases/download/v0.2.16/libwmf-0.2.16.tar.gz', source_sha256=hashlib.sha256(pathlib.Path(args.source_archive).read_bytes()).hexdigest(), png_decoder=subprocess.check_output([args.ffmpeg, '-version'], text=True).splitlines()[0], scope='authored 10x10 opaque rectangles and polygon with explicit solid brush and null pen; independent producers and stroke fidelity unqualified', images=rows)
pathlib.Path(args.report).write_text(json.dumps(report, indent=2) + '\n')
print(f'{len(rows)} independent pixel comparisons passed')
