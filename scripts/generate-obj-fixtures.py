#!/usr/bin/env python3
"""CC0 OBJ sources, independent trimesh oracles and explicit authored concave oracle.
Requires numpy and trimesh==4.11.2 in the external Python fixture environment.
"""
from pathlib import Path
import hashlib
import struct
import numpy as np
import trimesh

root = Path(__file__).resolve().parent.parent / 'tests/fixtures/models'
root.mkdir(parents=True, exist_ok=True)
assert trimesh.__version__ == '4.11.2'
cases = {
    'obj-triangle.obj': 'v -1 -1 0\nv 1 -1 0\nv 0 1 0\nf 1 2 3\n',
    'obj-negative-quad.obj': 'o quad\ng one two\ns 4\nv 0 0 0\nv 2 0 0\nv 2 2 0\nv 0 2 0\nvt 0 0\nvt 1 0\nvt 1 1\nvt 0 1\nvn 0 0 1\nf -4/-4/-1 -3/-3/-1 -2/-2/-1 -1/-1/-1\n',
    'obj-clockwise.obj': 'v 0 0 0\nv 0 2 0\nv 2 2 0\nv 2 0 0\nf 1 2 3 4\n',
    'obj-weight.obj': 'v 4 0 0 2\nv 0 2 0\nv 0 0 0\nf 1 2 3\n',
    'obj-yz.obj': 'v 0 0 0\nv 0 2 0\nv 0 2 2\nv 0 0 2\nf 1 2 3 4\n',
    'obj-concave-u.obj': '# CC0 U shape: area 7, open notch x=1..2 above y=1.\nv 0 0 0\nv 3 0 0\nv 3 3 0\nv 2 3 0\nv 2 1 0\nv 1 1 0\nv 1 3 0\nv 0 3 0\nf 1 2 3 4 5 6 7 8\n',
    'concave.obj': '# CC0 authored L shape. The missing upper-right square must remain empty.\no L-shape\ng geometry\ns off\nv 0 0 0\nv 2 0 0\nv 2 1 0\nv 1 1 0\nv 1 2 0\nv 0 2 0\nf 1 2 3 4 5 6\n',
}
manifest = ['# source\toracle\tprovenance\tsource_sha256\toracle_sha256']
for name, source in cases.items():
    path = root / name
    path.write_text(source)
    if name == 'obj-concave-u.obj':
        rectangles = [(0,0,3,1),(0,1,1,3),(2,1,3,3)]
        authored = []
        for x0,y0,x1,y1 in rectangles:
            authored.extend([[[x0,y0,0],[x1,y0,0],[x1,y1,0]],[[x0,y0,0],[x1,y1,0],[x0,y1,0]]])
        triangles = np.array(authored, dtype=np.float32)
        provenance = 'authored-area-and-cutout'
    elif name == 'concave.obj':
        points = np.array([[0,0,0],[2,0,0],[2,1,0],[1,1,0],[1,2,0],[0,2,0]], dtype=np.float32)
        triangles = points[np.array([[0,1,3],[1,2,3],[0,3,5],[3,4,5]])]
        provenance = 'authored-area-and-cutout'
    else:
        model = trimesh.load(path, process=False, force='mesh')
        triangles = np.asarray(model.triangles, dtype=np.float32)
        provenance = 'trimesh-4.11.2'
    oracle = name + '.triangles.f32le'
    data = struct.pack('<' + 'f' * triangles.size, *triangles.flatten())
    (root / oracle).write_bytes(data)
    manifest.append('\t'.join([name, oracle, provenance, hashlib.sha256(path.read_bytes()).hexdigest(), hashlib.sha256(data).hexdigest()]))
(root / 'obj-manifest.tsv').write_text('\n'.join(manifest) + '\n')
print(f'{len(cases)} OBJ sources and geometry oracles generated')
