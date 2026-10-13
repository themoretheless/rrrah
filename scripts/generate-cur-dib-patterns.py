#!/usr/bin/env python3
"""CC0 9x3 DIB cursors with packed palettes, bottom-up rows and mixed masks."""
from pathlib import Path
from PIL import Image
import struct,io,json,hashlib
root=Path(__file__).resolve().parents[1]/'tests/fixtures/data'
colors=[(255,0,0),(0,255,0),(0,0,255),(255,255,0)]
for depth in [1,4,8,16,24,32]:
    w,h=9,3;rgba=bytearray();rows=[];masks=[]
    for y in range(h):
        row=bytearray((w*depth+7)//8);mask=bytearray(4)
        for x in range(w):
            i=(x+y)% (2 if depth==1 else 4);r,g,b=colors[i]
            alpha=[0,64,128,255][(x+2*y)%4] if depth==32 else (0 if (x+y)%3==0 else 255)
            rgba+=bytes((r,g,b,alpha))
            if depth==1:row[x//8]|=i<<(7-x%8)
            elif depth==4:row[x//2]|=i<<(4 if x%2==0 else 0)
            elif depth==8:row[x]=i
            elif depth==16:struct.pack_into('<H',row,x*2,((r>>3)<<10)|((g>>3)<<5)|(b>>3))
            else:row[x*(depth//8):(x+1)*(depth//8)]=bytes((b,g,r,alpha))[:depth//8]
            if alpha==0:mask[x//8]|=1<<(7-x%8)
        row+=bytes((-len(row))%4);rows.append(row);masks.append(mask)
    palette=b'' if depth>=16 else b''.join(bytes((b,g,r,0)) for r,g,b in colors[:2 if depth==1 else 4])+bytes(4*((1<<depth)-(2 if depth==1 else 4)))
    xor=b''.join(reversed(rows));dib=struct.pack('<IiiHHIIiiII',40,w,2*h,1,depth,0,len(xor),0,0,0,0)+palette+xor+b''.join(reversed(masks))
    ico=struct.pack('<HHHBBBBHHII',0,1,1,w,h,0,0,1,depth,len(dib),22)+dib
    with Image.open(io.BytesIO(ico)) as im:assert im.convert('RGBA').tobytes()==rgba,depth
    cur=struct.pack('<HHHBBBBHHII',0,2,1,w,h,0,0,8,2,len(dib),22)+dib
    p=root/f'cur-dib-pattern-{depth}.cur';p.write_bytes(cur);p.with_suffix('.rgba8').write_bytes(rgba)
    p.with_suffix('.json').write_text(json.dumps({'license':'CC0-1.0','size':[w,h],'hotspot':[8,2],'depth':depth,'oracle':'Pillow independent per-entry ICO read','sha256':hashlib.sha256(cur).hexdigest()},indent=2)+'\n')
