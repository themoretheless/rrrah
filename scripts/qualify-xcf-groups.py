#!/usr/bin/env python3
"""Strict external XCF group pixel/metadata checks with a pinned Python oracle."""
import argparse
import hashlib
import importlib.metadata
import inspect
import json
from pathlib import Path
import subprocess
import tempfile
from gimpformats.gimpXcfDocument import GimpDocument


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('corpus', type=Path)
    parser.add_argument('--dump', required=True, type=Path)
    parser.add_argument('--report', required=True, type=Path)
    args = parser.parse_args()
    manifest = Path(__file__).resolve().parents[1] / 'docs/research/xcf-groups-python-pixels-2026-10-06.json'
    pinned = json.loads(manifest.read_text())
    dependencies = pinned['oracle_dependencies']
    for name, version in dependencies.items():
        if importlib.metadata.version(name) != version:
            raise SystemExit(f'Oracle dependency version mismatch: {name}')
    folder = Path(inspect.getfile(GimpDocument)).parent
    actual_hashes = {p.name: digest(p) for p in sorted(folder.glob('*.py'))}
    if actual_hashes != pinned['oracle_source_hashes']:
        raise SystemExit('Oracle source hash mismatch')
    sources = pinned['sources']
    for item in sources:
        source = args.corpus / item['url'].rsplit('/', 1)[1]
        if source.stat().st_size != item['bytes'] or digest(source) != item['sha256']:
            raise SystemExit(f'Source hash/length mismatch: {source.name}')
    binary = args.dump.resolve()
    rows, metadata, runs = [], [], []
    for item in sources:
        source = (args.corpus / item['url'].rsplit('/', 1)[1]).resolve()
        oracle = GimpDocument()
        oracle.load(str(source))
        raw_layers = oracle.raw_layers
        indices = {id(layer): index for index, layer in enumerate(raw_layers)}
        oracle_nodes = {}

        def visit(group, parent=None):
            for sibling, child in enumerate(group.children):
                is_group = hasattr(child, 'children')
                layer = child.layer_options if is_group else child
                index = indices[id(layer)]
                if index in oracle_nodes:
                    raise RuntimeError('Oracle tree contains duplicate layer')
                oracle_nodes[index] = {'parent': parent, 'sibling': sibling,
                                       'is_group': is_group}
                if is_group:
                    visit(child, index)

        visit(oracle.walkTree())
        if len(oracle_nodes) != len(raw_layers):
            raise RuntimeError('Oracle tree omits a source layer')
        with tempfile.TemporaryDirectory(prefix='rrrah-xcf-group-oracle-') as directory:
            run = subprocess.run([str(binary), str(source), directory],
                                 capture_output=True, text=True, check=True)
            records = [json.loads(line) for line in run.stdout.splitlines()]
            runs.append({'file': source.name, 'records': records})
            for index, layer in enumerate(raw_layers):
                expected = {
                    'tree_node': oracle_nodes[index],
                    'layer_group': {'is_group': bool(layer.isGroup), 'item_path': layer.itemPath or []},
                    'layer_attributes': {'visible': bool(layer.visible), 'opacity': layer.opacity,
                                         'offset_x': layer.xOffset, 'offset_y': layer.yOffset,
                                         'apply_mask': bool(layer.applyMask and layer.mask is not None)},
                }
                for selector, values in expected.items():
                    matches = [r for r in records if r.get(selector) == index]
                    actual = matches[0] if len(matches) == 1 else None
                    exact = actual is not None and all(actual.get(k) == v for k, v in values.items())
                    metadata.append({'file': source.name, 'index': index, 'selector': selector,
                                     'expected': values, 'actual': actual, 'exact': exact})
                for kind, image in [('layer', layer.image),
                                    ('mask', layer.mask.image if layer.mask is not None else None)]:
                    if image is None:
                        continue
                    expected_pixels = image.tobytes()
                    actual = (Path(directory) / f'{kind}-{index}.bin').read_bytes()
                    rows.append({'file': source.name, 'object': kind, 'index': index,
                                 'mode': image.mode, 'bytes': len(actual),
                                 'oracle_bytes': len(expected_pixels), 'exact': actual == expected_pixels,
                                 'different_bytes': sum(a != b for a, b in zip(actual, expected_pixels))
                                                    + abs(len(actual) - len(expected_pixels)),
                                 'actual_sha256': hashlib.sha256(actual).hexdigest()})
    report = {**{k: pinned[k] for k in ['oracle_package', 'version', 'scope', 'sources',
                                      'oracle_dependencies', 'oracle_source_hashes']},
              'decoder_sha256': digest(binary), 'rows': rows,
              'metadata_comparisons': metadata, 'runs': runs}
    args.report.parent.mkdir(parents=True, exist_ok=True)
    args.report.write_text(json.dumps(report, indent=2) + '\n')
    if (not rows or not all(r['exact'] for r in rows + metadata)
            or any(r['records'][-1].get('managed_used') != 0 for r in runs)):
        raise SystemExit(1)
    print(f'{len(rows)} pixel planes and {len(metadata)} metadata comparisons exact; managed usage zero')


if __name__ == '__main__':
    main()
