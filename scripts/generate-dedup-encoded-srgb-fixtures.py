"""80-digit independent Decimal sRGB-transfer and encoded affine fixtures."""
from pathlib import Path
from decimal import Decimal as D, getcontext
import hashlib, json, math, struct
getcontext().prec = 80
root = Path(__file__).resolve().parents[1] / 'crates/rrrah-dedup/tests/fixtures/encoded-srgb'
root.mkdir(parents=True, exist_ok=True)
def decode(v):
    if abs(v) <= D('.04045'):
        return v / D('12.92')
    return ((abs(v) + D('.055')) / D('1.055')) ** D('2.4') * (-1 if v < 0 else 1)
def encode(v):
    if abs(v) <= D('.0031308'):
        return v * D('12.92')
    return (D('1.055') * abs(v) ** (D(5) / D(12)) - D('.055')) * (-1 if v < 0 else 1)
values = ['-10000', '-4', '-1', '-.18', '-.0031308', '-.001', '0', '.001', '.0031308', '.18', '1', '4', '10000']
(root / 'transfer.tsv').write_text('\n'.join(f'{v}\t{encode(D(v))}' for v in values) + '\n')
data = {name: bytearray() for name in ['source', 'affine', 'broad-edit', 'alpha-edit']}
for y in range(17):
    for x in range(17):
        source = [(D(x) + D('.5')) / 17, (D(y) + D('.5')) / 17, D(x + y + 1) / 34]
        target = [v * D('.7') + D('.02') for v in source]
        broad = [v + D('.15') if 4 <= x <= 12 and 4 <= y <= 12 else v for v in target]
        for name, values_here in [('source', source), ('affine', target), ('broad-edit', broad), ('alpha-edit', target)]:
            alpha = 0 if name == 'alpha-edit' and 7 <= x <= 9 and 7 <= y <= 9 else 1
            rgb = [float(decode(v)) for v in values_here] if alpha else [math.nan] * 3
            data[name].extend(struct.pack('<4f', *rgb, alpha))
for name, pixels in data.items():
    (root / f'{name}.rgba32le').write_bytes(pixels)
files = sorted(p for p in root.iterdir() if p.name != 'manifest.json')
(root / 'manifest.json').write_text(json.dumps({'generator': Path(__file__).name, 'license': 'CC0-1.0', 'reference': 'https://www.w3.org/TR/2026/CRD-css-color-4-20260930/#color-conversion-code', 'precision_decimal_digits': 80, 'dimensions': [17, 17], 'scope': 'Authored encoded-space affine gain .7 and offset .02, central 9x9 encoded additive edit .15, central 3x3 alpha removal. Mathematical fixtures; no native rrrah conversion used.', 'files': [{'file': p.name, 'sha256': hashlib.sha256(p.read_bytes()).hexdigest()} for p in files]}, indent=2) + '\n')
