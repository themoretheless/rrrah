#!/usr/bin/env python3
"""Reproduce authored EMF comparisons with a separately supplied Java renderer."""
import argparse
import hashlib
import json
import pathlib
import struct
import subprocess
import tempfile

parser = argparse.ArgumentParser()
parser.add_argument('--java', required=True)
parser.add_argument('--jar', required=True)
parser.add_argument('--ffmpeg', required=True)
parser.add_argument('--report', required=True)
parser.add_argument('--svg-via-cairo', action='store_true', help='Use independent EMF-to-SVG backend and CairoSVG rasterization instead of Java2D')
args = parser.parse_args()
root = pathlib.Path(__file__).resolve().parents[1] / 'tests/fixtures/emf'
manifest = json.loads((root / 'manifest.json').read_text())
assert manifest['license'] == 'CC0-1.0' and manifest['images']
rows = []
with tempfile.TemporaryDirectory(prefix='rrrah-emf-oracle-') as directory:
    for item in manifest['images']:
        filename = item['file']
        assert pathlib.Path(filename).name == filename
        source = root / filename
        digest = hashlib.sha256(source.read_bytes()).hexdigest()
        assert digest == item['sha256'], f'{filename}: source hash mismatch'
        png = pathlib.Path(directory) / (filename + '.png')
        destination = png.with_suffix('.svg') if args.svg_via_cairo else png
        subprocess.run([args.java, '-Djava.awt.headless=true', '-jar', args.jar, str(source), str(destination)], check=True)
        if args.svg_via_cairo:
            import cairosvg
            cairosvg.svg2png(bytestring=destination.read_bytes(), write_to=str(png))
        png_bytes = png.read_bytes()
        assert png_bytes[:8] == b'\x89PNG\r\n\x1a\n'
        dimensions = list(struct.unpack('>II', png_bytes[16:24]))
        assert dimensions == [item['width'], item['height']], f'{filename}: dimensions differ'
        pixels = subprocess.check_output([args.ffmpeg, '-v', 'error', '-i', str(png), '-f', 'rawvideo', '-pix_fmt', 'rgba', '-'])
        expected = (root / (filename + '.rgba')).read_bytes()
        assert len(expected) == item['width'] * item['height'] * 4
        exact = pixels == expected
        differences = sum(pixels[n:n+4] != expected[n:n+4] for n in range(0, len(expected), 4))
        rows.append(dict(file=filename, source_sha256=digest, dimensions=dimensions, exact=exact, different_pixels=differences, rgba_sha256=hashlib.sha256(pixels).hexdigest(), expected_rgba_sha256=hashlib.sha256(expected).hexdigest()))
report = dict(renderer='separately supplied wmf2svg SVG + CairoSVG' if args.svg_via_cairo else 'separately supplied wmf2svg Java2D PNG', renderer_sha256=hashlib.sha256(pathlib.Path(args.jar).read_bytes()).hexdigest(), java_version=subprocess.run([args.java, '-version'], check=True, capture_output=True, text=True).stderr.strip(), png_decoder=subprocess.check_output([args.ffmpeg, '-version'], text=True).splitlines()[0], scope='authored stock/custom solid fills, null pen; independent producer and non-null stroke rasterization remain unqualified', images=rows)
if args.svg_via_cairo:
    import cairocffi
    report['cairosvg_version'] = cairosvg.__version__
    report['cairo_version'] = cairocffi.cairo_version_string()
pathlib.Path(args.report).write_text(json.dumps(report, indent=2) + '\n')
failed = [row['file'] for row in rows if not row['exact']]
print(f'{len(rows) - len(failed)}/{len(rows)} independent EMF comparisons passed')
if failed:
    raise SystemExit('Independent pixel mismatches: ' + ', '.join(failed))
