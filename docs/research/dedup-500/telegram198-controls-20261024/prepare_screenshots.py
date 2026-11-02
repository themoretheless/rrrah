import pathlib,json,urllib.request,hashlib,concurrent.futures
from PIL import Image,ImageOps,ImageDraw
b=pathlib.Path(__file__).parent;e=b.parent/'evidence/botnick--telegram-media-downloader';q=json.loads((b/'readme-screenshots-pending.json').read_text())['artifacts'];out=b/'screenshots';out.mkdir(exist_ok=True)
def get(x):
 raw=urllib.request.urlopen('https://raw.githubusercontent.com/botnick/telegram-media-downloader/5162cff1474ac2f3d0fb109bc1b6156d214cd33b/'+x['path'],timeout=40).read();assert hashlib.sha1(b'blob '+str(len(raw)).encode()+b'\0'+raw).hexdigest()==x['git_blob_sha1'];f=out/pathlib.Path(x['path']).name;f.write_bytes(raw);return {**x,'local':str(f),'sha256':hashlib.sha256(raw).hexdigest()}
with concurrent.futures.ThreadPoolExecutor(max_workers=4) as pool:records=list(pool.map(get,q))
for j in range(3):
 sheet=Image.new('RGB',(2000,1500),'white');draw=ImageDraw.Draw(sheet)
 for k,r in enumerate(records[j*4:j*4+4]):
  im=Image.open(r['local']).convert('RGB');im.thumbnail((980,710));x=(k%2)*1000;y=(k//2)*750;sheet.paste(im,(x,y+25));draw.text((x+5,y+5),r['path'],fill='black');r['sheet']=str(out/f'sheet-{j}.png')
 sheet.save(out/f'sheet-{j}.png')
(out/'render-manifest.json').write_text(json.dumps(records,indent=2)+'\n');print('12 pinned screenshots acquired/verified;3 sheets prepared,no view credit yet')
