import ast,io,json,contextlib
from pathlib import Path
source=Path('.dedup-work/pyfilefixity044-20261023/pyFileFixity/structural_adaptive_ecc.py')
tree=ast.parse(source.read_text());function=next(n for n in tree.body if isinstance(n,ast.FunctionDef) and n.name=='entry_fields')
namespace={};exec(compile(ast.Module(body=[function],type_ignores=[]),'original-entry-fields','exec'),namespace)
parse=namespace['entry_fields']
raw=b'name|5|P|S|HHNEXT'
parsed=parse(io.BytesIO(raw),[0,4],b'|')
assert parsed['filesize']==5 and parsed['ecc_field_pos']==[11,4]
with contextlib.redirect_stdout(io.StringIO()): malformed=parse(io.BytesIO(b'abcdef'),[0,6],b'|')
assert isinstance(malformed['filesize'],bytes)
print(json.dumps({'short_entry_metadata':{'declared_end':4,'path':parsed['relfilepath'].decode(),'ecc_bounds':parsed['ecc_field_pos']},'missing_delimiters':{'accepted':True,'filesize_type':type(malformed['filesize']).__name__,'ecc_bounds':malformed['ecc_field_pos']}},indent=2))
