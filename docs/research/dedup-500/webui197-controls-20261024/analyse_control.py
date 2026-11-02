import ast,pathlib,json,math,logging,types
import numpy as np
b=pathlib.Path(__file__).parent;s=b.parent/'evidence/WSH032--image-deduplicate-cluster-webui/remaining-pinned/img_dedup_clust/cluster_fn.py';tree=ast.parse(s.read_text());nodes=[n for n in tree.body if isinstance(n,ast.FunctionDef) and n.name in {'cluster_analyse','cluster_analyse_exception_wrapper','aoto_set_sklearn_model_n_value'}]
class Frame:
 def __init__(self,data):self.data=data;self.loc=self
 def __getitem__(self,key):return []
class Model:
 def __init__(self,labels):self.labels=labels
 def get_params(self):return {'n_clusters':2}
 def set_params(self,**kw):return self
 def fit_predict(self,X):return np.array(self.labels)
def forbidden(*a,**kw):raise AssertionError('score should not execute')
ns={'np':np,'math':math,'logging':logging,'tqdm':lambda x:x,'pd':types.SimpleNamespace(DataFrame=Frame,concat=lambda *a,**k:Frame({})),'visualization_2D':lambda x:np.zeros((len(x),2)),'silhouette_score':forbidden,'davies_bouldin_score':forbidden}
module=ast.Module(body=[ast.ImportFrom(module='__future__',names=[ast.alias(name='annotations')],level=0)]+nodes,type_ignores=[]);exec(compile(ast.fix_missing_locations(module),str(s),'exec'),ns)
# Decorator captured original closure; test raw function and actual error wrapper separately.
wrapped=ns['cluster_analyse'];raw=next(c.cell_contents for c in wrapped.__closure__ if callable(c.cell_contents));results=[]
for name,labels in [('one-cluster',[0,0,0]),('singletons',[0,1,2])]:
 state=((np.ones((3,2)),np.ones((3,2))),['x','y'],['a','b','c'])
 try:raw(3,state,Model(labels))
 except ValueError as error:observed_error=str(error)
 else:raise AssertionError('expected log2(0)')
 assert wrapped(3,state,Model(labels))==[None,None,None,None]
 results.append({'case':name,'raw_error':observed_error,'wrapped_return':[None]*4})
(b/'analyse-control-results.json').write_text(json.dumps({'scope':'Original complete cluster_analyse and decorator AST-extracted; real NumPy/math, deterministic model labels, minimal DataFrame/visualization/progress adapters. No pandas/sklearn/model/UI runtime. Metrics forbidden sentinel confirms neither called.','cases':results},indent=2)+'\n');print('2 analysis boundary controls reproduced log2(0) and wrapper fallback')
