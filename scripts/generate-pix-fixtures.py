"""Synthetic CC0 Alias PIX, independent FFmpeg pixel oracles."""
from pathlib import Path
import struct,subprocess,hashlib
root=Path(__file__).resolve().parent.parent/'tests/fixtures/raster'
lines=[l for l in (root/'manifest.tsv').read_text().splitlines() if not l.split('\t')[0].endswith('.pix')]
for depth in [8,24]:
 name=f'row-runs-{depth}.pix';header=struct.pack('>5H',3,2,123,456,depth)
 runs=bytes([2,17,1,29,1,43,2,61]) if depth==8 else bytes([2,43,29,17,1,89,73,61,1,255,0,0,2,0,0,255])
 data=header+runs;(root/name).write_bytes(data)
 subprocess.run(['ffmpeg','-v','error','-c:v','alias_pix','-i',str(root/name),'-frames:v','1','-f','rawvideo','-pix_fmt','rgba','-y',str(root/(name+'.rgba'))],check=True)
 lines.append(f'{name}\t3\t2\t0\t{name}.rgba\t{hashlib.sha256(data).hexdigest()}')
(root/'manifest.tsv').write_text('\n'.join(lines)+'\n')
