"""Generate independent Decimal linear-sRGB oracles from pinned FFmpeg native codes.
Usage: python3 scripts/qualify-w3c-png-linear-reference.py /fixture/directory
White=100 cd/m2, HLG peak=1000 cd/m2, gamma=1.2, zero-black display.
"""
import ast
from array import array
from decimal import Decimal as D, localcontext
import hashlib
import json
import pathlib
import struct
import sys

root = pathlib.Path(sys.argv[1]).resolve()
# Reuse independent xy matrix algebra, without running its authored PNG producer.
module = ast.parse((pathlib.Path(__file__).parent / 'generate-png-narrow-pq.py').read_text())
functions = ast.Module(body=[n for n in module.body if isinstance(n, ast.FunctionDef) and n.name in ('inverse', 'multiply', 'xyz_matrix')], type_ignores=[])
exec(compile(functions, 'independent-decimal-matrix', 'exec'))
inputs = json.loads((root / 'oracle-manifest.json').read_text())['files']
results = []
with localcontext() as ctx:
    ctx.prec = 70
    points = lambda rows: [(D(x), D(y)) for x, y in rows]
    src = xyz_matrix(points([('0.708','0.292'),('0.170','0.797'),('0.131','0.046')]))
    dst = xyz_matrix(points([('0.640','0.330'),('0.300','0.600'),('0.150','0.060')]))
    matrix = multiply(inverse(dst), src)
    for record in inputs:
        name = record['name']
        data = (root / (name + '.rgb48le')).read_bytes()
        if hashlib.sha256(data).hexdigest() != record['oracle_sha256']:
            raise ValueError('native oracle hash mismatch')
        codes = array('H'); codes.frombytes(data)
        if sys.byteorder != 'little': codes.byteswap()
        triples = list(zip(codes[::3], codes[1::3], codes[2::3]))
        unique = set(triples)
        transfers = {}
        hlg = 'HLG' in name
        for code in set(codes):
            v = (D(code)-D(4096))/D(56064) if '-NR.png' in name else D(code)/D(65535)
            if hlg:
                transfers[code] = -v*v/D(3) if v < 0 else (v*v/D(3) if v <= D('0.5') else (((v-D('0.55991073'))/D('0.17883277')).exp()+D('0.28466892'))/D(12))
            else:
                p = v ** (D(32)/D(2523))
                transfers[code] = (max(p-D(107)/D(128),D(0))/(D(2413)/D(128)-D(2392)/D(128)*p))**(D(8192)/D(1305))*D(100)
        cache = {}
        for rgb in unique:
            linear = [transfers[v] for v in rgb]
            if hlg:
                y = sum(v*w for v,w in zip(linear,[D('0.2627'),D('0.6780'),D('0.0593')]))
                gain = abs(y)**D('0.2')*D(10) if y else D(0)
                linear = [v*gain for v in linear]
            converted = [sum(a*b for a,b in zip(row,linear)) for row in matrix]
            cache[rgb] = struct.pack('<4f',*[float(v) for v in converted],1.)
        output = b''.join(cache[rgb] for rgb in triples)
        path = root / (name + '.linear-rgba32le')
        path.write_bytes(output)
        results.append(dict(name=name,pixels=len(triples),unique_rgb=len(unique),decimal_precision=70,bytes=len(output),sha256=hashlib.sha256(output).hexdigest()))
        print(name, 'linear reference ready', flush=True)
(root / 'linear-oracle-manifest.json').write_text(json.dumps(results,indent=2)+'\n')
