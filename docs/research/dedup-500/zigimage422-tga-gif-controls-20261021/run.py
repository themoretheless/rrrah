import struct, subprocess, json, hashlib
from pathlib import Path
from PIL import Image
folder=Path(__file__).resolve().parent
root=Path('.dedup-work/zigimage422-20261021'); binary=root/'tga-gif-control'
source=Path('docs/research/dedup-500/zigimage422-png-filter-controls-20261021/control.c')
subprocess.run(['clang','-std=c99','-O2','-I',str(root/'include'),str(source),'-lm','-o',str(binary)],check=True)
rows=[]
def check(name,data,expected,**extra):
    f=root/'tga-gif-fixture';f.write_bytes(data)
    p=subprocess.run([str(binary),str(f)],capture_output=True,check=True)
    assert p.stdout==bytes(expected),(name,list(p.stdout),expected)
    rows.append(dict(name=name,fixture_sha256=hashlib.sha256(data).hexdigest(),decoded_rgba=list(p.stdout),**extra))
    return p.stdout
red=bytes([0,0,255]);blue=bytes([255,0,0]);expected=[255,0,0,255,0,0,255,255]
def tga(w,h,desc,data,rle=False):
    return struct.pack('<BBBHHBHHHHBB',0,0,10 if rle else 2,0,0,0,0,0,w,h,24,desc)+data
check('tga_top_origin',tga(1,2,32,red+blue),expected)
check('tga_bottom_origin',tga(1,2,0,blue+red),expected)
check('tga_rle_literal',tga(1,2,32,bytes([1])+red+blue,True),expected)
check('tga_left_origin',tga(2,1,32,red+blue),expected)
check('tga_right_origin_same_storage',tga(2,1,48,red+blue),expected,observation='Horizontal descriptor bit does not change output; source ignores this bit. No independent external TGA conformance suite.')
outputs=[]
for colour in [(0,255,0),(0,0,255)]:
    first=Image.new('RGB',(2,2),(255,0,0)); second=Image.new('RGB',(2,2),colour)
    f=root/('animation-'+str(colour[1])+'.gif')
    first.save(f,save_all=True,append_images=[second],duration=[100,100],loop=0,optimize=False)
    with Image.open(f) as im:
        assert im.n_frames==2
        im.seek(1); assert im.convert('RGB').tobytes()==bytes(colour)*4
    outputs.append(check('gif_second_frame_'+str(colour),f.read_bytes(),[255,0,0,255]*4,independent_pillow_frames=2,second_frame_rgb=colour))
assert outputs[0]==outputs[1]
(folder/'result.json').write_text(json.dumps(dict(cases=rows,animation_observation='Distinct two-frame animations with identical first frame decode to byte-identical RGBA through original C filename API. This supports first-frame-only limitation; original Zig hashes not executed.',scope='Five authored TGA and two Pillow-generated GIF fixtures; pinned original C backend, no sanitizers or native pHash.'),indent=2)+'\n')
print('7 controls passed; two different animations have byte-identical decoded first frames')
