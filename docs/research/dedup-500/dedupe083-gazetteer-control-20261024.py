"""Original Gazetteer method bodies, synthetic deterministic fingerprints, owned SQLite."""
import ast, hashlib, itertools, json, pathlib, sqlite3, tempfile
from types import SimpleNamespace
p=pathlib.Path(__file__).resolve().parents[3]/'docs/research/dedup-500/evidence/dedupeio--dedupe/06-api.py'
b=p.read_bytes();assert hashlib.sha256(b).hexdigest()=='15d527f0762820388df94ee710735b10772b6986724782853d2f18effa7dde06'
c=next(x for x in ast.parse(b).body if isinstance(x,ast.ClassDef) and x.name=='GazetteerMatching')
functions=[next(x for x in reversed(c.body) if isinstance(x,ast.FunctionDef) and x.name==name) for name in ['index','blocks']]
for f in functions:f.decorator_list=[]
m=ast.Module(body=[ast.ImportFrom(module='__future__',names=[ast.alias(name='annotations')],level=0)]+functions,type_ignores=[])
ns={'sqlite3':sqlite3,'itertools':itertools,'core':SimpleNamespace(sqlite_id_type=lambda data:'integer')}
exec(compile(ast.fix_missing_locations(m),str(p),'exec'),ns)
class FP:
 def index_all(self,data):pass
 def __call__(self,items,target=False):
  for k,r in items:yield r['key'],k
with tempfile.TemporaryDirectory(prefix='rrrah-dedupe-gazetteer-') as td:
 disk=SimpleNamespace(db=td+'/owned.db',fingerprinter=FP(),indexed_data={})
 ns['index'](disk,{1:{'key':'old'}})
 assert len(list(ns['blocks'](disk,{10:{'key':'old'}})))==1
 ns['index'](disk,{1:{'key':'new'}})
 old=list(ns['blocks'](disk,{10:{'key':'old'}}))
 assert old==[[((10,{'key':'old'}),(1,{'key':'new'}))]]
 new=list(ns['blocks'](disk,{11:{'key':'new'}}))
 assert len(new)==1
 memory=SimpleNamespace(db=':memory:',fingerprinter=FP(),indexed_data={})
 ns['index'](memory,{1:{'key':'old'}})
 try:list(ns['blocks'](memory,{10:{'key':'old'}}))
 except sqlite3.OperationalError as ex:
  assert 'no such table: indexed_records' in str(ex)
  error=str(ex)
 else:raise AssertionError('expected separate SQLite memory connections')
print(json.dumps({'disk_initial_match':True,'disk_replacement_new_match':True,'disk_replacement_old_block_still_returns_updated_record':True,'memory_index_then_blocks_error':error,'scope':'Original index/blocks bodies only with synthetic fingerprint and ID adapters. No original learner, predicate implementation or full-package invocation; no final false-positive classification claimed.'},indent=2))
