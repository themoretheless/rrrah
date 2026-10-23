"""Execute unchanged parsed pairs body with explicit synthetic fingerprinter adapter."""
import ast
import hashlib
import itertools
import json
import pathlib
import sqlite3
import tempfile
from types import SimpleNamespace
p=pathlib.Path(__file__).resolve().parents[3]/'docs/research/dedup-500/evidence/dedupeio--dedupe/06-api.py'
b=p.read_bytes()
assert hashlib.sha256(b).hexdigest()=='15d527f0762820388df94ee710735b10772b6986724782853d2f18effa7dde06'
t=ast.parse(b)
c=next(n for n in t.body if isinstance(n,ast.ClassDef) and n.name=='DedupeMatching')
f=next(n for n in c.body if isinstance(n,ast.FunctionDef) and n.name=='pairs')
# Retain original method body, postpone its original type annotation evaluation.
m=ast.Module(body=[ast.ImportFrom(module='__future__',names=[ast.alias(name='annotations')],level=0),f],type_ignores=[])
ns={'sqlite3':sqlite3,'tempfile':tempfile,'core':SimpleNamespace(sqlite_id_type=lambda data: 'integer' if all(isinstance(k,int) for k in data) else 'text')}
exec(compile(ast.fix_missing_locations(m),str(p),'exec'),ns)
class FP:
 def __init__(self,blocks): self.blocks=blocks;self.reset=False
 def index_all(self,data): pass
 def __call__(self,items):
  for k,_ in items:
   for block in self.blocks[k]:yield block,k
 def reset_indices(self):self.reset=True
results=[]
for in_memory in [True,False]:
 for ids in [(1,2,3,4),('a','b','c','d')]:
  data={k:{'name':str(k)} for k in ids}
  blocks={ids[0]:['x','y'],ids[1]:['x','y'],ids[2]:['y'],ids[3]:['z']}
  fp=FP(blocks);obj=SimpleNamespace(fingerprinter=fp,in_memory=in_memory)
  actual={tuple(x[0] for x in pair) for pair in ns['pairs'](obj,data)}
  expected={(a,b) for a,b in itertools.combinations(ids,2) if set(blocks[a])&set(blocks[b])}
  assert actual==expected and len(actual)==3 and fp.reset
  results.append({'in_memory':in_memory,'id_kind':type(ids[0]).__name__,'unique_pairs':len(actual),'passed':True})
print(json.dumps({'controls':results,'scope':'Unchanged original pairs method body with synthetic fingerprinter and SQLite ID-type adapter. Original learner, predicates, core, package imports and upstream tests not executed.'},indent=2))
