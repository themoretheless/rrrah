import ast
from pathlib import Path
root=Path('.dedup-work/fourhash424-20261021')
def cls(path,name,ns):
 node=next(x for x in ast.parse((root/path).read_text()).body if isinstance(x,ast.ClassDef) and x.name==name)
 exec(compile(ast.Module(body=[node],type_ignores=[]),path,'exec'),ns);return ns[name]
ns={x:(lambda *a,**k:None) for x in ['Counter','Timer','Percentage','Bar']};ns['ProgressBar']=lambda **kw:lambda rows:rows
C=cls('compare_thread.py','compare',ns);c=C();calls=[];c.processAll=lambda *a:calls.append(a)
row=(1,'video','0','0','0','0');c.getSimilar([row],[row]);assert len(calls)==2
print('PASS original threaded comparison dispatches sole frame pair twice')
calls.clear();c.getSimilar([row,row],[row]);assert len(calls)==4
print('PASS two candidate rows dispatch four jobs instead of two')
F=cls('database/finditems.py','finditems',{});f=F.__new__(F)
class Cursor:
 def execute(self,*a):pass
 def fetchall(self):return []
f.cursor=Cursor();assert f.findCandidateHashes('missing') is None
print('PASS empty query returns None rather than empty iterable')
try:f.findCandidateID('missing')
except UnboundLocalError:print('PASS missing candidate ID raises uninitialized local error')
else:raise AssertionError('expected missing-ID failure')
print('RESULT 4 passed; original AST classes, synthetic progress/cursor/dispatch adapters; no threads/database/video entrypoints')
