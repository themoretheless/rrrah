#!/usr/bin/env python3
"""CC0 authored MRC2014 volumes; independent mrcfile 1.5.4 raw-unit oracles."""
from pathlib import Path
import hashlib
import mrcfile
import numpy as np

root = Path(__file__).resolve().parents[1] / 'tests/fixtures/raster'
rows = []
for typ, dtype, samples in [
    ('i8','i1',[-128,-127,-1,0,1,126,127]),
    ('i16','i2',[-32768,-257,-1,0,1,257,32767]),
    ('u16','u2',[0,1,2,257,32767,32768,65535]),
    ('f32','f4',[-100.5,-1,-0.0,0,0.5,1.0000001192092896,1000]),
    ('f16','f2',[-65504,-1,-0.0,0,2**-24,2**-14,1.0009765625,65504]),
]:
    for endian in ['little','big']:
        data = np.resize(np.array(samples,dtype=dtype),30).reshape((2,3,5)).astype(np.dtype(dtype).newbyteorder('>' if endian=='big' else '<'))
        name = f'mrc-{typ}-{endian}.mrc'
        path = root / name
        with mrcfile.new(str(path),overwrite=True) as out:
            out.set_data(data)
            out.header.label[:] = b''
            out.header.label[0] = b'CC0 scalar volume'
            out.header.nlabl = 1
            out.header.mapc, out.header.mapr, out.header.maps = 3,1,2
            out.header.origin.x, out.header.origin.y, out.header.origin.z = 12.5,-3,7
            out.header.exttyp = b'TEST'
            out.set_extended_header(np.arange(32,dtype='u1'))
        if typ == 'i8' and endian == 'big':
            encoded = path.read_bytes()
            header = np.frombuffer(encoded[:1024], dtype=mrcfile.dtypes.HEADER_DTYPE).astype(mrcfile.dtypes.HEADER_DTYPE.newbyteorder('>'))
            header['machst'] = [0x11,0x11,0,0]
            path.write_bytes(header.tobytes()+encoded[1024:])
        with mrcfile.open(str(path),permissive=False) as decoded:
            assert np.array_equal(decoded.data,data)
            assert decoded.header.machst[0] == (0x11 if endian=='big' else 0x44)
            for index in range(2):
                rgba = np.ones((15,4),dtype='<f4')
                rgba[:,:3] = decoded.data[index].flatten().astype('<f4')[:,None]
                oracle = name+f'-{index}.rgba32f'
                (root/oracle).write_bytes(rgba.tobytes())
                rows.append(f'{name}\t{index}\t5\t3\t{oracle}\t{hashlib.sha256(path.read_bytes()).hexdigest()}')
(root/'mrc-manifest.tsv').write_text('# mrcfile 1.5.4; file\tindex\twidth\theight\toracle\tsha256\n'+'\n'.join(rows)+'\n')
# Independent exhaustive IEEE half conversion: every input bit pattern, including NaN payloads.
bits = np.arange(65536,dtype='<u2')
with np.errstate(invalid='ignore'):
    (root/'mrc-half-all.r32f').write_bytes(bits.view('<f2').astype('<f4').tobytes())
