"""CC0 HEVC still-image fixtures encoded by libheif/x265.

Lossless RGB input and Pillow geometry/alpha are the pixel oracle; heif-convert
provides a secondary interoperability check, not an independent codec oracle.
"""
from pathlib import Path
import hashlib
import subprocess
import tempfile
from PIL import Image

root = Path(__file__).resolve().parent.parent / 'tests/fixtures/raster'
lines = [line for line in (root / 'manifest.tsv').read_text().splitlines()
         if not line.split('\t')[0].endswith(('.heic', '.heif'))]
with tempfile.TemporaryDirectory() as directory:
    directory = Path(directory)
    image = Image.new('RGBA', (64, 48))
    image.putdata([((x*3+y*5)%256, (x*7+y*11)%256, (x*13+y*17)%256,
                    [255,128,0,64][(x//16+y//12)%4])
                   for y in range(48) for x in range(64)])
    cases = [('alpha.heic', image, [], image),
             ('rotate90.heic', image, ['--rotate-cw','90'], image.transpose(Image.Transpose.ROTATE_270)),
             ('mirror.heif', image, ['--flip-h'], image.transpose(Image.Transpose.FLIP_LEFT_RIGHT))]
    profiled = Image.open(root/'pattern.profiled.png').convert('RGB')
    cases.append(('profiled.heic', profiled, [], profiled))
    for name, source, transforms, expected in cases:
        options = {}
        if name == 'profiled.heic':
            options['icc_profile'] = Image.open(root/'pattern.profiled.png').info['icc_profile']
        source.save(directory/'input.png', **options)
        subprocess.run(['heif-enc','-L','--hevc','-p','x265:pools=1',
                        '--colour_primaries','1','--transfer_characteristic','13',
                        *transforms,'-o',str(root/name),str(directory/'input.png')], check=True,
                       stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
        expected_bytes = expected.convert('RGBA').tobytes()
        subprocess.run(['heif-convert','--quiet',str(root/name),str(directory/'output.png')],check=True)
        actual = Image.open(directory/'output.png').convert('RGBA')
        assert actual.size == expected.size
        assert actual.tobytes() == expected_bytes, name
        (root/(name+'.rgba')).write_bytes(expected_bytes)
        lines.append(f'{name}\t{expected.width}\t{expected.height}\t0\t{name}.rgba\t'
                     + hashlib.sha256((root/name).read_bytes()).hexdigest())
    # Adjacent 12-bit values represented in a 16-bit PNG; never add this to
    # the RGBA8 corpus, which cannot express its precision contract.
    source = Image.frombytes('I;16', (64,48), b''.join(
        value.to_bytes(2,'little') for y in range(48) for x in range(64)
        for value in [32768 if x % 2 == 0 else 32784]))
    source.save(directory/'precision.png')
    subprocess.run(['heif-enc','-L','--hevc','-b','12','-p','x265:pools=1',
                    '--colour_primaries','1','--transfer_characteristic','13',
                    '-o',str(root/'precision12.heic'),str(directory/'precision.png')],
                   check=True,stdout=subprocess.DEVNULL,stderr=subprocess.DEVNULL)
(root/'manifest.tsv').write_text('\n'.join(lines)+'\n')
