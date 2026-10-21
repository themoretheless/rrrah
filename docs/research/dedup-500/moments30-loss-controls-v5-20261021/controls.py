import ast,json,sys
from pathlib import Path
sys.path.insert(0,str(Path('.dedup-work/immich15-deps').resolve()))
import numpy as np
class Torch:
 @staticmethod
 def norm(x,dim,keepdim):return np.linalg.norm(x,axis=dim,keepdims=keepdim)
source=ast.parse(Path(sys.argv[1]).read_text());names={'normalize_minus_one_to_one','get_flow_smoothness_loss'};nodes=[n for n in source.body if isinstance(n,ast.FunctionDef) and n.name in names];ns={'torch':Torch}
exec(compile(ast.Module(body=nodes,type_ignores=[]),sys.argv[1],'exec'),ns)
with np.errstate(divide='ignore',invalid='ignore'):
 result=ns['normalize_minus_one_to_one'](np.ones((2,2)))
assert np.isnan(result).all();print(json.dumps({'name':'constant-normalization-nonfinite','all_nan':True,'passed':True}))
a=ns['normalize_minus_one_to_one'](np.array([1.,2.,3.]));b=ns['normalize_minus_one_to_one'](np.array([12.,14.,16.]));assert np.array_equal(a,b);print(json.dumps({'name':'independent-affine-normalization-erases-scale-offset','equal':True,'passed':True}))
flow=np.array([0.,1.,4.,9.]).reshape(1,1,1,4,1);alpha=np.ones((1,1,1,4,1));actual=float(ns['get_flow_smoothness_loss'](flow,alpha));reference=9/(8+1e-6);assert np.isclose(actual,13/(8+1e-6));assert not np.isclose(actual,reference);print(json.dumps({'name':'flow-gradient-uses-last-row-not-neighbor','actual':actual,'adjacent_difference_reference':reference,'passed':True}))
