import ast, hashlib, io, json, logging, tempfile, traceback, types
from pathlib import Path
SOURCE=Path(__file__).resolve().parent/'intra095-20261024'
def load(path,cls,name,env):
 tree=ast.parse((SOURCE/path).read_text())
 nodes=tree.body if cls is None else next(x for x in tree.body if isinstance(x,ast.ClassDef) and x.name==cls).body
 node=next(x for x in nodes if isinstance(x,ast.FunctionDef) and x.name==name)
 exec(compile(ast.Module(body=[node],type_ignores=[]),str(SOURCE/path),'exec'),env)
 return env[name]
hash_env=dict(hashlib=hashlib,IMAGE_EXTS=('png',))
hashfn=load('scanner/hashFile.py',None,'hashFile',hash_env)
class DB:
 def __init__(self):self.events=[];self.rows=[]
 def begin(self):self.events.append('begin')
 def insertIntoDb(self,**kw):self.rows.append(kw)
 def commit(self):self.events.append('commit')
 def rollback(self):self.events.append('rollback');self.rows=[]
class Broken:
 def read(self):raise OSError('owned simulated read failure')
class Archive:
 def __init__(self,broken):self.broken=broken;self.closed=False
 def __iter__(self):return iter([('a.txt',io.BytesIO(b'first')),('b.txt',Broken() if self.broken else io.BytesIO(b'second'))])
 def close(self):self.closed=True
results=[]
for cancel,broken in [(False,False),(True,False),(False,True)]:
 state=types.SimpleNamespace(run=True);db=DB();archive=Archive(broken)
 def progress(value):
  if cancel:state.run=False
 worker=types.SimpleNamespace(dbApi=db,tlog=logging.getLogger('owned'),putProgQueue=progress)
 env=dict(uar=types.SimpleNamespace(ArchiveReader=lambda *a,**kw:archive),hasher=types.SimpleNamespace(hashFile=hashfn),scanner=types.SimpleNamespace(runState=state))
 error=None
 try:load('scanner/fileHasher.py','HashThread','scanArchive',env)(worker,'owned.zip',None)
 except OSError as e:error=str(e)
 results.append(dict(cancel_after_first=cancel,second_read_fails=broken,events=db.events,stored_members=[x['internalPath'] for x in db.rows],closed=archive.closed,error=error))
assert results[0]['stored_members']==['a.txt','b.txt']
assert results[1]['stored_members']==['a.txt'] and results[1]['events']==['begin','commit']
assert results[2]['events']==['begin','rollback'] and not results[2]['stored_members']
with tempfile.TemporaryDirectory(prefix='rrrah-intra095-owned-') as d:
 path=Path(d)/'item.bin';path.write_bytes(b'aaaa');old=hashlib.md5(path.read_bytes()).hexdigest();path.write_bytes(b'bbbb')
 events=[]
 db=types.SimpleNamespace(getItemsOnBasePath=lambda p:[dict(itemhash=old,pHash=None,imgx=None,imgy=None,internalPath='')])
 worker=types.SimpleNamespace(dbApi=db,putProgQueue=events.append,hashBareFile=lambda *a:events.append('rehash'),processImageFile=lambda *a:events.append('image'),processArchive=lambda *a:events.append('archive'))
 load('scanner/fileHasher.py','HashThread','processFile',dict(ARCH_EXTS=('zip',),IMAGE_EXTS=('png',),traceback=traceback))(worker,str(path))
 assert events==['skipped']
 stale=dict(bytes_changed=True,old_digest=old,current_digest=hashlib.md5(path.read_bytes()).hexdigest(),events=events)
print(json.dumps(dict(archive_cases=results,existing_bare_file=stale,scope='Exact AST-selected original scanArchive/processFile/hashFile bodies; repeatable archive and transactional DB adapters; real owned bare file;no PostgreSQL/archive decoder/image decoder/worker pool'),sort_keys=True))
