"""Retag independent OpenEXR float fixture; relocate absolute scanline offsets.
Channel payload remains byte-identical. No EXR codec or Rrrah code is used here.
"""
from pathlib import Path
import struct, hashlib, json, runpy
root = Path(__file__).resolve().parents[1]
fixtures = root / 'tests/fixtures/raster'
source = (fixtures / 'float-none.exr').read_bytes()
at = 8
while source[at]:
    at = source.index(0, at) + 1
    at = source.index(0, at) + 1
    length = struct.unpack_from('<I', source, at)[0]
    at += 4 + length
at += 1
first = struct.unpack_from('<Q', source, at)[0]
assert (first - at) % 8 == 0

def attribute(name, kind, value):
    return name.encode() + b'\0' + kind.encode() + b'\0' + struct.pack('<I', len(value)) + value

def tagged(attrs):
    extra = b''.join(attribute(*a) for a in attrs)
    offsets = b''.join(struct.pack('<Q', offset + len(extra))
                       for (offset,) in struct.iter_unpack('<Q', source[at:first]))
    return source[:at-1] + extra + b'\0' + offsets + source[first:]

records = {}
for code, coords, interop in [(9, [.708,.292,.170,.797,.131,.046,.3127,.3290], 'lin_rec2020_scene'),
                              (12, [.680,.320,.265,.690,.150,.060,.3127,.3290], 'lin_p3d65_scene')]:
    chroma = ('chromaticities', 'chromaticities', struct.pack('<8f', *coords))
    ident = ('colorInteropID', 'string', interop.encode())
    cases = {'chroma': [chroma], 'id': [ident], 'both': [chroma, ident],
             'id-neutral': [ident, ('adoptedNeutral','v2f',struct.pack('<2f',.3333,.3333))],
             'conflict': [chroma, ('colorInteropID','string', b'lin_rec709_scene')],
             'unknown': [chroma, ('colorInteropID','string', b'unknown')],
             'aces': [chroma, ('acesImageContainerFlag','int', struct.pack('<i',1))],
             'neutral': [chroma, ('adoptedNeutral','v2f', struct.pack('<2f',.3333,.3333))]}
    for case, attrs in cases.items():
        name = f'wide-{code}-{case}.exr'
        data = tagged(attrs)
        (fixtures / name).write_bytes(data)
        records[name] = hashlib.sha256(data).hexdigest()
for code, report in [(9, 'radiance-bt2020-matrix-oracle-2026-10-11.json'),
                     (12, 'radiance-p3-matrix-oracle-2026-10-11.json')]:
    matrix = json.loads((root / 'docs/research' / report).read_text())['matrix']
    samples = struct.unpack('<24f', (fixtures / 'float-none.exr.f32').read_bytes())
    transformed = []
    for at in range(0, len(samples), 4):
        transformed.extend(sum(row[c] * samples[at+c] for c in range(3)) for row in matrix)
        transformed.append(samples[at+3])
    name = f'wide-{code}.linear-srgb.f32'
    data = struct.pack('<24f', *transformed)
    (fixtures / name).write_bytes(data)
    records[name] = hashlib.sha256(data).hexdigest()
# A distinct adopted neutral changes adaptation, not the original RGB basis.
compute_matrix=runpy.run_path(str(root/'scripts/probe-linear-color-oracle.py'))['matrix']
for code,coords in [(9,[.708,.292,.170,.797,.131,.046,.3127,.3290]),
                    (12,[.680,.320,.265,.690,.150,.060,.3127,.3290])]:
    for tag in ['neutral','id-neutral']:
        effective=list(struct.unpack('<8f',struct.pack('<8f',*coords))) if tag=='neutral' else coords
        neutral=list(struct.unpack('<2f',struct.pack('<2f',.3333,.3333)))
        m=compute_matrix(effective,True,neutral)
        samples=struct.unpack('<24f',(fixtures/'float-none.exr.f32').read_bytes())
        values=[]
        for pixel in range(0,24,4):
            values.extend(sum(row[c]*samples[pixel+c] for c in range(3)) for row in m)
            values.append(samples[pixel+3])
        name=f'wide-{code}-{tag}.linear-srgb.f32';data=struct.pack('<24f',*values)
        (fixtures/name).write_bytes(data);records[name]=hashlib.sha256(data).hexdigest()
manifest = {'source': 'float-none.exr', 'source_sha256': hashlib.sha256(source).hexdigest(),
            'scope': 'metadata variants of independently encoded authored OpenEXR float samples; channel chunks unchanged',
            'files': records}
(root / 'tests/fixtures/exr-wide-color.json').write_text(json.dumps(manifest, indent=2)+'\n')
