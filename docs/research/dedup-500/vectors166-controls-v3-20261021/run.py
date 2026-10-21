"""Original full classes, actual NumPy, identity njit shim; not upstream pytest."""
from pathlib import Path
import sys,types,numpy as np
shim=types.ModuleType('numba');shim.njit=lambda f:f;sys.modules['numba']=shim
sys.path.insert(0,str(Path('.dedup-work/deduplicate166-20261021/src').resolve()))
from deduplicate_lib.plugins.duplicate_detection_algorithms.distance_matrix import DistanceMatrix

def make(data):
 return DistanceMatrix(tolerance=.5,dataset_array=np.array(data,dtype=float),max_vector_array_size=8)
d=make([[0.],[10.]])
assert d.get_dataset_unique_structures()==2
view=d.get_filled_dataset_array(); view[1,0]=0.
assert not d._dirty
assert d.get_dataset_unique_structures()==2
assert make(view.copy()).get_dataset_unique_structures()==1
print('PASS mutable filled dataset changes bytes without invalidating distance matrix; stale count2 versus fresh1')
d=make([[0.],[0.]])
assert d.get_dataset_unique_structures()==1
d.set_dataset_array(np.array([[0.],[10.]]))
assert d._dirty
assert d.deduplicate().tolist()==[[0.]]
assert d.get_dataset_unique_structures()==2
assert d.deduplicate().tolist()==[[0.],[10.]]
print('PASS deduplicate after same-shape dataset replacement uses old mask until explicit recomputation')
