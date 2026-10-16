"""Regenerate tiny KTX2 DFD oracles using pinned official Khronos C test code.
Reference code is downloaded/compiled in a temporary directory, never linked to production.
"""
from pathlib import Path
import tempfile, urllib.request, subprocess, hashlib, sys
ROOT=Path(__file__).resolve().parent.parent/'tests/fixtures/raster'
REVISION='060e13229e6d5ff58498270e449334acfd06a5bf'
HASHES={'createdfd.c': '5f625c9ea86e5439342627036255cb6488c95a2bb1943e8c68023983f43a5092', 'vk2dfd.c': '8203bed277d72ce10bbe1f7b70430270624943339bf1bf2022e9ad2b7dd6bdcd', 'vk2dfd.inl': '79bc70554302392bcc30bc04358c2c640932935fc72fac364ebd8789098779b3', 'dfd.h': '869101206a6e1791a0bd9ec8e3813e5c36e32fdf3ae6d08572859d8a7d1572ee', 'KHR/khr_df.h': '11c1f2283b1fe2b76761923571453edce1898127f3385b257f1828b083d69438', 'vulkan/vulkan_core.h': '6e38292ff8f06baad8521fb58e370deff6570c412e28e783506e4eed79af2db9', 'vulkan/vk_platform.h': 'c2aa81456e5d24f5931f9b2052aaeb85d1a6362abac9ee5fb2029a57579e03a7'}
FORMATS=[9, 16, 70, 77, 100, 103, 23, 29, 30, 36, 37, 43, 44, 50, 76, 83, 84, 90, 91, 97, 106, 109]
WRAPPER='#include <stdio.h>\n#include <stdlib.h>\n#include "dfd.h"\nint main(int argc, char **argv) {\n if(argc!=2) return 2;\n uint32_t *d=vk2dfd((enum VkFormat)strtoul(argv[1],0,0));\n if(!d) return 3;\n size_t n=d[0];int failed=fwrite(d,1,n,stdout)!=n;free(d);return failed;\n}\n'
def main():
 if sys.byteorder != "little": raise RuntimeError("reference wrapper requires a little-endian host")
 with tempfile.TemporaryDirectory(prefix='rrrah-dfd-reference-') as temporary:
  root=Path(temporary)
  for name,digest in HASHES.items():
   data=urllib.request.urlopen(f'https://raw.githubusercontent.com/KhronosGroup/dfdutils/{REVISION}/{name}').read()
   if hashlib.sha256(data).hexdigest()!=digest: raise RuntimeError(f'reference hash mismatch: {name}')
   target=root/name;target.parent.mkdir(parents=True,exist_ok=True);target.write_bytes(data)
  (root/'dump.c').write_text(WRAPPER)
  executable=root/'dump'
  subprocess.run(['cc','-std=c99','-O2','-I',str(root),str(root/'dump.c'),str(root/'createdfd.c'),str(root/'vk2dfd.c'),'-o',str(executable)],check=True)
  lines=['# vk\toracle\tbytes\tsha256']
  for vk in FORMATS:
   data=subprocess.check_output([str(executable),str(vk)]);name=f'ktx2-vk-{vk}-khronos.dfd'
   (ROOT/name).write_bytes(data)
   lines.append(f'{vk}\t{name}\t{len(data)}\t{hashlib.sha256(data).hexdigest()}')
  (ROOT/'ktx2-khronos-dfd-manifest.tsv').write_text('\n'.join(lines)+'\n')
if __name__=='__main__':main()
