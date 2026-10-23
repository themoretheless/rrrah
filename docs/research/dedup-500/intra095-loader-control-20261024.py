import ast,concurrent.futures,json,logging,types
from pathlib import Path
p=Path(__file__).resolve().parent/'intra095-20261024/loader.py';tree=ast.parse(p.read_text());fn=next(n for n in tree.body if isinstance(n,ast.FunctionDef) and n.name=='do_search');cls=next(n for n in tree.body if isinstance(n,ast.ClassDef) and n.name=='Importer');method=next(n for n in cls.body if isinstance(n,ast.FunctionDef) and n.name=='extract_edges')
class Progress:
 def __init__(self,items,**kw):self.items=items
 def __iter__(self):return iter(self.items)
 def set_description(self,s):pass
env=dict(concurrent=concurrent,tqdm=types.SimpleNamespace(tqdm=Progress),SEARCH_SCOPE_CONST=4,INSERT_CHUNK_SIZE_CONST=250)
exec(compile(ast.Module(body=[fn,method],type_ignores=[]),str(p),'exec'),env)
results=[]
for n in [3,51,53]:
 entries={i:7 for i in range(1,n+1)};saved=set();calls=[]
 class Tree:
  def getWithinDistance(self,h,d):calls.append(h);return set(entries)
 db=types.SimpleNamespace(insert_phash_link_many=lambda s:saved.update(s));owner=types.SimpleNamespace(log=logging.getLogger('owned'),db=db)
 env['extract_edges'](owner,entries,Tree(),search_dist=0)
 expected={(i,j,0) for i in entries for j in entries if i<j}
 results.append(dict(entries=n,search_calls_completed=len(calls),expected_edges=len(expected),saved_edges=len(saved),missing_edges=len(expected-saved),unexpected_edges=len(saved-expected)))
assert results[0]['saved_edges']==0 and results[1]['missing_edges']==0 and results[2]['missing_edges']==1
print(json.dumps(dict(cases=results,scope='Exact original do_search/extract_edges bodies with real ThreadPoolExecutor;owned constant fingerprint entries,complete tree-query and recording DB/progress adapters;no actual tree/database/graph services'),sort_keys=True))
