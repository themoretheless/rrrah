"""Expose original static half converters with memcpy wrappers; all16bit patterns."""
import ctypes as C,json,math,pathlib,struct,subprocess
root=pathlib.Path(__file__).resolve().parents[4];src=root/'.dedup-work/libphash454-20261021'
bridge=root/'.dedup-work/libphash454-half-bridge.c'
bridge.write_text('#include <string.h>\n#define STB_IMAGE_RESIZE_IMPLEMENTATION\n#include "stb_image_resize2.h"\nfloat half_decode(unsigned short bits){stbir__FP16 h;memcpy(&h,&bits,2);return stbir__half_to_float(h);}\nunsigned short half_encode(float f){stbir__FP16 h=stbir__float_to_half(f);unsigned short bits;memcpy(&bits,&h,2);return bits;}\n')
rows=[]
for name,flags in [('native',[]),('scalar',['-DSTBIR_NO_SIMD'])]:
 output=root/f'.dedup-work/libphash454-half-{name}.dylib'
 subprocess.run(['cc','-O2','-ffp-contract=off','-dynamiclib',*flags,'-I'+str(src/'vendor'),str(bridge),'-o',str(output)],check=True,capture_output=True)
 lib=C.CDLL(str(output));lib.half_decode.argtypes=[C.c_uint16];lib.half_decode.restype=C.c_float;lib.half_encode.argtypes=[C.c_float];lib.half_encode.restype=C.c_uint16
 nan_cases=0;roundtrips=0
 for bits in range(65536):
  reference=struct.unpack('<e',struct.pack('<H',bits))[0];actual=lib.half_decode(bits)
  if math.isnan(reference):assert math.isnan(actual);nan_cases+=1
  else:
   assert struct.pack('<f',actual)==struct.pack('<f',reference),(name,bits)
   assert lib.half_encode(actual)==bits,(name,bits);roundtrips+=1
 rows.append({'backend':name,'decode_patterns':65536,'nan_class_checks':nan_cases,'exact_non_nan_roundtrips':roundtrips,'mismatches':0})
print(json.dumps({'rows':rows,'method':'Original vendored static helper bodies compiledunchanged, thin memcpy wrappers; Pythonstruct IEEEbinary16 independent reference. Actual ARM/scalar O2 ffp-contractoff.','limits':['Not fullresampling Float16path','NaNpayload/signaling canonicalization notcompared','No arbitraryfloat32 roundingboundary corpus','No speedclaim orASAN']},indent=2))
