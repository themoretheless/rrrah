"""Complete original FilterStage, owned removal files; no cluster metadata files.

Stage/folder/log/stat labels are adapters. Metadata tuple reader deliberately
raises if called: cluster-metadata loading is not qualified by these controls.
"""
import ast
import contextlib
import __future__
import hashlib
import json
from pathlib import Path
import struct
from types import SimpleNamespace

source=Path(__file__).resolve().parents[1]/'evidence/icodywu--minhash-dedup/02-minhash.py'
tree=ast.parse(source.read_text());cls=next(n for n in tree.body if isinstance(n,ast.ClassDef) and n.name=='FilterStage')
class Stage:
    def stat_update(self,*a):pass
    def track_time(self):return contextlib.nullcontext()
class Log:
    def warning(self,*a):pass
class Folder:
    def __init__(self,path):self.path=path;path.mkdir(parents=True,exist_ok=True)
    def isfile(self,path):return (self.path/path).is_file()
    def exists(self,path):return (self.path/path).exists()
    def open(self,path,mode):return (self.path/path).open(mode)
def no_metadata_reader(*a,**k):raise AssertionError('metadata read outside qualified scope')
scope=dict(Stage=Stage,contextlib=contextlib,struct=struct,get_datafolder=lambda x:x,logger=Log(),StatLabel=SimpleNamespace(total='total',forwarded='forwarded',dropped='dropped'),read_tuples_from_file=no_metadata_reader)
exec(compile(ast.Module(body=[cls],type_ignores=[]),str(source),'exec',flags=__future__.annotations.compiler_flag),scope)
root=Path(__file__).with_name('owned-filter-fixtures');root.mkdir(exist_ok=True)
def execute(name,payload):
    folder=Folder(root/name)
    if payload is not None:(folder.path/'000000.remove').write_bytes(payload)
    documents=[SimpleNamespace(text=str(i),metadata={}) for i in range(4)]
    return list(scope['FilterStage'](folder).run(documents))
missing=execute('missing',None);assert [d.text for d in missing]==['0','1','2','3'];assert all(d.metadata=={} for d in missing)
sorted_out=execute('sorted',struct.pack('<II',1,2));assert [d.text for d in sorted_out]==['0','3'];assert all(d.metadata['minhash_cluster_id']==-1 for d in sorted_out)
unsorted=execute('unsorted',struct.pack('<II',2,1));assert [d.text for d in unsorted]==['0','1','3']
error=None
try:execute('truncated',b'\x01')
except struct.error as exc:error=str(exc)
assert error is not None
result={'scope':__doc__,'source_sha256':hashlib.sha256(source.read_bytes()).hexdigest(),'authored_controls_run':4,'original_tests_run':0,'observations':['missing removal and metadata files pass all documents unchanged','sorted removal IDs1,2 keep0,3 and set unknown-cluster metadata','unsorted removal IDs2,1 silently retain requested ID1; Stage3 writes sorted IDs, so malformed-boundary behavior only','truncated first removal record raises struct.error'],'truncated_error':error}
Path(__file__).with_name('filter-results.json').write_text(json.dumps(result,indent=2)+'\n')
print(json.dumps(result,indent=2))
