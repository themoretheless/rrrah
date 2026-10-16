"""Test-only independent type-7 point probe; Newton roots are not a certificate."""
import hashlib
import json
import math
import struct
import sys
from pathlib import Path
from pypdf import PdfReader

source = Path(sys.argv[1])
output = Path(sys.argv[2])
assert not output.exists()
assert hashlib.sha256(source.read_bytes()).hexdigest() == 'c5fc34346629c3379cee6065c45b7de0b620be822604551d1463d8e691b03d06'
sh = PdfReader(source).pages[0]['/Resources']['/Shading']['/Sh8'].get_object()
assert [int(sh[k]) for k in ['/ShadingType', '/BitsPerFlag', '/BitsPerCoordinate', '/BitsPerComponent']] == [7, 8, 32, 8]
assert '/Function' not in sh
raw = sh.get_data()
decode = list(map(float, sh['/Decode']))
assert len(decode) == 12
patches = []
at = 0
while at < len(raw):
    flag = raw[at]
    at += 1
    assert flag in range(4)
    if flag:
        prev, colors = patches[-1]
        start = 3 * flag
        points = [prev[(start + i) % 12] for i in range(4)]
        colors = [colors[flag], colors[(flag + 1) % 4]]
    else:
        points, colors = [], []
    while len(points) < 16:
        assert at + 8 <= len(raw)
        x, y = struct.unpack_from('>II', raw, at)
        at += 8
        points.append(tuple(decode[axis * 2] + value / 4294967295 * (decode[axis * 2 + 1] - decode[axis * 2]) for axis, value in enumerate([x, y])))
    while len(colors) < 4:
        assert at + 4 <= len(raw)
        colors.append(tuple(decode[4 + axis * 2] + raw[at + axis] / 255 * (decode[5 + axis * 2] - decode[4 + axis * 2]) for axis in range(4)))
        at += 4
    patches.append((points, colors))
assert at == len(raw)

def bezier(points, t):
    points = list(points)
    while len(points) > 1:
        points = [tuple((1 - t) * a + t * b for a, b in zip(p, q)) for p, q in zip(points, points[1:])]
    return points[0]

def derivative(points, t):
    n = len(points) - 1
    return bezier([tuple(n * (b - a) for a, b in zip(p, q)) for p, q in zip(points, points[1:])], t)

# PDF 1.6 printed page300: stream numbering in the 4x4 tensor.
indices = [[0, 11, 10, 9], [1, 12, 15, 8], [2, 13, 14, 7], [3, 4, 5, 6]]
device_point = tuple(map(float,sys.argv[3:5])) if len(sys.argv)==5 else (147.5,59.5)
assert len(device_point)==2 and all(math.isfinite(x) for x in device_point)
target = (device_point[0],device_point[1]-245.5)  # Page inversion plus Sh8 source-Y flip.
roots = []
for index, (points, colors) in enumerate(patches):
    if not all(min(p[a] for p in points) <= target[a] <= max(p[a] for p in points) for a in range(2)):
        continue
    grid = [[points[i] for i in row] for row in indices]
    found = []
    for su in [.1, .5, .9]:
        for sv in [.1, .5, .9]:
            u, v = su, sv
            for iteration in range(40):
                rows = [bezier(row, u) for row in grid]
                value = bezier(rows, v)
                du = bezier([derivative(row, u) for row in grid], v)
                dv = derivative(rows, v)
                dx, dy = value[0] - target[0], value[1] - target[1]
                if max(abs(dx), abs(dy)) < 1e-10:
                    break
                det = du[0] * dv[1] - du[1] * dv[0]
                if det == 0 or not math.isfinite(det):
                    break
                u -= (dx * dv[1] - dy * dv[0]) / det
                v -= (du[0] * dy - du[1] * dx) / det
                if not math.isfinite(u + v) or max(abs(u), abs(v)) > 8:
                    break
            value = bezier([bezier(row, u) for row in grid], v)
            residual = max(abs(value[a] - target[a]) for a in range(2))
            if 0 <= u <= 1 and 0 <= v <= 1 and residual < 1e-9 and not any(abs(u-a)+abs(v-b) < 1e-7 for a,b in found):
                found.append((u,v))
                weights = [(1-u)*(1-v), (1-u)*v, u*v, u*(1-v)]
                cmyk = [sum(w*c[a] for w,c in zip(weights,colors)) for a in range(4)]
                roots.append({'patch':index,'uv':[u,v],'residual':residual,'cmyk':cmyk})
result = {'patch_count':len(patches),'stream_sha256':hashlib.sha256(raw).hexdigest(),'source_point':target,'device_point':device_point,'roots':roots,'scope':'Independent stream decoding, de Casteljau geometry and bilinear color; 9 Newton seeds per hull candidate. Not complete root isolation, clipping certification or whole-page qualification.'}
output.write_text(json.dumps(result,indent=2)+'\n')
print(json.dumps(result,indent=2))
