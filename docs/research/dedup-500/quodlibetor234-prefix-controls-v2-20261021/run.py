import ast,types
from pathlib import Path
tree=ast.parse(Path('.dedup-work/quodlibetor234-20261021/distribute_setup.py').read_text())
node=next(n for n in tree.body if isinstance(n,ast.FunctionDef) and n.name=='_under_prefix')
env={'sys':types.SimpleNamespace(argv=[]),'USER_SITE':'/synthetic/user'}
exec(compile(ast.Module(body=[node],type_ignores=[]),'original-prefix','exec'),env)
def check(args,path):env['sys'].argv=['setup.py','install']+args;return env['_under_prefix'](path)
assert check(['--prefix','/synthetic/pkg'],'/synthetic/pkg/lib')
print('PASS separate prefix accepts actual child')
assert not check(['--prefix=/synthetic/pkg'],'/synthetic/pkg/lib')
print('PASS equals prefix incorrectly rejects actual child (splits root= instead of prefix=)')
assert check(['--root','/synthetic/pkg'],'/synthetic/pkg-other/lib')
print('PASS lexical startswith accepts sibling path outside requested root')
try:check(['--prefix'],'/synthetic/pkg')
except IndexError:pass
else:raise AssertionError('missing option value should reproduce IndexError')
print('PASS missing prefix value raises IndexError')
print('TOTAL4passed; original pure helper only, synthetic argv/paths; no bootstrap/download/install/extraction/deletion')
