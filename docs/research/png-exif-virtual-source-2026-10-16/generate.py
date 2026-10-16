from pathlib import Path
import struct,zlib
base=Path("tests/fixtures/raster/pattern.png").read_bytes()
body=bytes([73,73,42,0,8,0,0,0,1,0,18,1,3,0,1,0,0,0,6,0,0,0,0,0,0,0])+bytes(4*1024*1024)
kind=b"eXIf"
chunk=struct.pack(">I",len(body))+kind+body+struct.pack(">I",zlib.crc32(kind+body))
Path("/tmp/rrrah-png-large-exif-20261016.png").write_bytes(base[:33]+chunk+base[33:])
