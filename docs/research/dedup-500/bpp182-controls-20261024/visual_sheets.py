from PIL import Image,ImageOps,ImageDraw
from pathlib import Path
import json,hashlib
b=Path(__file__).resolve().parent;e=b.parent/'evidence/Arkalogy--best-photo-picker';xs=[x for x in json.loads((e/'tree.json').read_text())['tree'] if x['type']=='blob' and Path(x['path']).suffix.lower() in ['.png','.jpg','.jpeg','.gif','.webp','.svg','.ico']];icons=[x for x in xs if not x['path'].startswith('docs/screenshots/')];screens=[x for x in xs if x not in icons];groups=[icons]+[screens[i:i+2] for i in range(0,len(screens),2)];records=[]
for k,g in enumerate(groups):
 w,h,cols=(220,200,4) if k==0 else (900,620,1);canvas=Image.new('RGB',(w*cols,h*((len(g)+cols-1)//cols)),(235,235,235));draw=ImageDraw.Draw(canvas)
 for j,x in enumerate(g):
  f=e/'visual-pinned'/x['path'];raw=f.read_bytes();assert hashlib.sha1(b'blob '+str(len(raw)).encode()+b'\0'+raw).hexdigest()==x['sha']
  with Image.open(f) as im:
   sizes=sorted(im.ico.sizes()) if f.suffix=='.ico' else [im.size];rgba=im.convert('RGBA');thumb=ImageOps.contain(rgba,(w-12,h-38));px=(j%cols)*w+(w-thumb.width)//2;py=(j//cols)*h+30;canvas.paste(thumb,(px,py),thumb);draw.text(((j%cols)*w+5,(j//cols)*h+5),x['path'].split('/')[-1],fill='black');records.append({'path':x['path'],'sha256':hashlib.sha256(raw).hexdigest(),'git_blob_sha1':x['sha'],'source_sizes':sizes,'view_sheet':f'visual-{k}.png','view_scope':'decoded raster at fit size; ICO default selected frame only; not every icon resolution'})
 canvas.save(b/f'visual-{k}.png')
(b/'visual-inventory.json').write_text(json.dumps(records,indent=2)+'\n');print(len(records),'artifacts',len(groups),'sheets')
