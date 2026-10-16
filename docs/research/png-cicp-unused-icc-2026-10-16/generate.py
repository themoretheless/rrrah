from pathlib import Path
import struct,zlib
base=Path("tests/fixtures/raster/pattern.png").read_bytes()
profiled=Path("tests/fixtures/raster/pattern.profiled.png").read_bytes()
offset=8
while offset<len(profiled):
 size=struct.unpack(">I",profiled[offset:offset+4])[0]
 kind=profiled[offset+4:offset+8];data=profiled[offset+8:offset+8+size]
 if kind==b"iCCP":
  zero=data.index(0);profile=bytearray(zlib.decompress(data[zero+2:]));break
 offset+=12+size
profile.extend(bytes(4*1024*1024-len(profile)));profile[:4]=struct.pack(">I",len(profile))
def chunk(kind,body):return struct.pack(">I",len(body))+kind+body+struct.pack(">I",zlib.crc32(kind+body))
iccp=chunk(b"iCCP",b"Profile\0\0"+zlib.compress(profile))
cicp=chunk(b"cICP",bytes([1,8,0,1]))
Path("/tmp/rrrah-png-cicp-large-icc-20261016.png").write_bytes(base[:33]+iccp+cicp+base[33:])
