from pathlib import Path
import struct,zlib,runpy
root=Path(__file__).resolve().parents[3]
base=runpy.run_path(str(root/'scripts/generate-gray-icc-fixtures.py'))
chunk=base['chunk'];out=Path(__file__).resolve().parent
for name in ['gamma2','lut17']:
 icc=bytearray(base['profile']())
 if name=='lut17':
  # Replace the final kTRC payload with a sampled gamma-2 table.
  offset=struct.unpack_from('>I',icc,148)[0]
  curve=b'curv'+bytes(4)+struct.pack('>I',17)+struct.pack('>17H',*[round((i/16)**2*65535) for i in range(17)])
  icc=icc[:offset]+curve
  icc+=bytes((-len(icc))%4)
  struct.pack_into('>I',icc,152,len(curve));struct.pack_into('>I',icc,0,len(icc))
 (out/f'{name}.icc').write_bytes(icc)
 row=b'\0'+b''.join(struct.pack('>HH',i,65535-i) for i in range(65536))
 png=b'\x89PNG\r\n\x1a\n'+chunk(b'IHDR',struct.pack('>IIBBBBB',65536,1,16,4,0,0,0))
 png+=chunk(b'iCCP',b'Gray ramp\0\0'+zlib.compress(icc))+chunk(b'IDAT',zlib.compress(row))+chunk(b'IEND',b'')
 (out/f'{name}.png').write_bytes(png)
