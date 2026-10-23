"""Execute exact AST-extracted original functions; no ECC backend or module integration."""
import ast,io,json,os
from pathlib import Path
root=Path('.dedup-work/pyfilefixity044-20261023/pyFileFixity')
functions=[]
for path,names in [(root/'lib/eccman.py',{'compute_ecc_params'}),(root/'structural_adaptive_ecc.py',{'feature_scaling','stream_entry_assemble'})]:
    tree=ast.parse(path.read_text())
    functions.extend(n for n in tree.body if isinstance(n,ast.FunctionDef) and n.name in names)
namespace={'os':os}
exec(compile(ast.Module(body=functions,type_ignores=[]),'original-function-extract','exec'),namespace)
class Hasher:
    def __len__(self):return 2
source=io.BytesIO(b'abcde');ecc=io.BytesIO(b'HHXNEXTRECORD')
result=list(namespace['stream_entry_assemble'](Hasher(),source,ecc,{'ecc_field_pos':[0,3],'filesize':5},10,10,[0.5],constantmode=True))
assert len(result)==1 and result[0]['ecc']==b'XNEXT'
assert ecc.tell()==7
print(json.dumps({'declared_entry_end':3,'actual_ecc_cursor':ecc.tell(),'returned_hash':result[0]['hash'].decode(),'returned_ecc':result[0]['ecc'].decode(),'cross_entry_bytes_consumed':4},indent=2))
