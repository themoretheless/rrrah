#!/usr/bin/env python3
"""Metadata-only pasp controls for the pinned libavif RGB fixture; AV1 media unchanged."""
from pathlib import Path
import hashlib
import json
import struct

ROOT = Path(__file__).resolve().parents[1] / 'tests/fixtures/raster'

def boxes(data):
    at = 0
    while at < len(data):
        size = int.from_bytes(data[at:at+4], 'big')
        assert 8 <= size <= len(data)-at
        yield data[at+4:at+8], data[at+8:at+size]
        at += size
    assert at == len(data)

def box(tag, data):
    return struct.pack('>I', len(data)+8) + tag + data

def rewrite(original, horizontal, vertical, essential):
    top = list(boxes(original))
    meta = next(data for tag, data in top if tag == b'meta')
    children = list(boxes(meta[4:]))
    primary = next(data for tag, data in children if tag == b'pitm')
    assert primary[:4] == bytes(4)
    primary = int.from_bytes(primary[4:6], 'big')
    rebuilt = []
    for tag, data in children:
        if tag == b'iprp':
            properties = list(boxes(data))
            ipco = next(value for kind, value in properties if kind == b'ipco')
            index = len(list(boxes(ipco)))+1
            assert index < 128
            output = []
            for kind, value in properties:
                if kind == b'ipco':
                    value += box(b'pasp', struct.pack('>II', horizontal, vertical))
                elif kind == b'ipma':
                    assert value[:4] == bytes(4)
                    count = int.from_bytes(value[4:8], 'big')
                    updated, at = value[:8], 8
                    matched = 0
                    for _ in range(count):
                        item = value[at:at+2]; at += 2
                        length = value[at]; at += 1
                        associations = value[at:at+length]; at += length
                        if int.from_bytes(item, 'big') == primary:
                            associations += bytes([index | (128 if essential else 0)])
                            matched += 1
                        updated += item + bytes([len(associations)]) + associations
                    assert at == len(value) and matched == 1
                    value = updated
                output.append(box(kind, value))
            data = b''.join(output)
        rebuilt.append((tag, data))
    delta = sum(len(data)+8 for _, data in rebuilt)+4-len(meta)
    output = []
    for tag, data in rebuilt:
        if tag == b'iloc':
            data = bytearray(data)
            assert data[:6] == bytes.fromhex('000000004400')
            count, at = int.from_bytes(data[6:8], 'big'), 8
            for _ in range(count):
                at += 2
                assert data[at:at+2] == bytes(2); at += 2
                extents = int.from_bytes(data[at:at+2], 'big'); at += 2
                for _ in range(extents):
                    offset = int.from_bytes(data[at:at+4], 'big')
                    assert offset > 0
                    data[at:at+4] = (offset+delta).to_bytes(4, 'big'); at += 8
            assert at == len(data)
        output.append(box(tag, data))
    new_meta = meta[:4] + b''.join(output)
    result = b''.join(box(tag, new_meta if tag == b'meta' else data) for tag, data in top)
    assert [v for k,v in boxes(result) if k == b'mdat'] == [v for k,v in top if k == b'mdat']
    return result

if __name__ == '__main__':
    source = ROOT / 'avif-rgb.avif'
    original = source.read_bytes()
    cases = []
    for name, h, v, essential in [('square', 2, 2, False), ('square-essential', 2, 2, True),
                                  ('nonsquare', 2, 1, False), ('zero', 0, 0, False)]:
        generated = rewrite(original, h, v, essential)
        fixture = ROOT / f'avif-pasp-{name}.avif'
        if fixture.exists():
            assert fixture.read_bytes() == generated
        else:
            fixture.write_bytes(generated)
        cases.append({'fixture': fixture.name, 'spacing': [h,v], 'essential': essential,
                      'conforming_pasp': h > 0 and h == v,
                      'sha256': hashlib.sha256(generated).hexdigest(), 'media_unchanged': True})
    (ROOT / 'avif-square-pixel-fixtures.json').write_text(json.dumps({
        'specification': 'https://aomediacodec.github.io/av1-avif/v1.2.0.html',
        'pasp_requirement': 'If present, pixel aspect ratio shall be 1:1; spacing values must be positive',
        'source': source.name, 'source_sha256': hashlib.sha256(original).hexdigest(), 'cases': cases,
        'scope': 'pinned iloc-v0 absolute-extent layout; metadata only, no AV1 re-encode'}, indent=2)+'\n')
    print('Four AVIF pasp fixtures reproduced exactly; all AV1 media unchanged')
