import ast,json,contextlib,io
from pathlib import Path
from urllib.parse import urlparse
source=Path('.dedup-work/eyecatching492-20261021/eyecatching.py').read_text();tree=ast.parse(source);names={'is_valid_url','validate_url','validate_width','validate_threshold','validate_block_size'};nodes=[n for n in tree.body if isinstance(n,ast.FunctionDef) and n.name in names];ns=dict(urlparse=urlparse,exit=lambda:(_ for _ in ()).throw(SystemExit()));exec(compile(ast.Module(body=nodes,type_ignores=[]),'<original-cli-validation>','exec'),ns)
assert ns['is_valid_url']('not-a-url')=='';assert ns['validate_url']('not-a-url') is None
assert ns['validate_threshold'](3000) is None
assert ns['validate_block_size'](3000,8) is None
log=io.StringIO()
try:
 with contextlib.redirect_stdout(log):ns['validate_width'](0)
except SystemExit as e:assert e.code is None;code=e.code
else:raise AssertionError('expectedexit')
out=dict(invalid_url_accepted={'input':'not-a-url','predicate_result':'','validator_returns':None},threshold_above_hash_bits_accepted={'value':3000,'documented_max':63},block_larger_than_image_accepted={'block':3000,'image_width':8},invalid_width_success_exit={'value':0,'SystemExit_code':code,'effective_process_exit':0,'stdout':log.getvalue()},scope='Four controls call unchanged original validation functions extracted via AST; no Click CLI/backend/browser execution. Explicit exit adapter raises normal SystemExit(None).')
Path(__file__).with_name('result.json').write_text(json.dumps(out,indent=2)+'\n');print(json.dumps(out,indent=2))
