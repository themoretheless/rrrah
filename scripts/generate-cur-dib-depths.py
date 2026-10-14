#!/usr/bin/env python3
"""CC0 uncompressed indexed and BGRA CUR entries, with independent Pillow reads."""
from pathlib import Path
from PIL import Image
import io,struct,json,hashlib
root=Path(__file__).resolve().parents[1]/'tests/fixtures/raster'
for depth in [1,4,8,16,32]:
    entries=[(2,2,[255,0,0,128 if depth==32 else 255],[0,1]),(2,2,[0,255,0,64 if depth==32 else 255],[1,0]),(1,1,[0,0,255,255 if depth==32 else 0],[0,0])]
    streams=[]
    for w,h,pixel,point in entries:
        stride=((w*depth+31)//32)*4
        palette=b'' if depth>=16 else bytes([0,0,0,0,pixel[2],pixel[1],pixel[0],0])+bytes(4*((1<<depth)-2))
        row=(struct.pack('<H',((pixel[0]>>3)<<10)|((pixel[1]>>3)<<5)|(pixel[2]>>3))*w if depth==16 else bytes([pixel[2],pixel[1],pixel[0],pixel[3]])*w if depth==32 else bytes([sum(1<<(7-i) for i in range(w))]) if depth==1 else bytes([0x11]) if depth==4 and w==2 else bytes([0x10]) if depth==4 else bytes([1])*w)
        row=row+bytes(stride-len(row));xor=row*h
        mask=bytes([255 if pixel[3]==0 else 0])*(((w+31)//32)*4*h)
        header=struct.pack('<IiiHHIIiiII',40,w,h*2,1,depth,0,len(xor),0,0,0,0)
        stream=header+palette+xor+mask
        ico=struct.pack('<HHHBBBBHHII',0,1,1,w,h,0,0,1,depth,len(stream),22)+stream
        with Image.open(io.BytesIO(ico)) as im:assert im.convert('RGBA').tobytes()==bytes(pixel)*(w*h),(depth,im.convert('RGBA').tobytes())
        streams.append(stream)
    offset=54;out=bytearray(struct.pack('<HHH',0,2,3))
    for (w,h,_,point),stream in zip(entries,streams):
        out+=struct.pack('<BBBBHHII',w,h,0,0,*point,len(stream),offset);offset+=len(stream)
    out+=b''.join(streams);p=root/f'cur-selected-dib-{depth}.cur';p.write_bytes(out)
    p.with_suffix('.json').write_text(json.dumps({'license':'CC0-1.0','depth':depth,'entries':entries,'oracle':'Pillow per-entry ICO DIB decode','sha256':hashlib.sha256(out).hexdigest()},indent=2)+'\n')
