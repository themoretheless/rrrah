import ast,contextlib,json,logging,traceback,types
from pathlib import Path
base=Path(__file__).resolve().parent/'intra095-20261024'
def load(path,cls,name,env):
 tree=ast.parse((base/path).read_text());nodes=tree.body if cls is None else next(n for n in tree.body if isinstance(n,ast.ClassDef) and n.name==cls).body
 n=next(n for n in nodes if isinstance(n,ast.FunctionDef) and n.name==name)
 exec(compile(ast.Module(body=[n],type_ignores=[]),str(base/path),'exec'),env);return env[name]
logger=logging.getLogger('owned');logger.addHandler(logging.NullHandler());logger.propagate=False
class Cursor:
 def __init__(self,fail):self.events=[];self.fail=fail
 def execute(self,s):
  self.events.append(s)
  if s==self.fail:raise OSError('owned '+s+' failed')
rows=[]
transaction=load('cross_link/DbRoot.py','TransactionMixin','transaction',dict(contextmanager=contextlib.contextmanager,traceback=traceback))
for label,fail,body,commit in [('success',None,None,True),('body_error',None,ValueError('owned body'),True),('begin_error','BEGIN;',None,True),('commit_error','COMMIT;',None,True),('body_and_commit_error','COMMIT;',ValueError('owned body'),True),('keyboard_interrupt',None,KeyboardInterrupt('owned interrupt'),True),('unmanaged',None,ValueError('owned body'),False)]:
 cur=Cursor(fail);released=[];owner=types.SimpleNamespace(get_cursor=lambda:cur,release_cursor=lambda c:released.append(c),log=logger)
 error=None
 try:
  with transaction(owner,commit=commit):
   if body is not None:raise body
 except BaseException as e:error=type(e).__name__+':'+str(e)
 rows.append(dict(case=label,commands=cur.events,released=len(released),error=error))
assert rows[1]['commands']==['BEGIN;','ROLLBACK;','COMMIT;']
assert rows[2]['released']==rows[3]['released']==rows[4]['released']==0
assert rows[4]['error'].startswith('OSError') and rows[5]['commands']==['BEGIN;','COMMIT;']
returned=[];pool=types.SimpleNamespace(pool=types.SimpleNamespace(putconn=returned.append))
owner=types.SimpleNamespace(connections={123:'owned-connection'})
load('cross_link/DbRoot.py','DbBase','__del__',dict(dbPool=pool))(owner)
assert returned==[123]
removed=[]
remove=load('cross_link/RetreivalDbBase.py','ScraperDbBase','removeTags',{})
for tags in ['one two',None]:
 updates=[];owner=types.SimpleNamespace(getRowByValue=lambda **kw:dict(tags=tags,sourceUrl='owned'),updateDbEntry=lambda *a,**kw:updates.append(kw))
 err=None
 try:remove(owner,sourceUrl='owned',tags='one')
 except Exception as e:err=type(e).__name__
 removed.append(dict(existing=tags,updates=updates,error=err))
assert removed[0]['updates']==[{'tags':''}] and removed[1]['error']=='AttributeError'
print(json.dumps(dict(transaction_cases=rows,destructor_returned_thread_key_instead_of_connection=returned==[123],remove_tags=removed,scope='Exact AST-selected original methods with command-recording cursor/pool/row adapters;no database,filesystem changes,network or actual interrupt signal'),sort_keys=True))
