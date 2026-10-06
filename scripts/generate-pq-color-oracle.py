"""Independent CC0 PQ color vectors: Decimal EOTF, exact chromaticity matrices.
Primaries/white: ICC RGB registry BT.2020 and sRGB. Transfer: ITU H.273/ST2084.
No production coefficients or code are imported.
"""
from decimal import Decimal as D, getcontext
from fractions import Fraction as F
from itertools import product
from pathlib import Path
import random
import sys

getcontext().prec = 60


def inverse(matrix):
    rows = [list(row) + [F(i == j) for j in range(3)] for i, row in enumerate(matrix)]
    for i in range(3):
        pivot = next(j for j in range(i, 3) if rows[j][i])
        rows[i], rows[pivot] = rows[pivot], rows[i]
        divisor = rows[i][i]
        rows[i] = [v / divisor for v in rows[i]]
        for j in range(3):
            if j != i:
                factor = rows[j][i]
                rows[j] = [a - factor * b for a, b in zip(rows[j], rows[i])]
    return [row[3:] for row in rows]


def multiply(a, b):
    return [[sum(a[i][k] * b[k][j] for k in range(3)) for j in range(3)] for i in range(3)]


def rgb_xyz(primaries):
    xy = [(F(x), F(y)) for x, y in primaries]
    columns = [[x / y, F(1), (1 - x - y) / y] for x, y in xy]
    matrix = [list(row) for row in zip(*columns)]
    x, y = F("0.3127"), F("0.3290")
    white = [x / y, F(1), (1 - x - y) / y]
    weights = [sum(a * b for a, b in zip(row, white)) for row in inverse(matrix)]
    return [[matrix[i][j] * weights[j] for j in range(3)] for i in range(3)]


matrix = multiply(inverse(rgb_xyz([("0.64", "0.33"), ("0.30", "0.60"), ("0.15", "0.06")])),
                  rgb_xyz([("0.708", "0.292"), ("0.170", "0.797"), ("0.131", "0.046")]))
matrix = [[D(v.numerator) / D(v.denominator) for v in row] for row in matrix]
print("Independent chromaticity-derived matrix:")
for row in matrix:
    print([float(v) for v in row])


def pq(code):
    value = D(code) / D(65535)
    p = value ** (D(32) / D(2523))
    return (max(D(0), p - D(3424) / D(4096)) /
            (D(2413) / D(128) - D(2392) / D(128) * p)) ** (D(8192) / D(1305)) * D(10000)


vectors = list(product([0, 65535], repeat=3))
vectors += [(v, v, v) for v in [round(i * 65535 / 16) for i in range(17)]]
rng = random.Random(20842020)
vectors += [tuple(rng.randrange(65536) for _ in range(3)) for _ in range(64)]
lines = []
if "--hlg" in sys.argv:
    def scene(code):
        value = D(code) / D(65535)
        return value * value / D(3) if value <= D("0.5") else (
            ((value - D("0.55991073")) / D("0.17883277")).exp() + D("0.28466892")) / D(12)
    for white, peak, gamma in [(100, 1000, "1.2"), (203, 4000, "1.4"), (1, 100, "0.8")]:
        for index, rgb in enumerate(vectors):
            alpha = [0, 16384, 32768, 65535][index % 4]
            linear = [scene(code) for code in rgb]
            luminance = sum(a * b for a, b in zip(map(D, ["0.2627", "0.6780", "0.0593"]), linear))
            gain = luminance ** (D(gamma) - 1) * D(peak) / D(white) if luminance else D(0)
            linear = [v * gain for v in linear]
            output = [sum(a * b for a, b in zip(row, linear)) for row in matrix]
            lines.append("\t".join(map(str, [white, peak, gamma, *rgb, alpha, *output, D(alpha) / D(65535)])))
    filename = "hlg-color-decimal-oracle.tsv"
else:
    for white in [1, 100, 203, 1000, 10000]:
        for index, rgb in enumerate(vectors):
            alpha = [0, 16384, 32768, 65535][index % 4]
            linear = [pq(code) / D(white) for code in rgb]
            output = [sum(a * b for a, b in zip(row, linear)) for row in matrix]
            lines.append("\t".join(map(str, [white, *rgb, alpha, *output, D(alpha) / D(65535)])))
    filename = "pq-color-decimal-oracle.tsv"
path = Path(__file__).resolve().parent.parent / "tests/fixtures/raster" / filename
path.write_text("\n".join(lines) + "\n")
print("wrote", len(lines), "vectors")
