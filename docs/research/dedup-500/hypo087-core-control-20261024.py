"""Unchanged pure definitions from pinned dedup module, actual NumPy."""
import ast,json,pathlib
import numpy as np
p=pathlib.Path(__file__).resolve().parents[3]/'docs/research/dedup-500/evidence/OpSafari--hypoarena/02-dedup.py'
t=ast.parse(p.read_bytes());names={'l2_normalize','cosine_similarity','cosine_matrix','union_groups'}
nodes=[x for x in t.body if isinstance(x,ast.FunctionDef) and x.name in names]
m=ast.Module(body=[ast.ImportFrom(module='__future__',names=[ast.alias(name='annotations')],level=0)]+nodes,type_ignores=[])
ns={'np':np};exec(compile(ast.fix_missing_locations(m),str(p),'exec'),ns)
zero=np.zeros(3)
assert ns['cosine_similarity'](zero,zero)==1.0
matrix=ns['cosine_matrix'](np.array([zero,zero]))
assert matrix.tolist()==[[0.,0.],[0.,0.]]
assert ns['union_groups'](['a','b','c'],[('a','b'),('b','c')])==[['a','b','c']]
assert ns['cosine_similarity'](np.array([1.,0.]),np.array([0.,1.]))==0.
print(json.dumps({'zero_pair_cosine':1.,'zero_matrix_entries':matrix.tolist(),'chain_edges_group_all_three':True,'orthogonal_pair_cosine':0.,'scope':'Original pure function definitions with NumPy, no full package imports or upstream tests. Group connectivity is not proof of all-pairs similarity.'},indent=2))
