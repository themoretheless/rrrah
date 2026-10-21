import ast,json,sys,typing
from operator import itemgetter
from pathlib import Path
class Dataset: pass
class Sampler: pass
class DistributedSampler:
 def __init__(self,dataset,**kwargs):self.dataset=dataset
 def __iter__(self):return iter(range(len(self.dataset)))
source=ast.parse(Path(sys.argv[1]).read_text())
classes=[n for n in source.body if isinstance(n,ast.ClassDef) and n.name in ['DatasetFromSampler','DistributedSamplerWrapper']]
ns={'Dataset':Dataset,'Sampler':Sampler,'DistributedSampler':DistributedSampler,'Optional':typing.Optional,'itemgetter':itemgetter}
exec(compile(ast.Module(body=classes,type_ignores=[]),str(sys.argv[1]),'exec'),ns)
for values,error in [([],TypeError),([7],TypeError),([7,9],None)]:
 wrapper=ns['DistributedSamplerWrapper'](values)
 try:
  result=list(wrapper);assert error is None;assert result==values
  print(json.dumps({'input':values,'result':result,'passed':True}))
 except TypeError as exc:
  assert error is TypeError
  print(json.dumps({'input':values,'error_type':type(exc).__name__,'error':str(exc),'passed':True}))
