#!/usr/bin/env python3
"""CC0 authored base-header EMF stock-object rectangles and mathematical RGBA."""
import pathlib, struct, hashlib, json
root=pathlib.Path(__file__).resolve().parents[1]/'tests/fixtures/emf'
root.mkdir(parents=True,exist_ok=True)
rows=[]
for name,brush,rgb in [('black',4,(0,0,0)),('white',0,(255,255,255)),('red',None,(255,0,0)),('green',None,(0,255,0)),('blue',None,(0,0,255)),('saved-context',None,(255,0,0)),('polygon32',None,(255,0,0)),('polygon16',None,(255,0,0)),('fill-evenodd',None,(255,0,0)),('fill-winding',None,(255,0,0)),('fill-restored',None,(255,0,0)),('line-restored',None,(255,0,0)),('line-null-advance',None,(255,0,0)),('object-reuse',None,(0,0,255)),('header100',None,(255,0,0)),('header108',None,(255,0,0)),('description88',None,(255,0,0)),('description100',None,(255,0,0)),('description108',None,(255,0,0)),('mapping-lowmetric',None,(255,0,0)),('mapping-highmetric',None,(255,0,0)),('mapping-anisotropic',None,(255,0,0)),('mapping-restored',None,(255,0,0)),('mapping-reflected',None,(255,0,0))]+[(name,None,(255,0,0)) for name in ['bezier32','bezier16','bezierto32','bezierto16','lineto32','lineto16']]:
    header=bytearray(88)
    for offset,value in [(0,1),(4,88),(16,9),(20,9),(32,225),(36,225),(40,0x464d4520),(44,0x10000),(52,5),(56,1),(72,100),(76,100),(80,25),(84,25)]:
        struct.pack_into('<I',header,offset,value)
    selection = struct.pack('<3I',37,12,0x80000000|brush) if brush is not None else struct.pack('<6I',39,24,1,0,rgb[0]|rgb[1]<<8|rgb[2]<<16,0)+struct.pack('<3I',37,12,1)
    if brush is None:
        struct.pack_into('<I',header,52,6)
        struct.pack_into('<I',header,56,2)
    records=selection+struct.pack('<3I',37,12,0x80000008)+struct.pack('<6I',43,24,0,0,10,10)+struct.pack('<5I',14,20,0,0,20)
    if name == 'saved-context':
        insert=struct.pack('<2I',33,8)+struct.pack('<3I',37,12,0x80000004)+struct.pack('<2Ii',34,12,-1)
        records=records[:-44]+insert+records[-44:]
        struct.pack_into('<I',header,52,9)
    if name.startswith('polygon'):
        short = name == 'polygon16'
        points = struct.pack('<8h' if short else '<8i',0,0,10,0,10,10,0,10)
        polygon = struct.pack('<7I',86 if short else 3,28+len(points),0,0,9,9,4)+points
        records=records[:-44]+polygon+records[-20:]
    if name.startswith('fill-'):
        background=struct.pack('<3I',37,12,0x80000000)+struct.pack('<6I',43,24,0,0,10,10)+struct.pack('<3I',37,12,1)
        mode=struct.pack('<3I',19,12,2 if name == 'fill-winding' else 1)
        if name == 'fill-restored':
            mode+=struct.pack('<2I',33,8)+struct.pack('<3I',19,12,2)+struct.pack('<2Ii',34,12,-1)
        points=struct.pack('<16i',0,0,10,0,10,10,0,10,0,0,10,0,10,10,0,10)
        polygon=struct.pack('<7I',3,28+len(points),0,0,9,9,8)+points
        records=records[:-44]+background+mode+polygon+records[-20:]
        rgb=(255,0,0) if name == 'fill-winding' else (255,255,255)
    if name.startswith('line-'):
        pen=struct.pack('<7I',38,28,2,0,10,0,255)+struct.pack('<3I',37,12,2)
        move=struct.pack('<2I2i',27,16,0,5)
        saved=struct.pack('<2I',33,8)+struct.pack('<2I2i',27,16,0,0)+struct.pack('<2Ii',34,12,-1)
        line=struct.pack('<2I2i',54,16,10,5)
        if name == 'line-null-advance':
            advance=struct.pack('<2I2i',27,16,0,0)+struct.pack('<2I2i',54,16,0,5)
            records=records[:-44]+advance+pen+line+records[-20:]
        else:
            records=records[:-44]+pen+move+saved+line+records[-20:]
        struct.pack_into('<I',header,56,3)
    if name == 'object-reuse':
        reuse=struct.pack('<3I',37,12,0x80000000)+struct.pack('<3I',40,12,1)+struct.pack('<6I',39,24,1,0,0xff0000,0)+struct.pack('<3I',37,12,1)
        records=records[:-44]+reuse+records[-44:]
    if name.startswith('mapping-'):
        if name in ('mapping-lowmetric','mapping-highmetric'):
            low=name=='mapping-lowmetric'
            n=25 if low else 250
            state=struct.pack('<3I',17,12,2 if low else 3)
            rectangle=struct.pack('<2I4i',43,24,0,-n,n,0)
        else:
            state=struct.pack('<3I',17,12,8)+struct.pack('<2I2i',9,16,20,20)+struct.pack('<2I2i',11,16,10,10)
            if name == 'mapping-restored':
                state+=struct.pack('<2I',33,8)+struct.pack('<2I2i',9,16,100,100)+struct.pack('<2Ii',34,12,-1)
            if name == 'mapping-reflected':
                state+=struct.pack('<2I2i',11,16,-10,10)+struct.pack('<2I2i',12,16,10,0)
            rectangle=struct.pack('<6I',43,24,0,0,20,20)
        records=records[:-44]+state+rectangle+records[-20:]
    if name.startswith(('bezier','lineto')):
        short=name.endswith('16')
        continuing=name.startswith(('bezierto','lineto'))
        line=name.startswith('lineto')
        kind=(89 if short else 6) if line else ((88 if short else 5) if continuing else (85 if short else 2))
        values=[10,5] if line else ([3,5,7,5,10,5] if continuing else [0,5,3,5,7,5,10,5])
        points=struct.pack('<'+str(len(values))+('h' if short else 'i'),*values)
        command=struct.pack('<7I',kind,28+len(points),0,0,9,9,len(values)//2)+points
        pen=struct.pack('<7I',38,28,2,0,10,0,255)+struct.pack('<3I',37,12,2)
        move=struct.pack('<2I2i',27,16,0,5)
        records=records[:-44]+pen+move+command+records[-20:]
        struct.pack_into('<I',header,56,3)
    record_count=1
    at=0
    while at<len(records):
        record_count+=1
        at+=struct.unpack_from('<I',records,at+4)[0]
    assert at==len(records)
    struct.pack_into('<I',header,52,record_count)
    if name.startswith('header'):
        size=int(name[6:])
        header.extend(bytes(size-88))
        struct.pack_into('<I',header,4,size)
        if size == 108:
            struct.pack_into('<2I',header,100,25000,25000)
    if name.startswith('description'):
        fixed=int(name[11:])
        header.extend(bytes(fixed-88))
        description='rrrah\0Изображение\0\0'.encode('utf-16le')
        struct.pack_into('<2I',header,60,len(description)//2,fixed)
        header.extend(description)
        header.extend(bytes((-len(header))%4))
        struct.pack_into('<I',header,4,len(header))
    data=header+records;struct.pack_into('<I',data,48,len(data))
    filename=name+'.emf';(root/filename).write_bytes(data)
    (root/(filename+'.rgba')).write_bytes(bytes((*rgb,255))*100)
    rows.append(dict(file=filename,sha256=hashlib.sha256(data).hexdigest(),width=10,height=10,reference='mathematical opaque fill, null stock pen; exact independent comparison recorded separately'))
(root/'manifest.json').write_text(json.dumps(dict(license='CC0-1.0',images=rows),indent=2)+'\n')
