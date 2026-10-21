"""Actual unchanged upstream common.c; valid digest sizes only, no ASAN."""
import ctypes as C, hashlib, json, pathlib, random, subprocess
root=pathlib.Path(__file__).resolve().parents[4]
source=root/'.dedup-work/libphash454-20261021'
output=root/'.dedup-work/libphash454-common.dylib'
subprocess.run(['cc','-O2','-dynamiclib','-I'+str(source/'include'),'-I'+str(source/'src'),str(source/'src/hashes/common.c'),'-o',str(output)],check=True)
lib=C.CDLL(str(output))
class Digest(C.Structure):
    _fields_=[('data',C.c_uint8*64),('size',C.c_uint8),('reserved',C.c_uint8*7)]
for name,restype in [('ph_hamming_distance_digest',C.c_int),('ph_l2_distance',C.c_double)]:
    fn=getattr(lib,name);fn.argtypes=[C.POINTER(Digest)]*2;fn.restype=restype
rng=random.Random(454);cases=0
for size in range(65):
    for _ in range(128):
        a,b=Digest(),Digest();a.size=b.size=size
        for i in range(64):a.data[i]=rng.randrange(256);b.data[i]=rng.randrange(256)
        expected=sum((a.data[i]^b.data[i]).bit_count() for i in range(size))
        assert lib.ph_hamming_distance_digest(C.byref(a),C.byref(b))==expected
        squared=sum((a.data[i]-b.data[i])**2 for i in range(size))
        assert abs(lib.ph_l2_distance(C.byref(a),C.byref(b))**2-squared)<1e-7
        cases+=1
assert lib.ph_hamming_distance_digest(None,None)==-1
assert lib.ph_l2_distance(None,None)==-1
a,b=Digest(),Digest();b.size=1
assert lib.ph_hamming_distance_digest(C.byref(a),C.byref(b))==-1
assert lib.ph_l2_distance(C.byref(a),C.byref(b))==-1
print(json.dumps({'valid_digest_cases':cases,'sizes':[0,64],'hamming_and_l2_mismatches':0,'null_and_size_mismatch_guards':'passed','backend':'native arm64 NEON for digest Hamming; scalar L2','source_sha256':hashlib.sha256((source/'src/hashes/common.c').read_bytes()).hexdigest(),'limits':['No x86/AVX2 execution','No oversized digest or sanitizer execution','Not upstream full suite or hash/decode qualification']},indent=2))
