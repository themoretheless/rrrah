"""Unchanged original read_tuples_from_file function, actual stdlib byte streams."""
import ast
import hashlib
import io
import json
from pathlib import Path
import struct

source=Path(__file__).resolve().parents[1]/'evidence/icodywu--minhash-dedup/remaining-pinned/src/minhash_dedup/signatures.py'
tree=ast.parse(source.read_text());fn=next(n for n in tree.body if isinstance(n,ast.FunctionDef) and n.name=='read_tuples_from_file')
scope={'struct':struct};exec(compile(ast.Module(body=[fn],type_ignores=[]),str(source),'exec'),scope);read=scope['read_tuples_from_file']
payload=struct.pack('<6I',0,7,5,99,8,2**32-1)
assert list(read(io.BytesIO(payload),'2I',lines_to_buffer=2))==[(0,7),(5,99),(8,2**32-1)]
assert list(read(io.BytesIO(payload),'2I',lines_to_buffer=-1))==[(0,7),(5,99),(8,2**32-1)]
class ShortRead(io.BytesIO):
    def read(self,size=-1):return super().read(min(size,5) if size>=0 else 5)
short=None
try:list(read(ShortRead(payload),'2I',lines_to_buffer=2))
except struct.error as exc:short=str(exc)
assert short is not None
bad=None
try:list(read(io.BytesIO(payload),'2I',lines_to_buffer=0))
except ValueError as exc:bad=str(exc)
assert bad is not None
result={'scope':__doc__,'source_sha256':hashlib.sha256(source.read_bytes()).hexdigest(),'authored_controls_run':4,'original_tests_run':0,'observations':['aligned buffered input yields all3 little-endian records','unlimited mode yields same3 records',{'case':'valid complete stream with short5-byte reads raises instead of accumulating one8-byte record','error':short,'qualification':'generic stream boundary; actual fsspec/local read semantics and distributed pipeline not qualified'},'zero buffer count rejected with ValueError']}
Path(__file__).with_name('tuple-reader-results.json').write_text(json.dumps(result,indent=2)+'\n');print(json.dumps(result,indent=2))
