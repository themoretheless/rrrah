"""Pinned quickstart dataset and call; capacity reduced to8; identity njit shim."""
from pathlib import Path
import sys,types,numpy as np
shim=types.ModuleType('numba');shim.njit=lambda f:f;sys.modules['numba']=shim
sys.path.insert(0,str(Path('.dedup-work/deduplicate166-20261021/src').resolve()))
from deduplicate_lib.plugins.duplicate_detection_algorithms.distance_matrix import DistanceMatrix
dataset=np.array([[1.,2.],[1.01,2.01],[5.,6.]])
dda=DistanceMatrix(tolerance=.1,dataset_array=dataset,max_vector_array_size=8)
try:dda.deduplicate()
except ValueError as e:
 assert 'Unique vector indices array shape does not match dataset' in str(e)
 print('PASS documented fresh batch deduplicate call raises stale/uninitialized mask ValueError')
else:raise AssertionError('expected missing mask')
assert dda.get_dataset_unique_structures()==2
assert dda.deduplicate().tolist()==[[1.,2.],[5.,6.]]
print('PASS explicit unique-mask computation repairs documented batch flow on this dataset')
