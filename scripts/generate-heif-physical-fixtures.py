"""CC0 metadata-only HEIF fixtures; existing compressed media stays byte exact.

Only the committed producer's iloc-v0 / 4-byte layout is supported. The MIAF
compatibility declaration is omitted; these fixtures assert HEIF compatibility.
Native API observations are retained separately in the research report.
"""
from pathlib import Path
import hashlib
import json
import struct

ROOT = Path(__file__).resolve().parents[1] / 'tests/fixtures/raster'


def boxes(data):
    at = 0
    while at < len(data):
        size = int.from_bytes(data[at:at + 4], 'big')
        assert 8 <= size <= len(data) - at
        yield data[at + 4:at + 8], data[at + 8:at + size]
        at += size
    assert at == len(data)


def box(kind, data):
    return struct.pack('>I', len(data) + 8) + kind + data


def rewrite(original):
    top = list(boxes(original))
    old_brand = next(data for kind, data in top if kind == b'ftyp')
    brand = old_brand[:8] + b''.join(old_brand[i:i + 4]
        for i in range(8, len(old_brand), 4) if old_brand[i:i + 4] != b'miaf')
    old_meta = next(data for kind, data in top if kind == b'meta')
    children = list(boxes(old_meta[4:]))
    primary = next(data for kind, data in children if kind == b'pitm')
    assert primary[:4] == bytes(4)
    primary = int.from_bytes(primary[4:6], 'big')
    rebuilt = []
    for kind, data in children:
        if kind == b'iprp':
            props = list(boxes(data))
            ipco = next(value for tag, value in props if tag == b'ipco')
            index = len(list(boxes(ipco))) + 1
            assert index < 128
            output = []
            for tag, value in props:
                if tag == b'ipco':
                    value += box(b'pasp', struct.pack('>II', 2, 1))
                elif tag == b'ipma':
                    assert value[:4] == bytes(4)
                    count = int.from_bytes(value[4:8], 'big')
                    updated, at = value[:8], 8
                    for _ in range(count):
                        item = value[at:at + 2]
                        at += 2
                        length = value[at]
                        at += 1
                        associations = value[at:at + length]
                        at += length
                        if int.from_bytes(item, 'big') == primary:
                            associations += bytes([index])
                        updated += item + bytes([len(associations)]) + associations
                    assert at == len(value)
                    value = updated
                output.append(box(tag, value))
            data = b''.join(output)
        rebuilt.append((kind, data))
    delta = (sum(len(data) + 8 for _, data in rebuilt) + 4 - len(old_meta)
             + len(brand) - len(old_brand))
    output = []
    for kind, data in rebuilt:
        if kind == b'iloc':
            data = bytearray(data)
            assert data[:6] == bytes.fromhex('000000004440')
            count, at = int.from_bytes(data[6:8], 'big'), 8
            for _ in range(count):
                at += 2
                assert data[at:at + 2] == bytes(2)
                at += 2
                offset = int.from_bytes(data[at:at + 4], 'big')
                assert offset > 0
                data[at:at + 4] = (offset + delta).to_bytes(4, 'big')
                at += 4
                extents = int.from_bytes(data[at:at + 2], 'big')
                at += 2 + 8 * extents
            assert at == len(data)
        output.append(box(kind, data))
    meta = old_meta[:4] + b''.join(output)
    result = b''.join(box(kind, brand if kind == b'ftyp' else meta if kind == b'meta' else data)
                      for kind, data in top)
    assert [data for kind, data in boxes(result) if kind == b'mdat'] == [
        data for kind, data in top if kind == b'mdat']
    return result


if __name__ == '__main__':
    cases = []
    for name in ('alpha.heic', 'rotate90.heic', 'mirror.heif'):
        original = (ROOT / name).read_bytes()
        generated = rewrite(original)
        fixture = ROOT / ('heif-physical-' + name)
        if fixture.exists():
            assert fixture.read_bytes() == generated, 'existing fixture drift'
        else:
            fixture.write_bytes(generated)
        cases.append({'fixture': fixture.name, 'source': name,
                      'source_sha256': hashlib.sha256(original).hexdigest(),
                      'sha256': hashlib.sha256(generated).hexdigest(),
                      'requested_spacing': [2, 1], 'coded_media_unchanged': True,
                      'pixel_oracle': name + '.rgba'})
    (ROOT / 'heif-physical-pixel-aspect.json').write_text(json.dumps({
        'scope': 'CC0 metadata-only HEIF fixtures; MIAF compatibility declaration omitted',
        'cases': cases,
    }, indent=2) + '\n')
    print('Three metadata-only HEIF fixtures reproduced byte exactly')
