"""Authored PNG3 narrow PQ neutral controls; standard-library producer."""
import json
import pathlib
import struct
import zlib
from decimal import Decimal as D, localcontext

root = pathlib.Path(__file__).resolve().parents[1] / 'tests/fixtures/raster'
def chunk(kind, data):
    return struct.pack('>I', len(data)) + kind + data + struct.pack('>I', zlib.crc32(kind + data))
rows = []
for bits in (8, 16):
    scale = 2 ** (bits - 8)
    codes = [0, 16 * scale, 64 * scale, 128 * scale, 235 * scale, (1 << bits) - 1]
    width, height = 60, 48
    scan = bytearray()
    for y in range(height):
        scan.append(0)
        for x in range(width):
            code = codes[x // 10]
            samples = [code] * 3 + [(1 << bits) - 1]
            scan.extend(bytes(samples) if bits == 8 else struct.pack('>4H', *samples))
    data = b'\x89PNG\r\n\x1a\n'
    data += chunk(b'IHDR', struct.pack('>IIBBBBB', width, height, bits, 6, 0, 0, 0))
    data += chunk(b'cICP', bytes([9, 16, 0, 0]))
    data += chunk(b'IDAT', zlib.compress(scan, 9)) + chunk(b'IEND', b'')
    name = f'png-narrow-pq-color-{bits}.png'
    (root / name).write_bytes(data)
    with localcontext() as ctx:
        ctx.prec = 70
        expected = []
        for code in codes:
            x = (D(code) - D(16 * scale)) / D(219 * scale)
            p = abs(x) ** (D(32) / D(2523))
            value = (max(p - D(107) / D(128), D(0)) / (D(2413) / D(128) - D(2392) / D(128) * p)) ** (D(8192) / D(1305)) * D(100)
            expected.append(float(-value if x < 0 else value))
    rows.append(dict(name=name, bits=bits, codes=codes, neutral_linear=expected))
(root / 'png-narrow-pq-reference.json').write_text(json.dumps(rows, indent=2) + '\n')

# Derive the conversion matrix independently from chromaticities at 70 digits.
def inverse(a):
    a = [list(row) + [D(i == j) for j in range(3)] for i, row in enumerate(a)]
    for i in range(3):
        divisor = a[i][i]
        a[i] = [v / divisor for v in a[i]]
        for j in range(3):
            if j != i:
                factor = a[j][i]
                a[j] = [v - factor * w for v, w in zip(a[j], a[i])]
    return [r[3:] for r in a]
def multiply(a, b):
    return [[sum(a[i][k] * b[k][j] for k in range(3)) for j in range(3)] for i in range(3)]
def xyz_matrix(points):
    columns = [[x / y, D(1), (1 - x - y) / y] for x, y in points]
    base = [[columns[j][i] for j in range(3)] for i in range(3)]
    white = [D('0.3127') / D('0.3290'), D(1), (1 - D('0.3127') - D('0.3290')) / D('0.3290')]
    scales = [sum(v * w for v, w in zip(row, white)) for row in inverse(base)]
    return [[base[i][j] * scales[j] for j in range(3)] for i in range(3)]
colored = []
with localcontext() as ctx:
    ctx.prec = 70
    points = lambda values: [(D(x), D(y)) for x, y in values]
    src = xyz_matrix(points([('0.708', '0.292'), ('0.170', '0.797'), ('0.131', '0.046')]))
    dst = xyz_matrix(points([('0.640', '0.330'), ('0.300', '0.600'), ('0.150', '0.060')]))
    matrix = multiply(inverse(dst), src)
    for bits in (8, 16):
        scale, maximum = 2 ** (bits - 8), (1 << bits) - 1
        black, white = 16 * scale, 235 * scale
        bands = [[white, black, black], [black, white, black], [black, black, white], [maximum, 128 * scale, 0]]
        alphas = [0, maximum // 4, maximum // 2, maximum]
        cells, scan = [], bytearray()
        for alpha in alphas:
            for codes in bands:
                linear = []
                for code in codes:
                    x = (D(code) - black) / D(219 * scale)
                    p = abs(x) ** (D(32) / D(2523))
                    v = (max(p - D(107) / D(128), D(0)) / (D(2413) / D(128) - D(2392) / D(128) * p)) ** (D(8192) / D(1305)) * D(100)
                    linear.append(-v if x < 0 else v)
                rgb = [sum(v * w for v, w in zip(row, linear)) for row in matrix]
                cells.append([float(v) for v in rgb] + [float(D(alpha) / maximum)])
        for y in range(48):
            scan.append(0)
            for x in range(64):
                values = bands[x // 16] + [alphas[y // 12]]
                scan.extend(bytes(values) if bits == 8 else struct.pack('>4H', *values))
        name = f'png-narrow-pq-chromatic-alpha-{bits}.png'
        data = b'\x89PNG\r\n\x1a\n' + chunk(b'IHDR', struct.pack('>IIBBBBB', 64, 48, bits, 6, 0, 0, 0))
        data += chunk(b'cICP', bytes([9, 16, 0, 0])) + chunk(b'IDAT', zlib.compress(scan, 9)) + chunk(b'IEND', b'')
        (root / name).write_bytes(data)
        colored.append(dict(name=name, bits=bits, cells=cells))
(root / 'png-narrow-pq-chromatic-reference.json').write_text(json.dumps(colored, indent=2) + '\n')
