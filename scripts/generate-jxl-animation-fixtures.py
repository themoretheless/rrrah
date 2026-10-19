#!/usr/bin/env python3
"""CC0 animated JPEG XL controls; compare libjxl frames to Pillow GIF composition."""
from pathlib import Path
import tempfile,subprocess,json,hashlib
from PIL import Image
root=Path(__file__).resolve().parents[1]/'tests/fixtures/raster'
rows=[]
def presentations(path):
 im=Image.open(path);values=[];durations=[]
 for i in range(im.n_frames):
  im.seek(i);rgba=bytearray(im.convert('RGBA').tobytes())
  for j in range(0,len(rgba),4):
   if rgba[j+3]==0:rgba[j:j+3]=bytes(3)
  values.append(bytes(rgba));durations.append(im.info['duration'])
 return values,durations,im.info.get('loop')
with tempfile.TemporaryDirectory() as directory:
 d=Path(directory)
 for alpha in [False,True]:
  name='animation-alpha.jxl' if alpha else 'animation-rgb.jxl';frames=[]
  for n in range(3):
   if alpha:
    im=Image.new('P',(5,3));im.putpalette([0,0,0,255,0,0,0,255,0,0,0,255,17,29,43,61,73,89,250,240,230,120,40,200]+[0]*744)
    im.putdata([0 if (x+y+n)%4==0 else (x+y+n)%7+1 for y in range(3) for x in range(5)])
   else:
    im=Image.new('RGB',(5,3));im.putdata([((x*43+n*71)%256,(y*89+n*37)%256,(x*11+y*29+n*61)%256) for y in range(3) for x in range(5)])
   frames.append(im)
  options=dict(save_all=True,append_images=frames[1:],duration=[40,90,130],loop=2,optimize=False,disposal=2 if alpha else 1)
  if alpha:options['transparency']=0
  frames[0].save(d/'input.gif',**options)
  subprocess.run(['cjxl',str(d/'input.gif'),str(root/name),'-d','0','-e','1'],check=True,capture_output=True)
  subprocess.run(['djxl',str(root/name),str(d/'decoded.png')],check=True,capture_output=True)
  expected,times,input_loop=presentations(d/'input.gif');actual,out_times,output_loop=presentations(d/'decoded.png')
  assert actual==expected and times==out_times and len(actual)==3
  for i,frame in enumerate(actual):(root/f'{name}.{i}.rgba').write_bytes(frame)
  rows.append(dict(name=name,license='CC0',width=5,height=3,frames=3,durations_ms=times,input_gif_loop=input_loop,decoded_apng_loop=output_loop,loop_scope='Producer output must be inspected independently; GIF repeat count must not be assumed preserved by cjxl.',sha256=hashlib.sha256((root/name).read_bytes()).hexdigest(),rgba_sha256=[hashlib.sha256(f).hexdigest() for f in actual]))
(root/'jxl-animation.json').write_text(json.dumps(dict(scope='libjxl cjxl/djxl frames and durations agree with independently composed Pillow GIF. Invisible RGB normalized to zero. These fixtures do not establish viewer support.',producer=subprocess.run(['cjxl','--version'],capture_output=True,text=True).stdout.strip(),fixtures=rows),indent=2)+'\n')
