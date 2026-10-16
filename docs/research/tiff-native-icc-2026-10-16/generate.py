from pathlib import Path
import struct
original=Path("tests/fixtures/raster/oriented.profiled.tif").read_bytes()
endian="<" if original[:2]==b"II" else ">"
assert struct.unpack_from(endian+"H",original,2)[0]==42
directory=struct.unpack_from(endian+"I",original,4)[0]
count=struct.unpack_from(endian+"H",original,directory)[0]
for index in range(count):
 entry=directory+2+index*12
 if struct.unpack_from(endian+"H",original,entry)[0]==34675:
  length,offset=struct.unpack_from(endian+"II",original,entry+4);profile=original[offset:offset+length];break
for length in [65536,4*1024*1024]:
 data=bytearray(original);offset=len(data)
 padded=bytearray(profile)+bytes(length-len(profile));padded[:4]=struct.pack(">I",length)
 struct.pack_into(endian+"II",data,entry+4,length,offset);data.extend(padded)
 Path(f"/tmp/rrrah-tiff-icc-{length}-20261016.tif").write_bytes(data)
