import ast, pathlib, types, os
source=pathlib.Path('.dedup-work/filemanager052-20261023/libs/input_for_path.py').read_text()
cls=next(n for n in ast.parse(source).body if isinstance(n,ast.ClassDef))
methods=[n for n in cls.body if isinstance(n,ast.FunctionDef) and n.name in {'__get_completion_for','input_on_change'}]
messages=[]
ns={'os':types.SimpleNamespace(path=os.path,listdir=lambda p:['folder','file.txt']), 'computer_friendly':lambda p:p,'user_friendly':lambda p:p,'transform_aliases':lambda w,p:p,'sublime':types.SimpleNamespace(error_message=messages.append)}
ns['os'].path=types.SimpleNamespace(sep=os.path.sep,dirname=os.path.dirname,basename=os.path.basename,join=os.path.join,isdir=lambda p:p.endswith('/folder'))
exec(compile(ast.Module(body=methods,type_ignores=[]),'<original completion methods>','exec'),ns)
obj=types.SimpleNamespace()
assert ns['__get_completion_for'](obj,'/virtual/',True,'alphabetic',False,True)==('', ['file.txt','folder/'])
try:ns['__get_completion_for'](obj,'/virtual/',True,None,False,True)
except ValueError:pass
else:raise AssertionError('null should raise')
settings={'completions':['a','b'],'completions_index':0}
obj=types.SimpleNamespace(window=None,user_on_change=None,log_in_status_bar=False,input=types.SimpleNamespace(settings=types.SimpleNamespace(get=lambda k,d=None:settings.get(k,d))))
try:ns['input_on_change'](obj,'')
except IndexError:pass
else:raise AssertionError('cleared input should raise')
print('PASS: alphabetic positive control; documented null raises ValueError; cleared input with two stored completions raises IndexError. No filesystem or UI operations.')
