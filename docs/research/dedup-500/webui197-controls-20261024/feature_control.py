import ast,pathlib,tempfile,json,logging,os,typing
import numpy as np
b=pathlib.Path(__file__).parent;s=b.parent/'evidence/WSH032--image-deduplicate-cluster-webui/remaining-pinned/img_dedup_clust/cluster_fn.py';t=ast.parse(s.read_text());names={'_check_error','read_wd14_npz_files','_bipolarize'};nodes=[n for n in t.body if isinstance(n,ast.FunctionDef) and n.name in names];ns={'np':np,'os':os,'logging':logging,'List':typing.List,'Union':typing.Union,'Tuple':typing.Tuple,'WD14_NPZ_ARRAY_PREFIX':'layer','_X_OUTSIDE_LAYER_INDEX':0};exec(compile(ast.Module(body=nodes,type_ignores=[]),str(s),'exec'),ns)
with tempfile.TemporaryDirectory(prefix='review197-feature-') as root:
 p=pathlib.Path(root);np.savez(p/'good.npz',layer0=np.array([.1,.2]),layer2=np.array([7.,8.]));inside,outside,errors=ns['read_wd14_npz_files'](root,['missing.npz','good.npz'],2);assert errors==[0] and inside.tolist()==[[1.,1.],[7.,8.]]
 np.savez(p/'partial.npz',layer2=np.array([3.,4.]))
 try:ns['read_wd14_npz_files'](root,['partial.npz','good.npz'],2)
 except ValueError as err:partial_error=str(err)
 else:raise AssertionError('partial missing layer unexpectedly accepted')
 assert ns['_bipolarize'](np.array([[0,0],[1,1]])).tolist()==[[1,0],[1,0]]
result={'scope':'Unchanged AST-extracted three original functions, real NumPy and owned temporary NPZs; no model/sklearn/UI.','cases':3,'findings':['Missing entire NPZ becomes all-ones row and error index returned.','NPZ containing requested inside layer but missing outside layer appends inside twice across try/except, producing ragged matrix and ValueError instead of documented replacement.','Bipolarize ties/all-zero rows choose first category by argmax.'],'partial_layer_error':partial_error}
(b/'feature-control-results.json').write_text(json.dumps(result,indent=2)+'\n');print('3 feature controls passed, including partial-NPZ failure')
