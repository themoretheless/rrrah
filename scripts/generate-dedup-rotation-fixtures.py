"""CC0 seeded linear-intensity fixtures, independently resampled by Pillow."""
from pathlib import Path
import hashlib,json,random,math
from PIL import Image,ImageFilter,__version__
root=Path(__file__).resolve().parent.parent/'crates/rrrah-dedup/tests/fixtures/rotation';root.mkdir(parents=True,exist_ok=True)
def image(seed):
    rng=random.Random(seed)
    return Image.frombytes('L',(160,160),bytes(rng.randrange(256) for _ in range(160*160))).filter(ImageFilter.GaussianBlur(1.2)).convert('RGBA')
base=image(842173);images={'base':base,'unrelated':image(413029),'mirrored':base.transpose(Image.Transpose.FLIP_LEFT_RIGHT)}
cases=[]
for angle in [17,-37,63]:
    name=f'angle-{angle}';images[name]=base.rotate(angle,resample=Image.Resampling.BILINEAR,fillcolor=(0,0,0,255))
    a=math.cos(math.radians(angle));b=-math.sin(math.radians(angle));c=79.5
    cases.append({'name':name,'degrees':angle,'a':a,'b':b,'translation':[c*(1-a+b),c*(1-a-b)]})
scales=[]
for size,angle in [(216,0),(117,0),(216,17)]:
    name=f'scale-{size}-angle-{angle}'
    resized=base.resize((size,size),resample=Image.Resampling.BILINEAR)
    images[name]=resized.rotate(angle,resample=Image.Resampling.BILINEAR,fillcolor=(0,0,0,255)) if angle else resized
    ratio=size/160;a=ratio*math.cos(math.radians(angle));b=-ratio*math.sin(math.radians(angle));center=(size-1)/2
    scales.append({'name':name,'size':size,'degrees':angle,'a':a,'b':b,'translation':[center-a*79.5+b*79.5,center-b*79.5-a*79.5]})
for name,value in images.items():
    value.save(root/f'{name}.png');(root/f'{name}.rgba').write_bytes(value.tobytes())
(root/'manifest.json').write_text(json.dumps({'license':'CC0-1.0','pillow':__version__,'input_policy':'RGBA code values interpreted as linear intensities, opaque alpha','dimensions':[160,160],'cases':cases,'scale_cases':scales,'sha256':{p.name:hashlib.sha256(p.read_bytes()).hexdigest() for p in sorted(root.iterdir()) if p.name!='manifest.json'}},indent=2)+'\n')
print('Independent rotations and unrelated negative saved')
