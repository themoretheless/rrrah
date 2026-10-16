from pathlib import Path
import struct,zlib
base=Path("tests/fixtures/raster/pattern.png").read_bytes()
body=b"Comment\0"+b"x"*(4*1024*1024)
kind=b"tEXt"
chunk=struct.pack(">I",len(body))+kind+body+struct.pack(">I",zlib.crc32(kind+body))
Path("/tmp/rrrah-png-large-text-20261016.png").write_bytes(base[:33]+chunk+base[33:])
