"""Compare admitted RLEW raw strips with unmodified LibTIFF's native encoder.

No rrrah decoding is used. Temporary external encodings are removed; compact
results pin retained source manifests rather than adding duplicate TIFF files.
"""
from pathlib import Path
import ctypes as C,ctypes.util,hashlib,json,tempfile
root=Path(__file__).resolve().parents[1]/'tests/fixtures/raster'
lib=C.CDLL(ctypes.util.find_library('tiff'))
lib.TIFFOpen.argtypes=[C.c_char_p,C.c_char_p];lib.TIFFOpen.restype=C.c_void_p
lib.TIFFSetField.argtypes=[C.c_void_p,C.c_uint32];lib.TIFFSetField.restype=C.c_int
lib.TIFFClose.argtypes=[C.c_void_p]
lib.TIFFWriteScanline.argtypes=[C.c_void_p,C.c_void_p,C.c_uint32,C.c_uint16];lib.TIFFWriteScanline.restype=C.c_int
lib.TIFFRawStripSize.argtypes=[C.c_void_p,C.c_uint32];lib.TIFFRawStripSize.restype=C.c_ssize_t
lib.TIFFReadRawStrip.argtypes=[C.c_void_p,C.c_uint32,C.c_void_p,C.c_ssize_t];lib.TIFFReadRawStrip.restype=C.c_ssize_t
lib.TIFFGetVersion.restype=C.c_char_p

def strips(path,count):
 tif=lib.TIFFOpen(str(path).encode(),b'r');assert tif
 result=[]
 try:
  for index in range(count):
   size=lib.TIFFRawStripSize(tif,index);assert size>0
   data=C.create_string_buffer(size);assert lib.TIFFReadRawStrip(tif,index,data,size)==size
   result.append(data.raw)
 finally:lib.TIFFClose(tif)
 return result

results=[]
with tempfile.TemporaryDirectory(prefix='rrrah-native-rlew-') as temporary:
 for manifest in ['fax2w-libtiff-reference.json','fax2w-long-libtiff-reference.json','fax2w-single-libtiff-reference.json']:
  original=(root/manifest).read_bytes();rows=json.loads(original)['rows'];exact=0;different=[];strip_count=0
  for row in rows:
   source=root/row['name'];assert hashlib.sha256(source.read_bytes()).hexdigest()==row['source_sha256']
   long='-long-' in source.name;single='-single-' in source.name
   width,height=(2049,11) if long else (13,7)
   rps=1 if single else (5 if long else 3)
   mode=next(m for m in ['w8l','w8b','wl','wb'] if '-'+m+'-' in source.name)
   fill=int(source.stem.split('-fill')[1].split('-')[0]);orientation=int(source.stem.rsplit('-ori',1)[1])
   target=Path(temporary)/source.name;tif=lib.TIFFOpen(str(target).encode(),mode.encode());assert tif
   try:
    for tag,value in [(256,width),(257,height),(258,1),(259,32771),(262,0),(266,fill),(277,1),(278,rps),(284,1),(274,orientation)]:
     assert lib.TIFFSetField(tif,tag,C.c_int(value))==1
    for y in range(height):
     data=bytearray((width+7)//8)
     for x in range(width):
      black=[False,True,x<width//2,x%513==0,x>width-4][y%5] if long else (x*3+y*5+x*y)%7<3
      if black:data[x//8]|=1<<(7-x%8)
     buf=C.create_string_buffer(bytes(data));assert lib.TIFFWriteScanline(tif,buf,y,0)==1
   finally:lib.TIFFClose(tif)
   count=(height+rps-1)//rps;actual=strips(target,count);expected=strips(source,count);strip_count+=count
   if actual==expected:exact+=1
   else:different.append({'source':source.name,'strip_lengths_native':[len(b) for b in actual],'strip_lengths_assembled':[len(b) for b in expected],'different_strips':[i for i,(a,b) in enumerate(zip(actual,expected)) if a!=b]})
  results.append({'manifest':manifest,'manifest_sha256':hashlib.sha256(original).hexdigest(),'sources':len(rows),'strips':strip_count,'native_encoder_exact':exact,'different':different})
print(json.dumps({'scope':'Unmodified independent LibTIFF native RLEW encoder; every compressed strip byte compared with retained row-assembled controls; authored patterns only, not external scanner originals or an independent full-image decoder. No rrrah output used.','libtiff':lib.TIFFGetVersion().decode(),'results':results},indent=2))
assert all(x['native_encoder_exact']==x['sources'] for x in results)
