#!/usr/bin/env python3
"""CC0 authored 24-bit CUR entries with padded DIB rows and AND masks."""
from pathlib import Path
from PIL import Image
import io,struct,json,hashlib
root=Path(__file__).resolve().parents[1]/'tests/fixtures/raster'
entries=[(2,2,[255,0,0,255],[0,1]),(2,2,[0,255,0,255],[1,0]),(1,1,[0,0,255,0],[0,0])]
streams=[]
for w,h,pixel,point in entries:
    b=io.BytesIO();Image.new('RGB',(w,h),tuple(pixel[:3])).save(b,format='BMP')
    bmp=b.getvalue();start=struct.unpack_from('<I',bmp,10)[0]
    header=bytearray(bmp[14:54]);assert struct.unpack_from('<I',header)[0]==40
    struct.pack_into('<i',header,8,h*2)
    mask=bytes([255 if pixel[3]==0 else 0])*(((w+31)//32)*4*h)
    stream=bytes(header)+bmp[start:]+mask
    ico=struct.pack('<HHHBBBBHHII',0,1,1,w,h,0,0,1,24,len(stream),22)+stream
    with Image.open(io.BytesIO(ico)) as im:assert im.convert('RGBA').tobytes()==bytes(pixel)*(w*h)
    streams.append(stream)
offset=6+16*len(entries);out=bytearray(struct.pack('<HHH',0,2,len(entries)))
for (w,h,_,point),stream in zip(entries,streams):
    out+=struct.pack('<BBBBHHII',w,h,0,0,*point,len(stream),offset);offset+=len(stream)
out+=b''.join(streams)
p=root/'cur-selected-dib.cur';p.write_bytes(out)
(root/'cur-selected-dib.json').write_text(json.dumps({'license':'CC0-1.0','entries':entries,'oracle':'Pillow independently reads each DIB as ICO including AND mask','sha256':hashlib.sha256(out).hexdigest()},indent=2)+'\n')
