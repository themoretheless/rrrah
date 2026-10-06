#!/usr/bin/env python3
"""CC0 PLY sources and independent native-column readbacks. Requires plyfile==1.1.3."""
from pathlib import Path
import hashlib
import importlib.metadata
import numpy as np
from plyfile import PlyData, PlyElement, PlyListProperty

assert importlib.metadata.version('plyfile') == '1.1.3'
root=Path(__file__).resolve().parent.parent/'tests/fixtures/models'
root.mkdir(parents=True,exist_ok=True)
types={'i1':'I8','u1':'U8','i2':'I16','u2':'U16','i4':'I32','u4':'U32','f4':'F32','f8':'F64'}
def kind(dtype):
    d=np.dtype(dtype)
    return types[d.kind+str(d.itemsize)]
def rectangles():
    triangles=[]
    for x0,y0,x1,y1 in [(0,0,3,1),(0,1,1,3),(2,1,3,3)]:
        triangles.extend([[[x0,y0,0],[x1,y0,0],[x1,y1,0]],[[x0,y0,0],[x1,y1,0],[x0,y1,0]]])
    return np.array(triangles,dtype='<f8')
manifest=['# source\ttriangles\toracle\tprovenance\tsource_sha256']
for case,precision,shift in [('32','f4',0),('64','f8',0),('offset64','f8',10**12),('u64','f8',0)]:
    points=np.array([[0,0,0],[3,0,0],[3,3,0],[2,3,0],[2,1,0],[1,1,0],[1,3,0],[0,3,0]] if case=='u64' else [[-1,-1,0],[1,-1,0],[0,1,0]],dtype=precision)
    if shift: points+=np.array([shift,-shift,shift],dtype=precision)
    dtype=[('red','u1'),('z',precision),('x',precision),('y',precision),('green','u1'),('blue','u1'),('alpha','u1'),('nx','f4'),('ny','f4'),('nz','f4'),('signed8','i1'),('signed16','i2'),('unsigned16','u2'),('signed32','i4'),('unsigned32','u4'),('temperature','f8'),('unused_nan','f4'),('samples','O')]
    vertex=np.zeros(len(points),dtype=dtype)
    for axis,name in enumerate(['x','y','z']): vertex[name]=points[:,axis]
    vertex['red']=17;vertex['green']=91;vertex['blue']=211;vertex['alpha']=200
    vertex['nz']=1;vertex['signed8']=-128;vertex['signed16']=-32768;vertex['unsigned16']=65535;vertex['signed32']=-(2**31);vertex['unsigned32']=2**32-1
    vertex['temperature']=np.pi
    vertex['unused_nan']=np.array([0x7fc12345]*len(points),dtype='u4').view('f4')
    for i in range(len(points)): vertex['samples'][i]=np.arange(i%3,dtype='f4')+0.125
    face=np.empty(1,dtype=[('vertex_indices','O'),('material_id','u2')])
    face['vertex_indices'][0]=np.arange(len(points),dtype='i4');face['material_id']=7
    edge=np.array([(0,1,0.625)],dtype=[('vertex1','i4'),('vertex2','i4'),('confidence','f4')])
    elements=[PlyElement.describe(vertex,'vertex',len_types={'samples':'u2'},val_types={'samples':'f4'}),PlyElement.describe(face,'face'),PlyElement.describe(edge,'edge')]
    for label,text,byteorder in [('ascii',True,'<'),('le',False,'<'),('be',False,'>')]:
        name=f'ply-{case}-{label}.ply'
        path=root/name
        document=PlyData(elements,text=text,byte_order=byteorder,comments=['CC0 authored geometry'],obj_info=['native type oracle'])
        if text:
            document.write(path)
        else:
            # plyfile's scalar writer selects dtype.type and loses explicit
            # endian under this numpy version. Author bytes with endian-aware
            # 0-D arrays; keep its reader as the independent readback.
            with path.open('wb') as output:
                output.write((document.header+'\n').encode('ascii'))
                for element in elements:
                    for row in element.data:
                        for prop in element.properties:
                            dtype=np.dtype(prop.val_dtype).newbyteorder(byteorder)
                            value=row[prop.name]
                            if isinstance(prop,PlyListProperty):
                                output.write(np.array(len(value),dtype=np.dtype(prop.len_dtype).newbyteorder(byteorder)).tobytes())
                                output.write(np.asarray(value,dtype=dtype).tobytes())
                            else:
                                output.write(np.array(value,dtype=dtype).tobytes())
        reference=PlyData.read(path,mmap=False)
        for axis,name_axis in enumerate(['x','y','z']):
            np.testing.assert_array_equal(reference['vertex'][name_axis],points[:,axis])
        schema=[];native=[]
        for element in reference.elements:
            for prop in element.properties:
                dtype=np.dtype(prop.val_dtype).newbyteorder('<')
                if isinstance(prop,PlyListProperty):
                    rows=[np.asarray(v,dtype=dtype) for v in element[prop.name]]
                    offsets=np.array([0]+np.cumsum([len(v) for v in rows]).tolist(),dtype='<u4')
                    values=np.concatenate(rows) if rows else np.empty(0,dtype=dtype)
                    native.append(offsets.tobytes());count=kind(prop.len_dtype);offset_count=len(offsets)
                else:
                    values=np.asarray(element[prop.name],dtype=dtype);count='-';offset_count=0
                data=values.tobytes();native.append(data)
                schema.append('\t'.join([element.name,prop.name,kind(prop.val_dtype),count,str(offset_count),str(len(data))]))
        (root/(name+'.native.bin')).write_bytes(b''.join(native))
        (root/(name+'.schema.tsv')).write_text('\n'.join(schema)+'\n')
        if case=='u64':
            triangles=rectangles();provenance='plyfile-native+authored-concave-surface'
        else:
            p=np.column_stack([reference['vertex'][axis] for axis in ['x','y','z']])
            triangles=p[np.asarray(reference['face']['vertex_indices'][0]).reshape(1,3)].astype('<f8')
            provenance='plyfile-1.1.3-native'
        oracle=name+'.triangles.f64le';(root/oracle).write_bytes(triangles.tobytes())
        manifest.append('\t'.join([name,str(len(triangles)),oracle,provenance,hashlib.sha256(path.read_bytes()).hexdigest()]))
(root/'ply-manifest.tsv').write_text('\n'.join(manifest)+'\n')
print('12 PLY sources, native columns/schemas and triangle oracles generated')
