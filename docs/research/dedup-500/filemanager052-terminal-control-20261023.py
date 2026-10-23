import ast, pathlib, types
source=pathlib.Path('.dedup-work/filemanager052-20261023/commands/open_terminal.py').read_text()
cls=next(n for n in ast.parse(source).body if isinstance(n,ast.ClassDef))
method=next(n for n in cls.body if isinstance(n,ast.FunctionDef) and n.name=='open_terminal')
calls=[]
ns={'os':types.SimpleNamespace(path=types.SimpleNamespace(isfile=lambda p:False)), 'sublime':types.SimpleNamespace(status_message=lambda s:None),'user_friendly':lambda p:p,'subprocess':types.SimpleNamespace(Popen=lambda cmd,**kw:calls.append((list(cmd),kw)))}
exec(compile(ast.Module(body=[method],type_ignores=[]),'<original open_terminal>','exec'),ns)
cmd=['terminal','--working-directory=$cwd']
ns['open_terminal'](None,cmd,'/first','example')
ns['open_terminal'](None,cmd,'/second','example')
assert calls[0][0][1]=='--working-directory=/first'
assert calls[1][0][1]=='--working-directory=/first'
assert calls[1][1]['cwd']=='/second'
print('PASS: unchanged upstream method reuses first substituted directory on second invocation; subprocess replaced with recorder')
