import ast, pathlib, types, sys, json, hashlib
source=pathlib.Path(__file__).with_name('webui.py');raw=source.read_bytes();tree=ast.parse(raw)
fn=next(n for n in tree.body if isinstance(n,ast.FunctionDef) and n.name=='api_dedup');fn.decorator_list=[]
calls=[]
class Subprocess:
 @staticmethod
 def run(*args,**kwargs):
  calls.append({'args':args,'kwargs':kwargs});return types.SimpleNamespace(returncode=1,stdout='',stderr='fixture error')
env={'subprocess':Subprocess,'sys':sys,'SCRIPT_DIR':pathlib.Path('/owned/not-executed'),'jsonify':lambda x:x}
exec(compile(ast.Module(body=[fn],type_ignores=[]),str(source),'exec'),env)
r=env['api_dedup']();assert r['ok'] is True and r['message']=='Dedup complete.'
assert '--apply' in calls[0]['args'][0]
def fail(*args,**kwargs):raise RuntimeError('fixture exception')
Subprocess.run=fail
r2=env['api_dedup']();assert r2==({'error':'fixture exception'},500)
print(json.dumps({'source_sha256':hashlib.sha256(raw).hexdigest(),'controls_passed':2,'nonzero_exit_result':r,'exception_result':r2,'call':calls,'scope':'Exact original function AST with decorators removed; subprocess recorder and jsonify identity; no Flask server or child process execution'},indent=2))
