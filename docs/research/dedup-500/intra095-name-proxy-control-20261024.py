import ast,json,logging,os,re,tempfile,threading,types
from pathlib import Path
p=Path(__file__).resolve().parent/'intra095-20261024/cross_link/nameTools.py';tree=ast.parse(p.read_text());cls=next(n for n in tree.body if isinstance(n,ast.ClassDef) and n.name=='DirNameProxy')
fnames={'makeFilenameSafe','sanitizeString','removeBrackets','prepFilenameForMatching','extractRating'}
nodes=[n for n in tree.body if isinstance(n,ast.FunctionDef) and n.name in fnames]
nodes += [n for n in tree.body if isinstance(n,ast.Assign) and any(isinstance(x,ast.Name) and x.id=='bracketStripRe' for x in n.targets)]
methods=[n for n in cls.body if isinstance(n,ast.FunctionDef) and n.name in {'__contains__','__getitem__','filterPreppedNameThroughDB','_processItemIntoRet','getDirDict','checkUpdate'}]
nodes.append(ast.ClassDef(name='Proxy',bases=[],keywords=[],body=methods,decorator_list=[]))
env=dict(os=os,re=re,time=types.SimpleNamespace(time=lambda:100.0),getCanonicalMangaUpdatesName=lambda s:s)
exec(compile(ast.fix_missing_locations(ast.Module(body=nodes,type_ignores=[])),str(p),'exec'),env)
proxy=env['Proxy']();proxy.log=logging.getLogger('owned');proxy.testMode=True;proxy.notifierRunning=False;proxy._dirDicts={}
empty_contains='   ' in proxy;empty_get=proxy['   ']
assert empty_contains and empty_get['fqPath'] is None
with tempfile.TemporaryDirectory(prefix='rrrah-intra095-owned-') as d:
 for name in ['Report.1','Report1']:(Path(d)/name).mkdir()
 mapping=proxy.getDirDict(d)
 assert len(mapping)==1
 collision=dict(source_directories=2,normalized_keys=list(mapping),retained_directory=Path(next(iter(mapping.values()))).name)
proxy.paths={0:dict(dir=str(Path(d)/'missing'),lastScan=100.0,interval=3600.0)};proxy.lastCheck=0;proxy.maxRate=5;proxy.updateLock=threading.Lock();proxy.eventH=types.SimpleNamespace(getClearChangedStatus=lambda p:True)
error=None
try:proxy.checkUpdate(force=True)
except Exception as e:error=type(e).__name__
lock_free=proxy.updateLock.acquire(blocking=False)
if lock_free:proxy.updateLock.release()
assert error=='ValueError' and not lock_free
print(json.dumps(dict(empty_key_membership=empty_contains,empty_key_has_path=empty_get['fqPath'] is not None,normalization_collision=collision,forced_missing_directory_refresh_error=error,update_lock_available_after_error=lock_free,scope='Exact selected original name/proxy definitions;identity canonical-name adapter,controlled time,real owned directories and lock;no import-time DB/cache objects,watcher,rename or network'),sort_keys=True))
