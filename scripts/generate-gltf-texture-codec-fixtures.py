#!/usr/bin/env python3
"""CC0 sample/framing controls; existing JPEG and independent FFmpeg PNG oracles."""
import hashlib,json,struct,subprocess,tempfile,zlib
from pathlib import Path
ROOT=Path(__file__).resolve().parents[1]
r=ROOT/'tests/fixtures/glb';out=r/'texture-codec';out.mkdir(exist_ok=True)
def chunk(k,d):return struct.pack('>I',len(d))+k+d+struct.pack('>I',zlib.crc32(k+d)&0xffffffff)
def png(w,h,depth,color,rows,extras=b'',tail=b''):
    return b'\x89PNG\r\n\x1a\n'+chunk(b'IHDR',struct.pack('>IIBBBBB',w,h,depth,color,0,0,0))+extras+chunk(b'IDAT',zlib.compress(b''.join(b'\0'+row for row in rows)))+tail+chunk(b'IEND',b'')
records=[]
def save(name,data,rgba,w,h,depth=8,tolerance=0):
    (out/name).write_bytes(data);ref=name+'.rgba'+('16le' if depth==16 else '');(out/ref).write_bytes(rgba)
    records.append(dict(file=name,reference=ref,width=w,height=h,depth=depth,tolerance=tolerance,sha256=hashlib.sha256(data).hexdigest(),reference_sha256=hashlib.sha256(rgba).hexdigest()))
rgba=[[0,32,255,255],[255,128,64,128],[10,200,240,0],[15,25,35,45],[220,130,40,77],[255,255,255,1]]
rows=[bytes(sum(rgba[i:i+3],[])) for i in [0,3]]
save('rgba8.png',png(3,2,8,6,rows),bytes(sum(rgba,[])),3,2)
exif=b'II*\0\x08\0\0\0\x01\0\x12\x01\x03\0\x01\0\0\0\x06\0\0\0\0\0\0\0'
meta=chunk(b'gAMA',struct.pack('>I',100000))+chunk(b'pHYs',struct.pack('>IIB',5,7,0))+chunk(b'eXIf',exif)+chunk(b'iCCP',b'ignored\0\0'+zlib.compress(b'not an ICC profile'*4096))+chunk(b'iTXt',b'malformed ignored text')
save('ignored-metadata.png',png(3,2,8,6,rows,meta),bytes(sum(rgba,[])),3,2)
animation=chunk(b'acTL',struct.pack('>II',2,0))+chunk(b'fcTL',struct.pack('>IIIIIHHBB',0,3,2,0,0,1,10,0,0))
tail=chunk(b'fcTL',struct.pack('>IIIIIHHBB',1,3,2,0,0,1,10,0,0))+chunk(b'fdAT',struct.pack('>I',2)+zlib.compress(b'\0'+b'\xff'*12+b'\0'+b'\xff'*12))
save('apng-default.png',png(3,2,8,6,rows,animation,tail),bytes(sum(rgba,[])),3,2)
for depth in [8,16]:
    for color,channels in [(0,1),(4,2),(2,3),(6,4)]:
        if depth==8 and color==6:continue
        maximum=(1<<depth)-1;values=[[(i*7919+c*1531+1)%maximum for c in range(channels)] for i in range(6)]
        rowdata=[bytes(sum(values[i:i+3],[])) for i in [0,3]] if depth==8 else [b''.join(struct.pack('>H',v) for px in values[i:i+3] for v in px) for i in [0,3]]
        expected=[]
        for v in values:expected+=(v[:3] if channels>=3 else [v[0]]*3)+([v[-1]] if channels in [2,4] else [maximum])
        raw=bytes(expected) if depth==8 else b''.join(struct.pack('<H',v) for v in expected)
        save(f'color{color}-{depth}.png',png(3,2,depth,color,rowdata),raw,3,2,depth)
palette=bytes([0,20,255,255,0,30,50,150,250,1,2,3]);alpha=bytes([0,100,200,255]);indices=[0,1,2,3,2,1]
expected=bytes(v for index in indices for v in [*palette[index*3:index*3+3],alpha[index]])
save('palette2.png',png(3,2,2,3,[bytes([0b00011000]),bytes([0b11100100])],chunk(b'PLTE',palette)+chunk(b'tRNS',alpha)),expected,3,2)
for name in ['pattern.jpg','pattern.progressive.jpg']:
    source=ROOT/'tests/fixtures/raster'/name;data=source.read_bytes();raw=Path(str(source)+'.rgba').read_bytes();save(name,data,raw,16,16,tolerance=2)
    def app(marker,payload):return b'\xff'+bytes([marker])+struct.pack('>H',len(payload)+2)+payload
    tagged=data[:2]+app(0xe1,b'Exif\0\0'+exif)+app(0xe2,b'ICC_PROFILE\0\1\1'+b'ignored invalid profile')+data[2:]
    save('tagged-'+name,tagged,raw,16,16,tolerance=2)
b=(r/'BoxTextured.glb').read_bytes();data=b[2204:5954]
with tempfile.NamedTemporaryFile(suffix='.png') as f:
    f.write(data);f.flush();pixels=subprocess.run(['ffmpeg','-v','error','-i',f.name,'-frames:v','1','-f','rawvideo','-pix_fmt','rgba','-'],check=True,capture_output=True).stdout
(out/'BoxTextured.rgba').write_bytes(pixels)
(r/'texture-codec-manifest.json').write_text(json.dumps(dict(fixtures=records,original_box=dict(width=256,height=256,bytes=len(pixels),sha256=hashlib.sha256(pixels).hexdigest()),reference='authored PNG samples; existing independent JPEG references; FFmpeg original BoxTextured PNG'),indent=2)+'\n')
(out/'cases.tsv').write_text(''.join(f"{v['file']}\t{v['reference']}\t{v['width']}\t{v['height']}\t{v['depth']}\t{v['tolerance']}\n" for v in records))
print(len(records),'texture codec cases; original RGBA bytes',len(pixels))
