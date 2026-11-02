from pathlib import Path
from concurrent.futures import ThreadPoolExecutor,as_completed
from urllib.request import urlopen
from urllib.parse import quote
import hashlib,json
b=Path(__file__).resolve().parent;e=b.parent/'evidence/garypen--deduplicate';xs=[x for x in json.loads((e/'tree.json').read_text())['tree'] if x['type']=='blob' and x['path'].startswith('target/')]
def one(x):
 f=e/'remaining-pinned'/x['path'];f.parent.mkdir(parents=True,exist_ok=True)
 try:
  if not f.exists():
   data=urlopen('https://raw.githubusercontent.com/garypen/deduplicate/8f191f2dcf5e083f5b76f5de90cc60e1253f85b0/'+quote(x['path'],safe='/'),timeout=25).read();f.write_bytes(data)
  data=f.read_bytes();sha=hashlib.sha1(b'blob '+str(len(data)).encode()+b'\0'+data).hexdigest();assert sha==x['sha'];return {'path':x['path'],'bytes':len(data),'git_blob_sha1':sha,'sha256':hashlib.sha256(data).hexdigest(),'verified':True}
 except Exception as ex:return {'path':x['path'],'verified':False,'error':repr(ex)}
rows=[]
with ThreadPoolExecutor(max_workers=8) as pool:
 for future in as_completed([pool.submit(one,x) for x in xs]):
  rows.append(future.result())
  if len(rows)%100==0:print(len(rows),'/',len(xs),flush=True)
rows.sort(key=lambda x:x['path']);(b/'reports-acquisition.json').write_text(json.dumps({'commit':'8f191f2dcf5e083f5b76f5de90cc60e1253f85b0','verified':sum(x['verified'] for x in rows),'expected':len(xs),'artifacts':rows},indent=2)+'\n');print('verified',sum(x['verified'] for x in rows),'/',len(xs),flush=True)
