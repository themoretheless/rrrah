"""Read-only LibTIFF RLEW decoder audit; explicit library isolates diagnostics."""
from pathlib import Path
import argparse,ctypes as C,ctypes.util,hashlib,json
parser=argparse.ArgumentParser();parser.add_argument('--library');parser.add_argument('--first',type=int);args=parser.parse_args()
root=Path(__file__).resolve().parents[1]/'tests/fixtures/raster'
lib=C.CDLL(args.library or ctypes.util.find_library('tiff'))
lib.TIFFOpen.argtypes=[C.c_char_p,C.c_char_p];lib.TIFFOpen.restype=C.c_void_p
lib.TIFFClose.argtypes=[C.c_void_p]
lib.TIFFGetField.argtypes=[C.c_void_p,C.c_uint32];lib.TIFFGetField.restype=C.c_int
lib.TIFFReadEncodedStrip.argtypes=[C.c_void_p,C.c_uint32,C.c_void_p,C.c_ssize_t];lib.TIFFReadEncodedStrip.restype=C.c_ssize_t
lib.TIFFGetVersion.restype=C.c_char_p
results=[];checked=0
for manifest in ['fax2w-libtiff-reference.json','fax2w-long-libtiff-reference.json','fax2w-single-libtiff-reference.json']:
 rows=json.loads((root/manifest).read_text())['rows'];exact=0;different=[]
 for row in rows:
  if args.first is not None and checked>=args.first:break
  checked+=1
  source=root/row['name'];assert hashlib.sha256(source.read_bytes()).hexdigest()==row['source_sha256']
  tif=lib.TIFFOpen(str(source).encode(),b'r');assert tif
  try:
   values=[]
   for tag in [256,257,278]:
    value=C.c_uint32();assert lib.TIFFGetField(tif,tag,C.byref(value))==1;values.append(value.value)
   width,height,rps=values;orientation=C.c_uint16();assert lib.TIFFGetField(tif,274,C.byref(orientation))==1
   data=bytearray();stride=(width+7)//8
   for index in range((height+rps-1)//rps):
    size=min(rps,height-index*rps)*stride;buf=C.create_string_buffer(size)
    assert lib.TIFFReadEncodedStrip(tif,index,buf,size)==size;data.extend(buf.raw)
  finally:lib.TIFFClose(tif)
  expected=(root/row['rgba8_file']).read_bytes();assert hashlib.sha256(expected).hexdigest()==row['rgba8_sha256']
  actual=bytearray(len(expected));o=orientation.value;ow,oh=(width,height) if o<=4 else (height,width)
  assert (ow,oh)==(row['width'],row['height'])
  for y in range(height):
   for x in range(width):
    dx,dy={1:(x,y),2:(width-1-x,y),3:(width-1-x,height-1-y),4:(x,height-1-y),5:(y,x),6:(height-1-y,x),7:(height-1-y,width-1-x),8:(y,width-1-x)}[o]
    value=0 if (data[y*stride+x//8]>>(7-x%8))&1 else 255
    at=(dy*ow+dx)*4;actual[at:at+4]=bytes([value,value,value,255])
  if actual==expected:exact+=1
  else:different.append({'source':source.name,'different_bytes':sum(a!=b for a,b in zip(actual,expected))})
 results.append({'manifest':manifest,'manifest_sha256':hashlib.sha256((root/manifest).read_bytes()).hexdigest(),'checked':exact+len(different),'exact':exact,'different':different})
 if args.first is not None and checked>=args.first:break
print(json.dumps({'scope':'Explicit test-only LibTIFF decoder audit; no rrrah output used. A modified reference library is a diagnostic control, not an unmodified independent oracle.','library':args.library or ctypes.util.find_library('tiff'),'version':lib.TIFFGetVersion().decode(),'results':results},indent=2))
