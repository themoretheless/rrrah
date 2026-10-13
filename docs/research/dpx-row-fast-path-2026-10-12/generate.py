from pathlib import Path
import struct
root=Path(__file__).resolve().parents[3]
p=root/'tests/fixtures/raster'
h=bytearray((p/'dpx-8-be-50-p0.dpx').read_bytes()[:2048]);w,height,padding=4097,2,7
rows=[];oracle=bytearray()
for y in range(height):
 row=bytearray()
 for x in range(w):
  rgb=bytes([(x+y)%256,(x*17+y*31)%256,(x*43+y*13)%256]);row+=rgb;oracle+=rgb+b'\xff'
 rows.append(bytes(row)+b'\xa5'*padding)
size=2048+sum(map(len,rows))
for at,value in [(4,2048),(16,size),(772,w),(776,height),(808,2048),(812,padding),(816,0)]:struct.pack_into('>I',h,at,value)
(p/'dpx-8-wide-row-boundary.dpx').write_bytes(bytes(h)+b''.join(rows))
(p/'dpx-8-wide-row-boundary.dpx.u8').write_bytes(oracle)
