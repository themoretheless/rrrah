import pathlib,subprocess,ctypes,json,random
root=pathlib.Path(__file__).resolve().parents[4];here=pathlib.Path(__file__).resolve().parent
rng=random.Random(454);cases=[]
for bpp in [1,2,3,4,6,8]:
 for width in [1,2,3,7,8,9,17,31,33]:
  for pattern in [0,1]:
   n=bpp*width;raw=bytes(rng.randrange(256) for _ in range(n)) if pattern else bytes([255]*n)
   prev=bytes(rng.randrange(256) for _ in range(n)) if pattern else bytes(n)
   for filt in range(5):
    encoded=[]
    for i,value in enumerate(raw):
     a=raw[i-bpp] if i>=bpp else 0;b=prev[i];c=prev[i-bpp] if i>=bpp else 0
     p=a+b-c;dist=[abs(p-a),abs(p-b),abs(p-c)];paeth=[a,b,c][dist.index(min(dist))]
     predictor=[0,a,b,(a+b)//2,paeth][filt];encoded.append((value-predictor)%256)
    cases.append((bpp,filt,prev,bytes(encoded),raw))
rows=[]
for backend,defs in [('arm64',[]),('scalar',['-DSPNG_DISABLE_OPT'])]:
 out=root/f'.dedup-work/spng454-20261021/filter-{backend}.dylib'
 subprocess.run(['cc','-O2','-shared','-fPIC','-DSPNG_STATIC']+defs+['-I'+str(root/'.dedup-work/spng454-20261021/spng'),str(here/'spng-filter-control.c'),'-lz','-lm','-o',str(out)],check=True)
 f=ctypes.CDLL(str(out)).observed_defilter;f.argtypes=[ctypes.c_void_p]*3+[ctypes.c_size_t,ctypes.c_uint,ctypes.c_uint]
 for bpp,filt,prev,enc,raw in cases:
  result=ctypes.create_string_buffer(len(raw));a=ctypes.create_string_buffer(prev);b=ctypes.create_string_buffer(enc)
  assert f(a,b,result,len(raw),bpp,filt)==0
  assert result.raw==raw,(backend,bpp,filt,len(raw))
 rows.append({'backend':backend,'cases':len(cases),'mismatches':0})
print(json.dumps({'rows':rows,'method':'Independent Python PNG filter encoder including Paeth tie order, original static defilter called through padded observer. Same inputs both backends.','limits':['Valid small rows only; observer adds 64 bytes of padding, so not a buffer boundary safety test.','Not full PNG parser, all SIMD targets or performance qualification.']},indent=2))
