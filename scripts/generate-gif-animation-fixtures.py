"""CC0 authored GIFs and Pillow independent selected-presentation oracles."""
from pathlib import Path
from PIL import Image
import hashlib
root=Path('tests/fixtures/raster'); root.mkdir(exist_ok=True,parents=True)
rows=[]
for disposal in (1,2,3):
 frames=[]
 for i in range(4):
  f=Image.new('P',(5,3),0); f.putpalette([0,0,0,255,0,0,0,255,0,0,0,255]+[0]*756)
  if i==0:
   for y in range(3):
    for x in range(5):f.putpixel((x,y),1)
  else:
   for y in range(2):
    for x in range(i-1,i+1):f.putpixel((x,y),2 if i<3 else 3)
  frames.append(f)
 name=f'gif-animation-disposal-{disposal}.gif';p=root/name
 frames[0].save(p,save_all=True,append_images=frames[1:],duration=[30,70,0,110],loop=2,transparency=0,disposal=[1,disposal,disposal,1],optimize=False)
 with Image.open(p) as image:
  for i in range(image.n_frames):
   image.seek(i); oracle=f'gif-animation-disposal-{disposal}-frame-{i}.rgba';(root/oracle).write_bytes(image.convert('RGBA').tobytes())
   rows.append('\t'.join(map(str,[name,i,5,3,image.info.get('duration',0),image.info.get('loop',''),oracle,hashlib.sha256(p.read_bytes()).hexdigest()])))
(root/'gif-animation-manifest.tsv').write_text('# source\tindex\twidth\theight\tdelay_ms\trepeats\toracle\tsource_sha256\n'+'\n'.join(rows)+'\n')
