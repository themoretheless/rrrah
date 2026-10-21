import ctypes,pathlib,json
root=pathlib.Path(__file__).resolve().parents[4]
lib=ctypes.CDLL(str(root/'.dedup-work/spng454-20261021/decoder.dylib'))
lib.spng_ctx_new.argtypes=[ctypes.c_int];lib.spng_ctx_new.restype=ctypes.c_void_p
lib.spng_ctx_free.argtypes=[ctypes.c_void_p]
lib.spng_get_option.argtypes=[ctypes.c_void_p,ctypes.c_int,ctypes.POINTER(ctypes.c_int)]
lib.spng_set_option.argtypes=[ctypes.c_void_p,ctypes.c_int,ctypes.c_int]
rows=[]
for requested in [1,100,1000,2000,0]:
 ctx=lib.spng_ctx_new(0);assert ctx
 before,after=ctypes.c_int(),ctypes.c_int()
 assert lib.spng_get_option(ctx,11,ctypes.byref(before))==0
 result=lib.spng_set_option(ctx,11,requested)
 assert lib.spng_get_option(ctx,11,ctypes.byref(after))==0
 assert before.value==1000
 assert (result,after.value)==((0,0) if requested==0 else (1,1000))
 rows.append(dict(requested=requested,before=before.value,set_return=result,after=after.value))
 lib.spng_ctx_free(ctx)
print(json.dumps({'rows':rows,'finding':'Fresh contexts reject every tested positive chunk-count limit, including unchanged default1000; zero succeeds. Actual original public API, no image input.','limits':['Five option-only calls; no decoding/resource consumption effect measured.','Existing cached build from v23 pinned spng.c, AppleClang O2.']},indent=2))
