"""Negative pitch compared to explicit row reversal, actual original easy API."""
import ctypes as C,json,pathlib,random,subprocess
root=pathlib.Path(__file__).resolve().parents[4];source=root/'.dedup-work/libphash454-20261021';rng=random.Random(454);checks=0
sizes=[(1,1),(1,17),(17,1),(2,3),(17,19),(33,32)]
for mode,flags in [('native',[]),('scalar',['-DSTBIR_NO_SIMD'])]:
 path=root/f'.dedup-work/libphash454-strides-{mode}.dylib'
 subprocess.run(['cc','-O2','-ffp-contract=off','-dynamiclib',*flags,'-I'+str(source/'src'),'-I'+str(source/'include'),str(source/'src/image/resize.c'),'-o',str(path)],check=True,capture_output=True)
 lib=C.CDLL(str(path));fn=lib.stbir_resize_uint8_linear;fn.restype=C.c_void_p
 fn.argtypes=[C.c_void_p,C.c_int,C.c_int,C.c_int,C.c_void_p,C.c_int,C.c_int,C.c_int,C.c_int]
 for w,h in sizes:
  data=bytes(rng.randrange(256) for _ in range(w*h));buf=C.create_string_buffer(data)
  reverse=b''.join(data[y*w:(y+1)*w] for y in range(h-1,-1,-1));revbuf=C.create_string_buffer(reverse)
  for dw,dh in sizes:
   def resize(inp,pitch,output_pitch):
    output=(C.c_uint8*(dw*dh))();pointer=C.addressof(output)+(dw*(dh-1) if output_pitch<0 else 0)
    assert fn(inp,w,h,pitch,pointer,dw,dh,output_pitch,1)
    return bytes(output)
   expected=resize(C.addressof(revbuf),w,dw)
   negative=resize(C.addressof(buf)+w*(h-1),-w,dw);assert negative==expected;checks+=1
   normal=resize(C.addressof(buf),w,dw)
   output_negative=resize(C.addressof(buf),w,-dw)
   assert output_negative==b''.join(normal[y*dw:(y+1)*dw] for y in range(dh-1,-1,-1));checks+=1
   both=resize(C.addressof(buf)+w*(h-1),-w,-dw)
   assert both==b''.join(expected[y*dw:(y+1)*dw] for y in range(dh-1,-1,-1));checks+=1
print(json.dumps({'checks':checks,'mismatches':0,'sizes':sizes,'backends':['nativeARM','STBIR_NO_SIMD'],'method':'Actual original stbir_resize_uint8_linear, valid allocated buffers, row reversal independently constructs reference. RebuiltO2ffp-contractoff libraries.','limits':['Grayuint8only, noalpha/sRGB','Backendpatterns independentlyseeded, notcrossbackendbyteparity claim','No speed or memorysanitizer qualification']},indent=2))
