import ast,types
from pathlib import Path
source=Path('/Users/themoretheless/Documents/Sources/rrrah/.dedup-work/filemanager052-20261023/libs/sublimefunctions.py');tree=ast.parse(source.read_text());fn=next(x for x in tree.body if isinstance(x,ast.FunctionDef) and x.name=='yes_no_cancel_panel');calls=[]
window=types.SimpleNamespace(show_quick_panel=lambda items,done,*args:calls.append(done));env={'sublime':types.SimpleNamespace(active_window=lambda:window)};exec(compile(ast.fix_missing_locations(ast.Module(body=[fn],type_ignores=[])),str(source),'exec'),env);panel=env['yes_no_cancel_panel']
try:panel('message',None,None,None)
except AttributeError:pass
else:raise AssertionError('default strings unexpectedly supported')
# Original rename No callback signature, with no supplied args.
def open_file(self):return None
panel(['message'],None,open_file,None,yes_text=['Yes'],no_text=['No'],cancel_text=['Cancel'])
try:calls[-1](2)
except TypeError:pass
else:raise AssertionError('missing self unexpectedly supplied')
print('original panel: default string labels raise AttributeError; No callback requiring self raises TypeError; no UI or files changed')
