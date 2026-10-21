"""Compare original public loaders with/without spng; observer reads first stored byte."""
import ctypes,pathlib,json,struct,zlib
root=pathlib.Path(__file__).resolve().parents[4];p=root/'.dedup-work/spng454-20261021'
def chunk(t,b):return struct.pack('>I',len(b))+t+b+struct.pack('>I',zlib.crc32(t+b))
rows=[]
for backend in ['fallback','spng']:
 lib=ctypes.CDLL(str(p/f'public-{backend}.dylib'))
 lib.ph_create.argtypes=[ctypes.POINTER(ctypes.c_void_p)];lib.ph_free.argtypes=[ctypes.c_void_p]
 lib.ph_context_set_load_grayscale.argtypes=[ctypes.c_void_p,ctypes.c_int]
 lib.ph_load_from_memory.argtypes=[ctypes.c_void_p,ctypes.c_void_p,ctypes.c_size_t]
 lib.ph_load_from_file.argtypes=[ctypes.c_void_p,ctypes.c_char_p]
 lib.ph_context_get_dimensions.argtypes=[ctypes.c_void_p]+[ctypes.POINTER(ctypes.c_int)]*3
 lib.observed_first.argtypes=[ctypes.c_void_p]
 for color,pixel in [(0,b'\x80'),(2,b'\xff\0\0'),(4,b'\x80\xff'),(6,b'\xff\0\0\xff')]:
  png=b'\x89PNG\r\n\x1a\n'+chunk(b'IHDR',struct.pack('>IIBBBBB',1,1,8,color,0,0,0))+chunk(b'IDAT',zlib.compress(b'\0'+pixel))+chunk(b'IEND',b'')
  file=p/f'valid-{color}.png';file.write_bytes(png)
  for gray in [0,1]:
   for mode in ['memory','file']:
    ctx=ctypes.c_void_p();assert lib.ph_create(ctypes.byref(ctx))==0
    lib.ph_context_set_load_grayscale(ctx,gray);buf=ctypes.create_string_buffer(png)
    rc=lib.ph_load_from_memory(ctx,buf,len(png)) if mode=='memory' else lib.ph_load_from_file(ctx,str(file).encode())
    assert rc==0,(backend,color,gray,mode,rc)
    w,h,c=ctypes.c_int(),ctypes.c_int(),ctypes.c_int();lib.ph_context_get_dimensions(ctx,ctypes.byref(w),ctypes.byref(h),ctypes.byref(c))
    first=lib.observed_first(ctx);assert (w.value,h.value)==(1,1)
    rows.append(dict(backend=backend,color_type=color,gray=gray,mode=mode,channels=c.value,first_byte=first))
    lib.ph_free(ctx)
for row in rows:
 if row['gray']:
  assert row['channels']==1
  assert row['first_byte']==(128 if row['color_type'] in [0,4] else 76)
print(json.dumps({'public_loads_succeeded':len(rows),'rows':rows,'limits':['Valid synthetic 1x1 inputs only, opaque alpha; first stored byte and dimensions observed, not all hash outputs.','Original sources compiled O2 arm64; no PH_TESTING, ASAN or external libpng/WebP/TurboJPEG.','Native-channel mode may differ across backend recipes; not asserted equivalent.']},indent=2))
