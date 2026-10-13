from pathlib import Path
import sys
width,height=4000,3000
row=bytes(i%256 for i in range(width*3))
out=bytearray()
for x in range(width): out += row[x*3:x*3+3]+b"\xff"
Path(sys.argv[1]+".u8").write_bytes(bytes(out)*height)
