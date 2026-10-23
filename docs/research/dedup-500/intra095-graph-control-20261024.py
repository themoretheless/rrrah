import ast,contextlib,json,types
from pathlib import Path
p=Path(__file__).resolve().parent/'intra095-20261024/graphApi.py';tree=ast.parse(p.read_text());cls=next(n for n in tree.body if isinstance(n,ast.ClassDef) and n.name=='GraphApi');nodes=[n for n in tree.body if isinstance(n,ast.FunctionDef) and n.name in {'add_phash4_link','insert_entries','add_link_sets'}]+[n for n in cls.body if isinstance(n,ast.FunctionDef) and n.name in {'insert_link','insert_link_batch'}];env={};exec(compile(ast.Module(body=nodes,type_ignores=[]),str(p),'exec'),env)
queries=[];tx=types.SimpleNamespace(run=lambda q,**kw:queries.append(kw));session=types.SimpleNamespace(write_transaction=lambda fn,**kw:fn(tx,**kw));owner=types.SimpleNamespace(_driver=types.SimpleNamespace(session=lambda:contextlib.nullcontext(session)))
error=None
try:env['insert_link'](owner,(1,'owned-a'),(2,'owned-b'))
except TypeError as e:error=str(e)
assert error and 'from_intpath' in error and 'to_intpath' in error
assert not queries
env['insert_link_batch'](owner,[((1,'owned-a','page-a'),(2,'owned-b','page-b'))])
assert len(queries)==1 and queries[0]['from_intpath']=='page-a' and queries[0]['to_intpath']=='page-b'
print(json.dumps(dict(single_link_error=error,batch_queries=len(queries),batch_parameters=queries,scope='Exact original graph functions and methods;callback/session/transaction adapters,no Neo4j driver or network'),sort_keys=True))
