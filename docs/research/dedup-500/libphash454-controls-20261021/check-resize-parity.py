"""Original resize wrappers and vendored resizer: ARM/scalar byte parity, no speed claim."""
import ctypes as C,json,pathlib,random,subprocess
root=pathlib.Path(__file__).resolve().parents[4];source=root/'.dedup-work/libphash454-20261021';libs=[]
for name,flags in [('native',[]),('scalar',['-DSTBIR_NO_SIMD'])]:
 output=root/f'.dedup-work/libphash454-resize-{name}.dylib'
 subprocess.run(['cc','-O2','-ffp-contract=off','-dynamiclib',*flags,'-I'+str(source/'src'),'-I'+str(source/'include'),str(source/'src/image/resize.c'),'-o',str(output)],check=True,capture_output=True)
 lib=C.CDLL(str(output))
 for fn in ['ph_resize_box','ph_resize_lanczos']:
  getattr(lib,fn).argtypes=[C.POINTER(C.c_uint8),C.c_int,C.c_int,C.POINTER(C.c_uint8),C.c_int,C.c_int]
 libs.append(lib)
dimensions=[(1,1),(1,17),(17,1),(2,2),(7,9),(16,16),(17,19),(32,33),(63,65)]
rng=random.Random(454);comparisons=0;identity_cases=0
for sw,sh in dimensions:
 for pattern in ['black','white','random']:
  values=[0 if pattern=='black' else 255 if pattern=='white' else rng.randrange(256) for _ in range(sw*sh)]
  original=(C.c_uint8*(sw*sh))(*values)
  for dw,dh in dimensions:
   for function in ['ph_resize_box','ph_resize_lanczos']:
    outputs=[]
    for lib in libs:
     target=(C.c_uint8*(dw*dh))();getattr(lib,function)(original,sw,sh,target,dw,dh);outputs.append(bytes(target))
    assert outputs[0]==outputs[1],(sw,sh,dw,dh,pattern,function)
    if sw==dw and sh==dh:
     assert outputs[0]==bytes(values);identity_cases+=1
    if pattern in ['black','white']:assert outputs[0]==bytes([0 if pattern=='black' else 255])*(dw*dh)
    comparisons+=1
print(json.dumps({'backend_comparisons':comparisons,'identity_cases':identity_cases,'mismatches':0,'dimensions':dimensions,'patterns':['black','white','random'],'method':'Original resize.c and exact vendored header, nativeARM versus STBIR_NO_SIMD, O2 ffp-contract=off. Byte parity plus identity/constant invariants.','limits':['Gray uint8only, positive packedstrides','Not independent general-resampling oracle','No alpha/sRGB/float/multithread/allocfailure qualification','No timingclaim or ASAN']},indent=2))
