#!/usr/bin/env python3
"""Author a CC0 binary STL grid for reproducible load measurements."""
import argparse
import pathlib
import struct

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument('--directory', type=pathlib.Path, default=pathlib.Path('target/bench/models'))
parser.add_argument('--side', type=int, default=500)
args = parser.parse_args()
if not 1 <= args.side <= 4096:
    parser.error('--side must be between 1 and 4096')
args.directory.mkdir(parents=True, exist_ok=True)
path = args.directory / 'grid.stl'
with path.open('wb') as output:
    output.write(b'CC0 authored planar grid; no units or color'.ljust(80, b'\0'))
    output.write(struct.pack('<I', args.side * args.side))
    for y in range(args.side):
        for x in range(args.side):
            output.write(struct.pack('<12fH', 0, 0, 1, x, y, 0, x + 1, y, 0, x, y + 1, 0, 0))
print(f'{path}: {args.side * args.side} facets, {path.stat().st_size} bytes')

obj = args.directory / 'grid.obj'
with obj.open('w') as output:
    output.write('# CC0 authored planar grid; no units or materials\no grid\ng geometry\ns off\n')
    for y in range(args.side + 1):
        for x in range(args.side + 1):
            output.write(f'v {x} {y} 0\n')
    output.write('vn 0 0 1\n')
    stride = args.side + 1
    for y in range(args.side):
        for x in range(args.side):
            first = y * stride + x + 1
            output.write(f'f {first}//1 {first + 1}//1 {first + stride}//1\n')
print(f'{obj}: {args.side * args.side} triangles, {obj.stat().st_size} bytes')

for precision,code,shift in [('32','f',0),('64','d',10**12)]:
    ply = args.directory / f'grid{precision}.ply'
    scalar = 'float' if precision == '32' else 'double'
    header = f'ply\nformat binary_little_endian 1.0\ncomment CC0 authored grid\nelement vertex {(args.side+1)**2}\nproperty {scalar} x\nproperty {scalar} y\nproperty {scalar} z\nelement face {args.side**2}\nproperty list uchar int vertex_indices\nend_header\n'
    with ply.open('wb') as output:
        output.write(header.encode('ascii'))
        for y in range(args.side+1):
            for x in range(args.side+1):
                output.write(struct.pack('<3'+code,x+shift,y-shift,shift))
        for y in range(args.side):
            for x in range(args.side):
                first=y*(args.side+1)+x
                output.write(struct.pack('<B3i',3,first,first+1,first+args.side+1))
    print(f'{ply}: {args.side**2} triangles, {ply.stat().st_size} bytes')

for binary in [False, True]:
    off=args.directory/('grid-binary.off' if binary else 'grid-ascii.off')
    with off.open('wb') as output:
        if binary:
            output.write(b'OFF BINARY # CC0 authored grid\n')
            output.write(struct.pack('>3i',(args.side+1)**2,args.side**2,0))
        else:
            output.write(f'OFF\n# CC0 authored grid\n{(args.side+1)**2} {args.side**2} 0\n'.encode())
        for y in range(args.side+1):
            for x in range(args.side+1):
                output.write(struct.pack('>3f',x,y,0) if binary else f'{x} {y} 0\n'.encode())
        for y in range(args.side):
            for x in range(args.side):
                first=y*(args.side+1)+x
                output.write(struct.pack('>5i',3,first,first+1,first+args.side+1,0) if binary else f'3 {first} {first+1} {first+args.side+1}\n'.encode())
    print(f'{off}: {args.side**2} triangles, {off.stat().st_size} bytes')
