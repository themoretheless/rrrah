import ast,contextlib,json,logging,types
from pathlib import Path
p=Path(__file__).resolve().parent/'intra095-20261024/dbApi.py';tree=ast.parse(p.read_text());cls=next(n for n in tree.body if isinstance(n,ast.ClassDef) and n.name=='DbApi');names={'insert_phash_link_many','begin','transaction'};nodes=[n for n in cls.body if isinstance(n,ast.FunctionDef) and n.name in names]
class OperationalError(Exception):pass
class IntegrityError(Exception):pass
env=dict(contextmanager=contextlib.contextmanager,psycopg2=types.SimpleNamespace(OperationalError=OperationalError,IntegrityError=IntegrityError));exec(compile(ast.Module(body=nodes,type_ignores=[]),str(p),'exec'),env)
results=[]
for generator in [False,True]:
 saved=[];fail=[True]
 class Cursor:
  def execute(self,q,params):
   if fail[0]:fail[0]=False;raise OperationalError('owned one-shot failure')
   saved.append(params)
 @contextlib.contextmanager
 def tx():yield Cursor()
 owner=types.SimpleNamespace(tableName='owned',transaction=tx,log=logging.getLogger('owned'))
 entries=[(1,2,4),(2,3,4),(3,4,4)]
 env['insert_phash_link_many'](owner,iter(entries) if generator else entries)
 results.append(dict(single_pass_input=generator,saved=saved))
assert len(results[0]['saved'])==3 and len(results[1]['saved'])==2
commands=[];fail=[True]
class BoundaryCursor:
 def execute(self,q):
  commands.append(q)
  if q=='BEGIN;' and fail[0]:fail[0]=False;raise OperationalError('owned BEGIN failure')
owner=types.SimpleNamespace(conn=types.SimpleNamespace(cursor=lambda:BoundaryCursor()),inLargeTransaction=False,log=logging.getLogger('owned'))
try:env['begin'](owner)
except OperationalError:pass
state=owner.inLargeTransaction
with env['transaction'](owner):commands.append('BODY')
assert state and commands==['BEGIN;','BODY']
print(json.dumps(dict(link_retry_cases=results,failed_begin_leaves_large_transaction_flag=state,subsequent_transaction_commands=commands,scope='Exact original DB method bodies with owned cursor/transaction/error adapters;no PostgreSQL connection,query execution or persistence'),sort_keys=True))
