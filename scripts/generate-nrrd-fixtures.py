#!/usr/bin/env python3
"""CC0 authored NRRD arrays; independent pynrrd 1.1.3 readback oracles."""
from pathlib import Path
import hashlib
import nrrd
import numpy as np

root = Path(__file__).resolve().parents[1] / 'tests/fixtures/raster'
rows = []
for typ, dtype, samples in [
    ('u8', 'uint8', [0, 1, 2, 127, 128, 254, 255]),
    ('u16', 'uint16', [0, 1, 2, 257, 32767, 32768, 65535]),
    ('i16', 'int16', [-32768, -257, -1, 0, 1, 257, 32767]),
    ('f32', 'float32', [-100.5, -1, -0.0, 0, 0.5, 1.0000001192092896, 1000]),
]:
    data = np.resize(np.array(samples, dtype=dtype), 30).reshape((5, 3, 2), order='F')
    for encoding, endian in [('raw', 'little'), ('raw', 'big'), ('gzip', 'little'), ('ascii', 'little')]:
        name = f'nrrd-{typ}-{encoding}-{endian}.nrrd'
        path = root / name
        stored = data.astype(data.dtype.newbyteorder('>' if endian == 'big' else '<'))
        nrrd.write(str(path), stored, header={'encoding': encoding, 'endian': endian,
            'kinds': ['domain'] * 3, 'content': 'CC0 scalar volume',
            'measurement': 'raw units'}, index_order='F')
        encoded = path.read_bytes()
        encoded = b'\n'.join(line for line in encoded.split(b'\n') if not line.startswith(b'# on '))
        path.write_bytes(encoded)
        actual, header = nrrd.read(str(path), index_order='F')
        if data.dtype.itemsize > 1 and encoding != 'ascii':
            assert header['endian'] == endian
        assert np.array_equal(actual, data)
        for index in range(2):
            values = actual[:, :, index].flatten(order='F').astype('<f4')
            rgba = np.ones((15, 4), dtype='<f4')
            rgba[:, :3] = values[:, None]
            oracle = name + f'-{index}.rgba32f'
            (root / oracle).write_bytes(rgba.tobytes())
            rows.append(f'{name}\t{index}\t5\t3\t{oracle}\t{hashlib.sha256(path.read_bytes()).hexdigest()}')
(root / 'nrrd-manifest.tsv').write_text('# pynrrd 1.1.3; file\tindex\twidth\theight\toracle\tsha256\n' + '\n'.join(rows) + '\n')
