import pathlib,subprocess,ctypes,json
root=pathlib.Path(__file__).resolve().parents[4];here=pathlib.Path(__file__).resolve().parent
out=root/'.dedup-work/spng454-20261021/sizes.dylib'
subprocess.run(['cc','-O2','-shared','-fPIC','-DSPNG_STATIC','-I'+str(root/'.dedup-work/spng454-20261021/spng'),str(here/'spng-size-control.c'),'-lz','-lm','-o',str(out)],check=True)
f=ctypes.CDLL(str(out)).observed_sizes;f.argtypes=[ctypes.c_uint]*4+[ctypes.POINTER(ctypes.c_size_t)]*2
count=0
for color,channels,depths in [(0,1,[1,2,4,8,16]),(2,3,[8,16]),(3,1,[1,2,4,8]),(4,2,[8,16]),(6,4,[8,16])]:
 for depth in depths:
  for width in [1,2,3,7,8,9,17,31,33]:
   for height in [1,2,3,17]:
    scan,size=ctypes.c_size_t(),ctypes.c_size_t()
    assert f(color,depth,width,height,ctypes.byref(scan),ctypes.byref(size))==0
    assert scan.value==1+(width*channels*depth+7)//8
    assert size.value==width*height*3
    count+=1
print(json.dumps({'actual_original_static_helper_cases':count,'mismatches':0,'reference':'Independent integer ceil(bits/8)+filter byte and width*height*3 output bytes.','limits':['Only valid small IHDR combinations, no pixels decoded.','Does not establish allocation limits, overflow-boundary behavior or malformed-input coverage.']},indent=2))
