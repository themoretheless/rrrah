"""CC0 linear DXT2/DXT4 PVR wrappers; Pillow BC samples plus explicit float32 unassociation."""
from pathlib import Path
from PIL import Image
from io import BytesIO
import hashlib
import struct
ROOT = Path(__file__).resolve().parent.parent / 'tests/fixtures/raster'
def f32(x):
    return struct.unpack('<f', struct.pack('<f', x))[0]
lines = ['# source\tmip\twidth\theight\tkind\tcolor\toracle\tover\tsha256']
for fmt, fourcc in [(8, b'DXT3'), (10, b'DXT5')]:
    encoded, native = [], []
    for index in range(9):
        color = struct.pack('<HHI', 0xffff if index % 2 else 0, 0 if index % 2 else 0xffff, 0xe4e4e4e4)
        alpha = bytes([0x88]*8) if fmt == 8 else bytes([128, 64]) + int('7654321076543210',8).to_bytes(6,'little')
        block = alpha + color
        header = [124,0x81007,4,4,16,0,1] + [0]*11
        header += [32,4,int.from_bytes(fourcc,'little'),0,0,0,0,0,0x1000,0,0,0,0]
        with Image.open(BytesIO(b'DDS '+struct.pack('<31I',*header)+block)) as image:
            pixels = image.convert('RGBA').tobytes()
        output = bytearray()
        for offset in range(0,64,4):
            rgba = pixels[offset:offset+4]
            a = f32(rgba[3]/255)
            values = [f32(f32(c/255)/a) for c in rgba[:3]] + [a]
            output.extend(struct.pack('<4f',*values))
        encoded.append(block); native.append(bytes(output))
    for fx in [0,1]:
        for fy in [0,1]:
            name=f'pvr-dxt{2 if fmt==8 else 4}-linear-{fx}{fy}.pvr'
            custom=b'CC0 custom metadata'
            metadata=b'PVR\x03'+struct.pack('<II',3,3)+bytes([fx,fy,0])+b'CC0!'+struct.pack('<II',42,len(custom))+custom
            source=b'PVR\x03'+struct.pack('<IQ9I',0,fmt,0,0,5,9,1,1,1,4,len(metadata))+metadata
            records=[];start=0
            for level in range(4):
                w,h=max(1,9>>level),max(1,5>>level)
                cols,rows=(w+3)//4,(h+3)//4
                count=cols*rows
                source+=b''.join(encoded[start:start+count])
                output=bytearray()
                for y in range(h):
                    for x in range(w):
                        sx,sy=(w-1-x if fx else x),(h-1-y if fy else y)
                        block=start+(sy//4)*cols+sx//4
                        at=((sy%4)*4+sx%4)*16
                        output.extend(native[block][at:at+16])
                oracle=f'{name}.mip{level}.rgba32f'
                (ROOT/oracle).write_bytes(output)
                records.append(f'{name}\t{level}\t{w}\t{h}\trgba32f\t0\t{oracle}\t-')
                start+=count
            (ROOT/name).write_bytes(source)
            digest=hashlib.sha256(source).hexdigest()
            lines.extend(record+'\t'+digest for record in records)
(ROOT/'pvr-associated-bc-manifest.tsv').write_text('\n'.join(lines)+'\n')
print('8 linear DXT2/DXT4 sources;32 selected mip oracles')
