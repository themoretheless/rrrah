from pathlib import Path
from PIL import Image
import random,json,hashlib
root=Path('.dedup-work/dhash440-orientation-20261021');root.mkdir(exist_ok=True)
rng=random.Random(440);w,h=12,8;pixels=[tuple(rng.randrange(256) for _ in range(3)) for _ in range(w*h)];original=Image.new('RGB',(w,h));original.putdata(pixels)
rows=[]
for orientation in range(1,9):
 exif=Image.Exif();exif[274]=orientation;tagged=root/f'tagged-{orientation}.png';original.save(tagged,exif=exif)
 ow,oh=(w,h) if orientation<=4 else (h,w);expected=[]
 for y in range(oh):
  for x in range(ow):
   sx,sy={1:(x,y),2:(w-1-x,y),3:(w-1-x,h-1-y),4:(x,h-1-y),5:(y,x),6:(y,h-1-x),7:(w-1-y,h-1-x),8:(w-1-y,x)}[orientation];expected.append(pixels[sy*w+sx])
 physical=Image.new('RGB',(ow,oh));physical.putdata(expected);path=root/f'physical-{orientation}.png';physical.save(path)
 rows.append(dict(orientation=orientation,tagged=str(tagged.resolve()),physical=str(path.resolve()),expected_dimensions=[ow,oh],expected_pixel_sha256=hashlib.sha256(physical.tobytes()).hexdigest()))
(root/'cases.json').write_text(json.dumps(rows,indent=2)+'\n')
