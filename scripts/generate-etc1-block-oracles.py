"""CC0 synthetic ETC1 blocks, independently decoded by pinned AOSP ETC1.
The Apache-2.0 reference decoder is temporary and never a runtime dependency.
"""
from pathlib import Path
import base64
import ctypes
import hashlib
import random
import subprocess
import tempfile
import urllib.request

ROOT = Path(__file__).resolve().parent.parent / 'tests/fixtures/raster'
BASE = 'https://android.googlesource.com/platform/frameworks/native/+/refs/heads/main/'

def source(path):
    return base64.b64decode(urllib.request.urlopen(BASE + path + '?format=TEXT').read())

with tempfile.TemporaryDirectory() as temporary:
    directory = Path(temporary)
    (directory / 'ETC1').mkdir()
    cpp = source('opengl/libs/ETC1/etc1.cpp')
    assert hashlib.sha256(cpp).hexdigest() == '5e6e1f9d5515f9a4ee58ef3e40ec71ed23995ae1410c72db3ccab6e25ae1931b'
    (directory / 'etc1.cpp').write_bytes(cpp)
    (directory / 'ETC1/etc1.h').write_bytes(source('opengl/include/ETC1/etc1.h'))
    subprocess.run(['clang++', '-std=c++11', '-O2', '-shared', '-fPIC', '-I', str(directory),
                    str(directory / 'etc1.cpp'), '-o', str(directory / 'oracle.so')], check=True)
    lib = ctypes.CDLL(str(directory / 'oracle.so'))
    lib.etc1_decode_image.argtypes = [ctypes.c_char_p, ctypes.c_void_p, ctypes.c_uint,
                                    ctypes.c_uint, ctypes.c_uint, ctypes.c_uint]
    rng = random.Random(0x45544331)
    blocks = bytearray()
    outputs = bytearray()
    for case in range(4096):
        differential = (case >> 1) & 1
        flip = case & 1
        table0 = (case >> 2) & 7
        table1 = (case >> 5) & 7
        colors = []
        for channel in range(3):
            if differential:
                delta = ((case >> (channel + 3)) & 7) - 4
                low, high = max(0, -delta), min(31, 31 - delta)
                base = [low, high, rng.randint(low, high)][(case >> 8) % 3]
                colors.append((base << 3) | (delta & 7))
            else:
                colors.append(rng.randrange(256))
        selectors = [0, 0xffffffff, 0xaaaaaaaa, 0x55555555, rng.getrandbits(32)][case % 5]
        block = bytes(colors + [(table0 << 5) | (table1 << 2) | (differential << 1) | flip])
        block += selectors.to_bytes(4, 'big')
        output = (ctypes.c_ubyte * 48)()
        assert lib.etc1_decode_image(block, output, 4, 4, 3, 12) == 0
        blocks.extend(block)
        outputs.extend(bytes(output))
    (ROOT / 'etc1-block-oracles.bin').write_bytes(blocks)
    (ROOT / 'etc1-block-oracles.rgb').write_bytes(outputs)
    for name in ['etc1-block-oracles.bin', 'etc1-block-oracles.rgb']:
        print(name, hashlib.sha256((ROOT / name).read_bytes()).hexdigest())
