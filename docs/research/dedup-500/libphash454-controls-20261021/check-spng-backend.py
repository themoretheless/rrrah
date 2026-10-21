import ctypes,pathlib,json,struct,zlib
p=pathlib.Path(__file__).resolve().parents[4]/'.dedup-work/spng454-20261021';lib=ctypes.CDLL(str((p/'decoder.dylib').resolve()));f=lib.ph_decode_png_mem;f.argtypes=[ctypes.c_void_p,ctypes.c_ulong]+[ctypes.POINTER(ctypes.c_int)]*3+[ctypes.c_int];f.restype=ctypes.c_void_p
free=ctypes.CDLL(None).free;free.argtypes=[ctypes.c_void_p]
def chunk(t,b):return struct.pack('>I',len(b))+t+b+struct.pack('>I',zlib.crc32(t+b))
rows=[]
for color,pixel in [(0,b'\x80'),(2,b'\xff\0\0'),(4,b'\x80\xff'),(6,b'\xff\0\0\xff')]:
 png=b'\x89PNG\r\n\x1a\n'+chunk(b'IHDR',struct.pack('>IIBBBBB',1,1,8,color,0,0,0))+chunk(b'IDAT',zlib.compress(b'\0'+pixel))+chunk(b'IEND',b'')
 for req in [1,3]:
  buf=ctypes.create_string_buffer(png);w,h,c=ctypes.c_int(),ctypes.c_int(),ctypes.c_int();out=f(buf,len(png),ctypes.byref(w),ctypes.byref(h),ctypes.byref(c),req)
  values=list(ctypes.string_at(out,w.value*h.value*c.value)) if out else None
  rows.append({'png_color_type':color,'requested_channels':req,'success':bool(out),'pixels':values})
  if out:free(out)
print(json.dumps({'method':'Actual unmodified libphash PNG spng wrapper linked to pinned original spng.c, AppleClang O2, valid synthetic 1x1 PNGs, direct backend calls without stb fallback.','rows':rows,'limits':['Eight direct-backend cases only; no full spng source review, native full build, performance or malformed-input qualification.']},indent=2))
