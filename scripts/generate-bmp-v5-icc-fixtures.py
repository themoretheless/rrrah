#!/usr/bin/env python3
"""CC0 authored BMP V5 + independent LittleCMS double linear-sRGB oracle."""
import ctypes as c
import ctypes.util
import hashlib
import json
from pathlib import Path
import struct

root = Path(__file__).resolve().parents[1] / 'tests/fixtures/raster'
profile = (root / 'animation-linear-float.icc').read_bytes()
rgba = [(64, 128, 192, 255), (200, 30, 100, 128), (17, 231, 80, 1), (110, 59, 219, 0)]
files = []
for top in (False, True):
    b = bytearray(138)
    b[:2] = b'BM'
    def u(at, value): struct.pack_into('<I', b, at, value)
    u(10, 138); u(14, 124); u(18, 2)
    struct.pack_into('<i', b, 22, -2 if top else 2)
    struct.pack_into('<HH', b, 26, 1, 32)
    u(30, 3); u(34, 16)
    for at, value in [(54, 0xff0000), (58, 0xff00), (62, 0xff), (66, 0xff000000),
                      (70, 0x4d424544), (122, 4), (126, 140), (130, len(profile))]: u(at, value)
    pixels = rgba if top else rgba[2:] + rgba[:2]
    b.extend(bytes(v for r, g, blue, a in pixels for v in (blue, g, r, a)))
    b.extend(profile); u(2, len(b))
    name = f'bmp-v5-linear-icc-{"top" if top else "bottom"}.bmp'
    (root / name).write_bytes(b); files.append(name)
lib = c.CDLL(ctypes.util.find_library('lcms2'))
def bind(name, result, *args):
    f = getattr(lib, name); f.restype = result; f.argtypes = list(args); return f
class xyY(c.Structure): _fields_ = [('x', c.c_double), ('y', c.c_double), ('Y', c.c_double)]
class Primaries(c.Structure): _fields_ = [('r', xyY), ('g', xyY), ('b', xyY)]
build = bind('cmsBuildGamma', c.c_void_p, c.c_void_p, c.c_double)
create = bind('cmsCreateRGBProfile', c.c_void_p, c.POINTER(xyY), c.POINTER(Primaries), c.POINTER(c.c_void_p))
openp = bind('cmsOpenProfileFromMem', c.c_void_p, c.c_void_p, c.c_uint32)
transform = bind('cmsCreateTransform', c.c_void_p, c.c_void_p, c.c_uint32, c.c_void_p, c.c_uint32, c.c_uint32, c.c_uint32)
do = bind('cmsDoTransform', None, c.c_void_p, c.c_void_p, c.c_void_p, c.c_uint32)
gamma = build(None, 1.0); assert gamma
curves = (c.c_void_p * 3)(gamma, gamma, gamma)
white = xyY(.3127, .3290, 1.)
primaries = Primaries(xyY(.64, .33, 1.), xyY(.30, .60, 1.), xyY(.15, .06, 1.))
destination = create(c.byref(white), c.byref(primaries), curves); assert destination
memory = c.create_string_buffer(profile); source = openp(memory, len(profile)); assert source
rgb_double = (1 << 22) | (4 << 16) | (3 << 3)
t = transform(source, rgb_double, destination, rgb_double, 1, 0x140); assert t
incoming = (c.c_double * 12)(*(v / 255 for p in rgba for v in p[:3]))
output = (c.c_double * 12)(); do(t, incoming, output, 4)
reference = {'license': 'CC0', 'rgba8': rgba, 'linear_rgb_f64': list(output),
             'oracle': 'LittleCMS double RGB, relative colorimetric, no optimization/cache, linear sRGB D65 destination',
             'lcms_version': bind('cmsGetEncodedCMMversion', c.c_uint32)(),
             'sha256': {name: hashlib.sha256((root / name).read_bytes()).hexdigest() for name in files + ['animation-linear-float.icc']}}
(root / 'bmp-v5-icc-reference.json').write_text(json.dumps(reference, indent=2) + '\n')
bind('cmsDeleteTransform', None, c.c_void_p)(t)
close = bind('cmsCloseProfile', c.c_int, c.c_void_p); close(source); close(destination)
bind('cmsFreeToneCurve', None, c.c_void_p)(gamma)
