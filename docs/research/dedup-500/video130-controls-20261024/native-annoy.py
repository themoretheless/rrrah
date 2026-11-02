import ast,typing,functools,json
from pathlib import Path
import numpy as np
from annoy import AnnoyIndex
b=Path('docs/research/dedup-500/evidence/benetech--VideoDeduplication');ns=dict(np=np,AnnoyIndex=AnnoyIndex)
def cls(file,name,ns):
 t=ast.parse((b/file).read_text());c=next(x for x in t.body if isinstance(x,ast.ClassDef) and x.name==name);exec(compile(ast.Module(body=[c],type_ignores=[]),str(b/file),'exec'),ns);return ns[name]
A=cls('deep-annoy_neighbors.py','AnnoyNNeighbors',ns);ns.update(Any=typing.Any,Dict=typing.Dict,Collection=typing.Collection,Tuple=typing.Tuple,cached_property=functools.cached_property,AnnoyNNeighbors=A);N=cls('deep-neighbors.py','NeighborMatcher',ns)
z=np.zeros(500);other=z.copy();other[0]=1
m=N({'a':z,'b':z,'far':other});hits=m.find_matches({'a':z},max_distance=0.001);assert len(hits)==1 and hits[0][:2]==('a','b')
assert m.find_matches({'a':z},max_distance=0)==[]
cap=N({f'v{i}':z for i in range(21)});limited=cap.find_matches({'query':z},max_distance=0.001);print('actual_capped_count',len(limited));assert len(limited)<21
try:A(optimize_trees=True).fit([z,z]);raise AssertionError('optimize accepted')
except TypeError as e: optimize_error=str(e)
try:m.find_matches({});raise AssertionError('empty accepted')
except ValueError:pass
out={'controls_passed':5,'scope':'Both original wrapper class bodies unchanged;actual Annoy1.17.3 native C++ index and NumPy2.5.4;500-dimensional synthetic vectors;no model or video execution','findings':['Positive close-pair detected and self/far excluded.','Threshold0 excludes exact distinct vectors.',f'Default cap/approximate index returned{len(limited)} of21 zero-distance neighbors for separate query.','optimize_trees=True raises TypeError due extra argument to __optimize__.','Empty needles raises ValueError.'],'optimize_error':optimize_error};Path(__file__).with_name('native-results.json').write_text(json.dumps(out,indent=2)+'\n');print(json.dumps(out))
