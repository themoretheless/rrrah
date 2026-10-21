"""Original method ASTs; authored base state and hash candidates, actual NumPy."""
from pathlib import Path
import ast,numpy as np
C=Path('.dedup-work/deduplicate166-20261021/src/deduplicate_lib/plugins/duplicate_detection_algorithms')
def method(file,name):
 t=ast.parse((C/file).read_text());cl=next(n for n in t.body if isinstance(n,ast.ClassDef));m=next(n for n in cl.body if isinstance(n,ast.FunctionDef) and n.name==name);mod=ast.Module(body=[m],type_ignores=[]);ns={'np':np};exec(compile(mod,str(C/file),'exec'),ns);return ns[name]
class State:pass
f=method('distance_matrix.py','get_dataset_unique_structures')
x=State();x.pre_dda_processing=lambda:None;x.vector_count=3;x.tolerance=.5
x.distance_matrix=np.array([[0,10,10],[10,0,0],[10,0,0]],dtype=float)
assert f(x)==1 and x.unique_vector_indices.tolist()==[True,False,False]
print('PASS matrix isolatedfirst plus laterduplicatepair loses both later representatives')
x.distance_matrix=np.array([[0,0,10],[0,0,10],[10,10,0]],dtype=float)
assert f(x)==2 and x.unique_vector_indices.tolist()==[True,False,True]
print('PASS matrix samepartition reordered retains two representatives')
g=method('multi_hashing.py','duplicate_check');x=State();x.pre_dda_processing=lambda:None;x.create_hash_vector=lambda:np.array([11,22,33,44]);x.warn_if_vector_dict_mismatch=lambda h:None;x.perturbations=4;x.acceptance_threshold=.75
x.hash_dict={i:{v:{i:True}} for i,v in enumerate([11,22,33,44])}
assert g(x) is True
assert all(sum(i in d.get(v,{}) for d,v in zip(x.hash_dict.values(),[11,22,33,44]))==1 for i in range(4))
print('PASS multihash accepts collective votes from four distinct candidate indices; nonehas75percent')
