#!/usr/bin/env python3
"""Author a prem control by changing item references only; preserve compressed media."""
import hashlib
import importlib.util
import json
from pathlib import Path
import struct

HERE = Path(__file__).resolve().parent
spec = importlib.util.spec_from_file_location('avif_boxes', HERE / 'generate-avif-square-pixel-fixtures.py')
helper = importlib.util.module_from_spec(spec)
spec.loader.exec_module(helper)
box, boxes = helper.box, helper.boxes
ROOT = HERE.parent / 'tests/fixtures/raster'

def generate(original):
    top = list(boxes(original))
    meta = next(value for kind, value in top if kind == b'meta')
    children = list(boxes(meta[4:]))
    pitm = next(value for kind, value in children if kind == b'pitm')
    assert pitm[:4] == bytes(4)
    primary = int.from_bytes(pitm[4:6], 'big')
    iref = next(value for kind, value in children if kind == b'iref')
    assert iref[:4] == bytes(4)
    refs = list(boxes(iref[4:]))
    aux = [v for k, v in refs if k == b'auxl' and v[2:4] == b'\x00\x01' and int.from_bytes(v[4:6], 'big') == primary]
    assert len(aux) == 1 and len(aux[0]) == 6
    alpha = int.from_bytes(aux[0][:2], 'big')
    assert primary != alpha and not any(k == b'prem' for k, _ in refs)
    extra = box(b'prem', struct.pack('>HHH', primary, 1, alpha))
    updated = []
    for kind, value in children:
        if kind == b'iref':
            value += extra
        elif kind == b'iloc':
            value = bytearray(value)
            assert value[:6] == bytes.fromhex('000000004400')
            count, at = int.from_bytes(value[6:8], 'big'), 8
            for _ in range(count):
                at += 2
                assert value[at:at+2] == bytes(2)
                at += 2
                extents = int.from_bytes(value[at:at+2], 'big')
                at += 2
                for _ in range(extents):
                    offset = int.from_bytes(value[at:at+4], 'big')
                    assert offset > 0
                    value[at:at+4] = (offset + len(extra)).to_bytes(4, 'big')
                    at += 8
            assert at == len(value)
        updated.append(box(kind, value))
    new_meta = meta[:4] + b''.join(updated)
    result = b''.join(box(k, new_meta if k == b'meta' else v) for k, v in top)
    assert [v for k, v in boxes(result) if k == b'mdat'] == [v for k, v in top if k == b'mdat']
    return result, primary, alpha

if __name__ == '__main__':
    source = ROOT / 'avif-alpha-icc.avif'
    original = source.read_bytes()
    result, primary, alpha = generate(original)
    dest = ROOT / 'avif-alpha-icc-prem.avif'
    if dest.exists():
        assert dest.read_bytes() == result
    else:
        dest.write_bytes(result)
    manifest = dict(source=source.name, source_sha256=hashlib.sha256(original).hexdigest(),
        fixture=dest.name, sha256=hashlib.sha256(result).hexdigest(), primary_item=primary,
        alpha_item=alpha, media_unchanged=True,
        scope='Authored prem interpretation control, not an independently re-encoded premultiplied producer fixture',
        acceptance='Native prem association, straight-alpha conversion and display/swap comparisons remain to be executed')
    (ROOT / 'avif-premultiplied-fixture.json').write_text(json.dumps(manifest, indent=2)+'\n')
    print('AVIF prem fixture reproduced; compressed media unchanged')
