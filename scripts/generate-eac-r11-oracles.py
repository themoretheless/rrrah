"""CC0 synthetic EAC blocks; pinned Ericsson decoder as temporary independent11-bit oracle.
Production must not link this C++ reference. Original source is not copied into repository.
"""
from pathlib import Path
import ctypes
import hashlib
import random
import struct
import subprocess
import tempfile
import urllib.request
import sys
import zlib
FULL = "--full-bases" in sys.argv[1:]
ROOT=Path(__file__).resolve().parent.parent/'tests/fixtures/raster'
URL='https://raw.githubusercontent.com/Ericsson/ETCPACK/master/source/etcdec.cxx'
SHA='54f11eabcbd9d9fe6508e9b5bccac621fb6d43d7f1fab9db53dfa54748cfbb55'
with tempfile.TemporaryDirectory() as temporary:
    directory=Path(temporary)
    cpp=urllib.request.urlopen(URL).read()
    assert hashlib.sha256(cpp).hexdigest()==SHA
    (directory/'etcdec.cxx').write_bytes(cpp)
    (directory/'wrapper.cpp').write_text('''extern int formatSigned;
void setupAlphaTable();
void decompressBlockAlpha16bitC(unsigned char*,unsigned char*,int,int,int,int,int);
extern "C" void oracle(unsigned char* source,unsigned char* output,int sign) {
 setupAlphaTable();formatSigned=sign;
 decompressBlockAlpha16bitC(source,output,4,4,0,0,1);
}
''')
    subprocess.run(['clang++','-std=c++11','-O2','-shared','-fPIC',str(directory/'etcdec.cxx'),str(directory/'wrapper.cpp'),'-o',str(directory/'oracle.dylib')],check=True)
    lib=ctypes.CDLL(str(directory/'oracle.dylib'))
    lib.oracle.argtypes=[ctypes.c_char_p,ctypes.POINTER(ctypes.c_ubyte),ctypes.c_int]
    rng=random.Random(0x45414331)
    blocks=bytearray(); outputs=[bytearray(),bytearray()]
    bases=list(range(256)) if FULL else [0,1,2,7,15,31,63,64,65,127,128,129,192,253,254,255]
    for base in bases:
        for table in range(16):
            for multiplier in range(16):
                selectors=list(range(8))*2;rng.shuffle(selectors)
                indices=0
                for selector in selectors: indices=(indices<<3)|selector
                block=bytes([base,(multiplier<<4)|table])+indices.to_bytes(6,'big')
                blocks.extend(block)
                for signed in [0,1]:
                    out=(ctypes.c_ubyte*32)();lib.oracle(block,out,signed); raw=bytes(out)
                    # PGMOUT reference converts11-bit values to16-bit big-endian;
                    # reverse the exact extension and signed32768 offset.
                    for i in range(0,32,2):
                        value=int.from_bytes(raw[i:i+2],'big')-(32768 if signed else 0)
                        value=-((-value)>>5) if value<0 else value>>5
                        outputs[signed].extend(struct.pack('<h',value))
    for name,data in [('eac-r11-block-oracles.bin',blocks),('eac-r11-block-oracles.u11le',outputs[0]),('eac-r11-block-oracles.i11le',outputs[1])]:
        if FULL:
            name=name.replace("eac-r11-block-", "eac-r11-full-")+".zlib"
            data=zlib.compress(data,9)
        (ROOT/name).write_bytes(data)
        print(name,len(data),hashlib.sha256(data).hexdigest())
