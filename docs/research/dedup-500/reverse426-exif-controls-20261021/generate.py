from pathlib import Path
from PIL import Image,TiffImagePlugin
import hashlib,json
r=Path('.dedup-work/reverse426-exif-20261021');r.mkdir(exist_ok=True);rows=[]
for name,n,d in [('normal',1,60),('zero_numerator',0,1)]:
 exif=Image.Exif();exif[34665]={33434:TiffImagePlugin.IFDRational(n,d)};p=r/(name+'.jpg');Image.new('RGB',(8,8),(23,45,67)).save(p,exif=exif)
 rows.append(dict(name=name,exposure_numerator=n,exposure_denominator=d,sha256=hashlib.sha256(p.read_bytes()).hexdigest()))
Path('docs/research/dedup-500/reverse426-exif-controls-20261021/fixtures.json').write_text(json.dumps(rows,indent=2)+'\n')
