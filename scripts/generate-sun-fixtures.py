"""CC0 synthetic Sun Raster fixtures, decoded independently by Pillow/FFmpeg.
Run after generate-raster-fixtures.py to add the Sun corpus entries.
"""
from pathlib import Path
import hashlib
import struct
import subprocess
from PIL import Image
root = Path(__file__).resolve().parent.parent / 'tests/fixtures/raster'
width, height = 3, 2
rgb = bytes([255,0,0, 0,255,0, 0,0,255, 128,128,128, 255,255,255, 0,0,0])
def header(depth, kind, payload, palette=b''):
    return struct.pack('>8I',0x59a66a95,width,height,depth,len(payload),kind,int(bool(palette)),len(palette))+palette+payload
def rle(raw):
    result=bytearray(); i=0
    while i<len(raw):
        n=1
        while i+n<len(raw) and raw[i+n]==raw[i] and n<256: n+=1
        if n>1: result.extend([128,n-1,raw[i]])
        elif raw[i]==128: result.extend([128,0])
        else: result.append(raw[i])
        i+=n
    return result
bgr=bytearray()
for y in range(height):
    for x in range(width):
        p=rgb[(y*width+x)*3:(y*width+x+1)*3];bgr.extend(p[::-1])
    bgr.append(0)
rows=rgb[:9]+b'\0'+rgb[9:]+b'\0'
palette=bytes([255,0,0,128,255,0, 0,255,0,128,255,0, 0,0,255,128,255,0])
xrgb=bytes(v for i in range(0,len(rgb),3) for v in (i*7%256,*rgb[i:i+3]))
xbgr=bytes(v for i in range(0,len(rgb),3) for v in (i*7%256,*rgb[i:i+3][::-1]))
fixtures={
    'sun-xrgb.ras':header(32,3,xrgb),
    'sun-xbgr.ras':header(32,1,xbgr),
    'sun-xbgr-rle.ras':header(32,2,rle(xbgr)),
    'sun-bgr.ras':header(24,1,bgr),
    'sun-rgb.ras':header(24,3,rows),
    'sun-rle.ras':header(24,2,rle(bgr)),
    'sun-palette.ras':header(8,1,bytes([0,1,2,0,3,4,5,0]),palette),
    'sun-mono.ras':header(1,1,bytes([0xa0,0,0x40,0])),
}
lines=[line for line in (root/'manifest.tsv').read_text().splitlines() if not line.startswith('sun-')]
for name,data in fixtures.items():
    path=root/name;path.write_bytes(data)
    with Image.open(path) as image:
        image.load(); oracle=image.convert('RGBA').tobytes()
    if name == 'sun-rle.ras' or name.startswith('sun-x'):
        # Pillow sun_rle skips odd-width row padding. FFmpeg honors it.
        oracle = subprocess.check_output(['ffmpeg', '-v', 'error', '-i', str(path), '-f', 'rawvideo', '-pix_fmt', 'rgba', 'pipe:1'])
        expected = bytes(v for i in range(0, len(rgb), 3) for v in (*rgb[i:i+3], 255))
        assert oracle == expected
    (root/(name+'.rgba')).write_bytes(oracle)
    lines.append(f'{name}\t{width}\t{height}\t0\t{name}.rgba\t{hashlib.sha256(data).hexdigest()}')
(root/'manifest.tsv').write_text('\n'.join(lines)+'\n')
