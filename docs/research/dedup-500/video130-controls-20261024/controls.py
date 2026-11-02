import ast,json,typing,functools
from pathlib import Path
import numpy as np
b=Path('docs/research/dedup-500/evidence/benetech--VideoDeduplication')
class ExactFixtureNeighbors:
 def __init__(self,n_neighbors,metric):self.n=n_neighbors
 def fit(self,vs):self.vs=np.array(vs)
 def kneighbors(self,vs):
  distances=np.linalg.norm(np.array(vs)[:,None,:]-self.vs[None,:,:],axis=2);indices=np.argsort(distances,axis=1,kind='stable')[:,:self.n];return np.take_along_axis(distances,indices,axis=1),indices
ns=dict(Any=typing.Any,Dict=typing.Dict,Collection=typing.Collection,Tuple=typing.Tuple,cached_property=functools.cached_property,AnnoyNNeighbors=ExactFixtureNeighbors)
tree=ast.parse((b/'deep-neighbors.py').read_text());cls=next(x for x in tree.body if isinstance(x,ast.ClassDef));exec(compile(ast.Module(body=[cls],type_ignores=[]),'original-NeighborMatcher','exec'),ns);N=ns['NeighborMatcher']
try:N({});raise AssertionError('empty accepted')
except ValueError:pass
m=N({'a':[0.,0.],'b':[0.,0.]});assert m.find_matches({'a':[0.,0.]},max_distance=0)==[]
positive=m.find_matches({'a':[0.,0.],'b':[0.,0.]},max_distance=0.001);assert len(positive)==1;assert positive[0][0]!=positive[0][1];assert positive[0][2]==0
ns2={};tree=ast.parse((b/'deep-winnow_utils_matches.py').read_text());f=next(x for x in tree.body if isinstance(x,ast.FunctionDef) and x.name=='unique');exec(compile(ast.Module(body=[f],type_ignores=[]),'original-legacy-unique','exec'),ns2);assert ns2['unique']({'query':1,'match':234})==ns2['unique']({'query':12,'match':34})
out={'controls_passed':4,'scope':'Original NeighborMatcher class with explicit deterministic exact-distance fixture replacing Annoy backend;real NumPy2.5.4;legacy unique function original AST. No neural model,Annoy index,video decoding,DB or full pipeline execution.','findings':['Empty haystack constructor raises ValueError.','Zero-distance threshold excludes identical distinct vectors because comparison is strict.','Positive threshold returns one symmetric-deduplicated pair and skips self.','Legacy delimiter-free unique aliases pair(1,234) and(12,34);current local matcher uses tuple keys instead.']};Path(__file__).with_name('results.json').write_text(json.dumps(out,indent=2)+'\n');print(json.dumps(out))
