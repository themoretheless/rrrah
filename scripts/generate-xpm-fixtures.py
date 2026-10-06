"""CC0 XPM3 fixture with multi-byte keys; independent FFmpeg oracle."""
from pathlib import Path
import subprocess
import hashlib
root=Path(__file__).resolve().parent.parent/'tests/fixtures/raster'
data=b'''/* XPM */
static char *test[] = {
"3 2 4 2",
"aa c #ff0000",
"bb c #00ff00",
"cc c #0000ff",
".. c None",
"aabbcc",
"..ccaa"
};
'''
name='palette.xpm';path=root/name;path.write_bytes(data)
oracle=subprocess.check_output(['ffmpeg','-v','error','-i',str(path),'-f','rawvideo','-pix_fmt','rgba','pipe:1'])
assert oracle==bytes([255,0,0,255,0,255,0,255,0,0,255,255,0,0,0,0,0,0,255,255,255,0,0,255])
(root/(name+'.rgba')).write_bytes(oracle)
lines=[l for l in (root/'manifest.tsv').read_text().splitlines() if not l.startswith(name+'\t')]
lines.append(f'{name}\t3\t2\t0\t{name}.rgba\t{hashlib.sha256(data).hexdigest()}')
(root/'manifest.tsv').write_text('\n'.join(lines)+'\n')
