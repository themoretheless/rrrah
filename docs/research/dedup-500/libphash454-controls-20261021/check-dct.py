"""Compile original helper bodies; remove only unused public hash wrapper."""
import ctypes as C,hashlib,json,math,pathlib,random,subprocess
root=pathlib.Path(__file__).resolve().parents[4];source=root/'.dedup-work/libphash454-20261021'
text=(source/'src/hashes/phash.c').read_text();start=text.index('PH_API ph_error_t ph_compute_phash(');opening=text.index('{',start);depth=1;end=opening+1
while depth:
    depth+=(text[end]=='{')-(text[end]=='}');end+=1
helper=root/'.dedup-work/libphash454-dct-helpers.c';helper.write_text(text[:start]+text[end:])
libpath=root/'.dedup-work/libphash454-dct-helpers.dylib'
subprocess.run(['cc','-O2','-dynamiclib','-I'+str(source/'src'),'-I'+str(source/'include'),str(helper),'-o',str(libpath)],check=True)
lib=C.CDLL(str(libpath));F=C.c_float;U=C.c_uint8
lib.ph_get_dct_matrix_32.restype=C.POINTER(F);matrix=lib.ph_get_dct_matrix_32()
lib.ph_dct2_partial.argtypes=[C.POINTER(F),C.POINTER(U),C.c_int,C.c_int,C.POINTER(F)]
lib.ph_median_bitpack.argtypes=[C.POINTER(F),C.c_int];lib.ph_median_bitpack.restype=C.c_uint64
maximum=0.;rng=random.Random(454)
for case in range(32):
    pixels=[rng.randrange(256) for _ in range(1024)]
    if case==0:pixels=[0]*1024
    if case==1:pixels=[255]*1024
    actual=(F*64)();lib.ph_dct2_partial(matrix,(U*1024)(*pixels),32,8,actual)
    # Independently generated double-precision orthonormal DCT-II basis.
    basis=[[math.sqrt((1 if i==0 else 2)/32)*math.cos(math.pi*(j+.5)*i/32) for j in range(32)] for i in range(8)]
    rows=[[sum(basis[j][k]*pixels[y*32+k] for k in range(32)) for j in range(8)] for y in range(32)]
    for i in range(8):
        for j in range(8):
            reference=sum(basis[i][y]*rows[y][j] for y in range(32));error=abs(actual[i*8+j]-reference);maximum=max(maximum,error);assert error<.05
median_cases=0
for n in range(1,65):
    for _ in range(32):
        values=[rng.randrange(-16,17) for _ in range(n)];ordered=sorted(values)
        median=(ordered[(n-1)//2]+ordered[n//2])/2
        expected=sum(1<<i for i,x in enumerate(values) if x>median)
        assert lib.ph_median_bitpack((F*n)(*values),n)==expected;median_cases+=1
print(json.dumps({'dct32_images':32,'coefficients_checked':2048,'maximum_absolute_error':maximum,'tolerance':.05,'median_cases':median_cases,'mismatches':0,'source_sha256':hashlib.sha256(text.encode()).hexdigest(),'method':'Original helper bodies unchanged; unused ph_compute_phash wrapper omitted to avoid unrelated decoder/resizer linkage. Apple ARM64 NEON path, no ASAN.','limits':['Not full public image-hash runtime or upstream suite','No other DCT sizes or concurrency qualification','No timing claim']},indent=2))
