"""CC0 WAD3 archives. vgio independently parses directory/mip indices;
Pillow expands an authored palette. This is not a full WAD3 palette oracle.
"""
from pathlib import Path
import struct,hashlib,io
from PIL import Image
from vgio.quake.wad import Header,Entry,Miptexture
r=Path(__file__).resolve().parent.parent/'tests/fixtures/raster';lines=[]
for reverse in [False,True]:
 b=bytearray(12);entries=[];palettes={}
 specs=[('opaque',16,16),('{cutout',32,16)]
 for name,w,h in (list(reversed(specs)) if reverse else specs):
  start=len(b);tex=bytearray(40);tex[:len(name)]=name.encode();struct.pack_into('<II',tex,16,w,h)
  for level in range(4):
   struct.pack_into('<I',tex,24+level*4,len(tex));lw,lh=w>>level,h>>level;tex+=bytes((x+y*lw+level*11)%256 for y in range(lh) for x in range(lw))
  palette=bytes(v for i in range(256) for v in [(i*3)%256,(i*5)%256,(i*7)%256]);palettes[name]=palette
  tex+=struct.pack('<H',256)+palette;tex+=bytes((-len(tex))%4);b+=tex
  entries.append((name,start,len(tex),0x43))
 start=len(b);b+=b'CC0';entries.append(('metadata',start,3,0))
 directory=len(b);entries.sort(key=lambda e: (e[0]!='metadata', e[0].startswith('{')))
 for name,start,length,kind in entries:b+=struct.pack('<III4B16s',start,length,length,kind,0,0,0,name.encode())
 struct.pack_into('<4sII',b,0,b'WAD3',len(entries),directory)
 file=f'wad3-{"reordered" if reverse else "textures"}.wad';(r/file).write_bytes(b)
 stream=io.BytesIO(b);header=Header.read(stream);stream.seek(header.directory_offset)
 parsed=[Entry.read(stream) for _ in range(header.lump_count)];index=0
 for entry in parsed:
  if entry.type!=0x43:continue
  stream.seek(entry.file_offset);mip=Miptexture.read(stream)
  im=Image.frombytes('P',(mip.width,mip.height),bytes(mip.pixels[:mip.width*mip.height]));im.putpalette(palettes[mip.name])
  if mip.name.startswith('{'):im.info['transparency']=255
  oracle=f'{file}-{index}.rgba';(r/oracle).write_bytes(im.convert('RGBA').tobytes())
  lines.append(f'{file}\t{index}\t{mip.width}\t{mip.height}\t{oracle}\t{hashlib.sha256(b).hexdigest()}');index+=1
(r/'wad-manifest.tsv').write_text('\n'.join(lines)+'\n')
