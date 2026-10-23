import ast,json
from pathlib import Path
source=Path('.dedup-work/pyfilefixity044-20261023/pyFileFixity/header_ecc.py')
tree=ast.parse(source.read_text());function=next(n for n in tree.body if isinstance(n,ast.FunctionDef) and n.name=='entry_assemble')
namespace={'_izip':zip,'_range':range}
exec(compile(ast.Module(body=[function],type_ignores=[]),'original-header-entry-assemble','exec'),namespace)
assemble=namespace['entry_assemble'];params={'message_size':5,'hash_size':2,'ecc_size':3}
empty=assemble({'ecc_field':b''},params,10,'',b'abcdefghij')
short=assemble({'ecc_field':b'HH123'},params,10,'',b'abcdefghij')
assert empty==[] and len(short)==1 and short[0]['message']==b'abcde'
print(json.dumps({'source_header_bytes':10,'empty_track_blocks':len(empty),'one_block_track_source_bytes_covered':sum(len(x['message']) for x in short),'remaining_unreported_source_bytes':5},indent=2))
