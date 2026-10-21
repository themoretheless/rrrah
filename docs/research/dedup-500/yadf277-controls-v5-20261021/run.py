"""Original auxiliary Python script, authored temporary bytes, no cleaner execution."""
import importlib.util,contextlib,io,json,hashlib,tempfile
from pathlib import Path
p=Path('.dedup-work/yadf277-20261021/scripts/yadf.py');spec=importlib.util.spec_from_file_location('upstream_yadf',p);u=importlib.util.module_from_spec(spec);spec.loader.exec_module(u)
out=io.StringIO()
with contextlib.redirect_stdout(out):u.ldjson([['a','b'],['c','d']])
assert out.getvalue()=='["a", "b"]["c", "d"]'
try:json.loads(out.getvalue())
except json.JSONDecodeError:pass
else:raise AssertionError('expected missing record separators')
print('PASS auxiliary ldjson concatenates groups without line separators')
with tempfile.TemporaryDirectory() as td:
 p=Path(td)/'bytes';data=b'\x00\xffabc\n'*2000;p.write_bytes(data)
 digest,path=u.hash_file(str(p),hashlib.sha384)
 assert digest==hashlib.sha384(data).digest() and path==str(p)
 print('PASS auxiliary streaming hash matches independent whole-buffer SHA384')
