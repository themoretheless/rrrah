#!/usr/bin/env python3
"""Compare native legacy flatten against pinned independent parsing and Pillow alpha composition."""
import argparse
import hashlib
import importlib.metadata
import inspect
import json
from pathlib import Path
import subprocess
import tempfile
from PIL import Image, ImageChops
from gimpformats.gimpXcfDocument import GimpDocument


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('corpus', type=Path)
    parser.add_argument('--dump', required=True, type=Path)
    parser.add_argument('--report', required=True, type=Path)
    args = parser.parse_args()
    root = Path(__file__).resolve().parents[1]
    pinned = json.loads((root / 'docs/research/xcf-groups-python-pixels-2026-10-06.json').read_text())
    for name, version in pinned['oracle_dependencies'].items():
        if importlib.metadata.version(name) != version:
            raise SystemExit(f'Oracle dependency version mismatch: {name}')
    folder = Path(inspect.getfile(GimpDocument)).parent
    if {p.name: digest(p) for p in sorted(folder.glob('*.py'))} != pinned['oracle_source_hashes']:
        raise SystemExit('Oracle source hash mismatch')
    sources = [{
        'path': args.corpus / '1x1-violet-legacy.xcf',
        'sha256': '4be5c35b47e5e918d96067da20a94c78ba4c8b662bf010b92c41095737f9203a',
        'origin': 'https://raw.githubusercontent.com/shivshank/xcf-rs/3e1ef2cfb10142c467013f678c152fdaeeba5b6d/tests/samples/1x1-violet-legacy.xcf',
    }]
    for name in ['normal-overlay-v1', 'profiled-normal-overlay-v1', 'normal-mask-enabled-v1', 'normal-mask-disabled-v1', 'normal-mask-negative-offset-v1', 'normal-mask-hidden-v1', 'normal-mask-zero-opacity-v1', 'normal-mask-half-opacity-v1', 'normal-gray-v1', 'normal-gray-alpha-v1', 'normal-indexed-v1']:
        manifest = json.loads((root / f'tests/fixtures/xcf/{name}-manifest.json').read_text())
        sources.append({'path': root / f'tests/fixtures/xcf/{name}.xcf',
                        'sha256': manifest['sha256'], 'origin': manifest['construction'],
                        'manifest_rgba': bytes.fromhex(manifest['expected_rgba_hex'] if 'expected_rgba_hex' in manifest else manifest['expected_encoded_rgba_hex'])})
    # Verify every input before invoking either decoder.
    for item in sources:
        if digest(item['path']) != item['sha256']:
            raise SystemExit(f'Source hash mismatch: {item["path"]}')
    binary = args.dump.resolve()
    rows = []
    for item in sources:
        document = GimpDocument()
        document.load(str(item['path']))
        canvas = Image.new('RGBA', (document.width, document.height))
        for layer in reversed(document.raw_layers):
            if layer.image.mode not in ('RGB', 'RGBA', 'L', 'LA'):
                raise SystemExit('Oracle scope excludes unknown pixel modes')
            # The pinned parser preserves PROP_OPACITY integers unnormalized.
            opacity = layer.opacity / 255 if type(layer.opacity) is int else layer.opacity
            if layer.blendMode.name != 'NORMAL_LEGACY' or opacity not in (0.0, 128 / 255, 1.0):
                raise SystemExit('Oracle scope requires normal legacy and zero/128-of-255/unit opacity')
            if layer.visible and opacity != 0.0:
                if document.baseColorMode == 2:
                    # The independent parser exposes indexed data as L without
                    # attaching its independently parsed document colormap.
                    # Admit only opaque kind4 here; kind5 alpha semantics need
                    # an independent GIMP renderer qualification.
                    if layer.colorMode != 4 or layer.image.mode != 'L' or not document.colorMap:
                        raise SystemExit('Indexed oracle scope requires opaque kind4 with colormap')
                    image = Image.frombytes('P', layer.image.size, layer.image.tobytes())
                    image.putpalette([component for color in document.colorMap for component in color])
                    image = image.convert('RGBA')
                else:
                    image = layer.image.convert('RGBA')
                if opacity != 1.0:
                    # Only this fixture's endpoint RGB over opaque endpoint RGB
                    # is qualified; alpha-byte quantization is not a general float oracle.
                    image.putalpha(image.getchannel('A').point(lambda a: int(a * opacity + 0.5)))
                if layer.mask is not None and layer.applyMask:
                    mask = layer.mask.image
                    if mask.mode != 'L' or mask.size != image.size or set(mask.tobytes()) - {0, 255}:
                        raise SystemExit('Oracle scope requires native binary mask endpoints')
                    image.putalpha(ImageChops.multiply(image.getchannel('A'), mask))
                canvas.alpha_composite(image, (layer.xOffset, layer.yOffset))
        expected = canvas.tobytes()
        if 'manifest_rgba' in item and expected != item['manifest_rgba']:
            raise SystemExit(f'Independent oracle disagrees with authored manifest: {item["path"]}')
        with tempfile.TemporaryDirectory(prefix='rrrah-xcf-flatten-oracle-') as temporary:
            output = Path(temporary) / 'actual.rgba'
            run = subprocess.run([str(binary), str(item['path']), str(output)],
                                 capture_output=True, text=True, check=True)
            records = [json.loads(line) for line in run.stdout.splitlines()]
            actual = output.read_bytes()
        dimensions = records[0]['width'] == document.width and records[0]['height'] == document.height
        released = records[-1].get('managed_used') == 0
        rows.append({'source': str(item['path']), 'source_sha256': item['sha256'],
                     'origin': item['origin'], 'width': document.width, 'height': document.height,
                     'expected_sha256': hashlib.sha256(expected).hexdigest(),
                     'actual_sha256': hashlib.sha256(actual).hexdigest(),
                     'exact_pixels': actual == expected, 'dimensions_match': dimensions,
                     'manifest_matches_oracle': expected == item['manifest_rgba'] if 'manifest_rgba' in item else None,
                     'managed_release': released, 'managed_peak': records[-1]['managed_peak']})
    success = all(row['exact_pixels'] and row['dimensions_match'] and row['managed_release'] for row in rows)
    args.report.write_text(json.dumps({'status': 'passed' if success else 'failed',
        'decoder_sha256': digest(binary), 'oracle_dependencies': pinned['oracle_dependencies'],
        'oracle_source_hashes': pinned['oracle_source_hashes'], 'rows': rows,
        'scope': 'Encoded legacy normal RGB/RGBA/gray/gray-alpha and one opaque v1 indexed flatten: one external opaque single-pixel source and authored alpha-overlay/binary-mask sources including disabled masks and negative-offset clipping and zero-opacity exclusion and one authored 128-of-255 opacity endpoint-color case. No intermediate masks, general variable-opacity rounding, indexed-alpha or v0 indexed, groups, modern blending, ICC transform or GIMP presentation qualification.'}, indent=2) + '\n')
    if not success:
        raise SystemExit('Legacy flatten oracle mismatch')
    print(f'{len(rows)} flattened images exact; dimensions match; managed ownership zero')


if __name__ == '__main__':
    main()
