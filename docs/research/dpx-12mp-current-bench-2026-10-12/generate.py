from pathlib import Path
import struct,sys
root=Path(__file__).resolve().parents[3]
header=bytearray((root/'tests/fixtures/raster/dpx-8-be-50-p0.dpx').read_bytes()[:2048])
width,height=4000,3000
for at,value in [(4,2048),(16,2048+width*height*3),(772,width),(776,height),(808,2048),(812,0),(816,0)]:
 struct.pack_into('>I',header,at,value)
row=bytes(index%256 for index in range(width*3))
Path(sys.argv[1]).write_bytes(bytes(header)+row*height)
