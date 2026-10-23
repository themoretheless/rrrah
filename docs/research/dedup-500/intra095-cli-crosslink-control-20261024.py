import ast,logging,os,sys,tempfile,types,json,inspect
from pathlib import Path
from unittest.mock import patch
base=Path(__file__).resolve().parent/'intra095-20261024'
def load(path,cls,name,env):
 t=ast.parse((base/path).read_text());nodes=t.body if cls is None else next(n for n in t.body if isinstance(n,ast.ClassDef) and n.name==cls).body;n=next(n for n in nodes if isinstance(n,ast.FunctionDef) and n.name==name);exec(compile(ast.Module(body=[n],type_ignores=[]),str(base/path),'exec'),env);return env[name]
forwarded=[]
def process(filePath,**kw):forwarded.append(kw);return '',None,{}
rpc=types.SimpleNamespace(connect=lambda *a,**kw:types.SimpleNamespace(root=types.SimpleNamespace(processDownload=process)))
with tempfile.TemporaryDirectory(prefix='rrrah-intra095-owned-') as d:
 p=Path(d)/'archive.zip';p.write_bytes(b'owned')
 method=load('deduplicator/ProcessArchive.py',None,'commandLineProcess',dict(os=os))
 scanner_stub=types.ModuleType('scanner');log_stub=types.ModuleType('scanner.logSetup');scanner_stub.logSetup=log_stub;log_stub.initLogging=lambda:None
 with patch.dict(sys.modules,{'rpyc':rpc,'scanner':scanner_stub,'scanner.logSetup':log_stub}):
  for disabled in [False,True]:method(types.SimpleNamespace(sourcePath=str(p),noContext=False,noPhash=disabled))
 assert forwarded[0]==forwarded[1]
 # Keep only policy fields in persisted results, not disposable absolute paths.
 policies=[dict(distance=x['distance'],locked=x['locked'],moveToPath=x['moveToPath'],has_noPhash_option='noPhash' in x) for x in forwarded]
target=load('deduplicator/ProcessArchive.py',None,'processDownload',dict(ArchChecker=object))
binding_error=None
try:inspect.signature(target).bind('owned.zip',**forwarded[0])
except TypeError as e:binding_error=str(e)
assert binding_error and 'locked' in binding_error
updates=[];tags=[]
def rows(**kw):return [dict(dbId=7 if kw['downloadpath']=='owned-src' else 9)]
owner=types.SimpleNamespace(log=logging.getLogger('owned'),getRowsByValue=rows,updateDbEntryById=lambda *a,**kw:updates.append([a,kw]),addTags=lambda **kw:tags.append(kw))
load('cross_link/processDownload.py','DownloadProcessor','crossLink',dict(os=os))(owner,'owned-src/source.zip','owned-dst/dest.zip')
assert tags[-1]['dbId']==9 and tags[-1]['tags']=='crosslink-9'
print(json.dumps(dict(cli_no_phash_changes_forwarded_request=False,receiver_binding_error=binding_error,forwarded_policies=policies,crosslink_tags=tags,destination_links_to_itself=True,scope='Exact original commandLineProcess and crossLink bodies;owned temporary file,RPClocal stub and row/update/tag adapters;no network,realDB,archive processing,move or delete'),sort_keys=True))
