"""Selected pinned AST fingerprint function; synthetic functions only."""
import ast
import hashlib
import inspect
import json
from pathlib import Path
BASE=Path(__file__).resolve().parent
EV=BASE/'evidence/Hao0321--claude-skill-code-cleanup'
source=(EV/'03-audit_core.py').read_bytes()
pin=next(e for e in json.loads((EV/'tree.json').read_text())['tree'] if e['path']=='code-cleanup-helper/scripts/audit_core.py')
assert hashlib.sha1(b'blob '+str(len(source)).encode()+b'\0'+source).hexdigest()==pin['sha']
tree=ast.parse(source)
node=next(n for n in tree.body if isinstance(n,ast.FunctionDef) and n.name=='_function_fingerprint')
namespace={'ast':ast}
exec(compile(ast.Module(body=[node],type_ignores=[]),'<pinned-function-fingerprint>','exec'),namespace)
def fingerprint(code):
    return namespace['_function_fingerprint'](ast.parse(code).body[0])[0]
def function(code,**context):
    exec(code,context)
    return context['f']
a='def f(x=1):\n return x\n'
b='def f(x=2):\n return x\n'
assert fingerprint(a)==fingerprint(b)
assert function(a)()!=function(b)()
synchronous='def f(x):\n return x\n'
asynchronous='async def f(x):\n return x\n'
assert fingerprint(synchronous)==fingerprint(asynchronous)
assert not inspect.iscoroutinefunction(function(synchronous))
assert inspect.iscoroutinefunction(function(asynchronous))
assert fingerprint('@one\n'+synchronous)==fingerprint('@two\n'+synchronous)
assert fingerprint('def renamed(x):\n "docstring"\n return x\n')==fingerprint(synchronous)
assert fingerprint('def f(y):\n return y\n')!=fingerprint(synchronous)
free='def f():\n return TOKEN\n'
assert function(free,TOKEN=1)()!=function(free,TOKEN=2)()
assert fingerprint(free)==fingerprint(free)
print(json.dumps({'git_blob_verified':True,'controls_passed':6,'different_default_behavior_same_fingerprint':True,'sync_async_same_fingerprint':True,'decorators_excluded':True,'docstring_and_name_excluded':True,'variable_renaming_changes_fingerprint':True,'global_context_not_encoded':True,'full_module_or_original_suite_run':False},indent=2))
