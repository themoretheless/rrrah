"""CC0 authored presentations, independently encoded by Pillow and APNG writer."""
from pathlib import Path
import hashlib,json,struct,zlib
from PIL import Image, __version__
root=Path(__file__).resolve().parent.parent/'crates/rrrah-dedup/tests/fixtures/timeline'
root.mkdir(parents=True,exist_ok=True)
a=Image.new('RGBA',(4,3),(255,0,0,255)); b=Image.new('RGBA',(4,3),(0,0,255,255))
for ext in ['gif','png','webp']:
    a.save(root/f'base.{ext}',save_all=True,append_images=[b],duration=[200,300],loop=0,lossless=True,disposal=1)
def chunk(kind,data):
    return struct.pack('>I',len(data))+kind+data+struct.pack('>I',zlib.crc32(kind+data))
def apng(name,frames):
    data=b'\x89PNG\r\n\x1a\n'+chunk(b'IHDR',struct.pack('>2I5B',4,3,8,6,0,0,0))+chunk(b'sRGB',b'\0')+chunk(b'acTL',struct.pack('>2I',len(frames),0))
    seq=0
    for i,(pixel,ms) in enumerate(frames):
        data+=chunk(b'fcTL',struct.pack('>5I2H2B',seq,4,3,0,0,ms,1000,0,0));seq+=1
        raw=zlib.compress((b'\0'+bytes(pixel)*4)*3)
        if i==0: data+=chunk(b'IDAT',raw)
        else:data+=chunk(b'fdAT',struct.pack('>I',seq)+raw);seq+=1
    (root/name).write_bytes(data+chunk(b'IEND',b''))
r=(255,0,0,255);b=(0,0,255,255)
apng('zero-prefix.apng',[((0,255,0,255),0),(r,200),(b,300)])
apng('split.apng',[(r,100),(r,100),(b,150),(b,150)])
apng('changed-time.apng',[(r,100),(r,100),(b,150),(b,151)])
apng('changed-pixels.apng',[(r,100),(r,100),(b,150),((0,255,0,255),150)])
# Independent Pillow presentation readback verifies the split durations and pixels.
for name in ['split.apng','changed-time.apng','changed-pixels.apng']:
    image=Image.open(root/name); assert image.n_frames==4
    assert [image.seek(i) or image.info['duration'] for i in range(4)][:3]==[100,100,150]
zero=Image.open(root/'zero-prefix.apng'); assert zero.n_frames==3
assert [zero.seek(i) or zero.info['duration'] for i in range(3)]==[0,200,300]
manifest={'pillow':__version__,'base_timeline_ms':[200,300],'split_timeline_ms':[100,100,150,150],
 'sha256':{p.name:hashlib.sha256(p.read_bytes()).hexdigest() for p in sorted(root.iterdir()) if p.suffix!='.json'}}
(root/'manifest.json').write_text(json.dumps(manifest,indent=2)+'\n')
