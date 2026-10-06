"""CC0 hand-authored DDS blocks; RGBA oracles decoded by Pillow."""
from pathlib import Path
import struct,hashlib,subprocess,tempfile
from PIL import Image
root=Path(__file__).resolve().parent.parent/'tests/fixtures/raster'
def dds(fourcc,block):
    h=[124,0x81007,4,4,len(block),0,1]+[0]*11
    h += [32,4,int.from_bytes(fourcc,'little'),0,0,0,0,0,0x1000,0,0,0,0]
    return b'DDS '+struct.pack('<31I',*h)+block
color=struct.pack('<HHI',0xf800,0x07e0,0xe4e4e4e4)
fixtures={
 'bc1.dds':dds(b'DXT1',color),
 'bc1-alpha.dds':dds(b'DXT1',struct.pack('<HHI',0,0xffff,0xe4e4e4e4)),
 'bc2.dds':dds(b'DXT3',bytes([0x10,0x32,0x54,0x76,0x98,0xba,0xdc,0xfe])+color),
 'bc3.dds':dds(b'DXT5',bytes([255,0])+int('0123456701234567',8).to_bytes(6,'little')+color),
}
def raw_dds(bits,masks,payload,pitch):
    h=[124,0x100f,3,3,pitch,0,1]+[0]*11
    h += [32,0x41 if masks[3] else 0x40,0,bits,*masks,0x1000,0,0,0,0]
    return b'DDS '+struct.pack('<31I',*h)+payload
fixtures.update({
 'rgb565.dds':raw_dds(16,[0xf800,0x7e0,0x1f,0],b''.join(struct.pack('<3H',0xf800,0x07e0,0x001f)+b'\0\0' for _ in range(3)),8),
 'rgb24.dds':raw_dds(24,[0xff0000,0xff00,0xff,0],bytes([0,0,255,0,255,0,255,0,0])*3,9),
 'bgra32.dds':raw_dds(32,[0xff0000,0xff00,0xff,0xff000000],bytes([0,0,255,0,0,255,0,128,255,0,0,255])*3,12),
})
dx10_names=[]
for fmt in [28,29,87,88,91,93]:
 base=bytearray(fixtures['bgra32.dds'][:128]);base[80:84]=struct.pack('<I',4);base[84:88]=b'DX10'
 payload=fixtures['bgra32.dds'][128:]
 if fmt in (28,29): payload=bytes(v for i in range(0,len(payload),4) for v in (payload[i+2],payload[i+1],payload[i],payload[i+3]))
 name=f'dx10-{fmt}.dds';dx10_names.append(name)
 fixtures[name]=bytes(base)+struct.pack('<5I',fmt,3,0,1,1)+payload
lines=[l for l in (root/'manifest.tsv').read_text().splitlines() if not l.split('\t')[0].endswith('.dds')]
for name,data in fixtures.items():
 path=root/name;path.write_bytes(data)
 if name in ('dx10-28.dds','dx10-29.dds'):
  with Image.open(path) as image: oracle=image.convert('RGBA').tobytes()
  width,height=3,3
 elif name in ('rgb565.dds','rgb24.dds','bgra32.dds') or name in dx10_names:
  reference=data
  if name in dx10_names:
   fmt=int(name[5:-4]);masks=[0xff0000,0xff00,0xff,0 if fmt in (88,93) else 0xff000000]
   reference=raw_dds(32,masks,data[148:],12)
  if name=='rgb565.dds':
   # FFmpeg ignores pitch: decode an equivalent tightly packed reference.
   reference=raw_dds(16,[0xf800,0x7e0,0x1f,0],b''.join(struct.pack('<3H',0xf800,0x07e0,0x001f) for _ in range(3)),6)
  with tempfile.NamedTemporaryFile(suffix='.dds') as temporary:
   temporary.write(reference);temporary.flush()
   oracle=subprocess.check_output(['ffmpeg','-v','error','-i',temporary.name,'-f','rawvideo','-pix_fmt','rgba','pipe:1'])
  width,height=3,3
 else:
  with Image.open(path) as image: oracle=image.convert('RGBA').tobytes()
  width,height=4,4
 if name in dx10_names:
  fmt=int(name[5:-4]);expected=bytes([255,0,0,255 if fmt in (88,93) else 0,0,255,0,255 if fmt in (88,93) else 128,0,0,255,255])*3
  assert oracle==expected
 (root/(name+'.rgba')).write_bytes(oracle)
 lines.append(f'{name}\t{width}\t{height}\t1\t{name}.rgba\t{hashlib.sha256(data).hexdigest()}')
(root/'manifest.tsv').write_text('\n'.join(lines)+'\n')
