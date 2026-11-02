"""16-bit TIFF index controls, with full 65536-entry16-bit color planes."""
from pathlib import Path
import struct
root=Path(__file__).resolve().parents[1]/'tests/fixtures/raster'
indices=[0,1,255,256,4095,32767,32768,40000,65534,65535]
for big in [False,True]:
 for endian,magic,suffix in [('<',b'II','le'),('>',b'MM','be')]:
  pack=lambda fmt,*v:struct.pack(endian+fmt,*v)
  at=(16+8+10*20+8) if big else (8+2+10*12+4)
  tags=[(256,4,1,5),(257,4,1,2),(258,3,1,16),(259,3,1,1),(262,3,1,3),
        (273,4,1,at+65536*6),(277,3,1,1),(278,4,1,2),(279,4,1,20),(320,3,65536*3,at)]
  data=bytearray((magic+pack('HHHQ',43,8,0,16)+pack('Q',10)) if big else (magic+pack('HIH',42,8,10)))
  for tag,kind,count,value in tags:
   data+=pack('HHQ' if big else 'HHI',tag,kind,count)
   if count==1:
    field=pack('H' if kind==3 else 'I',value);data+=field+bytes((8 if big else 4)-len(field))
   else:data+=pack('Q' if big else 'I',value)
  data+=pack('Q' if big else 'I',0)
  for c in range(3):
   for i in range(65536):data+=pack('H',[(i*251)%65536,65535-i,(i*73+19)%65536][c])
  data+=pack('10H',*indices)
  (root/f'palette-wide16-{"big-" if big else ""}{suffix}.tiff').write_bytes(data)
