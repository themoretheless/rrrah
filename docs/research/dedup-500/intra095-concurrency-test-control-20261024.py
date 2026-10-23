import ast,concurrent.futures,json,os,random,types
from pathlib import Path
p=Path(__file__).resolve().parent/'intra095-20261024/Tests/Test_BKTree_Concurrency_Mem.py';tree=ast.parse(p.read_text());cls=next(n for n in tree.body if isinstance(n,ast.ClassDef) and n.name=='TestSequenceFunctions_ConcurrentModification');method=next(n for n in cls.body if isinstance(n,ast.FunctionDef) and n.name=='test_concurrent_reads')
nodes=[n for n in tree.body if isinstance(n,ast.FunctionDef) and n.name in {'lookup_call','_VmB','resident'}]+[method]
futures=[]
class Executor(concurrent.futures.ThreadPoolExecutor):
 def submit(self,*a,**kw):
  f=super().submit(*a,**kw);futures.append(f);return f
calls=[]
class Tree:
 def getWithinDistance(self,*a):calls.append(a);return set()
env=dict(random=random,os=os,RANDOM_INIT=6461351,THREADS=12,TEST_SAMPLE_SIZE=10000,ThreadPoolExecutor=Executor)
exec(compile(ast.Module(body=nodes,type_ignores=[]),str(p),'exec'),env)
returned=env['test_concurrent_reads'](types.SimpleNamespace(tree=Tree()))
errors=[dict(type=type(f.exception()).__name__,message=str(f.exception())) for f in futures]
assert len(futures)==12 and all(x['type']=='TypeError' and 'offset' in x['message'] for x in errors) and not calls
rss=env['resident']();assert rss==0.0 and not Path('/proc/self/status').exists()
print(json.dumps(dict(test_method_returned_normally=returned is None,completed_tasks=sum(f.done() for f in futures),worker_errors=errors,actual_lookup_calls=len(calls),resident_on_current_macos=rss,scope='Exact original selected test/helper bodies with real ThreadPoolExecutor subclass recording futures;no original unittest setup,Cython tree or DB;native macOS procfs fallback'),sort_keys=True))
