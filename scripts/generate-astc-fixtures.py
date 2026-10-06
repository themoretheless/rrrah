"""CC0 ASTC textures, Arm astcenc encoder/reference decoder oracle.
Pass a built astcenc CLI path as the single argument (version 4.6.1 used).
"""
from pathlib import Path
import hashlib, subprocess, sys, tempfile
from PIL import Image

codec = sys.argv[1]
root = Path(__file__).resolve().parent.parent/'tests/fixtures/raster'
lines = [line for line in (root/'manifest.tsv').read_text().splitlines()
         if not line.split('\t')[0].endswith('.astc')]
footprints = [(4,4),(5,4),(5,5),(6,5),(6,6),(8,5),(8,6),(8,8),
              (10,5),(10,6),(10,8),(10,10),(12,10),(12,12)]
with tempfile.TemporaryDirectory() as directory:
    directory = Path(directory)
    source = Image.new('RGBA',(23,17))
    source.putdata([((x*13+y*7)%256,(x*19+y*11)%256,(x*23+y*17)%256,
                     [0,64,128,255][(x//3+y//4)%4])
                    for y in range(17) for x in range(23)])
    source.save(directory/'input.png')
    for bx,by in footprints:
        name = f'alpha-{bx}x{by}.astc'
        subprocess.run([codec,'-cs',str(directory/'input.png'),str(root/name),
                        f'{bx}x{by}','-fast','-j','1','-silent'],check=True)
        subprocess.run([codec,'-ds',str(root/name),str(directory/'output.png'),
                        '-j','1','-silent'],check=True)
        oracle = Image.open(directory/'output.png').convert('RGBA')
        assert oracle.size == source.size
        (root/(name+'.rgba')).write_bytes(oracle.tobytes())
        lines.append(f'{name}\t23\t17\t0\t{name}.rgba\t'
                     + hashlib.sha256((root/name).read_bytes()).hexdigest())
    # A real compressed HDR surface, independent OpenEXR float extraction.
    # Values above 1 force HDR endpoint coding; alpha is authored opaque.
    source = bytearray(b'#?RADIANCE\nFORMAT=32-bit_rle_rgbe\n\n-Y 17 +X 23\n')
    for y in range(17):
        for x in range(23):
            source.extend([(x*7+y*3)%128+128, (x*5+y*11)%128+64,
                           (x*13+y*7)%128+32, 131])
    (directory/'input.hdr').write_bytes(source)
    subprocess.run([codec,'-ch',str(directory/'input.hdr'),str(root/'hdr-compressed.astc'),
                    '6x6','-medium','-j','1','-silent'],check=True)
    subprocess.run([codec,'-dh',str(root/'hdr-compressed.astc'),str(directory/'output.exr'),
                    '-j','1','-silent'],check=True)
    import OpenEXR
    import numpy as np
    oracle = OpenEXR.File(str(directory/'output.exr')).channels()['RGBA'].pixels
    assert oracle.shape == (17,23,4)
    assert np.isfinite(oracle).all() and (oracle[..., :3] > 1.0).any()
    (root/'hdr-compressed.astc.rgba32f').write_bytes(oracle.astype('<f4').tobytes())
(root/'manifest.tsv').write_text('\n'.join(lines)+'\n')
