import ast,queue,types
from pathlib import Path
import numpy as np
root=Path('/Users/themoretheless/Documents/Sources/rrrah/.dedup-work/sift497-20261023/viz/sift_viz')
def method(file,cls,name,env):
 tree=ast.parse((root/file).read_text());c=next(x for x in tree.body if isinstance(x,ast.ClassDef) and x.name==cls);f=next(x for x in c.body if isinstance(x,ast.FunctionDef) and x.name==name)
 for a in f.args.args:a.annotation=None
 f.returns=None
 mod=ast.Module(body=[f],type_ignores=[]);exec(compile(ast.fix_missing_locations(mod),str(root/file),'exec'),env);return env[name]
reader=method('player.py','VideoPlayer','_read_frames',{'queue':queue})
class SplitPipe:
 def __init__(self):self.chunks=[b'a',b'bc'];self.calls=0
 def read(self,n):self.calls+=1;return self.chunks.pop(0) if self.chunks else b''
pipe=SplitPipe();s=types.SimpleNamespace(_frame_w=1,_frame_h=1,_proc=types.SimpleNamespace(stdout=pipe),_active=True,_frame_q=queue.Queue())
reader(s,7);assert s._frame_q.get_nowait()==(7,None) and pipe.chunks==[b'bc'] and pipe.calls==1
hit=method('scatter.py','ScatterController','_hit_test_2d',{'np':np,'POINT_HIT_RADIUS_PX':12})
identity=types.SimpleNamespace(transform=lambda x:x);s=types.SimpleNamespace(_ax=types.SimpleNamespace(transData=identity));event=types.SimpleNamespace(xdata=0,ydata=0)
assert hit(s,event,np.array([[0.,0.],[0.,0.]]))==0
assert hit(s,event,np.array([[20.,0.],[30.,0.]])) is None
print('unchanged extracted methods: short read treated as EOF despite remaining frame bytes; coincident points select first index; radius rejection passed')
