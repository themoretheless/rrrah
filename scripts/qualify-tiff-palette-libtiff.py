"""Independent LibTIFF scanline/ColorMap oracle for authored TIFF controls.
LibTIFF is test-only and is never linked into the production decoder.
"""
from pathlib import Path
import ctypes as C
import ctypes.util
import hashlib,json,struct
root=Path(__file__).resolve().parents[1]/'tests/fixtures/raster'
name=ctypes.util.find_library('tiff')
if not name:raise RuntimeError('independent libtiff unavailable')
lib=C.CDLL(name)
lib.TIFFOpen.argtypes=[C.c_char_p,C.c_char_p];lib.TIFFOpen.restype=C.c_void_p
lib.TIFFClose.argtypes=[C.c_void_p]
lib.TIFFGetField.argtypes=[C.c_void_p,C.c_uint32];lib.TIFFGetField.restype=C.c_int
lib.TIFFScanlineSize64.argtypes=[C.c_void_p];lib.TIFFScanlineSize64.restype=C.c_uint64
lib.TIFFReadScanline.argtypes=[C.c_void_p,C.c_void_p,C.c_uint32,C.c_uint16];lib.TIFFReadScanline.restype=C.c_int
lib.TIFFGetVersion.restype=C.c_char_p
rows=[]
files=sorted(set(root.glob('palette16-*.tiff'))|set(root.glob('palette-packed*.tiff'))|set(root.glob('palette-wide16-*.tiff')))
for path in files:
 tif=lib.TIFFOpen(str(path).encode(),b'r')
 if not tif:raise RuntimeError('LibTIFF open failed: '+path.name)
 try:
  def field(tag,kind):
   value=kind();assert lib.TIFFGetField(tif,tag,C.byref(value))==1;return value.value
  width,height,bits=field(256,C.c_uint32),field(257,C.c_uint32),field(258,C.c_uint16)
  assert bits in [1,2,4,8,16]
  count=1<<bits
  R= C.POINTER(C.c_uint16)();G=C.POINTER(C.c_uint16)();B=C.POINTER(C.c_uint16)()
  assert lib.TIFFGetField(tif,320,C.byref(R),C.byref(G),C.byref(B))==1
  palette=bytearray()
  for channel,p in enumerate([R,G,B]):
   for i in range(count):
    if bits==16:expected=[(i*251)%65536,65535-i,(i*73+19)%65536][channel]
    elif bits==8:expected=[i*251,65535-i*251,(i*i*31)%65536][channel]
    else:expected=[i*3911,65535-i*3911,(i*i*997)%65536][channel]
    assert p[i]==expected,(path.name,i,channel,p[i],expected)
    palette+=struct.pack('<H',p[i])
  size=lib.TIFFScanlineSize64(tif);assert 0<size<4096
  buffer=C.create_string_buffer(size)
  indices=[]
  for y in range(height):
   assert lib.TIFFReadScanline(tif,buffer,y,0)==1
   if bits==16:row=list(C.cast(buffer,C.POINTER(C.c_uint16))[:width])
   else:
    data=buffer.raw;per=8//bits;mask=(1<<bits)-1
    row=[(data[x//per]>>(8-bits*(x%per+1)))&mask for x in range(width)]
   indices+=row
  expected=([0,1,255,256,4095,32767,32768,40000,65534,65535] if bits==16 else
            [0,1,127,255,17,31,64,128] if bits==8 else
            [(x+y*3)%count for y in range(3) for x in range(5)])
  assert indices==expected,path.name
  rows.append({'name':path.name,'source_sha256':hashlib.sha256(path.read_bytes()).hexdigest(),
               'width':width,'height':height,'bits':bits,'palette_entries':count,
               'palette16_sha256':hashlib.sha256(palette).hexdigest(),'every_palette_entry_checked':True,
               'indices':indices})
 finally:lib.TIFFClose(tif)
output={'oracle':lib.TIFFGetVersion().decode().splitlines()[0],
        'library_sha256':hashlib.sha256(Path(name).read_bytes()).hexdigest(),
        'scope':'Independent LibTIFF every scanline index and every native16 ColorMap entry; no color rendering',
        'rows':rows}
(root/'palette-libtiff-reference.json').write_text(json.dumps(output,indent=2)+'\n')
print(json.dumps({'files':len(rows),'version':output['oracle'],'all_indices_and_palette_entries_exact':True}))
