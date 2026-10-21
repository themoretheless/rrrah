"""Real upstream blur and Laplacian, native ARM versus scalar and independent integer oracle."""
import ctypes as C,json,pathlib,random,subprocess
root=pathlib.Path(__file__).resolve().parents[4];source=root/'.dedup-work/libphash454-20261021';libs=[]
for mode,flags in [('native',[]),('scalar',['-U__ARM_NEON','-U__ARM_NEON__'])]:
    output=root/f'.dedup-work/libphash454-filters-{mode}.dylib'
    subprocess.run(['cc','-O2','-dynamiclib',*flags,'-I'+str(source/'src'),'-I'+str(source/'include'),*[str(source/p) for p in ['src/core.c','src/loader.c','src/image/color.c','src/image/filters.c']],'-o',str(output)],check=True)
    lib=C.CDLL(str(output));lib.ph_create.argtypes=[C.POINTER(C.c_void_p)];lib.ph_free.argtypes=[C.c_void_p]
    lib.ph_apply_gaussian_blur.argtypes=[C.c_void_p,C.POINTER(C.c_uint8),C.c_int,C.c_int,C.POINTER(C.c_uint8)]
    lib.ph_apply_laplacian_3x3.argtypes=[C.POINTER(C.c_uint8),C.c_int,C.c_int,C.POINTER(C.c_uint8)];libs.append(lib)
rng=random.Random(454);cases=0
for w in [1,2,3,15,16,17,18,31,32,33]:
 for h in [1,2,3,4]:
  for repeat in range(2):
   pixels=[rng.randrange(256) for _ in range(w*h)];temp=pixels.copy();blur=pixels.copy();lap=pixels.copy()
   if w>=3 and h>=3:
    for y in range(h):
     for x in range(1,w-1):temp[y*w+x]=(pixels[y*w+x-1]+2*pixels[y*w+x]+pixels[y*w+x+1])//4
    blur=temp.copy()
    for y in range(1,h-1):
     for x in range(w):blur[y*w+x]=(temp[(y-1)*w+x]+2*temp[y*w+x]+temp[(y+1)*w+x])//4
   for y in range(1,h-1):
    for x in range(1,w-1):lap[y*w+x]=min(255,max(0,5*pixels[y*w+x]-pixels[(y-1)*w+x]-pixels[(y+1)*w+x]-pixels[y*w+x-1]-pixels[y*w+x+1]))
   for lib in libs:
    context=C.c_void_p();assert lib.ph_create(C.byref(context))==0
    try:
     original=(C.c_uint8*(w*h))(*pixels);output=(C.c_uint8*(w*h))()
     lib.ph_apply_gaussian_blur(context,original,w,h,output);assert list(output)==blur
     # Gaussian explicitly stages all horizontal input before writing output.
     inplace=(C.c_uint8*(w*h))(*pixels);lib.ph_apply_gaussian_blur(context,inplace,w,h,inplace);assert list(inplace)==blur
     lib.ph_apply_laplacian_3x3(original,w,h,output);assert list(output)==lap
    finally:lib.ph_free(context)
   cases+=1
print(json.dumps({'images':cases,'backend_variants':2,'checks_per_image_per_backend':['Gaussian out-of-place','Gaussian in-place','Laplacian out-of-place'],'comparisons':cases*2*3,'mismatches':0,'method':'Original source compiled unchanged; native ARM and explicitly disabled NEON scalar build, independent integer edge/rounding oracle. No ASAN.','limits':['No allocation-failure injection','No Laplacian in-place contract asserted','No performance comparison']},indent=2))
