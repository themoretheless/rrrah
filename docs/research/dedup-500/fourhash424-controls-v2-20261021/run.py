import ast
from pathlib import Path
from types import SimpleNamespace as N
root=Path('.dedup-work/fourhash424-20261021')
def cls(path,name,ns):
 tree=ast.parse((root/path).read_text());node=next(x for x in tree.body if isinstance(x,ast.ClassDef) and x.name==name)
 exec(compile(ast.Module(body=[node],type_ignores=[]),path,'exec'),ns);return ns[name]
G=cls('generate.py','generate',{});g=G('video')
assert g.is_image('FRAME.JPG','frames')=='frames/frame.jpg';print('PASS original image filter lowercases actual filename spelling')
assert g.is_image('a.jpg.backup','frames')=='frames/a.jpg.backup';print('PASS substring .jpg admits non-image backup suffix')
try:g.is_image('notes.txt','frames')
except UnboundLocalError:print('PASS non-image filename raises uninitialized local error')
else:raise AssertionError('expected original invalid-path failure')
class Cap:
 def __init__(self):self.n=0;self.open=True
 def isOpened(self):return self.open
 def read(self):self.n+=1;return True,object()
 def get(self,key):return 3 if key=='count' else self.n*1000
 def release(self):self.open=False
cv=N(CAP_PROP_FRAME_COUNT='count',CAP_PROP_POS_MSEC='time')
E=cls('frames.py','extraction',{'cv2':cv,'imutils':N(resize=lambda frame,**k:frame)})
e=E.__new__(E);e.cap=Cap();e.location='frames';saved=[];e.saveImages=lambda name,frame:saved.append(name)
e.extract();assert len(saved)==2;print('PASS declared3frame video stops after2frames (last frame omitted)')
class FailedCap(Cap):
 def read(self):return False,None
E=cls('frames.py','extraction',{'cv2':cv,'imutils':N(resize=lambda frame,**k: (_ for _ in ()).throw(ValueError('resize None')) if frame is None else frame)})
e=E.__new__(E);e.cap=FailedCap();e.location='frames';e.saveImages=lambda *a:None
try:e.extract()
except ValueError:print('PASS failed frame read still passed to resize; ret flag unchecked')
else:raise AssertionError('expected resize adapter failure')
print('RESULT 5 controls passed; original AST classes and synthetic capture/resize adapters; no video decoder/database/writer threads executed')
