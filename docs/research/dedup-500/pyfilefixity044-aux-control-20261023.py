import io,json,sys
from pathlib import Path
root=Path('.dedup-work/pyfilefixity044-20261023').resolve()
sys.path.insert(0,str(root/'dependencies'))
sys.path.insert(0,str(root/'pyFileFixity'))
from lib import aux_funcs
class ShortReader(io.BytesIO):
    def read(self,n=-1):
        return super().read(min(n,4) if n>=0 else 4)
regular=aux_funcs.get_next_entry(io.BytesIO(b'##abcdef##ghi'),entrymarker=b'##',only_coord=False,blocksize=8)
short=aux_funcs.get_next_entry(ShortReader(b'##abcdef##ghi'),entrymarker=b'##',only_coord=False,blocksize=8)
assert regular==b'abcdef' and short==b'ab'
failures={}
for name,args in [('group_files_by_size',({'a':8,'b':4},2)),('group_files_by_size_fast',({'a':8,'b':4},2)),('group_files_by_size_simple',({'a':8,'b':4},2)),('gen_rand_fileslist',(1,))]:
    try:getattr(aux_funcs,name)(*args)
    except (NameError,AttributeError) as e:failures[name]=type(e).__name__+': '+str(e)
assert len(failures)==4
print(json.dumps({'regular_entry':regular.decode(),'short_read_entry':short.decode(),'python3_failures':failures},indent=2))
