import ast, pathlib, os, types
from urllib.parse import quote
from datetime import datetime
source=pathlib.Path('.dedup-work/filemanager052-20261023/libs/send2trash/plat_other.py').read_text()
names={'is_parent','format_date','info_for','trash_move'}
body=[n for n in ast.parse(source).body if isinstance(n,ast.FunctionDef) and n.name in names]
ns={'op':os.path,'text_type':str,'fsencode':os.fsencode,'quote':quote,'datetime':datetime,'FILES_DIR':b'files','INFO_DIR':b'info','INFO_SUFFIX':b'.trashinfo'}
exec(compile(ast.Module(body=body,type_ignores=[]),'<upstream functions>','exec'),ns)
assert ns['is_parent']('/tmp/volume','/tmp/volume-other/a') is True
assert 'Path=../volume-other/a\n' in ns['info_for']('/tmp/volume-other/a','/tmp/volume')
events=[]
ns['os']=types.SimpleNamespace(rename=lambda a,b:events.append(('rename',a,b)))
ns['check_create']=lambda p:None
ns['op']=types.SimpleNamespace(basename=os.path.basename,join=os.path.join,splitext=os.path.splitext,exists=lambda p:False)
def fail_open(*args,**kwargs):
    events.append(('metadata_open',args[0]))
    raise OSError('controlled metadata write failure')
ns['open']=fail_open
try: ns['trash_move'](b'/source/a',b'/trash')
except OSError: pass
else: raise AssertionError('expected failure')
assert [e[0] for e in events]==['rename','metadata_open']
print('PASS: prefix containment emits parent traversal; metadata failure occurs after recorded rename without rollback. No filesystem mutations.')
