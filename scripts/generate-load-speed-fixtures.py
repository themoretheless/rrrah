"""CC0 synthetic inputs for warm decode/display preparation comparisons."""
from pathlib import Path
import struct,zlib,importlib.util,sys
root=Path(sys.argv[1] if len(sys.argv)>1 else 'target/bench/raster-load/fixtures');root.mkdir(parents=True,exist_ok=True)
def chunk(k,d):return struct.pack('>I',len(d))+k+d+struct.pack('>I',zlib.crc32(k+d))
w=h=256;count=24
b=b'\x89PNG\r\n\x1a\n'+chunk(b'IHDR',struct.pack('>2I5B',w,h,8,6,0,0,0))+chunk(b'sRGB',b'\0')+chunk(b'acTL',struct.pack('>II',count,0));seq=0
for i in range(count):
 b+=chunk(b'fcTL',struct.pack('>5I2H2B',seq,w,h,0,0,1,60,0,0));seq+=1
 row=bytes([(i*17)%256,127,211,255])*w;data=zlib.compress((b'\0'+row)*h)
 if i==0:b+=chunk(b'IDAT',data)
 else:b+=chunk(b'fdAT',struct.pack('>I',seq)+data);seq+=1
b+=chunk(b'IEND',b'');(root/'frames.apng').write_bytes(b)
path=Path(__file__).with_name('generate-ktx2-fixtures.py');spec=importlib.util.spec_from_file_location('ktx',path);module=importlib.util.module_from_spec(spec);spec.loader.exec_module(module)
w=h=1024;data=bytes([127,83,211,255])*(w*h);mips=[]
for i in range(1,11):mips.append(bytes([127+i,83,211,255])*((w>>i)*(h>>i)))
(root/'levels.ktx2').write_bytes(module.make(43,4,1,w,h,data,compression=3,mips=mips))
b=b'\x89PNG\r\n\x1a\n'+chunk(b'IHDR',struct.pack('>2I5B',w,h,8,6,0,0,0))+chunk(b'sRGB',b'\0')+chunk(b'IDAT',zlib.compress((b'\0'+bytes([127,83,211,255])*w)*h))+chunk(b'IEND',b'')
(root/'still.png').write_bytes(b)
floats=struct.pack('<4f',2.,-.5,.1,.25)*(w*h)
(root/'linear.ktx2').write_bytes(module.make(109,4,4,w,h,floats,compression=3))
# Twenty-four independent full-canvas Aseprite cels: selected decode work stays
# one frame, and the first full RGBA cel can inflate directly into the canvas.
def ase_chunk(k,d):return struct.pack('<IH',len(d)+6,k)+d
ase_layer=ase_chunk(0x2004,struct.pack('<6HB',1,0,0,0,0,0,255)+bytes(3)+struct.pack('<H',5)+b'Layer')
ase_profile=ase_chunk(0x2007,struct.pack('<HHI',1,0,0)+bytes(8))
frames=[]
for i in range(24):
 pixels=bytes([(i*17)%256,127,211,255])*(w*h)
 cel=ase_chunk(0x2005,struct.pack('<HhhBHh',0,0,0,255,2,0)+bytes(5)+struct.pack('<HH',w,h)+zlib.compress(pixels))
 chunks=[ase_layer,ase_profile,cel] if i==0 else [cel]
 body=b''.join(chunks);frames.append(struct.pack('<IHHH2xI',len(body)+16,0xf1fa,len(chunks),17,0)+body)
header=bytearray(128);body=b''.join(frames)
struct.pack_into('<IHHHHHIH',header,0,len(body)+128,0xa5e0,24,w,h,32,1,17);header[34:36]=b'\x01\x01'
(root/'frames.aseprite').write_bytes(header+body)
