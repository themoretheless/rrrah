import ast,json,mmh3,tempfile,types,os
from pathlib import Path
b=Path('docs/research/dedup-500/evidence/Silv3rHorn--evtx2json');ns={'mmh3':mmh3,'PARSED_RECORDS':{'owned':set()}}
s=ast.parse((b/'03-evtx2json.py').read_text());f=next(n for n in s.body if isinstance(n,ast.FunctionDef) and n.name=='_isdup');exec(compile(ast.Module(body=[f],type_ignores=[]),'original-_isdup','exec'),ns)
seen={};collision=None
for i in range(250000):
 text=json.dumps({'owned_record':i});h=mmh3.hash(text)
 if h in seen and seen[h]!=text:collision=(seen[h],text,h);break
 seen[h]=text
assert collision is not None
first,second,h=collision;assert first!=second;assert ns['_isdup'](first,'owned') is False;assert ns['_isdup'](second,'owned') is True
# Repeated content and separate-channel positive controls.
ns['PARSED_RECORDS']['other']=set();assert ns['_isdup'](first,'owned') is True;assert ns['_isdup'](second,'other') is False
source=ast.parse((b/'02-evtl_selector.py').read_text());wanted=[n for n in source.body if isinstance(n,ast.FunctionDef) and n.name in ['_check_file','_validate_input']];sel={'os':os,'EVTX_HEADER':b'ElfFile\0','LOGS':[]};exec(compile(ast.Module(body=wanted,type_ignores=[]),'original-selector','exec'),sel)
with tempfile.TemporaryDirectory(prefix='evtx129-owned-') as root:
 file=Path(root)/'a.evtx';file.write_bytes(b'ElfFile\0owned');opts=types.SimpleNamespace(dir=None,file=str(file),evtxtract=False);assert sel['_validate_input'](opts);opts.file=str(Path(root)/'missing.evtx');assert sel['_validate_input'](opts) is True
out={'controls_passed':4,'scope':'Original AST _isdup and selector functions;real mmh3 5.2.0;owned synthetic text/files;no EVTX decoding or actual event data','collision':{'first':first,'second':second,'hash':h},'findings':['Different JSON records with same32bit Murmur hash suppressed as duplicates.','Repeated identical text suppressed.','Separate channel keeps independent hash sets.','Global LOGS retains prior input;second missing-file validation returns true.']};Path(__file__).with_name('results.json').write_text(json.dumps(out,indent=2)+'\n');print(json.dumps(out))
