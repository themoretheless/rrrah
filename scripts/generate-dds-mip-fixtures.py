"""CC0 authored DDS 2D mip chains; standalone-mip Pillow pixel oracles."""
from pathlib import Path
from io import BytesIO
import hashlib
import struct
from PIL import Image, DdsImagePlugin  # Register only the required container plugin.

ROOT = Path(__file__).resolve().parent.parent / 'tests/fixtures/raster'
DIMS = [(9, 5), (4, 2), (2, 1), (1, 1)]


def header(width, height, count, fourcc, bits=0, masks=(0, 0, 0, 0), dxgi=None):
    compressed = fourcc in (b'DXT1', b'DXT3', b'DXT5') or dxgi in (71, 72, 77, 78)
    size = 8 if fourcc == b'DXT1' or dxgi in (71, 72) else 16
    pitch = ((width + 3) // 4) * ((height + 3) // 4) * size if compressed else width * (bits // 8)
    flags = 0x1007 | (0x80000 if compressed else 8) | (0x20000 if count > 1 else 0)
    values = [124, flags, height, width, pitch, 0, count] + [0] * 11
    values += [32, 4 if fourcc else (0x41 if masks[3] else 0x40),
               int.from_bytes(fourcc or b'\0\0\0\0', 'little'), bits, *masks,
               0x1000 | (0x400008 if count > 1 else 0), 0, 0, 0, 0]
    result = b'DDS ' + struct.pack('<31I', *values)
    if dxgi is not None:
        result += struct.pack('<5I', dxgi, 3, 0, 1, 1)
    return result


lines = ['# source\tmip\twidth\theight\tcolor\toracle\tsha256']
for label, fourcc, bits, masks, dxgi in [
    ('bc1', b'DXT1', 0, (0, 0, 0, 0), None),
    ('bc2', b'DXT3', 0, (0, 0, 0, 0), None),
    ('bc3', b'DXT5', 0, (0, 0, 0, 0), None),
    ('bc1-srgb', b'DX10', 0, (0, 0, 0, 0), 72),
    ('bc3-linear', b'DX10', 0, (0, 0, 0, 0), 77),
    ('rgb565', None, 16, (0xf800, 0x7e0, 0x1f, 0), None),
    ('rgb24', None, 24, (0xff0000, 0xff00, 0xff, 0), None),
    ('bgra32', None, 32, (0xff0000, 0xff00, 0xff, 0xff000000), None),
    ('rgba32-srgb', b'DX10', 32, (0, 0, 0, 0), 29),
]:
    payloads = []
    oracles = []
    for level, (width, height) in enumerate(DIMS):
        compressed = bits == 0
        if compressed:
            color = struct.pack('<HHI', [0xf800, 0x7e0, 0x1f, 0xffff][level], 0, 0)
            if fourcc == b'DXT3':
                block = bytes([0x10 + level * 0x11]) * 8 + color
            elif fourcc == b'DXT5' or dxgi == 77:
                block = bytes([64 + level * 50, 0]) + bytes(6) + color
            else:
                block = color
            payload = block * (((width + 3) // 4) * ((height + 3) // 4))
        elif bits == 16:
            payload = struct.pack('<H', [0xf800, 0x7e0, 0x1f, 0xffff][level]) * (width * height)
        else:
            pixel = bytes([20 + level * 31, 80 + level * 17, 170 - level * 19, level * 70])
            payload = pixel[:bits // 8] * (width * height)
        # Pillow lacks BC1 sRGB; identical raw blocks use UNORM for sample readback.
        reference_dxgi = 71 if dxgi == 72 else dxgi
        reference = header(width, height, 1, fourcc, bits, masks, reference_dxgi) + payload
        with Image.open(BytesIO(reference), formats=["DDS"]) as image:
            oracle = image.convert('RGBA').tobytes()
        assert len(oracle) == width * height * 4
        payloads.append(payload)
        oracles.append(oracle)
    source = header(*DIMS[0], len(DIMS), fourcc, bits, masks, dxgi) + b''.join(payloads)
    name = f'dds-mips-{label}.dds'
    (ROOT / name).write_bytes(source)
    digest = hashlib.sha256(source).hexdigest()
    color = 'srgb' if dxgi in (29, 72) else ('linear' if dxgi == 77 else 'assumed')
    for level, ((width, height), oracle) in enumerate(zip(DIMS, oracles)):
        target = f'{name}.mip{level}.rgba'
        (ROOT / target).write_bytes(oracle)
        lines.append(f'{name}\t{level}\t{width}\t{height}\t{color}\t{target}\t{digest}')
(ROOT / 'dds-mip-manifest.tsv').write_text('\n'.join(lines) + '\n')
