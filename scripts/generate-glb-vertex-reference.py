#!/usr/bin/env python3
"""Producer BoxTextured corner reference using Python json/struct, no rrrah runtime."""
import hashlib,json,struct
from pathlib import Path
root=Path(__file__).resolve().parents[1]/'tests/fixtures/glb'
source=root/'BoxTextured.glb';b=source.read_bytes()
n=struct.unpack_from('<I',b,12)[0];d=json.loads(b[20:20+n]);binary=20+n+8
p=d['meshes'][0]['primitives'][0]
def values(index):
    a=d['accessors'][index];v=d['bufferViews'][a['bufferView']]
    count={'SCALAR':1,'VEC2':2,'VEC3':3}[a['type']]
    fmt={5123:'H',5126:'f'}[a['componentType']];size=struct.calcsize('<'+fmt)
    return [struct.unpack_from('<'+fmt*count,b,binary+v.get('byteOffset',0)+a.get('byteOffset',0)+i*v.get('byteStride',count*size)) for i in range(a['count'])]
indices=values(p['indices']);pos=values(p['attributes']['POSITION']);norm=values(p['attributes']['NORMAL']);uv=values(p['attributes']['TEXCOORD_0'])
rows=[]
for corner,(vertex,) in enumerate(indices):
    rows.append((corner//3,corner%3,vertex,*pos[vertex],*norm[vertex],*uv[vertex]))
output=root/'BoxTextured-vertices.tsv';output.write_text(''.join('\t'.join(map(str,row))+'\n' for row in rows))
(root/'vertex-reference-manifest.json').write_text(json.dumps(dict(scope='original producer local-space indexed positions, normals, TEXCOORD_0',corners=len(rows),source_sha256=hashlib.sha256(b).hexdigest(),reference_sha256=hashlib.sha256(output.read_bytes()).hexdigest()),indent=2)+'\n')
