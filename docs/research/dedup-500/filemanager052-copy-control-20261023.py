import ast,os,types
from pathlib import Path
source=Path('/Users/themoretheless/Documents/Sources/rrrah/.dedup-work/filemanager052-20261023/commands/copy.py')
t=ast.parse(source.read_text());cls=next(x for x in t.body if isinstance(x,ast.ClassDef));fn=next(x for x in cls.body if isinstance(x,ast.FunctionDef) and x.name=='run');out=[]
env={'os':os,'sublime':types.SimpleNamespace(set_clipboard=out.append)};exec(compile(ast.fix_missing_locations(ast.Module(body=[fn],type_ignores=[])),str(source),'exec'),env)
s=types.SimpleNamespace(window=types.SimpleNamespace(folders=lambda:['/tmp/project']))
env['run'](s,'relative path',['/tmp/project-other/a']);assert out[-1]=='../project-other/a'
env['run'](s,'path from root',['/tmp/project-other/a']);assert out[-1]=='/../project-other/a'
print('original copied method accepts sibling path by substring containment; root-relative result=/../project-other/a; clipboard replaced by private list')
