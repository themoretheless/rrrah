"""Pinned original signature arithmetic methods with real NumPy, config DTO only.

No tokenization/NLTK, stage IO, corpus, scheduler or signature-analysis kernel.
"""
import ast
import hashlib
import json
from pathlib import Path
from types import SimpleNamespace
import numpy as np

source=Path(__file__).resolve().parents[1]/'evidence/icodywu--minhash-dedup/02-minhash.py'
tree=ast.parse(source.read_text())
cls=next(n for n in tree.body if isinstance(n,ast.ClassDef) and n.name=='SignatureStage')
names={'parameters','get_signature','_global_minhash','_global_maxhash'}
cls.body=[n for n in cls.body if isinstance(n,ast.FunctionDef) and n.name in names]
cls.bases=[]
scope={'np':np,'_mersenne_prime':np.uint64((1<<61)-1)}
exec(compile(ast.Module(body=[cls],type_ignores=[]),str(source),'exec'),scope)
def stage(precision=64):
    obj=scope['SignatureStage']()
    obj.config=SimpleNamespace(seed=1,num_buckets=2,hashes_per_bucket=2,hash_config=SimpleNamespace(precision=precision,np_dtype=np.uint64 if precision==64 else np.uint32,max=np.uint64((1<<precision)-1)))
    obj.num_hashes=4
    obj._parameters=None
    return obj

s=stage();a=np.array([1,2,3,4],dtype=np.uint64).reshape(-1,1)
sig=s.get_signature(a)
assert sig==stage().get_signature(a[::-1])
assert sig==s.get_signature(np.concatenate([a,a]))
narrow=stage(32).get_signature(a)
assert len(narrow)==2 and all(len(b)==2 for b in narrow) and all(0<=x<=2**32-1 for b in narrow for x in b)
empty_error=None
try:s.get_signature(np.empty((0,1),dtype=np.uint64))
except ValueError as exc:empty_error=type(exc).__name__
assert empty_error=='ValueError'
result={'scope':__doc__,'numpy':np.__version__,'source_sha256':hashlib.sha256(source.read_bytes()).hexdigest(),'authored_controls_run':4,'observations':['fixed seed and shuffled shingle order preserve signature','duplicate shingles preserve signature','32-bit precision fits band shape and unsigned32 range','direct empty primitive raises ValueError; reviewed run() explicitly skips empty shingles, so not a demonstrated pipeline failure'],'signature':sig,'original_tests_run':0}
Path(__file__).with_name('signature-results.json').write_text(json.dumps(result,indent=2)+'\n')
print(json.dumps(result,indent=2))
