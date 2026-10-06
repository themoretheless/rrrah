"""CC0 synthetic ILBM/PBM, independent FFmpeg RGBA oracles."""
from pathlib import Path
import struct,subprocess,hashlib
root=Path(__file__).resolve().parent.parent/'tests/fixtures/raster'
lines=[l for l in (root/'manifest.tsv').read_text().splitlines() if not l.split('\t')[0].endswith('.iff')]
def chunk(name,data):return name+struct.pack('>I',len(data))+data+bytes(len(data)%2)
for form in [b'ILBM',b'PBM ']:
 for compression in [0,1]:
  for masking in [0,2]:
   planar=form==b'ILBM';planes=2 if planar else 8
   header=struct.pack('>HHhhBBBBHBBhh',3,2,0,0,planes,masking,compression,0,2,1,1,3,2)
   cmap=bytes([255,0,0,0,255,0,0,0,255,17,29,43])
   indices=[[0,1,2],[3,2,1]];rows=[]
   for indices_row in indices:
    if planar:
     for p in range(planes): rows.append(bytes([sum(((v>>p)&1)<<(7-x) for x,v in enumerate(indices_row)),0]))
    else:rows.append(bytes(indices_row+[0]))
   body=b''.join((bytes([len(row)-1])+row if compression else row) for row in rows)
   payload=form+chunk(b'BMHD',header)+chunk(b'CMAP',cmap)+chunk(b'BODY',body)
   data=b'FORM'+struct.pack('>I',len(payload))+payload
   name=f'{form.decode().strip().lower()}-{compression}-{masking}.iff';(root/name).write_bytes(data)
   subprocess.run(['ffmpeg','-v','error','-i',str(root/name),'-frames:v','1','-f','rawvideo','-pix_fmt','rgba','-y',str(root/(name+'.rgba'))],check=True)
   lines.append(f'{name}\t3\t2\t0\t{name}.rgba\t{hashlib.sha256(data).hexdigest()}')
for label,planes,masking,camg in [('mask-plane',2,1,0),('truecolor',24,0,0),('ehb',6,0,128)]:
 values=[[0,1,2],[3,2,1]] if planes==2 else [[0x2b1d11,0x59493d,0xff0000],[0x0000ff,0x00ff00,0xe6f0fa]] if planes==24 else [[0,31,32],[33,62,63]]
 header=struct.pack('>HHhhBBBBHBBhh',3,2,0,0,planes,masking,1,0,0,1,1,3,2)
 cmap=bytes([255,0,0,0,255,0,0,0,255,17,29,43]) if planes==2 else bytes(v for i in range(32) for v in (i*8,255-i*4,i*3)) if planes==6 else b''
 rows=[]
 for row_values in values:
  for p in range(planes):rows.append(bytes([sum(((v>>p)&1)<<(7-x) for x,v in enumerate(row_values)),0]))
  if masking==1:rows.append(bytes([0xa0,0]))
 body=b''.join(bytes([1])+row for row in rows)
 payload=b'ILBM'+chunk(b'BMHD',header)+(chunk(b'CMAP',cmap) if cmap else b'')+(chunk(b'CAMG',struct.pack('>I',camg)) if camg else b'')+chunk(b'BODY',body)
 data=b'FORM'+struct.pack('>I',len(payload))+payload;name=f'ilbm-{label}.iff';(root/name).write_bytes(data)
 subprocess.run(['ffmpeg','-v','error','-i',str(root/name),'-frames:v','1','-f','rawvideo','-pix_fmt','rgba','-y',str(root/(name+'.rgba'))],check=True)
 lines.append(f'{name}\t3\t2\t0\t{name}.rgba\t{hashlib.sha256(data).hexdigest()}')
for masking in [0,1,2]:
 values=[[2,17,34,51,1],[18,35,52,3,16]];w,h=5,2;planes=6
 header=struct.pack('>HHhhBBBBHBBhh',w,h,0,0,planes,masking,1,0,2,1,1,w,h)
 cmap=bytes(v for i in range(16) for v in (i*17,255-i*17,(i*51)%256))
 rows=[]
 for row_values in values:
  for p in range(planes):rows.append(bytes([sum(((v>>p)&1)<<(7-x) for x,v in enumerate(row_values)),0]))
  if masking==1:rows.append(bytes([0xa8,0]))
 body=b''.join(bytes([1])+row for row in rows)
 payload=b'ILBM'+chunk(b'BMHD',header)+chunk(b'CMAP',cmap)+chunk(b'CAMG',struct.pack('>I',2048))+chunk(b'BODY',body)
 data=b'FORM'+struct.pack('>I',len(payload))+payload;name=f'ilbm-ham6-{masking}.iff';(root/name).write_bytes(data)
 subprocess.run(['ffmpeg','-v','error','-i',str(root/name),'-frames:v','1','-f','rawvideo','-pix_fmt','rgba','-y',str(root/(name+'.rgba'))],check=True)
 # FFmpeg loses mask-plane alpha in HAM; retain its RGB oracle and apply
 # alpha directly from the authored BMHD/mask bits, never from Rrrah.
 if masking:
  rgba=bytearray((root/(name+'.rgba')).read_bytes())
  for y,row_values in enumerate(values):
   for x,value in enumerate(row_values):rgba[(y*w+x)*4+3]=255 if (x%2==0 if masking==1 else value!=2) else 0
  (root/(name+'.rgba')).write_bytes(rgba)
 lines.append(f'{name}\t5\t2\t0\t{name}.rgba\t{hashlib.sha256(data).hexdigest()}')
(root/'manifest.tsv').write_text('\n'.join(lines)+'\n')
