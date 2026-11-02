"""Original PruneStage binary bucket reader; real owned files, folder adapter only."""
import ast
import hashlib
import io
import json
from pathlib import Path
import struct
import typing

source=Path(__file__).resolve().parents[1]/'evidence/icodywu--minhash-dedup/02-minhash.py'
tree=ast.parse(source.read_text())
original=next(n for n in tree.body if isinstance(n,ast.ClassDef) and n.name=='PruneStage')
names={'_pack_key','_unpack_key','_iter_bucket_rows'}
nodes=[n for n in original.body if isinstance(n,ast.FunctionDef) and n.name in names]
attrs=[n for n in original.body if isinstance(n,ast.Assign) and any(isinstance(t,ast.Name) and t.id in {'_count_struct','_pair_struct','_read_id_mask'} for t in n.targets)]
original.body=attrs+nodes;original.bases=[]
scope=dict(vars(typing),io=io,struct=struct)
exec(compile(ast.Module(body=[original],type_ignores=[]),str(source),'exec'),scope)
directory=Path(__file__).with_name('owned-bucket-fixtures');directory.mkdir(exist_ok=True)
class Folder:
    def open(self,path,mode):
        return (directory/path).open(mode)
reader=scope['PruneStage']();reader.input_folder=Folder();reader.buffer_size_bytes=32
observations=[]
def rows(name,payload):
    (directory/name).write_bytes(payload)
    return list(reader._iter_bucket_rows(name))
payload=struct.pack('<I4I',2,0,7,5,2**32-1)
valid=rows('valid.dups',payload)
assert [reader._unpack_key(x) for x in valid[0][1]]==[(0,7),(5,2**32-1)]
observations.append({'case':'valid binary records preserve reader and max uint32 document IDs','passed':True})
for name,payload,fragment in [('short-count.dups',b'\x02','truncated bucket count'),('short-pair.dups',struct.pack('<I',2)+struct.pack('<II',0,7),'truncated pair payload')]:
    error=None
    try:rows(name,payload)
    except ValueError as exc:error=str(exc)
    assert error and fragment in error
    observations.append({'case':name,'error':error})
assert rows('empty.dups',b'')==[]
observations.append({'case':'empty binary file yields no rows','passed':True})
result={'scope':__doc__,'source_sha256':hashlib.sha256(source.read_bytes()).hexdigest(),'authored_controls_run':4,'original_tests_run':0,'observations':observations,'not_executed':['complete PruneStage','BucketStage heap merge','cross-band subset algorithm','scheduler','real corpus']}
Path(__file__).with_name('bucket-reader-results.json').write_text(json.dumps(result,indent=2)+'\n')
print(json.dumps(result,indent=2))
