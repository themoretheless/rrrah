import ast,sys,json,tempfile,threading,time
from pathlib import Path
from typing import Optional
from types import SimpleNamespace
sys.path[:0]=[str(Path('.dedup-work/mediadedupe444-20261021').resolve()),str(Path('.dedup-work/mediahash478-extra-deps-20261021').resolve())]
import engine
from PIL import Image
source=Path('.dedup-work/mediadedupe444-20261021/app.py').read_text();tree=ast.parse(source)
worker=next(n for n in tree.body if isinstance(n,ast.FunctionDef) and n.name=='scan_worker')
messages=next(n for n in tree.body if isinstance(n,ast.Assign) and any(isinstance(t,ast.Name) and t.id=='SCAN_MESSAGES' for t in n.targets))
class Request(SimpleNamespace):
 def model_dump(self):return vars(self)
ns=dict(fd=engine,Path=Path,Optional=Optional,ScanRequest=Request,threading=threading,time=time,json=json,JOBS={},JOBS_LOCK=threading.Lock(),log_action=lambda text:None,_start_next_job=lambda:None)
exec(compile(ast.Module(body=[messages,worker],type_ignores=[]),'<original-app-worker>','exec'),ns)
out={}
with tempfile.TemporaryDirectory() as tmp:
 root=Path(tmp);Image.new('RGB',(4,4),'red').save(root/'one.png');engine.DB_PATH=root/'cache.sqlite'
 for name,kind,d2 in [('overlapping_directories','images',str(root)),('unknown_media_type','unrecognized',None)]:
  req=Request(dir1=str(root),dir2=d2,media_type=kind,workers=1,lang='en',phash_threshold=8)
  ns['JOBS'][name]=dict(status='running',log=[],results=None,error=None,stop_event=threading.Event(),_started_at=time.time())
  ns['scan_worker'](name,req);job=ns['JOBS'][name];assert job['status']=='done',job
  groups=job['results']['image_exact']
  if d2:assert len(groups)==1 and len(groups[0])==2 and groups[0][0]['path']==groups[0][1]['path']
  else:assert not groups
  out[name]=dict(status=job['status'],exact_group_count=len(groups),same_path_twice=bool(groups and groups[0][0]['path']==groups[0][1]['path']))
out['scope']='Unchanged original scan_worker and SCAN_MESSAGES compiled from AST; actual engine/SQLite/Pillow. Request adapter supplies attributes/model_dump; queue dispatch disabled. FastAPI/uvicorn absent, no HTTP route/model/frontend runtime qualification.'
Path(__file__).with_name('result.json').write_text(json.dumps(out,indent=2)+'\n');print(json.dumps(out,indent=2))
