from pathlib import Path
import ctypes as ct,subprocess,json,math,hashlib
C=Path('.dedup-work/audio399-20261021');out=C/'libkiss-review.dylib'
subprocess.run(['cc','-shared','-fPIC',str(C/'kiss_fft.c'),str(C/'kiss_fftr.c'),'-lm','-o',str(out)],check=True)
class Complex(ct.Structure):_fields_=[('r',ct.c_float),('i',ct.c_float)]
lib=ct.CDLL(str(out.resolve()));lib.kiss_fft_alloc.argtypes=[ct.c_int,ct.c_int,ct.c_void_p,ct.c_void_p];lib.kiss_fft_alloc.restype=ct.c_void_p;lib.kiss_fft.argtypes=[ct.c_void_p,ct.POINTER(Complex),ct.POINTER(Complex)];lib.kiss_fftr_alloc.argtypes=lib.kiss_fft_alloc.argtypes;lib.kiss_fftr_alloc.restype=ct.c_void_p;lib.kiss_fftr.argtypes=[ct.c_void_p,ct.POINTER(ct.c_float),ct.POINTER(Complex)];clib=ct.CDLL(None);clib.free.argtypes=[ct.c_void_p]
rows=[]
for n in [2,3,4,5,7,8,9,10,12,16,25,32,64,128]:
 vals=[complex(math.sin(i*.37)+.2*math.cos(i*.13),math.cos(i*.29)*.1) for i in range(n)];inp=(Complex*n)(*[Complex(v.real,v.imag) for v in vals]);dst=(Complex*n)();cfg=lib.kiss_fft_alloc(n,0,None,None);assert cfg;lib.kiss_fft(cfg,inp,dst);clib.free(cfg)
 expected=[sum(v*complex(math.cos(-2*math.pi*k*j/n),math.sin(-2*math.pi*k*j/n)) for j,v in enumerate(vals)) for k in range(n)];err=max(abs(complex(dst[k].r,dst[k].i)-expected[k]) for k in range(n));assert err<1e-4*n;rows.append({'kind':'complex','n':n,'max_absolute_error':err})
 if n%2==0:
  real=[v.real for v in vals];ri=(ct.c_float*n)(*real);ro=(Complex*(n//2+1))();cfg=lib.kiss_fftr_alloc(n,0,None,None);assert cfg;lib.kiss_fftr(cfg,ri,ro);clib.free(cfg);ref=[sum(v*complex(math.cos(-2*math.pi*k*j/n),math.sin(-2*math.pi*k*j/n)) for j,v in enumerate(real)) for k in range(n//2+1)];err=max(abs(complex(ro[k].r,ro[k].i)-ref[k]) for k in range(n//2+1));assert err<1e-4*n;rows.append({'kind':'real','n':n,'max_absolute_error':err})
r={'status':'passed','oracle':'Independent direct complex exponential DFT in Python double precision','cases':rows,'source_sha256':{p:hashlib.sha256((C/p).read_bytes()).hexdigest() for p in ['kiss_fft.c','kiss_fftr.c','_kiss_fft_guts.h','kiss_fft.h','kiss_fftr.h']},'limits':['Original vendored default floating-point KissFFT only; no AudioHash.c execution, native pHash/ASAN or fixed-point/SIMD/OpenMP qualification.']};Path('docs/research/dedup-500/audio399-controls-20261021/fft-result.json').write_text(json.dumps(r,indent=2)+'\n');print('passed',len(rows),'FFT cases')
