import struct, subprocess, json, hashlib
from pathlib import Path
folder=Path(__file__).resolve().parent
root=Path('.dedup-work/zigimage422-20261021')
binary=root/'bmp-control'
source=Path('docs/research/dedup-500/zigimage422-png-filter-controls-20261021/control.c')
subprocess.run(['clang','-std=c99','-O2','-I',str(root/'include'),str(source),'-lm','-o',str(binary)],check=True)
def bmp(width,height,bpp,pixels):
    return b'BM'+struct.pack('<IHHI',54+len(pixels),0,0,54)+struct.pack('<IiiHHIIiiII',40,width,height,1,bpp,0,len(pixels),0,0,0,0)+pixels
cases=[]
def check(name,data,expected):
    path=root/'bmp-control.bmp';path.write_bytes(data)
    p=subprocess.run([str(binary),str(path)],capture_output=True,check=True)
    assert p.stdout==bytes(expected),(name,list(p.stdout),expected)
    cases.append(dict(name=name,rgba=list(p.stdout),fixture_sha256=hashlib.sha256(data).hexdigest()))
# Independent top-to-bottom oracle: red row, then blue row; each 24-bit row padded to four bytes.
red=bytes([0,0,255,0]);blue=bytes([255,0,0,0])
check('24bit_bottom_up',bmp(1,2,24,blue+red),[255,0,0,255,0,0,255,255])
check('24bit_top_down',bmp(1,-2,24,red+blue),[255,0,0,255,0,0,255,255])
check('32bit_all_zero_alpha_becomes_opaque',bmp(2,1,32,bytes([0,0,255,0,255,0,0,0])),[255,0,0,255,0,0,255,255])
check('32bit_mixed_alpha_retains_zero',bmp(2,1,32,bytes([0,0,255,0,255,0,0,128])),[255,0,0,0,0,0,255,128])
(folder/'result.json').write_text(json.dumps(dict(cases=cases,scope='Actual pinned original C filename decoder, desired RGBA4. Four hand-authored BMP known-pixel controls; not Zig runtime, masks, indexed BMP, or full format qualification.'),indent=2)+'\n')
print(json.dumps(cases,indent=2))
