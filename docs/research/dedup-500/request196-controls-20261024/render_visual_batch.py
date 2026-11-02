from pathlib import Path
import json,subprocess,hashlib
from PIL import Image,ImageChops,ImageOps,ImageDraw
b=Path(__file__).resolve().parent;root=b.parent/'evidence/garypen--deduplicate/remaining-pinned';pending=json.loads((b/'pending-visuals.json').read_text());batch=pending[:24];n=1
while (b/f'visual-batch{n:02}').exists():n+=1
out=b/f'visual-batch{n:02}';out.mkdir();rows=[];thumbs=[]
for i,p in enumerate(batch):
 f=root/p;folder=out/f'{i:02}';folder.mkdir(exist_ok=True);r=subprocess.run(['qlmanage','-t','-s','1200','-o',str(folder),str(f)],capture_output=True,text=True,timeout=20);assert r.returncode==0;png=folder/(f.name+'.png');im=Image.open(png).convert('RGB');bbox=ImageChops.difference(im,Image.new('RGB',im.size,'white')).getbbox();assert bbox;cropped=im.crop(bbox);thumb=ImageOps.contain(cropped,(940,550));thumbs.append(thumb);rows.append({'path':p,'sha256':hashlib.sha256(f.read_bytes()).hexdigest(),'render':str(png),'source_render_size':list(im.size),'whitespace_crop':list(bbox),'sheet':str(out/f'sheet-{i//4}.png'),'cell':i%4})
for start in range(0,len(rows),4):
 sheet=Image.new('RGB',(1920,1200),'white');draw=ImageDraw.Draw(sheet)
 for j in range(start,min(start+4,len(rows))):
  cell=j-start;x=(cell%2)*960;y=(cell//2)*600;label=rows[j]['path'].replace('target/criterion/get/','');draw.text((x+10,y+6),label,fill='black');im=thumbs[j];sheet.paste(im,(x+(960-im.width)//2,y+35))
 sheet.save(out/f'sheet-{start//4}.png')
(out/'render-manifest.json').write_text(json.dumps(rows,indent=2)+'\n');print('Rendered',len(rows),'originalSVGs rendered;no review credit yet')
